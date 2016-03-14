## A pauseless, concurrent, generational, parallel mark-and-sweep  garbage collector in Rust

This is a very experimental garbage collector primarily built to research
the viability of a pauseless mechanism. May eat laundry etc etc.

[![Build Status](https://travis-ci.org/pliniker/mo-gc.svg?branch=master)](https://travis-ci.org/pliniker/mo-gc)

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

* [Original rough design RFC](https://github.com/pliniker/mo-gc/blob/master/doc/Project-RFC.md)
* [Some discussion](https://github.com/pliniker/mo-gc/issues/1) on the original RFC.
* See the [Implementation Notes](https://github.com/pliniker/mo-gc/blob/master/doc/Implementation-Notes.md)
  for a technical description of how the current implementation works.
* See the [TODO](https://github.com/pliniker/mo-gc/blob/master/TODO.md)
  for unresolved issues.

### Tradeoffs

* no stop-the-world pauses whatsoever, not even incremental pauses
* multiprocessor friendly - GC runs in parallel with mutator threads
* opt-in standalone library not tied to any VM or other runtime

But:

* throughput overhead on mutator threads is the use of the journal and
  the need for transactional data structures
* the root set read from the journal by the GC thread lags behind the
  real time root set while tracing touches the real time pointer
  references (see TODO for resulting issues)
* currently Rust doesn't have a way to specify destructors as potentially
  being unsafe, which they are in a GC managed environment when they
  attempt to dereference already freed objects

### Why

My interest in this project is in building a foundation, written in Rust, for
a language runtime on top of Rust.

Rust itself is not in need of a garbage collector and if it was, this might
not be it since this is experimental and has outstanding issues.

It cannot generally provide a drop-in replacement for `Rc<T>`, or `Arc<T>`
though it may be possible in some cases.

For more general purpose, ergonomic collectors, see
[rust-gc](https://github.com/manishearth/rust-gc) or
[bacon-rajan-cc](https://github.com/fitzgen/bacon-rajan-cc).

This is also not primarily intended to be an ergonomic GC for
concurrent data structures in Rust. See
[crossbeam](https://github.com/aturon/crossbeam/) instead.

### Using

Add the following to `Cargo.toml`:

```
[dependencies]
mo = { git = "https://github.com/pliniker/mo-gc/" }
```

### About this Project

* Copyright &copy; 2015 Peter Liniker <peter.liniker@gmail.com>
* Licensed under dual MIT/Apache-2.0

Named after [M-O](http://pixar.wikia.com/wiki/M-O).

### Contributing

Collaboration is welcome. Email me at the address above or find me on `#rust` as `pliniker`.
