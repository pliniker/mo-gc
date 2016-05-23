initSidebarItems({"constant":[["BUFFER_RUN",""],["CACHE_LINE",""],["DEC",""],["FLAGS_MASK",""],["INC",""],["INC_BIT",""],["JOURNAL_BUFFER_SIZE",""],["JOURNAL_RUN",""],["MAJOR_COLLECT_THRESHOLD",""],["MARK_BIT",""],["MARK_MASK",""],["MAX_SLEEP_DUR",""],["MIN_SLEEP_DUR",""],["NEW",""],["NEW_BIT",""],["NEW_INC",""],["NEW_MASK",""],["PTR_MASK",""],["TRAVERSE_BIT",""]],"fn":[["make_journal","Return a Sender/Receiver pair that can be handed over to other threads. The capacity is the requested size of each internal buffer and will be rounded to the next power of two."]],"struct":[["AppThread","An Application Thread, manages a thread-local reference to a tx channel"],["Gc","Non-atomic pointer type. This type is `!Sync` and thus is useful for presenting a Rust-ish API to a data structure where aliasing and mutability must follow the standard rules: there can be only one mutator."],["GcAtomic","Atomic pointer type that points at a traceable object. This type is `Sync` and can be used to build concurrent data structures."],["GcBox","GcBox struct and traits: a boxed object that is GC managed"],["GcRoot","Root smart pointer, sends reference count changes to the journal."],["GcThread","The Garbage Collection thread handle."],["ParHeap","This references all known GC-managed objects and handles marking and sweeping; parallel mark and sweep version."],["Receiver","A journal reader type which can be sent to another thread"],["Sender","A journal writer type which can be sent to another thread"],["TraceStack","A type that contains a stack of objects to trace into. This type is separated out from the main Heap type so that different collection strategies can be implemented without affecting the client code. The `Trace` trait depends only this type, then, and not the whole Heap type."],["YoungHeap","Type that composes all the things we need to run garbage collection on young generation objects."]],"trait":[["CollectOps","A trait that describes collection operations on a Heap"],["StatsLogger","Type that provides counters for the GC to gain some measure of performance."],["Trace","Trace trait. Every type that can be managed by the GC must implement this trait. This trait is unsafe in that incorrectly implementing it can cause Undefined Behavior."],["TraceOps","A trait that describes Trace operations on a Heap"]]});