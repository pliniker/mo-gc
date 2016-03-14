//! A partially-parallel young generation collector.
//!
//! Reading the journal into the root map is single-threaded.
//!
//! This is similar in construction to ParHeap, except that this object map must deal
//! with reference counts from the journal.


use std::cmp::max;
use std::mem::transmute;
use std::raw::TraitObject;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use scoped_pool::Pool;

use constants::{BUFFER_RUN, DEC, FLAGS_MASK, INC, JOURNAL_RUN, NEW, NEW_BIT, NEW_INC};
use heap::{CollectOps, Object, ObjectBuf, RootMap, RootMeta, TraceStack};
use gcthread::{EntryReceiver, JournalList, ptr_shift};
use statistics::StatsLogger;
use trace::Trace;


/// Type that composes all the things we need to run garbage collection on young generation
/// objects.
///
/// The roots trie maps object addresses to their reference counts, vtables and `NEW` object
/// flags.
///
/// During tracing, positive reference count objects and non-`NEW` objects are considered
/// possible roots and only `NEW` objects are considered for marking and sweeping. Entries
/// can be both roots and `NEW`.
///
/// Collection is run in a thread pool across all CPUs by default by sharding the root trie
/// across threads.
pub struct YoungHeap<S: StatsLogger, T: CollectOps + Send> {
    /// Size of the thread pool
    num_threads: usize,

    /// A list of AppThread journals to read from
    journals: JournalList,

    /// Map of object addresses to reference counts and other data
    roots: RootMap,

    /// Buffer of deferred negative reference count adjustments
    deferred: ObjectBuf,

    /// The mature object space
    mature: T,

    /// Something that implements statistics logging
    logger: S,
}


impl<S: StatsLogger, T: CollectOps + Send> YoungHeap<S, T> {
    /// Create a new young generation heap and roots reference count tracker
    pub fn new(num_threads: usize, mature: T, logger: S) -> YoungHeap<S, T> {
        YoungHeap {
            num_threads: num_threads,
            journals: JournalList::new(),
            roots: RootMap::new(),
            deferred: ObjectBuf::new(),
            mature: mature,
            logger: logger,
        }
    }

    /// Add a new journal to the list of journals to read
    pub fn add_journal(&mut self, recv: EntryReceiver) {
        self.journals.push(recv);
    }

    /// Returns the number of journals currently connected to the GC
    pub fn num_journals(&self) -> usize {
        self.journals.len()
    }

    /// Read all journals for a number of iterations, updating the roots and keeping a reference
    /// count increment for each, and putting decrements into the deferred buffer.
    ///
    /// This function is single-threaded and is the biggest GC throughput bottleneck. Setting a
    /// value in the trie is slow compared to allocation and writing/reading the journal.
    ///
    /// Easily consumes 80% of linear GC time. TODO: parallelize this function.
    ///
    /// Returns the number of journal entries read.
    pub fn read_journals(&mut self) -> usize {
        let mut entry_count = 0;

        // read through the journals a few times
        for _ in 0..JOURNAL_RUN {

            // for each journal
            for journal in self.journals.iter_mut() {


                // read the journal until empty or a limited number of entries have been pulled
                for entry in journal.iter_until_empty().take(BUFFER_RUN) {

                    entry_count += 1;

                    match entry.ptr & FLAGS_MASK {
                        NEW_INC => {
                            let ptr = entry.ptr >> ptr_shift();
                            self.roots.set(ptr, RootMeta::one(entry.vtable, NEW_BIT));
                        }

                        NEW => {
                            let ptr = entry.ptr >> ptr_shift();
                            self.roots.set(ptr, RootMeta::zero(entry.vtable, NEW_BIT));
                        }

                        INC => {
                            let ptr = entry.ptr >> ptr_shift();

                            let meta = self.roots.get_default_mut(ptr, || {
                                RootMeta::zero(entry.vtable, 0)
                            });

                            meta.inc();
                        }

                        DEC => self.deferred.push(entry),

                        _ => unreachable!(),
                    }
                }
            }
        }

        // remove any disconnected journals
        self.journals.retain(|ref j| !j.is_disconnected());

        entry_count
    }

    /// Do a young generation collection. Returns the number of new objects in the young generation
    /// heap.
    pub fn minor_collection(&mut self, pool: &mut Pool) -> usize {
        self.mark(pool);
        let (young_size, drop_count) = self.sweep(pool);
        self.merge_deferred(pool);

        self.logger.add_dropped(drop_count);

        young_size
    }

    /// Do a major collection, moving `NEW` objects to the mature heap and tracing the mature heap
    pub fn major_collection(&mut self, pool: &mut Pool) {
        // first move any new-objects into the mature heap by copying and unsetting the new-object
        // flag in the roots
        for (ptr, meta) in self.roots.iter_mut() {
            if !meta.unsync_is_unrooted() && meta.is_new() {
                // object must have a positive reference count and be marked as new-object to be
                // moved to the mature set
                self.mature.add_object(ptr, meta.vtable());
                // unset the new-object bit. This object will now be treated as a simple reference
                // counted root and won't be dropped from here.
                meta.set_not_new();
            }
        }

        let (heap_size, drop_count) = self.mature.collect(pool, &mut self.roots);

        self.logger.current_heap_size(heap_size);
        self.logger.add_dropped(drop_count);
    }

    /// Use >0 refcount objects and 0-refcount non-new objects to mark new objects
    fn mark(&mut self, pool: &mut Pool) {

        let shared_objects = self.roots.borrow_sync();
        let sharded_objects = shared_objects.borrow_sharded(self.num_threads);

        pool.scoped(|scope| {

            for shard in sharded_objects.iter() {
                let objects = shared_objects.clone();
                // here there is a shard of the heap and a shared reference to the whole
                // heap (objects) for each thread

                scope.execute(move || {
                    let mut stack = TraceStack::new();

                    for (root_ptr, root_meta) in shard.iter() {
                        if !root_meta.unsync_is_unrooted() || !root_meta.is_new() {
                            // read the shard to find roots, which are non-zero-refcount
                            // entries. Also consider non-new entries as possible roots of new
                            // objects: this is our equivalent of searching a card table

                            if root_meta.mark_and_needs_trace() {
                                // mark the root, and if it needs tracing then look into it
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
                    }
                });
            }
        });
    }

    /// Drop unmarked new objects and remove unrooted objects.
    /// Returns tuple (young_object_count, dropped_count)
    fn sweep(&mut self, pool: &mut Pool) -> (usize, usize) {
        // set counters
        let collect_young_count= Arc::new(AtomicUsize::new(0));
        let collect_drop_count = Arc::new(AtomicUsize::new(0));

        let mut split_objects = self.roots.borrow_sharded(self.num_threads);

        pool.scoped(|scope| {

            for mut node in split_objects.drain() {

                // pass a reference to each counter to each thread
                let young_count = collect_young_count.clone();
                let drop_count = collect_drop_count.clone();

                scope.execute(move || {

                    let mut young_counter = 0;
                    let mut drop_counter = 0;

                    node.retain_if(|ptr, meta| {

                        if meta.is_new_and_unmarked() {
                            drop_counter += 1;

                            // unmarked new-object (implies zero-refcount)
                            let obj = Object::from_trie_ptr(ptr, meta.vtable);
                            let tobj: TraitObject = Object::into(obj);

                            unsafe {
                                let fatptr: *mut Trace = transmute(tobj);
                                let owned = Box::from_raw(fatptr);
                                drop(owned);
                            }

                            false

                        } else if !meta.is_new() && meta.unsync_is_unrooted() {
                            false

                        } else {
                            if meta.is_new() {
                                young_counter += 1;
                            }

                            meta.unmark();
                            true
                        }
                    });

                    // write out the counters
                    young_count.fetch_add(young_counter, Ordering::SeqCst);
                    drop_count.fetch_add(drop_counter, Ordering::SeqCst);
                });
            }
        });

        // return the counters
        (collect_young_count.load(Ordering::Acquire),
         collect_drop_count.load(Ordering::Acquire))
    }

    /// Move the deferred refcount decrements into the root set's reference counts.
    fn merge_deferred(&mut self, pool: &mut Pool) {
        let chunk_size = max(1, self.deferred.len() / self.num_threads);

        {
            let shared_roots = self.roots.borrow_sync();
            let chunks = self.deferred.chunks(chunk_size);

            pool.scoped(|scope| {

                for chunk in chunks {

                    let roots = shared_roots.clone();

                    scope.execute(move || {
                        for object in chunk {
                            let ptr = object.ptr >> ptr_shift();

                            if let Some(ref mut meta) = roots.get(ptr) {
                                // this is the only place where the reference count needs to
                                // be thread-safely adjusted
                                meta.dec();
                            } else {
                                // there should never be something in the deferred buffer that
                                // isn't in the heap
                                unreachable!();
                            }
                        }
                    });
                }
            });
        }

        self.deferred.clear();
    }

    /// Return a reference to the logger
    pub fn logger(&mut self) -> &mut S {
        &mut self.logger
    }

    /// Call to return the logger on shutdown
    pub fn shutdown(self) -> S {
        self.logger
    }
}
