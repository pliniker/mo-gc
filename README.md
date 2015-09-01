# A pauseless concurrent garbage collector in Rust

### Summary

Application threads maintain precise-rooted GC-managed pointers through smart
pointers on the stack that write reference-count increments and decrements to a
journal.

The reference-count journal is read by a GC thread that
maintains the actual reference count numbers in a cache of roots. When a
reference count reaches zero, the GC thread moves the pointer to a heap cache
data structure that is used by a tracing collector.

Because the GC thread runs concurrently with the application threads without
explicitly stopping them to maintain synchronization, all GC-managed data
structures that reference other GC-managed objects must be transactional
in their updates and persistent.

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

### Why

Many languages are hosted in the inherently unsafe languages C and/or C++,
from Python to GHC.

My interest in this project is in building a foundation, written in Rust, for
interpreted languages on top of Rust. Since Rust is a modern
language for expressing low-level interactions with hardware, it is an
ideal alternative to C/C++ while providing the opportunity to avoid classes
of bugs common to C/C++ by default.

A garbage collector is an essential
luxury for most styles of programming, in fact, how memory is managed in an
interpreter can be an asset or a liability that becomes so intertwined with
the language semantics itself that it can be a feat to modernize years later.

With that in mind, this GC is designed from the ground up to be concurrent
and never stop the world. The caveat is that data structures
need to be designed for lock-free, concurrent reads and writes. In this world,
the GC is just another thread, reading data structures and freeing any that
are no longer live.

Those seem reasonable tradeoffs in a time when scaling out by adding
processors rather than up through increased clock speed is now the status quo.

This is not particularly intended to be a general purpose GC, providing
a near plug-and-play replacement for `Rc<T>`. For that, I recommend looking
at [rust-gc](https://github.com/manishearth/rust-gc) or
[bacon-rajan-cc](https://github.com/fitzgen/bacon-rajan-cc).

### About this Project

* Copyright &copy; 2015 Peter Liniker <peter.liniker@gmail.com>
* Licensed under the MPLv2

Since I picture this algorithm as a robot chasing frantically
after all the garbage, never quite catching up, it is named for
[M-O](http://pixar.wikia.com/wiki/M-O), the cleaning robot from [WALL-E](https://www.youtube.com/watch?v=mfLHhnDzPcc).
