use crate::{emulator::Riscv64Cpu, virtmem::LittleEndianVirtualMemory, UART};

type Emulator = Riscv64Cpu<LittleEndianVirtualMemory>;
pub fn syscall_linux_abi_entry_point(emu: &mut Emulator) {
    // Source: man syscall
    let syscall_number = emu.read_reg(17); // a7

    // a0-a5
    let argument_1 = || emu.read_reg(10);
    let argument_2 = || emu.read_reg(11);
    let argument_3 = || emu.read_reg(12);
    let argument_4 = || emu.read_reg(13);
    let argument_5 = || emu.read_reg(14);
    let argument_6 = || emu.read_reg(15);

    match syscall_number {
        1 => exit_1(emu, argument_1() as usize),
        _ => {
            use core::fmt::Write;
            writeln!(
                UART.lock(),
                "Syscall number {}, is not implemented, ignoring!",
                syscall_number
            )
            .unwrap();
        }
    }
}

fn exit_1(emu: &mut Emulator, exit_number: usize) {
    use core::fmt::Write;
    writeln!(UART.lock(), "exit({}) called!", exit_number).unwrap();
    emu.halt();
}
