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
stopping them to synchronize with them, all GC-managed data
structures that refer to other GC-managed objects must provide a safe
concurrent trace function.

Data structures' trace functions can implement any transactional 
mechanism that provides the GC an immutable snapshot of the data structure's 
nested pointers for the duration of the trace function call.

[Technical RFC](https://github.com/pliniker/mo-gc/blob/master/doc/Project-RFC.md)
and [discussion](https://github.com/pliniker/mo-gc/issues/1)

### Tradeoffs

* no stop-the-world pauses whatsoever
* multiprocessor friendly - GC runs in parallel with application threads
* opt-in standalone library not tied to any VM or other runtime

But:

* throughput overhead on application threads is the use of the journal and
the need for transactional data structures
* potentially a lot of garbage can be created
* currently Rust doesn't have a way to specify destructors as potentially
being unsafe, which they are in a GC managed environment when they
attempt to dereference already freed objects

### Why

Many languages and runtimes are hosted in the inherently unsafe languages
C and/or C++, from Python to GHC.

My interest in this project is in building a foundation, written in Rust, for
language runtimes on top of Rust. Since Rust is a modern
language for expressing low-level interactions with hardware, it is an
ideal alternative to C/C++ while providing the opportunity to avoid classes
of bugs common to C/C++ by default.

With the brilliant, notable exception of Rust, a garbage collector is an
essential luxury for most styles of programming. But how memory is managed in 
a language can be an asset or a liability that becomes so intertwined with
the language semantics itself that it can even become impossible to modernize
years later.

With that in mind, this GC is designed from the ground up to be concurrent
and never stop the world. The caveat is that data structures
need to be designed for concurrent reads and writes. In this world,
the GC is just another thread, reading data structures and freeing any that
are no longer live.

That seems a reasonable tradeoff in a time when scaling out by adding
processors rather than up through increased clock speed is now the status quo.

### What this is not

This is not particularly intended to be a general purpose GC, providing
a near drop-in replacement for `Rc<T>`, though it may be possible.
For that, I recommend looking at 
[rust-gc](https://github.com/manishearth/rust-gc) or
[bacon-rajan-cc](https://github.com/fitzgen/bacon-rajan-cc).

This is also not primarily intended to be an ergonomic, native GC for all 
concurrent data structures in Rust. For that, I recommend a first look at 
[crossbeam](https://github.com/aturon/crossbeam/).

Finally, this is not a proposal to include such a library into Rust `std`.

### About this Project

* Copyright &copy; 2015 Peter Liniker <peter.liniker@gmail.com>
* Licensed under the MPLv2

Since I picture this algorithm as a robot chasing frantically
after all the garbage, never quite catching up, it is named for
[M-O](http://pixar.wikia.com/wiki/M-O), the cleaning robot from [WALL-E](https://www.youtube.com/watch?v=mfLHhnDzPcc).
