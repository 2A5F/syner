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

    pub fn ptr(&self) -> Ptr<T> {
        Ptr::new(self._ptr)
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

unsafe impl<T> Send for BoxPtr<T> {}
unsafe impl<T> Sync for BoxPtr<T> {}

pub struct Ptr<T> {
    _ptr: NonNull<UnsafeCell<T>>,
    __: std::marker::PhantomData<T>,
}

impl<T> Clone for Ptr<T> {
    fn clone(&self) -> Self {
        Self {
            _ptr: self._ptr.clone(),
            __: self.__.clone(),
        }
    }
}
impl<T> Copy for Ptr<T> {}

impl<T> Ptr<T> {
    pub fn new(ptr: NonNull<UnsafeCell<T>>) -> Self {
        Self {
            _ptr: ptr,
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

unsafe impl<T> Send for Ptr<T> {}
unsafe impl<T> Sync for Ptr<T> {}
