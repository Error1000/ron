#![no_std]
#![feature(c_size_t)]
#![feature(c_variadic)]

use core::{ptr::null_mut, ffi::VaList, ops::{DivAssign, Rem}};

pub mod cstr;
pub mod mem;
pub mod sys;

use sys::lseek;

use crate::{
    cstr::{strlen, isspace},
    sys::{close, free, malloc, open, read, write, O_APPEND, O_CREAT, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY},
};

const EOF: core::ffi::c_int = -1;



#[cfg(not(feature = "nostartfiles"))]
#[panic_handler]
fn panic(info: &::core::panic::PanicInfo) -> ! {
    use sys::exit;

    use crate::sys::STDOUT_FILENO;

    if let Some(msg) = info.payload().downcast_ref::<&str>() {
        unsafe{ let _ = write(STDOUT_FILENO as core::ffi::c_int, msg.as_ptr() as *const core::ffi::c_char, msg.len()); }
    }

    unsafe{exit(0xDED)}
}

#[cfg(not(feature = "nostartfiles"))]
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    use crate::sys::{read_argc, read_argv, setup_general_pointer};
    use sys::exit;

    setup_general_pointer();

    exit(main(read_argc(), read_argv()))
}

#[cfg(not(feature = "nostartfiles"))]
extern "C" {
    pub fn main(argc: core::ffi::c_int, argv: *const *const core::ffi::c_char) -> core::ffi::c_int;
}


mod specifier_parsing {
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
                _ => {} // Try something else so don't return
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

}

#[no_mangle]
pub unsafe extern "C" fn vfprintf(f: *mut FILE, format_str: *const core::ffi::c_char, mut args: VaList) -> core::ffi::c_int {
    // The format string consists of ordinary multibyte characters (except %), which are copied unchanged into the output stream, and conversion specifications
    // Returns:
    // number of characters transmitted to the output stream or negative value if an output error or an encoding error (for string and character conversion specifiers) occurred
    // Source: https://en.cppreference.com/w/c/io/fprintf and https://cplusplus.com/reference/cstdio/printf/
    let mut characters_transmitted: core::ffi::c_int = 0;
    let format_str_len = strlen(format_str);
    use specifier_parsing::*;
    let mut parsing_conversion_specification = false;
    let mut parsed_specification: Option<PrintfConversionSpecification> = None;
    let mut specification_under_construction  = UnfinishedPrintfConversionSpecification::default();

    for i in 0..format_str_len {
        let format_char = *format_str.add(i as usize) as u8;
        // This parses the conversion specification, absorbing all it's characters
        if !parsing_conversion_specification {
            match format_char {
                b'%' => {
                    // Conversion specification introductory character, starts a conversion specification
                    parsing_conversion_specification = true;
                    // Set up specification under construction ( should be unnecessary )
                    specification_under_construction = UnfinishedPrintfConversionSpecification::default();
                },
                _ => {
                    let bytes_written = write((*f).fileno, format_str.add(i as usize), 1);
                    if bytes_written < 1 {
                        return -1;
                    }else{
                        characters_transmitted += 1;
                    }
                }
            }
        }else{
            // Character c is after a %, a.k.a is a part of a Conversion specification
            specification_under_construction = 
            match add_char_to_printf_specification(specification_under_construction, format_char) {
                Err(new) => new,
                Ok(finished_specification) => {
                    parsed_specification = Some(finished_specification);
                    parsing_conversion_specification = false;
                    UnfinishedPrintfConversionSpecification::default()
                }
            }
        }

        // Once the last character is parsed, the execution of the parsed specification starts immediately, 
        // while i is still on the last character of the specification ( not after it )
        if let Some(specification) = parsed_specification {
            // Do the actual formatting
            // FIXME: Implement width, flags and precision and finish all specifiers
            enum Casing { Lower, Upper}

            // Writes "n" to "output_str", in radix specified by "base"
            // SAFTEY: Assumes that "output_str" is big enough to contain all the digits of "n"
            // Returns: the index of the left-most digit - 1
            unsafe fn number_to_string_in_radix<T>(output_str: &mut [u8], mut n: T, base: T, case: Casing) -> usize 
            where T: Ord + From<u8> + DivAssign + Rem + Copy,
                  u8: TryFrom<<T as Rem>::Output> {
                // We start with the last digit ( the digit most to the right )
                let mut ind = output_str.len()-1;
                while n > T::from(0) {
                    // Maps the last digit of the number to a character
                    let last_digit_char = 
                    match u8::try_from(n%base).unwrap_unchecked() {
                        0 => b'0', 1 => b'1', 2 => b'2', 3 => b'3', 4 => b'4', 5 => b'5', 6 => b'6', 7 => b'7', 8 => b'8', 9 => b'9',
                        10 => match case { Casing::Lower => b'a', Casing::Upper => b'A' }
                        11 => match case { Casing::Lower => b'b', Casing::Upper => b'B' }
                        12 => match case { Casing::Lower => b'c', Casing::Upper => b'C' }
                        13 => match case { Casing::Lower => b'd', Casing::Upper => b'D' }
                        14 => match case { Casing::Lower => b'e', Casing::Upper => b'E' }
                        15 => match case { Casing::Lower => b'f', Casing::Upper => b'F' }
                        _ => panic!("Radix of number in printf too big!")
                    };
                    output_str[ind] = last_digit_char;
                    ind -= 1;
                    n /= base;
                }
                return ind; 
            }

    
            match specification.specifier {
                ConversionSpecifier::SignedDecimalInteger | ConversionSpecifier::SignedInteger => {
                    let mut n = args.arg::<core::ffi::c_int>();
                    let is_negative = n < 0;
                    n = n.abs(); // We will always parse the number as if it is positive and then put the sign afterwards

                    // 3.32192809488736234 = log2(10)
                    // (core::mem::size_of::<core::ffi::c_int>()*8) = log2(maximum value)
                    // so we have log2(MAX)/log2(10) = log10(MAX),
                    // floor(log10(n))+1 is the number of digits in base 10 that n has.
                    let mut output_str = [b'?'; ((core::mem::size_of::<core::ffi::c_int>()*8) as f64/3.32192809488736234f64) as usize + 1 + 1 /* sign */];
                    let mut ind = number_to_string_in_radix(&mut output_str, n, 10, Casing::Lower/*irrelevant for any base <= 10*/);
                    if is_negative { output_str[ind] = b'-'; ind -= 1; /* make sure we keep ind one to the left of the beginning, as that is how it will be if there is no sign */ }

                    let amount_of_str_to_write = output_str.len()-(ind+1);
                    let bytes_written = write((*f).fileno, (output_str.as_ptr() as *const core::ffi::c_char).add(ind+1), amount_of_str_to_write);
                    if bytes_written < amount_of_str_to_write as isize {
                        return -1;
                    }else{
                        characters_transmitted += amount_of_str_to_write as core::ffi::c_int;
                    }
                },

                ConversionSpecifier::UnsignedDecimalInteger => {
                    let n = args.arg::<core::ffi::c_uint>();
                    // 3.32192809488736234 = log2(10)
                    let mut output_str = [b'?'; ((core::mem::size_of::<core::ffi::c_uint>()*8) as f64/3.32192809488736234f64) as usize + 1];
                    let ind = number_to_string_in_radix(&mut output_str, n, 10, Casing::Lower/*irrelevant for any base <= 10*/);
                    
                    let amount_of_str_to_write = output_str.len()-(ind+1);
                    let bytes_written = write((*f).fileno, (output_str.as_ptr() as *const core::ffi::c_char).add(ind+1), amount_of_str_to_write);
                    if bytes_written < amount_of_str_to_write as isize {
                        return -1;
                    }else{
                        characters_transmitted += amount_of_str_to_write as core::ffi::c_int;
                    }
                },

                ConversionSpecifier::UnsignedOctalInteger => {
                    let n = args.arg::<core::ffi::c_uint>();
                    // 3 = log2(8)
                    let mut output_str = [b'?'; ((core::mem::size_of::<core::ffi::c_uint>()*8)/3) as usize + 1];
                    let ind = number_to_string_in_radix(&mut output_str, n, 8, Casing::Lower /*irrelevant for any base <= 10*/);

                    let amount_of_str_to_write = output_str.len()-(ind+1);
                    let bytes_written = write((*f).fileno, (output_str.as_ptr() as *const core::ffi::c_char).add(ind+1), amount_of_str_to_write);
                    if bytes_written < amount_of_str_to_write as isize {
                        return -1;
                    }else{
                        characters_transmitted += amount_of_str_to_write as core::ffi::c_int;
                    }
                },

                ConversionSpecifier::UnsignedHexIntegerLowerCase => {
                    let n = args.arg::<core::ffi::c_uint>();
                    // 4 = log2(16)
                    let mut output_str = [b'?'; ((core::mem::size_of::<core::ffi::c_uint>()*8)/4) as usize + 1];
                    let ind = number_to_string_in_radix(&mut output_str, n, 16, Casing::Lower);

                    let amount_of_str_to_write = output_str.len()-(ind+1);
                    let bytes_written = write((*f).fileno, (output_str.as_ptr() as *const core::ffi::c_char).add(ind+1), amount_of_str_to_write);
                    if bytes_written < amount_of_str_to_write as isize {
                        return -1;
                    }else{
                        characters_transmitted += amount_of_str_to_write as core::ffi::c_int;
                    }
                },

                ConversionSpecifier::UnsignedHexIntegerUpperCase => {
                    let n = args.arg::<core::ffi::c_uint>();
                    // 4 = log2(16)
                    let mut output_str = [b'?'; ((core::mem::size_of::<core::ffi::c_uint>()*8)/4) as usize + 1 /* ceil */];
                    let ind = number_to_string_in_radix(&mut output_str, n, 16, Casing::Upper);

                    let amount_of_str_to_write = output_str.len()-(ind+1);
                    let bytes_written = write((*f).fileno, (output_str.as_ptr() as *const core::ffi::c_char).add(ind+1), amount_of_str_to_write);
                    if bytes_written < amount_of_str_to_write as isize {
                        return -1;
                    }else{
                        characters_transmitted += amount_of_str_to_write as core::ffi::c_int;
                    }
                },

                ConversionSpecifier::Character => {
                    let character_arg = args.arg::<core::ffi::c_char>();
                    let bytes_written = write((*f).fileno, &character_arg, 1);
                    if bytes_written < 1 {
                        return -1;
                    }else{
                        characters_transmitted += 1;
                    }        
                },

                ConversionSpecifier::String => {
                    let string_arg = args.arg::<*mut core::ffi::c_char>();
                    let string_arg_len = strlen(string_arg);
                    let bytes_written = write((*f).fileno, string_arg, string_arg_len as usize);
                    if bytes_written < string_arg_len as isize {
                        return -1;
                    }else{
                        characters_transmitted += string_arg_len as core::ffi::c_int;
                    } 
                },

                ConversionSpecifier::Pointer => {
                    let n = args.arg::<core::ffi::c_size_t>();
                    // 4 = log2(16)
                    let mut output_str = [b'?'; ((core::mem::size_of::<core::ffi::c_size_t>()*8)/4) as usize + 1 + 2 /* for the 0x */];
                    let mut ind = number_to_string_in_radix(&mut output_str, n, 16, Casing::Lower);
                    output_str[ind] = b'x';
                    ind -= 1;
                    output_str[ind] = b'0';
                    ind -= 1;

                    let amount_of_str_to_write = output_str.len()-(ind+1);
                    let bytes_written = write((*f).fileno, (output_str.as_ptr() as *const core::ffi::c_char).add(ind+1), amount_of_str_to_write);
                    if bytes_written < amount_of_str_to_write as isize {
                        return -1;
                    }else{
                        characters_transmitted += amount_of_str_to_write as core::ffi::c_int;
                    }
                },

                ConversionSpecifier::Escape => {
                    let bytes_written = write((*f).fileno, "%".as_ptr() as *const core::ffi::c_char, 1);
                    if bytes_written < 1 {
                        return -1;
                    }else{
                        characters_transmitted += 1;
                    }
                },

                ConversionSpecifier::Meta => unimplemented!("Implement printf specification 'n'!"),
                _ => unimplemented!("Implement printf specification!"),
            }
            // We did the actual formatting, so reset the parsed specification
            parsed_specification = None;
        }
    }
    
    return characters_transmitted;
}

#[no_mangle]
pub unsafe extern "C" fn vfscanf(f: *mut FILE, format_str: *const core::ffi::c_char, mut args: VaList) -> core::ffi::c_int {
    // Returns: Number of receiving arguments successfully assigned, or EOF if read failure occurs before the first receiving argument was assigned.
    // Source: https://en.cppreference.com/w/c/io/vfscanf
    let mut arguments_assigned: Option<core::ffi::c_int> = None;
    let format_str_len = strlen(format_str);
    use specifier_parsing::*;
    let mut parsing_conversion_specification = false;
    let mut parsed_specification: Option<ScanfConversionSpecification> = None;
    let mut specification_under_construction  = UnfinishedScanfConversionSpecification::default();

    // The format string consists of:
    // - non-whitespace multibyte characters except %: each such character in the format string consumes exactly one identical character from the input stream, or causes the function to fail if the next character on the stream does not compare equal.
    // - whitespace characters: any single whitespace character in the format string consumes all available consecutive whitespace characters from the input (determined as if by calling isspace in a loop). Note that there is no difference between "\n", " ", "\t\t", or other whitespace in the format string.
    // - conversion specifications
    // Source: https://en.cppreference.com/w/c/io/vfscanf

    let mut stream_char: u8 = 0;
    if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); }

    for i in 0..format_str_len {
        let mut should_advance_stream = true;
        let format_char = *format_str.add(i as usize) as u8;

        if !parsing_conversion_specification {
            if isspace(format_char as core::ffi::c_int) != 0 { // Whitespace characters
                // Read until stream_char is no longer whitespace, but we still need to process that char that is not whitespace so mark an overread
                while isspace(stream_char as core::ffi::c_int) != 0 {
                    if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); }
                }

                should_advance_stream = false; // We over-read
            } else if format_char != b'%' { // Non-whitespace characters except %
                if stream_char != format_char {
                    return arguments_assigned.unwrap_or(EOF);
                }
            } else if format_char == b'%' {
                parsing_conversion_specification = true;
                // Set up specification under construction
                // TODO: The rest of the code should always leave the specification under construction in the default state by this point anyways
                specification_under_construction = UnfinishedScanfConversionSpecification::default();
                should_advance_stream = false;  // Don't consume char from stream while parsing specification 
            }   
        }else{
            should_advance_stream = false; // Don't consume char from stream while parsing specification
            specification_under_construction = match add_char_to_scanf_specification(specification_under_construction, format_char) {
                Err(new) => new,
                Ok(finished_specification) => {
                    parsed_specification = Some(finished_specification);
                    parsing_conversion_specification = false;
                    UnfinishedScanfConversionSpecification::default()
                }
            };
        }

        // Once the last character is parsed, the execution of the parsed specification starts immediately, 
        // while i is still on the last character of the specification ( not after it )
        if let Some(specification) = parsed_specification {
            match specification.specifier {
                ConversionSpecifier::SignedDecimalInteger => {
                    let n = args.arg::<*mut core::ffi::c_int>();
                    // Read a number with optional + or -
                    #[derive(PartialEq)]
                    enum ParsedSign {
                        POSITIVE,
                        NEGATIVE
                    }

                    let mut number_sign = ParsedSign::POSITIVE;
                    if stream_char == b'+' || stream_char == b'-' {
                        if stream_char == b'+' { number_sign = ParsedSign::POSITIVE; } else if stream_char == b'-' { number_sign = ParsedSign::NEGATIVE; }
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); }
                    }

                    let mut parsed_n = 0;

                    while stream_char == b'0' || stream_char == b'1' || stream_char == b'2' || stream_char == b'3' || stream_char == b'4' || stream_char == b'5' || stream_char == b'6' || stream_char == b'7' || stream_char == b'8' || stream_char == b'9' {
                        parsed_n = parsed_n*10 + (stream_char-b'0') as i32;
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); }
                    }
                    if number_sign == ParsedSign::NEGATIVE { parsed_n = -parsed_n;}
                    should_advance_stream = false; // We over-read
                    *n = parsed_n;
                    arguments_assigned = if let Some(val) = arguments_assigned { Some(val+1) } else { Some(1) };
                }
                _ => unimplemented!("Implement scanf specification!")
            }
            // We did the actual formatting, so reset the parsed specification
            parsed_specification = None;
        }

        if should_advance_stream {
            if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); }
        }
    }

    return arguments_assigned.unwrap_or(EOF);
}

#[no_mangle]
pub unsafe extern "C" fn fputs(str: *const core::ffi::c_char, file: *mut FILE) -> core::ffi::c_int {
    // Writes every character from the null-terminated string str to the output stream stream, as if by repeatedly executing fputc.
    // The terminating null character from str is not written. 

    // Returns:
    // On success, returns a non-negative value
    // On failure, returns EOF and sets the error indicator (see ferror()) on stream. 
    // Source: https://en.cppreference.com/w/c/io/fputs
    
    let str_len = strlen(str);
    let res = write((*file).fileno, str, str_len as usize);
    if res == str_len  as isize {
        return 1;
    } else {
        // An error occurred or we couldn't write all of the string 
        return EOF;
    }
}

#[no_mangle]
pub unsafe extern "C" fn fgets(str: *mut core::ffi::c_char, count: core::ffi::c_int, file: *mut FILE) -> *mut core::ffi::c_char {
    let count = count as usize;
    for i in 0..count-1 {
        let res = read((*file).fileno, str.add(i), 1);
        if res == 0 {
            break; // We reached EOF
        } else if res < 0 {
            return null_mut(); // Error occurred
        }
        if *str.add(i) == b'\n' as i8 { break; }
    }
    *str.add(count-1) = b'\0' as i8;
    return str;
}

#[no_mangle]
pub unsafe extern "C" fn puts(str: *const core::ffi::c_char) -> core::ffi::c_int {
    // Writes every character from the null-terminated string str and one additional newline character '\n' to the output stream stdout, as if by repeatedly executing fputc.
    // The terminating null character from str is not written. 
    // Returns:
    // On success, returns a non-negative value
    // On failure, returns EOF and sets the error indicator (see ferror()) on stream
    // Source: https://en.cppreference.com/w/c/io/puts

    let res = write(sys::STDOUT_FILENO as core::ffi::c_int, str, strlen(str) as core::ffi::c_size_t);
    if res < 0 {
        return EOF;
    }
    
    let res = write(sys::STDOUT_FILENO as core::ffi::c_int, (&"\n").as_ptr() as *const core::ffi::c_char, 1);
    if res < 0 {
        return EOF;
    }

    return 1;
}

#[no_mangle]
pub unsafe extern "C" fn perror(str: *const core::ffi::c_char) -> core::ffi::c_int {
    let mut t = 0;
    let res = write(sys::STDERR_FILENO as core::ffi::c_int, str, strlen(str) as core::ffi::c_size_t);
    if res < 0 {
        return res as core::ffi::c_int;
    } else {
        t += res;
    }
    let res = write(sys::STDERR_FILENO as core::ffi::c_int, (&"\n").as_ptr() as *const core::ffi::c_char, 1);
    if res < 0 {
        return res as core::ffi::c_int;
    } else {
        t += res;
    }
    t as core::ffi::c_int
}

#[repr(C)]
pub struct FILE {
    fileno: core::ffi::c_int,
}

#[no_mangle]
pub unsafe extern "C" fn fopen(filename: *const core::ffi::c_char, mode: *const core::ffi::c_char) -> *mut FILE {
    let mode = core::ffi::CStr::from_ptr(mode as *const i8);
    let mode = if let Ok(val) = mode.to_str() {
        val
    } else {
        return null_mut();
    };

    // Transform flags into unix open options and extra options on the FILE struct if necessary
    // FIXME: Ensure that this is standards compliant
    let flags = match (mode.chars().nth(0), mode.chars().nth(1) == Some('+')) {
        (Some('r'), false) => O_RDONLY,
        (Some('w'), false) => O_WRONLY | O_CREAT | O_TRUNC,
        (Some('a'), false) => O_WRONLY | O_APPEND | O_CREAT,

        (Some('r'), true) => O_RDWR,
        (Some('w'), true) => O_RDWR | O_CREAT | O_TRUNC,
        (Some('a'), true) => O_RDWR | O_APPEND | O_CREAT,
        _ => return null_mut(),
    };

    let fd = open(filename, flags as core::ffi::c_int);
    if fd < 0 {
        return null_mut();
    }

    let file_ptr = malloc(core::mem::size_of::<FILE>()) as *mut FILE;
    if file_ptr.is_null() {
        return null_mut();
    }
    *file_ptr = FILE { fileno: fd };
    return file_ptr;
}

#[no_mangle]
pub unsafe extern "C" fn fclose(f: *mut FILE) -> core::ffi::c_int {
    if close((*f).fileno) < 0 {
        return -1;
    }
    free(f as *mut _);
    0
}

#[no_mangle]
pub unsafe extern "C" fn fwrite(
    buf: *const core::ffi::c_char,
    size: core::ffi::c_size_t,
    count: core::ffi::c_size_t,
    f: *mut FILE,
) -> core::ffi::c_size_t {
    let bytes = size * count;
    if bytes == 0 {
        return 0;
    }
    let res = write((*f).fileno, buf, bytes);
    if res < 0 {
        return 0;
    }
    (res as core::ffi::c_size_t) / size
}

#[no_mangle]
pub unsafe extern "C" fn fread(
    buf: *mut core::ffi::c_char,
    size: core::ffi::c_size_t,
    count: core::ffi::c_size_t,
    f: *mut FILE,
) -> core::ffi::c_size_t {
    let bytes = size * count;
    if bytes == 0 {
        return 0;
    }

    let res = read((*f).fileno, buf, bytes);
    if res < 0 {
        return 0;
    }
    (res as core::ffi::c_size_t) / size
}

#[no_mangle]
pub unsafe extern "C" fn fseek(f: *mut FILE, offset: core::ffi::c_long, origin: core::ffi::c_int) -> core::ffi::c_int {
    if lseek(unsafe { &*f }.fileno, offset, origin) > 0 {
        return 0;
    } else {
        return -1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn fputc(ch: core::ffi::c_int, f: *mut FILE) -> core::ffi::c_int {
    // Return value
    // On success, returns the written character.
    // On failure, returns EOF and sets the error indicator (see ferror()) on stream. 
    // Source: https://en.cppreference.com/w/c/io/fputc

    let bytes_written: core::ffi::c_ssize_t = write((*f).fileno, &(ch as core::ffi::c_char), 1);

    if bytes_written <= 0 { 
        // On failure, returns EOF and sets the error indicator (see ferror()) on stream. 
        // FIXME: Set error indicator
        return -1; 
    } else {
        // On success, returns the written character.
        return ch;
    }
}

#[no_mangle]
pub unsafe extern "C" fn fgetc(f: *mut FILE) -> core::ffi::c_int {
    // Returns
    // On success, returns the obtained character as an unsigned char converted to an int. On failure, returns EOF.

    // If the failure has been caused by end-of-file condition, additionally sets the eof indicator (see feof()) on stream. 
    // If the failure has been caused by some other error, sets the error indicator (see ferror()) on stream. 
    // Source: https://en.cppreference.com/w/c/io/fgetc

    // FIXME: Set eof/error indicator
    let mut res: core::ffi::c_char = 0;
    let bytes_read = read((*f).fileno, &mut res, 1);

    if bytes_read <= 0 {
        return -1;
    }else{
        return res as core::ffi::c_int;
    }
}