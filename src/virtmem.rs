use core::{marker::PhantomData, arch::asm, convert::TryInto};
use alloc::vec::Vec;

use crate::emulator::EmulatorMemory;


#[derive(Debug)]

struct VirtRange {
    virtual_start: usize,
    phys_start: usize,
    len: usize
}

impl VirtRange {
    fn try_map(&self, virt_addr: usize) -> Option<usize> {
        if (self.virtual_start..self.virtual_start+self.len).contains(&virt_addr){
            return Some(virt_addr-self.virtual_start+self.phys_start);
        }else{
            return None;
        }
    }

    fn virtually_overlaps_with(&self, other: &VirtRange) -> bool {
        // If our start is after their end, then we don't overlap
        if self.virtual_start >= other.virtual_start+other.len { return false; }

        // Our if our end is before their start, then we don't overlap
        if self.virtual_start+self.len <= other.virtual_start { return false; }

        // Otherwise return true
        return true;
    }
}

#[derive(Debug)]
pub struct LittleEndianVirtualMemory {
    backing_storage: Vec<u8>,
    map: Vec<VirtRange>
}

impl LittleEndianVirtualMemory {

    pub fn new() -> Self {
        Self {
            backing_storage: Vec::new(),
            map: Vec::new()
        }
    }
    fn try_map(&self, virt_addr: usize) -> Option<usize> {
        self.map.iter().find_map(|range| range.try_map(virt_addr))
    }

    pub fn add_region(&mut self, phys_addr: usize, virt_addr: usize, data: &[u8]) -> Option<()> {
        // First check for overlap in the virtual space
        // (Overlap in the physical space is fine, as lone as one virtual address only maps to one physical address it's fine)

        let new_range = VirtRange{
            virtual_start: virt_addr,
            phys_start: phys_addr,
            len: data.len()
        };

        if self.map.iter().any(|range|range.virtually_overlaps_with(&new_range)) {
            return None;
        }
        
        // Then add the data to the backing storage
        
        // Make sure we have enough space
        if phys_addr+data.len() > self.backing_storage.len(){
            self.backing_storage.resize(phys_addr+data.len(), 0);
        }

        // Copy the data
        let mut indx = phys_addr;
        for byte in data {
            self.backing_storage[indx] = *byte;
            indx += 1;
        }


        // Finally add the range to the map
        self.map.push(new_range);
     
        Some(())
    }
}

impl EmulatorMemory for LittleEndianVirtualMemory {
    fn read_u8_ne(&self, addr: u64) -> u8 {
        let phys_addr = if let Some(val) = self.try_map(addr as usize) { val } else {panic!("Virtual address: {} should be mapped!", addr)};
        self.backing_storage[phys_addr]
    }

    fn write_u8_ne(&mut self, addr: u64, val: u8) {
        let phys_addr = if let Some(val) = self.try_map(addr as usize) { val } else {panic!("Virtual address: {} should be mapped!", addr)};
        self.backing_storage[phys_addr] = val;
    }

    fn read_u16_ne(&self, addr: u64) -> u16 {
        let phys_addr = if let Some(val) = self.try_map(addr as usize) { val } else {panic!("Virtual address: {} should be mapped!", addr)};
        u16::from_le_bytes(self.backing_storage[phys_addr..phys_addr+core::mem::size_of::<u16>()].try_into().unwrap())
    }

    fn write_u16_ne(&mut self, addr: u64, val: u16) {
        let phys_addr = if let Some(val) = self.try_map(addr as usize) { val } else {panic!("Virtual address: {} should be mapped!", addr)};
        let mut indx = 0;
        for byte in val.to_le_bytes(){
            self.backing_storage[phys_addr+indx] = byte;
            indx += 1;
        }
    }

    fn read_u32_ne(&self, addr: u64) -> u32 {
        let phys_addr = if let Some(val) = self.try_map(addr as usize) { val } else {panic!("Virtual address: {} should be mapped!", addr)};
        u32::from_le_bytes(self.backing_storage[phys_addr..phys_addr+core::mem::size_of::<u32>()].try_into().unwrap())
    }

    fn write_u32_ne(&mut self, addr: u64, val: u32) {
        let phys_addr = if let Some(val) = self.try_map(addr as usize) { val } else {panic!("Virtual address: {} should be mapped!", addr)};
        let mut indx = 0;
        for byte in val.to_le_bytes(){
            self.backing_storage[phys_addr+indx] = byte;
            indx += 1;
        }
    }

    fn read_u64_ne(&self, addr: u64) -> u64 {
        let phys_addr = if let Some(val) = self.try_map(addr as usize) { val } else {panic!("Virtual address: {} should be mapped!", addr)};
        u64::from_le_bytes(self.backing_storage[phys_addr..phys_addr+core::mem::size_of::<u64>()].try_into().unwrap())
    }

    fn write_u64_ne(&mut self, addr: u64, val: u64) {
        let phys_addr = if let Some(val) = self.try_map(addr as usize) { val } else {panic!("Virtual address: {} should be mapped!", addr)};
        let mut indx = 0;
        for byte in val.to_le_bytes(){
            self.backing_storage[phys_addr+indx] = byte;
            indx += 1;
        }
    }

    fn read_u32_le(&self, addr: u64) -> u32 {
        let phys_addr = if let Some(val) = self.try_map(addr as usize) { val } else {panic!("Virtual address: {} should be mapped!", addr)};
        u32::from_le_bytes(self.backing_storage[phys_addr..phys_addr+core::mem::size_of::<u32>()].try_into().unwrap())
    }
}

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
    address_space: PhantomData<A>,
}

pub type KernPointer<T> = Pointer<KernelSpace, T>;

/// Docs for in: https://www.felixcloutier.com/x86/in
/// Docs for out: https://www.felixcloutier.com/x86/out
/// From docs there are no ways to provide an address with more than 16 bits, so the port address space is 16 bits
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
            address_space: PhantomData,
            is_port: self.is_port,
        }    
    }
}


impl Pointer<KernelSpace, u8> {
    // SAFTEY: Constructors assume address is in correct space
    pub unsafe fn from_mem(a: *mut u8) -> Self {
        Self {
            inner: a,
            address_space: PhantomData,
            is_port: false,
        }
    }
    pub unsafe fn from_port(p: u16) -> Self {
        Self {
            inner: p as *mut u8,
            address_space: PhantomData,
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

impl Pointer<KernelSpace, u16> {
    // SAFTEY: Constructors assume address is in correct space
    pub unsafe fn from_mem(a: *mut u16) -> Self {
        Self {
            inner: a,
            address_space: PhantomData,
            is_port: false,
        }
    }
    pub unsafe fn from_port(p: u16) -> Self {
        Self {
            inner: p as *mut u16,
            address_space: PhantomData,
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