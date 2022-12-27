use alloc::string::String;
use alloc::vec;
use alloc::{borrow::ToOwned, vec::Vec, collections::BTreeMap};
use core::{fmt::Debug, ptr::null};
use packed_struct::EnumCatchAll;
use rlibc::mem::memcpy;

use crate::virtmem::VirtRegion;
use crate::{
    allocator::{self, BasicAlloc},
    elf::{elf_header, elf_program_header, ElfFile},
    emulator::Riscv64Cpu,
    syscall, vfs,
    virtmem::{LittleEndianVirtualMemory, VirtualMemory},
};

pub struct ProgramNode {
    pub vfs_node: vfs::Node,
    pub cursor: u64, /* Note cursor points at the byte to write to next, not before or after it */
    pub flags: usize,
    pub path: vfs::Path,
    pub reference_count: usize // Keeps track of the number of references so that we don't close a fd that still has references to it
}

#[derive(Debug, Clone, Copy)]
pub enum FdMapping {
    Index(usize),
    Stdin,
    Stdout,
    Stderr,
}

impl Debug for ProgramNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProgramNode").field("cursor", &self.cursor).field("flags", &self.flags).field("path", &self.path).field("reference_count", &self.reference_count).finish()
    }
}

#[derive(Debug)]
pub struct ProgramData {
    // The Options are used to maintain the indices of the elements
    // In open_nodes this is needed because indices into it are stored in fd_mapping, and i don't want to update
    // fd_mapping when removing a ProgramNode
    // In fd_mapping this is needed because indices into it are fds and the program
    // won't update it's fds just because i remove a fd from the middle
    pub open_nodes: Vec<Option<ProgramNode>>,
    pub fd_mapping: Vec<Option<FdMapping>>, // Maps fds to node indices
    pub cwd: vfs::Path,
    pub env: BTreeMap<String, u64>,
    pub heap_virtual_start_addr: u64,
    pub max_virtual_heap_size: u64,
    pub program_alloc: allocator::BasicAlloc,
}

impl ProgramData {
    fn new(
        heap_virtual_start_addr: u64,
        max_virtual_heap_size: u64,
        alloc: allocator::BasicAlloc,
        cwd: vfs::Path,
        env: BTreeMap<String, u64>,
    ) -> Self {
        ProgramData { open_nodes: Vec::new(), fd_mapping: vec![Some(FdMapping::Stdin), Some(FdMapping::Stdout), Some(FdMapping::Stderr)], cwd, env, heap_virtual_start_addr, max_virtual_heap_size, program_alloc: alloc }
    }
}

#[derive(Debug)]
pub struct Program {
    pub emu: Riscv64Cpu<LittleEndianVirtualMemory>,
    pub data: ProgramData,
}

impl Program {

    fn calculate_initial_heap_size(args: &[&str], env: &BTreeMap<&str, &str>) -> usize {
        // Calculate total args size
        let mut total_size = core::mem::size_of::<u64>() * args.len(); // For array

        // For each element in array
        for e in args {
            total_size += e.len() + 1;
        }

        // For each variable value in the env
        for env_variable_value in env.values(){
            total_size += env_variable_value.len()+1;
        }

        // NOTE: 1024, here, is a magic number with no meaning or testing to ensure it is the optimal value
        return total_size+1024; // Extra room for growth
    }


    fn load_args_into_memory(args: &[&str], allocator: &mut BasicAlloc, heap_region: &mut VirtRegion) -> Option<*mut u64> {
        // Note: We load the arguments on the heap not on the stack
        // Allocate space for arguments pointer array
        let args_ptr_array = allocator
            .alloc(core::alloc::Layout::from_size_align(core::mem::size_of::<u64>() * args.len(), 1).ok()?)
            as *mut u64;

        for (index, arg) in args.iter().enumerate() {
            // Allocate space for arg
            let arg_ptr = allocator.alloc(core::alloc::Layout::from_size_align(arg.len() + 1, 1).ok()?);

            // Copy the argument to user-land
            unsafe { memcpy(arg_ptr as *mut _, arg.as_ptr() as *const _, arg.len()) };
            unsafe { *arg_ptr.add(arg.len()) = b'\0' }; // c strings are null terminated

            // We need to "reverse map" the arg pointer to get the virtual address to place in the array
            let virtual_arg_ptr = {
                let offset_in_heap = unsafe { (arg_ptr as *mut u8).sub(heap_region.backing_storage.as_mut_ptr() as usize) };
                heap_region.try_reverse_map(offset_in_heap as usize).unwrap_or(null::<u8>() as u64)
            };

            // NOTE: count is in units of T!
            unsafe { *args_ptr_array.add(index) = virtual_arg_ptr };
        }

        return Some(args_ptr_array);
    }

    fn load_env_into_memory(env: &BTreeMap<&str, &str>, allocator: &mut BasicAlloc, heap_region: &mut VirtRegion) -> Option<BTreeMap<String, u64>> {
        let mut map = BTreeMap::new();
        for (key, value) in env.iter() {
            // Allocate space for variable value
            let env_ptr = allocator.alloc(core::alloc::Layout::from_size_align(value.len() + 1, 1).ok()?);

            // Copy the environment variable to user-land
            unsafe { memcpy(env_ptr as *mut _, value.as_ptr() as * const _, value.len())};
            unsafe { *env_ptr.add(value.len()) = b'\0'; } // c strings are null terminated

            // We need to "reverse map" the env pointer to get the virtual address to place in the returned map
            let virtual_env_ptr = {
                let offset_in_heap = unsafe { (env_ptr as *mut u8).sub(heap_region.backing_storage.as_mut_ptr() as usize)};
                heap_region.try_reverse_map(offset_in_heap as usize).unwrap_or(null::<u8>() as u64)
            };
            map.insert(String::from(*key), virtual_env_ptr);
        }
        Some(map)
    }


    pub fn from_elf(elf_data: &[u8], args: &[&str], cwd: vfs::Path, env: &BTreeMap<&str, &str>) -> Option<Program> {
        // elf_data contains the bytes of the elf file
        let elf = ElfFile::from_bytes(elf_data)?;

        if elf.header.instruction_set != elf_header::InstructionSet::RiscV {
            return None;
        }

        if elf.header.elf_type != elf_header::ElfType::EXECUTABLE {
            return None;
        }

        let mut virt_mem = LittleEndianVirtualMemory::new();

        let mut lower_virt_addr = 0; // Used to keep track of first virtual address that is free, so we can put the heap there
        // Map elf into virtual memory
        for header in elf.program_headers {
            if header.segment_type == EnumCatchAll::from(elf_program_header::ProgramHeaderType::Load) {
                let mut segment_data = elf_data
                    [header.segment_file_offset as usize..(header.segment_file_offset + header.segment_file_size) as usize]
                    .to_owned();
                segment_data.resize(header.segment_virtual_size as usize, 0);

                if header.segment_virtual_address + segment_data.len() as u64 > lower_virt_addr {
                    lower_virt_addr = header.segment_virtual_address + segment_data.len() as u64;
                }
                
                virt_mem.add_region(header.segment_virtual_address, &segment_data)?;
            }
        }

        const PROGRAM_STACK_SIZE: u64 = 8 * 1024;
        // Add 8kb of stack space at the end of the virtual address space
        virt_mem.add_region(
            u64::MAX - (PROGRAM_STACK_SIZE) + 1,     /* the address itself is included in the region */
            &vec![0u8; PROGRAM_STACK_SIZE as usize], // NOTE: We don't use [] because that would allocate 1MB on the stack, then move it to the heap, which might overflow the stack
        )?;

        // Add Heap Region
        virt_mem.add_region(
            lower_virt_addr,                                /* the address itself is included in the region */
            &vec![0u8; Self::calculate_initial_heap_size(args, env)], // Note: Can't be zero-sized otherwise mapping won't work
        )?;
        
        let heap_region = virt_mem.try_map_mut(lower_virt_addr)?.0;
        
        let mut allocator = BasicAlloc::from(heap_region.backing_storage.as_mut_ptr(), heap_region.len());

        let args_ptr_array = Self::load_args_into_memory(args, &mut allocator, heap_region)?;
        let prog_env = Self::load_env_into_memory(env, &mut allocator, heap_region)?;

        let mut emu = Riscv64Cpu::from(virt_mem, elf.header.program_entry, syscall::syscall_linux_abi_entry_point);
        
        // Setup argc and argv
        emu.write_reg(10, args.len() as u64); // argc

        let virtual_args_ptr_array = {
            // We still need to "reverse map" the args ptr array pointer
            let heap_region = emu.memory.try_map_mut(lower_virt_addr)?.0;
            let offset_in_heap = unsafe { (args_ptr_array as *mut u8).sub(heap_region.backing_storage.as_mut_ptr() as usize) };
            heap_region.try_reverse_map(offset_in_heap as usize).unwrap_or(null::<u8>() as u64)
        };

        emu.write_reg(11, virtual_args_ptr_array as u64); // argv
        

        Some(Program {
            emu,
            data: ProgramData::new(
                lower_virt_addr,
                u64::MAX - (PROGRAM_STACK_SIZE + lower_virt_addr),
                allocator,
                cwd,
                prog_env,
            ),
        })
    }

    pub fn tick(&mut self) -> Option<()> {
        self.emu.tick(&mut self.data)
    }
}
