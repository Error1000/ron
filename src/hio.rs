use crate::SpecialKeys;
// Human input/output
#[repr(u8)]
#[derive(PartialEq)]
pub enum KeyboardPacketType{
    KEY_PRESSED, KEY_RELEASED
}
pub struct KeyboardPacket {
    pub scancode: u8,
    pub char_codepoint: Option<char>,
    pub special_keys: SpecialKeys,
    pub typ: KeyboardPacketType,
}

impl KeyboardPacket{
    pub fn shift_codepoint(&self) -> Option<char> {
        let c = self.char_codepoint.map(|v|{
        match v.to_ascii_uppercase() {
            '1' => '!',
            '2' => '@',
            '3' => '#',
            '4' => '$',
            '5' => '%',
            '6' => '&',
            '8' => '*',
            '9' => '(',
            '0' => ')',
            '-' => '_',
            '=' => '+',
            '[' => '{',
            ']' => '}',
            ';' => ':',
            '\'' => '"',
            ',' => '<',
            '.' => '>',
            '/' => '?',
            '\\' => '|',
            _ => v.to_ascii_uppercase(),
        }
    });
    c
    }
}