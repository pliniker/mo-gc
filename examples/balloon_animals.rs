

extern crate mo_gc;
use mo_gc::{Gc, GcRoot, GcThread, StatsLogger, Trace, TraceOps, TraceStack};


struct Segment {
    next: Gc<Segment>,
}


impl Segment {
    fn new() -> Segment {
        Segment {
            next: Gc::null()
        }
    }

    fn join_to(&mut self, to: Gc<Segment>) {
        self.next = to;
    }
}


unsafe impl Trace for Segment {
    fn traversible(&self) -> bool {
        true
    }

    unsafe fn trace(&self, heap: &mut TraceStack) {
        if let Some(ptr) = self.next.as_raw() {
            heap.push_to_trace(&*ptr);
        }
    }
}


struct Balloon {
    head: Gc<Segment>,
    tail: Gc<Segment>,
}


impl Balloon {
    fn inflate() -> Balloon {
        let body = Gc::new(Segment::new());
        Balloon {
            head: body,
            tail: body,
        }
    }

    fn twist(&mut self) {
        let mut new_seg = Gc::new(Segment::new());
        new_seg.join_to(self.head);
        self.head = new_seg;
    }

    fn complete(&mut self) {
        self.tail.next = self.head;
    }

    fn count(&mut self) {
        let mut count = 0;
        let mut current = self.head;

        loop {
            current = current.next;
            count += 1;

            if current.is(self.tail) {
                break;
            }
        }

        if count != 1000 {
            println!("snake is short - only {} segments", count);
        }
    }
}


unsafe impl Trace for Balloon {
    fn traversible(&self) -> bool {
        true
    }

    unsafe fn trace(&self, heap: &mut TraceStack) {
        heap.push_to_trace(&*self.head as &Trace);
    }
}


fn snake() {
    // this many snake balloons
    for _snake in 0..5000 {
        let mut balloon = GcRoot::new(Balloon::inflate());

        // with this many segments each
        for _segment in 0..1000 {
            balloon.twist();
        }

        balloon.complete();
        balloon.count();
    }
}


fn main() {
    let gc = GcThread::spawn_gc();

    let snake_handle = gc.spawn(|| snake());

    let logger = gc.join().expect("gc failed");
    logger.dump_to_stdout();

    snake_handle.join().expect("snake failed");
}
