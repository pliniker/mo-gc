# A pauseless concurrent garbage collector in Rust

### Abstract

Application threads maintain precise-rooted GC-managed pointers through smart
pointers on the stack that write reference-count increments and decrements to a
write-only journal. The reference-count journal is read by a GC thread that
maintains the actual reference count numbers in a cache of GC roots. When a
reference count reaches zero, the GC thread moves the pointer to a heap cache
data structure that keeps no reference counts but that is used by a tracing
collector.

Because the GC thread has no synchronization between itself
and the application threads besides the inc/dec journal, all data structures
that contain nested GC-managed pointers must be immutable in their GC-managed
relationships: persistent data structures must be used to avoid data races.

[Technical RFC](https://github.com/pliniker/mo-gc/blob/master/doc/Project-RFC.md)
and [discussion](https://github.com/pliniker/mo-gc/issues/1)

### Tradeoffs

* no stop-the world
* opt-in standalone library

But:

* data structures containing GC-managed pointers must be immutable
* referencing GC-managed pointers in a `drop()` is currently legal but unsafe
as the order of collection is non-deterministic

### About this Project

* Copyright &copy; 2015 Peter Liniker <peter.liniker@gmail.com>
* Licensed under the MPLv2

Since I mentally visualize this library as a robot chasing frantically
after all the garbage, it is named for [M-O](http://pixar.wikia.com/wiki/M-O),
the cleaning robot from WALL-E.
