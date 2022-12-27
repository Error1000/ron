use core::ops::{BitOr, BitOrAssign};

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct ConversionFlags(u8);

impl BitOr for ConversionFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        return ConversionFlags(self.0 | rhs.0);
    }
}

impl BitOrAssign for ConversionFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

pub mod conversion_flag {
    use super::ConversionFlags;

    pub const NONE: ConversionFlags = ConversionFlags(0);

    pub const LEFT_JUSTIFY: ConversionFlags = ConversionFlags(1 << 0); // '-'
    pub const ALWAYS_PRECEED_WITH_SIGN: ConversionFlags = ConversionFlags(1 << 1); // '+'
    pub const PRECEED_WITH_BLANK_SPACE_IF_NO_SIGN: ConversionFlags = ConversionFlags(1 << 2); // ' '
    pub const PRECEED_WITH_BASE_MARKING: ConversionFlags = ConversionFlags(1 << 3); // '#', adds 0x, 0 or 0X to the beginning of numbers 0x and 0X for hex, and 0 for octal
    pub const LEFT_PAD_WITH_ZEROES: ConversionFlags = ConversionFlags(1 << 4); // '0', left-pads with zeroes instead of spacing *when* padding is specified
    
    pub const ALL_FLAGS: ConversionFlags = ConversionFlags(LEFT_JUSTIFY.0 | ALWAYS_PRECEED_WITH_SIGN.0 | PRECEED_WITH_BLANK_SPACE_IF_NO_SIGN.0 | PRECEED_WITH_BASE_MARKING.0 | LEFT_PAD_WITH_ZEROES.0);
}

impl TryFrom<u8> for ConversionFlags {
    type Error = ();
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            b'-' => Ok(conversion_flag::LEFT_JUSTIFY),
            b'+' => Ok(conversion_flag::ALWAYS_PRECEED_WITH_SIGN),
            b' ' => Ok(conversion_flag::PRECEED_WITH_BLANK_SPACE_IF_NO_SIGN),
            b'#' => Ok(conversion_flag::PRECEED_WITH_BASE_MARKING),
            b'0' => Ok(conversion_flag::LEFT_PAD_WITH_ZEROES),
            _ => Err(())
        }
    }
}

#[derive(PartialEq)]
pub enum ConversionSpecifier {
    SignedInteger, // 'i'
    SignedDecimalInteger, // 'd'
    UnsignedDecimalInteger, // 'u'
    UnsignedOctalInteger, // 'o'
    UnsignedHexIntegerLowerCase, // 'x'
    UnsignedHexIntegerUpperCase, // 'X'
    DecimalFloatLowerCase, // 'f'
    DeicmalFloatUpperCase, // 'F'
    ScientificNotationLowerCase, // 'e'
    ScientificNotationUpperCase, // 'E'
    ShortestFloatLowerCase, // 'g'
    ShortestFloatUpperCase, // 'G'
    HexFloatLowerCase, // 'a'
    HexFloatUpperCase, // 'A'
    Character, // 'c'
    String, // 's'
    Pointer, // 'p'
    Escape, // '%' (allow you to write a % character to the stream)

    // NOTE: It's called meta because it writes to it's argument, instead of reading it, it changes the meaning of a conversion itself therefore it's meta
    Meta, // 'n' (stores number of printed characters so far in the location pointed to by the arg being formatted)
    Unparsed // There has to be a specifier in a conversion, but we may not have parsed it yet so we still need some way to mark that
}

impl TryFrom<u8> for ConversionSpecifier {
    type Error = ();
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            b'i' => Ok(Self::SignedInteger),
            b'd' => Ok(Self::SignedDecimalInteger),
            b'u' => Ok(Self::UnsignedDecimalInteger),
            b'o' => Ok(Self::UnsignedOctalInteger),
            b'x' => Ok(Self::UnsignedHexIntegerLowerCase),
            b'X' => Ok(Self::UnsignedHexIntegerUpperCase),
            b'f' => Ok(Self::DecimalFloatLowerCase),
            b'F' => Ok(Self::DeicmalFloatUpperCase),
            b'e' => Ok(Self::ScientificNotationLowerCase),
            b'E' => Ok(Self::ScientificNotationUpperCase),
            b'g' => Ok(Self::ShortestFloatLowerCase),
            b'G' => Ok(Self::ShortestFloatUpperCase),
            b'a' => Ok(Self::HexFloatLowerCase),
            b'A' => Ok(Self::HexFloatUpperCase),
            b'c' => Ok(Self::Character),
            b's' => Ok(Self::String),
            b'p' => Ok(Self::Pointer),
            b'%' => Ok(Self::Escape),
            b'n' => Ok(Self::Meta),
            _ => Err(())
        }
    }
}

#[derive(PartialEq)]
pub enum PrintfConversionWidth {
    Number(usize),
    Meta, // '*' The width is not specified in the format string, but as an additional integer value argument preceding the argument that has to be formatted.
    None
}

#[derive(PartialEq)]
pub enum ScanfConversionWidth {
    Number(usize),
    None
}

#[derive(PartialEq)]
pub enum ConversionPrecision {
    Number(usize),
    Meta, // '*' The precision is not specified in the format string, but as an additional integer value argument preceding the argument that has to be formatted. 
    None
}

#[derive(PartialEq)]
pub enum ConversionLength {
    Double, // 'L'
    MaxLengthForPointer, // 't'
    MaxLengthForSize, // 'z'
    MaxLengthForInt, // 'j'
    LongLong, // 'll'
    Long, // 'l'
    Half, // 'h'
    Byte, // 'hh'
    None
}

pub struct PrintfConversionSpecification {
    pub flags: ConversionFlags,
    pub width: PrintfConversionWidth,
    pub precision: ConversionPrecision,
    pub length: ConversionLength,
    pub specifier: ConversionSpecifier
}

pub struct ScanfConversionSpecification {
    pub assignment_suppression: bool,
    pub width: ScanfConversionWidth,
    pub length: ConversionLength,
    pub specifier: ConversionSpecifier
}

pub struct UnfinishedPrintfConversionSpecification {
    conversion_under_construction: PrintfConversionSpecification,
    parsing_width: bool,
    parsing_precision: bool,
    parsing_length: bool,
}

impl Default for UnfinishedPrintfConversionSpecification {
    fn default() -> Self {
        Self { 
            conversion_under_construction: PrintfConversionSpecification{
                flags: conversion_flag::NONE,
                width: PrintfConversionWidth::None,
                precision: ConversionPrecision::None,
                length: ConversionLength::None,
                specifier: ConversionSpecifier::Unparsed
            }, 
            parsing_width: false, 
            parsing_precision: false,
            parsing_length: false
        }
    }
}

pub struct UnfinishedScanfConversionSpecification {
    conversion_under_construction: ScanfConversionSpecification,
    parsing_width: bool,
    parsing_length: bool,
}

impl Default for UnfinishedScanfConversionSpecification {
    fn default() -> Self {
        Self { 
            conversion_under_construction: ScanfConversionSpecification {
                assignment_suppression: false,
                width: ScanfConversionWidth::None,
                length: ConversionLength::None,
                specifier: ConversionSpecifier::Unparsed
            }, 
            parsing_width: false, 
            parsing_length: false
        }
    }
}

// Returns: Ok if the Conversion has been parsed, Err if the Conversion still requires more characters to be parsed
// WARNING: Assumes char is part of a specification (a.k.a after a '%' character)
// WARNING: Does not assume order of parts, so while the spec lists the flags before the length, 
// this will, for example, parse the flags even if they appear after the already parsed length, so for example %l# is ok which is *fine* since it's undefined behavior ( C99 specification 7.19.6.1, paragraph 9:  "If a conversion specification is invalid, the behavior is undefined.)" )
pub fn add_char_to_printf_specification(mut initial: UnfinishedPrintfConversionSpecification, c: u8) -> Result<PrintfConversionSpecification, UnfinishedPrintfConversionSpecification> {
    if initial.parsing_width {
        match c {
            b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                // Add digit to width
                // Get current value
                if let PrintfConversionWidth::Number(cur_width) = initial.conversion_under_construction.width {
                    initial.conversion_under_construction.width = PrintfConversionWidth::Number(cur_width*10 + (c-b'0') as usize);
                } else {
                    // We have started parsing the conversion width, but haven't initialized it to be a Number
                    // What?
                    // That should be impossible since we always initialize it to Number before setting parsing_conversion_width
                    // Eh.. forget about it, just don't parse the conversion
                    // FIXME: Better logging
                    initial.conversion_under_construction.width = PrintfConversionWidth::None;
                    initial.parsing_width = false;
                }
                return Err(initial); // We parsed this character, move on
            }
            _ => {
                // Encountered non digit character, finish parsing conversion width number
                initial.parsing_width = false;
                // And try to use the character for something else so don't return
            }
        }
    }

    if initial.parsing_precision {
        match c {
            b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                // Add digit to precision
                // Get current value
                if let ConversionPrecision::Number(cur_precision) = initial.conversion_under_construction.precision {
                    initial.conversion_under_construction.precision = ConversionPrecision::Number(cur_precision*10 + (c-b'0') as usize);
                } else {
                    // We have started parsing the conversion precision, but haven't initialized it to be a Number ( this is normal )
                    // So let's do that.
                    initial.conversion_under_construction.precision = ConversionPrecision::Number((c-b'0') as usize);
                }
                return Err(initial); // We parsed this character, move on
            }

            b'*' => {
                initial.conversion_under_construction.precision = ConversionPrecision::Meta;
                initial.parsing_precision = false;
                return Err(initial); // We parsed this character, move on
            }

            _ => {
                // Encountered non digit character, finish parsing conversion precision number
                initial.parsing_precision = false;
                // And try to use the character for something else so don't return
            }
        }
    }

    if initial.parsing_length {
        initial.parsing_length = false;
        match c {
            b'l' => {
                if initial.conversion_under_construction.length == ConversionLength::Long {
                    initial.conversion_under_construction.length = ConversionLength::LongLong; 
                    return Err(initial);
                } else { 
                    // Some other character (probably h) followed by l?, so hl?, no. it must be a apart of something else, don't return, and assume that the length is just the first character
                }
            }
            
            b'h' => {
                if initial.conversion_under_construction.length == ConversionLength::Half {
                    initial.conversion_under_construction.length = ConversionLength::Byte; 
                    return Err(initial);
                } else {
                    // Some other character (probably l) followed by b?, so lh?, no. it must be a apart of something else, don't return, and assume that the length is just the first character
                }
            }

            _ => {
                // Encountered a normal character not part of a double character 'll' or 'hh'
                // Assume we have already parsed the full length and are now finished
                // Try to use the character for something else so don't return
            }
        }
    }

    // If we have no flags yet maybe this character is the flag specifier
    if initial.conversion_under_construction.flags != conversion_flag::ALL_FLAGS {
        if let Ok(val) = ConversionFlags::try_from(c) {
            initial.conversion_under_construction.flags |= val;
            return Err(initial); // We parsed this character, return
        }
    }

    // If we have no width yet then maybe this character is part of the width
    if initial.conversion_under_construction.width == PrintfConversionWidth::None {
        // NOTE: Width doesn't have a + or - sign in front it's just a plain number
        match c {
            b'*' => initial.conversion_under_construction.width = PrintfConversionWidth::Meta,
            b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                // The width is a number
                // Add digit to width
                // and mark the start of parsing the width as a number
                initial.conversion_under_construction.width = PrintfConversionWidth::Number((c-b'0') as usize);
                initial.parsing_width = true;
                return Err(initial); // We parsed this character, return
            }
            _ => {} // The character was not part of the width, try something else, don't return
        }
    }

    // If we have no precision yet then maybe this character is part of the precision
    if initial.conversion_under_construction.precision == ConversionPrecision::None {
        // NOTE: Precision always starts with a .
        match c {
            b'.' => {
                initial.parsing_precision = true;
                return Err(initial); // We parsed this character, return
            }
            _ => {} // The character was not part of the precision, try something else, don't return
        }
    }

    // If we have no length yet then maybe this character is part of the length
    if initial.conversion_under_construction.length == ConversionLength::None {
        match c {
            b'h' => { initial.conversion_under_construction.length = ConversionLength::Half; initial.parsing_length = true; /* set parsing_length because it could be hh */ return Err(initial); }
            b'l' => { initial.conversion_under_construction.length = ConversionLength::Long; initial.parsing_length = true; /* set parsing_length because it could be ll */ return Err(initial); }
            b'j' => { initial.conversion_under_construction.length = ConversionLength::MaxLengthForInt; return Err(initial); }
            b'z' => { initial.conversion_under_construction.length = ConversionLength::MaxLengthForSize; return Err(initial); }
            b't' => { initial.conversion_under_construction.length = ConversionLength::MaxLengthForPointer; return Err(initial); }
            b'L' => { initial.conversion_under_construction.length = ConversionLength::Double; return Err(initial); }
            _ => {} // The character was not part of the length, try something else, don't return
        }
    }

    // If we have no specifier yet then maybe this character is part of the specifier
    if initial.conversion_under_construction.specifier == ConversionSpecifier::Unparsed {
        if let Ok(val) = ConversionSpecifier::try_from(c) {
            initial.conversion_under_construction.specifier = val;

            // Once we find the specifier we have finished parsing the conversion
            return Ok(initial.conversion_under_construction);
        }
    }

    // Unrecognized character, ignore it
    // FIXME: Better logging
    return Err(initial);
}

// Returns: Ok if the Conversion has been parsed, Err if the Conversion still requires more characters to be parsed
// WARNING: Assumes char is part of a specification (a.k.a after a '%' character)
// TODO: Support sets in specifier
// WARNING: Does not assume order of parts, so while the spec lists the assignment-suppressing char before the length, 
// this will, for example, parse the assignment-supressing char even if it appears after the already parsed length, so for example %l* is ok which is *fine* since it's undefined behavior ( C99 specification 7.19.6.2, paragraph 13: "If a conversion specification is invalid, the behavior is undefined."  )
pub fn add_char_to_scanf_specification(mut initial: UnfinishedScanfConversionSpecification, c: u8) -> Result<ScanfConversionSpecification, UnfinishedScanfConversionSpecification> {
    if initial.parsing_width {
        match c {
            b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                if let ScanfConversionWidth::Number(cur_width) = initial.conversion_under_construction.width {
                    initial.conversion_under_construction.width = ScanfConversionWidth::Number(cur_width*10+(c-b'0') as usize);
                }else{
                    // We have started parsing the conversion width, but haven't initialized it to be a Number
                    // What?
                    // That should be impossible since we always initialize it to Number before setting parsing_conversion_width
                    // Eh.. forget about it, just don't parse the conversion
                    // FIXME: Better logging
                    initial.conversion_under_construction.width = ScanfConversionWidth::None;
                    initial.parsing_width = false;
                }
                return Err(initial);
            }

            _ => {
                // Encountered non digit character, finish parsing conversion width number
                initial.parsing_width = false;
                // And try to use the character for something else so don't return
            }
        }
    }

    if initial.parsing_length {
        initial.parsing_length = false;
        match c {
            b'l' => {
                if initial.conversion_under_construction.length == ConversionLength::Long {
                    initial.conversion_under_construction.length = ConversionLength::LongLong; 
                    return Err(initial);
                } else { 
                    // Some other character (probably h) followed by l?, so hl?, no. it must be a apart of something else, don't return, and assume that the length is just the first character
                    // FIXME: Better logging
                }
            }
            
            b'h' => {
                if initial.conversion_under_construction.length == ConversionLength::Half {
                    initial.conversion_under_construction.length = ConversionLength::Byte; 
                    return Err(initial);
                } else {
                    // Some other character (probably l) followed by b?, so lh?, no. it must be a apart of something else, don't return, and assume that the length is just the first character
                    // FIXME: Better logging
                }
            }

            _ => {
                // Encountered a normal character not part of a double character 'll' or 'hh'
                // Assume we have already parsed the full length and are now finished
                // Try to use the character for something else so don't return
            }
        }
    }

    // If we have no assignment suppression then maybe this characters marks that
    if initial.conversion_under_construction.assignment_suppression == false {
        match c {
            b'*' => {
                initial.conversion_under_construction.assignment_suppression = true;
                return Err(initial); // We parsed this character, return
            }
            _ => {} // Try something else so don't return
        }
    }

    // If we have no width then maybe this character is part of the width
    if initial.conversion_under_construction.width == ScanfConversionWidth::None {
        match c {
            b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                // Add digit to width
                // and mark the start of parsing the width as a number
                initial.conversion_under_construction.width = ScanfConversionWidth::Number((c-b'0') as usize);
                initial.parsing_width = true;
                return Err(initial); // We parsed this character, return
            }
            _ => {} // TThe character was not part of the width, try something else, don't return
        }
    }

    // If we have no length yet then maybe this character is part of the length
    if initial.conversion_under_construction.length == ConversionLength::None {
        match c {
            b'h' => { initial.conversion_under_construction.length = ConversionLength::Half; initial.parsing_length = true; /* set parsing_length because it could be hh */ return Err(initial); }
            b'l' => { initial.conversion_under_construction.length = ConversionLength::Long; initial.parsing_length = true; /* set parsing_length because it could be ll */ return Err(initial); }
            b'j' => { initial.conversion_under_construction.length = ConversionLength::MaxLengthForInt; return Err(initial); }
            b'z' => { initial.conversion_under_construction.length = ConversionLength::MaxLengthForSize; return Err(initial); }
            b't' => { initial.conversion_under_construction.length = ConversionLength::MaxLengthForPointer; return Err(initial); }
            b'L' => { initial.conversion_under_construction.length = ConversionLength::Double; return Err(initial); }
            _ => {} // The character was not part of the length, try something else, don't return
        }
    }

    // If we have no specifier yet then maybe this character is part of the specifier
    if initial.conversion_under_construction.specifier == ConversionSpecifier::Unparsed {
        if let Ok(val) = ConversionSpecifier::try_from(c) {
            initial.conversion_under_construction.specifier = val;

            // Once we find the specifier we have finished parsing the conversion
            return Ok(initial.conversion_under_construction);
        }
    }

    // Unknown character, ignore it
    // FIXME: Better logging
    return Err(initial);
}