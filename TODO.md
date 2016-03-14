# Testing

* integration tests
* benchmarks

# Examples

* build some data structures, esp concurrent data structures
* see crossbeam for treiber stack example

# Issues

## Race condition

There is currently a race condition where a pointer is read from the heap, rooted and then that
pointer value on the heap is overwritten during the mark/sweep phase of collection. The
rooting should ensure that the referenced object is marked, but the journal is not being
read at this point and the reference count increment is too late to stop the object from being
swept.

This race condition means that the mutator threads cannot currently use this GC as fully general
purpose, or rather that data structures must be persistent.

The sequence of events causing the race condition is:

 * GC stops reading journal, enters mark phase
 * mutator reads pointer to object A from heap, roots A, writing to journal
 * mutator overwrites pointer on heap with new object B reference
 * GC traces heap, marking new object B but not previously referenced object A
 * GC sweeps, dropping A even though A was rooted

The benefit of fixing this issue is that this GC design becomes general purpose.

### Additional write barrier

This race condition might be avoided by an additional synchronous write barrier: if a pointer A
on the heap is going to be replaced by pointer B, the object A might be marked as "pinned"
to prevent the sweep phase from dropping it. The sweep phase would unpin the object, after
which if it has been rooted, the reference count increment will be picked up from the journal
before the next mark phase.

This solution has the downside of adding a word to the size of every object,
the cost of an atomic store on the app-thread side and the cost of an atomic load and store
on the sweep phase. It would also make programs that use this GC less fork-friendly, as
pinning objects would incur copy-on-write costs for memory pages that might otherwise remain
read-only.

Question: just how atomic would the pinning operation need to be? It only needs to take effect
during the mark phase but the pin flag would need to be readable by the sweep phase.

Experimentation will determine if this mechanism is worth the cost. There may be alternative
implementation options that are more efficient: perhaps using a shared data structure to
write pinned object pointers to that is consumed by a phase between mark and sweep that
sets the marked flag on those objects?

### Use the journal

The journal contains the rooting information needed to avoid this problem. Another possible
solution may be to read the journal in the mark phase, _after_ marking any new roots, before
moving on to the sweep phase.

This needs further thought.

## Performance Bottlenecks

### Journal processing

`Trie::set()` is the bottleneck in `YoungHeap::read_journals()`. This is a single-threaded
function and consumes most of the GC linear time. It is the single greatest throughput limiter.
If insertion into `bitmaptrie::Trie` could be parallelized, throughput would improve.

One option is to process each mutator journal on a separate thread but defer new-object
insertion to a single thread. This way some parallelism is gained for processing reference
count increments. This is still not optimal though.

### The Allocator

Building on the generic allocator: jemalloc maintains a radix trie for allocation so there
are two tries, increasing CPU and memory requirements. A custom allocator would
solve this problem, but would introduce the problem of writing a scalable, fragmentation-
minimizing allocator.

## Collection Scheduling

This is currently very simple and has not been tuned at all.
See `gcthread::gc_thread()` and `constants::*` for tuning.
