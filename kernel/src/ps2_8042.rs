use crate::{
    hio::{KeyboardPacket, KeyboardPacketType},
    virtmem::KernPointer,
    X86Default,
};
use packed_struct::prelude::*;

#[derive(PackedStruct, Clone, Copy)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "2")]
pub struct SpecialKeys {
    #[packed_field(bits = "0")]
    pub left_shift: bool,
    #[packed_field(bits = "1")]
    pub right_shift: bool,
    #[packed_field(bits = "2")]
    pub left_alt: bool,
    #[packed_field(bits = "3")]
    pub right_alt: bool,
    #[packed_field(bits = "4")]
    pub left_ctrl: bool,
    #[packed_field(bits = "5")]
    pub right_ctrl: bool,
    #[packed_field(bits = "6")]
    pub caps_lock: bool,
    #[packed_field(bits = "7")]
    pub up_arrow: bool,
    #[packed_field(bits = "8")]
    pub down_arrow: bool,
    #[packed_field(bits = "9")]
    pub left_arrow: bool,
    #[packed_field(bits = "10")]
    pub right_arrow: bool,
    #[packed_field(bits = "11")]
    pub esc: bool,
}

impl SpecialKeys {
    pub fn any_shift(&self) -> bool {
        self.left_shift || self.right_shift
    }
    pub fn any_alt(&self) -> bool {
        self.left_alt || self.right_alt
    }
    pub fn any_ctrl(&self) -> bool {
        self.left_ctrl || self.right_alt
    }
}

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

/// FIXME; We assume the PS/2 controller exists, is already initialised and no devices are plugged or unplugged ever, oh and also that all communication is 100% reliable
/// Also assumes first ps/2 port is keyboard, and for now just disables the second one ( if it exists )
// What could go wrong ¯\_(ツ)_/¯
pub struct PS2Device {
    data: KernPointer<u8>,
    status_and_command: KernPointer<u8>,
    special_keys: SpecialKeys,
}

impl X86Default for PS2Device {
    unsafe fn x86_default() -> Self {
        let mut ps2 = Self {
            data: KernPointer::<u8>::from_port(0x60),
            status_and_command: KernPointer::<u8>::from_port(0x64),
            special_keys: SpecialKeys::unpack(&[0, 0]).unwrap(),
        };
        wait_for!(
            !StatusRegister::unpack_from_slice(&[ps2.status_and_command.read()])
                .unwrap()
                .is_input_buf_full
        );
        ps2.status_and_command.write(0xA7);
        ps2
    }
}

pub const SCAN_CODE_SET_1: [char; 128] = [
    ' ', ' ', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\r', '\t', 'q', 'w',
    'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n', ' ', 'a', 's', 'd', 'f', 'g', 'h', 'j',
    'k', 'l', ';', '\'', '`', ' ', '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', ' ',
    '*', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
];

impl PS2Device {
    pub unsafe fn read_byte(&mut self) -> u8 {
        wait_for!(
            StatusRegister::unpack_from_slice(&[self.status_and_command.read()])
                .unwrap()
                .is_output_buf_full
        );
        self.data.read()
    }

    /// NOTE: Assumes scan code set 1
    pub unsafe fn read_packet(&mut self) -> KeyboardPacket {
        let mut b = self.read_byte();
        let mut multibyte = false;
        // Multibyte
        if b == 0xE0 {
            b = self.read_byte();
            multibyte = true;
        }
        let old_special = self.special_keys;

        match b {
            0x2A => self.special_keys.left_shift = true,
            0xAA => self.special_keys.left_shift = false,

            0x36 => self.special_keys.right_shift = true,
            0xB6 => self.special_keys.right_shift = false,

            0x1D if !multibyte => self.special_keys.left_ctrl = true,
            0x9D if !multibyte => self.special_keys.left_ctrl = false,

            0x1D if multibyte => self.special_keys.right_ctrl = true,
            0x9D if multibyte => self.special_keys.right_ctrl = false,

            0x38 if !multibyte => self.special_keys.left_alt = true,
            0xB8 if !multibyte => self.special_keys.left_alt = false,

            0x38 if multibyte => self.special_keys.right_alt = true,
            0xB8 if multibyte => self.special_keys.right_alt = false,

            0x3A => self.special_keys.caps_lock = true,
            0xBA => self.special_keys.caps_lock = false,

            0x48 => self.special_keys.up_arrow = true,
            0xC8 => self.special_keys.up_arrow = false,

            0x50 => self.special_keys.down_arrow = true,
            0xD0 => self.special_keys.down_arrow = false,

            0x4D => self.special_keys.right_arrow = true,
            0xCD => self.special_keys.right_arrow = false,

            0x4B => self.special_keys.left_arrow = true,
            0xCB => self.special_keys.left_arrow = false,

            0x01 => self.special_keys.esc = true,
            0x81 => self.special_keys.esc = false,

            _ => {}
        }
        let dc = SCAN_CODE_SET_1[(b & 0x7f) as usize];
        KeyboardPacket {
            scancode: b & 0x7F,
            char_codepoint: if dc == ' ' && (b & 0x7F) != 0x39 {
                None
            } else {
                Some(dc)
            },
            special_keys: if b & 0x80 == 0 {
                self.special_keys
            } else {
                old_special
            },
            typ: if b & 0x80 == 0 {
                KeyboardPacketType::KeyPressed
            } else {
                KeyboardPacketType::KeyReleased
            },
        }
    }
}
