use alloc::borrow::ToOwned;
use packed_struct::EnumCatchAll;

use crate::{
    elf::{elf_header, elf_program_header, ElfFile},
    emulator::Riscv64Cpu,
    syscall,
    virtmem::{LittleEndianVirtualMemory, VirtualMemory},
};

#[derive(Debug)]
pub struct Program {
    emu: Riscv64Cpu<LittleEndianVirtualMemory>,
}

impl Program {
    pub fn from_elf(elf_data: &[u8]) -> Option<Program> {
        let elf = ElfFile::from_bytes(elf_data)?;
        if elf.header.instruction_set != elf_header::InstructionSet::RiscV {
            return None;
        }
        if elf.header.elf_type != elf_header::ElfType::EXECUTABLE {
            return None;
        }

        let mut virt_mem = LittleEndianVirtualMemory::new();

        // Map elf into virtual memory
        let mut cur_phys = 0;
        for header in elf.program_headers {
            if header.segment_type == EnumCatchAll::from(elf_program_header::ProgramHeaderType::Load) {
                let mut segment_data = elf_data
                    [header.segment_file_offset as usize..(header.segment_file_offset + header.segment_file_size) as usize]
                    .to_owned();
                segment_data.resize(header.segment_virtual_size as usize, 0);
                virt_mem.add_region(cur_phys, header.segment_virtual_address as usize, &segment_data)?;
                cur_phys += segment_data.len();
            }
        }

        // Add 4kb stack at the end of the address space
        virt_mem.add_region(
            cur_phys,
            usize::MAX - 4096 + 1, /* the address itself is included in the region */
            &[0; 4096],
        )?;

        let emu = Riscv64Cpu::from(virt_mem, elf.header.program_entry, syscall::syscall_linux_abi_entry_point);
        Some(Program { emu })
    }

    pub fn run(&mut self) {
        while self.emu.tick().is_some() {}
    }
}
