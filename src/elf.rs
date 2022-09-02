use alloc::vec::Vec;
use packed_struct::prelude::*;

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
enum Endianess {
    LITTLE = 1,
    BIG = 2,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
enum ArchWidth {
    Width32Bit = 1,
    Width64Bit = 2,
}

#[derive(PackedStruct)]
struct ElfIdentification {
    magic: [u8; 4],
    #[packed_field(size_bytes = "1", ty = "enum")]
    arch_width: ArchWidth,
    #[packed_field(size_bytes = "1", ty = "enum")]
    endianess: Endianess,
    elf_version: u8
}

// TODO: Use a macro to avoid the copy-pasta
pub mod elf_header {
    use super::*;

    #[derive(PrimitiveEnum_u16, Clone, Copy, PartialEq, Debug)]
    pub enum ElfType {
        RELOCATABLE = 1,
        EXECUTABLE = 2,
        SHARED = 3,
        CORE = 4,
    }

    #[derive(PrimitiveEnum_u16, Clone, Copy, PartialEq, Debug)]
    pub enum InstructionSet {
        SPARC = 2,
        X86 = 3,
        MIPS = 8,
        PowerPC = 0x14,
        ARM = 0x28,
        SuperH = 0x2A,
        IA64 = 0x32,
        X86_64 = 0x3E,
        AArch64 = 0xB7,
        RiscV = 0xF3,
    }

    #[derive(PackedStruct)]
    #[packed_struct(endian = "lsb")]
    pub struct ElfHeader32BitLittle {
        #[packed_field(size_bytes = "2", ty = "enum")]
        elf_type: ElfType,
        #[packed_field(size_bytes = "2", ty = "enum")]
        instruction_set: InstructionSet,
        elf_header_version: u32,
        program_entry: u32,
        program_header_table_offset: u32,
        section_header_table_offset: u32,
        flags: u32,
        header_size: u16,
        program_header_table_entry_size: u16,
        program_header_table_len: u16,
        section_header_table_entry_size: u16,
        section_header_table_len: u16,
        section_header_table_index_of_names: u16,
    }

    #[derive(PackedStruct)]
    #[packed_struct(endian = "msb")]
    pub struct ElfHeader32BitBig {
        #[packed_field(size_bytes = "2", ty = "enum")]
        elf_type: ElfType,
        #[packed_field(size_bytes = "2", ty = "enum")]
        instruction_set: InstructionSet,
        elf_header_version: u32,
        program_entry: u32,
        program_header_table_offset: u32,
        section_header_table_offset: u32,
        flags: u32,
        header_size: u16,
        program_header_table_entry_size: u16,
        program_header_table_len: u16,
        section_header_table_entry_size: u16,
        section_header_table_len: u16,
        section_header_table_index_of_names: u16,
    }

    #[derive(PackedStruct)]
    #[packed_struct(endian = "lsb")]
    pub struct ElfHeader64BitLittle {
        #[packed_field(size_bytes = "2", ty = "enum")]
        elf_type: ElfType,
        #[packed_field(size_bytes = "2", ty = "enum")]
        instruction_set: InstructionSet,
        elf_header_version: u32,
        program_entry: u64,
        program_header_table_offset: u64,
        section_header_table_offset: u64,
        flags: u32,
        header_size: u16,
        program_header_table_entry_size: u16,
        program_header_table_len: u16,
        section_header_table_entry_size: u16,
        section_header_table_len: u16,
        section_header_table_index_of_names: u16,
    }

    #[derive(PackedStruct)]
    #[packed_struct(endian = "msb")]
    pub struct ElfHeader64BitBig {
        #[packed_field(size_bytes = "2", ty = "enum")]
        elf_type: ElfType,
        #[packed_field(size_bytes = "2", ty = "enum")]
        instruction_set: InstructionSet,
        elf_header_version: u32,
        program_entry: u64,
        program_header_table_offset: u64,
        section_header_table_offset: u64,
        flags: u32,
        header_size: u16,
        program_header_table_entry_size: u16,
        program_header_table_len: u16,
        section_header_table_entry_size: u16,
        section_header_table_len: u16,
        section_header_table_index_of_names: u16,
    }

    pub struct UniversalElfHeader {
        pub elf_type: ElfType,
        pub instruction_set: InstructionSet,
        pub elf_header_version: u32,
        pub program_entry: u64,
        pub program_header_table_offset: u64,
        pub section_header_table_offset: u64,
        pub flags: u32,
        pub header_size: u16,
        pub program_header_table_entry_size: u16,
        pub program_header_table_len: u16,
        pub section_header_table_entry_size: u16,
        pub section_header_table_len: u16,
        pub section_header_table_index_of_names: u16,
    }

    impl From<ElfHeader32BitBig> for UniversalElfHeader {
        fn from(header: ElfHeader32BitBig) -> Self {
            Self {
                elf_type: header.elf_type,
                instruction_set: header.instruction_set,
                elf_header_version: header.elf_header_version,
                program_entry: header.program_entry.into(),
                program_header_table_offset: header.program_header_table_offset.into(),
                section_header_table_offset: header.section_header_table_offset.into(),
                flags: header.flags,
                header_size: header.header_size,
                program_header_table_entry_size: header.program_header_table_entry_size,
                program_header_table_len: header.program_header_table_len,
                section_header_table_entry_size: header.section_header_table_entry_size,
                section_header_table_len: header.section_header_table_len,
                section_header_table_index_of_names: header.section_header_table_index_of_names,
            }
        }
    }

    impl From<ElfHeader32BitLittle> for UniversalElfHeader {
        fn from(header: ElfHeader32BitLittle) -> Self {
            Self {
                elf_type: header.elf_type,
                instruction_set: header.instruction_set,
                elf_header_version: header.elf_header_version,
                program_entry: header.program_entry.into(),
                program_header_table_offset: header.program_header_table_offset.into(),
                section_header_table_offset: header.section_header_table_offset.into(),
                flags: header.flags,
                header_size: header.header_size,
                program_header_table_entry_size: header.program_header_table_entry_size,
                program_header_table_len: header.program_header_table_len,
                section_header_table_entry_size: header.section_header_table_entry_size,
                section_header_table_len: header.section_header_table_len,
                section_header_table_index_of_names: header.section_header_table_index_of_names,
            }
        }
    }

    impl From<ElfHeader64BitBig> for UniversalElfHeader {
        fn from(header: ElfHeader64BitBig) -> Self {
            Self {
                elf_type: header.elf_type,
                instruction_set: header.instruction_set,
                elf_header_version: header.elf_header_version,
                program_entry: header.program_entry,
                program_header_table_offset: header.program_header_table_offset,
                section_header_table_offset: header.section_header_table_offset,
                flags: header.flags,
                header_size: header.header_size,
                program_header_table_entry_size: header.program_header_table_entry_size,
                program_header_table_len: header.program_header_table_len,
                section_header_table_entry_size: header.section_header_table_entry_size,
                section_header_table_len: header.section_header_table_len,
                section_header_table_index_of_names: header.section_header_table_index_of_names,
            }
        }
    }

    impl From<ElfHeader64BitLittle> for UniversalElfHeader {
        fn from(header: ElfHeader64BitLittle) -> Self {
            Self {
                elf_type: header.elf_type,
                instruction_set: header.instruction_set,
                elf_header_version: header.elf_header_version,
                program_entry: header.program_entry,
                program_header_table_offset: header.program_header_table_offset,
                section_header_table_offset: header.section_header_table_offset,
                flags: header.flags,
                header_size: header.header_size,
                program_header_table_entry_size: header.program_header_table_entry_size,
                program_header_table_len: header.program_header_table_len,
                section_header_table_entry_size: header.section_header_table_entry_size,
                section_header_table_len: header.section_header_table_len,
                section_header_table_index_of_names: header.section_header_table_index_of_names,
            }
        }
    }
}

pub mod elf_program_header {
    use super::*;
    #[derive(PrimitiveEnum_u32, Clone, Copy, PartialEq, Debug)]
    pub enum ProgramHeaderType {
        Null = 0,
        Load = 1,
        Dynamic = 2,
        Interp = 3,
        Note = 4,
    }

    #[derive(PackedStruct, Debug)]
    #[packed_struct(bit_numbering = "lsb0", size_bytes = "4")]
    pub struct Flags {
        #[packed_field(bits = "0", size_bits = "1")]
        pub executable: bool,
        #[packed_field(bits = "1", size_bits = "1")]
        pub writeable: bool,
        #[packed_field(bits = "2", size_bits = "1")]
        pub readable: bool,
    }

    #[derive(PackedStruct)]
    #[packed_struct(endian = "msb")]
    pub struct ProgramHeader32BitBig {
        #[packed_field(size_bytes = "4", ty = "enum")]
        segment_type: EnumCatchAll<ProgramHeaderType>, // NOTE: We use EnumCatchALl because we don't want to crash when loading an unknown type program header because according to: https://wiki.osdev.org/ELF, "There are more values, but mostly contain architecture/environment specific information, which is probably not required for the majority of ELF files."
        seegment_file_offset: u32,
        segment_virtual_address: u32,
        unused: u32,
        segment_file_size: u32,
        segment_virtual_size: u32,
        #[packed_field(size_bytes = "4")]
        flags: Flags,
    }

    #[derive(PackedStruct)]
    #[packed_struct(endian = "lsb")]
    pub struct ProgramHeader32BitLittle {
        #[packed_field(size_bytes = "4", ty = "enum")]
        segment_type: EnumCatchAll<ProgramHeaderType>, // NOTE: We use EnumCatchALl because we don't want to crash when loading an unknown type program header because according to: https://wiki.osdev.org/ELF, "There are more values, but mostly contain architecture/environment specific information, which is probably not required for the majority of ELF files."
        seegment_file_offset: u32,
        segment_virtual_address: u32,
        unused: u32,
        segment_file_size: u32,
        segment_virtual_size: u32,
        #[packed_field(size_bytes = "4")]
        flags: Flags,
    }

    #[derive(PackedStruct)]
    #[packed_struct(endian = "msb")]
    pub struct ProgramHeader64BitBig {
        #[packed_field(size_bytes = "4", ty = "enum")]
        segment_type: EnumCatchAll<ProgramHeaderType>, // NOTE: We use EnumCatchALl because we don't want to crash when loading an unknown type program header because according to: https://wiki.osdev.org/ELF, "There are more values, but mostly contain architecture/environment specific information, which is probably not required for the majority of ELF files."
        #[packed_field(size_bytes = "4")]
        flags: Flags,
        segment_file_offset: u64,
        segment_virtual_address: u64,
        unused: u64,
        segment_file_size: u64,
        segment_virtual_size: u64,
    }

    #[derive(PackedStruct)]
    #[packed_struct(endian = "lsb")]
    pub struct ProgramHeader64BitLittle {
        #[packed_field(size_bytes = "4", ty = "enum")]
        segment_type: EnumCatchAll<ProgramHeaderType>, // NOTE: We use EnumCatchALl because we don't want to crash when loading an unknown type program header because according to: https://wiki.osdev.org/ELF, "There are more values, but mostly contain architecture/environment specific information, which is probably not required for the majority of ELF files."
        #[packed_field(size_bytes = "4")]
        flags: Flags,
        segment_file_offset: u64,
        segment_virtual_address: u64,
        unused: u64,
        segment_file_size: u64,
        segment_virtual_size: u64,
    }

    pub struct UniversalProgramHeader {
        pub segment_type: EnumCatchAll<ProgramHeaderType>, // NOTE: We use EnumCatchALl because we don't want to crash when loading an unknown type program header because according to: https://wiki.osdev.org/ELF, "There are more values, but mostly contain architecture/environment specific information, which is probably not required for the majority of ELF files."
        pub flags: Flags,
        pub segment_file_offset: u64,
        pub segment_virtual_address: u64,
        pub segment_file_size: u64,
        pub segment_virtual_size: u64,
    }

    impl From<ProgramHeader32BitBig> for UniversalProgramHeader {
        fn from(header: ProgramHeader32BitBig) -> Self {
            Self {
                segment_type: header.segment_type,
                flags: header.flags,
                segment_file_offset: header.seegment_file_offset.into(),
                segment_virtual_address: header.segment_virtual_address.into(),
                segment_file_size: header.segment_file_size.into(),
                segment_virtual_size: header.segment_virtual_size.into(),
            }
        }
    }

    impl From<ProgramHeader32BitLittle> for UniversalProgramHeader {
        fn from(header: ProgramHeader32BitLittle) -> Self {
            Self {
                segment_type: header.segment_type,
                flags: header.flags,
                segment_file_offset: header.seegment_file_offset.into(),
                segment_virtual_address: header.segment_virtual_address.into(),
                segment_file_size: header.segment_file_size.into(),
                segment_virtual_size: header.segment_virtual_size.into(),
            }
        }
    }

    impl From<ProgramHeader64BitBig> for UniversalProgramHeader {
        fn from(header: ProgramHeader64BitBig) -> Self {
            Self {
                segment_type: header.segment_type,
                flags: header.flags,
                segment_file_offset: header.segment_file_offset,
                segment_virtual_address: header.segment_virtual_address,
                segment_file_size: header.segment_file_size,
                segment_virtual_size: header.segment_virtual_size,
            }
        }
    }

    impl From<ProgramHeader64BitLittle> for UniversalProgramHeader {
        fn from(header: ProgramHeader64BitLittle) -> Self {
            Self {
                segment_type: header.segment_type,
                flags: header.flags,
                segment_file_offset: header.segment_file_offset,
                segment_virtual_address: header.segment_virtual_address,
                segment_file_size: header.segment_file_size,
                segment_virtual_size: header.segment_virtual_size,
            }
        }
    }
}


use elf_header::*;
use elf_program_header::*;

pub struct ElfFile {
    pub header: UniversalElfHeader,
    pub program_headers: Vec<UniversalProgramHeader>,
}

impl ElfFile {
    pub fn from_bytes(bytes: &[u8]) -> Option<ElfFile> {
        use core::convert::TryInto;
        let mut curr_offset = 0;
        // First parse identification
        let id: ElfIdentification =
            ElfIdentification::unpack(bytes[curr_offset..curr_offset+ElfIdentification::packed_bytes_size(None).ok()?].try_into().ok()?).ok()?;
        curr_offset += Self::get_ondisk_identification_size();

        if id.magic != [0x7f, b'E', b'L', b'F'] {
            return None;
        }

        if id.elf_version != 1 {
            return None;
        }

        let universal_header: UniversalElfHeader = match (id.endianess, id.arch_width) {
            (Endianess::LITTLE, ArchWidth::Width32Bit) => ElfHeader32BitLittle::unpack(bytes[curr_offset..curr_offset+ElfHeader32BitLittle::packed_bytes_size(None).ok()?].try_into().ok()?).ok()?.into(),
            (Endianess::LITTLE, ArchWidth::Width64Bit) => ElfHeader64BitLittle::unpack(bytes[curr_offset..curr_offset+ElfHeader64BitLittle::packed_bytes_size(None).ok()?].try_into().ok()?).ok()?.into(),
            (Endianess::BIG, ArchWidth::Width32Bit) => ElfHeader32BitBig::unpack(bytes[curr_offset..curr_offset+ElfHeader32BitBig::packed_bytes_size(None).ok()?].try_into().ok()?).ok()?.into(),
            (Endianess::BIG, ArchWidth::Width64Bit) => ElfHeader64BitBig::unpack(bytes[curr_offset..curr_offset+ElfHeader64BitBig::packed_bytes_size(None).ok()?].try_into().ok()?).ok()?.into()
        };

        if usize::from(universal_header.header_size) != Self::get_ondisk_elf_header_size(id.arch_width) {
            return None;
        }

        if universal_header.elf_header_version != 1 {
            return None;
        }

        // Now read program header table
        let program_headers = {
            let mut vec: Vec<UniversalProgramHeader> = Vec::new();
            let mut curr_offset = universal_header.program_header_table_offset as usize;
            for _ in 0..universal_header.program_header_table_len {
                let universal_program_header = match (id.endianess, id.arch_width) {
                    (Endianess::LITTLE, ArchWidth::Width32Bit) => ProgramHeader32BitLittle::unpack(bytes[curr_offset..curr_offset+ProgramHeader32BitLittle::packed_bytes_size(None).ok()?].try_into().ok()?).ok()?.into(),
                    (Endianess::LITTLE, ArchWidth::Width64Bit) => ProgramHeader64BitLittle::unpack(bytes[curr_offset..curr_offset+ProgramHeader64BitLittle::packed_bytes_size(None).ok()?].try_into().ok()?).ok()?.into(),
                    (Endianess::BIG, ArchWidth::Width32Bit) => ProgramHeader32BitBig::unpack(bytes[curr_offset..curr_offset+ProgramHeader32BitBig::packed_bytes_size(None).ok()?].try_into().ok()?).ok()?.into(),
                    (Endianess::BIG, ArchWidth::Width64Bit) => ProgramHeader64BitBig::unpack(bytes[curr_offset..curr_offset+ProgramHeader64BitBig::packed_bytes_size(None).ok()?].try_into().ok()?).ok()?.into()
                };
                vec.push(universal_program_header);
                curr_offset += universal_header.program_header_table_entry_size as usize;
            }
            vec
        };

        Some(ElfFile {
            header: universal_header,
            program_headers
        })
    }

    fn get_ondisk_identification_size() -> usize {
        16
    }

    fn get_ondisk_elf_header_size(width: ArchWidth) -> usize {
        match width {
            ArchWidth::Width32Bit => 52,
            ArchWidth::Width64Bit => 64,
        }
    }

}
