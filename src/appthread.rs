//! Types for the mutator to use to build data structures


use std::cell::Cell;
use std::mem::transmute;
use std::ops::{Deref, DerefMut};
use std::ptr::{null, null_mut};
use std::raw::TraitObject;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::thread;

use constants::{INC_BIT, JOURNAL_BUFFER_SIZE, NEW_BIT, TRAVERSE_BIT};
use gcthread::{JournalSender, EntrySender};
use heap::{Object, TraceStack};
use journal;
use trace::Trace;


/// Each thread gets it's own EntrySender
thread_local!(
    static GC_JOURNAL: Cell<*const EntrySender> = Cell::new(null())
);


/// GcBox struct and traits: a boxed object that is GC managed
pub struct GcBox<T: Trace> {
    value: T,
}


/// Root smart pointer, sends reference count changes to the journal.
///
/// Whenever a reference to an object on the heap must be retained on the stack, this type must be
/// used. It's use will ensure that the object will be seen as a root.
pub struct GcRoot<T: Trace> {
    ptr: *mut GcBox<T>,
}


/// Non-atomic pointer type. This type is `!Sync` and thus is useful for presenting a Rust-ish
/// API to a data structure where aliasing and mutability must follow the standard rules: there
/// can be only one mutator.
///
/// *Important note:* even though this type is `!Sync`, any data structures that are composed of
/// `Gc` pointers must still be designed with the awareness that the GC thread will call `trace()`
/// at any point and so, must still be thread safe!
///
/// This is not a root pointer type. It should be used inside data structures to reference other
/// GC-managed objects.
pub struct Gc<T: Trace> {
    ptr: *mut GcBox<T>,
}


/// Atomic pointer type that points at a traceable object. This type is `Sync` and can be used to
/// build concurrent data structures.
///
/// This type should be used inside data structures to reference other GC-managed objects, but
/// provides interior mutability and atomic methods.
///
/// TODO: cas, swap etc for GcRoot and Gc
pub struct GcAtomic<T: Trace> {
    ptr: AtomicPtr<GcBox<T>>,
}


/// An Application Thread, manages a thread-local reference to a tx channel
///
/// TODO: a version of `spawn()` is required that can be called from an existing mutator thread.
pub struct AppThread;


impl AppThread {
    /// As thread::spawn but takes a journal Sender to initialize the thread_local instance with.
    pub fn spawn_from_gc<F, T>(tx: JournalSender, f: F) -> thread::JoinHandle<T>
        where F: FnOnce() -> T,
              F: Send + 'static,
              T: Send + 'static
    {
        thread::spawn(move || {
            let (jtx, jrx) = journal::make_journal(JOURNAL_BUFFER_SIZE);

            tx.send(jrx).expect("Failed to send a new Journal to the GC thread!");

            GC_JOURNAL.with(|j| {
                j.set(&jtx);
            });

            f()
        })
    }
}

// Reference count functions. Only new-objects need to specify the traverse bit.

#[inline]
fn as_traitobject<T: Trace>(object: &T) -> TraitObject {
    let trace: &Trace = object;
    unsafe { transmute(trace) }
}


/// Write a reference count increment to the journal for a newly allocated object
#[inline]
fn write<T: Trace>(object: &T, is_new: bool, flags: usize) {
    GC_JOURNAL.with(|j| {
        let tx = unsafe { &*j.get() };

        let tobj = as_traitobject(object);

        // set the refcount-increment bit
        let ptr = (tobj.data as usize) | flags;

        // set the traversible bit
        let mut vtable = tobj.vtable as usize;
        if is_new && object.traversible() {
            vtable |= TRAVERSE_BIT;
        }

        tx.send(Object {
            ptr: ptr,
            vtable: vtable,
        });
    });
}

// GcBox implementation

impl<T: Trace> GcBox<T> {
    fn new(value: T) -> GcBox<T> {
        GcBox {
            value: value,
        }
    }
}


unsafe impl<T: Trace> Trace for GcBox<T> {
    #[inline]
    fn traversible(&self) -> bool {
        self.value.traversible()
    }

    #[inline]
    unsafe fn trace(&self, heap: &mut TraceStack) {
        self.value.trace(heap);
    }
}

// GcRoot implementation

impl<T: Trace> GcRoot<T> {
    /// Put a new object on the heap and hand ownership to the GC, writing a reference count
    /// increment to the journal.
    pub fn new(value: T) -> GcRoot<T> {
        let boxed = Box::new(GcBox::new(value));
        write(&*boxed, true, NEW_BIT | INC_BIT);

        GcRoot {
            ptr: Box::into_raw(boxed)
        }
    }

    fn from_raw(ptr: *mut GcBox<T>) -> GcRoot<T> {
        let root = GcRoot { ptr: ptr };
        write(&*root, false, INC_BIT);
        root
    }

    fn ptr(&self) -> *mut GcBox<T> {
        self.ptr
    }

    fn value(&self) -> &T {
        unsafe { &(*self.ptr).value }
    }

    fn value_mut(&mut self) -> &mut T {
        unsafe { &mut (*self.ptr).value }
    }
}


impl<T: Trace> Drop for GcRoot<T> {
    fn drop(&mut self) {
        write(&**self, false, 0);
    }
}


impl<T: Trace> Deref for GcRoot<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value()
    }
}


impl<T: Trace> DerefMut for GcRoot<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value_mut()
    }
}


impl<T: Trace> Clone for GcRoot<T> {
    fn clone(&self) -> Self {
        GcRoot::from_raw(self.ptr())
    }
}

// Gc implementation

impl<T: Trace> Gc<T> {
    /// Creates a new null pointer.
    pub fn null() -> Gc<T> {
        Gc {
            ptr: null_mut(),
        }
    }

    /// Move a value to the heap and create a pointer to it.
    pub fn new(value: T) -> Gc<T> {
        let boxed = Box::new(GcBox::new(value));
        write(&*boxed, true, NEW_BIT);

        Gc {
            ptr: Box::into_raw(boxed)
        }
    }

    /// Return the raw pointer value, or None if it is a null pointer.
    pub fn as_raw(&self) -> Option<*mut GcBox<T>> {
        if self.ptr.is_null() {
            None
        } else {
            Some(self.ptr)
        }
    }

    /// Pointer equality comparison.
    pub fn is(&self, other: Gc<T>) -> bool {
        self.ptr == other.ptr
    }

    fn from_raw(ptr: *mut GcBox<T>) -> Gc<T> {
        Gc {
            ptr: ptr,
        }
    }

    fn ptr(&self) -> *mut GcBox<T> {
        self.ptr
    }

    fn value(&self) -> &T {
        unsafe { &(*self.ptr).value }
    }

    fn value_mut(&mut self) -> &mut T {
        unsafe { &mut (*self.ptr).value }
    }
}


impl<T: Trace> Deref for Gc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value()
    }
}


impl<T: Trace> DerefMut for Gc<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value_mut()
    }
}


impl<T: Trace> Clone for Gc<T> {
    fn clone(&self) -> Self {
        Gc {
            ptr: self.ptr,
        }
    }
}


impl<T: Trace> Copy for Gc<T> {}

// GcAtomic implementation

impl<T: Trace> GcAtomic<T> {
    /// Instantiate a new null pointer
    pub fn null() -> GcAtomic<T> {
        GcAtomic {
            ptr: AtomicPtr::new(null_mut())
        }
    }

    /// Instantiate a new pointer, moving `value` to the heap. Writes to the journal.
    pub fn new(value: T) -> GcAtomic<T> {
        let boxed = Box::new(GcBox::new(value));
        write(&*boxed, true, NEW_BIT);

        GcAtomic {
            ptr: AtomicPtr::new(Box::into_raw(boxed)),
        }
    }

    /// Root the pointer by loading it into a `GcRoot<T>`
    ///
    /// Panics if `order` is `Release` or `AcqRel`.
    pub fn load_into_root(&self, order: Ordering) -> GcRoot<T> {
        let root = GcRoot {
            ptr: self.ptr.load(order),
        };

        write(&*root, false, INC_BIT);
        root
    }

    /// Copy the pointer into a new `Gc` instance.
    ///
    /// Panics if `order` is `Release` or `AcqRel`.
    pub fn load_into_gc(&self, order: Ordering) -> Gc<T> {
        Gc::from_raw(self.ptr.load(order))
    }

    /// Fetch the current raw pointer value
    ///
    /// Panics if `order` is `Release` or `AcqRel`.
    pub fn load_raw(&self, order: Ordering) -> *mut GcBox<T> {
        self.ptr.load(order)
    }

    /// Replace the current pointer value with the pointer from the given `GcRoot`.
    ///
    /// Panics if `order` is `Acquire` or `AcqRel`.
    pub fn store_from_root(&self, root: GcRoot<T>, order: Ordering) {
        self.ptr.store(root.ptr(), order);
    }

    /// Replace the current pointer value with the pointer from the given `Gc`.
    ///
    /// Panics of `order` is `Acquire` or `AcqRel`.
    pub fn store_from_gc(&self, gc: Gc<T>, order: Ordering) {
        self.ptr.store(gc.ptr(), order);
    }

    /// Replace the current pointer value with the given raw pointer
    ///
    /// Panics if `order` is `Acquire` or `AcqRel`.
    pub fn store_raw(&self, ptr: *mut GcBox<T>, order: Ordering) {
        self.ptr.store(ptr, order);
    }
}
