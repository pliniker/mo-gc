# A pauseless concurrent garbage collector in Rust

### Summary

Application threads maintain precise-rooted GC-managed pointers through smart
pointers on the stack that write reference-count increments and decrements to a
journal. The reference-count journal is read by a GC thread that
maintains the actual reference count numbers in a cache of roots. When a
reference count reaches zero, the GC thread moves the pointer to a heap cache
data structure that is used by a tracing collector.

Because the GC thread has no synchronization between itself
and the application threads besides the journal, all data structures
that contain nested GC-managed pointers must be immutable in their GC-managed
relationships: persistent data structures must be used to avoid data races.

[Technical RFC](https://github.com/pliniker/mo-gc/blob/master/doc/Project-RFC.md)
and [discussion](https://github.com/pliniker/mo-gc/issues/1)

### Tradeoffs

* no stop-the-world pauses whatsoever
* multiprocessor friendly - GC runs in parallel with application threads
* opt-in standalone library not tied to any VM or other runtime

But:

* throughput overhead on application threads is the use of the journal and
the need for persistent data structures
* potentially a lot of garbage is created

### About this Project

* Copyright &copy; 2015 Peter Liniker <peter.liniker@gmail.com>
* Licensed under the MPLv2

Since I picture this algorithm as a robot chasing frantically
after all the garbage, never quite catching up, it is named for
[M-O](http://pixar.wikia.com/wiki/M-O), the cleaning robot from [WALL-E](https://www.youtube.com/watch?v=mfLHhnDzPcc).
