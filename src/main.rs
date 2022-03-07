#![no_std]
#![no_main]
#![feature(bench_black_box)]
#![feature(abi_efiapi)]

use char_device::CharDevice;
use framebuffer::{FrameBuffer, Pixel};
use hio::KeyboardPacketType;
use ps2_8042::SpecialKeys;
use uart_16550::UARTDevice;
use vga::Vga;
use core::{arch::asm};

macro_rules! wait_for {
  ($cond:expr) => {
      while !$cond { core::hint::spin_loop() }
  };
}

trait X86Default { unsafe fn x86_default() -> Self; }


#[panic_handler]
fn panic(_p: &::core::panic::PanicInfo) -> ! { 
    unsafe{ asm!("cli"); }
    loop { unsafe { asm!("hlt"); } }
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
pub unsafe extern "C" fn memcmp(str1: *const u8, str2: *const u8, n: usize) -> i32 {
    let str1_size = str1 as *mut usize;
    let str2_size = str2 as *mut usize;
    let n_size = n/core::mem::size_of::<usize>();
    let mut ineq_i = 0;
    let mut eq: bool = true;
    for i in 0..n_size {
        if *str1_size.offset(i as isize) != *str2_size.offset(i as isize) { 
            eq = false; 
            for subi in i*core::mem::size_of::<usize>()..(i+1)*core::mem::size_of::<usize>(){
                if *str1.offset(i as isize) != *str2.offset(i as isize){ineq_i = subi; break; }
            } 
            break; 
        } 
    }
    for i in n_size*core::mem::size_of::<usize>() .. n {if *str1.offset(i as isize) != *str2.offset(i as isize){ eq = false; ineq_i = i; break; } }
    if eq { return 0; }else { return *str1.offset(ineq_i as isize) as i32 - *str2.offset(ineq_i as isize) as i32; }
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
pub unsafe extern "C" fn bcmp(str1: *const u8, str2: *const u8, n: usize) -> i32 {
    if n == 0 { return 0; }
    memcmp(str1, str2, n)
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
    char_buf: [char; 100*100], //FIXME: Should replace with a dynamic array(vector)
    cursor_char: char,
    color: Pixel
}
impl<'a> Terminal<'a>{
    fn new(fb: &'a mut dyn FrameBuffer, color: Pixel) -> Self {
        Terminal{
            fb,
            cursor_pos: (0, 0),
            char_buf: [' '; 100*100],
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
        for i in 0..100*100 { self.char_buf[i] = ' '; }
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
        self.cursor_char = self.char_buf[self.cursor_pos.0 + self.cursor_pos.1*self.fb.get_cols()];
        self.fb.write_char(self.cursor_pos.0, self.cursor_pos.1, '_', self.color);
        self.char_buf[self.cursor_pos.0 + self.cursor_pos.1*self.fb.get_cols()] = '_';
    }
    
    fn erase_visual_cursor(&mut self){
        self.fb.write_char(self.cursor_pos.0, self.cursor_pos.1, self.cursor_char, self.color);
        self.char_buf[self.cursor_pos.0 + self.cursor_pos.1*self.fb.get_cols()] = self.cursor_char;
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
                self.char_buf[self.cursor_pos.0 + self.cursor_pos.1*self.fb.get_cols()] = c;
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

    let mut uart = unsafe { uart_16550::UARTDevice::x86_default() };
    uart.init();
    kprintln("Hello, world!", &mut uart);

    
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


    kprintln("If you see this then that means the framebuffer subsystem didn't instantly crash the kernel :)", &mut uart);
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

    "# ".chars().for_each(|c| { term.write_char(c); });

    let mut ignore_inc_x: bool = false;
    // Basically an ad-hoc ArrayString (arrayvec crate)
    let mut cmd_buf: [u8; 100*100] = [b' '; 100*100]; 
    let mut buf_ind = 0; // Also length of buf, a.k.a portion of buf used
    'big_loop: loop {
        ignore_inc_x = false;
        let b = unsafe { ps2.read_packet() };
        // kprint_u8(b.scancode, &mut uart);

        if b.typ == KeyboardPacketType::KEY_RELEASED && b.special_keys.contains(SpecialKeys::ESC) { break; }
        if b.typ == KeyboardPacketType::KEY_RELEASED { continue; }

        if b.special_keys.contains(SpecialKeys::UP_ARROW) {
           term.visual_cursor_up();
        } else if b.special_keys.contains(SpecialKeys::DOWN_ARROW){
           term.visual_cursor_down();
        } else if b.special_keys.contains(SpecialKeys::RIGHT_ARROW) {
            term.visual_cursor_right();
        }else if b.special_keys.contains(SpecialKeys::LEFT_ARROW) {
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

        if buf_ind < cmd_buf.len() { cmd_buf[buf_ind] = c as u8; }
        if !ignore_inc_x { buf_ind += 1; }

    }

    // Shutdown
    kprintln("\nIt's now safe to turn off your computer!", &mut uart);
    fb.fill(0, 0, fb.get_width(), fb.get_height(), Pixel{r: 0, g: 0, b: 0});
    let s = "It's now safe to turn off your computer!";
    s.chars().enumerate().for_each(|(ind, c)|{fb.write_char(ind+fb.get_cols()/2-s.len()/2, (fb.get_rows()-1)/2, c, Pixel{r: 0xff ,g: 0xff, b: 0x55});});
    
    
    unsafe{ asm!("cli"); }
    loop { unsafe { asm!("hlt"); } }
}
