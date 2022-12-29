use alloc::vec::Vec;
use core::{arch::asm, marker::PhantomData, alloc::Allocator, convert::TryInto};

use crate::emulator::EmulatorMemory;

#[derive(Debug, Clone)]

pub struct VirtRegion<A: Allocator> {
    virtual_start: u64, // Note: virtual_start "points" to the beginning of the region, not one after or one before
    pub backing_storage: Vec<u8, A>,
}

impl<A: Allocator> VirtRegion<A> {
    // Returns: Offset into region memory
    pub fn try_map(&self, virt_addr: u64) -> Option<usize> {
        if (self.virtual_start..=self.get_virtual_end_inclusive()).contains(&virt_addr) {
            return Some((virt_addr - self.virtual_start) as usize);
        } else {
            return None;
        }
    }

    // Returns: Virtual address
    pub fn try_reverse_map(&self, offset_in_region: usize) -> Option<u64> {
        if offset_in_region > self.len() {
            return None;
        } else {
            return Some(offset_in_region as u64 + self.virtual_start);
        }
    }

    pub fn overlaps_virtually_with(&self, other: &Self) -> bool {
        // If our start is after their end, then we don't overlap
        if self.virtual_start > other.get_virtual_end_inclusive() {
            return false;
        }

        // If our end is before their start, then we don't overlap
        if self.get_virtual_end_inclusive() < other.virtual_start {
            return false;
        }

        // Otherwise return true
        return true;
    }

    pub fn get_virtual_end_inclusive(&self) -> u64 {
        // If len is 1 then the end is the start, so in other words we need to offset by -1
        self.virtual_start - 1 + (self.len() as u64)
    }

    pub fn len(&self) -> usize {
        self.backing_storage.len()
    }
}


pub struct MappingInfo {
    pub offset_in_region: usize,
    pub region_index: usize
}

pub trait VirtualMemory {
    type A: Allocator;
    fn try_map(&self, virt_addr: u64) -> Option<(&VirtRegion<Self::A>, MappingInfo)>;
    fn try_map_mut(&mut self, virt_addr: u64) -> Option<(&mut VirtRegion<Self::A>, MappingInfo)>;

    fn add_region(&mut self, virt_addr: u64, data: Vec<u8, Self::A>) -> Option<()>;
    fn remove_region(&mut self, region_index: usize);
}

#[derive(Debug, Clone)]
pub struct LittleEndianVirtualMemory<A: Allocator> {
    map: Vec<VirtRegion<A>>,
}

impl<A: Allocator> LittleEndianVirtualMemory<A> {
    pub fn new() -> Self {
        Self { map: Vec::new() }
    }

    pub fn get_number_of_regions(&self) -> usize {
        self.map.len()
    }
}

impl<A: Allocator> VirtualMemory for LittleEndianVirtualMemory<A> {
    type A = A;

    // Returns: A tuple containing a mutable reference to the region and the offset into it at which you will find the data at the virtual address
    fn try_map_mut(&mut self, virt_addr: u64) -> Option<(&mut VirtRegion<A>, MappingInfo)> {
        self.map.iter_mut().enumerate()
        .find_map(|(index, range)| 
                  range.try_map(virt_addr).map(|offset| (range, MappingInfo{offset_in_region: offset, region_index: index}))
                 )
    }

    // Returns: A tuple containing a reference to the region and the offset into it at which you will find the data at the virtual address
    fn try_map(&self, virt_addr: u64) -> Option<(&VirtRegion<A>, MappingInfo)> {
        self.map.iter().enumerate()
        .find_map(|(index, range)|
                  range.try_map(virt_addr).map(|offset| (range, MappingInfo{offset_in_region: offset, region_index: index}))
                 )
    }

    fn add_region(&mut self, virt_addr: u64, data: Vec<u8, A>) -> Option<()> {
        let new_range = VirtRegion { virtual_start: virt_addr, backing_storage: data };

        // First check for overlap in the virtual space
        // (Overlap in the physical space is fine, as long as one virtual address only maps to one physical address it's fine)

        if self.map.iter().any(|range| range.overlaps_virtually_with(&new_range)) {
            return None;
        }

        // Then add the range to the map
        self.map.push(new_range);

        Some(())
    }

    fn remove_region(&mut self, region_index: usize) {
        self.map.remove(region_index);
    }
}

impl<T> EmulatorMemory for T
where
    T: VirtualMemory,
{
    fn read_u8_ne(&self, addr: u64) -> u8 {
        let region =
            if let Some(val) = self.try_map(addr) { val } else { panic!("Virtual address: {} should be mapped!", addr) };
        region.0.backing_storage[region.1.offset_in_region]
    }

    fn write_u8_ne(&mut self, addr: u64, val: u8) {
        let region =
            if let Some(val) = self.try_map_mut(addr) { val } else { panic!("Virtual address: {} should be mapped!", addr) };
        region.0.backing_storage[region.1.offset_in_region] = val;
    }

    // FIXME: Reading/writing more than 1 byte across a region boundary is not supported
    fn read_u16_ne(&self, addr: u64) -> u16 {
        let region =
            if let Some(val) = self.try_map(addr) { val } else { panic!("Virtual address: {} should be mapped!", addr) };
        u16::from_le_bytes(region.0.backing_storage[region.1.offset_in_region..region.1.offset_in_region + core::mem::size_of::<u16>()].try_into().unwrap())
    }

    fn write_u16_ne(&mut self, addr: u64, val: u16) {
        let region =
            if let Some(val) = self.try_map_mut(addr) { val } else { panic!("Virtual address: {} should be mapped!", addr) };
        let mut indx = 0;
        for byte in val.to_le_bytes() {
            region.0.backing_storage[region.1.offset_in_region + indx] = byte;
            indx += 1;
        }
    }

    fn read_u32_ne(&self, addr: u64) -> u32 {
        let region =
            if let Some(val) = self.try_map(addr) { val } else { panic!("Virtual address: {} should be mapped!", addr) };
        u32::from_le_bytes(region.0.backing_storage[region.1.offset_in_region..region.1.offset_in_region + core::mem::size_of::<u32>()].try_into().unwrap())
    }

    fn write_u32_ne(&mut self, addr: u64, val: u32) {
        let region =
            if let Some(val) = self.try_map_mut(addr) { val } else { panic!("Virtual address: {} should be mapped!", addr) };
        let mut indx = 0;
        for byte in val.to_le_bytes() {
            region.0.backing_storage[region.1.offset_in_region + indx] = byte;
            indx += 1;
        }
    }

    fn read_u64_ne(&self, addr: u64) -> u64 {
        let region =
            if let Some(val) = self.try_map(addr) { val } else { panic!("Virtual address: {} should be mapped!", addr) };
        u64::from_le_bytes(region.0.backing_storage[region.1.offset_in_region..region.1.offset_in_region + core::mem::size_of::<u64>()].try_into().unwrap())
    }

    fn write_u64_ne(&mut self, addr: u64, val: u64) {
        let region =
            if let Some(val) = self.try_map_mut(addr) { val } else { panic!("Virtual address: {} should be mapped!", addr) };
        let mut indx = 0;
        for byte in val.to_le_bytes() {
            region.0.backing_storage[region.1.offset_in_region + indx] = byte;
            indx += 1;
        }
    }

    fn read_u32_le(&self, addr: u64) -> u32 {
        let region =
            if let Some(val) = self.try_map(addr) { val } else { panic!("Virtual address: {} should be mapped!", addr) };
        u32::from_le_bytes(region.0.backing_storage[region.1.offset_in_region..region.1.offset_in_region + core::mem::size_of::<u32>()].try_into().unwrap())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct KernPointer<T>
where
    T: ?Sized,
{
    inner: *mut T,
    is_port: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct UserPointer<T>
where
    T: ?Sized,
{
    inner: u64,
    phantom_hold: PhantomData<*mut T>,
}

/// Docs for in: https://www.felixcloutier.com/x86/in
/// Docs for out: https://www.felixcloutier.com/x86/out
/// From docs there are no ways to provide an address with more than 16 bits, so the port address space is 16 bits
/// The form of the instruction which accepts 8 bits can not access the entire port address space:
/// "Using the DX register as a source operand allows I/O port addresses from 0 to 65,535 to be accessed; using a byte immediate allows I/O port addresses 0 to 255 to be accessed."
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(always)]
unsafe fn port_outb(addr: u16, val: u8) {
    asm!("out dx, al", in("al") val, in("dx") addr, options(nostack, nomem));
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(always)]
unsafe fn port_inb(addr: u16) -> u8 {
    let mut res: u8;
    asm!("in al, dx", out("al") res, in("dx") addr, options(nostack, nomem));
    return res;
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(always)]
unsafe fn port_outh(addr: u16, val: u16) {
    asm!("out dx, ax", in("ax") val, in("dx") addr, options(nostack, nomem));
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(always)]
unsafe fn port_inh(addr: u16) -> u16 {
    let mut res: u16;
    asm!("in ax, dx", out("ax") res, in("dx") addr, options(nostack, nomem));
    return res;
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline(always)]
unsafe fn port_outb(addr: u16, val: u8) {
    unimplemented!("The port_outb function is either not avilable on your architecture or your architecture is not supported.");
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline(always)]
unsafe fn port_inb(addr: u16) -> u8 {
    unimplemented!("The port_inb function is either not avilable on your architecture or your architecture is not supported.");
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline(always)]
unsafe fn port_outh(addr: u16, val: u16) {
    unimplemented!("The port_outh function is either not avilable on your architecture or your architecture is not supported.");
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline(always)]
unsafe fn port_inh(addr: u16) -> u16 {
    unimplemented!("The port_inh function is either not avilable on your architecture or your architecture is not supported.");
}

impl<T> KernPointer<T>
where
    T: Sized,
{
    pub unsafe fn offset(&self, o: isize) -> Self {
        // FIXME: Does offsetting a port "address" work the same way as offestting a real memory address?
        Self { inner: self.inner.offset(o), is_port: self.is_port }
    }
}

impl<T> UserPointer<T>
where
    T: ?Sized,
{
    pub fn get_inner(&self) -> u64 {
        self.inner as u64
    }
}

impl KernPointer<u8> {
    // SAFETY: Constructors assume address is in correct space
    pub unsafe fn from_mem(addr: *mut u8) -> Self {
        Self { inner: addr, is_port: false }
    }
    pub unsafe fn from_port(port: u16) -> Self {
        Self { inner: port as *mut u8, is_port: true }
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

impl KernPointer<u16> {
    // SAFTEY: Constructors assume address is in correct space
    pub unsafe fn from_mem(addr: *mut u16) -> Self {
        Self { inner: addr, is_port: false }
    }

    pub unsafe fn from_port(port: u16) -> Self {
        Self { inner: port as *mut u16, is_port: true }
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

impl UserPointer<u8> {
    // SAFTEY: Constructors assume address is in correct space
    pub unsafe fn from_mem(addr: u64) -> Self {
        Self { inner: addr, phantom_hold: PhantomData }
    }

    pub fn try_as_ptr<'mem>(&self, virtual_memory: &'mem mut impl VirtualMemory) -> Option<*mut u8> {
        let region = virtual_memory.try_map_mut(self.inner)?;
        Some(unsafe { region.0.backing_storage.as_mut_ptr().add(region.1.offset_in_region) })
    }

    pub fn try_as_ref<'mem>(&self, virtual_memory: &'mem mut impl VirtualMemory) -> Option<&'mem mut u8> {
        let region = virtual_memory.try_map_mut(self.inner)?;
        region.0.backing_storage.get_mut(region.1.offset_in_region)
    }
}

impl UserPointer<usize> {
    // SAFTEY: Constructors assume address is in correct space
    pub unsafe fn from_mem(addr: u64) -> Self {
        Self { inner: addr, phantom_hold: PhantomData }
    }

    pub fn try_as_ptr<'mem>(&self, virtual_memory: &'mem mut impl VirtualMemory) -> Option<*mut usize> {
        let region = virtual_memory.try_map_mut(self.inner)?;
        Some(unsafe { region.0.backing_storage.as_mut_ptr().add(region.1.offset_in_region)  as *mut usize})
    }

    pub fn try_as_ref<'mem>(&self, virtual_memory: &'mem mut impl VirtualMemory) -> Option<usize> {
        let region = virtual_memory.try_map_mut(self.inner)?;
        Some(usize::from_le_bytes(region.0.backing_storage.get_mut(region.1.offset_in_region..region.1.offset_in_region+core::mem::size_of::<usize>())?.try_into().unwrap()))
    }
}

impl UserPointer<[u8]> {
    // SAFETY: Constructors assume address is in correct space
    pub unsafe fn from_mem(addr: u64) -> Self {
        Self { inner: addr, phantom_hold: PhantomData }
    }

    pub fn try_as_ptr<'mem>(&self, virtual_memory: &'mem mut impl VirtualMemory) -> Option<*mut u8> {
        let region = virtual_memory.try_map_mut(self.inner)?;
        Some(unsafe { region.0.backing_storage.as_mut_ptr().add(region.1.offset_in_region * core::mem::size_of::<u8>()) })
    }

    pub fn try_as_ref<'mem>(&self, virtual_memory: &'mem mut impl VirtualMemory, count: usize) -> Option<&'mem mut [u8]> {
        let region = virtual_memory.try_map_mut(self.inner)?;
        Some(&mut region.0.backing_storage[region.1.offset_in_region..region.1.offset_in_region + count])
    }
}
