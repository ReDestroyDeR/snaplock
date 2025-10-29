use std::cell::{RefCell, RefMut};
use std::sync::Arc;

#[derive(Clone)]
pub struct CloneCell<T> {
    inner: Arc<RefCell<T>>,
}

impl<T> CloneCell<T> {
    pub fn new(val: T) -> Self {
        Self {
            inner: Arc::new(RefCell::new(val)),
        }
    }
}

impl<T: Clone> CloneCell<T> {
    pub fn get(&self) -> T {
        self.inner.borrow().clone()
    }

    pub fn set(&self, value: T) {
        let mut borrowed: Result<RefMut<T>, _> = self.inner.try_borrow_mut();

        while borrowed.is_err() {
            borrowed = self.inner.try_borrow_mut()
        }

        *borrowed.unwrap() = value;
    }
}
