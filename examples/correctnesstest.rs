#![feature(alloc_system)]
extern crate alloc_system;


extern crate mo_gc;

use mo_gc::{GcThread, GcRoot, StatsLogger, Trace};


struct Thing {
    value: [usize; 4]
}


unsafe impl Trace for Thing {}


impl Thing {
    fn new() -> Thing {
        Thing {
            value: [42; 4]
        }
    }
}


impl Drop for Thing {
    fn drop(&mut self) {
        // any heap corruption might be evident here
        assert!(self.value[0] == 42);
        assert!(self.value[1] == 42);
        assert!(self.value[2] == 42);
        assert!(self.value[3] == 42);
    }
}


fn app() {
    for _ in 0..10000000 {
        let _new = GcRoot::new(Thing::new());
    }
}


fn main() {
    let gc = GcThread::spawn_gc();

    let app_handle = gc.spawn(|| app());

    let logger = gc.join().expect("gc failed");
    logger.dump_to_stdout();

    app_handle.join().expect("app failed");
}
