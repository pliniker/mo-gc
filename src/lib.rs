#![feature(alloc)]
#![feature(core_intrinsics)]
#![feature(heap_api)]
#![feature(raw)]
#![feature(unique)]


//! # mo-gc
//!
//! A pauseless, concurrent, generational, parallel mark-and-sweep garbage collector.
//!
//! This is an experimental design to research an idea into a pauseless garbage collector.
//!
//! The GC handles multiple OS thread mutators without stopping their worlds. It does this by
//! deferring reference counting of stack-rooted pointers to the GC thread through a journal
//! of stack root changes. The journal itself is fast to write to, adding an amortized 25% to
//! the cost of `Box::new()` using jemalloc for a 64 byte object.
//!
//! Thus the mutator never needs to be stopped for it's stack to be scanned or for any collection
//! phase.
//!
//! See [project TODO](https://github.com/pliniker/mo-gc/blob/master/TODO.md) for limitations.
//!
//! ## Usage
//!
//! Usage is best illustrated by the examples provided.


extern crate bitmaptrie;
extern crate num_cpus;
extern crate scoped_pool;
extern crate time;


mod appthread;
mod constants;
mod gcthread;
mod heap;
mod journal;
mod parheap;
mod statistics;
mod trace;
mod youngheap;


pub use appthread::{AppThread, Gc, GcAtomic, GcBox, GcRoot};
pub use constants::*;
pub use gcthread::GcThread;
pub use heap::{CollectOps, TraceOps, TraceStack};
pub use journal::{make_journal, Receiver, Sender};
pub use parheap::ParHeap;
pub use statistics::StatsLogger;
pub use trace::Trace;
pub use youngheap::YoungHeap;
