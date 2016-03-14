
* Date: 2016-03-13

# Implementation Notes

The current implementation has been tested on x86 and x86_64. It has not bee tested on
ARM, though the ARM weaker memory model may highlight some flaws.

## The journal

The journal is designed to never block the mutator. Each mutator thread allocates a buffer to
write reference count adjustments to. When the buffer is full, a new buffer is allocated.
The GC thread consumes the buffers. Thus the journal behaves like an infinitely sized
SPSC queue. Each mutator gets its own journal.

The values written by the mutator to a buffer are essentially `TraitObject`s that describe
a pointer to an object and the `Trace` trait virtual table. The virtual table pointer is
required to provide the `drop()` and `Trace::trace()` methods, as the GC thread does not
know concrete types at runtime.

Because heap allocations are word aligned, a pointer's two least significant bits can be used
as bit flags.

The object address has four possible values in it's LSBs:

* 0: reference count decrement
* 1: reference count increment
* 2: new object allocated, no reference count adjustment
* 3: new object allocated, reference count increment

The object vtable has one flag value that can be set:

* 2: the object is a container of other GC-managed objects and must be traced. This flag saves
  the mark phase from making virtual function calls for scalar objects.

### Advantages

The mutator thread will never be blocked on writing to the journal unless the application hits
out-of-memory, thus providing a basic pauselessness guarantee.

The journal is very fast, not requiring atomics on the x86/64 TSO-memory-model architecture.

### Disadvantages

If the GC thread cannot keep up with the mutator(s), the journal will continue to allocate
new buffers faster than the GC thread can consume them, contributing to the OOM death march.

## Young generation heap and root reference counts

A young-generation heap map is implemented using a bitmapped vector trie, whose indeces are
word-sized: keys are object addresses, values are a composition of root reference count, the object
vtable and a word for flags for marking and sweeping.

The addresses used as keys are right-shifted to eliminate the least significant bits that are
always zero because heap allocations are word aligned.

The flags set on the object address have been processed at this point and the heap updated
accordingly. Reference count decrements are written to a deferred buffer for processing later.

For new objects, the heap map flags for the object are marked as `NEW`. These are the young
generation objects. Other entries in the map not marked as `NEW` are stack roots only.

Thus the young generation heap map combines pure stack-root references and new object references.

A typical generational GC keeps a data structure such a as a card table to discover pointers from
the mature object heap into the young generation heap. Write barriers are required to update the
card table when mature objects are written to. In our case, the non-`NEW` stack-root
references act as the set of mature objects that may have references to young generation objects.
Essentially, the journal is a type of write barrier.

When the young generation heap enters a mark phase, all objects that have a non-zero reference
count are considered potential roots. Only `NEW` objects are considered during sweeping.

Both marking and sweeping are done in parallel: during the mark phase, the heap map is sharded across
multiple threads for scanning for roots while each thread can look up entries in the whole map for
marking; during the sweep phase, the heap map is sharded across multiple threads for sweeping.

### Advantages

This combined roots and new-objects map makes for a straightforwardly parallelizable mark and 
sweep implementation. The trie can be sharded into sub-tries and each sub-trie can be processed 
independently and mutated independently of the others while remaining thread safe without 
requiring locks or atomic access.

### Disadvantages

Inserting into the trie is currently not parallelizable, making reading the journal into the trie
a single-threaded affair, impacting GC throughput.

On high rates of new object allocation, the GC thread currently cannot keep up with the
mutators rate of writing to the journal. The cause of this is not the journal itself: reading
and writing the journal can be done very fast. However, inserting and updating the heap map 
causes the GC thread to process the journal at half the rate at which a single mutator thread 
can allocate new objects.

If journal processing (trie insertion) can be parallelized, the GC throughput will hugely improve.

One part-way step may be to parallelize reference count updates while still processing new
objects in sequence.

## The mature object heap

This heap map is similar to the young generation heap but does not consider reference counts
or new objects. Marking and sweeping is parallelized similarly.

A mature heap collection is triggered when the young generation heap reaches a threshold count of
`NEW` objects that it is managing. `NEW` object data is copied to the mature heap trie and
the original entries in the young generation are unmarked as `NEW`. They become plain stack
root entries.
