#![no_std]
#![no_main]
#![feature(bench_black_box)]
#![feature(abi_efiapi)]
#![feature(default_alloc_error_handler)]
#![feature(const_fn_trait_bound)]
#![feature(const_ptr_offset)]
#![feature(lang_items)]

extern crate alloc;


use alloc::boxed::Box;
use alloc::string::String;
use primitives::{Mutex, LazyInitialised};

use crate::alloc::vec::Vec;
use crate::char_device::CharDevice;
use crate::framebuffer::{FrameBuffer, Pixel};
use crate::hio::KeyboardPacketType;
use crate::ps2_8042::SpecialKeys;
use crate::uart_16550::UARTDevice;
use crate::vga::Vga;

macro_rules! wait_for {
  ($cond:expr) => {
      while !$cond { core::hint::spin_loop() }
  };
}


trait X86Default { unsafe fn x86_default() -> Self; }


#[panic_handler]
fn panic(p: &::core::panic::PanicInfo) -> ! { 
    let mut s = String::new();
    use core::fmt::Write;
    kprintln("", &mut UART.lock());
    if write!(s, "Ron {}", p).is_err(){
        kprintln("Bad panic, panic info cannot be formatted correctly, maybe OOM?", &mut UART.lock());
    }else{
        kprintln(&s, &mut UART.lock());
    }
    loop{}
}


mod multiboot;
mod ps2_8042;
mod uart_16550;
mod vga;
mod virtmem;
mod hio;
mod ata;
mod efi;
mod framebuffer;
mod char_device;
mod allocator;
mod primitives;

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if n < core::mem::size_of::<usize>(){
        for i in 0..n {*dest.offset(i as isize) = *src.offset(i as isize); }
        return dest;
    }
    let dest_size = dest as *mut usize;
    let src_size = src as *mut usize;
    let n_size = n/core::mem::size_of::<usize>();

    for i in 0..n_size { *dest_size.offset(i as isize) = *src_size.offset(i as isize); }
    for i in n_size*core::mem::size_of::<usize>() .. n { *dest.offset(i as isize) = *src.offset(i as isize); }
    return dest;
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(ptr1: *const u8, ptr2: *const u8, n: usize) -> i32 {
    let ptr1_size = ptr1 as *mut usize;
    let ptr2_size = ptr2 as *mut usize;
    let n_size = n/core::mem::size_of::<usize>();
    let mut ineq_i = 0;
    let mut eq: bool = true;
    for i in 0..n_size {
        if *ptr1_size.add(i) != *ptr2_size.add(i) { 
            eq = false; 
            for subi in i*core::mem::size_of::<usize>()..(i+1)*core::mem::size_of::<usize>(){
                if *ptr1.add(i) != *ptr2.add(i){ineq_i = subi; break; }
            } 
            break; 
        } 
    }
    for i in n_size*core::mem::size_of::<usize>() .. n {if *ptr1.offset(i as isize) != *ptr2.offset(i as isize){ eq = false; ineq_i = i; break; } }
    if eq { return 0; }else { return *ptr1.add(ineq_i) as i32 - *ptr2.add(ineq_i) as i32; }
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, c: isize, n: usize) -> *mut u8 {
    let c = c as u8;
    if n < core::mem::size_of::<usize>(){
        for i in 0..n {*dest.offset(i as isize) = c; }
        return dest;
    }
    let dest_size = dest as *mut usize;
    let n_size = n/core::mem::size_of::<usize>();
    let mut c_size = 0usize;
    // Endianness dosen't matter
    for i in 0..core::mem::size_of::<usize>()/core::mem::size_of::<u8>(){
        c_size |= (c as usize) << (i*core::mem::size_of::<u8>());
    }
    for i in 0..n_size {*(dest_size.offset(i as isize)) = c_size; }
    for i in n_size*core::mem::size_of::<usize>()..n { *(dest.offset(i as isize)) = c; }
    return dest;
}

#[no_mangle]
pub unsafe extern "C" fn bcmp(ptr1: *const u8, ptr2: *const u8, n: usize) -> i32 {
    if n == 0 { return 0; }
    memcmp(ptr1, ptr2, n)
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8{
    if (dest as *const u8) == src { return dest; }
    let src_range = src..src.add(n);
    let has_overlap =  src_range.contains(&(dest as *const u8)) || src_range.contains(&(dest.add(n) as *const u8));
    if !has_overlap{
        memcpy(dest, src, n);
    } else if (dest as *const u8) < src{
        for i in 0..n{*dest.add(i) = *src.add(i); }
    } else if (dest as *const u8) > src{
        for i in (0..n).rev(){*dest.add(i) = *src.add(i); }
    }
    dest
}


pub static UART: Mutex<LazyInitialised<UARTDevice>> = Mutex::from(LazyInitialised::new());

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
    kprint(unsafe{ from_utf8_unchecked(&u16_to_str_hex(((data >> 16) & 0xffff) as u16))}, uart);
    kprint(unsafe{ from_utf8_unchecked(&u16_to_str_hex((data & 0xffff) as u16))}, uart);
    kprint(" ", uart);
}

fn kprint_u64(data: u64, uart: &mut UARTDevice){
    kprint("0x", uart);
    kprint(unsafe{ from_utf8_unchecked(&u16_to_str_hex(((data >> 48) & 0xffff) as u16))}, uart);
    kprint(unsafe{ from_utf8_unchecked(&u16_to_str_hex(((data >> 32) & 0xffff) as u16))}, uart);
    kprint(unsafe{ from_utf8_unchecked(&u16_to_str_hex(((data >> 16) & 0xffff) as u16))}, uart);
    kprint(unsafe{ from_utf8_unchecked(&u16_to_str_hex((data & 0xffff) as u16))}, uart);
    kprint(" ", uart);
}

fn kprint_dump<T>(ptr: *const T, bytes: usize, uart: &mut UARTDevice){
    let arr = unsafe{core::slice::from_raw_parts(core::mem::transmute::<_, *mut u32>(ptr), bytes/core::mem::size_of::<u32>())};
    for e in arr{ kprint_u32(*e, uart); }  
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

struct Terminal<'a>{
    fb: &'a mut dyn FrameBuffer,
    cursor_pos: (usize, usize),
    cursor_char: char,
    color: Pixel
}
impl<'a> Terminal<'a>{
    fn new(fb: &'a mut dyn FrameBuffer, color: Pixel) -> Self {
        Terminal{
            fb,
            cursor_pos: (0, 0),
            cursor_char: ' ',
            color
        }
    }
    fn clear(&mut self){
        for i in 0..self.fb.get_height(){
            for j in 0..self.fb.get_width(){
                self.fb.set_pixel(j, i,Pixel{r: 0, g: 0, b: 0});
            }
        }
        self.cursor_pos = (0, 0);
    }
    fn cursor_up(&mut self){
        if self.cursor_pos.1 == 0 { return; }
        self.cursor_pos.1 -= 1;
    }
    fn cursor_down(&mut self){
        if self.cursor_pos.1 >= self.fb.get_rows()-1 { self.cursor_pos.1 = 0; }
        else {  self.cursor_pos.1 += 1; }
    }
    fn cursor_right(&mut self){
        if self.cursor_pos.0 >= self.fb.get_cols()-1 {self.cursor_pos.0 = 0; self.cursor_down(); return;}
        self.cursor_pos.0 += 1;
    }
    fn cursor_left(&mut self){
        if self.cursor_pos.0 == 0{ return;}
        self.cursor_pos.0 -= 1;
    }

    pub fn visual_cursor_up(&mut self){
        self.erase_visual_cursor();
        self.cursor_up();
        self.update_visual_cursor();
    }
    pub fn visual_cursor_left(&mut self){
        self.erase_visual_cursor();
        self.cursor_left();
        self.update_visual_cursor();
    }
    pub fn visual_cursor_right(&mut self){
        self.erase_visual_cursor();
        self.cursor_right();
        self.update_visual_cursor();
    }

    pub fn visual_cursor_down(&mut self){
        self.erase_visual_cursor();
        self.cursor_down();
        self.update_visual_cursor();
    }

    fn update_visual_cursor(&mut self){
        self.fb.write_char(self.cursor_pos.0, self.cursor_pos.1, '_', self.color);
    }
    
    fn erase_visual_cursor(&mut self){
        self.fb.write_char(self.cursor_pos.0, self.cursor_pos.1, self.cursor_char, self.color);
    }

    fn write_char(&mut self, c: char){
        self.erase_visual_cursor(); // erase current cursor
        match c{
            '\n' => {
                self.cursor_down();
                self.cursor_pos.0 = 0;
            },
            '\r' => {
               self.cursor_left(); // Go to char
               self.erase_visual_cursor();
            },
            c => {
                self.fb.write_char(self.cursor_pos.0, self.cursor_pos.1, c, self.color);
                self.cursor_right();
            }
        }
        self.update_visual_cursor();
    }
}
// reg1 and reg2 are used for multiboot
#[no_mangle]
pub extern "C" fn main(r1: u32, r2: u32) -> ! {
    let multiboot_data= multiboot::init(r1 as usize, r2 as usize);
    unsafe{ UART.lock().set(UARTDevice::x86_default()); }
    UART.lock().init();
    kprintln("Hello, world!", &mut UART.lock());

    
    let mut efi_system_table_ptr = 0usize;
    let mut i = 0;
    loop{
        let id = multiboot_data[i];
        let mut len = multiboot_data[i+1];
        if len%8 != 0 { len += 8-len%8; }
        len /= core::mem::size_of::<u32>() as u32;
        if id == 0 && len == 2 { break; }

        if id == 0xB || id == 0xC {
            for j in i+2..i+2+(core::mem::size_of::<usize>()/core::mem::size_of::<u32>()){
                efi_system_table_ptr |= (multiboot_data[j] as usize) << ((j-i-2)*32); // assumes little endian
            }
        }
        i += len as usize;
    }
    allocator::ALLOCATOR.lock().init((1*1024*1024) as *mut u8, 100*1024);
    let vga;
    let mut fb: Option<&mut dyn framebuffer::FrameBuffer>;
    let o;
    let mut uo;
    fb = framebuffer::try_setup_efi_framebuffer(efi_system_table_ptr as *mut efi::EfiSystemTable, 800, 600).map(|x|x as &mut dyn framebuffer::FrameBuffer);
    if fb.is_none(){
        vga = unsafe { Vga::x86_default() };
        o = framebuffer::try_setup_vga_framebuffer(vga, 800, 600);
        if o.is_some(){
            uo = o.unwrap();
            fb = Some((&mut uo) as &mut dyn FrameBuffer);
        }
    }
    let mut fb = fb.unwrap();

    fb.fill(0, 0, fb.get_width(), fb.get_height(), Pixel{r: 0, g: 0, b: 0});    
    let mut term = Terminal::new(fb, Pixel{r: 0x0, g: 0xa8, b: 0x54 });


    kprintln("If you see this then that means the framebuffer subsystem didn't instantly crash the kernel :)", &mut UART.lock());

    // Temporary ATA code to test ata driver
    // NOTE: master device is not necessarilly the device from which the os was booted
    let mut ata_bus = unsafe{ ata::ATABus::x86_default() };
    let master_r = unsafe{ ata_bus.identify(ata::ATADevice::MASTER) };
    if let Some(id_data) = master_r {
        kprint_u16(id_data[0], &mut UART.lock());
        kprint_u16(id_data[83], &mut UART.lock());
        kprint_u16(id_data[88], &mut UART.lock());
        kprint_u16(id_data[93], &mut UART.lock());

        kprint_u16(id_data[61], &mut UART.lock());
        kprint_u16(id_data[60], &mut UART.lock());
        "Sector count of master device: 0x".chars().for_each(|c|term.write_char(c));
        u16_to_str_hex(id_data[60]).iter().for_each(|c| term.write_char(*c as char));
        
        kprint_u16(id_data[103], &mut UART.lock());
        kprint_u16(id_data[102], &mut UART.lock());
        kprint_u16(id_data[101], &mut UART.lock());
        kprint_u16(id_data[100], &mut UART.lock());
        
        kprintln("Found master device on primary ata bus!", &mut UART.lock())
    }else{
        "No master device on primary ata bus!\n".chars().for_each(|c|term.write_char(c));
        panic!();
    }

    "\nFirst sector on master device on primary ata bus: \n".chars().for_each(|c|term.write_char(c));
    // Read first sector
    let sdata = unsafe{ata_bus.read_sector(ata::ATADevice::MASTER, ata::LBA28{low: 0, mid: 0, hi: 0})}.unwrap();
    for e in sdata{
        u16_to_str_hex(e).iter().for_each(|c|term.write_char(*c as char));
        term.write_char(' ');
    }
    term.write_char('\n');
    //////////
    
    let mut ps2 = unsafe { ps2_8042::PS2Device::x86_default() };

    "# ".chars().for_each(|c| { term.write_char(c); });

    let mut ignore_inc_x: bool; 
    // Basically an ad-hoc ArrayString (arrayvec crate)
    let mut cmd_buf: [u8; 80] = [b' '; 80]; 
    let mut buf_ind = 0; // Also length of buf, a.k.a portion of buf used
    'big_loop: loop {
        ignore_inc_x = false;
        let b = unsafe { ps2.read_packet() };
        // kprint_u8(b.scancode, &mut UART.lock());

        if b.typ == KeyboardPacketType::KEY_RELEASED && b.special_keys.ESC { break; }
        if b.typ == KeyboardPacketType::KEY_RELEASED { continue; }

        if b.special_keys.UP_ARROW {
           term.visual_cursor_up();
        } else if b.special_keys.DOWN_ARROW{
           term.visual_cursor_down();
        } else if b.special_keys.RIGHT_ARROW {
            term.visual_cursor_right();
        }else if b.special_keys.LEFT_ARROW {
            term.visual_cursor_left();
        }

        let mut c = match b.char_codepoint {
            Some(v) => v,
            None => continue
        };

        if b.special_keys.any_shift() { c = b.shift_codepoint().unwrap(); }

        term.write_char(c);
        if c == '\r' { ignore_inc_x = true; if buf_ind > 0 { buf_ind -= 1; } }
        if c == '\n' {
            let bufs = unsafe{from_utf8_unchecked(&cmd_buf[..buf_ind])}.trim();
            buf_ind = 0; // Flush buffer
            let mut splat = bufs.split_inclusive(' ');
            if let Some(cmnd) = splat.next(){
                // Handle shell built ins
                if cmnd.contains("puts"){
                    while let Some(arg) = splat.next(){
                        arg.chars().for_each(|c| {
                            term.write_char(c);
                        });
                    }
                    term.write_char('\n');
                }else if cmnd.contains("whoareyou"){
                    "Ron\n".chars().for_each(|c|term.write_char(c));
                }else if cmnd.contains("help"){
                    "puts whoareyou clear exit help\n".chars().for_each(|c|term.write_char(c));
                }else if cmnd.contains("clear"){
                    term.clear();
                }else if cmnd.contains("elp"){
                    "No elp!\n".chars().for_each(|c|term.write_char(c));
                }else if cmnd.contains("exit"){
                    break 'big_loop;
                }

            }

            "# ".chars().for_each(|c| { term.write_char(c); });
            continue;
        }

        if buf_ind < cmd_buf.len() { 
            cmd_buf[buf_ind] = c as u8; 
            if !ignore_inc_x { buf_ind += 1; }
        }

    }

    // Shutdown
    kprintln("\nIt's now safe to turn off your computer!", &mut UART.lock());
    fb.fill(0, 0, fb.get_width(), fb.get_height(), Pixel{r: 0, g: 0, b: 0});
    let s = "It's now safe to turn off your computer!";
    s.chars().enumerate().for_each(|(ind, c)|{fb.write_char(ind+fb.get_cols()/2-s.len()/2, (fb.get_rows()-1)/2, c, Pixel{r: 0xff ,g: 0xff, b: 0x55});});
    
    loop{}
}
