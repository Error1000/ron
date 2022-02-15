#![no_std]
#![no_main]
#![feature(bench_black_box)]

use hio::KeyboardPacketType;
use ps2_8042::SpecialKeys;
use uart_16550::UARTDevice;
use vga::{MixedRegisterState, Text80x25, Unblanked, Vga};
use core::{arch::asm, mem::{uninitialized, MaybeUninit}, ffi};

macro_rules! wait_for {
  ($cond:expr) => {
      while !$cond { core::hint::spin_loop() }
  };
}

trait X86Default { unsafe fn x86_default() -> Self; }


#[panic_handler]
fn panic(_p: &::core::panic::PanicInfo) -> ! { loop {} }


mod multiboot;
mod ps2_8042;
mod uart_16550;
mod vga;
mod virtmem;
mod hio;
mod ata;

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

fn kprint_u16(data: u16, uart: &mut UARTDevice){
    let buf = u16_to_str_hex(data);
    kprint("0x", uart);
    kprint(unsafe{ from_utf8_unchecked(&buf)}, uart);
    kprint(" ", uart);
}

fn kprint_u32(data: u32, uart: &mut UARTDevice){
    kprint("0x", uart);
    kprint(unsafe{ from_utf8_unchecked(&u16_to_str_hex(((data >> 16) & 0xfff) as u16))}, uart);
    kprint(unsafe{ from_utf8_unchecked(&u16_to_str_hex((data & 0xffff) as u16))}, uart);
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
    // N.B.: Be careful about direction, you might get mirrored numbers otherwise and not realise and spend several hours debugging why your driver doesn't work because it doesn't read the right value even though it does it's just the debugging function that's broken >:(
    s[0] = (val / 100) + b'0';
    s[1] = (val / 10) % 10 + b'0';
    s[2] = (val) % 10 + b'0';
    return s;
}

fn u8_to_str_hex(val: u8) -> [u8; 2] {
    let mut s: [u8; 2] = [0; 2];
    s[0] = val / 16 + if val / 16 >= 10 { b'A' - 10 } else { b'0' };
    s[1] = val % 16 + if val % 16 >= 10 { b'A' - 10 } else { b'0' };
    s
}

fn u16_to_str_hex(val: u16) -> [u8; 4] {
    let mut s: [u8; 4] = [0; 4];
    for i in 0..=3u16 {
        let digit = ((val / 16u16.pow(i.into())) % 16) as u8;
        s[3-i as usize] = digit + if digit >= 10 { b'A' - 10 } else { b'0' };
    }
    s
}

pub const unsafe fn from_utf8_unchecked(v: &[u8]) -> &str {
    // SAFETY: the caller must guarantee that the bytes `v` are valid UTF-8.
    // Also relies on `&str` and `&[u8]` having the same layout.
    core::mem::transmute(v)
}

struct Vga80x25Terminal<'a, STATE: MixedRegisterState> {
    cursor: (usize/*x*/, usize/*y*/),
    color: u8,
    vga: &'a mut Vga<Text80x25, STATE>
}

impl<'a, STATE: MixedRegisterState> Vga80x25Terminal<'a, STATE>{
    pub fn new(vga: &'a mut Vga<Text80x25, STATE>, color: u8) -> Self {
        Self {
            cursor: (0, 0),
            color,
            vga
        }
    }

    pub unsafe fn write_char(&mut self, c: char) {
        match c {
            '\n' => {
                self.cursor.0 = 0;
                if self.cursor.1 < (25-1){
                    self.cursor.1 += 1;
                }else{
                    self.cursor.1 = 0;
                }
                self.vga.set_cursor_position(self.cursor.0, self.cursor.1);
            },
            '\r' => {
                if self.cursor.0 > 0 {
                    self.cursor.0 -= 1;
                } else if self.cursor.1 > 0 {
                    self.cursor.1 -= 1;
                    self.cursor.0 = 79;
                }
                self.vga.write_char(self.cursor.0, self.cursor.1, b' ', self.color);
                self.vga.set_cursor_position(self.cursor.0, self.cursor.1);
            },
            _ => {
                self.vga.write_char(self.cursor.0, self.cursor.1, c as u8, self.color);
                self.cursor_right();
            }
        }
    }

    pub unsafe fn cursor_right(&mut self){
        if self.cursor.0 < (80 - 1) {
            self.cursor.0 += 1;
        }else{
            // Wrap forward to start of next line
            self.cursor_down();
            self.cursor.0 = 0;
        }
        self.vga.set_cursor_position(self.cursor.0, self.cursor.1);
    }

    pub unsafe fn cursor_left(&mut self){
        if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
        }else{
            // Wrap back to end of last line
            self.cursor_up();
            self.cursor.0 = 79;
        }
        self.vga.set_cursor_position(self.cursor.0, self.cursor.1); 
    }

    pub unsafe fn cursor_down(&mut self){
        if self.cursor.1 < (25 - 1) {
            self.cursor.1 += 1;
        } else {
            // Wrap to first line
            self.cursor.1 = 0;
        }
        self.vga.set_cursor_position(self.cursor.0, self.cursor.1); 
    }

    pub unsafe fn cursor_up(&mut self){
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
        }else{
            // Wrap to last line
            self.cursor.1 = 24;
        }
        self.vga.set_cursor_position(self.cursor.0, self.cursor.1); 
    }

    pub unsafe fn clear(&mut self){
        for _ in 0..(25-self.cursor.1+1) {
            self.cursor_down();
        }
        for _ in 0..(80-self.cursor.0) {
            self.cursor_left();
        }
        for _ in 0..80*25 { self.write_char(' '); }
        self.cursor_up();
    }

}


// reg1 and reg2 are used for multiboot
#[no_mangle]
pub extern "C" fn main(r1: u32, r2: u32) -> ! {
    multiboot::init(r1, r2);
    let mut uart = unsafe { uart_16550::UARTDevice::x86_default() };
    uart.init();
    kprintln("Hello, world!", &mut uart);

    let vga = unsafe { Vga::x86_default() };

    // Switch to 80x25 text mode
    let vga = unsafe { vga.blank_screen() };
    let vga = unsafe { vga.set_mode::<Text80x25>() };
    let mut vga = unsafe { vga.unblank_screen() };

    clear_screen(&mut vga);

    let mut term = Vga80x25Terminal::new(&mut vga, 0x0A);


    kprintln("If you see this then that means the vga subsystem didn't instantly crash the kernel :)", &mut uart);
    /*
    // Temporary ATA code to test ata driver
    // NOTE: master device is not necessarilly the device from which the os was booted
    let mut ata_bus = unsafe{ ata::ATABus::x86_default() };
    let master_r = unsafe{ ata_bus.identify(ata::ATADevice::MASTER) };
    if let Some(id_data) = master_r {
        kprint_u16(id_data[0], &mut uart);
        kprint_u16(id_data[83], &mut uart);
        kprint_u16(id_data[88], &mut uart);
        kprint_u16(id_data[93], &mut uart);

        kprint_u16(id_data[61], &mut uart);
        kprint_u16(id_data[60], &mut uart);
        "Sector count of master device: 0x".chars().for_each(|c|unsafe{term.write_char(c)});
        u16_to_str_hex(id_data[60]).iter().for_each(|c| unsafe{term.write_char(*c as char)});
        
        kprint_u16(id_data[103], &mut uart);
        kprint_u16(id_data[102], &mut uart);
        kprint_u16(id_data[101], &mut uart);
        kprint_u16(id_data[100], &mut uart);
        
        kprintln("Found master device on primary ata bus!", &mut uart)
    }else{
        "No master device on primary ata bus!\n".chars().for_each(|c|unsafe{term.write_char(c)});
        panic!();
    }

    "\nFirst sector on master device on primary ata bus: \n".chars().for_each(|c|unsafe{term.write_char(c)});
    // Read first sector
    let sdata = unsafe{ata_bus.read_sector(ata::ATADevice::MASTER, ata::LBA28{low: 0, mid: 0, hi: 0})}.unwrap();
    for e in sdata{
        unsafe{
        u16_to_str_hex(e).iter().for_each(|c|term.write_char(*c as char));
        term.write_char(' ');
        }
    }
    //////////
*/
    let mut ps2 = unsafe { ps2_8042::PS2Device::x86_default() };

    "# ".chars().for_each(|c| unsafe { term.write_char(c); });

    let mut ignore_inc_x: bool = false;
    // Basically an ad-hoc ArrayString (arrayvec crate)
    let mut cmd_buf: [u8; 80] = [b' '; 80]; 
    let mut buf_ind = 0; // Also length of buf, a.k.a portion of buf used
    loop {
        ignore_inc_x = false;
        let b = unsafe { ps2.read_packet() };
        // kprint_u8(b.scancode, &mut uart);

        if b.typ == KeyboardPacketType::KEY_RELEASED && b.special_keys.contains(SpecialKeys::ESC) { break; }
        if b.typ == KeyboardPacketType::KEY_RELEASED { continue; }

        if b.special_keys.contains(SpecialKeys::UP_ARROW) {
           unsafe{ term.cursor_up(); }
        } else if b.special_keys.contains(SpecialKeys::DOWN_ARROW){
           unsafe{ term.cursor_down(); }
        } else if b.special_keys.contains(SpecialKeys::RIGHT_ARROW) {
           unsafe{ term.cursor_right(); }
        }else if b.special_keys.contains(SpecialKeys::LEFT_ARROW) {
           unsafe{ term.cursor_left(); }
        }

        let mut c = match b.char_codepoint {
            Some(v) => v,
            None => continue
        };

        if b.special_keys.any_shift() { c = b.shift_codepoint().unwrap(); }

        unsafe{ term.write_char(c); }

        if c == '\r' { ignore_inc_x = true; if buf_ind > 0 { buf_ind -= 1; } }
        if c == '\n' {
            let bufs = unsafe{from_utf8_unchecked(&cmd_buf[..buf_ind])};
            buf_ind = 0; // Flush buffer

            let mut splat = bufs.split_inclusive(' ');
            if let Some(cmnd) = splat.next(){
                // Handle shell built ins
                if cmnd.contains("echo"){
                    while let Some(arg) = splat.next(){
                        arg.chars().for_each(|c| unsafe{
                            term.write_char(c);
                        });
                    }
                }else if cmnd.contains("uname"){
                    "Ron".chars().for_each(|c|unsafe{term.write_char(c)});
                }else if cmnd.contains("help"){
                    "echo uname clear help".chars().for_each(|c|unsafe{term.write_char(c)});
                }else if cmnd.contains("clear"){
                    unsafe{ term.clear(); }
                }

                unsafe{ term.write_char('\n'); } // All inbuilt commands leave a \n at the end so that when they end the prompt appears on a new line
            }

            "# ".chars().for_each(|c| unsafe { term.write_char(c); });
            continue;
        }

        cmd_buf[buf_ind] = c as u8;
        if !ignore_inc_x { buf_ind += 1; }

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
