use core::{sync::atomic::AtomicBool, cell::UnsafeCell};
use core::ops::{Deref, DerefMut};
use core::fmt::{Debug, Formatter, Error};


pub struct LazyInitialised<T>{
    inner: Option<T>
}

impl<T> LazyInitialised<T>{
    pub const fn uninit() -> Self{
        Self{inner: None}
    }

    pub fn unset(&mut self){
        self.inner = None;
    }

    pub fn set(&mut self, val: T){
        self.inner = Some(val);
    }

    pub fn is_initialised(&self) -> bool { self.inner.is_some() }
}

impl<T> Debug for LazyInitialised<T>
where T: Debug {
     
fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> { 
    self.inner.fmt(f)
 }

}
impl<T> Deref for LazyInitialised<T>{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("Value should be initialised!")
    }
}

impl<T> DerefMut for LazyInitialised<T>{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("Value should be initialised!")
    }
}

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


impl<T> Debug for Mutex<T>
where T: Debug{

fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> { 
   f.debug_struct("Mutex").field("lock", &self.lock).field("inner", unsafe{&*self.inner.get()}).finish()
}

}
impl<T> Mutex<T> 
where T: Debug{
    pub fn is_locked(&self) -> bool{
        self.lock.load(core::sync::atomic::Ordering::Relaxed)
    }

    pub const fn from(val: T) -> Self{
        Self{
            inner: UnsafeCell::new(val),
            lock: AtomicBool::new(false)
        }
    }

    pub fn with(&self, f: fn (MutexGuard<T>)){
        f(self.lock());
    }

    pub fn lock(&self) -> MutexGuard<T>{
        let mut deadlock_warning_iter_count = 1_000_000; // FIXME: Arbitrary number
        while self.lock.compare_exchange_weak(false, true, core::sync::atomic::Ordering::Acquire, core::sync::atomic::Ordering::Relaxed).is_err() {
            while self.is_locked(){ 
                core::hint::spin_loop(); 
                deadlock_warning_iter_count -= 1; 
                if deadlock_warning_iter_count == 0{ panic!("Tried one million (1,000,000) times but couldn't lock mutex: {:?} :(, is your system too fast, or too slow?!", self); }
            }
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