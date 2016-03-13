//! An SPSC queue implemented internally as a sequence of SPSC buffers.
//!
//! This queue will allocate new buffers indefinitely and eat up memory if the receiver doesn't
//! keep up. Performance is better if the receiver keeps up as the allocator will likely reuse
//! the same set of memory for each buffer.
//!
//! Because of TSO on x86, the store order by the sender means that the receiver can load values
//! from the buffer without worrying that it'll read invalid data ahead of the sender.
//! On other architectures, we use atomics with the associated performance penalty.


use std::cell::Cell;
use std::intrinsics::{needs_drop, abort};
use std::mem::{align_of, size_of};
use std::ptr::{null_mut, read, write, Unique};
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
use std::sync::atomic::AtomicUsize;

extern crate alloc;
use self::alloc::heap::{allocate, deallocate};

use constants::CACHE_LINE;


/// TSO means that we don't need atomics on x86 and that will speed things up.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
struct MaybeAtomicUsize {
    value: Cell<usize>,
}


/// On weaker memory model platforms, default to atomics.
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
struct MaybeAtomicUsize {
    value: AtomicUsize,
}


/// A one-shot spsc buffer: once it's full and has been read, it is disposed of and a new Buffer<T>
/// is allocated.
struct Buffer<T> {
    data: Unique<T>,

    capacity: usize,

    head: MaybeAtomicUsize,

    _cachepadding: [u8; CACHE_LINE],

    tail: MaybeAtomicUsize,
    tail_max: MaybeAtomicUsize,

    next: AtomicPtr<Buffer<T>>,
}


/// Since the buffers are linked together by raw pointers, this struct assumes ownership of that
/// unsafe relationship, presenting it as safe.
struct BufferQueue<T> {
    // this pointer is only accessed by the Receiver
    head: Cell<*mut Buffer<T>>,

    _cachepadding: [u8; CACHE_LINE],

    // this pointer is only accessed by the Sender
    tail: Cell<*mut Buffer<T>>,
    // this value only written once by the Sender, read by the Receiver
    hup: Cell<bool>,
}


/// An iterator type that iters until the receiver returns empty.
pub struct EmptyIter<'a, T: 'a> {
    receiver: &'a mut Receiver<T>,
}


/// Similar to std::sync::mpsc::TryRecvError
pub enum RecvResult {
    Empty,
    Disconnected,
}


/// A journal reader type which can be sent to another thread
pub struct Receiver<T> {
    buffer: Arc<BufferQueue<T>>,
}


/// A journal writer type which can be sent to another thread
pub struct Sender<T> {
    buffer: Arc<BufferQueue<T>>,
}


unsafe impl<T> Send for Sender<T> {}
unsafe impl<T> Send for Receiver<T> {}


impl<T> BufferQueue<T> {
    fn new(capacity: usize) -> BufferQueue<T> {
        let first_buffer = Box::new(Buffer::new(capacity));
        let ptr = Box::into_raw(first_buffer);

        BufferQueue {
            head: Cell::new(ptr),
            _cachepadding: [0; CACHE_LINE],
            tail: Cell::new(ptr),
            hup: Cell::new(false),
        }
    }

    /// use by Sender only
    fn tail(&self) -> *mut Buffer<T> {
        self.tail.get()
    }

    /// use by Receiver only
    fn head(&self) -> *mut Buffer<T> {
        self.head.get()
    }

    /// use by Receiver only
    fn replace_head(&self, next_head: *mut Buffer<T>) {
        unsafe { Box::from_raw(self.head.get()) };
        self.head.set(next_head);
    }

    /// use by Receiver only
    fn head_is_completed(&self) -> bool {
        unsafe { &*self.head() }.is_completed()
    }

    /// use by Receiver only
    fn next_head(&self) -> Option<*mut Buffer<T>> {
        unsafe { &*self.head() }.next_buffer()
    }
}


impl<T> Drop for BufferQueue<T> {
    /// Drop all unread buffers.
    fn drop(&mut self) {
        let mut head = Some(self.head.get());

        loop {
            let mut next = None;
            if let Some(head) = head {
                next = unsafe { &*head }.next_buffer();

                unsafe {
                    let owned = Box::from_raw(head);
                    drop(owned);
                };

                if let None = next {
                    break;
                }
            }
            head = next;
        }
    }
}


impl<T> Sender<T> {
    /// Send a value to the Receiver. TODO this should probably return some kind of error on
    /// receiver hup.
    pub fn send(&self, item: T) {
        let result = unsafe { &*self.buffer.tail() }.write(item);

        if let Some(new_tail) = result {
            self.buffer.tail.set(new_tail);
        }
    }
}


impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // mark the last buffer as completed and set the HUP flag
        unsafe { &*self.buffer.tail() }.mark_completed();
        self.buffer.hup.set(true);
    }
}


impl<T> Receiver<T> {
    /// Read a value from the queue if there is one available, otherwise return without blocking
    pub fn try_recv(&self) -> Result<T, RecvResult> {
        let head = unsafe { &*self.buffer.head() };
        let result = head.try_read();

        match result {
            Some(value) => Ok(value),

            None => {
                // is this buffer completed by the sender?
                if self.buffer.head_is_completed() {

                    // is there a next buffer?
                    if let Some(next_head) = self.buffer.next_head() {
                        self.buffer.replace_head(next_head);

                        // peek at next buffer for a value befure returning empty
                        let new_head = unsafe { &*self.buffer.head() };
                        if let Some(value) = new_head.try_read() {
                            Ok(value)
                        } else {
                            Err(RecvResult::Empty)
                        }

                    } else {
                        // no further buffer, did we get hung-up on?
                        if self.buffer.hup.get() {
                            Err(RecvResult::Disconnected)
                        } else {
                            Err(RecvResult::Empty)
                        }
                    }
                } else {
                    Err(RecvResult::Empty)
                }
            }
        }
    }


    /// Make an Iterator that returns values until the queue is empty or disconnected.
    pub fn iter_until_empty(&mut self) -> EmptyIter<T> {
        EmptyIter { receiver: self }
    }

    /// Has the Sender hung up?
    pub fn is_disconnected(&self) -> bool {
        if self.buffer.hup.get() {
            if let None = self.buffer.next_head() {
                return unsafe { &*self.buffer.head() }.is_empty();
            }
        }

        false
    }
}


/// Return a Sender/Receiver pair that can be handed over to other threads. The capacity is the
/// requested size of each internal buffer and will be rounded to the next power of two.
pub fn make_journal<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let buffer = Arc::new(BufferQueue::new(capacity));

    (Sender { buffer: buffer.clone() },
     Receiver { buffer: buffer })
}


impl<T> Buffer<T> {
    /// Create a new Buffer<T> instance, rounding the capacity up to the nearest power of two.
    fn new(requested_capacity: usize) -> Buffer<T> {
        let rounded_capacity = requested_capacity.next_power_of_two();

        let data = unsafe {
            let array = allocate(rounded_capacity * size_of::<T>(), align_of::<T>());
            if array.is_null() {
                abort()
            };
            Unique::new(array as *mut T)
        };

        Buffer {
            data: data,
            capacity: rounded_capacity,
            head: MaybeAtomicUsize::new(0),
            _cachepadding: [0; CACHE_LINE],
            tail: MaybeAtomicUsize::new(0),
            tail_max: MaybeAtomicUsize::new(rounded_capacity as usize),
            next: AtomicPtr::new(null_mut()),
        }
    }

    /// Write to the buffer, returning Some(new_buffer) if the current one was full.
    fn write(&self, item: T) -> Option<*mut Buffer<T>> {
        let tail = self.tail.load(Ordering::Relaxed);

        if tail < self.tail_max.load(Ordering::Relaxed) {
            // write to this buffer
            unsafe { write(self.data.offset(tail as isize), item) };
            self.tail.fetch_add(1, Ordering::Release);
            None
        } else {
            // allocate a new buffer and write to that
            let buffer = Box::new(Buffer::new(self.capacity));
            buffer.write(item);

            // save the pointer to the new buffer for the receiver
            let ptr = Box::into_raw(buffer);
            self.next.store(ptr, Ordering::Release);

            Some(ptr)
        }
    }

    /// Read the next item from the buffer, returning None if the buffer is full or if the contents
    /// thus far have been consumed.
    fn try_read(&self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);

        if head < self.tail.load(Ordering::Acquire) {
            // read from this buffer
            let item = unsafe { read(self.data.offset(head as isize)) };
            self.head.fetch_add(1, Ordering::Relaxed);
            Some(item)
        } else {
            None
        }
    }

    /// Check the completion status.
    fn is_completed(&self) -> bool {
        self.tail_max.load(Ordering::Relaxed) == self.tail.load(Ordering::Acquire)
    }

    /// Mark this buffer as full.
    fn mark_completed(&self) {
        self.tail_max.store(self.tail.load(Ordering::Relaxed), Ordering::Relaxed);
    }

    /// Check for contents.
    fn is_empty(&self) -> bool {
        self.head.load(Ordering::Relaxed) == self.tail_max.load(Ordering::Relaxed)
    }

    /// Fetch the pointer to the next buffer if the Sender has written one.
    fn next_buffer(&self) -> Option<*mut Buffer<T>> {
        let ptr = self.next.load(Ordering::Acquire);

        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
}


impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        unsafe {
            // pop any remaining items if they need to be officially dropped
            if needs_drop::<T>() {
                loop {
                    match self.try_read() {
                        None => break,
                        _ => (),
                    }
                }
            }

            deallocate(self.data.get_mut() as *mut T as *mut u8,
                       self.capacity * size_of::<T>(),
                       align_of::<T>());
        }
    }
}


impl<'a, T> Iterator for EmptyIter<'a, T> {
    type Item = T;

    /// Ignores disconnected state
    fn next(&mut self) -> Option<Self::Item> {
        if let Ok(item) = self.receiver.try_recv() {
            Some(item)
        } else {
            None
        }
    }
}


#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl MaybeAtomicUsize {
    fn new(value: usize) -> MaybeAtomicUsize {
        MaybeAtomicUsize { value: Cell::new(value) }
    }

    #[inline]
    fn load(&self, _ordering: Ordering) -> usize {
        self.value.get()
    }

    #[inline]
    fn store(&self, value: usize, _ordering: Ordering) {
        self.value.set(value);
    }

    #[inline]
    fn fetch_add(&self, value: usize, _ordering: Ordering) -> usize {
        let old = self.value.get();
        self.value.set(old + value);
        old
    }
}


#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
impl MaybeAtomicUsize {
    fn new(value: usize) -> MaybeAtomicUsize {
        MaybeAtomicUsize { value: AtomicUsize::new(value) }
    }

    #[inline]
    fn load(&self, ordering: Ordering) -> usize {
        self.value.load(ordering)
    }

    #[inline]
    fn store(&self, value: usize, ordering: Ordering) {
        self.value.store(value, ordering);
    }

    #[inline]
    fn fetch_add(&self, value: usize, ordering: Ordering) -> usize {
        self.value.fetch_add(value, ordering)
    }
}


#[cfg(test)]
mod tests {

    use super::{make_journal, RecvResult};


    const TEST_COUNT: usize = 12345;
    const TEST_BUFFER_SIZE: usize = 32;


    #[test]
    fn test_rx_tx() {
        let (tx, rx) = make_journal::<usize>(TEST_BUFFER_SIZE);

        for i in 0..TEST_COUNT {
            tx.send(i);

            let mut value = None;

            while let None = value {
                match rx.try_recv() {
                    Ok(packet) => {
                        assert!(packet == i);
                        value = Some(packet);
                    }

                    // may get Empty on transitioning from one buffer to the next
                    Err(RecvResult::Empty) => continue,
                    Err(RecvResult::Disconnected) => assert!(false),
                }
            }
        }
    }

    #[test]
    fn test_disconnect() {
        let (tx, rx) = make_journal::<usize>(TEST_BUFFER_SIZE);

        drop(tx);

        match rx.try_recv() {
            Err(RecvResult::Disconnected) => (),
            _ => assert!(false),
        }
    }

    #[test]
    fn test_running_disconnect_tx() {
        let (tx, rx) = make_journal::<usize>(TEST_BUFFER_SIZE);

        // buffer up some values
        for i in 0..TEST_COUNT {
            tx.send(i);
        }

        drop(tx);

        // should still be able to receive all buffered values
        for i in 0..TEST_COUNT {
            let mut value = None;

            while let None = value {
                match rx.try_recv() {
                    Ok(packet) => {
                        assert!(packet == i);
                        value = Some(packet);
                    }

                    // may get Empty on transitioning from one buffer to the next
                    Err(RecvResult::Empty) => continue,
                    Err(RecvResult::Disconnected) => assert!(false),
                }
            }
        }

        // should be disconnected
        match rx.try_recv() {
            Err(RecvResult::Disconnected) => (),
            _ => assert!(false),
        }
    }

    #[test]
    fn test_disconnect_rx() {
        let (tx, rx) = make_journal::<usize>(TEST_BUFFER_SIZE);

        drop(rx);

        tx.send(42);

        // TODO: tx.send() should return a Result with a disconnected status
    }
}
