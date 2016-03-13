//! Numerous constants used as parameters to GC behavior


// Journal parameters
pub const JOURNAL_BUFFER_SIZE: usize = 32768;
pub const BUFFER_RUN: usize = 1024;
pub const JOURNAL_RUN: usize = 32;
pub const MAX_SLEEP_DUR: usize = 100;  // milliseconds
pub const MIN_SLEEP_DUR: usize = 1;    // milliseconds
pub const MAJOR_COLLECT_THRESHOLD: usize = 1 << 20;

// Cache line in bytes
pub const CACHE_LINE: usize = 64;

// Bits and masks
pub const PTR_MASK: usize = !3;
pub const MARK_BIT: usize = 1;
pub const MARK_MASK: usize = !1;
pub const TRAVERSE_BIT: usize = 2;

// mask for low bits of address of object through journal
pub const FLAGS_MASK: usize = 3;

// bit number that indicates whether a reference count is being incremented
pub const INC_BIT: usize = 1;
// // bit number that indicates whether or not an object is newly allocated or not
pub const NEW_BIT: usize = 2;
pub const NEW_MASK: usize = !2;

// Values found in the 2 bits masked by FLAGS_MASK
// new object, increment refcount value
pub const NEW_INC: usize = 3;
// new object not rooted value
pub const NEW: usize = 2;
// old object, increment refcount value
pub const INC: usize = 1;
// decrement refcount value
pub const DEC: usize = 0;
