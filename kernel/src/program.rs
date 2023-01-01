use alloc::borrow::ToOwned;
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

    // Returns: The value of argv for the program ( a virtual pointer to the first of the virtual pointers that point to the arguments loaded in virtual memory as c-strings )
    pub fn load_args_into_virtual_memory<'arg>(args: impl Iterator<Item = &'arg str>, args_len: usize, virt_mem: &mut impl VirtualMemory<A = &'static ProgramBasicAlloc>, virtual_allocator: &mut BasicAlloc) -> Option<u64> {
        // Note: We load the arguments on the heap not on the stack
        // Allocate space for arguments pointer array
        let mut argv = Vec::<u8, &'static allocator::ProgramBasicAlloc>::new_in(&allocator::PROGRAM_ALLOCATOR);
        argv.clear();
        argv.resize(args_len*core::mem::size_of::<u64>(), 0);

        let argv_virtual_ptr = virtual_allocator.alloc(core::alloc::Layout::from_size_align(argv.len()*core::mem::size_of::<u64>(), 1).ok()?) as u64;
        // It's a virtual pointer to an array of pointers to the arguments
        // A.k.a it's the value of &argv, which is what the program will get
        if argv_virtual_ptr == 0 { return None; }

        for (index, arg) in args.enumerate() {
            // Allocate space for the argument and copy it in there

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
                argv[index*core::mem::size_of::<u64>() + byte_index] = *byte;
            }
        }

        virt_mem.add_region(argv_virtual_ptr, argv)?;

        Some(argv_virtual_ptr)
    }

    // Returns: A map from the keys(variable names) to a virtual pointer where the value of that variable has been loaded as a c-string
    pub fn load_env_into_virtual_memory<'a>(env: impl Iterator<Item=(&'a str, &'a str)>, virt_mem: &mut impl VirtualMemory<A = &'static ProgramBasicAlloc>, virtual_allocator: &mut BasicAlloc) -> Option<BTreeMap<String, u64>> {
        let mut map = BTreeMap::new();
        for (key, value) in env {
            // Allocate space for variable value
            let mut allocated_env_value = Vec::<u8, &'static ProgramBasicAlloc>::new_in(&allocator::PROGRAM_ALLOCATOR);
            allocated_env_value.clear();
            allocated_env_value.resize(value.len()+1, 0u8);

            // Copy the environment variable value to user-land
            for (indx, c) in value.chars().enumerate() {
                allocated_env_value[indx] = c as u8;
            }
            { // Place a null char at the end because it's a c-string
                let end_indx =  allocated_env_value.len()-1;
                allocated_env_value[end_indx] = b'\0';
            }

            let virtual_ptr_to_env_value = virtual_allocator.alloc(core::alloc::Layout::from_size_align(allocated_env_value.len()*core::mem::size_of::<u8>(), 1).ok()?) as u64;
            if virtual_ptr_to_env_value == 0 { return None; } // If the virtual allocator couldn't find space in the virtual space ( this is unlikely since at the time of writing this comment the virtual space has addresses 64-bit wide), then fail

            virt_mem.add_region(virtual_ptr_to_env_value, allocated_env_value)?;
            map.insert(key.to_owned(), virtual_ptr_to_env_value);
        }
        Some(map)
    }

    // Returns: lowest virt addr that is after all segments loaded, a.k.a the address at the end of the convex hull
    pub fn map_elf_into_virtual_memory(elf: &ElfFile, elf_bytes: &[u8], virt_mem: &mut impl VirtualMemory<A = &'static ProgramBasicAlloc>) -> Option<u64> {
        let mut lower_virt_addr = 0; // Used to keep track of first virtual address that is free, so we can put the virtual allocator(heap) there
        
        // Map elf into virtual memory
        for header in elf.program_headers.iter() {
            if header.segment_type == EnumCatchAll::from(elf_program_header::ProgramHeaderType::Interp) {
                // FIXME: Support elf interpreters
                return None;
            }
            
            if header.segment_type == EnumCatchAll::from(elf_program_header::ProgramHeaderType::Load) {
                // Read segment data into a vector allocated using the program allocator
                let segment = {
                    let segment_data = &elf_bytes[header.segment_file_offset as usize..(header.segment_file_offset + header.segment_file_size) as usize];
                    
                    let mut segment = Vec::new_in(&allocator::PROGRAM_ALLOCATOR);
                    segment.clear();
                    segment.extend(segment_data);
                    // Some segments have a bigger virtual size than physical, however, for simplicity, our system requires that virtual and physical segments be the same size 
                    // So we resize it to it's virtual size.
                    // For example: the .bss segment
                    segment.resize(header.segment_virtual_size as usize, 0); 
                    segment
                };

                // Keep track of the end
                if header.segment_virtual_address + segment.len() as u64 > lower_virt_addr {
                    lower_virt_addr = header.segment_virtual_address + segment.len() as u64;
                }
                
                virt_mem.add_region(header.segment_virtual_address, segment)?;
            }
        }

        Some(lower_virt_addr)
    }


    pub fn from_elf(elf_bytes: &[u8], args: &[&str], cwd: vfs::Path, env: &BTreeMap<&str, &str>) -> Option<Program> {
        let elf = ElfFile::from_bytes(elf_bytes)?;

        if elf.header.instruction_set != elf_header::InstructionSet::RiscV {
            return None;
        }

        if elf.header.elf_type != elf_header::ElfType::EXECUTABLE {
            return None;
        }

        let mut virt_mem = LittleEndianVirtualMemory::new();

        let lower_virt_addr = Self::map_elf_into_virtual_memory(&elf, &elf_bytes, &mut virt_mem)?; // Used to keep track of first virtual address that is free, so we can put the virtual allocator(heap) there
       
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
        // as they require addresses however elf regions and the stack are currently the only ones where that is a problem so we just do those and then we 
        // mark the virtual address at the end of the elf regions and the begging of the stack and use the virtual space in-between for
        // all other regions that don't need a specific virtual location

        let mut virtual_allocator = BasicAlloc::from(lower_virt_addr as *mut u8, (u64::MAX - (PROGRAM_STACK_SIZE + lower_virt_addr)) as usize, true);


        let argv_virtual_ptr = Self::load_args_into_virtual_memory(args.iter().map(|arg|*arg), args.len(), &mut virt_mem, &mut virtual_allocator)?;
        let prog_env = Self::load_env_into_virtual_memory(env.iter().map(|(key, value)|(*key, *value)), &mut virt_mem, &mut virtual_allocator)?;

        let mut emu = Riscv64Cpu::from(virt_mem, elf.header.program_entry, syscall::syscall_linux_abi_entry_point);
        
        // Setup argc and argv
        emu.write_reg(10, args.len() as u64); // argc
        emu.write_reg(11, argv_virtual_ptr as u64); // argv is of type char**, so it's a double pointer

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
