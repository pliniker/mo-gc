
* Date: 2015-08-24
* Discussion issue: [mo-gc#1](https://github.com/pliniker/mo-gc/issues/1)

# Summary

Application threads maintain precise-rooted GC-managed pointers through smart
pointers on the stack that write reference-count increments and decrements to a
write-only journal. The reference-count journal is read by a GC thread that
maintains the actual reference count numbers in a cache of GC roots. When a
reference count reaches zero, the GC thread moves the pointer to a heap cache
data structure that keeps no reference counts but that is used for mark and
sweep collection.

Because the GC thread has no synchronization between itself
and the application threads besides the inc/dec journal, all data structures
that contain nested GC-managed pointers must be immutable in their GC-managed
relationships: persistent data structures must be used to avoid data races.

...
...
