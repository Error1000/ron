use crate::{
    hio::{KeyboardPacket, KeyboardKey, KeyboardPacketType},
    primitives::{LazyInitialised, Mutex},
    virtmem::KernPointer,
    X86Default,
};
use packed_struct::prelude::*;

pub static KEYBOARD_INPUT: Mutex<LazyInitialised<PS2Device>> = Mutex::from(LazyInitialised::uninit());

#[derive(PackedStruct, Clone, Copy, Debug)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "1")]
pub struct KeyboardModifiers {
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
    pub num_lock: bool,
}

impl KeyboardModifiers {
    pub fn none() -> KeyboardModifiers {
        KeyboardModifiers{
            left_shift: false,
            right_shift: false,
            left_alt: false,
            right_alt: false,
            left_ctrl: false,
            right_ctrl: false,
            caps_lock: false,
            num_lock: false
        }
    }

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

/// FIXME: We assume the PS/2 controller exists, is already initialized and no devices are plugged or unplugged ever, oh and also that all communication is 100% reliable
/// Also assumes first ps/2 port is keyboard, and for now just disables the second one ( if it exists )
// What could go wrong ¯\_(ツ)_/¯
#[derive(Debug)]
pub struct PS2Device {
    data: KernPointer<u8>,
    status_and_command: KernPointer<u8>,
    active_modifiers: KeyboardModifiers,
}

impl X86Default for PS2Device {
    unsafe fn x86_default() -> Self {
        let mut ps2 = Self {
            data: KernPointer::<u8>::from_port(0x60),
            status_and_command: KernPointer::<u8>::from_port(0x64),
            active_modifiers: KeyboardModifiers::none()
        };

        wait_for!(!StatusRegister::unpack_from_slice(&[ps2.status_and_command.read()]).unwrap().is_input_buf_full);
        ps2.status_and_command.write(0xA7);
        ps2
    }
}

impl PS2Device {
    unsafe fn try_read_byte(&mut self) -> Option<u8> {
        if !(StatusRegister::unpack_from_slice(&[self.status_and_command.read()]).unwrap().is_output_buf_full) {
            return None;
        } else {
            return Some(self.data.read());
        }
    }

    unsafe fn read_byte(&mut self) -> u8 {
        let mut res;
        wait_for!({
            res = self.try_read_byte();
            res.is_some()
        });
        return res.unwrap();
    }

    // Reads a set 1 or set 2 scan code
    unsafe fn read_scancode(&mut self) -> Option<u32> {
        let mut byte = self.try_read_byte()?;
        let scancode;
        // Handle multi-byte
        if byte == 0xE0 {
            byte = self.read_byte();
            if byte == 0xF0 {
                byte = self.read_byte();
                scancode = (0xE0 << 16) | (0xF0 << 8) | byte as u32;
            } else{
                scancode = (0xE0 << 8) | byte as u32;
            }
        } else if byte == 0xF0 {
            byte = self.read_byte();
            scancode = (0xF0 << 8) | byte as u32;
        } else {
            scancode = byte as u32;
        }

        Some(scancode)
    }

    // NOTE: This only supports scan code set 1
    pub unsafe fn try_read_packet(&mut self) -> Option<KeyboardPacket> {
        let scancode = self.read_scancode()?;
        let key = KeyboardKey::from_scancode_in_set1(scancode)?;
        let packet_type = if crate::hio::is_scancode_in_set1_pressed(scancode) { KeyboardPacketType::KeyPressed } else { KeyboardPacketType::KeyReleased };


        // Change state of modifiers
        match (key, packet_type) {
            (KeyboardKey::LeftShift, KeyboardPacketType::KeyPressed) => self.active_modifiers.left_shift = true,
            (KeyboardKey::LeftShift, KeyboardPacketType::KeyReleased) => self.active_modifiers.left_shift = false,

            (KeyboardKey::RightShift, KeyboardPacketType::KeyPressed) => self.active_modifiers.right_shift = true,
            (KeyboardKey::RightShift, KeyboardPacketType::KeyReleased) => self.active_modifiers.right_shift = false,

            (KeyboardKey::LeftCtrl, KeyboardPacketType::KeyPressed) => self.active_modifiers.left_ctrl = true,
            (KeyboardKey::LeftCtrl, KeyboardPacketType::KeyReleased) => self.active_modifiers.left_ctrl = false,

            (KeyboardKey::RightCtrl, KeyboardPacketType::KeyPressed) => self.active_modifiers.right_ctrl = true,
            (KeyboardKey::RightCtrl, KeyboardPacketType::KeyReleased) => self.active_modifiers.right_ctrl = false,

            (KeyboardKey::LeftAlt, KeyboardPacketType::KeyPressed) => self.active_modifiers.left_alt = true,
            (KeyboardKey::LeftAlt, KeyboardPacketType::KeyReleased) => self.active_modifiers.left_alt = false,

            (KeyboardKey::RightAlt, KeyboardPacketType::KeyPressed) => self.active_modifiers.right_alt = true,
            (KeyboardKey::RightAlt, KeyboardPacketType::KeyReleased) => self.active_modifiers.right_alt = false,

            (KeyboardKey::CapsLock, KeyboardPacketType::KeyPressed) => self.active_modifiers.caps_lock = !self.active_modifiers.caps_lock,
            _ => {}
        }

        Some(KeyboardPacket {
            key,
            modifiers: self.active_modifiers,
            packet_type
        })
    }

    pub unsafe fn read_packet(&mut self) -> KeyboardPacket {
        let mut res;
        wait_for!({
            res = self.try_read_packet();
            res.is_some()
        });
        return res.unwrap();
    }
}
