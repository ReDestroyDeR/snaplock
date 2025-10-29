mod cell;

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLockWriteGuard, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::cell::CloneCell;

/// A versioned lock implementation inspired by Multi-Version Concurrency Control (MVCC).
///
/// `VLock` provides concurrent access to data with lock-free reads and exclusive writes.
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
/// use vlock::VLock;
///
/// let lock = VLock::new(String::from("Hello"));
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
pub struct VLock<T> {
    inner: RwLock<(T, usize)>,
    latest: CloneCell<Arc<T>>,
    latest_version: AtomicUsize,
}

// todo: prove. set up miri
unsafe impl<T: Send> Send for VLock<T> {}
unsafe impl<T: Send> Sync for VLock<T> {}

impl<T: Clone> VLock<T> {
    /// Creates a new `VLock` with the initial value.
    ///
    /// # Arguments
    ///
    /// * `inner` - The initial value to be protected by the lock.
    ///
    /// # Examples
    ///
    /// ```
    /// use vlock::VLock;
    ///
    /// let lock = VLock::new(42);
    /// assert_eq!(*lock.read(), 42);
    /// ```
    pub fn new(inner: T) -> Self {
        Self {
            inner: RwLock::new((inner.clone(), 0)),
            latest: CloneCell::new(Arc::new(inner)),
            latest_version: AtomicUsize::new(0),
        }
    }

    /// Acquires an exclusive write lock on the protected data.
    ///
    /// This method blocks until no other writers hold the lock. The returned
    /// `VLockWriteGuard` allows mutable access to the data. When the guard is
    /// dropped, the version number is incremented and subsequent reads will
    /// see the updated value.
    ///
    /// # Returns
    ///
    /// A `VLockWriteGuard` that provides mutable access to the data.
    ///
    /// # Examples
    ///
    /// ```
    /// use vlock::VLock;
    ///
    /// let lock = VLock::new(0);
    /// {
    ///     let mut guard = lock.write();
    ///     *guard = 42;
    /// } // Guard dropped here, version incremented
    /// assert_eq!(*lock.read(), 42);
    /// ```
    pub fn write(&self) -> VLockWriteGuard<'_, T> {
        let mut guard = self.inner.write().unwrap();
        guard.1 += 1; // increment version
        VLockWriteGuard(guard)
    }

    /// Acquires a shared read lock on the protected data.
    ///
    /// This method attempts to get the latest version of the data with minimal
    /// blocking. It first tries a non-blocking read of the current version,
    /// and if that fails or detects a newer version, it falls back to a
    /// cached snapshot.
    ///
    /// # Returns
    ///
    /// A `VLockReadGuard` that provides immutable access to a snapshot of the data.
    ///
    /// # Examples
    ///
    /// ```
    /// use vlock::VLock;
    ///
    /// let lock = VLock::new("data");
    /// let guard = lock.read();
    /// assert_eq!(*guard, "data");
    /// ```
    pub fn read(&self) -> VLockReadGuard<T> {
        if let Ok(read) = self.inner.try_read() {
            let (data, version) = read.deref();
            if self.latest_version.swap(*version, Ordering::Acquire) < *version {
                self.latest.set(Arc::new(data.clone()));
            }
        }

        VLockReadGuard(self.latest.get())
    }
}

/// A read guard that provides immutable access to a snapshot of `VLock` data.
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
/// use vlock::VLock;
///
/// let lock = VLock::new(100);
/// let guard = lock.read();
/// assert_eq!(*guard, 100);
/// ```
pub struct VLockReadGuard<T>(pub Arc<T>);

impl<T> Deref for VLockReadGuard<T> {
    type Target = T;

    /// Dereferences to the contained data.
    ///
    /// # Examples
    ///
    /// ```
    /// use vlock::VLock;
    ///
    /// let lock = VLock::new("hello");
    /// let guard = lock.read();
    /// assert_eq!(*guard, "hello");
    /// ```
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

/// A write guard that provides exclusive mutable access to `VLock` data.
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
/// use vlock::VLock;
///
/// let lock = VLock::new(vec![1, 2, 3]);
/// {
///     let mut guard = lock.write();
///     guard.push(4);
/// } // Changes are committed and version incremented when guard is dropped
/// let reader = lock.read();
/// assert_eq!(*reader, vec![1, 2, 3, 4]);
/// ```
pub struct VLockWriteGuard<'a, T>(pub RwLockWriteGuard<'a, (T, usize)>);

impl<'a, T> Deref for VLockWriteGuard<'a, T> {
    type Target = T;

    /// Dereferences to the contained data for reading.
    ///
    /// # Examples
    ///
    /// ```
    /// use vlock::VLock;
    ///
    /// let lock = VLock::new(42);
    /// let guard = lock.write();
    /// assert_eq!(*guard, 42);
    /// ```
    fn deref(&self) -> &Self::Target {
        let (value, _) = self.0.deref();
        value
    }
}

impl<'a, T> DerefMut for VLockWriteGuard<'a, T> {
    /// Provides mutable access to the contained data.
    ///
    /// # Examples
    ///
    /// ```
    /// use vlock::VLock;
    ///
    /// let lock = VLock::new(String::from("hello"));
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
        let lock = VLock::new(String::from("Hello"));
        let mut writer = lock.write();

        writer.push_str(", World!");
        let reader = lock.read();
        assert_eq!(*reader, "Hello", "Should read under write lock");

        drop(writer);

        assert_eq!(*reader, "Hello", "Should read old data after the drop of write lock");
    }
}