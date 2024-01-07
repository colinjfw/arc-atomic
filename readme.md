Provides an atomic pointer to an [`Arc`]

The [`AtomicArc`] is an `AtomicPtr` to an [`Arc`], supporting atomic swap and
store operations. The underlying [`Arc`] reference counts manage deallocation of
the underlying memory whereas the [`AtomicArc`] manages which underlying [`Arc`]
is loaded by a thread at any point in time. Specifically, load operations on an
[`AtomicArc`] increment the reference count returning a strong reference to the
[`Arc`] ensuring that the underlying memory is only dropped when all references
have been dropped.

# Example

It's common to wrap [`AtomicArc`] in an [`Arc`] itself to allow sharing across
threads.

```
# use std::sync::Arc;
# use std::thread;
# use atomic_arc::AtomicArc;
let arc = Arc::new(AtomicArc::new(Arc::new(1)));
let handle = arc.clone();
thread::spawn(move || {
    let val = *handle.load();
    println!("{val}"); // may print '1' or '2'
});
let prev = *arc.swap(Arc::new(2));
println!("{prev}"); // prints '1'
```

# Design

Sequentially consistent ordering is used for all load/swap operations on the
underlying `AtomicPtr`. This ensures that a swap operation will atomically swap
a pointer to a new `Arc` which will be observed by all subsequent loads on any
thread. This behaviour is verified using tests with `loom` and compiles under
`miri`.

This crate provides similar functionality as [arc-swap](https://docs.rs/arc-swap),
although the underlying mechanism for providing swappability is simplified by
avoiding any attempt at providing a thread-local pointer cache.
