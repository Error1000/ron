use core::{marker::PhantomData, arch::asm};

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

/// Docs for in: https://www.felixcloutier.com/x86/in
/// Docs for out: https://www.felixcloutier.com/x86/out
/// From docs there are no ways to provide an address with more than 16 btis, so the port address space is 16 bits
/// The form of the instruction which accepts 8 bits can not access the entire port address space:
/// "Using the DX register as a source operand allows I/O port addresses from 0 to 65,535 to be accessed; using a byte immediate allows I/O port addresses 0 to 255 to be accessed."
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

// FIXME: Deal with little-endian vs big-endian
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


impl<A: AddressSpace, T> Pointer<A, T>{
    pub unsafe fn offset(&self, o: isize) -> Self {
        // FIXME: Does offsetting a port "address" work the same way as offestting a real memory address?
        Self {
            inner: self.inner.offset(o),
            space: PhantomData,
            is_port: self.is_port,
        }    
    }
}


impl<A: AddressSpace> Pointer<A, u8> {
    // SAFTEY: Constructors assume address is in correct space
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
    // SAFTEY: Constructors assume address is in correct space
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