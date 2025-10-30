use crate::{SnapshotLock, SnapshotLockReadGuard, SnapshotLockWriteGuard};
use arc_swap::ArcSwap;
use async_lock::{RwLock, RwLockWriteGuard};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub type AsyncSnapshotLock<T> = SnapshotLock<T, RwLock<(T, usize)>>;

impl<T: Clone> SnapshotLock<T, RwLock<(T, usize)>> {
    /// Creates a new `SnapshotLock` with the initial value.
    ///
    /// # Arguments
    ///
    /// * `inner` - The initial value to be protected by the lock.
    ///
    /// # Examples
    ///
    /// ```
    /// use snaplock::SnapshotLock;
    ///
    /// let lock = SnapshotLock::new_async(42);
    /// assert_eq!(*lock.read(), 42);
    /// ```
    pub fn new_async(inner: T) -> Self {
        Self {
            inner: RwLock::new((inner.clone(), 0)),
            latest: ArcSwap::new(Arc::new(inner)),
            latest_version: AtomicUsize::new(0),
        }
    }

    /// Acquires an exclusive write lock on the protected data.
    ///
    /// This method won't yield until no other writers hold the lock. The returned
    /// `SnapshotLockWriteGuard` allows mutable access to the data. When the guard is
    /// dropped, the version number is incremented and subsequent reads will
    /// see the updated value.
    ///
    /// # Returns
    ///
    /// A `SnapshotLockWriteGuard` that provides mutable access to the data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use snaplock::SnapshotLock;
    ///
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let lock = SnapshotLock::new_async(0);
    /// {
    ///     let mut guard = lock.write().await;
    ///     *guard = 42;
    /// } // Guard dropped here, version incremented
    /// assert_eq!(*lock.read(), 42);
    /// # })
    /// ```
    pub async fn write(&self) -> SnapshotLockWriteGuard<'_, T, RwLockWriteGuard<'_, (T, usize)>> {
        let mut guard = self.inner.write().await;
        guard.1 += 1; // increment version
        SnapshotLockWriteGuard(guard, PhantomData)
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
    /// A `SnapshotLockReadGuard` that provides immutable access to a snapshot of the data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use snaplock::SnapshotLock;
    ///
    /// let lock = SnapshotLock::new_async("data");
    /// let guard = lock.read();
    /// assert_eq!(*guard, "data");
    /// ```
    pub fn read(&self) -> SnapshotLockReadGuard<T> {
        if let Some(read) = self.inner.try_read() {
            let (data, version) = read.deref();
            if self.latest_version.swap(*version, Ordering::Acquire) < *version {
                self.latest.store(Arc::new(data.clone()));
            }
        }

        SnapshotLockReadGuard(arc_swap::Guard::into_inner(self.latest.load()))
    }
}
