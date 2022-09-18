use alloc::vec;
use alloc::{borrow::ToOwned, vec::Vec};
use core::{fmt::Debug, ptr::null};
use packed_struct::EnumCatchAll;
use rlibc::mem::memcpy;

use crate::{
    allocator::{self, BasicAlloc},
    elf::{elf_header, elf_program_header, ElfFile},
    emulator::Riscv64Cpu,
    syscall, vfs,
    virtmem::{LittleEndianVirtualMemory, VirtualMemory},
};

pub struct ProgramFileDescriptor {
    pub vfs_node: vfs::Node,
    pub cursor: u64, /* Note cursor points at the byte to write to next, not before or after it */
    pub flags: usize,
}

impl Debug for ProgramFileDescriptor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProgramFileDescriptor").field("cursor", &self.cursor).field("flags", &self.flags).finish()
    }
}

#[derive(Debug)]
pub struct ProgramData {
    pub open_fds: Vec<Option<ProgramFileDescriptor>>,
    pub heap_virtual_start_addr: u64,
    pub max_virtual_heap_size: u64,
    pub program_alloc: allocator::BasicAlloc,
}

impl ProgramData {
    fn new(heap_virtual_start_addr: u64, max_virtual_heap_size: u64, alloc: allocator::BasicAlloc) -> Self {
        ProgramData { open_fds: Vec::new(), heap_virtual_start_addr, max_virtual_heap_size, program_alloc: alloc }
    }
}

#[derive(Debug)]
pub struct Program {
    pub emu: Riscv64Cpu<LittleEndianVirtualMemory>,
    pub data: ProgramData,
}

impl Program {
    pub fn from_elf(elf_data: &[u8], args: &[&str]) -> Option<Program> {
        let elf = ElfFile::from_bytes(elf_data)?;
        if elf.header.instruction_set != elf_header::InstructionSet::RiscV {
            return None;
        }
        if elf.header.elf_type != elf_header::ElfType::EXECUTABLE {
            return None;
        }

        let mut virt_mem = LittleEndianVirtualMemory::new();

        let mut max_lower_virt_addr = 0;
        // Map elf into virtual memory
        for header in elf.program_headers {
            if header.segment_type == EnumCatchAll::from(elf_program_header::ProgramHeaderType::Load) {
                let mut segment_data = elf_data
                    [header.segment_file_offset as usize..(header.segment_file_offset + header.segment_file_size) as usize]
                    .to_owned();
                segment_data.resize(header.segment_virtual_size as usize, 0);
                if header.segment_virtual_address + segment_data.len() as u64 > max_lower_virt_addr {
                    max_lower_virt_addr = header.segment_virtual_address + segment_data.len() as u64;
                }
                virt_mem.add_region(header.segment_virtual_address, &segment_data)?;
            }
        }

        const PROGRAM_STACK_SIZE: u64 = 8 * 1024;
        // Add 8kb of stack space at the end of the address space
        virt_mem.add_region(
            u64::MAX - (PROGRAM_STACK_SIZE) + 1,     /* the address itself is included in the region */
            &vec![0u8; PROGRAM_STACK_SIZE as usize], // NOTE: We don't use [] because that would allocate 1MB on the stack, then move it to the heap, which might overflow the stack
        )?;

        // Calculate total args size
        let mut total_arg_size = core::mem::size_of::<u64>() * args.len(); // For array

        // For each element in array
        for e in args {
            total_arg_size += e.len() + 1;
        }

        // Add Heap Region
        virt_mem.add_region(
            max_lower_virt_addr,               /* the address itself is included in the region */
            &vec![0u8; total_arg_size + 1024], // Note: Can't be zero-sized otherwise mapping won't work
        )?;

        let heap_region = virt_mem.try_map_mut(max_lower_virt_addr)?.0;

        let mut allocator = BasicAlloc::from(heap_region.backing_storage.as_mut_ptr(), heap_region.len());

        // Note: We load the arguments on the heap not on the stack
        // Allocate space for arguments pointer array
        let args_ptr_array = allocator
            .alloc(core::alloc::Layout::from_size_align(core::mem::size_of::<u64>() * args.len(), 1).ok()?)
            as *mut u64;

        for e in args.iter().enumerate() {
            let index = e.0;
            let arg: &str = e.1;

            // Allocate space for arg
            let arg_ptr = allocator.alloc(core::alloc::Layout::from_size_align(arg.len() + 1, 1).ok()?);
            unsafe { memcpy(arg_ptr, arg.as_ptr(), arg.len()) };
            unsafe { *arg_ptr.add(arg.len()) = 0 }; // c strings are null terminated

            // We still need to "reverse map" the arg pointer
            let virtual_arg_ptr = {
                let offset_in_heap = unsafe { (arg_ptr as *mut u8).sub(heap_region.backing_storage.as_mut_ptr() as usize) };
                heap_region.try_reverse_map(offset_in_heap as usize).unwrap_or(null::<u8>() as u64)
            };
            unsafe { *args_ptr_array.add(index) = virtual_arg_ptr };
        }

        let mut emu = Riscv64Cpu::from(virt_mem, elf.header.program_entry, syscall::syscall_linux_abi_entry_point);
        emu.write_reg(10, args.len() as u64);
        let virtual_args_ptr_array = {
            // We still need to "reverse map" the args ptr array pointer
            let heap_region = emu.memory.try_map_mut(max_lower_virt_addr)?.0;
            let offset_in_heap = unsafe { (args_ptr_array as *mut u8).sub(heap_region.backing_storage.as_mut_ptr() as usize) };
            heap_region.try_reverse_map(offset_in_heap as usize).unwrap_or(null::<u8>() as u64)
        };

        emu.write_reg(11, virtual_args_ptr_array as u64);
        Some(Program {
            emu,
            data: ProgramData::new(max_lower_virt_addr, u64::MAX - (PROGRAM_STACK_SIZE + max_lower_virt_addr), allocator),
        })
    }

    pub fn run(&mut self) {
        while self.emu.tick(&mut self.data).is_some() {}
    }
}
