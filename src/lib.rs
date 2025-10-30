use arc_swap::ArcSwap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

/// Asynchronous API
#[cfg(feature = "async")]
mod async_lock_impl;
/// Synchronous API
mod std_impl;

#[cfg(feature = "async")]
pub use crate::async_lock_impl::*;

/// A versioned lock implementation inspired by Multi-Version Concurrency Control (MVCC).
///
/// `SnapshotLock` provides concurrent access to data with lock-free reads and exclusive writes.
/// It maintains multiple versions of the data to allow readers to access consistent
/// snapshots without blocking writers.
///
/// # Type Parameters
///
/// - `T`: The type of data being protected. Must be `Clone`. Surrounding lock is `Send` and `Sync` if `T` is `Send`
///
/// # Examples
///
/// ```
/// use snaplock::SnapshotLock;
///
/// let lock = SnapshotLock::new(String::from("Hello"));
///
/// // Multiple readers can access the data concurrently
/// let reader1 = lock.read();
/// let reader2 = lock.read();
/// assert_eq!(*reader1, "Hello");
/// assert_eq!(*reader2, "Hello");
///
/// // Writers have exclusive access
/// let mut writer = lock.write();
/// writer.push_str(", World!");
///
/// // During write readers see old values
/// let reader3 = lock.read();
/// assert_eq!(*reader3, "Hello");
/// drop(writer);
///
/// // When write is finished, new readers see the updated value
/// let reader4 = lock.read();
/// assert_eq!(*reader4, "Hello, World!");
///
/// // Old readers still see their old value
/// assert_eq!(*reader1, "Hello");
/// assert_eq!(*reader2, "Hello");
/// assert_eq!(*reader3, "Hello");
/// ```
pub struct SnapshotLock<T, Inner> {
    inner: Inner,
    latest: ArcSwap<T>,
    latest_version: AtomicUsize,
}

/// A read guard that provides immutable access to a snapshot of `SnapshotLock` data.
///
/// This guard holds a shared reference to the data and ensures that the
/// snapshot remains consistent throughout its lifetime. Multiple read guards
/// can exist concurrently without blocking each other.
///
/// # Type Parameters
///
/// - `T`: The type of data being accessed.
///
/// # Examples
///
/// ```
/// # use snaplock::SnapshotLock;
/// let lock = SnapshotLock::new(100);
/// let guard = lock.read();
/// assert_eq!(*guard, 100);
/// ```
pub struct SnapshotLockReadGuard<T>(pub Arc<T>);

impl<T> Deref for SnapshotLockReadGuard<T> {
    type Target = T;

    /// Dereferences to the contained data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use snaplock::SnapshotLock;
    /// let lock = SnapshotLock::new("hello");
    /// let guard = lock.read();
    /// assert_eq!(*guard, "hello");
    /// ```
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

/// A write guard that provides exclusive mutable access to `SnapshotLock` data.
///
/// This guard holds an exclusive lock on the data and allows modifications.
/// When the guard is dropped, the version number is incremented and the
/// changes become visible to subsequent readers.
///
/// # Type Parameters
///
/// - `T`: The type of data being modified.
/// - `'a`: The lifetime of the lock guard.
///
/// # Examples
///
/// ```
/// # use snaplock::SnapshotLock;
/// let lock = SnapshotLock::new(vec![1, 2, 3]);
/// {
///     let mut guard = lock.write();
///     guard.push(4);
/// } // Changes are committed and version incremented when guard is dropped
/// let reader = lock.read();
/// assert_eq!(*reader, vec![1, 2, 3, 4]);
/// ```
pub struct SnapshotLockWriteGuard<'a, T, Guard: 'a>(pub Guard, PhantomData<&'a T>);

impl<'a, T, Guard: 'a + Deref<Target = (T, usize)>> Deref for SnapshotLockWriteGuard<'a, T, Guard> {
    type Target = T;

    /// Dereferences to the contained data for reading.
    ///
    /// # Examples
    ///
    /// ```
    /// # use snaplock::SnapshotLock;
    /// let lock = SnapshotLock::new(42);
    /// let guard = lock.write();
    /// assert_eq!(*guard, 42);
    /// ```
    fn deref(&self) -> &Self::Target {
        let (value, _) = self.0.deref();
        value
    }
}

impl<'a, T, Guard: 'a + DerefMut<Target = (T, usize)>> DerefMut
    for SnapshotLockWriteGuard<'a, T, Guard>
{
    /// Provides mutable access to the contained data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use snaplock::SnapshotLock;
    /// let lock = SnapshotLock::new(String::from("hello"));
    /// {
    ///     let mut guard = lock.write();
    ///     guard.push_str(" world");
    /// } // Changes committed here
    /// assert_eq!(*lock.read(), "hello world");
    /// ```
    fn deref_mut(&mut self) -> &mut Self::Target {
        let (value, _) = self.0.deref_mut();
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_should_not_ub_on_read_after_write_drop() {
        let lock = SnapshotLock::new(String::from("Hello"));
        let mut writer = lock.write();

        writer.push_str(", World!");
        let reader = lock.read();
        assert_eq!(*reader, "Hello", "Should read under write lock");

        drop(writer);

        assert_eq!(
            *reader, "Hello",
            "Should read old data after the drop of write lock"
        );
    }
}
