#![doc = include_str!("../readme.md")]

use std::{fmt, mem, ptr};

#[cfg(not(loom))]
use std::sync::{
    atomic::{AtomicPtr, Ordering},
    Arc,
};

#[cfg(loom)]
use loom::sync::{
    atomic::{AtomicPtr, Ordering},
    Arc,
};

/// An atomic pointer to an [`Arc`].
///
/// This pointer provides a safe atomic pointer to an [`Arc`]. Each load will
/// clone the [`Arc`] ensuring that concurrent reads/writes will only drop the
/// value when all references are decremented. The inner [`Arc`] can be swapped
/// atomically with another value.
///
/// This value is not itself cloneable and can itself be wrapped in an [`Arc`].
pub struct AtomicArc<T> {
    ptr: AtomicPtr<T>,
}

impl<T> AtomicArc<T> {
    /// Creates a new atomic pointer to an [`Arc`].
    pub fn new(arc: Arc<T>) -> Self {
        let raw = Arc::into_raw(arc) as *mut _;
        let ptr = AtomicPtr::new(ptr::null_mut());
        ptr.store(raw, Ordering::SeqCst);
        Self { ptr }
    }

    /// Load the current value cloning the inner [`Arc`].
    ///
    /// This will increment a reference count of the current [`Arc`] and return
    /// this to the caller.
    pub fn load(&self) -> Arc<T> {
        // Ordering: Loads must be ordered globally after all store operations
        // to allow the ref-cnt of the underlying arc to track references.
        let raw = self.ptr.load(Ordering::SeqCst);

        // Safety: original arc is always created with 'into_raw'.
        unsafe {
            // We want an arc but we don't want to actually decrement the ref
            // count being held by the pointer.
            let arc = mem::ManuallyDrop::new(Arc::from_raw(raw));
            // Now clone the original and provide it to the caller.
            mem::ManuallyDrop::into_inner(arc.clone())
        }
    }

    fn swap_ptr(&self, raw: *mut T) -> Arc<T> {
        // Ordering: Orders all writes (globally) before the final drop of this value.
        let prev = self.ptr.swap(raw, Ordering::SeqCst);
        // Safety: Original arc is always created with 'into_raw'.
        unsafe {
            // Consumes the original reference count.
            Arc::from_raw(prev)
        }
    }

    /// Replace the current value, dropping the previously stored [`Arc`].
    pub fn store(&self, arc: Arc<T>) {
        let _ = self.swap(arc);
    }

    /// Swap the current value, returning the previously stored [`Arc`].
    ///
    /// The returned [`Arc`] may have additional references still held by other
    /// load calls previously requested.
    pub fn swap(&self, arc: Arc<T>) -> Arc<T> {
        let raw = Arc::into_raw(arc) as *mut _;

        self.swap_ptr(raw)
    }
}

impl<T> Drop for AtomicArc<T> {
    fn drop(&mut self) {
        let _ = self.swap_ptr(ptr::null_mut());
    }
}

impl<T: fmt::Debug> fmt::Debug for AtomicArc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AtomicArc").field(&self.load()).finish()
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use std::thread;

    use super::*;

    struct PrintOnDrop(i32);

    impl Drop for PrintOnDrop {
        fn drop(&mut self) {
            println!("drop: {}", self.0);
        }
    }

    #[test]
    fn basics() {
        let arc = AtomicArc::new(Arc::new(PrintOnDrop(1)));
        let _orig = arc.load();
        arc.swap(Arc::new(PrintOnDrop(2)));
    }

    #[test]
    fn concurrent() {
        let arc = Arc::new(AtomicArc::new(Arc::new(PrintOnDrop(1))));

        let t1 = thread::spawn({
            let handle = arc.clone();
            move || {
                for _ in 0..10 {
                    println!("{}", handle.load().0);
                }
            }
        });
        let t2 = thread::spawn({
            let handle = arc.clone();
            move || {
                for _ in 0..10 {
                    println!("{}", handle.load().0);
                }
            }
        });

        arc.swap(Arc::new(PrintOnDrop(2)));
        arc.swap(Arc::new(PrintOnDrop(3)));

        t1.join().unwrap();
        t2.join().unwrap();
    }
}

#[cfg(all(test, loom))]
mod loom_tests {
    use loom::thread;

    use super::*;

    #[test]
    fn single_thread() {
        loom::model(|| {
            let arc = Arc::new(AtomicArc::new(Arc::new(0)));
            let handle = arc.clone();
            thread::spawn(move || {
                let v = *handle.load();
                assert!(v == 0 || v == 1);
            });
            arc.swap(Arc::new(1));
        });
    }

    #[test]
    fn two_threads() {
        loom::model(|| {
            let arc = Arc::new(AtomicArc::new(Arc::new(0)));
            let handle1 = arc.clone();
            let handle2 = arc.clone();
            thread::spawn(move || {
                let v = *handle1.load();
                assert!(v == 0 || v == 1);
            });
            thread::spawn(move || {
                let v = *handle2.load();
                assert!(v == 0 || v == 1);
            });
            arc.swap(Arc::new(1));
        });
    }

    #[test]
    fn init_and_swap() {
        loom::model(|| {
            let arc = Arc::new(AtomicArc::new(Arc::new(0)));
            arc.swap(Arc::new(1));
            let handle = arc.clone();
            thread::spawn(move || {
                let v = *handle.load();
                assert!(v == 1 || v == 2);
            });
            arc.swap(Arc::new(2));
        });
    }
}
