#![feature(alloc)]
#![feature(core_intrinsics)]
#![feature(heap_api)]
#![feature(raw)]
#![feature(unique)]


//! # MO-GC
//!
//! A pauseless, concurrent, generational, parallel mark-and-sweep garbage collector.
//!
//! ## Capabilities and Benefits
//!
//! This is an experimental design to research an idea into a truly pauseless garbage collector.
//!
//! The GC handles multiple OS thread mutators without stopping their worlds. It does this by
//! deferring reference counting of stack-rooted pointers to the GC thread through a journal
//! of stack root changes. The journal itself is fast to write to, adding about 25% to
//! the cost of `Box::new()` for a 64 byte object.
//!
//! Thus the mutator never needs to be stopped for it's stack to be scanned or for any collection
//! phase.
//!
//! ## Limitations
//!
//! ### Race Conditions
//!
//! There is currently a race condition that means that data structures must be persistent, they
//! cannot be fully mutable.
//!
//! ### Throughput
//!
//! `Trie::set()` is the bottleneck in `YoungHeap::read_journals()`. This is a single-threaded
//! function and consumes most of the GC linear time. It is the single greatest throughput limiter.
//!
//! ### Collection Scheduling
//!
//! This is currently very simple and has not been tuned at all. Minor heap collection occurs after
//! every journal read and major collections occur when the minor heap object count reaches a
//! threshold.
//!
//! ## Usage
//!
//! Usage is best illustrated by the examples and tests provided.


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
pub use gcthread::GcThread;
pub use heap::{CollectOps, TraceOps, TraceStack};
pub use parheap::ParHeap;
pub use statistics::StatsLogger;
pub use trace::Trace;
