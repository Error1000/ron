use core::{marker::PhantomData, ops::Add};

pub trait AddressSpace {}

#[derive(Clone, Copy, Debug)]
pub struct KernelSpace {}
impl AddressSpace for KernelSpace {}

#[derive(Clone, Copy, Debug)]
pub struct Pointer<A, T>
where
    A: AddressSpace,
{
    inner: *mut T,
    is_port: bool,
    space: PhantomData<A>,
}
pub type KernPointer<T> = Pointer<KernelSpace, T>;

#[inline(always)]
unsafe fn port_outb(addr: u16, val: u8) {
    asm!("out dx, al", in("al") val, in("dx") addr, options(nostack, nomem));
}

#[inline(always)]
unsafe fn port_inb(addr: u16) -> u8 {
    let mut res: u8;
    asm!("in al, dx", out("al") res, in("dx") addr, options(nostack, nomem));
    return res;
}

#[inline(always)]
unsafe fn port_outh(addr: u16, val: u16) {
    asm!("out dx, ax", in("ax") val, in("dx") addr, options(nostack, nomem));
}

#[inline(always)]
unsafe fn port_inh(addr: u16) -> u16 {
    let mut res: u16;
    asm!("in ax, dx", out("ax") res, in("dx") addr, options(nostack, nomem));
    return res;
}

impl<A: AddressSpace> Pointer<A, u8> {
    // SAFTEY: Constructors assume address is in kernel space
    // if it was actually a user-space address this can be a problem
    pub unsafe fn from_mem(a: *mut u8) -> Self {
        Self {
            inner: a,
            space: PhantomData,
            is_port: false,
        }
    }
    pub unsafe fn from_port(p: u16) -> Self {
        Self {
            inner: p as *mut u8,
            space: PhantomData,
            is_port: true,
        }
    }

    pub unsafe fn offset(&self, o: isize) -> Self {
        Self {
            inner: self.inner.offset(o),
            space: PhantomData,
            is_port: self.is_port,
        }
    }

    #[inline(always)]
    pub unsafe fn write(&mut self, val: u8) {
        if self.is_port {
            // How to break all rust rules in one easy step
            port_outb(self.inner as u16, val);
        } else {
            core::ptr::write_volatile(self.inner, val);
        }
    }

    #[inline(always)]
    pub unsafe fn read(&self) -> u8 {
        if self.is_port {
            port_inb(self.inner as u16)
        } else {
            *self.inner
        }
    }
}

impl<A: AddressSpace> Pointer<A, u16> {
    // SAFTEY: Constructors assume address is in kernel space
    // if it was actually a user-space address this can be a problem
    pub unsafe fn from_mem(a: *mut u16) -> Self {
        Self {
            inner: a,
            space: PhantomData,
            is_port: false,
        }
    }
    pub unsafe fn from_port(p: u16) -> Self {
        Self {
            inner: p as *mut u16,
            space: PhantomData,
            is_port: true,
        }
    }

    pub unsafe fn offset(&self, o: isize) -> Self {
        Self {
            inner: self.inner.offset(o),
            space: PhantomData,
            is_port: self.is_port,
        }
    }

    #[inline(always)]
    pub unsafe fn write(&mut self, val: u16) {
        if self.is_port {
            // How to break all rust rules in one easy step
            port_outh(self.inner as u16, val);
        } else {
            core::ptr::write_volatile(self.inner, val);
        }
    }

    #[inline(always)]
    pub unsafe fn read(&self) -> u16 {
        if self.is_port {
            port_inh(self.inner as u16)
        } else {
            *self.inner
        }
    }
}
