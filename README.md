## An experimental garbage collector in Rust

This is a very experimental garbage collector primarily built to research the viability of a
pauseless mechanism that does not depend on compiler GC support. It currently in development
and is not ready for use.

* [![Build Status](https://travis-ci.org/pliniker/mo-gc.svg?branch=master)](https://travis-ci.org/pliniker/mo-gc)

### Further iformation

* [Original draft design outline](https://github.com/pliniker/mo-gc/blob/master/doc/Project-RFC.md)
* [Some discussion](https://github.com/pliniker/mo-gc/issues/1) on the original design.
* [Implementation Notes](https://github.com/pliniker/mo-gc/blob/master/doc/Implementation-Notes.md)
* [Documentation](https://pliniker.github.io/mo-gc/), but also see the examples.
* [TODO](https://github.com/pliniker/mo-gc/blob/master/TODO.md) lists some issues.

### See also

* [rust-gc](https://github.com/manishearth/rust-gc)
* [crossbeam](https://github.com/aturon/crossbeam/)
* [bacon-rajan-cc](https://github.com/fitzgen/bacon-rajan-cc)

### About this Project

* Copyright &copy; 2015 Peter Liniker <peter.liniker@gmail.com>
* Licensed under dual MIT/Apache-2.0
* Named after [M-O](http://pixar.wikia.com/wiki/M-O).

### Contributing

Collaboration is welcome! See the TODO file for a list of things that need to be thought through,
open an issue, email me with your questions and ideas or find me on `#rust` as `pliniker`.
