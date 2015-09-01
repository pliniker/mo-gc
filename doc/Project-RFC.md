
* Date: 2015-08-24
* Discussion issue: [pliniker/mo-gc#1](https://github.com/pliniker/mo-gc/issues/1)

# Summary

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
mechanism that provides the GC a snapshot of the data structure's 
nested pointers for the duration of the trace function call.

# Why

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

# What this is not

This is not particularly intended to be a general purpose GC, providing
a near drop-in replacement for `Rc<T>`, though it may be possible.
For that, I recommend looking at 
[rust-gc](https://github.com/manishearth/rust-gc) or
[bacon-rajan-cc](https://github.com/fitzgen/bacon-rajan-cc).

This is also not primarily intended to be an ergonomic, native GC for all 
concurrent data structures in Rust. For that, I recommend a first look at 
[crossbeam](https://github.com/aturon/crossbeam/).

Finally, this is not a proposal to include such a library into Rust `std`.

# Assumptions

This RFC assumes the use of the default Rust allocator, jemalloc, throughout
the GC. No custom allocator is described here at this time. Correspondingly,
the performance characteristics of jemalloc should be assumed.

# Journal Implementation

## Application Threads

The purpose of using a journal is to minimize the burden on the application
threads as much as possible, pushing as much workload as possible over to the
GC thread, while avoiding pauses.

To give an idea of how this should work, in the most straightforward
implementation, the journal can simply be a `std::sync::mpsc` channel shared
between application threads and sending reference count adjustments to the
GC thread, that is, +1 and -1 for pointer clone and drop respectively.

Performance for multiple application threads writing to an mpsc, with each
write causing an allocation, can be improved on based on the
[single writer principle][9]
by 1) giving each application thread its own channel and 2) buffering journal
entries and passing a reference to the buffer through the channel.

Buffering journal entries should give good cache locality and reduce the number
of extra allocations per object created. The application threads are responsible
for allocating new buffers when they have filled the current one, the GC thread
is responsible for freeing those buffers when they have been fully read.

When newly rooting a pointer to the stack, the current buffer must be accessed.
One solution is to use Thread Local Storage so that each thread will be able
to access its own buffer at any time. The overhead of looking up the TLS
pointer is a couple of extra instructions in a release build to check that
the buffer data has been initialized

A journal buffer maintains a count at offset 0 to indicate how many words of
adjustment data have been written. This count should be written to using
[release](https://doc.rust-lang.org/std/sync/atomic/enum.Ordering.html) ordering
while the GC thread should read the count using acquire ordering.

This design should improve on typical reference counting implementations, where
the count is maintained on the object being counted itself, through better
cache locality: the application threads only need to write to a buffer
that can simply be an array of contiguous memory.

Finally, it should be noted that the root smart-pointers shouldn't necessarily
be churning out reference count adjustments. This is Rust: prefer to borrow
a root smart-pointer before cloning it. This is one of the main features that
makes implementing this in Rust so attractive.

## Garbage Collection Thread

In the simplest `std::sync::mpsc` use case, the GC thread reads reference count
adjustments from the channel. For each inc/dec adjustment, it must look up the
associated pointer in a data structure and update the total reference count
for that pointer.

In the case of multiple channels, each sending a buffer of adjustments at a
time, there will naturally be an ordering problem:

Thread A may, for a pointer, write the following to its journal:

|Action|adjustment| |
| --- | --- | --- |
|new pointer|+1||
|clone pointer|+1|(move cloned pointer to Thread B)|
|drop pointer|-1||

Thread B may do the following a brief moment later after receiving the
cloned pointer:

|Action|adjustment| |
| --- | --- | --- |
|drop pointer|-1|(drop cloned pointer)|

The order in which these adjustments are processed by the GC thread may well
be out of order, and there is no information available to restore the correct
order. The decrement from Thread B might be processed first, followed by the
first increment from Thread A, giving a momentary reference count of 0. The
collector may kick in at that point, freeing the object and resulting in a
possible use-after-free and possibly a double-free.

Here, learning from [Bacon2003][1], decrement adjustments should be
buffered by an amount of time sufficient to clear all increment adjustments
that occurred prior to those decrements. An appropriate amount of time might
be provided by scanning the application threads'
buffers one further iteration before applying the buffered decrements.

Increment adjustments can be applied immediately, always.

# Collector Implementation

While more advanced or efficient algorithms might be applied here, this section
will describe how a straightforward mark and sweep can be applied.

As in [rust-gc][4], all types participating in GC must implement
a trait that allows that type to be traced. (This is an inconvenience that
a compiler plugin may be able to alleviate for many cases.)

The GC thread maintains two trie structures: one to map from roots to
reference counts; a second to map from heap objects to any metadata needed to
run `drop()` against them and bits for marking while tracing.

The roots trie is traversed, calling the trace function for each. Every visited
object is marked in the heap trie.

Then the heap trie is traversed and every unmarked entry is `drop()`ped and
the live objects unmarked.

It is worth noting that by using a separate trie for the heap and root pointers
that this GC scheme remains `fork()` memory friendly: the act of updating 
reference counts and marking heap objects does not force a copy-on-write for 
every touched page of memory.

## Concurrent Data Structures

To prevent data races between the application threads and the GC thread, all
GC-managed data structures that contain pointers to other GC-managed objects
must be transactional in updates to those relationships. That is, a 
`GcRoot<Vec<i32>>` can contain mutable data where the mutability follows 
the Rust static analysis rules, but a `GcRoot<Vec<GcBox<i32>>>` must be
reimplemented with a transactional runtime nature.

The `Vec::trace()` method has to be able to provide a readonly
snapshot of its contents to the GC thread and atomic updates to its 
contents.

Applying a compile-time distinction between these may be possible using the
type system. Indeed, presenting a safe API is one of the challenges in
implementing this.

As the `trace()` method is part of the data structure itself, data structures
should be free to implement any method of atomic update without the GC
code or thread needing to be aware of transactions occurring.

Fortunately, concurrent data structures are fairly widely researched and 
in use by 2015.

## Generational Optimization

Since the application threads write a journal of all root pointers, all
pointers that the application uses will be recorded. It may be possible 
for the GC thread to use that fact to process batches of journal changes
in a generational manner, rather than having to trace the entire heap
on every iteration. This needs further investigation.

## Parallel Collection

The tries used in the GC should be amenable to parallelizing tracing which
may be particularly beneficial in conjunction with tracing the whole heap.

# Benchmarking

Benchmarking should be primarily done in a single threaded context, where the
overhead of the GC will be measurable against the application code.

As a base comparison, a non-GC purely compile-time memory managed set of
benchmarks should be compared against.

# Tradeoffs

How throughput compares to other GC algorithms is left to
readers more experienced in the field to say. My guess is that with the overhead
of the journal while doing mostly new-generation collections that this
algorithm should be competitive.

Non-atomic objects must be transactional, adding the cost associated with 
concurrent data structures: the garbage generated. In some circumstances there 
could be enormous amounts of garbage generated, raising the overall overhead of 
using the GC to where the GC thread affects throughput.

Jemalloc is said to give low fragmentation rates compared to other malloc
implementations, but fragmentation is likely nonetheless.

At least this one language/compiler safety issue remains: referencing
GC-managed pointers in a `drop()` is currently considered safe by the compiler
as it has no awareness of the GC, but doing so is of course unsafe as the order 
of collection is non-deterministic leading to possible use-after-free in custom
`drop()` functions.

# Rust Library Compatibility

As the GC takes over the lifetime management of any objects put under its
control - and that transfer of control is completely under the control of
the programmer - any Rust libraries should work with it, including low-level
libraries such as [coroutine-rs](https://github.com/rustcc/coroutine-rs) and
by extension [mioco](https://github.com/dpc/mioco).

This GC will never interfere with any code that uses only the native Rust
memory management.

# Improvements

## Compiler Plugin

It is possible to give the compiler some degree of awareness of GC requirements
through custom plugins, as implemented in [rust-gc][4] and [servo][13]. The same
may be applicable here.

In the future, this implementation would surely benefit from aspects of the
planned [tracing hooks][5].

## Copying Collector

Any form of copying or moving collector would require a custom allocator and
probably a Baker-style read barrier. The barrier could be implemented on the
root smart pointers with the added expense of the application threads having to
check whether the pointer must be updated on every dereference. There are
pitfalls here though as the Rust compiler may optimize dereferences with
pointers taking temporary but hard-to-discover root in CPU registers. It may 
be necessary to use the future tracing hooks to discover all roots to avoid 
Bad Things happening.

# Patent Issues

I have read through the patents granted to IBM and David F. Bacon that cover
reference counting and have come to the conclusion that nothing described here
infringes.

I have not read further afield though. My assumption has been that there is
prior art for most garbage collection methods at this point.

# References

* [Bacon2003][1] Bacon et al, A Pure Reference Counting Garbage Collector
* [Bacon2004][2] Bacon et al, A Unified Theory of Garbage Collection
* [Oxischeme][3] Nick Fitzgerald, Memory Management in Oxischeme
* [Manishearth/rust-gc][4] Manish Goregaokar, rust-gc project
* [Rust blog][5] Rust in 2016
* [rust-lang/rust#11399][6] Add garbage collector to std::gc
* [rust-lang/rfcs#415][7] Garbage collection
* [rust-lang/rust#2997][8] Tracing GC in rust
* [Mechanical Sympathy][9] Martin Thompson, Single Writer Principle
* [michaelwoerister/rs-persistent-datastructures][10] Michael Woerister, HAMT in Rust
* [crossbeam][11] Aaron Turon, Lock-freedom without garbage collection
* [Shenandoah][12] Shenandoah, a low-pause GC for the JVM
* [Servo][13] Servo blog, JavaScript: Servo’s only garbage collector

[1]: http://researcher.watson.ibm.com/researcher/files/us-bacon/Bacon03Pure.pdf
[2]: http://www.cs.virginia.edu/~cs415/reading/bacon-garbage.pdf
[3]: http://fitzgeraldnick.com/weblog/60/
[4]: https://github.com/Manishearth/rust-gc
[5]: http://blog.rust-lang.org/2015/08/14/Next-year.html
[6]: https://github.com/rust-lang/rust/pull/11399
[7]: https://github.com/rust-lang/rfcs/issues/415
[8]: https://github.com/rust-lang/rust/issues/2997
[9]: http://mechanical-sympathy.blogspot.co.uk/2011/09/single-writer-principle.html
[10]: https://github.com/michaelwoerister/rs-persistent-datastructures
[11]: http://aturon.github.io/blog/2015/08/27/epoch/
[12]: https://www.youtube.com/watch?v=QcwyKLlmXeY
[13]: https://blog.mozilla.org/research/2014/08/26/javascript-servos-only-garbage-collector/
