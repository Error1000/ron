#![no_std]
#![no_main]
#![feature(bench_black_box)]
#![feature(asm)]

use hio::KeyboardPacketType;
use ps2_8042::SpecialKeys;
use uart_16550::UARTDevice;
use vga::{MixedRegisterState, Text80x25, Unblanked, Vga, VgaMode};
use core::convert::TryInto;
use core::convert::TryFrom;

macro_rules! wait_for {
  ($cond:expr) => {
      while !$cond { core::hint::spin_loop() }
  };
}

trait UnsafeDefault { unsafe fn unsafe_default() -> Self; }

mod ps2_8042;
mod uart_16550;
mod vga;
mod virtmem;
mod hio;

#[panic_handler]
fn panic(_p: &::core::panic::PanicInfo) -> ! { loop {} }

const MULTIBOOT_MAGIC: u32 = 0x1badb002;
const MULTIBOOT_FLAGS: u32 = 0x00000000;

#[no_mangle]
pub static multiboot_header: [u32; 3] = [
    MULTIBOOT_MAGIC,
    MULTIBOOT_FLAGS,
    -((MULTIBOOT_MAGIC + MULTIBOOT_FLAGS) as i32) as u32,
    // Exec info ( omitted because i use elf )
    //	0, 0, 0, 0, 0,
    // Graphics request omitted because it is not guaranteed
    //	0, 0, 0, 0
];

// Can't be bothered to make smart implementations
// I'm just gonna let the compiler babysit me for now
// TODO: Make these faster
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n { *dest.offset(i as isize) = *src.offset(i as isize); }
    dest
}

// TODO: This does not behave fully like the standard says
// It always returns as if str2 is less than str1 if they are not equal
#[no_mangle]
pub unsafe extern "C" fn memcmp(str1: *const u8, str2: *const u8, n: usize) -> i32 {
    let mut eq: bool = true;
    for i in 0..n { if *str1.offset(i as isize) != *str2.offset(i as isize) { eq = false; break; } }
    if eq { return 0; }else { return 1; }
}

#[no_mangle]
pub unsafe extern "C" fn bcmp(str1: *const u8, str2: *const u8, n: usize) -> i32 {
    if n == 0 { return 0; }
    memcmp(str1, str2, n) // TODO: Unnecessarily complex
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    for i in 0..n { *(dest.offset(i as isize)) = c as u8; }
    dest
}

fn kprint(s: &str, uart: &mut UARTDevice) {
    s.chars().for_each(|c| {
        uart.send(c as u8);
        if c == '\n' {
            uart.send(b'\r')
        }
    });
}

fn kprintln(s: &str, uart: &mut UARTDevice) {
    kprint(s, uart);
    kprint("\n", uart);
}

fn kprint_u8(data: u8, uart: &mut UARTDevice) {
    let buf = u8_to_str_hex(data);
    kprint("0x", uart);
    kprint(unsafe { from_utf8_unchecked(&buf) }, uart);
    kprint(" ", uart);
}

fn clear_screen(vga: &mut Vga<Text80x25, Unblanked>){
    for y in 0..25 {
        for x in 0..80 {
            unsafe { vga.write_char(x, y, b' ', 0x0F) }
        }
    }
}

fn u8_to_str_decimal(val: u8) -> [u8; 3] {
    let mut s: [u8; 3] = [0; 3];
    // N.B.: Be careful about direction, you might get mirrored numbers otherwise and not realise and spend several hours debugging why your driver doesn't work because it doesn't read the right value even though it does it's just the debugging that's broken >:(
    s[0] = (val / 100) + b'0';
    s[1] = (val / 10) % 10 + b'0';
    s[2] = (val) % 10 + b'0';
    return s;
}

fn u8_to_str_hex(val: u8) -> [u8; 2] {
    let mut s: [u8; 2] = [0; 2];
    s[0] = val / 16 + if val / 16 >= 10 { b'A' - 10 } else { b'0' };
    s[1] = val % 16 + if val % 16 >= 10 { b'A' - 10 } else { b'0' };
    return s;
}

pub const unsafe fn from_utf8_unchecked(v: &[u8]) -> &str {
    // SAFETY: the caller must guarantee that the bytes `v` are valid UTF-8.
    // Also relies on `&str` and `&[u8]` having the same layout.
    core::mem::transmute(v)
}

// EAX and EBX are used for multiboot
#[no_mangle]
pub extern "C" fn main(eax: u32, _ebx: u32) -> ! {
    // Why yes rust i do need the statics i declared, please don't optimise them out of the executable
    core::hint::black_box(core::convert::identity(multiboot_header.as_ptr()));
    if eax != 0x2BADB002 { panic!("Kernel was not booted by a multiboot-compatible bootloader! ") }
    // let m: &MultibootInfo = unsafe{&*(ebx as *const MultibootInfo)};
    let mut uart = unsafe { uart_16550::UARTDevice::unsafe_default() };
    uart.init();
    kprintln("Hello, world!", &mut uart);
    let vga = unsafe { Vga::unsafe_default() };

    // Switch to 80x25 text mode
    let vga = unsafe { vga.blank_screen() };
    let vga = unsafe { vga.set_mode::<Text80x25>() };
    let mut vga = unsafe { vga.unblank_screen() };

    clear_screen(&mut vga);

  


    kprintln("If you see this then that means the vga subsystem didn't instantly crash the kernel :)", &mut uart);

    let mut ps2 = unsafe { ps2_8042::PS2Device::unsafe_default() };
    let mut x = 2isize;
    let mut y = 0isize;
    b"# ".iter().enumerate().for_each(|(i, c)| unsafe {
        vga.write_char(i, y.try_into().expect("Y should have valid value"), *c, 0x0A);
    });

    let mut ignore_inc_x: bool = false;
    // Basically an ad-hoc ArrayString (arrayvec crate)
    let mut cmd_buf: [u8; 80] = [b' '; 80]; 
    let mut buf_ind = 0; // Also length of buf, a.k.a portion of buf used
    loop {
        ignore_inc_x = false;
        if x >= 0 { unsafe{ vga.set_cursor_position(x as usize, y as usize); }}
        let b = unsafe { ps2.read_packet() };
        // kprint_u8(b.scancode, &mut uart);

        if b.typ == KeyboardPacketType::KEY_RELEASED && b.special_keys.contains(SpecialKeys::ESC) { break; }
        if b.typ == KeyboardPacketType::KEY_RELEASED { continue; }

        if b.special_keys.contains(SpecialKeys::UP_ARROW) {
            y -= 1;
            if y < 0{ y = 0; }
        } else if b.special_keys.contains(SpecialKeys::DOWN_ARROW){
            y += 1;
            if y > 24 { y = 24; }
        } else if b.special_keys.contains(SpecialKeys::RIGHT_ARROW) {
          x += 1;
          if x > 79 { x = 79; }
        }else if b.special_keys.contains(SpecialKeys::LEFT_ARROW) {
          x -= 1;
          if x < 0 { x = 0; }
        }

        let mut c = match b.char_codepoint {
            Some(v) => v,
            None => continue
        };

        if c == '\r' {
            ignore_inc_x = true;
            if x > 0 {
                x -= 1;
                buf_ind -= 1;
            } else if y > 0 { // Implements cursor wrapping-back to the end of the last line
                y -= 1;
                x = 77;
            }
            c = ' ';
        } else if c == '\n' {
            x = 0;
            y += 1;
            if y >= 25 {
              y = 0;
            }


            let bufs = unsafe{from_utf8_unchecked(&cmd_buf[..buf_ind])};
            buf_ind = 0; // Flush buffer

            // kprintln( bufs, &mut uart);
            let mut splat = bufs.split_inclusive(' ');
            if let Some(cmnd) = splat.next(){
                // Handle command parsing
                if cmnd.contains("echo"){
                    let  mut lx = 0;
                    while let Some(arg) = splat.next(){
                        arg.chars().enumerate().for_each(|(i, c)| unsafe{
                            vga.write_char(lx + usize::try_from(x).expect("X should have valid value") + i, y.try_into().expect("Y should have valid value"), c as u8, 0x0A);
                        });
                        lx += arg.len();
                    }
                    y += 1;
                }
            }

            // TODO: Should have a single function that writes to vga, that is called both for user input and application text
            b"# ".iter().enumerate().for_each(|(i, c)| unsafe {
                vga.write_char(usize::try_from(x).expect("X should have valid value") + i, y.try_into().expect("Y should have valid value"), *c, 0x0A);
            });            
            x += 2;
            
            continue;
        }

        if b.special_keys.any_shift() { c = b.shift_codepoint().unwrap(); }

        cmd_buf[buf_ind] = c as u8;
        if !ignore_inc_x { buf_ind += 1; }
        unsafe { vga.write_char(x.try_into().expect("X should have valid value"), y.try_into().expect("Y should have valid value"), c as u8, 0x0A); }


        if !ignore_inc_x { x += 1; }

        if x > 80 {
            y += 1;
            x = 0;
        }
        if y > 25 { y = 0; }

    }

    // Shutdown
    kprintln("\nIt's now safe to turn off your computer!", &mut uart);
    clear_screen(&mut vga);
    unsafe{
        let s = "It's now safe to turn off your computer!";
        s.chars().enumerate().for_each(|(ind, c)|vga.write_char(ind+80/2-s.len()/2, 25/2-1/2, c as u8, 0x0E));
    }
    unsafe{ asm!("cli"); }
    loop { unsafe { asm!("hlt"); } }
}
