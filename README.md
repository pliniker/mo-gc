## An experimental garbage collector in Rust

This is a very experimental garbage collector primarily built to research the viability of a
write barrier mechanism that does not depend on compiler GC support.


* [![Build Status](https://travis-ci.org/pliniker/mo-gc.svg?branch=master)](https://travis-ci.org/pliniker/mo-gc)

### Further information

Please read the [Introduction to mo-gc](http://pliniker.github.io/mo-gc-intro.html) first.

* [Ideas](http://pliniker.github.io/mo-gc-ideas.html) expands on the further direction in the introduction.
* [API Documentation](https://pliniker.github.io/mo-gc/), but also see the examples.
* [Implementation Notes](https://github.com/pliniker/mo-gc/blob/master/doc/Implementation-Notes.md)
* [Original draft design outline](https://github.com/pliniker/mo-gc/blob/master/doc/Project-RFC.md)
* [Original discussion issue](https://github.com/pliniker/mo-gc/issues/1) on the original design.

### See also

* [rust-gc](https://github.com/manishearth/rust-gc)
* [crossbeam](https://github.com/aturon/crossbeam/)
* [bacon-rajan-cc](https://github.com/fitzgen/bacon-rajan-cc)

### About this Project

* Copyright &copy; 2015 Peter Liniker <peter.liniker@gmail.com>
* Licensed under dual MIT/Apache-2.0
* Named after [M-O](http://pixar.wikia.com/wiki/M-O).

