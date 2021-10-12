#![no_std]
#![no_main]
#![feature(bench_black_box)]
#![feature(asm)]

use uart_16550::UARTDevice;
use vga::{Text80x25, Vga};

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
mod integrated_shell;

#[panic_handler]
fn panic(_p: &::core::panic::PanicInfo) -> ! {
    loop {}
}

const MULTIBOOT_MAGIC: u32 = 0x1badb002;
const MULTIBOOT_FLAGS: u32 = 0x00000000;

#[no_mangle]
pub static multiboot_header: [u32; 3] = [
    MULTIBOOT_MAGIC,
    MULTIBOOT_FLAGS,
    -((MULTIBOOT_MAGIC + MULTIBOOT_FLAGS) as i32) as u32,
    // Exec info
    //	0, 0, 0, 0, 0,
    // Graphics request omitted because it is not guaranteed
    //	0, 0, 0, 0
];

// Can't be bothered to make smart implementations
// I'm just gonna let the compiler babysit me for now
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        *dest.offset(i as isize) = *src.offset(i as isize);
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    for i in 0..n {
        *(dest.offset(i as isize)) = c as u8;
    }
    dest
}

#[repr(C)]
#[repr(packed)]
struct MultibootInfo {
    flags: u32,
}

/// TODO:
/// Clean-room rewrite vendored vga

fn kprint(s: &str, uart: &mut UARTDevice) {
    s.chars().for_each(|c| {
        uart.send(c as u8);
        if c == '\n' {
            uart.send(b'\r')
        }
    });
}
fn kprint_u8(data: u8, buf: &mut [u8], uart: &mut UARTDevice) {
    u8_to_str_hex(data, buf);
    kprint("0x", uart);
    kprint(unsafe { from_utf8_unchecked(&buf) }, uart);
    kprint(" ", uart);
}

fn u8_to_str_decimal(val: u8, s: &mut [u8]) {
    // N.B.: Be careful about direction, you might get mirrored numbers otherwise and not realise and spend several hours debugging why your driver doesn't work because it doesn't read the right value even though it does it's just the debugging that's broken >:(
    s[0] = (val / 100) + b'0';
    s[1] = (val / 10) % 10 + b'0';
    s[2] = (val) % 10 + b'0';
}

fn u8_to_str_hex(val: u8, s: &mut [u8]) {
    s[0] = val / 16 + if val / 16 >= 10 { b'A' - 10 } else { b'0' };
    s[1] = val % 16 + if val % 16 >= 10 { b'A' - 10 } else { b'0' };
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
    if eax != 0x2BADB002 { panic!("Kernel was not booted by a a multiboot-compatible bootloader! ") }
    // let m: &MultibootInfo = unsafe{&*(ebx as *const MultibootInfo)};
    let mut uart = unsafe { uart_16550::UARTDevice::unsafe_default() };
    uart.init();
    kprint("Hello, world!\n", &mut uart);
    let vga = unsafe { Vga::unsafe_default() };

    // Switch to 80x25 text mode
    let vga = unsafe { vga.blank_screen() };
    let vga = unsafe { vga.set_mode::<Text80x25>() };
    let mut vga = unsafe { vga.unblank_screen() };

    // Clear screen
    for y in 0..25 {
        for x in 0..80 {
            unsafe { vga.write_char(x, y, b' ', 0x0A) }
        }
    }

    b"Hello, world!"
        .iter()
        .enumerate()
        .for_each(|(x, c)| unsafe {
            vga.write_char(x, 0, *c, 0x0A);
        });
    b"VGA!!!".iter().enumerate().for_each(|(x, c)| unsafe {
        vga.write_char(x, 1, *c, 0x0A);
    });


    kprint("Got here!", &mut uart);
    // let mut buf  = [0u8; 3];
    let mut ps2 = unsafe { ps2_8042::PS2Device::unsafe_default() };
    let mut shift = false;
    let mut x = 0isize;
    let mut y = 0isize;
    let mut sub_x_next_time: bool = false;
    let mut multibyte: bool = false;
    loop {
        sub_x_next_time = false;
        if x >= 0{ unsafe{ vga.set_cursor_position(x as usize, y as usize); }}
        let b = unsafe { ps2.read_byte() };
        // kprint_u8(b, &mut buf, &mut uart);
        let mut c = ps2_8042::scan_code_set_1[b as usize];

        if multibyte {
          if b == 0x48 {
              y -= 1;
              if y < 0{ y = 0; }
          } else if b == 0x50{
              y += 1;
              if y > 24 { y = 24; }
          } else if b == 0x4D {
            x += 1;
            if x > 79 { x = 79; }
          }else if b == 0x4B {
            x -= 1;
            if x < 0 { x = 0; }
          }
          multibyte = false;
          continue;
        }

        if b == 0x2A || b == 0x36 {
            shift = true;
        }
        if b == 0xAA || b == 0xB6 {
            shift = false;
        }

        if b == 0xE0 {
          multibyte = true;
        }

        if c == ' ' && b != 0x39 {
          continue;
        }

        if c == '\r' {
            if x > 0 {
                x -= 1;
            } else if y > 0 {
                y -= 1;
                x = 78;
            }
            c = ' ';
            sub_x_next_time = true;
        } else if c == '\n' {
            x = 0;
            y += 1;
            if y > 25 {
              y = 0;
            }
            continue;
        }
        if shift {
            c = c.to_ascii_uppercase();
            c = match c {
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
                _ => c,
            };
        }
        unsafe {
            vga.write_char(x as usize, y as usize, c as u8, 0x0A);
        }
        x += 1;
        if sub_x_next_time { x -= 1; }
        if x > 80 {
            y += 1;
            x = 0;
        }
        if y > 25 {
            y = 0;
        }
    }

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
