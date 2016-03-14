//! Performance counters and statistics


use std::cmp::max;

use time::{get_time, Timespec};


/// Type that provides counters for the GC to gain some measure of performance.
pub trait StatsLogger: Send {
    /// mark start of time
    fn mark_start_time(&mut self);
    /// mark end of time
    fn mark_end_time(&mut self);
    /// add a number of milliseconds that the GcThread was asleep
    fn add_sleep(&mut self, ms: usize);

    /// add a count of dropped objects
    fn add_dropped(&mut self, count: usize);
    /// give the current heap object count
    fn current_heap_size(&mut self, size: usize);

    /// print statistics
    fn dump_to_stdout(&self);

    /// log something to stdout
    fn log(&self, string: &str) {
        println!("{}", string);
    }
}


pub struct DefaultLogger {
    max_heap_size: usize,

    total_dropped: usize,
    drop_iterations: usize,

    start_time: Timespec,
    stop_time: Timespec,
    sleep_time: u64,
}


unsafe impl Send for DefaultLogger {}


impl DefaultLogger {
    pub fn new() -> DefaultLogger {
        DefaultLogger {
            max_heap_size: 0,
            total_dropped: 0,
            drop_iterations: 0,
            start_time: Timespec::new(0, 0),
            stop_time: Timespec::new(0, 0),
            sleep_time: 0,
        }
    }
}


impl StatsLogger for DefaultLogger {
    fn mark_start_time(&mut self) {
        self.start_time = get_time();
    }

    fn mark_end_time(&mut self) {
        self.stop_time = get_time();
    }

    fn add_sleep(&mut self, ms: usize) {
        self.sleep_time += ms as u64;
    }

    fn add_dropped(&mut self, count: usize) {
        self.total_dropped += count;
        self.drop_iterations += 1;
    }

    fn current_heap_size(&mut self, size: usize) {
        self.max_heap_size = max(self.max_heap_size, size);
    }

    fn dump_to_stdout(&self) {
        // calculate timing
        let total_time = max((self.stop_time - self.start_time).num_milliseconds(), 1);
        let active_time = total_time - self.sleep_time as i64;
        let percent_active_time = active_time * 100 / total_time;

        // calculate drop rate
        let dropped_per_second = self.total_dropped as i64 * 1000 / active_time;

        println!("max-heap {}; dropped {} (per second {}); active {}/{}ms ({}%)",
                 self.max_heap_size,
                 self.total_dropped,
                 dropped_per_second,
                 active_time,
                 total_time,
                 percent_active_time);
    }
}
