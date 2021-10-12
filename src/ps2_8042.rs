use crate::{virtmem::KernPointer, UnsafeDefault};
use packed_struct::prelude::*;

#[derive(PackedStruct)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "1")]
struct StatusRegister {
    #[packed_field(bits = "0")]
    is_output_buf_full: bool,
    #[packed_field(bits = "1")]
    is_input_buf_full: bool,
    #[packed_field(bits = "2")]
    system_flag: bool,
    #[packed_field(bits = "3")]
    selector: bool, // false = data goes to ps/2 device, 1 = data goes to ps/2 controller command
    #[packed_field(bits = "6")]
    timeout_error: bool,
    #[packed_field(bits = "7")]
    parity_error: bool,
}

/// FIXME; We assume the PS/2 controller exists, is already initialised and no devices are plugged or unplugged ever, oh anf also that all communication is 100% reliable
/// Also asumes first ps/2 port is keyboard, and for now just disables the second one ( if it exists )
// What could go wrong ¯\_(ツ)_/¯
pub struct PS2Device {
    data: KernPointer<u8>,
    status_and_command: KernPointer<u8>,
}

impl UnsafeDefault for PS2Device {
    unsafe fn unsafe_default() -> Self {
        let mut ps2 = Self {
            data: KernPointer::<u8>::from_port(0x60),
            status_and_command: KernPointer::<u8>::from_port(0x64),
        };
        wait_for!(!StatusRegister::unpack_from_slice(&[ps2.status_and_command.read()]).unwrap().is_input_buf_full);
        ps2.status_and_command.write(0xA7);
        ps2
    }
}

pub const scan_code_set_1: [char; 256] = [
    ' ', ' ', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\r', '\t', 'q', 'w',
    'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n', ' ', 'a', 's', 'd', 'f', 'g', 'h', 'j',
    'k', 'l', ';', '\'', '`', ' ', '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', ' ',
    '*', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
];

impl PS2Device {
    pub unsafe fn read_byte(&mut self) -> u8 {
        wait_for!(StatusRegister::unpack_from_slice(&[self.status_and_command.read()]).unwrap().is_output_buf_full);
        self.data.read()
    }
}
