//! Core heap traits and data types
//!
//! TODO: RootMeta and ObjectMeta have some things in common, perhaps use traits to abstract
//! the differences, then perhaps YoungHeap and ParHeap can share more code?


use std::cell::Cell;
use std::mem::transmute;
use std::raw::TraitObject;
use std::sync::atomic::{AtomicUsize, Ordering};

use bitmaptrie::Trie;
use scoped_pool::Pool;

use constants::{MARK_BIT, MARK_MASK, NEW_BIT, NEW_MASK, PTR_MASK, TRAVERSE_BIT};
use gcthread::ptr_shift;
use trace::Trace;


pub type ObjectBuf = Vec<Object>;
pub type RootMap = Trie<RootMeta>;
pub type HeapMap = Trie<ObjectMeta>;


/// A trait that describes Trace operations on a Heap
pub trait TraceOps {
    /// Buffer the given object for future tracing on the trace stack. This method should be called
    /// by objects that implement the Trace trait, from the Trace::trace() method.
    fn push_to_trace(&mut self, object: &Trace);
}


/// A trait that describes collection operations on a Heap
pub trait CollectOps {
    /// Add an object directly to the heap.
    fn add_object(&mut self, ptr: usize, vtable: usize);

    /// Run a collection iteration on the heap. Return the total heap size and the number of
    /// dropped objects.
    fn collect(&mut self, thread_pool: &mut Pool, roots: &mut RootMap) -> (usize, usize);
}


/// A journal item. Essentially just a Send-able TraitObject
#[derive(Copy, Clone)]
pub struct Object {
    pub ptr: usize,
    pub vtable: usize,
}


/// Root pointer metadata
pub struct RootMeta {
    /// the root reference count. This gets decremented by multiple threads and thus must be
    /// thread safe.
    pub refcount: AtomicUsize,
    /// the Trace trait vtable pointer
    pub vtable: usize,
    /// bits for flags
    pub flags: Cell<usize>,
}


/// A GC-managed pointer's metadata
pub struct ObjectMeta {
    /// Using bit 0 as the mark bit (MARK_BIT)
    /// Using bit 1 to indicate traversibility (TRAVERSE_BIT)
    /// Normally we'd use an AtomicUsize, but since the operations on the value are one-way,
    /// i.e. setting a mark bit in parallel, or unsetting it in parallel, we don't need to worry
    /// about data races. The worst that will happen is that two threads will try to trace the
    /// same object concurrently.
    pub vtable: Cell<usize>,
}


/// A type that contains a stack of objects to trace into. This type is separated out from the
/// main Heap type so that different collection strategies can be implemented without affecting
/// the client code. The `Trace` trait depends only this type, then, and not the whole Heap
/// type.
pub struct TraceStack {
    stack: ObjectBuf,
}


unsafe impl Send for Object {}

unsafe impl Send for RootMeta {}
unsafe impl Sync for RootMeta {}

unsafe impl Send for ObjectMeta {}
// We're using a Cell and not an Atomic in ObjectMeta but that is ok for how we are using it.
unsafe impl Sync for ObjectMeta {}


impl Object {
    pub fn from_trie_ptr(ptr: usize, vtable: usize) -> Object {
        Object {
            ptr: ptr << ptr_shift(),
            vtable: vtable,
        }
    }

    // Return this object as a Trace trait object reference
    pub fn as_trace(&self) -> &Trace {
        let tobj: TraitObject = Object::into(*self);
        unsafe { transmute(tobj) }
    }
}


impl From<TraitObject> for Object {
    fn from(tobj: TraitObject) -> Object {
        Object {
            ptr: tobj.data as usize,
            vtable: tobj.vtable as usize,
        }
    }
}


impl Into<TraitObject> for Object {
    fn into(self) -> TraitObject {
        TraitObject {
            data: self.ptr as *mut (),
            // make sure traverse and mark bits are cleared
            vtable: (self.vtable & PTR_MASK) as *mut (),
        }
    }
}


impl RootMeta {
    pub fn new(refcount: usize, vtable: usize, flags: usize) -> RootMeta {
        RootMeta {
            refcount: AtomicUsize::new(refcount),
            vtable: vtable,
            flags: Cell::new(flags),
        }
    }

    // Initialize with a reference count of 1
    pub fn one(vtable: usize, flags: usize) -> RootMeta {
        Self::new(1, vtable, flags)
    }

    // Initialize with a reference count of 0
    pub fn zero(vtable: usize, flags: usize) -> RootMeta {
        Self::new(0, vtable, flags)
    }

    // Increment the reference count by 1
    #[inline]
    pub fn inc(&self) {
        self.refcount.fetch_add(1, Ordering::SeqCst);
    }

    // Decrement the reference count by 1
    #[inline]
    pub fn dec(&self) {
        self.refcount.fetch_sub(1, Ordering::SeqCst);
    }

    // Increment the reference count by 1, thread unsafe
    #[inline]
    pub fn unsync_inc(&self) {
        let refcount = self.unsync_refcount();
        refcount.set(refcount.get() + 1);
    }

    // Decrement the reference count by 1, thread unsafe
    #[inline]
    pub fn unsync_dec(&self) {
        let refcount = self.unsync_refcount();
        refcount.set(refcount.get() - 1);
    }

    // Return true if this object has a zero reference count, thread unsafe
    #[inline]
    pub fn unsync_is_unrooted(&self) -> bool {
        let refcount = self.unsync_refcount();
        refcount.get() == 0
    }

    // Return true if this is a new object
    #[inline]
    pub fn is_new(&self) -> bool {
        self.flags.get() & NEW_BIT != 0
    }

    // Return true if this is a new object and the mark bit is unset
    #[inline]
    pub fn is_new_and_unmarked(&self) -> bool {
        self.flags.get() & (MARK_BIT | NEW_BIT) == NEW_BIT
    }

    #[inline]
    pub fn set_not_new(&self) {
        self.flags.set(self.flags.get() & NEW_MASK);
    }

    // Mark this object and return true if it needs to be traced into
    #[inline]
    pub fn mark_and_needs_trace(&self) -> bool {
        let flags = self.flags.get();

        let was_unmarked = flags & MARK_BIT == 0;
        if was_unmarked {
            self.flags.set(flags | MARK_BIT);
        }

        was_unmarked && flags & TRAVERSE_BIT != 0
    }

    // Reset the mark bit back to 0
    #[inline]
    pub fn unmark(&self) {
        self.flags.set(self.flags.get() & MARK_MASK);
    }

    // Returns the vtable without any flags set
    #[inline]
    pub fn vtable(&self) -> usize {
        self.vtable & PTR_MASK
    }

    // oh the horror, to save a few clock cycles
    #[inline]
    fn unsync_refcount(&self) -> &Cell<usize> {
        let refcount: &Cell<usize> = unsafe { transmute(&self.refcount) };
        refcount
    }
}


impl ObjectMeta {
    pub fn new(vtable: usize) -> ObjectMeta {
        ObjectMeta { vtable: Cell::new(vtable) }
    }

    // Mark this object and return true if it needs to be traced into
    #[inline]
    pub fn mark_and_needs_trace(&self) -> bool {
        let vtable = self.vtable.get();

        let was_marked = vtable & MARK_BIT == 0;
        if !was_marked {
            self.vtable.set(vtable | MARK_BIT);
        }

        !was_marked && vtable & TRAVERSE_BIT != 0
    }

    // Query the mark bit
    #[inline]
    pub fn is_marked(&self) -> bool {
        self.vtable.get() & MARK_BIT != 0
    }

    // Unset the mark bit
    #[inline]
    pub fn unmark(&self) {
        let vtable = self.vtable.get();
        self.vtable.set(vtable & MARK_MASK);
    }

    // Get the vtable ptr without mark or traverse bits set
    #[inline]
    pub fn vtable(&self) -> usize {
        self.vtable.get() & PTR_MASK
    }
}


impl TraceStack {
    pub fn new() -> TraceStack {
        TraceStack { stack: ObjectBuf::new() }
    }

    pub fn push(&mut self, obj: Object) {
        self.stack.push(obj);
    }

    pub fn pop(&mut self) -> Option<Object> {
        self.stack.pop()
    }

    // Create initial contents from a slice of Objects
    pub fn from_roots(&mut self, slice: &[Object]) {
        self.stack.extend_from_slice(slice);
    }
}


impl TraceOps for TraceStack {
    fn push_to_trace(&mut self, object: &Trace) {
        let tobj: TraitObject = unsafe { transmute(object) };
        self.stack.push(Object::from(tobj));
    }
}
