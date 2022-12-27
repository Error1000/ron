#![no_std]
#![feature(c_size_t)]
#![feature(c_variadic)]

use core::{ptr::null_mut, ffi::VaList, ops::{DivAssign, Rem}};

pub mod cstr;
pub mod mem;
pub mod sys;
pub mod specifier_parsing;

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
                ConversionSpecifier::SignedDecimalInteger | ConversionSpecifier::SignedInteger => { // 'd' or 'i'
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

                ConversionSpecifier::UnsignedDecimalInteger => { // 'u'
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

                ConversionSpecifier::UnsignedOctalInteger => { // 'o'
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

                ConversionSpecifier::UnsignedHexIntegerLowerCase => { // 'x'
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

                ConversionSpecifier::UnsignedHexIntegerUpperCase => { // 'X'
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

                ConversionSpecifier::Character => { // 'c'
                    let character_arg = args.arg::<core::ffi::c_char>();
                    let bytes_written = write((*f).fileno, &character_arg, 1);
                    if bytes_written < 1 {
                        return -1;
                    }else{
                        characters_transmitted += 1;
                    }        
                },

                ConversionSpecifier::String => { // 's'
                    let string_arg = args.arg::<*mut core::ffi::c_char>();
                    let string_arg_len = strlen(string_arg);
                    let bytes_written = write((*f).fileno, string_arg, string_arg_len as usize);
                    if bytes_written < string_arg_len as isize {
                        return -1;
                    }else{
                        characters_transmitted += string_arg_len as core::ffi::c_int;
                    } 
                },

                ConversionSpecifier::Pointer => { // 'p'
                    let n = args.arg::<*const core::ffi::c_void>();
                    // 4 = log2(16)
                    let mut output_str = [b'?'; ((core::mem::size_of::<core::ffi::c_size_t>()*8)/4) as usize + 1 + 2 /* for the 0x */];
                    let mut ind = number_to_string_in_radix(&mut output_str, n as usize, 16, Casing::Lower);
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

                ConversionSpecifier::Escape => { // '%'
                    let bytes_written = write((*f).fileno, "%".as_ptr() as *const core::ffi::c_char, 1);
                    if bytes_written < 1 {
                        return -1;
                    }else{
                        characters_transmitted += 1;
                    }
                },

                ConversionSpecifier::Meta => { // 'n'
                    *args.arg::<*mut core::ffi::c_int>() = characters_transmitted;
                }

                ConversionSpecifier::DecimalFloatLowerCase => unimplemented!("Implement printf specification 'f'!"),
                ConversionSpecifier::DeicmalFloatUpperCase => unimplemented!("Implement printf specification 'F'!"),
                ConversionSpecifier::ScientificNotationLowerCase => unimplemented!("Implement printf specification 'e'!"),
                ConversionSpecifier::ScientificNotationUpperCase => unimplemented!("Implement printf specification 'E'!"),
                ConversionSpecifier::ShortestFloatLowerCase => unimplemented!("Implement printf specification 'g'!"),
                ConversionSpecifier::ShortestFloatUpperCase => unimplemented!("Implement printf specification 'G'!"),
                ConversionSpecifier::HexFloatLowerCase => unimplemented!("Implement printf specification 'a'!"),
                ConversionSpecifier::HexFloatUpperCase => unimplemented!("Implement printf specification 'A'!"),
                ConversionSpecifier::Unparsed => panic!("Impossible printf state, conversion specifer is still unparsed even though the parsing finished!"),
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
    let mut characters_read: usize = 0;
    if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}

    for i in 0..format_str_len {
        let mut should_advance_stream = true;
        let format_char = *format_str.add(i as usize) as u8;

        if !parsing_conversion_specification {
            if isspace(format_char as core::ffi::c_int) != 0 { // Whitespace characters
                // Read until stream_char is no longer whitespace, but we still need to process the char that is not whitespace so mark an over-read
                while isspace(stream_char as core::ffi::c_int) != 0 {
                    if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                }

                should_advance_stream = false; // We over-read
            } else if format_char != b'%' { // Non-whitespace characters except %
                if stream_char != format_char {
                    return arguments_assigned.unwrap_or(EOF);
                }
            } else if format_char == b'%' {
                should_advance_stream = false;  // Don't consume char from stream while parsing specification
                parsing_conversion_specification = true;
                // Set up specification under construction
                // TODO: The rest of the code should always leave the specification under construction in the default state by this point anyways
                specification_under_construction = UnfinishedScanfConversionSpecification::default();
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
            // Do the actual formatting
            // FIXME: Implement precision and finish all specifiers

            #[derive(PartialEq)]
            enum ParsedSign {
                POSITIVE,
                NEGATIVE
            }

            #[derive(PartialEq)]
            enum ParsedBase {
                BASE10,
                BASE8,
                BASE16
            }

            fn char_to_digit(c: u8) -> Option<u8> {
                Some(match c {
                    b'0' => 0, 
                    b'1' => 1,
                    b'2' => 2, 
                    b'3' => 3, 
                    b'4' => 4, 
                    b'5' => 5, 
                    b'6' => 6, 
                    b'7' => 7, 
                    b'8' => 8, 
                    b'9' => 9, 
                    b'a' | b'A' => 10,
                    b'b' | b'B' => 11,
                    b'c' | b'C' => 12,
                    b'd' | b'D' => 13,
                    b'e' | b'E' => 14,
                    b'f' | b'F' => 15,
                    _ => return None
                })
            }

            match specification.specifier {
                ConversionSpecifier::Escape => { // '%'
                    if stream_char != b'%' {
                        return arguments_assigned.unwrap_or(0);
                    }
                }

                ConversionSpecifier::Character => { // 'c'
                    // FIXME: Maybe don't allow the opportunity to write to null, but to be fair right now the only alternative that i can think of is duplicating the entire logic which also seems iffy
                    let c = if !specification.assignment_suppression { args.arg::<*mut core::ffi::c_char>() } else { core::ptr::null_mut() };
                    if let ScanfConversionWidth::Number(len) = specification.width {
                        if !specification.assignment_suppression { *c = stream_char as i8; }
                        for i in 1..len {
                            if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                            if !specification.assignment_suppression { *(c.add(i)) = stream_char as i8; }
                        }
                    }else {
                        if !specification.assignment_suppression { *c = stream_char as i8; }
                    }

                    arguments_assigned = if let Some(val) = arguments_assigned { Some(val+1) } else { Some(1) };
                }

                ConversionSpecifier::String => { // 's'
                    // FIXME: Maybe don't allow the opportunity to write to null, but to be fair right now the only alternative that i can think of is duplicating the entire logic which also seems iffy
                    let s = if !specification.assignment_suppression{ args.arg::<*mut core::ffi::c_char>() } else { core::ptr::null_mut() };
                    let mut s_pos = 0;
                    if let ScanfConversionWidth::Number(len) = specification.width {
                        if !specification.assignment_suppression { *(s.add(s_pos)) = stream_char as i8; }
                        s_pos += 1;

                        let mut found_space = false;
                        for _ in 1..len {
                            if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                            if isspace(stream_char.into()) == 0 { 
                                if !specification.assignment_suppression { *(s.add(s_pos)) = stream_char as i8; }
                                s_pos += 1; 
                            } else { 
                                found_space = true; 
                                break; 
                            }
                        }

                        if found_space {
                            // If we found a space then we over-read
                            should_advance_stream = false;
                        }
                    }else{
                        while isspace(stream_char.into()) == 0 {
                            if !specification.assignment_suppression { *(s.add(s_pos)) = stream_char as i8; }
                            s_pos += 1;
                            if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                        }
                        should_advance_stream = false; // We always read until we find a space so we always over-read
                    }

                    if !specification.assignment_suppression { *(s.add(s_pos)) = b'\0' as i8; }
                    arguments_assigned = if let Some(val) = arguments_assigned { Some(val+1) } else { Some(1) };
                }

                ConversionSpecifier::SignedDecimalInteger | ConversionSpecifier::UnsignedDecimalInteger => { // 'd' / 'u'
                    // Read a number with optional + or -

                    let mut number_sign = ParsedSign::POSITIVE;
                    if stream_char == b'+' || stream_char == b'-' {
                        if stream_char == b'+' { number_sign = ParsedSign::POSITIVE; } else if stream_char == b'-' { number_sign = ParsedSign::NEGATIVE; }
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                    }

                    let mut parsed_n = None;

                    while stream_char == b'0' || stream_char == b'1' || stream_char == b'2' || stream_char == b'3' || stream_char == b'4' || stream_char == b'5' || stream_char == b'6' || stream_char == b'7' || stream_char == b'8' || stream_char == b'9' {
                        if let Some(val) = parsed_n {
                            parsed_n = Some(val*10 + char_to_digit(stream_char).unwrap() as i32);
                        } else {
                            parsed_n = Some(char_to_digit(stream_char).unwrap() as i32);
                        }
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                    }

                    if number_sign == ParsedSign::NEGATIVE { parsed_n = parsed_n.map(|val| -val);}
                    should_advance_stream = false; // We read until stream_char is no loner a digit, but we still need to parse the non-digit we over-read
                    if let Some(val) = parsed_n { if !specification.assignment_suppression { *args.arg::<*mut core::ffi::c_int>() = val; } } else { return arguments_assigned.unwrap_or(0); }
                    arguments_assigned = if let Some(val) = arguments_assigned { Some(val+1) } else { Some(1) };
                }

                ConversionSpecifier::SignedInteger => { // 'i'
                    // Read a number with optional + or - and possible base marking ("0x"/"0")

                    let mut number_sign = ParsedSign::POSITIVE;
                    if stream_char == b'+' || stream_char == b'-' {
                        if stream_char == b'+' { number_sign = ParsedSign::POSITIVE; } else if stream_char == b'-' { number_sign = ParsedSign::NEGATIVE; }
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                    }
                        
                    let mut number_base = ParsedBase::BASE10;
                    if stream_char == b'0' {
                        number_base = ParsedBase::BASE8;
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                        if stream_char == b'x' || stream_char == b'X' {
                            number_base = ParsedBase::BASE16;
                            if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                        }else{
                            // Nothing as it could still be base8 so don't assume matching failure just yet
                        }
                    }
                        
                    let mut parsed_n = None;
                    match number_base {
                        ParsedBase::BASE10 => 
                            while stream_char == b'0' || stream_char == b'1' || stream_char == b'2' || stream_char == b'3' || stream_char == b'4' || stream_char == b'5' || stream_char == b'6' || stream_char == b'7' || stream_char == b'8' || stream_char == b'9' {
                                if let Some(val) = parsed_n {
                                    parsed_n = Some(val*10 + char_to_digit(stream_char).unwrap() as i32);
                                }else{
                                    parsed_n = Some(char_to_digit(stream_char).unwrap() as i32);
                                }
                                if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                            }

                        ParsedBase::BASE8 =>
                            while stream_char == b'0' || stream_char == b'1' || stream_char == b'2' || stream_char == b'3' || stream_char == b'4' || stream_char == b'5' || stream_char == b'6' || stream_char == b'7' {
                                if let Some(val) = parsed_n {
                                    parsed_n = Some(val*8 + char_to_digit(stream_char).unwrap() as i32);
                                }else{
                                    parsed_n = Some(char_to_digit(stream_char).unwrap() as i32);
                                }                                    
                                if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                            }

                        ParsedBase::BASE16 => 
                            while stream_char == b'0' || stream_char == b'1' || stream_char == b'2' || stream_char == b'3' || stream_char == b'4' || stream_char == b'5' || stream_char == b'6' || stream_char == b'7' || stream_char == b'8' || stream_char == b'9' 
                                || (stream_char == b'a' || stream_char == b'A') || (stream_char == b'b' || stream_char == b'B') || (stream_char == b'c' || stream_char == b'C') || (stream_char == b'd' || stream_char == b'D') || (stream_char == b'e' || stream_char == b'E') || (stream_char == b'F' || stream_char == b'F') {
                                if let Some(val) = parsed_n {
                                    parsed_n = Some(val*16 + char_to_digit(stream_char).unwrap() as i32);
                                }else{
                                    parsed_n = Some(char_to_digit(stream_char).unwrap() as i32);
                                }
                                if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                            }
                    }
                        
                    if number_sign == ParsedSign::NEGATIVE { parsed_n = parsed_n.map(|val| -val); }
                    should_advance_stream = false; // We read until stream_char is no loner a digit, but we still need to parse the non-digit we over-read
                    if let Some(val) = parsed_n { if !specification.assignment_suppression { *args.arg::<*mut core::ffi::c_int>() = val; } } else { return arguments_assigned.unwrap_or(0); }
                    arguments_assigned = if let Some(val) = arguments_assigned { Some(val+1) } else { Some(1) };
                }


                ConversionSpecifier::UnsignedOctalInteger => { // 'o'
                    // Read a number with optional + or - and possible base marking ("0")

                    let mut number_sign = ParsedSign::POSITIVE;
                    if stream_char == b'+' || stream_char == b'-' {
                        if stream_char == b'+' { number_sign = ParsedSign::POSITIVE; } else if stream_char == b'-' { number_sign = ParsedSign::NEGATIVE; }
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                    }
                    
                    if stream_char == b'0' {
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                    }
                    
                    let mut parsed_n = None;

                    while stream_char == b'0' || stream_char == b'1' || stream_char == b'2' || stream_char == b'3' || stream_char == b'4' || stream_char == b'5' || stream_char == b'6' || stream_char == b'7' {
                        if let Some(val) = parsed_n {
                            parsed_n = Some(val*8 + char_to_digit(stream_char).unwrap() as i32);
                        }else{
                            parsed_n = Some(char_to_digit(stream_char).unwrap() as i32);
                        }                                    
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                    }

                    if number_sign == ParsedSign::NEGATIVE { parsed_n = parsed_n.map(|val| -val); }
                    should_advance_stream = false; // We read until stream_char is no loner a digit, but we still need to parse the non-digit we over-read
                    if let Some(val) = parsed_n { if !specification.assignment_suppression { *args.arg::<*mut core::ffi::c_int>() = val; } } else { return arguments_assigned.unwrap_or(0); }
                    arguments_assigned = if let Some(val) = arguments_assigned { Some(val+1) } else { Some(1) };
                }


                ConversionSpecifier::Pointer => { // 'p'
                    // Read a number with base marking ("0x")

                    if stream_char == b'0' {
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                        if stream_char == b'x' || stream_char == b'X' {
                            if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                        }else{
                            return arguments_assigned.unwrap_or(0); // We can't parse octal when told to parse hex
                        }
                    }else{
                        return arguments_assigned.unwrap_or(0); // We can't parse octal when told to parse hex
                    }
                
                    let mut parsed_n = None;

                    while stream_char == b'0' || stream_char == b'1' || stream_char == b'2' || stream_char == b'3' || stream_char == b'4' || stream_char == b'5' || stream_char == b'6' || stream_char == b'7' || stream_char == b'8' || stream_char == b'9' 
                    || (stream_char == b'a' || stream_char == b'A') || (stream_char == b'b' || stream_char == b'B') || (stream_char == b'c' || stream_char == b'C') || (stream_char == b'd' || stream_char == b'D') || (stream_char == b'e' || stream_char == b'E') || (stream_char == b'F' || stream_char == b'F') {
                        if let Some(val) = parsed_n {
                            parsed_n = Some(val*16 + char_to_digit(stream_char).unwrap() as usize);
                        }else{
                            parsed_n = Some(char_to_digit(stream_char).unwrap() as usize);
                        }
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                    }

                    should_advance_stream = false; // We read until stream_char is no loner a digit, but we still need to parse the non-digit we over-read
                    if let Some(val) = parsed_n { if !specification.assignment_suppression { *args.arg::<*mut *mut core::ffi::c_void>() = val as *mut core::ffi::c_void; } } else { return arguments_assigned.unwrap_or(0); }
                    arguments_assigned = if let Some(val) = arguments_assigned { Some(val+1) } else { Some(1) };
                }


                ConversionSpecifier::UnsignedHexIntegerLowerCase | ConversionSpecifier::UnsignedHexIntegerUpperCase => { // 'x' / 'X'
                    // Read a number with optional + or - and possible base marking ("0x")

                    let mut number_sign = ParsedSign::POSITIVE;
                    if stream_char == b'+' || stream_char == b'-' {
                        if stream_char == b'+' { number_sign = ParsedSign::POSITIVE; } else if stream_char == b'-' { number_sign = ParsedSign::NEGATIVE; }
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                    }
            
                    if stream_char == b'0' {
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                        if stream_char == b'x' || stream_char == b'X' {
                            if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                        }else{
                            return arguments_assigned.unwrap_or(0); // We can't parse octal when told to parse hex
                        }
                    }
            
                    let mut parsed_n = None;

                    while stream_char == b'0' || stream_char == b'1' || stream_char == b'2' || stream_char == b'3' || stream_char == b'4' || stream_char == b'5' || stream_char == b'6' || stream_char == b'7' || stream_char == b'8' || stream_char == b'9' 
                    || (stream_char == b'a' || stream_char == b'A') || (stream_char == b'b' || stream_char == b'B') || (stream_char == b'c' || stream_char == b'C') || (stream_char == b'd' || stream_char == b'D') || (stream_char == b'e' || stream_char == b'E') || (stream_char == b'F' || stream_char == b'F') {
                        if let Some(val) = parsed_n {
                            parsed_n = Some(val*16 + char_to_digit(stream_char).unwrap() as i32);
                        }else{
                            parsed_n = Some(char_to_digit(stream_char).unwrap() as i32);
                        }
                        if read((*f).fileno, (&mut stream_char) as *mut u8 as *mut core::ffi::c_char, 1) < 0 { return arguments_assigned.unwrap_or(EOF); } else { characters_read += 1;}
                    }

                    if number_sign == ParsedSign::NEGATIVE { parsed_n = parsed_n.map(|val| -val); }
                    should_advance_stream = false; // We read until stream_char is no loner a digit, but we still need to parse the non-digit we over-read
                    if let Some(val) = parsed_n { if !specification.assignment_suppression { *args.arg::<*mut core::ffi::c_int>() = val; } } else { return arguments_assigned.unwrap_or(0); }
                    arguments_assigned = if let Some(val) = arguments_assigned { Some(val+1) } else { Some(1) };
                }

                ConversionSpecifier::Meta => { // 'n'
                    if !specification.assignment_suppression {
                        *args.arg::<*mut core::ffi::c_uint>() = characters_read as u32; 
                    }
                    should_advance_stream = false; // Meta doesn't consume anything
                }

                ConversionSpecifier::DecimalFloatLowerCase => unimplemented!("Implement scanf specification 'f'!"),
                ConversionSpecifier::DeicmalFloatUpperCase => unimplemented!("Implement scanf specification 'F'!"),
                ConversionSpecifier::ScientificNotationLowerCase => unimplemented!("Implement scanf specification 'e'!"),
                ConversionSpecifier::ScientificNotationUpperCase => unimplemented!("Implement scanf specification 'E'!"),
                ConversionSpecifier::ShortestFloatLowerCase => unimplemented!("Implement scanf specification 'g'!"),
                ConversionSpecifier::ShortestFloatUpperCase => unimplemented!("Implement scanf specification 'G'!"),
                ConversionSpecifier::HexFloatLowerCase => unimplemented!("Implement scanf specification 'a'!"),
                ConversionSpecifier::HexFloatUpperCase => unimplemented!("Implement scanf specification 'A'!"),
                ConversionSpecifier::Unparsed => panic!("Impossible scanf state, conversion specifer is still unparsed even though the parsing finished!"),
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
    // If fgets() returns NULL, the destination array may have been changed and may not have a null character. Never rely on the array after getting NULL from fgets().
    // Source: https://stackoverflow.com/questions/1660228/does-fgets-always-terminate-the-char-buffer-with-0

    let count = count as usize;
    let mut i = 0;
    loop {
        let res = read((*file).fileno, str.add(i), 1);
        if res == 0 {
            break; // We reached EOF
        } else if res < 0 {
            return null_mut(); // Error occurred
        }
        if *str.add(i) == b'\n' as i8 { break; }
        i += 1;
        if i >= count-1 { break; }
    }
    *str.add(i) = b'\0' as i8;
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