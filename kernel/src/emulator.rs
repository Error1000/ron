use core::fmt::Debug;
use packed_struct::prelude::*;

pub trait EmulatorMemory {
    // WARNING: For an implementation to be spec compliant, it must be:
    // "byte-address invariant, the following property holds: if a
    // byte is stored to memory at some address in some endianness, then a byte-sized load from that
    // address in any endianness returns the stored value." (RISC-V Volume I, section 2.6)

    // NE - native endian, LE - little endian
    fn read_u8_ne(&self, addr: u64) -> u8;
    fn write_u8_ne(&mut self, addr: u64, val: u8);

    fn read_u16_ne(&self, addr: u64) -> u16;
    fn write_u16_ne(&mut self, addr: u64, val: u16);

    fn read_u32_ne(&self, addr: u64) -> u32;
    fn write_u32_ne(&mut self, addr: u64, val: u32);

    fn read_u64_ne(&self, addr: u64) -> u64;
    fn write_u64_ne(&mut self, addr: u64, val: u64);

    fn read_u32_le(&self, addr: u64) -> u32; // For reading instructions
                                             // Source: RISC-V Volume I 20191213, Section 1.5, in a footnote: "We have to fix the order in which instruction parcels are stored in memory, independent
                                             // of memory system endianness, to ensure that the length-encoding bits always appear first in
                                             // halfword address order"
                                             // If they didn't do this then there would be ambigous cases in big endian, like: 40 85 01 37 01 37
                                             // where 40 85 = 0x4085 - li x1, 1
                                             // 01 37 01 37 = 0x01370137 - lui x2, 0x01370
                                             // but 40 85 01 37 = 0x40850137 could also be lui x2, 0x040850
                                             // Howver because it's little endian, then it is: 85 40 37 01 37 01
                                             // And it is no longer ambigous
                                             // As 85 40 37 01 = 0x01374085, cannot be lui
}

mod riscv_instruction {
    use super::*;

    pub enum RiscvInstType {
        RType,
        IType,
        SType,
        BType,
        UType,
        JType,
    }

    pub enum RiscvCompressedInstType {
        CRType,
        CIType,
        CSSType,
        CIWType,
        CLType,
        CSType,
        CAType,
        CBType,
        CJType,
    }

    // Note: This is intended to only be 6 bits
    #[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq)]
    pub enum RiscvOpcode {
        OPIMM = 0b0010011,   // I-type
        OPIMM32 = 0b0011011, // I-type
        LUI = 0b0110111,     // U-type
        AUIPC = 0b0010111,   // U-type
        OP = 0b0110011,      // R-type
        OP32 = 0b0111011,    // R-type
        JAL = 0b1101111,     // J-type
        JALR = 0b1100111,    // I-type
        BRANCH = 0b1100011,  // B-type
        LOAD = 0b0000011,    // I-type
        STORE = 0b0100011,   // S-type
        MISCMEM = 0b0001111, // I-type
        SYSTEM = 0b1110011,  // I-type
    }

    impl RiscvOpcode {
        pub fn get_type(&self) -> RiscvInstType {
            match self {
                RiscvOpcode::OPIMM => RiscvInstType::IType,
                RiscvOpcode::OPIMM32 => RiscvInstType::IType,
                RiscvOpcode::LUI => RiscvInstType::UType,
                RiscvOpcode::AUIPC => RiscvInstType::UType,
                RiscvOpcode::OP => RiscvInstType::RType,
                RiscvOpcode::OP32 => RiscvInstType::RType,
                RiscvOpcode::JAL => RiscvInstType::JType,
                RiscvOpcode::JALR => RiscvInstType::IType,
                RiscvOpcode::BRANCH => RiscvInstType::BType,
                RiscvOpcode::LOAD => RiscvInstType::IType,
                RiscvOpcode::STORE => RiscvInstType::SType,
                RiscvOpcode::MISCMEM => RiscvInstType::IType, // NOTE: Not quite I type but close enough
                RiscvOpcode::SYSTEM => RiscvInstType::IType,
            }
        }
    }

    // Note: This is intended to only be 2 bits
    #[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq)]
    pub enum RiscvCompressedOpcode {
        C0 = 0b00,
        C1 = 0b01,
        C2 = 0b10,
    }

    pub fn get_compressed_instruction_type(inst: u16) -> Option<RiscvCompressedInstType> {
        Some(
            match (
                RiscvCompressedOpcode::from_primitive((inst & 0b11) as u8)?,
                (inst >> 13) & 0b111,
                (inst >> 10) & 0b11,
            ) {
                (RiscvCompressedOpcode::C0, 0b000, _) => RiscvCompressedInstType::CIWType,
                (RiscvCompressedOpcode::C0, 0b001, _) => RiscvCompressedInstType::CLType,
                (RiscvCompressedOpcode::C0, 0b010, _) => RiscvCompressedInstType::CLType,
                (RiscvCompressedOpcode::C0, 0b011, _) => RiscvCompressedInstType::CLType,
                (RiscvCompressedOpcode::C0, 0b100, _) => return None, // Reserved
                (RiscvCompressedOpcode::C0, 0b101, _) => RiscvCompressedInstType::CSType,
                (RiscvCompressedOpcode::C0, 0b110, _) => RiscvCompressedInstType::CSType,
                (RiscvCompressedOpcode::C0, 0b111, _) => RiscvCompressedInstType::CSType,

                (RiscvCompressedOpcode::C1, 0b000, _) => RiscvCompressedInstType::CIType,
                (RiscvCompressedOpcode::C1, 0b001, _) => RiscvCompressedInstType::CIType, /* Note: This is different on RV32 */
                (RiscvCompressedOpcode::C1, 0b010, _) => RiscvCompressedInstType::CIType,
                (RiscvCompressedOpcode::C1, 0b011, _) => RiscvCompressedInstType::CIType,

                (RiscvCompressedOpcode::C1, 0b100, 0b00) => RiscvCompressedInstType::CBType,
                (RiscvCompressedOpcode::C1, 0b100, 0b01) => RiscvCompressedInstType::CBType,
                (RiscvCompressedOpcode::C1, 0b100, 0b10) => RiscvCompressedInstType::CBType,
                (RiscvCompressedOpcode::C1, 0b100, 0b11) => RiscvCompressedInstType::CAType,

                (RiscvCompressedOpcode::C1, 0b101, _) => RiscvCompressedInstType::CJType,
                (RiscvCompressedOpcode::C1, 0b110, _) => RiscvCompressedInstType::CBType,
                (RiscvCompressedOpcode::C1, 0b111, _) => RiscvCompressedInstType::CBType,

                (RiscvCompressedOpcode::C2, 0b000, _) => RiscvCompressedInstType::CIType,
                (RiscvCompressedOpcode::C2, 0b001, _) => RiscvCompressedInstType::CIType,
                (RiscvCompressedOpcode::C2, 0b010, _) => RiscvCompressedInstType::CIType,
                (RiscvCompressedOpcode::C2, 0b011, _) => RiscvCompressedInstType::CIType,
                (RiscvCompressedOpcode::C2, 0b100, _) => RiscvCompressedInstType::CRType,
                (RiscvCompressedOpcode::C2, 0b101, _) => RiscvCompressedInstType::CSSType,
                (RiscvCompressedOpcode::C2, 0b110, _) => RiscvCompressedInstType::CSSType,
                (RiscvCompressedOpcode::C2, 0b111, _) => RiscvCompressedInstType::CSSType,
                _ => return None,
            },
        )
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "4", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvRTypeInstruction {
        #[packed_field(bits = "0..=6", ty = "enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits = "7..=11")]
        pub rd: u8,
        #[packed_field(bits = "12..=14")]
        pub funct3: u8,
        #[packed_field(bits = "15..=19")]
        pub rs1: u8,
        #[packed_field(bits = "20..=24")]
        pub rs2: u8,
        #[packed_field(bits = "25..=31")]
        pub funct7: u8,
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "4", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvITypeInstruction {
        #[packed_field(bits = "0..=6", ty = "enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits = "7..=11")]
        pub rd: u8,
        #[packed_field(bits = "12..=14")]
        pub funct3: u8,
        #[packed_field(bits = "15..=19")]
        pub rs1: u8,
        #[packed_field(bits = "20..=31")]
        imm: u16,
    }

    impl RiscvITypeInstruction {
        // Note truncates imm to 12 bits
        pub fn from(opcode: RiscvOpcode, rd: u8, funct3: u8, rs1: u8, imm: u32) -> Self {
            RiscvITypeInstruction {
                opcode,
                rd,
                funct3,
                rs1,
                imm: imm as u16,
            }
        }

        pub fn parse_imm(&self) -> u32 {
            // 12 bits in length
            let top_bit: u32 = 1 << (12 - 1);
            let all_ones: u32 = (1 << 12) - 1;

            let non_sign_extended_imm: u32 = u32::from(u16::from(self.imm));
            let sign = (non_sign_extended_imm & top_bit) != 0;
            if sign {
                return non_sign_extended_imm | (!all_ones); // Make sure the top bits are one
            } else {
                // NOTE: This should be a no-op, because it should zero-extened anyway, but for clarity i added it anyway
                return non_sign_extended_imm & all_ones; // Make sure the top bits are zero
            }
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "4", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvSTypeInstruction {
        #[packed_field(bits = "0..=6", ty = "enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits = "7..=11")]
        imm_low5: u8,
        #[packed_field(bits = "12..=14")]
        pub funct3: u8,
        #[packed_field(bits = "15..=19")]
        pub rs1: u8,
        #[packed_field(bits = "20..=24")]
        pub rs2: u8,
        #[packed_field(bits = "25..=31")]
        imm_hi7: u8,
    }

    impl RiscvSTypeInstruction {
        // Immediate will be truncated to 12 bits
        pub fn from(opcode: RiscvOpcode, funct3: u8, rs1: u8, rs2: u8, imm: u32) -> Self {
            RiscvSTypeInstruction {
                opcode,
                imm_low5: ((imm >> 0) & 0b1_1111) as u8,
                funct3,
                rs1,
                rs2,
                imm_hi7: ((imm >> 5) & 0b111_1111) as u8,
            }
        }

        pub fn parse_imm(&self) -> u32 {
            // 12 bits in length
            let top_bit: u32 = 1 << (12 - 1);
            let all_ones: u32 = (1 << 12) - 1;

            let non_sign_extended_imm: u32 =
                u32::from(u8::from(self.imm_low5)) | u32::from(u8::from(self.imm_hi7)) << 5;
            let sign = (non_sign_extended_imm & top_bit) != 0;
            if sign {
                return non_sign_extended_imm | (!all_ones); // Make sure the top bits are one
            } else {
                // NOTE: This should be a no-op, because it should zero-extened anyway, but for clarity i added it anyway
                return non_sign_extended_imm & all_ones; // Make sure the top bits are zero
            }
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "4", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvBTypeInstruction {
        #[packed_field(bits = "0..=6", ty = "enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits = "7")]
        imm_bit11: u8,
        #[packed_field(bits = "8..=11")]
        imm_bits_4_1: u8,
        #[packed_field(bits = "12..=14")]
        pub funct3: u8,
        #[packed_field(bits = "15..=19")]
        pub rs1: u8,
        #[packed_field(bits = "20..=24")]
        pub rs2: u8,
        #[packed_field(bits = "25..=30")]
        imm_bits_10_5: u8,
        #[packed_field(bits = "31")]
        imm_bit12: u8,
    }

    impl RiscvBTypeInstruction {
        // Note: Immediate gets truncated to 13 bits
        pub fn from(opcode: RiscvOpcode, funct3: u8, rs1: u8, rs2: u8, imm: u32) -> Self {
            RiscvBTypeInstruction {
                opcode,
                imm_bit11: ((imm >> 11) & 1) as u8,
                imm_bits_4_1: ((imm >> 1) & 0b1111) as u8,
                imm_bits_10_5: ((imm >> 5) & 0b1111_11) as u8,
                imm_bit12: ((imm >> 12) & 1) as u8,
                funct3,
                rs1,
                rs2,
            }
        }

        pub fn parse_imm(&self) -> u32 {
            // 13 bits in length, as we start at bit 0
            let top_bit: u32 = 1 << (13 - 1);
            let all_ones: u32 = (1 << 13) - 1;

            let non_sign_extended_imm: u32 = u32::from(u8::from(self.imm_bits_4_1)) << 1
                | u32::from(u8::from(self.imm_bits_10_5)) << 5
                | u32::from(u8::from(self.imm_bit11)) << 11
                | u32::from(u8::from(self.imm_bit12)) << 12;
            let sign = (non_sign_extended_imm & top_bit) != 0;
            if sign {
                return non_sign_extended_imm | (!all_ones); // Make sure the top bits are one
            } else {
                // NOTE: This should be a no-op, because it should zero-extened anyway, but for clarity i added it anyway
                return non_sign_extended_imm & all_ones; // Make sure the top bits are zero
            }
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "4", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvUTypeInstruction {
        #[packed_field(bits = "0..=6", ty = "enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits = "7..=11")]
        pub rd: u8,
        #[packed_field(bits = "12..=31")]
        imm: u32,
    }

    impl RiscvUTypeInstruction {
        // Note: Immediate bottom 12 bits will be ignored
        pub fn from(opcode: RiscvOpcode, rd: u8, imm: u32) -> Self {
            RiscvUTypeInstruction {
                opcode,
                rd,
                imm: imm >> 12,
            }
        }

        pub fn parse_imm(&self) -> u32 {
            u32::from(self.imm) << 12
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "4", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvJTypeInstruction {
        #[packed_field(bits = "0..=6", ty = "enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits = "7..=11")]
        pub rd: u8,
        #[packed_field(bits = "12..=19")]
        imm_bits_19_12: u8,
        #[packed_field(bits = "20")]
        imm_bit11: u8,
        #[packed_field(bits = "21..=30")]
        imm_bits_10_1: u16,
        #[packed_field(bits = "31")]
        imm_bit20: u8,
    }

    impl RiscvJTypeInstruction {
        // Will truncate immediate to 21 bits
        pub fn from(opcode: RiscvOpcode, rd: u8, imm: u32) -> Self {
            RiscvJTypeInstruction {
                opcode,
                rd,
                imm_bits_19_12: ((imm >> 12) & 0b1111_1111) as u8,
                imm_bit11: ((imm >> 11) & 0b1) as u8,
                imm_bits_10_1: ((imm >> 1) & 0b1111_1111_11) as u16,
                imm_bit20: ((imm >> 20) & 0b1) as u8,
            }
        }

        pub fn parse_imm(&self) -> u32 {
            // 21 bits in length, as we start at bit 0
            let top_bit: u32 = 1 << (21 - 1);
            let all_ones: u32 = (1 << 21) - 1;

            let non_sign_extended_imm: u32 = u32::from(u16::from(self.imm_bits_10_1)) << 1
                | u32::from(u8::from(self.imm_bit11)) << 11
                | u32::from(u8::from(self.imm_bits_19_12)) << 12
                | u32::from(u8::from(self.imm_bit20)) << 20;
            let sign = (non_sign_extended_imm & top_bit) != 0;
            if sign {
                return non_sign_extended_imm | (!all_ones); // Make sure the top bits are one
            } else {
                // NOTE: This should be a no-op, because it should zero-extened anyway, but for clarity i added it anyway
                return non_sign_extended_imm & all_ones; // Make sure the top bits are zero
            }
        }
    }

    // Compressed instructions

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "2", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvCRTypeInstruction {
        #[packed_field(bits = "0..=1", ty = "enum")]
        pub opcode: RiscvCompressedOpcode,
        #[packed_field(bits = "2..=6")]
        pub rs2: u8,
        #[packed_field(bits = "7..=11")]
        pub rd_rs1: u8,
        #[packed_field(bits = "12..=15")]
        pub funct4: u8,
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "2", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvCITypeInstruction {
        #[packed_field(bits = "0..=1", ty = "enum")]
        pub opcode: RiscvCompressedOpcode,
        #[packed_field(bits = "2..=6")]
        imm_part1: u8,
        #[packed_field(bits = "7..=11")]
        pub rd_rs1: u8,
        #[packed_field(bits = "12")]
        imm_part2: u8,
        #[packed_field(bits = "13..=15")]
        pub funct3: u8,
    }

    impl RiscvCITypeInstruction {
        pub fn parse_imm(&self) -> Option<u32> {
            match (self.funct3, self.opcode) {
                (0b000, RiscvCompressedOpcode::C2) => {
                    return Some(((self.imm_part2 as u32) & 1) << 5 | (self.imm_part1 as u32))
                }
                (0b010, RiscvCompressedOpcode::C2) => {
                    return Some(
                        ((self.imm_part1 as u32) & 0b11) << 6
                            | (self.imm_part2 as u32) << 5
                            | ((self.imm_part1 as u32) & 0b11100),
                    )
                }
                (0b011, RiscvCompressedOpcode::C2) => {
                    return Some(
                        ((self.imm_part1 as u32) & 0b111) << 6
                            | (self.imm_part2 as u32) << 5
                            | ((self.imm_part1 as u32) & 0b11000),
                    )
                }

                (0b010, RiscvCompressedOpcode::C1)
                | (0b000, RiscvCompressedOpcode::C1)
                | (0b001, RiscvCompressedOpcode::C1) => {
                    return Some(sign_extend::<u8, u32>(
                        (self.imm_part2 as u8) << 7
                            | (self.imm_part2 as u8) << 6
                            | (self.imm_part2 as u8) << 5
                            | (self.imm_part1 as u8),
                    ))
                }
                (0b011, RiscvCompressedOpcode::C1) => {
                    if self.rd_rs1 != 2 {
                        return Some(
                            sign_extend::<u8, u32>(
                                (self.imm_part2 as u8) << 7
                                    | (self.imm_part2 as u8) << 6
                                    | (self.imm_part2 as u8) << 5
                                    | (self.imm_part1 as u8),
                            ) << 12,
                        );
                    } else {
                        let non_sign_extended_imm = ((self.imm_part2 as u32) & 1) << 9
                            | (((self.imm_part1 as u32) >> 1) & 0b11) << 7
                            | (((self.imm_part1 as u32) >> 3) & 1) << 6
                            | ((self.imm_part1 as u32) & 0b1) << 5
                            | ((self.imm_part1 as u32) & 0b10000);
                        let sign_bit = (self.imm_part2 as u32) & 1 != 0;
                        let all_ones = 0b11_1111_1111;
                        let imm = if sign_bit {
                            non_sign_extended_imm | (!all_ones)
                        } else {
                            non_sign_extended_imm & all_ones
                        };
                        return Some(imm);
                    }
                }

                _ => return None,
            }
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "2", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvCSSTypeInstruction {
        #[packed_field(bits = "0..=1", ty = "enum")]
        pub opcode: RiscvCompressedOpcode,
        #[packed_field(bits = "2..=6")]
        pub rs2: u8,
        #[packed_field(bits = "7..=12")]
        imm: u8,
        #[packed_field(bits = "13..=15")]
        pub funct3: u8,
    }

    impl RiscvCSSTypeInstruction {
        pub fn parse_imm(&self) -> Option<u32> {
            match (self.funct3, self.opcode) {
                (0b110, RiscvCompressedOpcode::C2) => {
                    return Some((self.imm as u32) & 0b111100 | ((self.imm as u32) & 0b11) << 6)
                }
                (0b111, RiscvCompressedOpcode::C2) => {
                    return Some((self.imm as u32) & 0b111000 | ((self.imm as u32) & 0b111) << 6)
                }
                _ => return None,
            }
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "2", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvCIWTypeInstruction {
        #[packed_field(bits = "0..=1", ty = "enum")]
        pub opcode: RiscvCompressedOpcode,
        #[packed_field(bits = "2..=4")]
        compressed_rd: u8,
        #[packed_field(bits = "5..=12")]
        imm: u8,
        #[packed_field(bits = "13..=15")]
        pub funct3: u8,
    }

    impl RiscvCIWTypeInstruction {
        pub fn parse_imm(&self) -> Option<u32> {
            match (self.funct3, self.opcode) {
                (0b000, RiscvCompressedOpcode::C0) => {
                    return Some(
                        (((self.imm as u32) >> 2) & 0b1111) << 6
                            | (((self.imm as u32) >> 6) & 0b11) << 4
                            | (((self.imm as u32) >> 0) & 1) << 3
                            | (((self.imm as u32) >> 1) & 1) << 2,
                    )
                }

                _ => return None,
            }
        }

        pub fn parse_rd(&self) -> u8 {
            self.compressed_rd + 8
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "2", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvCLTypeInstruction {
        #[packed_field(bits = "0..=1", ty = "enum")]
        pub opcode: RiscvCompressedOpcode,
        #[packed_field(bits = "2..=4")]
        compressed_rd: u8,
        #[packed_field(bits = "5..=6")]
        imm_part1: u8,
        #[packed_field(bits = "7..=9")]
        compressed_rs1: u8,
        #[packed_field(bits = "10..=12")]
        imm_part2: u8,
        #[packed_field(bits = "13..=15")]
        pub funct3: u8,
    }

    impl RiscvCLTypeInstruction {
        pub fn parse_imm(&self) -> Option<u32> {
            match (self.funct3, self.opcode) {
                (0b010, RiscvCompressedOpcode::C0) => {
                    let imm_part1 = self.imm_part1 as u32;
                    let imm_part2 = self.imm_part2 as u32;
                    Some(((imm_part1 >> 0) & 1) << 6 | imm_part2 << 3 | ((imm_part1 >> 1) & 1) << 2)
                }
                (0b011, RiscvCompressedOpcode::C0) => {
                    let imm_part1 = self.imm_part1 as u32;
                    let imm_part2 = self.imm_part2 as u32;
                    Some(((imm_part1 >> 1) & 1) << 7 | ((imm_part1 >> 0) & 1) << 6 | imm_part2 << 3)
                }
                _ => return None,
            }
        }

        pub fn parse_rs1(&self) -> u8 {
            self.compressed_rs1 + 8
        }

        pub fn parse_rd(&self) -> u8 {
            self.compressed_rd + 8
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "2", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvCSTypeInstruction {
        #[packed_field(bits = "0..=1", ty = "enum")]
        pub opcode: RiscvCompressedOpcode,
        #[packed_field(bits = "2..=4")]
        compressed_rs2: u8,
        #[packed_field(bits = "5..=6")]
        imm_part1: u8,
        #[packed_field(bits = "7..=9")]
        compressed_rs1: u8,
        #[packed_field(bits = "10..=12")]
        imm_part2: u8,
        #[packed_field(bits = "13..=15")]
        pub funct3: u8,
    }

    impl RiscvCSTypeInstruction {
        pub fn parse_imm(&self) -> Option<u32> {
            match (self.funct3, self.opcode) {
                (0b110, RiscvCompressedOpcode::C0) => {
                    let imm_part1 = self.imm_part1 as u32;
                    let imm_part2 = self.imm_part2 as u32;
                    Some(((imm_part1 >> 0) & 1) << 6 | imm_part2 << 3 | ((imm_part1 >> 1) & 1) << 2)
                }
                (0b111, RiscvCompressedOpcode::C0) => {
                    let imm_part1 = self.imm_part1 as u32;
                    let imm_part2 = self.imm_part2 as u32;
                    Some(((imm_part1 >> 1) & 1) << 7 | ((imm_part1 >> 0) & 1) << 6 | imm_part2 << 3)
                }
                _ => return None,
            }
        }

        pub fn parse_rs1(&self) -> u8 {
            self.compressed_rs1 + 8
        }

        pub fn parse_rs2(&self) -> u8 {
            self.compressed_rs2 + 8
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "2", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvCATypeInstruction {
        #[packed_field(bits = "0..=1", ty = "enum")]
        pub opcode: RiscvCompressedOpcode,
        #[packed_field(bits = "2..=4")]
        compressed_rs2: u8,
        #[packed_field(bits = "5..=6")]
        pub funct2: u8,
        #[packed_field(bits = "7..=9")]
        compressed_rd_rs1: u8,
        #[packed_field(bits = "10..=15")]
        pub funct6: u8,
    }

    impl RiscvCATypeInstruction {
        pub fn parse_rs2(&self) -> u8 {
            self.compressed_rs2 + 8
        }

        pub fn parse_rd_rs1(&self) -> u8 {
            self.compressed_rd_rs1 + 8
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "2", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvCBTypeInstruction {
        #[packed_field(bits = "0..=1", ty = "enum")]
        pub opcode: RiscvCompressedOpcode,
        #[packed_field(bits = "2..=6")]
        imm_part1: u8,
        #[packed_field(bits = "7..=9")]
        compressed_rd_rs1: u8,
        #[packed_field(bits = "10..=12")]
        imm_part2: u8,
        #[packed_field(bits = "13..=15")]
        pub funct3: u8,
    }

    impl RiscvCBTypeInstruction {
        pub fn parse_imm(&self) -> Option<u32> {
            match (self.funct3, self.opcode) {
                (0b110, RiscvCompressedOpcode::C1) | (0b111, RiscvCompressedOpcode::C1) => {
                    let imm_part1 = self.imm_part1 as u32;
                    let imm_part2 = self.imm_part2 as u32;
                    let sign_bit = (imm_part2 >> 2) & 1 == 1;
                    let non_sign_extended_imm = ((imm_part2 >> 2) & 1) << 8
                        | ((imm_part1 >> 3) & 0b11) << 6
                        | (imm_part1 & 1) << 5
                        | (imm_part2 & 0b11) << 3
                        | (imm_part1 & 0b110);
                    let all_ones = 0b1_1111_1111 as u32;
                    let imm = if sign_bit {
                        non_sign_extended_imm | (!all_ones)
                    } else {
                        non_sign_extended_imm & all_ones
                    };
                    return Some(imm);
                }

                (0b100, RiscvCompressedOpcode::C1) => {
                    if self.parse_funct2() != 0b10 {
                        return Some(((self.imm_part2 as u32) >> 2) << 5 | (self.imm_part1 as u32));
                    } else {
                        return Some(sign_extend::<u8, u32>(
                            (self.imm_part2 >> 2) << 7
                                | (self.imm_part2 >> 2) << 6
                                | (self.imm_part2 >> 2) << 5
                                | self.imm_part1,
                        ));
                    }
                }

                _ => return None,
            }
        }

        pub fn parse_funct2(&self) -> u8 {
            self.imm_part2 & 0b11
        }

        pub fn parse_rd_rs1(&self) -> u8 {
            self.compressed_rd_rs1 + 8
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes = "2", bit_numbering = "lsb0", endian = "msb")]
    pub struct RiscvCJTypeInstruction {
        #[packed_field(bits = "0..=1", ty = "enum")]
        pub opcode: RiscvCompressedOpcode,
        #[packed_field(bits = "2")]
        imm_bit5: u8,
        #[packed_field(bits = "3..=5")]
        imm_3_1: u8,
        #[packed_field(bits = "6")]
        imm_bit7: u8,
        #[packed_field(bits = "7")]
        imm_bit6: u8,
        #[packed_field(bits = "8")]
        imm_bit10: u8,
        #[packed_field(bits = "9..=10")]
        imm_9_8: u8,
        #[packed_field(bits = "11")]
        imm_bit4: u8,
        #[packed_field(bits = "12")]
        imm_bit11: u8,
        #[packed_field(bits = "13..=15")]
        pub funct3: u8,
    }

    impl RiscvCJTypeInstruction {
        pub fn parse_imm(&self) -> Option<u32> {
            match (self.funct3, self.opcode) {
                (0b101, RiscvCompressedOpcode::C1) => Some(
                    (self.imm_bit11 as u32) << 11
                        | (self.imm_bit10 as u32) << 10
                        | (self.imm_9_8 as u32) << 8
                        | (self.imm_bit7 as u32) << 7
                        | (self.imm_bit6 as u32) << 6
                        | (self.imm_bit5 as u32) << 5
                        | (self.imm_bit4 as u32) << 4
                        | (self.imm_3_1 as u32) << 1,
                ),
                _ => return None,
            }
        }
    }
}

use riscv_instruction::*;

// NOTE: Does not support T's bigger than 256-bits wide
fn sign_extend<T, U>(val: T) -> U
where
    U: From<T>
        + From<u8>
        + core::ops::Shl<Output = U>
        + core::ops::Sub<Output = U>
        + core::ops::BitAnd<Output = U>
        + core::cmp::PartialEq
        + core::ops::Not<Output = U>
        + core::ops::BitOr<Output = U>
        + Copy,
{
    let val = U::from(val);

    let max_shift_amount = U::from((core::mem::size_of::<T>() * 8 - 1) as u8);
    let one: U = U::from(1);

    let top_bit: U = one << max_shift_amount;
    let all_ones: U = ((one << max_shift_amount) - one) | top_bit; // Designed like this to avoid overflow when T is U

    let sign = val & top_bit != U::from(0);
    if sign {
        val | !all_ones
    } else {
        val & all_ones /* NOTE: This should be a no-op, because it should zero-extened anyway, but for clarity i added it anyway */
    }
}

fn sign_extend_to_u64<T>(val: T) -> u64
where
    u64: From<T>,
{
    sign_extend::<T, u64>(val)
}

// TODO: Currently some illegal instructions don't halt the cpu, instead having the effect of a nop

pub struct Riscv64Cpu<MemType>
where
    MemType: EmulatorMemory,
{
    program_counter: u64,
    registers: [u64; 31],
    memory: MemType,
    halted: bool,
    syscall: fn(&mut Self),
}

impl<MemType> Debug for Riscv64Cpu<MemType>
where
    MemType: EmulatorMemory,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Riscv64Cpu")
            .field("program_counter", &self.program_counter)
            .field("registers", &self.registers)
            .field("halted", &self.halted)
            .finish()
    }
}

impl<MemType> Riscv64Cpu<MemType>
where
    MemType: EmulatorMemory,
{
    pub fn from(mem: MemType, start_address: u64, syscall: fn(&mut Self)) -> Riscv64Cpu<MemType> {
        Riscv64Cpu {
            program_counter: start_address,
            registers: [0u64; 31],
            memory: mem,
            halted: false,
            syscall,
        }
    }

    pub fn write_reg(&mut self, reg_n: u8, val: u64) {
        if reg_n != 0 {
            self.registers[usize::from(reg_n - 1)] = val;
        }
    }

    pub fn read_reg(&self, reg_n: u8) -> u64 {
        if reg_n == 0 {
            0
        } else {
            self.registers[usize::from(reg_n - 1)]
        }
    }

    pub fn halt(&mut self) {
        self.halted = true;
    }

    // Run one clock cycle
    // Note: Returns None when ticking fails ( for example maybe instruction parsing failed, or maybe the cpu is halted )
    pub fn tick(&mut self) -> Option<()> {
        if self.halted {
            return None;
        }
        let mut instruction = self.memory.read_u32_le(self.program_counter);
        let is_compressed = (instruction & 0b11) != 0b11;
        let inst_size = if is_compressed {
            core::mem::size_of::<u16>() as u64
        } else {
            core::mem::size_of::<u32>() as u64
        };
        if is_compressed {
            let compressed_inst = instruction as u16;

            // NOTE: This implementation ignores wether a C extension instruction is reserved or not
            // only bothering to check for the cases where the opcodes overlap

            match get_compressed_instruction_type(compressed_inst)? {
                RiscvCompressedInstType::CRType => {
                    let inst: RiscvCRTypeInstruction =
                        RiscvCRTypeInstruction::unpack(&compressed_inst.to_be_bytes()).ok()?;
                    match (inst.funct4, inst.opcode) {
                        (0b1000, RiscvCompressedOpcode::C2) => {
                            if inst.rs2 == 0 {
                                // C.JR
                                instruction = u32::from_msb_bytes(
                                    &RiscvITypeInstruction::from(
                                        RiscvOpcode::JALR,
                                        0, /*x0*/
                                        0b000,
                                        inst.rd_rs1,
                                        0,
                                    )
                                    .pack()
                                    .ok()?,
                                )
                            } else {
                                // C.MV
                                instruction = u32::from_msb_bytes(
                                    &RiscvRTypeInstruction {
                                        opcode: RiscvOpcode::OP,
                                        rd: inst.rd_rs1,
                                        funct3: 0b000,
                                        rs1: inst.rs2,
                                        rs2: 0,
                                        funct7: 0b0000000,
                                    }
                                    .pack()
                                    .ok()?,
                                )
                            }
                        }

                        (0b1001, RiscvCompressedOpcode::C2) => {
                            if inst.rs2 == 0 {
                                // C.JALR
                                if inst.rd_rs1 != 0 {
                                    // C.JALR is only valid when rs1̸=x0; the code point with rs1=x0 corresponds to the C.EBREAK instruction. (RISC-V Volume I, section 16.4)
                                    instruction = u32::from_msb_bytes(
                                        &RiscvITypeInstruction::from(
                                            RiscvOpcode::JALR,
                                            1, /*x1*/
                                            0b000,
                                            inst.rd_rs1,
                                            0,
                                        )
                                        .pack()
                                        .ok()?,
                                    )
                                } else {
                                    // C.EBREAK
                                    instruction = u32::from_msb_bytes(
                                        &RiscvITypeInstruction::from(
                                            RiscvOpcode::SYSTEM,
                                            0,
                                            0b000,
                                            0,
                                            1,
                                        )
                                        .pack()
                                        .ok()?,
                                    )
                                }
                            } else {
                                // C.ADD
                                // C.ADD is only valid when rs2̸=x0; the code points with rs2=x0 correspond to the C.JALR and C.EBREAK instructions. (RISC-V Volume I, section 16.5)
                                instruction = u32::from_msb_bytes(
                                    &RiscvRTypeInstruction {
                                        opcode: RiscvOpcode::OP,
                                        rd: inst.rd_rs1,
                                        funct3: 0b000,
                                        rs1: inst.rd_rs1,
                                        rs2: inst.rs2,
                                        funct7: 0b0000000,
                                    }
                                    .pack()
                                    .ok()?,
                                )
                            }
                        }

                        _ => (),
                    }
                }

                RiscvCompressedInstType::CIType => {
                    let inst: RiscvCITypeInstruction =
                        RiscvCITypeInstruction::unpack(&compressed_inst.to_be_bytes()).ok()?;
                    match (inst.funct3, inst.opcode) {
                        (0b000, RiscvCompressedOpcode::C2) =>
                        // C.SLLI
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvITypeInstruction::from(
                                    RiscvOpcode::OPIMM,
                                    inst.rd_rs1,
                                    0b001,
                                    inst.rd_rs1,
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b010, RiscvCompressedOpcode::C2) =>
                        // C.LWSP
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvITypeInstruction::from(
                                    RiscvOpcode::LOAD,
                                    inst.rd_rs1,
                                    0b010,
                                    2, /*sp*/
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b011, RiscvCompressedOpcode::C2) =>
                        // C.LDSP
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvITypeInstruction::from(
                                    RiscvOpcode::LOAD,
                                    inst.rd_rs1,
                                    0b011,
                                    2, /*sp*/
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b010, RiscvCompressedOpcode::C1) =>
                        // C.LI
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvITypeInstruction::from(
                                    RiscvOpcode::OPIMM,
                                    inst.rd_rs1,
                                    0b000,
                                    0, /*x0*/
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b011, RiscvCompressedOpcode::C1) => {
                            if inst.rd_rs1 != 2 {
                                // C.LUI
                                instruction = u32::from_msb_bytes(
                                    &RiscvUTypeInstruction::from(
                                        RiscvOpcode::LUI,
                                        inst.rd_rs1,
                                        inst.parse_imm()?,
                                    )
                                    .pack()
                                    .ok()?,
                                )
                            } else {
                                // C.ADDI16SP
                                instruction = u32::from_msb_bytes(
                                    &RiscvITypeInstruction::from(
                                        RiscvOpcode::OPIMM,
                                        2, /*sp*/
                                        0b000,
                                        2, /*sp*/
                                        inst.parse_imm()?,
                                    )
                                    .pack()
                                    .ok()?,
                                )
                            }
                        }

                        (0b000, RiscvCompressedOpcode::C1) =>
                        // C.ADDI
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvITypeInstruction::from(
                                    RiscvOpcode::OPIMM,
                                    inst.rd_rs1,
                                    0b000,
                                    inst.rd_rs1,
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b001, RiscvCompressedOpcode::C1) =>
                        // C.ADDIW
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvITypeInstruction::from(
                                    RiscvOpcode::OPIMM32,
                                    inst.rd_rs1,
                                    0b000,
                                    inst.rd_rs1,
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        _ => (),
                    }
                }

                RiscvCompressedInstType::CSSType => {
                    let inst: RiscvCSSTypeInstruction =
                        RiscvCSSTypeInstruction::unpack(&compressed_inst.to_be_bytes()).ok()?;
                    match (inst.funct3, inst.opcode) {
                        (0b110, RiscvCompressedOpcode::C2) =>
                        // C.SWSP
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvSTypeInstruction::from(
                                    RiscvOpcode::STORE,
                                    0b010,
                                    2, /*sp*/
                                    inst.rs2,
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b111, RiscvCompressedOpcode::C2) =>
                        // C.SDSP
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvSTypeInstruction::from(
                                    RiscvOpcode::STORE,
                                    0b011,
                                    2, /*sp*/
                                    inst.rs2,
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        _ => (),
                    }
                }

                RiscvCompressedInstType::CIWType => {
                    let inst: RiscvCIWTypeInstruction =
                        RiscvCIWTypeInstruction::unpack(&compressed_inst.to_be_bytes()).ok()?;
                    match (inst.funct3, inst.opcode) {
                        (0b000, RiscvCompressedOpcode::C0) =>
                        // C.ADDI4SPN
                        {
                            instruction = u32::from_be_bytes(
                                RiscvITypeInstruction::from(
                                    RiscvOpcode::OPIMM,
                                    inst.parse_rd(),
                                    0b000,
                                    2, /*sp*/
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }
                        _ => (),
                    }
                }

                RiscvCompressedInstType::CLType => {
                    let inst: RiscvCLTypeInstruction =
                        RiscvCLTypeInstruction::unpack(&compressed_inst.to_be_bytes()).ok()?;
                    match (inst.funct3, inst.opcode) {
                        (0b010, RiscvCompressedOpcode::C0) =>
                        // C.LW
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvITypeInstruction::from(
                                    RiscvOpcode::LOAD,
                                    inst.parse_rd(),
                                    0b010,
                                    inst.parse_rs1(),
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b011, RiscvCompressedOpcode::C0) =>
                        // C.LD
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvITypeInstruction::from(
                                    RiscvOpcode::LOAD,
                                    inst.parse_rd(),
                                    0b011,
                                    inst.parse_rs1(),
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        _ => (),
                    }
                }

                RiscvCompressedInstType::CSType => {
                    let inst: RiscvCSTypeInstruction =
                        RiscvCSTypeInstruction::unpack(&compressed_inst.to_be_bytes()).ok()?;
                    match (inst.funct3, inst.opcode) {
                        (0b110, RiscvCompressedOpcode::C0) =>
                        // C.SW
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvSTypeInstruction::from(
                                    RiscvOpcode::STORE,
                                    0b010,
                                    inst.parse_rs1(),
                                    inst.parse_rs2(),
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b111, RiscvCompressedOpcode::C0) =>
                        // C.SD
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvSTypeInstruction::from(
                                    RiscvOpcode::STORE,
                                    0b011,
                                    inst.parse_rs1(),
                                    inst.parse_rs2(),
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        _ => (),
                    }
                }

                RiscvCompressedInstType::CAType => {
                    let inst: RiscvCATypeInstruction =
                        RiscvCATypeInstruction::unpack(&compressed_inst.to_be_bytes()).ok()?;
                    match (inst.funct6, inst.funct2, inst.opcode) {
                        (0b100011, 0b11, RiscvCompressedOpcode::C1) =>
                        // C.AND
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvRTypeInstruction {
                                    opcode: RiscvOpcode::OP,
                                    rd: inst.parse_rd_rs1(),
                                    funct3: 0b111,
                                    rs1: inst.parse_rd_rs1(),
                                    rs2: inst.parse_rs2(),
                                    funct7: 0b0000000,
                                }
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b100011, 0b10, RiscvCompressedOpcode::C1) =>
                        // C.OR
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvRTypeInstruction {
                                    opcode: RiscvOpcode::OP,
                                    rd: inst.parse_rd_rs1(),
                                    funct3: 0b110,
                                    rs1: inst.parse_rd_rs1(),
                                    rs2: inst.parse_rs2(),
                                    funct7: 0b0000000,
                                }
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b100011, 0b01, RiscvCompressedOpcode::C1) =>
                        // C.XOR
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvRTypeInstruction {
                                    opcode: RiscvOpcode::OP,
                                    rd: inst.parse_rd_rs1(),
                                    funct3: 0b100,
                                    rs1: inst.parse_rd_rs1(),
                                    rs2: inst.parse_rs2(),
                                    funct7: 0b0000000,
                                }
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b100011, 0b00, RiscvCompressedOpcode::C1) =>
                        // C.SUB
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvRTypeInstruction {
                                    opcode: RiscvOpcode::OP,
                                    rd: inst.parse_rd_rs1(),
                                    funct3: 0b000,
                                    rs1: inst.parse_rd_rs1(),
                                    rs2: inst.parse_rs2(),
                                    funct7: 0b0100000,
                                }
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b100111, 0b01, RiscvCompressedOpcode::C1) =>
                        // C.ADDW
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvRTypeInstruction {
                                    opcode: RiscvOpcode::OP32,
                                    rd: inst.parse_rd_rs1(),
                                    funct3: 0b000,
                                    rs1: inst.parse_rd_rs1(),
                                    rs2: inst.parse_rs2(),
                                    funct7: 0b0000000,
                                }
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b100111, 0b00, RiscvCompressedOpcode::C1) =>
                        // C.SUBW
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvRTypeInstruction {
                                    opcode: RiscvOpcode::OP32,
                                    rd: inst.parse_rd_rs1(),
                                    funct3: 0b000,
                                    rs1: inst.parse_rd_rs1(),
                                    rs2: inst.parse_rs2(),
                                    funct7: 0b0100000,
                                }
                                .pack()
                                .ok()?,
                            )
                        }
                        _ => (),
                    }
                }

                RiscvCompressedInstType::CBType => {
                    let inst: RiscvCBTypeInstruction =
                        RiscvCBTypeInstruction::unpack(&compressed_inst.to_be_bytes()).ok()?;
                    match (inst.funct3, inst.opcode) {
                        (0b110, RiscvCompressedOpcode::C1) =>
                        // C.BEQZ
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvBTypeInstruction::from(
                                    RiscvOpcode::BRANCH,
                                    0b000,
                                    inst.parse_rd_rs1(),
                                    0, /*x0*/
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b111, RiscvCompressedOpcode::C1) =>
                        // C.BNEZ
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvBTypeInstruction::from(
                                    RiscvOpcode::BRANCH,
                                    0b001,
                                    inst.parse_rd_rs1(),
                                    0, /*x0*/
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }

                        (0b100, RiscvCompressedOpcode::C1) =>
                        // C.SRLI/C.SRAI/C.ANDI
                        {
                            match inst.parse_funct2() {
                                0b00 =>
                                // C.SRLI
                                {
                                    instruction = u32::from_msb_bytes(
                                        &RiscvITypeInstruction::from(
                                            RiscvOpcode::OPIMM,
                                            inst.parse_rd_rs1(),
                                            0b101,
                                            inst.parse_rd_rs1(),
                                            inst.parse_imm()?,
                                        )
                                        .pack()
                                        .ok()?,
                                    )
                                }

                                0b01 =>
                                // C.SRAI
                                {
                                    instruction = u32::from_msb_bytes(
                                        &RiscvITypeInstruction::from(
                                            RiscvOpcode::OPIMM,
                                            inst.parse_rd_rs1(),
                                            0b101,
                                            inst.parse_rd_rs1(),
                                            inst.parse_imm()? | 0b0100000_00000,
                                        )
                                        .pack()
                                        .ok()?,
                                    )
                                }

                                0b10 =>
                                // C.ANDI
                                {
                                    instruction = u32::from_msb_bytes(
                                        &RiscvITypeInstruction::from(
                                            RiscvOpcode::OPIMM,
                                            inst.parse_rd_rs1(),
                                            0b111,
                                            inst.parse_rd_rs1(),
                                            inst.parse_imm()?,
                                        )
                                        .pack()
                                        .ok()?,
                                    )
                                }

                                _ => (),
                            }
                        }

                        _ => (),
                    }
                }

                RiscvCompressedInstType::CJType => {
                    let inst: RiscvCJTypeInstruction =
                        RiscvCJTypeInstruction::unpack(&compressed_inst.to_be_bytes()).ok()?;
                    match (inst.funct3, inst.opcode) {
                        (0b101, RiscvCompressedOpcode::C1) =>
                        // C.J
                        {
                            instruction = u32::from_msb_bytes(
                                &RiscvJTypeInstruction::from(
                                    RiscvOpcode::JAL,
                                    0, /*x0*/
                                    inst.parse_imm()?,
                                )
                                .pack()
                                .ok()?,
                            )
                        }
                        _ => (),
                    }
                }
            }
        }

        // NOTE: I would expect the output to be [147, 0, 0, 1], since the struct is marked as little-endian, but it is [1, 0, 0, 147], that's because the byte array is always big-endian and the little-endian marker onyl applies to each field not to the endiannes of the byte array produced
        // Referance: Issue #92, https://github.com/hashmismatch/packed_struct.rs/issues/92
        // So therefore i am insteada using big endian for parsing instructions
        let opcode: RiscvOpcode = RiscvOpcode::from_primitive((instruction & 0b111_1111) as u8)?;
        match opcode.get_type() {
            RiscvInstType::RType => self.execute_rtype_inst(
                RiscvRTypeInstruction::unpack(&instruction.to_be_bytes()).ok()?,
            ),
            RiscvInstType::IType => self.execute_itype_inst(
                RiscvITypeInstruction::unpack(&instruction.to_be_bytes()).ok()?,
                inst_size,
            ),
            RiscvInstType::SType => self.execute_stype_inst(
                RiscvSTypeInstruction::unpack(&instruction.to_be_bytes()).ok()?,
            ),
            RiscvInstType::BType => self.execute_btype_inst(
                RiscvBTypeInstruction::unpack(&instruction.to_be_bytes()).ok()?,
                inst_size,
            ),
            RiscvInstType::UType => self.execute_utype_inst(
                RiscvUTypeInstruction::unpack(&instruction.to_be_bytes()).ok()?,
            ),
            RiscvInstType::JType => self.execute_jtype_inst(
                RiscvJTypeInstruction::unpack(&instruction.to_be_bytes()).ok()?,
                inst_size,
            ),
        }

        self.program_counter += inst_size;
        Some(())
    }

    fn execute_rtype_inst(&mut self, inst: RiscvRTypeInstruction) {
        match (inst.opcode, inst.funct3, inst.funct7) {
            // ADD performs the addition of rs1 and rs2. SUB performs the subtraction of rs2 from rs1. Overflows
            // are ignored and the low XLEN bits of results are written to the destination rd. (RISC-V Volume I, section 2.4)
            (RiscvOpcode::OP, 0b000, 0b0000000) => {
                // ADD
                self.write_reg(
                    inst.rd,
                    self.read_reg(inst.rs1)
                        .wrapping_add(self.read_reg(inst.rs2)),
                );
            }

            (RiscvOpcode::OP, 0b000, 0b0100000) => {
                // SUB
                self.write_reg(
                    inst.rd,
                    self.read_reg(inst.rs1)
                        .wrapping_sub(self.read_reg(inst.rs2)),
                );
            }

            // ADDW and SUBW are RV64I-only instructions that are defined analogously to ADD and SUB
            // but operate on 32-bit values and produce signed 32-bit results. Overflows are ignored, and the low
            // 32-bits of the result is sign-extended to 64-bits and written to the destination register (RISC-V Volume I, section 5.2)
            (RiscvOpcode::OP32, 0b000, 0b0000000) => {
                // ADDW
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        (self.read_reg(inst.rs1) as u32)
                            .wrapping_add(self.read_reg(inst.rs2) as u32),
                    ),
                );
            }

            (RiscvOpcode::OP32, 0b000, 0b0100000) => {
                // SUBW
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        (self.read_reg(inst.rs1) as u32)
                            .wrapping_sub(self.read_reg(inst.rs2) as u32),
                    ),
                );
            }

            // SLT and SLTU perform signed and unsigned compares respectively, writing 1 to rd if rs1 < rs2, 0 otherwise (RISC-V Volume I, section 2.4)
            (RiscvOpcode::OP, 0b010, 0b0000000) => {
                // SLT
                let signed_rs1 = self.read_reg(inst.rs1) as i64;
                let signed_rs2 = self.read_reg(inst.rs2) as i64;
                self.write_reg(inst.rd, if signed_rs1 < signed_rs2 { 1 } else { 0 });
            }

            (RiscvOpcode::OP, 0b011, 0b0000000) => {
                // SLTU
                let unsigned_rs1 = self.read_reg(inst.rs1);
                let unsigned_rs2 = self.read_reg(inst.rs2);
                self.write_reg(inst.rd, if unsigned_rs1 < unsigned_rs2 { 1 } else { 0 });
            }

            (RiscvOpcode::OP, 0b100, 0b0000000) => {
                // XOR
                self.write_reg(inst.rd, self.read_reg(inst.rs1) ^ self.read_reg(inst.rs2));
            }

            (RiscvOpcode::OP, 0b110, 0b0000000) => {
                // OR
                self.write_reg(inst.rd, self.read_reg(inst.rs1) | self.read_reg(inst.rs2));
            }

            (RiscvOpcode::OP, 0b111, 0b0000000) => {
                // AND
                self.write_reg(inst.rd, self.read_reg(inst.rs1) & self.read_reg(inst.rs2));
            }

            //SLL, SRL, and SRA perform logical left, logical right, and arithmetic right shifts on the value in
            //register rs1 by the shift amount (...) of register rs2 (RISC-V Volume I, section 2.4)

            //SLL, SRL, and SRA perform logical left, logical right, and arithmetic right shifts on the value
            //in register rs1 by the shift amount held in register rs2. In RV64I, only the low 6 bits of rs2 are
            //considered for the shift amount. (RISC-V Volume I, section 5.2)
            (RiscvOpcode::OP, 0b001, 0b0000000) => {
                // SLL
                let shamt_mask = 0b11_1111;
                self.write_reg(
                    inst.rd,
                    self.read_reg(inst.rs1) << (self.read_reg(inst.rs2) & shamt_mask),
                );
            }

            (RiscvOpcode::OP, 0b101, 0b0000000) => {
                // SRL
                let shamt_mask = 0b11_1111;
                self.write_reg(
                    inst.rd,
                    self.read_reg(inst.rs1) >> (self.read_reg(inst.rs2) & shamt_mask),
                );
            }

            (RiscvOpcode::OP, 0b101, 0b0100000) => {
                // SRA
                let shamt_mask = 0b11_1111;
                self.write_reg(
                    inst.rd,
                    ((self.read_reg(inst.rs1) as i64) >> (self.read_reg(inst.rs2) & shamt_mask))
                        as u64,
                );
            }

            // SLLW, SRLW, and SRAW are RV64I-only instructions that are analogously defined but operate
            // on 32-bit values and produce signed 32-bit results. The shift amount is given by rs2[4:0] (Risc-V Volume I, section 5.2)
            (RiscvOpcode::OP32, 0b001, 0b0000000) => {
                // SLLW
                let shamt_mask = 0b1_1111;
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        (self.read_reg(inst.rs1) as u32)
                            << ((self.read_reg(inst.rs2) as u32) & shamt_mask),
                    ),
                );
            }

            (RiscvOpcode::OP32, 0b101, 0b0000000) => {
                // SRLW
                let shamt_mask = 0b1_1111;
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        (self.read_reg(inst.rs1) as u32)
                            >> ((self.read_reg(inst.rs2) as u32) & shamt_mask),
                    ),
                );
            }

            (RiscvOpcode::OP32, 0b101, 0b0100000) => {
                // SRAW
                let shamt_mask = 0b1_1111;
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        ((self.read_reg(inst.rs1) as i32)
                            >> ((self.read_reg(inst.rs2) as u32) & shamt_mask))
                            as u32,
                    ),
                );
            }

            // M-extension code
            // -----------------------------------------------------------------------------------------------------------------------------------------------
            (RiscvOpcode::OP, 0b000, 0b0000001) => {
                // MUL
                // MUL performs an XLEN-bit×XLEN-bit multiplication of rs1 by rs2 and places the lower XLEN bits
                // in the destination register (RSIC-V Volume I, section 7.1)
                self.write_reg(
                    inst.rd,
                    self.read_reg(inst.rs1)
                        .wrapping_mul(self.read_reg(inst.rs2)),
                );
            }

            // MULH, MULHU, and MULHSU perform the same multiplication but re-
            // turn the upper XLEN bits of the full 2×XLEN-bit product, for signed×signed, unsigned×unsigned,
            // and signed rs1×unsigned rs2 multiplication, respectively. (RSIC-V Volume I, section 7.1)
            (RiscvOpcode::OP, 0b001, 0b0000001) => {
                // MULH
                let res = sign_extend::<u64, u128>(self.read_reg(inst.rs1))
                    .wrapping_mul(sign_extend::<u64, u128>(self.read_reg(inst.rs2)));
                self.write_reg(inst.rd, (res >> 64) as u64);
            }

            (RiscvOpcode::OP, 0b011, 0b0000001) => {
                // MULHU
                let res =
                    (self.read_reg(inst.rs1) as u128).wrapping_mul(self.read_reg(inst.rs2) as u128);
                self.write_reg(inst.rd, (res >> 64) as u64);
            }

            (RiscvOpcode::OP, 0b010, 0b0000001) => {
                // MULHSU
                let res = sign_extend::<u64, u128>(self.read_reg(inst.rs1))
                    .wrapping_mul(self.read_reg(inst.rs2) as u128);
                self.write_reg(inst.rd, (res >> 64) as u64);
            }

            (RiscvOpcode::OP32, 0b000, 0b0000001) => {
                // MULW
                // MULW is an RV64 instruction that multiplies the lower 32 bits of the source registers, placing the
                // sign-extension of the lower 32 bits of the result into the destination register.
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        (self.read_reg(inst.rs1) as u32)
                            .wrapping_mul(self.read_reg(inst.rs2) as u32),
                    ),
                );
            }

            // DIV and DIVU perform an XLEN bits by XLEN bits signed and unsigned integer division of rs1 by
            // rs2, rounding towards zero
            (RiscvOpcode::OP, 0b100, 0b0000001) => {
                // DIV
                self.write_reg(
                    inst.rd,
                    (self.read_reg(inst.rs1) as i64).wrapping_div(self.read_reg(inst.rs2) as i64)
                        as u64,
                );
            }

            (RiscvOpcode::OP, 0b101, 0b0000001) => {
                // DIVU
                self.write_reg(
                    inst.rd,
                    (self.read_reg(inst.rs1) as u64).wrapping_div(self.read_reg(inst.rs2) as u64),
                );
            }

            (RiscvOpcode::OP, 0b110, 0b0000001) => {
                // REM
                self.write_reg(
                    inst.rd,
                    (self.read_reg(inst.rs1) as i64).wrapping_rem(self.read_reg(inst.rs2) as i64)
                        as u64,
                );
            }

            (RiscvOpcode::OP, 0b111, 0b0000001) => {
                // REMU
                self.write_reg(
                    inst.rd,
                    (self.read_reg(inst.rs1) as u64).wrapping_rem(self.read_reg(inst.rs2) as u64),
                );
            }

            // DIVW and DIVUW are RV64 instructions that divide the lower 32 bits of rs1 by the lower 32
            // bits of rs2, treating them as signed and unsigned integers respectively, placing the 32-bit quotient
            // in rd, sign-extended to 64 bits. REMW and REMUW are RV64 instructions that provide the
            // corresponding signed and unsigned remainder operations respectively. Both REMW and REMUW
            // always sign-extend the 32-bit result to 64 bits, including on a divide by zero.
            (RiscvOpcode::OP32, 0b100, 0b0000001) => {
                // DIVW
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        (self.read_reg(inst.rs1) as i32)
                            .wrapping_div(self.read_reg(inst.rs2) as i32)
                            as u32,
                    ),
                );
            }

            (RiscvOpcode::OP32, 0b101, 0b0000001) => {
                // DIVUW
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        (self.read_reg(inst.rs1) as u32)
                            .wrapping_div(self.read_reg(inst.rs2) as u32),
                    ),
                );
            }

            (RiscvOpcode::OP32, 0b110, 0b0000001) => {
                // REMW
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        (self.read_reg(inst.rs1) as i32)
                            .wrapping_rem(self.read_reg(inst.rs2) as i32)
                            as u32,
                    ),
                );
            }

            (RiscvOpcode::OP32, 0b111, 0b0000001) => {
                // REMUW
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        (self.read_reg(inst.rs1) as u32)
                            .wrapping_rem(self.read_reg(inst.rs2) as u32),
                    ),
                );
            }

            // -----------------------------------------------------------------------------------------------------------------------------------------------
            // M-extension code
            _ => (),
        }
    }

    fn execute_itype_inst(&mut self, inst: RiscvITypeInstruction, inst_size: u64) {
        match (inst.opcode, inst.funct3) {
            (RiscvOpcode::OPIMM, 0b000) => {
                // ADDI
                // ADDI adds the sign-extended 12-bit immediate to register rs1. Arithmetic overflow is ignored and
                // the result is simply the low XLEN bits of the result. (RISC-V Volume I, section 2.4)
                self.write_reg(
                    inst.rd,
                    self.read_reg(inst.rs1)
                        .wrapping_add(sign_extend(inst.parse_imm())),
                );
            }

            (RiscvOpcode::OPIMM32, 0b000) => {
                // ADDIW
                // ADDIW is an RV64I instruction that adds the sign-extended 12-bit immediate to register rs1
                // and produces the proper sign-extension of a 32-bit result in rd. Overflows are ignored and the
                // result is the low 32 bits of the result sign-extended to 64 bits. (RSIC-V Volume I, section 5.2)
                self.write_reg(
                    inst.rd,
                    sign_extend((self.read_reg(inst.rs1) as u32).wrapping_add(inst.parse_imm())),
                );
            }

            (RiscvOpcode::OPIMM, 0b010) => {
                // SLTI
                // SLTI (set less than immediate) places the value 1 in register rd if register rs1 is less than the sign-
                // extended immediate when both are treated as signed numbers, else 0 is written to rd. (RISC-V Volume I, section 2.4)
                let signed_rs1 = self.read_reg(inst.rs1) as i64;
                let signed_imm = sign_extend_to_u64(inst.parse_imm()) as i64;
                self.write_reg(inst.rd, if signed_rs1 < signed_imm { 1 } else { 0 });
            }

            (RiscvOpcode::OPIMM, 0b011) => {
                // SLTIU
                // SLTIU is similar but compares the values as unsigned numbers (i.e., the immediate is first sign-extended to
                // XLEN bits then treated as an unsigned number). Note, SLTIU rd, rs1, 1 sets rd to 1 if rs1 equals
                // zero, otherwise sets rd to 0. (RISC-V Volume I, section 2.4)
                let unsigned_rs1 = self.read_reg(inst.rs1);
                let unsigned_imm = sign_extend_to_u64(inst.parse_imm());
                self.write_reg(inst.rd, if unsigned_rs1 < unsigned_imm { 1 } else { 0 });
            }

            // ANDI, ORI, XORI are logical operations that perform bitwise AND, OR, and XOR on register rs1
            // and the sign-extended 12-bit immediate and place the result in rd. (RISC-V Volume I, section 2.4)
            (RiscvOpcode::OPIMM, 0b100) => {
                // XORI
                self.write_reg(
                    inst.rd,
                    self.read_reg(inst.rs1) ^ sign_extend_to_u64(inst.parse_imm()),
                );
            }

            (RiscvOpcode::OPIMM, 0b110) => {
                // ORI
                self.write_reg(
                    inst.rd,
                    self.read_reg(inst.rs1) | sign_extend_to_u64(inst.parse_imm()),
                );
            }

            (RiscvOpcode::OPIMM, 0b111) => {
                // ANDI
                self.write_reg(
                    inst.rd,
                    self.read_reg(inst.rs1) & sign_extend_to_u64(inst.parse_imm()),
                );
            }

            // The operand to be shifted is in rs1, and the shift amount is encoded in the lower
            // 6 bits of the I-immediate field for RV64I (RISC-V Volume I, section 5.2)
            (RiscvOpcode::OPIMM, 0b001) => {
                // SLLI
                // SLLI is a logical left shift (zeros are shifted into the lower bits)
                let shamt_mask = 0b11_1111;
                self.write_reg(
                    inst.rd,
                    self.read_reg(inst.rs1) << (sign_extend_to_u64(inst.parse_imm()) & shamt_mask),
                );
            }

            (RiscvOpcode::OPIMM, 0b101) => {
                // SRLI/SRAI, depending on immediate
                // SRLI is a logical right shift (zeros are shifted into the upper bits); and SRAI is an arithmetic right
                // shift (the original sign bit is copied into the vacated upper bits).
                let shamt_mask = 0b11_1111;
                let imm = sign_extend_to_u64(inst.parse_imm());
                if imm & (!shamt_mask) != 0 {
                    // SRAI
                    // *** Arithmetic right shift on signed integer types, logical right shift on unsigned integer types. The Rust Referance, section 8.2.4
                    self.write_reg(
                        inst.rd,
                        ((self.read_reg(inst.rs1) as i64) >> (imm & shamt_mask)) as u64,
                    );
                } else {
                    // SRLI
                    self.write_reg(inst.rd, self.read_reg(inst.rs1) >> (imm & shamt_mask));
                }
            }

            // SLLIW, SRLIW, and SRAIW are RV64I-only instructions that are analogously defined but operate
            // on 32-bit values and produce signed 32-bit results. SLLIW, SRLIW, and SRAIW encodings with
            // imm[5] ̸ = 0 are reserved.
            (RiscvOpcode::OPIMM32, 0b001) => {
                // SLLIW
                // SLLIW is a logical left shift (zeros are shifted into the lower bits)
                let shamt_mask = 0b1_1111;
                self.write_reg(
                    inst.rd,
                    sign_extend(
                        (self.read_reg(inst.rs1) as u32) << (inst.parse_imm() & shamt_mask),
                    ),
                );
            }

            (RiscvOpcode::OPIMM32, 0b101) => {
                // SRLIW/SRAIW, depending on immediate
                // SRLIW is a logical right shift (zeros are shifted into the upper bits); and SRAIW is an arithmetic right
                // shift (the original sign bit is copied into the vacated upper bits).
                let shamt_mask = 0b1_1111;
                let imm = inst.parse_imm();
                if imm & (!shamt_mask) != 0 {
                    // SRAIW
                    // *** Arithmetic right shift on signed integer types, logical right shift on unsigned integer types. The Rust Referance, section 8.2.4
                    self.write_reg(
                        inst.rd,
                        sign_extend(
                            ((self.read_reg(inst.rs1) as i32) >> (imm & shamt_mask)) as u32,
                        ),
                    );
                } else {
                    // SRLIW
                    self.write_reg(
                        inst.rd,
                        sign_extend((self.read_reg(inst.rs1) as u32) >> (imm & shamt_mask)),
                    );
                }
            }

            (RiscvOpcode::JALR, 0b000) => {
                // JALR
                // The indirect jump instruction JALR (jump and link register) uses the I-type encoding. The target
                // address is obtained by adding the sign-extended 12-bit I-immediate to the register rs1, then setting
                // the least-significant bit of the result to zero. The address of the instruction following the jump
                // (pc+4) is written to register rd. Register x0 can be used as the destination if the result is not
                // required.
                self.write_reg(inst.rd, self.program_counter + inst_size); // For C(compressed) instructions, because we exapnd them to full instructions
                let new_program_counter = sign_extend_to_u64(inst.parse_imm())
                    .wrapping_add(self.read_reg(inst.rs1))
                    & (!0b1);
                // if new_program_counter == self.program_counter {
                //     use core::fmt::Write;
                //     writeln!(UART.lock(), "Detected infinite loop, halting cpu!").unwrap();
                //     self.halted = true;
                // }
                self.program_counter = new_program_counter.wrapping_sub(inst_size);
                // Subtract inst_size to counteract the pc increment in the tick function
            }

            // The effective address is obtained by adding register rs1
            // to the sign-extended 12-bit offset. Loads copy a value from memory to register rd. Stores copy the
            // value in register rs2 to memory.
            // The LW instruction loads a 32-bit value from memory into rd. LH loads a 16-bit value from memory,
            // then sign-extends to 32-bits before storing in rd. LHU loads a 16-bit value from memory but then
            // zero extends to 32-bits before storing in rd. LB and LBU are defined analogously for 8-bit values.

            // The LW instruction loads a 32-bit value from memory and sign-extends this to 64 bits before storing
            // it in register rd for RV64I. The LWU instruction, on the other hand, zero-extends the 32-bit value
            // from memory for RV64I. LH and LHU are defined analogously for 16-bit values, as are LB and
            // LBU for 8-bit values.
            (RiscvOpcode::LOAD, 0b000) => {
                // LB
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(
                    inst.rd,
                    sign_extend::<u8, u64>(self.memory.read_u8_ne(addr)),
                );
            }

            (RiscvOpcode::LOAD, 0b001) => {
                // LH
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(
                    inst.rd,
                    sign_extend::<u16, u64>(self.memory.read_u16_ne(addr)),
                );
            }

            (RiscvOpcode::LOAD, 0b010) => {
                // LW
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(
                    inst.rd,
                    sign_extend::<u32, u64>(self.memory.read_u32_ne(addr)),
                );
            }

            (RiscvOpcode::LOAD, 0b011) => {
                // LD
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, self.memory.read_u64_ne(addr));
            }

            (RiscvOpcode::LOAD, 0b110) => {
                // LWU
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, u64::from(self.memory.read_u32_ne(addr)));
            }

            (RiscvOpcode::LOAD, 0b101) => {
                // LHU
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, u64::from(self.memory.read_u16_ne(addr)));
            }

            (RiscvOpcode::LOAD, 0b100) => {
                // LBU
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, u64::from(self.memory.read_u8_ne(addr)));
            }

            (RiscvOpcode::SYSTEM, _) => {
                if inst.parse_imm() == 0 {
                    // ECALL
                    (self.syscall)(self)
                }
            }
            _ => (),
        }
    }

    fn execute_stype_inst(&mut self, inst: RiscvSTypeInstruction) {
        match (inst.opcode, inst.funct3) {
            // The effective address is obtained by adding register rs1
            // to the sign-extended 12-bit offset. Loads copy a value from memory to register rd. Stores copy the
            // value in register rs2 to memory.
            // The SW, SH, and SB instructions store 32-bit, 16-bit, and 8-bit values from the low bits of register
            // rs2 to memory (RISC-V Volume I, section 2.6)
            (RiscvOpcode::STORE, 0b000) => {
                // SB
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.memory.write_u8_ne(addr, self.read_reg(inst.rs2) as u8)
            }

            (RiscvOpcode::STORE, 0b001) => {
                // SH
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.memory
                    .write_u16_ne(addr, self.read_reg(inst.rs2) as u16)
            }

            (RiscvOpcode::STORE, 0b010) => {
                // SW
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.memory
                    .write_u32_ne(addr, self.read_reg(inst.rs2) as u32)
            }

            (RiscvOpcode::STORE, 0b011) => {
                // SD
                let addr = self
                    .read_reg(inst.rs1)
                    .wrapping_add(sign_extend(inst.parse_imm()));
                self.memory.write_u64_ne(addr, self.read_reg(inst.rs2))
            }
            _ => (),
        }
    }

    fn execute_btype_inst(&mut self, inst: RiscvBTypeInstruction, inst_size: u64) {
        match (inst.opcode, inst.funct3) {
            // All branch instructions use the B-type instruction format. The 12-bit B-immediate encodes signed
            // offsets in multiples of 2 bytes. The offset is sign-extended and added to the address of the branch
            // instruction to give the target address. The conditional branch range is ±4 KiB.

            // Branch instructions compare two registers. BEQ and BNE take the branch if registers rs1 and rs2
            // are equal or unequal respectively. BLT and BLTU take the branch if rs1 is less than rs2, using
            // signed and unsigned comparison respectively. BGE and BGEU take the branch if rs1 is greater
            // than or equal to rs2, using signed and unsigned comparison respectively.
            (RiscvOpcode::BRANCH, 0b000) => {
                // BEQ
                if self.read_reg(inst.rs1) == self.read_reg(inst.rs2) {
                    self.program_counter = self
                        .program_counter
                        .wrapping_add(sign_extend_to_u64(inst.parse_imm()))
                        .wrapping_sub(inst_size); // Subtract inst_size to counteract the pc increment in the tick function
                }
            }

            (RiscvOpcode::BRANCH, 0b001) => {
                // BNE
                if self.read_reg(inst.rs1) != self.read_reg(inst.rs2) {
                    self.program_counter = self
                        .program_counter
                        .wrapping_add(sign_extend_to_u64(inst.parse_imm()))
                        .wrapping_sub(inst_size); // Subtract inst_size to counteract the pc increment in the tick function
                }
            }

            (RiscvOpcode::BRANCH, 0b100) => {
                // BLT
                if (self.read_reg(inst.rs1) as i64) < (self.read_reg(inst.rs2) as i64) {
                    self.program_counter = self
                        .program_counter
                        .wrapping_add(sign_extend_to_u64(inst.parse_imm()))
                        .wrapping_sub(inst_size); // Subtract inst_size to counteract the pc increment in the tick function
                }
            }

            (RiscvOpcode::BRANCH, 0b101) => {
                // BGE
                if (self.read_reg(inst.rs1) as i64) >= (self.read_reg(inst.rs2) as i64) {
                    self.program_counter = self
                        .program_counter
                        .wrapping_add(sign_extend_to_u64(inst.parse_imm()))
                        .wrapping_sub(inst_size); // Subtract inst_size to counteract the pc increment in the tick function
                }
            }

            (RiscvOpcode::BRANCH, 0b110) => {
                // BLTU
                if self.read_reg(inst.rs1) < self.read_reg(inst.rs2) {
                    self.program_counter = self
                        .program_counter
                        .wrapping_add(sign_extend_to_u64(inst.parse_imm()))
                        .wrapping_sub(inst_size); // Subtract inst_size to counteract the pc increment in the tick function
                }
            }

            (RiscvOpcode::BRANCH, 0b111) => {
                // BGEU
                if self.read_reg(inst.rs1) >= self.read_reg(inst.rs2) {
                    self.program_counter = self
                        .program_counter
                        .wrapping_add(sign_extend_to_u64(inst.parse_imm()))
                        .wrapping_sub(inst_size); // Subtract inst_size to counteract the pc increment in the tick function
                }
            }
            _ => (),
        }
    }

    fn execute_utype_inst(&mut self, inst: RiscvUTypeInstruction) {
        match inst.opcode {
            RiscvOpcode::LUI => {
                self.write_reg(inst.rd, sign_extend_to_u64(inst.parse_imm()));
            }

            RiscvOpcode::AUIPC => {
                self.write_reg(
                    inst.rd,
                    sign_extend_to_u64(inst.parse_imm()).wrapping_add(self.program_counter),
                );
            }
            _ => (),
        }
    }

    fn execute_jtype_inst(&mut self, inst: RiscvJTypeInstruction, inst_size: u64) {
        match inst.opcode {
            RiscvOpcode::JAL => {
                // The jump and link (JAL) instruction uses the J-type format, where the J-immediate encodes a
                // signed offset in multiples of 2 bytes. The offset is sign-extended and added to the address of the
                // jump instruction to form the jump target address. Jumps can therefore target a ±1 MiB range.
                // JAL stores the address of the instruction following the jump (pc+4) into register rd (RISC-V Volume I, section 2.5)
                self.write_reg(inst.rd, self.program_counter + inst_size); // For C(compressed) instructions, because we expand them to full instructions
                let new_program_counter =
                    sign_extend_to_u64(inst.parse_imm()).wrapping_add(self.program_counter);
                // if new_program_counter == self.program_counter {
                //     use core::fmt::Write;
                //     writeln!(UART.lock(), "Detected infinite loop, halting cpu!").unwrap();
                //     self.halted = true;
                // }
                self.program_counter = new_program_counter.wrapping_sub(inst_size);
                // Subtract inst_size to counteract the pc increment in the tick function
            }
            _ => (),
        }
    }
}
