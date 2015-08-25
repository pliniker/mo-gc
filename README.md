# A pauseless concurrent garbage collector in Rust

### Abstract

Application threads maintain precise-rooted GC-managed pointers through smart
pointers on the stack that write reference-count increments and decrements to a
write-only journal. The reference-count journal is read by a GC thread that
maintains the actual reference count numbers in a cache of GC roots. When a
reference count reaches zero, the GC thread moves the pointer to a heap cache
data structure that keeps no reference counts but that is used for mark and
sweep collection.

Because the GC thread has no synchronization between itself
and the application threads besides the inc/dec journal, all data structures
that contain nested GC-managed pointers must be immutable in their GC-managed
relationships: persistent data structures must be used to avoid data races.

[Technical RFC](http://github.com/pliniker/mo-gc/blob/text/doc/Project-RFC.md)
and [discussion](https://github.com/pliniker/mo-gc/issues/1)

### Tradeoffs

* The application threads can run pauselessly, no stop-the-world required, as
the cost of scanning the stack for roots is amortized over time in the
application threads as they write to the inc/dec journal.
* The implementation can be a library without needing any compiler extensions.
* As a freestanding library now, it should be well placed to integrate with
future compiler GC awareness.
* The implementation can use the default Rust allocator.

But:

* Nested GC-managed pointer structures must be immutable in those relationships,
the added cost on the application threads is the requirement for persistent
data structures.
* As there is no stop-the-world, even incremental, any form of copying
collector would be a challenge to implement. However, a number of
efficient improvements on straightforward mark and sweep are likely possible.
* Some language/compiler safety issues remain: referencing GC-managed pointers
in a `drop()` is currently legal but unsafe as the order of collection is
non-deterministic.

### About this Project

* Copyright &copy; 2015 Peter Liniker
* Licensed under the MPLv2

Since I mentally visualize this library as a robot chasing frantically
after all the garbage, it is named for [M-O](http://disney.wikia.com/wiki/M-O),
the cleaning robot from WALL-E.
