use alloc::string::String;
use alloc::vec;
use alloc::{ vec::Vec, collections::BTreeMap};
use core::fmt::Debug;
use packed_struct::EnumCatchAll;

use crate::allocator::{ProgramBasicAlloc, BasicAlloc};
use crate::{
    allocator,
    elf::{elf_header, elf_program_header, ElfFile},
    emulator::Riscv64Cpu,
    syscall, vfs,
    virtmem::{LittleEndianVirtualMemory, VirtualMemory},
};

#[derive(Clone)]
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

#[derive(Debug, Clone, Copy)]
pub struct WaitInformation {
    pub cpid: usize,
    pub exit_code: usize
}

#[derive(Debug, Clone)]
pub enum ProgramState {
    RUNNING,
    RUNNING_NEW_CHILD_JUST_FORKED,
    WAITING_FOR_CHILD_PROCESS(Option<usize>),
    FINISHED_WAITING_FOR_CHILD_PROCESS(Option<WaitInformation>), // Used to allow to scheduler to inform the program that the child changed state
    TERMINATED_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT(usize), // equivalent to ZOMBIE
    TERMINATED_WAITING_TO_BE_DEALLOCATED(usize),
}

#[derive(Debug, Clone)]
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
    pub virtual_allocator: BasicAlloc,
    pub state: ProgramState,
    pub pid: Option<usize>, // Programs can technically be run without a set pid
    pub parent_pid: Option<usize>,
    pub exit_code: Option<usize>
}

impl ProgramData {
    fn new(
        cwd: vfs::Path,
        env: BTreeMap<String, u64>,
        virtual_allocator: BasicAlloc
    ) -> Self {
        ProgramData { open_nodes: Vec::new(), fd_mapping: vec![Some(FdMapping::Stdin), Some(FdMapping::Stdout), Some(FdMapping::Stderr)], cwd, env, virtual_allocator, state: ProgramState::RUNNING, pid: None, parent_pid: None, exit_code: None}
    }
}

pub type Emulator = Riscv64Cpu<LittleEndianVirtualMemory<&'static ProgramBasicAlloc>>;


#[derive(Debug)]
pub struct Program {
    pub emu: Emulator,
    pub data: ProgramData,
}

impl Program {
    pub fn new(emu: Emulator, prog_data: ProgramData) -> Program {
        Program { emu: emu, data: prog_data }
    }

    fn load_args_into_memory(args: &[&str], virt_mem: &mut impl VirtualMemory<A = &'static ProgramBasicAlloc>, virtual_allocator: &mut BasicAlloc) -> Option<u64> {
        // Note: We load the arguments on the heap not on the stack
        // Allocate space for arguments pointer array
        let mut args_ptrs = Vec::<u8, &'static allocator::ProgramBasicAlloc>::new_in(&allocator::PROGRAM_ALLOCATOR);
        args_ptrs.clear();
        args_ptrs.resize(args.len()*core::mem::size_of::<u64>(), 0);

        let args_ptrs_array_virtual_ptr = virtual_allocator.alloc(core::alloc::Layout::from_size_align(args_ptrs.len()*core::mem::size_of::<u64>(), 1).ok()?) as u64;
        // It's a virtual pointer to an array of pointers to the arguments
        // A.k.a it's the value of argv
        if args_ptrs_array_virtual_ptr == 0 { return None; }

        for (index, arg) in args.iter().enumerate() {
            // Allocate space for arg and copy it in there

            let mut allocated_arg = Vec::<u8, &'static ProgramBasicAlloc>::new_in(&allocator::PROGRAM_ALLOCATOR);
            allocated_arg.clear();
            allocated_arg.resize(arg.len()+1, 0u8);
            for (indx, c) in arg.chars().enumerate() {
                allocated_arg[indx] = c as u8;
            }

            {
                let end_indx = allocated_arg.len()-1;
                allocated_arg[end_indx] = b'\0';
            }

            let virtual_arg_ptr = virtual_allocator.alloc(core::alloc::Layout::from_size_align(allocated_arg.len()*core::mem::size_of::<u8>(), 1).ok()?) as u64;
            if virtual_arg_ptr == 0 { return None; }

            virt_mem.add_region(virtual_arg_ptr, allocated_arg)?;
            for (byte_index, byte) in virtual_arg_ptr.to_le_bytes().iter().enumerate() {
                args_ptrs[index*core::mem::size_of::<u64>() + byte_index] = *byte;
            }
        }

        virt_mem.add_region(args_ptrs_array_virtual_ptr, args_ptrs)?;

        Some(args_ptrs_array_virtual_ptr)
    }

    fn load_env_into_memory(env: &BTreeMap<&str, &str>, virt_mem: &mut impl VirtualMemory<A = &'static ProgramBasicAlloc>, virtual_allocator: &mut BasicAlloc) -> Option<BTreeMap<String, u64>> {
        let mut map = BTreeMap::new();
        for (key, value) in env.iter() {
            // Allocate space for variable value
            let mut allocated_env_value = Vec::<u8, &'static ProgramBasicAlloc>::new_in(&allocator::PROGRAM_ALLOCATOR);
            allocated_env_value.clear();
            allocated_env_value.resize(value.len()+1, 0u8);

            // Copy the environment variable to user-land
            for (indx, c) in value.chars().enumerate() {
                allocated_env_value[indx] = c as u8;
            }

            {
                let end_indx =  allocated_env_value.len()-1;
                allocated_env_value[end_indx] = b'\0';
            }

            let virtual_env_value_ptr = virtual_allocator.alloc(core::alloc::Layout::from_size_align(allocated_env_value.len()*core::mem::size_of::<u8>(), 1).ok()?) as u64;
            if virtual_env_value_ptr == 0 { return None; }

            virt_mem.add_region(virtual_env_value_ptr, allocated_env_value)?;
            map.insert(String::from(*key), virtual_env_value_ptr);
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

        let mut lower_virt_addr = 0; // Used to keep track of first virtual address that is free, so we can put the virtual allocator(heap) there
        // Map elf into virtual memory
        for header in elf.program_headers {
            if header.segment_type == EnumCatchAll::from(elf_program_header::ProgramHeaderType::Load) {
                let segment = {
                    let segment_data = &elf_data
                        [header.segment_file_offset as usize..(header.segment_file_offset + header.segment_file_size) as usize];
                    
                    let mut segment = Vec::new_in(&allocator::PROGRAM_ALLOCATOR);
                    segment.clear();
                    segment.extend(segment_data);
                    // Some segments have a bigger virtual size than physical, however, for simplicity, our system requires that virtual an physical segments be the same size 
                    // Having different sizes would require 2 different size variables in the VirtRegion struct. 
                    segment.resize(header.segment_virtual_size as usize, 0); 
                    segment
                };

                if header.segment_virtual_address + segment.len() as u64 > lower_virt_addr {
                    lower_virt_addr = header.segment_virtual_address + segment.len() as u64;
                }
                
                virt_mem.add_region(header.segment_virtual_address, segment)?;
            }
        }

        const PROGRAM_STACK_SIZE: u64 = 8 * 1024;
        let mut program_stack = Vec::new_in(&allocator::PROGRAM_ALLOCATOR);
        program_stack.clear();
        program_stack.resize(PROGRAM_STACK_SIZE as usize, 0u8);

        // Add 8kb of stack space at the end of the virtual address space
        virt_mem.add_region(
            u64::MAX - (PROGRAM_STACK_SIZE) + 1,     /* +1 because the address itself is included in the region */
            program_stack, // NOTE: We don't use [] because that would allocate 1MB on the stack, then move it to the heap, which might overflow the stack
        )?;


        // Create virtual allocator for the heap, this manages the locations of allocations on the heap in the virtual space
        // Or just generally the location of segments in virtual space, this can't be done for some segments like the elf regions and the stack
        // however elf regions and the stack are currently the only ones where that is a problem so we just do those and then we 
        // mark the virtual address at the end of the elf regions and the begging of the stack and use the virtual space in-between for
        // all other regions that don't need a specific virtual location

        // NOTE: this allocator does not have a real pointer, a.k.a the allocator must never dereference any pointers
        // This means that for the current allocator ( BasicAlloc ) we can't use realloc
        let mut virtual_allocator = BasicAlloc::from(lower_virt_addr as *mut u8, (u64::MAX - (PROGRAM_STACK_SIZE + lower_virt_addr)) as usize, true);


        let args_ptrs_array_virtual_ptr = Self::load_args_into_memory(args, &mut virt_mem, &mut virtual_allocator)?;
        let prog_env = Self::load_env_into_memory(env, &mut virt_mem, &mut virtual_allocator)?;

        let mut emu = Riscv64Cpu::from(virt_mem, elf.header.program_entry, syscall::syscall_linux_abi_entry_point);
        
        // Setup argc and argv
        emu.write_reg(10, args.len() as u64); // argc
        emu.write_reg(11, args_ptrs_array_virtual_ptr as u64); // argv
        

        Some(Program {
            emu,
            data: ProgramData::new(
                cwd,
                prog_env,
                virtual_allocator
            ),
        })
    }

    pub fn tick(&mut self) -> Option<()> {
        self.emu.tick(&mut self.data)
    }
}
