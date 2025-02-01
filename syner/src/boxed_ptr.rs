use std::{cell::UnsafeCell, ptr::NonNull};

pub struct BoxPtr<T> {
    _ptr: NonNull<UnsafeCell<T>>,
    __: std::marker::PhantomData<T>,
}

impl<T> BoxPtr<T> {
    pub fn new(value: T) -> Self {
        Self {
            _ptr: unsafe {
                NonNull::new_unchecked(Box::leak(Box::new(UnsafeCell::new(value))) as *mut _)
            },
            __: std::marker::PhantomData,
        }
    }

    pub fn as_ref(&self) -> &'static T {
        unsafe { &*(&*self._ptr.as_ptr()).get() }
    }

    pub fn as_mut(&self) -> &'static mut T {
        unsafe { (&mut *self._ptr.as_ptr()).get_mut() }
    }
}

impl<T> Drop for BoxPtr<T> {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self._ptr.as_ptr()) });
    }
}
