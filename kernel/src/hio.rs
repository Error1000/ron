use crate::ps2_8042::KeyboardModifiers;
// Human input/output
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum KeyboardPacketType {
    KeyPressed,
    KeyReleased,
}

pub struct KeyboardPacket {
    pub key: KeyboardKey,
    pub modifiers: KeyboardModifiers,
    pub packet_type: KeyboardPacketType,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum KeyboardKey {
    Unmapped{row: usize, column: usize}, // These keys are user-configurable, for the meaning of row and column refer to ANSI keyboard layout
    Escape,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, 
    Delete,
    UpArrow, DownArrow, LeftArrow, RightArrow,
    Backspace,
    Enter,
    Space,
    Tab,
    Insert,
    Home,
    PageUp,
    PageDown,
    End,
    PrintScreen,
    LeftShift,
    RightShift,
    LeftCtrl,
    RightCtrl,
    LeftSuper,
    RightSuper,
    LeftAlt,
    RightAlt,
    CapsLock,
    NumLock,
    ScrollLock,
    Power,
    Sleep,
    Wake,
    NextTrack,
    PrevTrack,
    Stop,
    Play,
    Mute,
    VolUp,
    VolDown,
    MediaSelect,
    EMail,
    Calculator,
    MyComputer,
    WWWSearch,
    WWWHome,
    WWWBack,
    WWWForward,
    WWWStop,
    WWWRefresh,
    WWWFavorites,
    Keypad0,
    Keypad1,
    Keypad2,
    Keypad3,
    Keypad4,
    Keypad5,
    Keypad6,
    Keypad7,
    Keypad8,
    Keypad9,
    KeypadSlash,
    KeypadStar,
    KeypadMinus,
    KeypadPlus,
    KeypadEnter,
    KeypadDot,
}

impl KeyboardKey {
    pub fn from_scancode_in_set1(code: u32) -> Option<KeyboardKey> {
        Some(match code & 0x7F {
            0x01 => KeyboardKey::Escape,

            0x29 => KeyboardKey::Unmapped { row: 0, column: 0 },
            0x02 => KeyboardKey::Unmapped { row: 0, column: 1 },
            0x03 => KeyboardKey::Unmapped { row: 0, column: 2 },
            0x04 => KeyboardKey::Unmapped { row: 0, column: 3 },
            0x05 => KeyboardKey::Unmapped { row: 0, column: 4 },
            0x06 => KeyboardKey::Unmapped { row: 0, column: 5 },
            0x07 => KeyboardKey::Unmapped { row: 0, column: 6 },
            0x08 => KeyboardKey::Unmapped { row: 0, column: 7 },
            0x09 => KeyboardKey::Unmapped { row: 0, column: 8 },
            0x0A => KeyboardKey::Unmapped { row: 0, column: 9 },
            0x0B => KeyboardKey::Unmapped { row: 0, column: 10 },
            0x0C => KeyboardKey::Unmapped { row: 0, column: 11 },
            0x0D => KeyboardKey::Unmapped { row: 0, column: 12 },
            0x0E => KeyboardKey::Backspace,
            0x0F => KeyboardKey::Tab,

            0x10 => KeyboardKey::Unmapped { row: 1, column: 0 },
            0x11 => KeyboardKey::Unmapped { row: 1, column: 1 },
            0x12 => KeyboardKey::Unmapped { row: 1, column: 2 },
            0x13 => KeyboardKey::Unmapped { row: 1, column: 3 },
            0x14 => KeyboardKey::Unmapped { row: 1, column: 4 },
            0x15 => KeyboardKey::Unmapped { row: 1, column: 5 },
            0x16 => KeyboardKey::Unmapped { row: 1, column: 6 },
            0x17 => KeyboardKey::Unmapped { row: 1, column: 7 },
            0x18 => KeyboardKey::Unmapped { row: 1, column: 8 },
            0x19 => KeyboardKey::Unmapped { row: 1, column: 9 },
            0x1A => KeyboardKey::Unmapped { row: 1, column: 10 },
            0x1B => KeyboardKey::Unmapped { row: 1, column: 11 },
            0x2B => KeyboardKey::Unmapped { row: 1, column: 12 },

            0x1C => KeyboardKey::Enter,
            0x1D => KeyboardKey::LeftCtrl,

            0x1E => KeyboardKey::Unmapped { row: 2, column: 0 },
            0x1F => KeyboardKey::Unmapped { row: 2, column: 1 },
            0x20 => KeyboardKey::Unmapped { row: 2, column: 2 },
            0x21 => KeyboardKey::Unmapped { row: 2, column: 3 },
            0x22 => KeyboardKey::Unmapped { row: 2, column: 4 },
            0x23 => KeyboardKey::Unmapped { row: 2, column: 5 },
            0x24 => KeyboardKey::Unmapped { row: 2, column: 6 },
            0x25 => KeyboardKey::Unmapped { row: 2, column: 7 },
            0x26 => KeyboardKey::Unmapped { row: 2, column: 8 },
            0x27 => KeyboardKey::Unmapped { row: 2, column: 9 },
            0x28 => KeyboardKey::Unmapped { row: 2, column: 10 },

            0x2A => KeyboardKey::LeftShift,

            0x2C => KeyboardKey::Unmapped { row: 3, column: 0 },
            0x2D => KeyboardKey::Unmapped { row: 3, column: 1 },
            0x2E => KeyboardKey::Unmapped { row: 3, column: 2 },
            0x2F => KeyboardKey::Unmapped { row: 3, column: 3 },
            0x30 => KeyboardKey::Unmapped { row: 3, column: 4 },
            0x31 => KeyboardKey::Unmapped { row: 3, column: 5 },
            0x32 => KeyboardKey::Unmapped { row: 3, column: 6 },
            0x33 => KeyboardKey::Unmapped { row: 3, column: 7 },
            0x34 => KeyboardKey::Unmapped { row: 3, column: 8 },
            0x35 => KeyboardKey::Unmapped { row: 3, column: 9 },

            0x36 => KeyboardKey::RightShift,
            0x37 => KeyboardKey::KeypadStar,
            0x38 => KeyboardKey::LeftAlt,
            0x39 => KeyboardKey::Space,
            0x3A => KeyboardKey::CapsLock,
            0x3B => KeyboardKey::F1,
            0x3C => KeyboardKey::F2,
            0x3D => KeyboardKey::F3,
            0x3E => KeyboardKey::F4,
            0x3F => KeyboardKey::F5,
            0x40 => KeyboardKey::F6,
            0x41 => KeyboardKey::F7,
            0x42 => KeyboardKey::F8,
            0x43 => KeyboardKey::F9,
            0x44 => KeyboardKey::F10,

            0x45 => KeyboardKey::NumLock,
            0x46 => KeyboardKey::ScrollLock,
            0x47 => KeyboardKey::Keypad7,
            0x48 => KeyboardKey::Keypad8,
            0x49 => KeyboardKey::Keypad9,
            0x4A => KeyboardKey::KeypadMinus,
            0x4B => KeyboardKey::Keypad4,
            0x4C => KeyboardKey::Keypad5,
            0x4D => KeyboardKey::Keypad6,
            0x4E => KeyboardKey::KeypadPlus,
            0x4F => KeyboardKey::Keypad1,
            0x50 => KeyboardKey::Keypad2,
            0x51 => KeyboardKey::Keypad3,
            0x52 => KeyboardKey::Keypad0,
            0x53 => KeyboardKey::KeypadDot,

            0x54 => return None, // Invalid
            0x55 => return None, // Invalid

            0x57 => KeyboardKey::F11,
            0x58 => KeyboardKey::F12,

            // Extended set
            0xE02A | 0xE037 => KeyboardKey::PrintScreen,
            0xE035 => KeyboardKey::KeypadSlash,
            0xE038 => KeyboardKey::RightAlt,
            0xE047 => KeyboardKey::Home,
            0xE048 => KeyboardKey::UpArrow,
            0xE049 => KeyboardKey::PageUp,
            0xE050 => KeyboardKey::DownArrow,
            0xE051 => KeyboardKey::PageDown,
            0xE052 => KeyboardKey::Insert,
            0xE053 => KeyboardKey::Delete,
            0xE05B => KeyboardKey::LeftSuper,
            0xE05C => KeyboardKey::RightSuper,

            // ACPI Scan Codes
            0xE05E => KeyboardKey::Power,
            0xE05F => KeyboardKey::Sleep,
            0xE063 => KeyboardKey::Wake,

            // Windows multimedia
            0xE019 => KeyboardKey::NextTrack,
            0xE010 => KeyboardKey::PrevTrack,
            0xE024 => KeyboardKey::Stop,
            0xE022 => KeyboardKey::Play,
            0xE020 => KeyboardKey::Mute,
            0xE030 => KeyboardKey::VolUp,
            0xE02E => KeyboardKey::VolDown,
            0xE06D => KeyboardKey::MediaSelect,
            0xE06C => KeyboardKey::EMail,
            0xE021 => KeyboardKey::Calculator,
            0xE06B => KeyboardKey::MyComputer,
            0xE065 => KeyboardKey::WWWSearch,
            0xE032 => KeyboardKey::WWWHome,
            0xE06A => KeyboardKey::WWWBack,
            0xE069 => KeyboardKey::WWWForward,
            0xE068 => KeyboardKey::WWWStop,
            0xE067 => KeyboardKey::WWWRefresh,
            0xE066 => KeyboardKey::WWWFavorites,

            _ => return None,
        })
    }
}

pub fn is_scancode_in_set1_pressed(code: u32) -> bool {
    return code & 0x80 == 0;
}

pub mod standard_usa_qwerty {
    use super::*;

    // Tries to map the key to a char based on the modifiers active
    pub fn parse_key(key: KeyboardKey, modifiers: KeyboardModifiers) -> Result<char, KeyboardKey> {
        // FIXME: Implement num pad and num lock support
        return match key {
            KeyboardKey::Unmapped{row: 0, column: 0} if !modifiers.any_shift() => Ok('`'),
            KeyboardKey::Unmapped{row: 0, column: 0} if modifiers.any_shift() => Ok('~'),

            KeyboardKey::Unmapped{row: 0, column: 1} if !modifiers.any_shift() => Ok('1'),
            KeyboardKey::Unmapped{row: 0, column: 1} if modifiers.any_shift() => Ok('!'),

            KeyboardKey::Unmapped{row: 0, column: 2} if !modifiers.any_shift() => Ok('2'),
            KeyboardKey::Unmapped{row: 0, column: 2} if modifiers.any_shift() => Ok('@'),

            KeyboardKey::Unmapped{row: 0, column: 3} if !modifiers.any_shift() => Ok('3'),
            KeyboardKey::Unmapped{row: 0, column: 3} if modifiers.any_shift() => Ok('#'),

            KeyboardKey::Unmapped{row: 0, column: 4} if !modifiers.any_shift() => Ok('4'),
            KeyboardKey::Unmapped{row: 0, column: 4} if modifiers.any_shift() => Ok('$'),

            KeyboardKey::Unmapped{row: 0, column: 5} if !modifiers.any_shift() => Ok('5'),
            KeyboardKey::Unmapped{row: 0, column: 5} if modifiers.any_shift() => Ok('%'),

            KeyboardKey::Unmapped{row: 0, column: 6} if !modifiers.any_shift() => Ok('6'),
            KeyboardKey::Unmapped{row: 0, column: 6} if modifiers.any_shift() => Ok('^'),

            KeyboardKey::Unmapped{row: 0, column: 7} if !modifiers.any_shift() => Ok('7'),
            KeyboardKey::Unmapped{row: 0, column: 7} if modifiers.any_shift() => Ok('&'),

            KeyboardKey::Unmapped{row: 0, column: 8} if !modifiers.any_shift() => Ok('8'),
            KeyboardKey::Unmapped{row: 0, column: 8} if modifiers.any_shift() => Ok('*'),

            KeyboardKey::Unmapped{row: 0, column: 9} if !modifiers.any_shift() => Ok('9'),
            KeyboardKey::Unmapped{row: 0, column: 9} if modifiers.any_shift() => Ok('('),

            KeyboardKey::Unmapped{row: 0, column: 10} if !modifiers.any_shift() => Ok('0'),
            KeyboardKey::Unmapped{row: 0, column: 10} if modifiers.any_shift() => Ok(')'),

            KeyboardKey::Unmapped{row: 0, column: 11} if !modifiers.any_shift() => Ok('-'),
            KeyboardKey::Unmapped{row: 0, column: 11} if modifiers.any_shift() => Ok('_'),

            KeyboardKey::Unmapped{row: 0, column: 12} if !modifiers.any_shift() => Ok('='),
            KeyboardKey::Unmapped{row: 0, column: 12} if modifiers.any_shift() => Ok('+'),

            KeyboardKey::Unmapped{row: 1, column: 0} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('q'),
            KeyboardKey::Unmapped{row: 1, column: 0} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('Q'),

            KeyboardKey::Unmapped{row: 1, column: 1} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('w'),
            KeyboardKey::Unmapped{row: 1, column: 1} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('W'),

            KeyboardKey::Unmapped{row: 1, column: 2} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('e'),
            KeyboardKey::Unmapped{row: 1, column: 2} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('E'),

            KeyboardKey::Unmapped{row: 1, column: 3} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('r'),
            KeyboardKey::Unmapped{row: 1, column: 3} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('R'),

            KeyboardKey::Unmapped{row: 1, column: 4} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('t'),
            KeyboardKey::Unmapped{row: 1, column: 4} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('T'),

            KeyboardKey::Unmapped{row: 1, column: 5} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('y'),
            KeyboardKey::Unmapped{row: 1, column: 5} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('Y'),

            KeyboardKey::Unmapped{row: 1, column: 6} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('u'),
            KeyboardKey::Unmapped{row: 1, column: 6} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('U'),

            KeyboardKey::Unmapped{row: 1, column: 7} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('i'),
            KeyboardKey::Unmapped{row: 1, column: 7} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('I'),

            KeyboardKey::Unmapped{row: 1, column: 8} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('o'),
            KeyboardKey::Unmapped{row: 1, column: 8} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('O'),

            KeyboardKey::Unmapped{row: 1, column: 9} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('p'),
            KeyboardKey::Unmapped{row: 1, column: 9} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('P'),

            KeyboardKey::Unmapped{row: 1, column: 10} if !modifiers.any_shift() => Ok('['),
            KeyboardKey::Unmapped{row: 1, column: 10} if modifiers.any_shift() => Ok('{'),

            KeyboardKey::Unmapped{row: 1, column: 11} if !modifiers.any_shift() => Ok(']'),
            KeyboardKey::Unmapped{row: 1, column: 11} if modifiers.any_shift() => Ok('}'),

            KeyboardKey::Unmapped{row: 1, column: 12} if !modifiers.any_shift() => Ok('\\'),
            KeyboardKey::Unmapped{row: 1, column: 12} if modifiers.any_shift() => Ok('|'),


            KeyboardKey::Unmapped{row: 2, column: 0} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('a'),
            KeyboardKey::Unmapped{row: 2, column: 0} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('A'), 

            KeyboardKey::Unmapped{row: 2, column: 1} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('s'),
            KeyboardKey::Unmapped{row: 2, column: 1} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('S'), 

            KeyboardKey::Unmapped{row: 2, column: 2} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('d'),
            KeyboardKey::Unmapped{row: 2, column: 2} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('D'), 

            KeyboardKey::Unmapped{row: 2, column: 3} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('f'),
            KeyboardKey::Unmapped{row: 2, column: 3} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('F'), 

            KeyboardKey::Unmapped{row: 2, column: 4} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('g'),
            KeyboardKey::Unmapped{row: 2, column: 4} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('G'), 

            KeyboardKey::Unmapped{row: 2, column: 5} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('h'),
            KeyboardKey::Unmapped{row: 2, column: 5} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('H'), 

            KeyboardKey::Unmapped{row: 2, column: 6} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('j'),
            KeyboardKey::Unmapped{row: 2, column: 6} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('J'), 

            KeyboardKey::Unmapped{row: 2, column: 7} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('k'),
            KeyboardKey::Unmapped{row: 2, column: 7} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('K'), 

            KeyboardKey::Unmapped{row: 2, column: 8} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('l'),
            KeyboardKey::Unmapped{row: 2, column: 8} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('L'), 

            KeyboardKey::Unmapped{row: 2, column: 9} if !modifiers.any_shift()  => Ok(';'),
            KeyboardKey::Unmapped{row: 2, column: 9} if modifiers.any_shift() => Ok(':'), 

            KeyboardKey::Unmapped{row: 2, column: 10} if !modifiers.any_shift() => Ok('\''),
            KeyboardKey::Unmapped{row: 2, column: 10} if modifiers.any_shift() => Ok('"'), 

            KeyboardKey::Unmapped{row: 3, column: 0} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('z'),
            KeyboardKey::Unmapped{row: 3, column: 0} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('Z'), 

            KeyboardKey::Unmapped{row: 3, column: 1} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('x'),
            KeyboardKey::Unmapped{row: 3, column: 1} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('X'), 

            KeyboardKey::Unmapped{row: 3, column: 2} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('c'),
            KeyboardKey::Unmapped{row: 3, column: 2} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('C'), 

            KeyboardKey::Unmapped{row: 3, column: 3} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('v'),
            KeyboardKey::Unmapped{row: 3, column: 3} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('V'), 

            KeyboardKey::Unmapped{row: 3, column: 4} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('b'),
            KeyboardKey::Unmapped{row: 3, column: 4} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('B'), 

            KeyboardKey::Unmapped{row: 3, column: 5} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('n'),
            KeyboardKey::Unmapped{row: 3, column: 5} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('N'), 

            KeyboardKey::Unmapped{row: 3, column: 6} if !(modifiers.any_shift() ^ modifiers.caps_lock) => Ok('m'),
            KeyboardKey::Unmapped{row: 3, column: 6} if modifiers.any_shift() ^ modifiers.caps_lock => Ok('M'), 

            KeyboardKey::Unmapped{row: 3, column: 7} if !modifiers.any_shift() => Ok(','),
            KeyboardKey::Unmapped{row: 3, column: 7} if modifiers.any_shift() => Ok('<'), 
            
            KeyboardKey::Unmapped{row: 3, column: 8} if !modifiers.any_shift() => Ok('.'),
            KeyboardKey::Unmapped{row: 3, column: 8} if modifiers.any_shift() => Ok('>'), 

            KeyboardKey::Unmapped{row: 3, column: 9} if !modifiers.any_shift() => Ok('/'),
            KeyboardKey::Unmapped{row: 3, column: 9} if modifiers.any_shift() => Ok('?'), 

            KeyboardKey::Space => Ok(' '),
            KeyboardKey::Tab => Ok('\t'),
            KeyboardKey::Enter => Ok('\n'),

            _ => Err(key)
        };
    }

}