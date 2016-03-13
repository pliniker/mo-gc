
extern crate stopwatch;
use stopwatch::Stopwatch;

extern crate mo_gc;

use mo_gc::{GcThread, GcRoot, Trace, StatsLogger};


const THING_SIZE: usize = 8;
const THING_COUNT: i64 = 2500000;


struct Thing {
    _data: [u64; THING_SIZE],
}


impl Thing {
    fn new() -> Thing {
        Thing { _data: [0; THING_SIZE] }
    }
}


unsafe impl Trace for Thing {}


fn app() {
    let sw = Stopwatch::start_new();

    for _ in 0..THING_COUNT {
        let _new = GcRoot::new(Thing::new());
    }

    let per_second = (THING_COUNT * 1000) / sw.elapsed_ms();
    println!("app allocated {} objects at {} objects per second", THING_COUNT, per_second);
    println!("app finished in {}ms", sw.elapsed_ms());
}


fn main() {
    let gc = GcThread::spawn_gc();

    let app_handle1 = gc.spawn(|| app());
    let app_handle2 = gc.spawn(|| app());

    let logger = gc.join().expect("gc failed");
    logger.dump_to_stdout();

    app_handle1.join().expect("app failed");
    app_handle2.join().expect("app failed");
}
