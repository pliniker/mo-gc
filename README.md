## A pauseless, concurrent, generational, parallel mark-and-sweep garbage collector in Rust

This is a very experimental garbage collector primarily built to research
the viability of a pauseless mechanism. May eat laundry etc etc.

Much has not yet been fully thought through, especially the how to
effectively build data structures while avoiding concurrency issues.

* [![Build Status](https://travis-ci.org/pliniker/mo-gc.svg?branch=master)](https://travis-ci.org/pliniker/mo-gc)
* [Documentation](https://pliniker.github.io/mo-gc/), but mostly see the examples.

### Summary

Mutator threads write stack-root reference count adjustments to a journal.

The journal is read concurrently by a GC thread that maintains the actual
reference count numbers in a roots map.

Tracing is run in the GC thread from the root map, by making a `trace()`
virtual function call to objects, which return the objects they reference.
This `trace()` function must be thread-safe since it is called concurrently
with the mutator.

Data structures' `trace()` functions can implement any transactional
mechanism that provides the GC an immutable snapshot of the data structure's
nested pointers for the duration of the trace function call.

* [Original draft design outline](https://github.com/pliniker/mo-gc/blob/master/doc/Project-RFC.md)
* [Some discussion](https://github.com/pliniker/mo-gc/issues/1) on the original RFC.
* See the [Implementation Notes](https://github.com/pliniker/mo-gc/blob/master/doc/Implementation-Notes.md)
  for a technical description of how the current implementation works.
* See the [TODO](https://github.com/pliniker/mo-gc/blob/master/TODO.md)
  for unresolved issues and areas for improvement.

### Tradeoffs

* no stop-the-world pauses whatsoever, not even incremental pauses
* multiprocessor friendly - GC runs in parallel with mutator threads
* opt-in standalone library not tied to any VM or other runtime

But:

* unproven design and implementation
* possibly undiscovered concurrency issues
* throughput overhead on mutator threads is the use of the journal and
  the need for transactional data structures
* currently Rust doesn't have a way to specify destructors as potentially
  being unsafe, which they are in a GC managed environment when they
  attempt to dereference already freed objects

### Why

My interest in this project is in building a foundation, written in Rust, for
a language runtime on top of Rust that doesn't place typical restrictions
on the runtime.

It is not likely to provide a drop-in replacement for `Rc<T>`, or `Arc<T>`
though it may be possible in some cases.

See also:
* [rust-gc](https://github.com/manishearth/rust-gc)
* [crossbeam](https://github.com/aturon/crossbeam/)
* [bacon-rajan-cc](https://github.com/fitzgen/bacon-rajan-cc)

### Using

Add the following to `Cargo.toml`:

```
[dependencies]
mo-gc = { git = "https://github.com/pliniker/mo-gc/" }
```

### About this Project

* Copyright &copy; 2015 Peter Liniker <peter.liniker@gmail.com>
* Licensed under dual MIT/Apache-2.0
* Named after [M-O](http://pixar.wikia.com/wiki/M-O).

### Contributing

Collaboration is welcome! See the TODO file for a list of things that need to be thought through,
open an issue, email me with your questions and ideas or find me on `#rust` as `pliniker`.
