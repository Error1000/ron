
use packed_struct::prelude::*;

use crate::UART;

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
    // If they didn't do this then there would be ambigous cases like: 80 45 

}

#[derive(Debug)]
pub struct Riscv64Cpu<'mem_life, MemType>
where MemType: EmulatorMemory {
    program_counter: u64,
    registers: [u64; 31],
    memory: &'mem_life mut MemType,
    halted: bool
}


mod riscv_instruction {
    use super::*;
    pub enum RiscvInstType {
        RType,
        IType,
        SType,
        BType,
        UType,
        JType
    }

    // Note: This is intended to only be 6 bits
    #[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq)]
    pub enum RiscvOpcode {
        OPIMM    = 0b0010011, // I-type
        OPIMM32  = 0b0011011, // I-type
        LUI      = 0b0110111, // U-type
        AUIPC    = 0b0010111, // U-type
        OP       = 0b0110011, // R-type
        OP32     = 0b0111011, // R-type
        JAL      = 0b1101111, // J-type
        JALR     = 0b1100111, // I-type
        BRANCH   = 0b1100011, // B-type
        LOAD     = 0b0000011, // I-type
        STORE    = 0b0100011, // S-type
        MISCMEM  = 0b0001111, // I-type
        SYSTEM   = 0b1110011  // I-type
    }

    impl RiscvOpcode{
        pub fn get_type(&self) -> RiscvInstType {
            match self {
                RiscvOpcode::OPIMM   => RiscvInstType::IType,
                RiscvOpcode::OPIMM32 => RiscvInstType::IType,
                RiscvOpcode::LUI     => RiscvInstType::UType,
                RiscvOpcode::AUIPC   => RiscvInstType::UType,
                RiscvOpcode::OP      => RiscvInstType::RType,
                RiscvOpcode::OP32    => RiscvInstType::RType,
                RiscvOpcode::JAL     => RiscvInstType::JType,
                RiscvOpcode::JALR    => RiscvInstType::IType,
                RiscvOpcode::BRANCH  => RiscvInstType::BType,
                RiscvOpcode::LOAD    => RiscvInstType::IType,
                RiscvOpcode::STORE   => RiscvInstType::SType,
                RiscvOpcode::MISCMEM => RiscvInstType::IType, // NOTE: Not quite I type but close enough
                RiscvOpcode::SYSTEM  => RiscvInstType::IType, // NOTE: Not quite I type but close enough
            }
        }
    }



    #[derive(PackedStruct)]
    #[packed_struct(size_bytes="4", bit_numbering="lsb0", endian="msb")]
    pub struct RiscvRTypeInstruction {
        #[packed_field(bits="0..=6", ty="enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits="7..=11")]
        pub rd: u8,
        #[packed_field(bits="12..=14")]
        pub funct3: u8,
        #[packed_field(bits="15..=19")]
        pub rs1: u8,
        #[packed_field(bits="20..=24")]
        pub rs2: u8,
        #[packed_field(bits="25..=31")]
        pub funct7: u8
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes="4", bit_numbering="lsb0", endian="msb")]
    pub struct RiscvITypeInstruction {
        #[packed_field(bits="0..=6", ty="enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits="7..=11")]
        pub rd: u8,
        #[packed_field(bits="12..=14")]
        pub funct3: u8,
        #[packed_field(bits="15..=19")]
        pub rs1: u8,
        #[packed_field(bits="20..=31")]
        imm: u16
    }

    impl RiscvITypeInstruction {
        pub fn parse_imm(&self) -> u32 {
            // 12 bits in length
            let top_bit: u32 = 1<<(12-1);
            let all_ones: u32 = (1<<12)-1;

            let non_sign_extended_imm: u32 = u32::from(u16::from(self.imm));
            let sign = (non_sign_extended_imm & top_bit) != 0;
            if sign {
                return non_sign_extended_imm | (!all_ones); // Make sure the top bits are one
            } else {
                // NOTE: This should be a no-op, because it should zero-extened anyway, but for clarity i added it anyway
                return non_sign_extended_imm  &  all_ones; // Make sure the top bits are zero
            }
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes="4", bit_numbering="lsb0", endian="msb")]
    pub struct RiscvSTypeInstruction {
        #[packed_field(bits="0..=6", ty="enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits="7..=11")]
        imm_low5: u8,
        #[packed_field(bits="12..=14")]
        pub funct3: u8,
        #[packed_field(bits="15..=19")]
        pub rs1: u8,
        #[packed_field(bits="20..=24")]
        pub rs2: u8,
        #[packed_field(bits="25..=31")]
        imm_hi7: u8,
    }

    impl RiscvSTypeInstruction {
        pub fn parse_imm(&self) -> u32 {
            // 12 bits in length
            let top_bit: u32 = 1<<(12-1);
            let all_ones: u32 = (1<<12)-1;

            let non_sign_extended_imm: u32 = u32::from(u8::from(self.imm_low5)) | u32::from(u8::from(self.imm_hi7)) << 5;
            let sign = (non_sign_extended_imm & top_bit) != 0;
            if sign {
                return non_sign_extended_imm | (!all_ones); // Make sure the top bits are one
            } else {
                // NOTE: This should be a no-op, because it should zero-extened anyway, but for clarity i added it anyway
                return non_sign_extended_imm  & all_ones; // Make sure the top bits are zero
            }
        }
    }


    #[derive(PackedStruct)]
    #[packed_struct(size_bytes="4", bit_numbering="lsb0", endian="msb")]
    pub struct RiscvBTypeInstruction {
        #[packed_field(bits="0..=6", ty="enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits="7")]
        imm_bit11: u8,
        #[packed_field(bits="8..=11")]
        imm_bits_4_1: u8,
        #[packed_field(bits="12..=14")]
        pub funct3: u8,
        #[packed_field(bits="15..=19")]
        pub rs1: u8,
        #[packed_field(bits="20..=24")]
        pub rs2: u8,
        #[packed_field(bits="25..=30")]
        imm_bits_10_5: u8,
        #[packed_field(bits="31")]
        imm_bit12: u8
    }

    impl RiscvBTypeInstruction {
        pub fn parse_imm(&self) -> u32 {
            // 13 bits in length, as we start at bit 0
            let top_bit: u32 = 1<<(13-1);
            let all_ones: u32 = (1<<13)-1;

            let non_sign_extended_imm: u32 = u32::from(u8::from(self.imm_bits_4_1)) << 1 | u32::from(u8::from(self.imm_bits_10_5)) << 5 | u32::from(u8::from(self.imm_bit11)) << 11 | u32::from(u8::from(self.imm_bit12)) << 12;
            let sign = (non_sign_extended_imm & top_bit) != 0;
            if sign {
                return non_sign_extended_imm | (!all_ones); // Make sure the top bits are one
            } else {
                // NOTE: This should be a no-op, because it should zero-extened anyway, but for clarity i added it anyway
                return non_sign_extended_imm  & all_ones; // Make sure the top bits are zero
            }
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes="4", bit_numbering="lsb0", endian="msb")]
    pub struct RiscvUTypeInstruction {
        #[packed_field(bits="0..=6", ty="enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits="7..=11")]
        pub rd: u8,
        #[packed_field(bits="12..=31")]
        imm: u32
    }

    impl RiscvUTypeInstruction {
        pub fn parse_imm(&self) -> u32 {
            u32::from(self.imm) << 12
        }
    }

    #[derive(PackedStruct)]
    #[packed_struct(size_bytes="4", bit_numbering="lsb0", endian="msb")]
    pub struct RiscvJTypeInstruction {
        #[packed_field(bits="0..=6", ty="enum")]
        pub opcode: RiscvOpcode,
        #[packed_field(bits="7..=11")]
        pub rd: u8,
        #[packed_field(bits="12..=19")]
        imm_bits_19_12: u8,
        #[packed_field(bits="20")]
        imm_bit11: u8,
        #[packed_field(bits="21..=30")]
        imm_bits_10_1: u16,
        #[packed_field(bits="31")]
        imm_bit20: u8,
    }

    impl RiscvJTypeInstruction {
        pub fn parse_imm(&self) -> u32 {
            // 21 bits in length, as we start at bit 0
            let top_bit: u32 = 1<<(21-1);
            let all_ones: u32 = (1<<21)-1;

            let non_sign_extended_imm: u32 = u32::from(u16::from(self.imm_bits_10_1)) << 1 | u32::from(u8::from(self.imm_bit11)) << 11 | u32::from(u8::from(self.imm_bits_19_12)) << 12 | u32::from(u8::from(self.imm_bit20)) << 20;
            let sign = (non_sign_extended_imm & top_bit) != 0;
            if sign {
                return non_sign_extended_imm | (!all_ones); // Make sure the top bits are one
            } else {
                // NOTE: This should be a no-op, because it should zero-extened anyway, but for clarity i added it anyway
                return non_sign_extended_imm &  all_ones; // Make sure the top bits are zero
            }
        }
    }

}

use riscv_instruction::*;

// Note: T and U must be less than 64-bits
fn sign_extend<T, U>(val: T) -> U 
where U: From<u64>,
      u64: From<T> {
    let val = u64::from(val);

    let max_shift_amount = core::mem::size_of::<T>()*8 - 1;
    let top_bit: u64 = 1u64 << max_shift_amount;
    let all_ones: u64  = ((1u64 << max_shift_amount) - 1) | top_bit; // Designed like this to avoid overflow when T == u64

    let sign = (val & top_bit) != 0;
    if sign {
        U::from(val | (!all_ones))
    } else {
        U::from(val &   all_ones) /* NOTE: This should be a no-op, because it should zero-extened anyway, but for clarity i added it anyway */
    }
}

fn sign_extend_to_u64<T>(val: T) -> u64
    where u64: From<T> {
    sign_extend::<T, u64>(val)
}

impl<MemType> Riscv64Cpu<'_, MemType>
where MemType: EmulatorMemory {

    // TODO:
    // ADD extensions
    // ADD syscall/trap instructions
    
    pub fn from<'mem>(mem: &'mem mut MemType, start_address: u64) -> Riscv64Cpu<'mem, MemType> {
        Riscv64Cpu {
            program_counter: start_address,
            registers: [0u64; 31],
            memory: mem,
            halted: false
        }
    }

    fn write_reg(&mut self, reg_n: u8, val: u64) {
        if reg_n != 0 { self.registers[usize::from(reg_n-1)] = val; }
    }

    fn read_reg(&self, reg_n: u8) -> u64 {
        if reg_n == 0 { 0 } else { self.registers[usize::from(reg_n-1)] }
    }

    // Run one clock cycle
    // Note: Returns None when ticking fails ( for example maybe instruction parsing failed, or maybe the cpu is halted )
    pub fn tick(&mut self) -> Option<()> {
        if self.halted { return None; }
        let instruction = self.memory.read_u32_le(self.program_counter);
        // NOTE: I would expect the output to be [147, 0, 0, 1], since the struct is marked as little-endian, but it is [1, 0, 0, 147], that's because the byte array is always big-endian and the little-endian marker onyl applies to each field not to the endiannes of the byte array produced
        // Referance: Issue #92, https://github.com/hashmismatch/packed_struct.rs/issues/92
        // So therefore i am insteada using big endian for parsing instructions
        let opcode: RiscvOpcode = RiscvOpcode::from_primitive((instruction & 0b111_1111) as u8)?;
        match opcode.get_type() {
            RiscvInstType::RType => self.execute_rtype_inst(RiscvRTypeInstruction::unpack(&instruction.to_be_bytes()).unwrap()),
            RiscvInstType::IType => self.execute_itype_inst(RiscvITypeInstruction::unpack(&instruction.to_be_bytes()).unwrap()),
            RiscvInstType::SType => self.execute_stype_inst(RiscvSTypeInstruction::unpack(&instruction.to_be_bytes()).unwrap()),
            RiscvInstType::BType => self.execute_btype_inst(RiscvBTypeInstruction::unpack(&instruction.to_be_bytes()).unwrap()),
            RiscvInstType::UType => self.execute_utype_inst(RiscvUTypeInstruction::unpack(&instruction.to_be_bytes()).unwrap()),
            RiscvInstType::JType => self.execute_jtype_inst(RiscvJTypeInstruction::unpack(&instruction.to_be_bytes()).unwrap()),
        }
        self.program_counter += core::mem::size_of::<u32>() as u64; 

        Some(())
    }

    fn execute_rtype_inst(&mut self, inst: RiscvRTypeInstruction) {
        match (inst.opcode, inst.funct3, inst.funct7) {
            // ADD performs the addition of rs1 and rs2. SUB performs the subtraction of rs2 from rs1. Overflows
            // are ignored and the low XLEN bits of results are written to the destination rd. (RISC-V Volume I, section 2.4)

            (RiscvOpcode::OP, 0b000, 0b0000000) => { // ADD
                self.write_reg(inst.rd, self.read_reg(inst.rs1).wrapping_add(self.read_reg(inst.rs2)));
            },

            (RiscvOpcode::OP, 0b000, 0b0100000) => { // SUB
                self.write_reg(inst.rd, self.read_reg(inst.rs1).wrapping_sub(self.read_reg(inst.rs2)));
            },


            // ADDW and SUBW are RV64I-only instructions that are defined analogously to ADD and SUB
            // but operate on 32-bit values and produce signed 32-bit results. Overflows are ignored, and the low
            // 32-bits of the result is sign-extended to 64-bits and written to the destination register (RISC-V Volume I, section 5.2)
            (RiscvOpcode::OP32, 0b000, 0b0000000) => { // ADDW
                self.write_reg(inst.rd, sign_extend((self.read_reg(inst.rs1) as u32).wrapping_add(self.read_reg(inst.rs2) as u32)));
            },

            (RiscvOpcode::OP32, 0b000, 0b0100000) => { // SUBW
                self.write_reg(inst.rd, sign_extend((self.read_reg(inst.rs1) as u32).wrapping_sub(self.read_reg(inst.rs2) as u32)));
            },



            // SLT and SLTU perform signed and unsigned compares respectively, writing 1 to rd if rs1 < rs2, 0 otherwise (RISC-V Volume I, section 2.4)
            (RiscvOpcode::OP, 0b010, 0b0000000) => { // SLT
                let signed_rs1 = self.read_reg(inst.rs1) as i64;
                let signed_rs2 = self.read_reg(inst.rs2) as i64;
                self.write_reg(inst.rd, if signed_rs1 < signed_rs2 { 1 } else { 0 });
            },

            (RiscvOpcode::OP, 0b011, 0b0000000) => { // SLTU
                let unsigned_rs1 = self.read_reg(inst.rs1);
                let unsigned_rs2 = self.read_reg(inst.rs2);
                self.write_reg(inst.rd, if unsigned_rs1 < unsigned_rs2 { 1 } else { 0 });
            },  

            (RiscvOpcode::OP, 0b100, 0b0000000) => { // XOR
                self.write_reg(inst.rd, self.read_reg(inst.rs1) ^ self.read_reg(inst.rs2));
            },

            (RiscvOpcode::OP, 0b110, 0b0000000) => { // OR
                self.write_reg(inst.rd, self.read_reg(inst.rs1) | self.read_reg(inst.rs2));
            },

            (RiscvOpcode::OP, 0b111, 0b0000000) => { // AND
                self.write_reg(inst.rd, self.read_reg(inst.rs1) & self.read_reg(inst.rs2));
            },

            //SLL, SRL, and SRA perform logical left, logical right, and arithmetic right shifts on the value in
            //register rs1 by the shift amount (...) of register rs2 (RISC-V Volume I, section 2.4)

            //SLL, SRL, and SRA perform logical left, logical right, and arithmetic right shifts on the value
            //in register rs1 by the shift amount held in register rs2. In RV64I, only the low 6 bits of rs2 are
            //considered for the shift amount. (RISC-V Volume I, section 5.2)

            (RiscvOpcode::OP, 0b001, 0b0000000) => { // SLL
                let shamt_mask = 0b11_1111;
                self.write_reg(inst.rd, self.read_reg(inst.rs1) << (self.read_reg(inst.rs2) & shamt_mask));
            },

            (RiscvOpcode::OP, 0b101, 0b0000000) => { // SRL
                let shamt_mask = 0b11_1111;
                self.write_reg(inst.rd, self.read_reg(inst.rs1) >> (self.read_reg(inst.rs2) & shamt_mask));
            },
            
            (RiscvOpcode::OP, 0b101, 0b0100000) => { // SRA
                let shamt_mask = 0b11_1111;
                self.write_reg(inst.rd, ((self.read_reg(inst.rs1) as i64) >> (self.read_reg(inst.rs2) & shamt_mask)) as u64);
            },


            // SLLW, SRLW, and SRAW are RV64I-only instructions that are analogously defined but operate
            // on 32-bit values and produce signed 32-bit results. The shift amount is given by rs2[4:0] (Risc-V Volume I, section 5.2)

            (RiscvOpcode::OP32, 0b001, 0b0000000) => { // SLLW
                let shamt_mask = 0b1_1111;
                self.write_reg(inst.rd, sign_extend((self.read_reg(inst.rs1) as u32) << ((self.read_reg(inst.rs2) as u32) & shamt_mask)));
            },

            (RiscvOpcode::OP32, 0b101, 0b0000000) => { // SRLW
                let shamt_mask = 0b1_1111;
                self.write_reg(inst.rd, sign_extend((self.read_reg(inst.rs1) as u32) >> ((self.read_reg(inst.rs2) as u32) & shamt_mask)));
            },
            
            (RiscvOpcode::OP32, 0b101, 0b0100000) => { // SRAW
                let shamt_mask = 0b1_1111;
                self.write_reg(inst.rd, sign_extend(((self.read_reg(inst.rs1) as i32) >> ((self.read_reg(inst.rs2) as u32) & shamt_mask)) as u32));
            },

            _ => ()
        }
    }

    fn execute_itype_inst(&mut self, inst: RiscvITypeInstruction) {
        match (inst.opcode, inst.funct3) {
            (RiscvOpcode::OPIMM, 0b000) => { // ADDI
                // ADDI adds the sign-extended 12-bit immediate to register rs1. Arithmetic overflow is ignored and
                // the result is simply the low XLEN bits of the result. (RISC-V Volume I, section 2.4)
                self.write_reg(inst.rd, self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm())));
            },

            (RiscvOpcode::OPIMM32, 0b000) => { // ADDIW
                // ADDIW is an RV64I instruction that adds the sign-extended 12-bit immediate to register rs1
                // and produces the proper sign-extension of a 32-bit result in rd. Overflows are ignored and the
                // result is the low 32 bits of the result sign-extended to 64 bits. (RSIC-V Volume I, section 5.2)
                self.write_reg(inst.rd, sign_extend((self.read_reg(inst.rs1) as u32).wrapping_add(inst.parse_imm())));
            },

            (RiscvOpcode::OPIMM, 0b010) => { // SLTI
                // SLTI (set less than immediate) places the value 1 in register rd if register rs1 is less than the sign-
                // extended immediate when both are treated as signed numbers, else 0 is written to rd. (RISC-V Volume I, section 2.4)
                let signed_rs1 = self.read_reg(inst.rs1) as i64;
                let signed_imm = sign_extend_to_u64(inst.parse_imm()) as i64;
                self.write_reg(inst.rd, if signed_rs1 < signed_imm { 1 } else { 0 });
            },
     
            (RiscvOpcode::OPIMM, 0b011) => { // SLTIU
                // SLTIU is similar but compares the values as unsigned numbers (i.e., the immediate is first sign-extended to
                // XLEN bits then treated as an unsigned number). Note, SLTIU rd, rs1, 1 sets rd to 1 if rs1 equals
                // zero, otherwise sets rd to 0. (RISC-V Volume I, section 2.4)
                let unsigned_rs1 = self.read_reg(inst.rs1);
                let unsigned_imm = sign_extend_to_u64(inst.parse_imm());
                self.write_reg(inst.rd, if unsigned_rs1 < unsigned_imm { 1 } else { 0 });
            },


            // ANDI, ORI, XORI are logical operations that perform bitwise AND, OR, and XOR on register rs1
            // and the sign-extended 12-bit immediate and place the result in rd. (RISC-V Volume I, section 2.4)

            (RiscvOpcode::OPIMM, 0b100) => { // XORI
                self.write_reg(inst.rd, self.read_reg(inst.rs1) ^ sign_extend_to_u64(inst.parse_imm()));
            },

            (RiscvOpcode::OPIMM, 0b110) => { // ORI
                self.write_reg(inst.rd, self.read_reg(inst.rs1) | sign_extend_to_u64(inst.parse_imm()));
            },

            (RiscvOpcode::OPIMM, 0b111) => { // ANDI
                self.write_reg(inst.rd, self.read_reg(inst.rs1) & sign_extend_to_u64(inst.parse_imm()));
            },


            // The operand to be shifted is in rs1, and the shift amount is encoded in the lower
            // 6 bits of the I-immediate field for RV64I (RISC-V Volume I, section 5.2)
            (RiscvOpcode::OPIMM, 0b001) => { // SLLI
                // SLLI is a logical left shift (zeros are shifted into the lower bits)
                let shamt_mask = 0b11_1111;
                self.write_reg(inst.rd, self.read_reg(inst.rs1) << (sign_extend_to_u64(inst.parse_imm()) & shamt_mask));
            },

            (RiscvOpcode::OPIMM, 0b101) => { // SRLI/SRAI, depending on immediate
                // SRLI is a logical right shift (zeros are shifted into the upper bits); and SRAI is an arithmetic right
                // shift (the original sign bit is copied into the vacated upper bits).
                let shamt_mask = 0b11_1111;
                let imm = sign_extend_to_u64(inst.parse_imm());
                if imm & (!shamt_mask) != 0 { // SRAI
                    // *** Arithmetic right shift on signed integer types, logical right shift on unsigned integer types. The Rust Referance, section 8.2.4
                    self.write_reg(inst.rd, ((self.read_reg(inst.rs1) as i64) >> (imm & shamt_mask)) as u64);
                } else { // SRLI
                    self.write_reg(inst.rd, self.read_reg(inst.rs1) >> (imm & shamt_mask));
                }
            },

            // SLLIW, SRLIW, and SRAIW are RV64I-only instructions that are analogously defined but operate
            // on 32-bit values and produce signed 32-bit results. SLLIW, SRLIW, and SRAIW encodings with
            // imm[5] ̸ = 0 are reserved.
            
            (RiscvOpcode::OPIMM32, 0b001) => { // SLLIW
                // SLLIW is a logical left shift (zeros are shifted into the lower bits)
                let shamt_mask = 0b1_1111;
                self.write_reg(inst.rd, sign_extend((self.read_reg(inst.rs1) as u32) << (inst.parse_imm() & shamt_mask)));
            }

            (RiscvOpcode::OPIMM32, 0b101) => { // SRLIW/SRAIW, depending on immediate
                // SRLIW is a logical right shift (zeros are shifted into the upper bits); and SRAIW is an arithmetic right
                // shift (the original sign bit is copied into the vacated upper bits).
                let shamt_mask = 0b1_1111;
                let imm = inst.parse_imm();
                if imm & (!shamt_mask) != 0 { // SRAIW
                    // *** Arithmetic right shift on signed integer types, logical right shift on unsigned integer types. The Rust Referance, section 8.2.4
                    self.write_reg(inst.rd, sign_extend(((self.read_reg(inst.rs1) as i32) >> (imm & shamt_mask)) as u32));
                } else { // SRLIW
                    self.write_reg(inst.rd, sign_extend((self.read_reg(inst.rs1) as u32) >> (imm & shamt_mask)));
                }
            },


            (RiscvOpcode::JALR, 0b000) => { // JALR
                // The indirect jump instruction JALR (jump and link register) uses the I-type encoding. The target
                // address is obtained by adding the sign-extended 12-bit I-immediate to the register rs1, then setting
                // the least-significant bit of the result to zero. The address of the instruction following the jump
                // (pc+4) is written to register rd. Register x0 can be used as the destination if the result is not
                // required.
                self.write_reg(inst.rd, self.program_counter + 4);
                let new_program_counter = sign_extend_to_u64(inst.parse_imm()).wrapping_add(self.read_reg(inst.rs1)) & (!0b1);
                if new_program_counter == self.program_counter {
                    use core::fmt::Write;
                    writeln!(UART.lock(), "Detected infinite loop, halting cpu!").unwrap();
                    self.halted = true;
                }
                self.program_counter = new_program_counter.wrapping_sub(4); // Subtract 4 to counteract the pc increment in the tick function
            },

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

            (RiscvOpcode::LOAD, 0b000) => { // LB
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, sign_extend::<u8, u64>(self.memory.read_u8_ne(addr)));
            },
           
            (RiscvOpcode::LOAD, 0b001) => { // LH
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, sign_extend::<u16, u64>(self.memory.read_u16_ne(addr)));
            },

            (RiscvOpcode::LOAD, 0b010) => { // LW
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, sign_extend::<u32, u64>(self.memory.read_u32_ne(addr)));
            },

            (RiscvOpcode::LOAD, 0b011) => { // LD
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, self.memory.read_u64_ne(addr));
            },

            
            (RiscvOpcode::LOAD, 0b110) => { // LWU
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, u64::from(self.memory.read_u32_ne(addr)));
            },

            (RiscvOpcode::LOAD, 0b100) => { // LBU
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, u64::from(self.memory.read_u8_ne(addr)));
            },

            (RiscvOpcode::LOAD, 0b101) => { // LHU
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.write_reg(inst.rd, u64::from(self.memory.read_u16_ne(addr)));
            },

            _ => ()
        }
    }

    fn execute_stype_inst(&mut self, inst: RiscvSTypeInstruction) {
        match (inst.opcode, inst.funct3) {
            // The effective address is obtained by adding register rs1
            // to the sign-extended 12-bit offset. Loads copy a value from memory to register rd. Stores copy the
            // value in register rs2 to memory.
            // The SW, SH, and SB instructions store 32-bit, 16-bit, and 8-bit values from the low bits of register
            // rs2 to memory (RISC-V Volume I, section 2.6)
            
            (RiscvOpcode::STORE, 0b000) => { // SB
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.memory.write_u8_ne(addr, self.read_reg(inst.rs2) as u8)
            },

            (RiscvOpcode::STORE, 0b001) => { // SH
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.memory.write_u16_ne(addr, self.read_reg(inst.rs2) as u16)
            },

            (RiscvOpcode::STORE, 0b010) => { // SW
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.memory.write_u32_ne(addr, self.read_reg(inst.rs2) as u32)
            },

            (RiscvOpcode::STORE, 0b011) => { // SD
                let addr = self.read_reg(inst.rs1).wrapping_add(sign_extend(inst.parse_imm()));
                self.memory.write_u64_ne(addr, self.read_reg(inst.rs2))
            },
            _ => ()
        }
    }

    fn execute_btype_inst(&mut self, inst: RiscvBTypeInstruction) {
        match (inst.opcode, inst.funct3) {

            // All branch instructions use the B-type instruction format. The 12-bit B-immediate encodes signed
            // offsets in multiples of 2 bytes. The offset is sign-extended and added to the address of the branch
            // instruction to give the target address. The conditional branch range is ±4 KiB.
            
            // Branch instructions compare two registers. BEQ and BNE take the branch if registers rs1 and rs2
            // are equal or unequal respectively. BLT and BLTU take the branch if rs1 is less than rs2, using
            // signed and unsigned comparison respectively. BGE and BGEU take the branch if rs1 is greater
            // than or equal to rs2, using signed and unsigned comparison respectively. 

            (RiscvOpcode::BRANCH, 0b000) => { // BEQ
                if self.read_reg(inst.rs1) == self.read_reg(inst.rs2) {
                    self.program_counter = self.program_counter.wrapping_add(sign_extend_to_u64(inst.parse_imm())).wrapping_sub(4); // Subtract 4 to counteract the pc increment in the tick function
                }
            },

            (RiscvOpcode::BRANCH, 0b001) => { // BNE
                if self.read_reg(inst.rs1) != self.read_reg(inst.rs2) {
                    self.program_counter = self.program_counter.wrapping_add(sign_extend_to_u64(inst.parse_imm())).wrapping_sub(4); // Subtract 4 to counteract the pc increment in the tick function
                }
            },

            (RiscvOpcode::BRANCH, 0b100) => { // BLT
                if (self.read_reg(inst.rs1) as i64) < (self.read_reg(inst.rs2) as i64) {
                    self.program_counter = self.program_counter.wrapping_add(sign_extend_to_u64(inst.parse_imm())).wrapping_sub(4); // Subtract 4 to counteract the pc increment in the tick function
                }
            },

            (RiscvOpcode::BRANCH, 0b101) => { // BGE
                if (self.read_reg(inst.rs1) as i64) >= (self.read_reg(inst.rs2) as i64) {
                    self.program_counter = self.program_counter.wrapping_add(sign_extend_to_u64(inst.parse_imm())).wrapping_sub(4); // Subtract 4 to counteract the pc increment in the tick function
                }
            },

            (RiscvOpcode::BRANCH, 0b110) => { // BLTU
                if self.read_reg(inst.rs1) < self.read_reg(inst.rs2) {
                    self.program_counter = self.program_counter.wrapping_add(sign_extend_to_u64(inst.parse_imm())).wrapping_sub(4); // Subtract 4 to counteract the pc increment in the tick function
                }
            },

            (RiscvOpcode::BRANCH, 0b111) => { // BGEU
                if self.read_reg(inst.rs1) >= self.read_reg(inst.rs2) {
                    self.program_counter = self.program_counter.wrapping_add(sign_extend_to_u64(inst.parse_imm())).wrapping_sub(4); // Subtract 4 to counteract the pc increment in the tick function
                }
            },
            _ => ()
        }
    }

    fn execute_utype_inst(&mut self, inst: RiscvUTypeInstruction) {
        match inst.opcode {
            RiscvOpcode::LUI => {
                self.write_reg(inst.rd, sign_extend_to_u64(inst.parse_imm()));
            },

            RiscvOpcode::AUIPC => {
                self.write_reg(inst.rd, sign_extend_to_u64(inst.parse_imm()).wrapping_add(self.program_counter));
            },
            _ => ()
        }
    }

    fn execute_jtype_inst(&mut self, inst: RiscvJTypeInstruction) {
        match inst.opcode {
            RiscvOpcode::JAL => {
                // The jump and link (JAL) instruction uses the J-type format, where the J-immediate encodes a
                // signed offset in multiples of 2 bytes. The offset is sign-extended and added to the address of the
                // jump instruction to form the jump target address. Jumps can therefore target a ±1 MiB range.
                // JAL stores the address of the instruction following the jump (pc+4) into register rd (RISC-V Volume I, section 2.5)
                self.write_reg(inst.rd, self.program_counter + 4);
                let new_program_counter = sign_extend_to_u64(inst.parse_imm()).wrapping_add(self.program_counter);
                if new_program_counter == self.program_counter {
                    use core::fmt::Write;
                    writeln!(UART.lock(), "Detected infinite loop, halting cpu!").unwrap();
                    self.halted = true;
                }
                self.program_counter = new_program_counter.wrapping_sub(4); // Subtract 4 to counteract the pc increment in the tick function
            },
            _ => ()
        }
    }
}