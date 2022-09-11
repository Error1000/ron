use core::convert::TryFrom;

use rlibc::SyscallNumber;

use crate::{
    emulator::Riscv64Cpu,
    virtmem::{self, LittleEndianVirtualMemory},
    UART,
};

type Emulator = Riscv64Cpu<LittleEndianVirtualMemory>;

pub fn syscall_linux_abi_entry_point(emu: &mut Emulator) {
    // Source: man syscall
    let syscall_number = emu.read_reg(17 /* a7 */);
    let syscall_number = if let Ok(val) = SyscallNumber::try_from(syscall_number as usize) {
        val
    } else {
        use core::fmt::Write;
        let _ = writeln!(UART.lock(), "Syscall number {}, is not implemented, ignoring!", syscall_number);
        return;
    };

    // a0-a5
    let argument_1 = || emu.read_reg(10);
    let argument_2 = || emu.read_reg(11);
    let argument_3 = || emu.read_reg(12);
    let argument_4 = || emu.read_reg(13);
    let argument_5 = || emu.read_reg(14);
    let argument_6 = || emu.read_reg(15);

    let return_value = |val, emu: &mut Emulator| emu.write_reg(10, val);

    match syscall_number {
        SyscallNumber::Exit => exit(emu, argument_1() as usize),
        // SAFETY: Argument 2 comes from the program itself so it should be in its address space
        SyscallNumber::Write => {
            let val = write(
                emu,
                argument_1() as usize,
                unsafe { virtmem::UserPointer::<[u8]>::from_mem(argument_2()) },
                argument_3() as usize,
            );
            return_value(val as i64 as u64, emu)
        }
        SyscallNumber::MaxValue => (),
    }
}

fn exit(emu: &mut Emulator, exit_number: usize) {
    use core::fmt::Write;
    writeln!(UART.lock(), "exit({}) called!", exit_number).unwrap();
    emu.halt();
}

fn write(emu: &mut Emulator, fd: usize, user_buf: virtmem::UserPointer<[u8]>, count: usize) -> i32 {
    let buf = if let Some(val) = user_buf.try_as_ref(&mut emu.memory, count) {
        val
    } else {
        return -1;
    };

    match fd {
        1 /* stdout */ | 2 /* stderr */ => {
            use crate::TERMINAL;
            use core::fmt::Write;
            let str_buf = if let Ok(val) = core::str::from_utf8(buf) { val } else { return -1; };
            let res = writeln!(TERMINAL.lock(), "{}", str_buf);
            if res.is_err() { return -1; }
            return count as i32;
        } //stdout
        _ => return -1,
    }
}
