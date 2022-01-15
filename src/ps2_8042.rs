use crate::{X86Default, hio::{KeyboardPacket, KeyboardPacketType}, virtmem::KernPointer};
use bitflags::bitflags;
use packed_struct::prelude::*;

bitflags!{
pub struct SpecialKeys : u16 {
    const LEFT_SHIFT = 1 << 0;
    const RIGHT_SHIFT = 1 << 1;
    const LEFT_ALT = 1 << 2;
    const RIGHT_ALT = 1 << 3;
    const LEFT_CTRL = 1 << 4;
    const RIGHT_CTRL = 1 << 5;
    const CAPS_LOCK = 1 << 6;
    const UP_ARROW = 1 << 7;
    const DOWN_ARROW = 1 << 8;
    const LEFT_ARROW = 1 << 9;
    const RIGHT_ARROW = 1 << 10;
    const ESC = 1 << 11;
}
}
impl SpecialKeys{
    pub fn any_shift(&self) -> bool { self.contains(SpecialKeys::LEFT_SHIFT) || self.contains(SpecialKeys::RIGHT_SHIFT) }
    pub fn any_alt(&self) -> bool { self.contains(SpecialKeys::LEFT_ALT) || self.contains(SpecialKeys::RIGHT_ALT) }
    pub fn any_ctrl(&self) -> bool { self.contains(SpecialKeys::LEFT_CTRL) || self.contains(SpecialKeys::RIGHT_CTRL) }

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
    special_keys: SpecialKeys
}

impl X86Default for PS2Device {
    unsafe fn x86_default() -> Self {
        let mut ps2 = Self {
            data: KernPointer::<u8>::from_port(0x60),
            status_and_command: KernPointer::<u8>::from_port(0x64),
            special_keys: SpecialKeys::empty()
        };
        wait_for!(!StatusRegister::unpack_from_slice(&[ps2.status_and_command.read()]).unwrap().is_input_buf_full);
        ps2.status_and_command.write(0xA7);
        ps2
    }
}

pub const scan_code_set_1: [char; 128] = [
    ' ', ' ', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\r', '\t', 'q', 'w',
    'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n', ' ', 'a', 's', 'd', 'f', 'g', 'h', 'j',
    'k', 'l', ';', '\'', '`', ' ', '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', ' ',
    '*', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' '
];

impl PS2Device {
    pub unsafe fn read_byte(&mut self) -> u8 {
        wait_for!(StatusRegister::unpack_from_slice(&[self.status_and_command.read()]).unwrap().is_output_buf_full);
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
            0x2A => self.special_keys |= SpecialKeys::LEFT_SHIFT,
            0xAA => self.special_keys &= !SpecialKeys::LEFT_SHIFT,

            0x36 => self.special_keys |= SpecialKeys::RIGHT_SHIFT,
            0xB6 => self.special_keys &= !SpecialKeys::RIGHT_SHIFT,

            0x1D if !multibyte => self.special_keys |= SpecialKeys::LEFT_CTRL,
            0x9D if !multibyte => self.special_keys &= !SpecialKeys::LEFT_CTRL,

            0x1D if multibyte => self.special_keys |= SpecialKeys::RIGHT_CTRL,
            0x9D if multibyte => self.special_keys &= !SpecialKeys::RIGHT_CTRL,

            0x38 if !multibyte => self.special_keys |= SpecialKeys::LEFT_ALT,
            0xB8 if !multibyte => self.special_keys &= !SpecialKeys::LEFT_ALT,

            0x38 if multibyte => self.special_keys |= SpecialKeys::RIGHT_ALT,
            0xB8 if multibyte => self.special_keys &= !SpecialKeys::RIGHT_ALT,

            0x3A => self.special_keys |= SpecialKeys::CAPS_LOCK,
            0xBA => self.special_keys &= !SpecialKeys::CAPS_LOCK,
            
            0x48 => self.special_keys |= SpecialKeys::UP_ARROW,
            0xC8 => self.special_keys &= !SpecialKeys::UP_ARROW,

            0x50 => self.special_keys |= SpecialKeys::DOWN_ARROW,
            0xD0 => self.special_keys &= !SpecialKeys::DOWN_ARROW,

            0x4D => self.special_keys |= SpecialKeys::RIGHT_ARROW,
            0xCD => self.special_keys &= !SpecialKeys::RIGHT_ARROW,

            0x4B => self.special_keys |= SpecialKeys::LEFT_ARROW,
            0xCB => self.special_keys &= !SpecialKeys::LEFT_ARROW,

            0x01 => self.special_keys |= SpecialKeys::ESC,
            0x81 => self.special_keys &= !SpecialKeys::ESC,

            _ => {},
        }
        let dc = scan_code_set_1[(b&0x7f) as usize];
        KeyboardPacket {
            scancode: b & 0x7F,
            char_codepoint: if dc == ' ' && (b & 0x7F) != 0x39 { None } else { Some(dc) },
            special_keys: if b & 0x80 == 0 { self.special_keys } else { old_special },
            typ: if b & 0x80 == 0 { KeyboardPacketType::KEY_PRESSED } else { KeyboardPacketType::KEY_RELEASED }
        }
    }

}
