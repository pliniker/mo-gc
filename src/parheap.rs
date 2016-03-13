//! A parallel collector for the entire heap.


use std::mem::transmute;
use std::raw::TraitObject;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use scoped_pool::Pool;

use gcthread::ptr_shift;
use heap::{CollectOps, HeapMap, Object, ObjectMeta, RootMap, TraceStack};
use trace::Trace;


/// This references all known GC-managed objects and handles marking and sweeping; parallel mark
/// and sweep version.
pub struct ParHeap {
    num_threads: usize,
    objects: HeapMap,
}


unsafe impl Send for ParHeap {}


impl ParHeap {
    /// In this heap implementation, work is split out into a thread pool. There is no knowing,
    /// though, how much work each split actually represents. One thread may receive a
    /// disproportionate amount of tracing or sweeping.
    pub fn new(num_threads: usize) -> ParHeap {
        ParHeap {
            num_threads: num_threads,
            objects: HeapMap::new(),
        }
    }

    /// A parallel mark implementation:
    ///  * shares a borrow of the main HeapMap among the thread pool
    ///  * divides the roots among the thread pool
    ///  * each thread traces from it's own slice of roots
    fn mark(&mut self, thread_pool: &mut Pool, roots: &mut RootMap) {
        // divide the roots among threads and trace
        let mut sharded_roots = roots.borrow_sharded(self.num_threads);

        thread_pool.scoped(|scope| {

            // borrow the main HeapMap for the duration of this scope
            let shared_objects = self.objects.borrow_sync();

            // split roots into a slice for each thread and hand a slice and an new-object
            // HeapMap to each job
            for roots in sharded_roots.drain() {

                // make a thread-local trace stack and reference to the heap
                let objects = shared_objects.clone();

                // mark using the thread-local slice of roots
                scope.execute(move || {

                    let mut stack = TraceStack::new();

                    for (root_ptr, root_meta) in roots.iter() {
                        if !root_meta.unsync_is_unrooted() && root_meta.mark_and_needs_trace() {
                            // read the shard to find roots, which are all positive-refcount
                            // entries. Trace the roots if they need it.

                            let obj = Object::from_trie_ptr(root_ptr, root_meta.vtable());

                            let object = obj.as_trace();
                            unsafe { object.trace(&mut stack) };

                            // now there may be some child objects on the trace stack: pull
                            // them off and mark them too
                            while let Some(obj) = stack.pop() {

                                let ptr = obj.ptr >> ptr_shift();
                                if let Some(meta) = objects.get(ptr) {

                                    if meta.mark_and_needs_trace() {
                                        let object = obj.as_trace();
                                        unsafe { object.trace(&mut stack) };
                                    }
                                }
                            }
                        }
                    }
                }); // execute
            }
        }); // scope
    }

    /// A parallel sweep implementation:
    ///  * the main HeapMap tree is split into subtrees and each thread is given a separate subtree
    ///    to sweep
    /// Returns a tuple of (heap_object_count, dropped_object_count)
    fn sweep(&mut self, thread_pool: &mut Pool) -> (usize, usize) {
        // set counters
        let collect_heap_size = Arc::new(AtomicUsize::new(0));
        let collect_drop_count = Arc::new(AtomicUsize::new(0));

        // shard the heap
        let mut sharded_objects = self.objects.borrow_sharded(self.num_threads);

        thread_pool.scoped(|scope| {

            for mut shard in sharded_objects.drain() {

                // pass a reference to each counter to each thread
                let heap_size = collect_heap_size.clone();
                let drop_count = collect_drop_count.clone();

                // each thread sweeps a sub-trie
                scope.execute(move || {

                    let mut heap_counter = 0;
                    let mut drop_counter = 0;

                    shard.retain_if(|ptr, meta| {
                        heap_counter += 1;

                        if !meta.is_marked() {
                            drop_counter += 1;

                            // if not marked, drop the object
                            let tobj = TraitObject {
                                data: (ptr << ptr_shift()) as *mut (),
                                vtable: meta.vtable() as *mut (),
                            };

                            unsafe {
                                let fatptr: *mut Trace = transmute(tobj);
                                let owned = Box::from_raw(fatptr);
                                drop(owned);
                            }

                            false

                        } else {
                            // unmark the object
                            meta.unmark();
                            true
                        }
                    });

                    // write out the counters
                    heap_size.fetch_add(heap_counter, Ordering::SeqCst);
                    drop_count.fetch_add(drop_counter, Ordering::SeqCst);
                });
            }
        });

        // return the counters
        (collect_heap_size.load(Ordering::Acquire),
         collect_drop_count.load(Ordering::Acquire))
    }
}


impl CollectOps for ParHeap {
    /// Add an object directly to the heap. `ptr` is assumed to already be right-shift adjusted
    fn add_object(&mut self, ptr: usize, vtable: usize) {
        self.objects.set(ptr, ObjectMeta::new(vtable));
    }

    /// Run a collection iteration on the heap. Return the total heap size and the number of
    /// dropped objects.
    fn collect(&mut self, thread_pool: &mut Pool, roots: &mut RootMap) -> (usize, usize) {
        self.mark(thread_pool, roots);
        self.sweep(thread_pool)
    }
}
