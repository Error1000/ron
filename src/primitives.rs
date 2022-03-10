use core::{sync::atomic::AtomicBool, cell::UnsafeCell};
use core::ops::{Deref, DerefMut};

pub struct MutexGuard<'lock_lifetime, T>{
    lock_ref: &'lock_lifetime AtomicBool,
    inner_ref: &'lock_lifetime mut T
}

pub struct Mutex<T> {
    lock: AtomicBool,
    inner: UnsafeCell<T>,
}

unsafe impl<T> Sync for Mutex<T>{ }
unsafe impl<T> Send for Mutex<T>{ }

impl<T> Mutex<T> {
    pub fn is_locked(&self) -> bool{
        self.lock.load(core::sync::atomic::Ordering::Relaxed)
    }

    pub const fn from(val: T) -> Self{
        Self{
            inner: UnsafeCell::new(val),
            lock: AtomicBool::new(false)
        }
    }

    pub fn lock(&self) -> MutexGuard<T>{
        while self.lock.compare_exchange_weak(false, true, core::sync::atomic::Ordering::Acquire, core::sync::atomic::Ordering::Relaxed).is_err() {
            while self.is_locked(){ core::hint::spin_loop(); }
        }
        
       MutexGuard{
        lock_ref: &self.lock,
        inner_ref: unsafe{&mut *self.inner.get()},
      }
    }

}

impl<'lock_lifetime, T> Deref for MutexGuard<'lock_lifetime, T>{
    type Target = T;
    fn deref(&self) -> &Self::Target{
        self.inner_ref
    }
}


impl<'lock_lifetime, T> DerefMut for MutexGuard<'lock_lifetime, T>{
    fn deref_mut(&mut self) -> &mut Self::Target{
        self.inner_ref
    }
}

impl<'lock_lifetime, T> Drop for MutexGuard<'lock_lifetime, T>{
    fn drop(&mut self){
        while self.lock_ref.compare_exchange_weak(true, false, core::sync::atomic::Ordering::Release, core::sync::atomic::Ordering::Relaxed).is_err(){
            core::hint::spin_loop();
        }
    }
}