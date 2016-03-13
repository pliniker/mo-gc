//! The Trace trait must be implemented by every type that can be GC managed.


use heap::TraceStack;


/// Trace trait. Every type that can be managed by the GC must implement this trait.
/// This trait is unsafe in that incorrectly implementing it can cause Undefined Behavior.
pub unsafe trait Trace {
    /// If the type can contain GC managed pointers, this must return true
    fn traversible(&self) -> bool {
        false
    }

    /// If the type can contain GC managed pointers, this must visit each pointer.
    /// This function must be thread-safe! It must read a snapshot of the data structure it is
    /// implemented for.
    unsafe fn trace(&self, _stack: &mut TraceStack) {}
}


unsafe impl Trace for usize {}
unsafe impl Trace for isize {}
unsafe impl Trace for i8 {}
unsafe impl Trace for u8 {}
unsafe impl Trace for i16 {}
unsafe impl Trace for u16 {}
unsafe impl Trace for i32 {}
unsafe impl Trace for u32 {}
unsafe impl Trace for i64 {}
unsafe impl Trace for u64 {}
unsafe impl Trace for f32 {}
unsafe impl Trace for f64 {}
unsafe impl<'a> Trace for &'a str {}
unsafe impl Trace for String {}
