//! Garbage collection thread


use std::any::Any;
use std::cmp::min;
use std::mem::size_of;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use num_cpus;
use scoped_pool::Pool;

use appthread::AppThread;
use constants::{MAJOR_COLLECT_THRESHOLD, MAX_SLEEP_DUR, MIN_SLEEP_DUR};
use heap::{CollectOps, Object};
use journal;
use parheap::ParHeap;
use statistics::{StatsLogger, DefaultLogger};
use youngheap::YoungHeap;


pub type EntryReceiver = journal::Receiver<Object>;
pub type EntrySender = journal::Sender<Object>;

pub type JournalReceiver = mpsc::Receiver<EntryReceiver>;
pub type JournalSender = mpsc::Sender<EntryReceiver>;

pub type JournalList = Vec<EntryReceiver>;


/// The Garbage Collection thread handle.
pub struct GcThread<S: StatsLogger> {
    /// This is cloned and given to app threads.
    tx_chan: JournalSender,

    /// The GC thread's handle to join on.
    handle: thread::JoinHandle<S>,
}


impl GcThread<DefaultLogger> {
    /// Spawn a GC thread with default parameters: a `ParHeap` and a `DefaultLogger` parallelized
    /// across all available CPUs.
    pub fn spawn_gc() -> GcThread<DefaultLogger> {
        let cores = num_cpus::get();
        Self::spawn_gc_with(cores, ParHeap::new(cores), DefaultLogger::new())
    }
}


impl<S: StatsLogger + 'static> GcThread<S> {
    /// Run the GC on the current thread, spawning another thread to run the application function
    /// on. Returns the AppThread std::thread::Thread handle. Caller must provide a custom
    /// StatsLogger implementation and a CollectOps heap implementation.
    pub fn spawn_gc_with<T>(num_threads: usize, mature: T, logger: S) -> GcThread<S>
        where T: CollectOps + Send + 'static
    {
        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || gc_thread(num_threads, rx, mature, logger));

        GcThread {
            tx_chan: tx,
            handle: handle,
        }
    }

    /// Spawn an app thread that journals to the GC thread.
    pub fn spawn<F, T>(&self, f: F) -> thread::JoinHandle<T>
        where F: FnOnce() -> T,
              F: Send + 'static,
              T: Send + 'static
    {
        AppThread::spawn_from_gc(self.tx_chan.clone(), f)
    }

    /// Wait for the GC thread to finish. On success, returns the object that implements
    /// `StatsLogger` for the calling thread to examine.
    pub fn join(self) -> Result<S, Box<Any + Send + 'static>> {
        self.handle.join()
    }
}


/// Main GC thread loop.
fn gc_thread<S, T>(num_threads: usize, rx_chan: JournalReceiver, mature: T, logger: S) -> S
    where S: StatsLogger,
          T: CollectOps + Send
{
    let mut pool = Pool::new(num_threads);

    let mut gc = YoungHeap::new(num_threads, mature, logger);

    // block, wait for first journal
    gc.add_journal(rx_chan.recv().expect("Failed to receive first app journal!"));

    gc.logger().mark_start_time();

    // next duration to sleep if all journals are empty
    let mut sleep_dur: usize = 0;

    // loop until all journals are disconnected
    while gc.num_journals() > 0 {

        // new appthread connected
        if let Ok(journal) = rx_chan.try_recv() {
            gc.add_journal(journal);
        }

        let entries_read = gc.read_journals();

        // sleep if nothing read from journal
        if entries_read == 0 {
            thread::sleep(Duration::from_millis(sleep_dur as u64));

            gc.logger().add_sleep(sleep_dur);

            // back off exponentially up to the max
            sleep_dur = min(sleep_dur * 2, MAX_SLEEP_DUR);
        } else {
            // reset next sleep duration on receiving no entries
            sleep_dur = MIN_SLEEP_DUR;
        }

        // TODO: base this call on a duration since last call?
        let young_count = gc.minor_collection(&mut pool);

        // do a major collection if the young count reaches a threshold and we're not just trying
        // to keep up with the app threads
        // TODO: force a major collection every n minutes
        if sleep_dur != MIN_SLEEP_DUR && young_count >= MAJOR_COLLECT_THRESHOLD {
            gc.major_collection(&mut pool);
        }
    }

    // do a final collection where all roots should be unrooted
    gc.minor_collection(&mut pool);
    gc.major_collection(&mut pool);

    // return logger to calling thread
    gc.logger().mark_end_time();
    gc.shutdown()
}


/// Pointers are word-aligned, meaning the least-significant 2 or 3 bits are always 0, depending
/// on the word size.
#[inline]
pub fn ptr_shift() -> i32 {
    if size_of::<usize>() == 32 {
        2
    } else {
        3
    }
}
