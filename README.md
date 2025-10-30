# Snapshot Lock | [snaplock](https://crates.io/crates/snaplock)

[crates.io](https://crates.io/crates/snaplock)

*MVCC without transactions or something*

> [!WARNING]
> This library is not production ready

Synchronous ReadWrite lock for Rust leveraging data versioning.

Example:

```rust
use snaplock::SnapshotLock;

fn main() {
    let lock = SnapshotLock::new(String::from("Hello"));
    
    // Multiple readers can access the data concurrently
    let reader1 = lock.read();
    let reader2 = lock.read();
    assert_eq!(*reader1, "Hello");
    assert_eq!(*reader2, "Hello");
    
    // Writers have exclusive access
    {
        let mut writer = lock.write();
        writer.push_str(", World!");
        
        // During write readers see old values
        let reader = lock.read();
        assert_eq!(*reader, "Hello");
    }
    
    // When write is finished, new readers see the updated value
    let reader3 = lock.read();
    assert_eq!(*reader3, "Hello, World!");
    
    // Old readers still see their old value
    assert_eq!(*reader1, "Hello");
    assert_eq!(*reader2, "Hello");
}
```

### MSRV Policy:

There is none. Current Minimum Supported Rust Version is 1.85.0

### Alternatives:
- [`crossbeam::sync::SharedLock`](https://docs.rs/crossbeam/latest/crossbeam/sync/struct.ShardedLock.html) - A sharded reader-writer lock.
- [vlock](https://crates.io/crates/vlock) - A fast and scalable multi-version shared state lock with wait-free read access.
- [parking_lot](https://crates.io/crates/parking_lot) - More compact and efficient implementations of the standard synchronization primitives.