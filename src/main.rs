#![no_std]
#![no_main]
#![feature(bench_black_box)]
#![feature(abi_efiapi)]
#![feature(default_alloc_error_handler)]
#![feature(const_fn_trait_bound)]
#![feature(const_ptr_offset)]
#![feature(lang_items)]

extern crate alloc;

use core::fmt::Debug;
use core::cell::RefCell;
use core::convert::{TryFrom, TryInto};
use core::fmt::Write;

use alloc::rc::Rc;
use alloc::string::String;
use ata::{ATADevice, ATADeviceFile, ATABus};
use vfs::{VFSNode, INode, IFolder, Node};
use primitives::{Mutex, LazyInitialised};
use vga::{Color256, Unblanked};

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
    let written = write!(s, "Ron {}", p).is_ok(); // FIXME: Crashes on virtualbox and real hardware but not on qemu?
    if !UART.is_locked(){
        writeln!(UART.lock()).unwrap();
        if !written{
            writeln!(UART.lock(), "Bad panic, panic info cannot be formatted correctly, maybe OOM?").unwrap();
        }else{
            writeln!(UART.lock(), "{}", &s).unwrap();
        }
    }
    if !TERMINAL.is_locked(){
        let mut lock = TERMINAL.lock();
        lock.write_char('\n');
        if !written{
         "Bad panic, panic info cannot be formatted correctly, maybe OOM?\n".chars().for_each(|c|lock.write_char(c));
     }else{
            s.chars().for_each(|c|lock.write_char(c));
           lock.write_char('\n');
     }
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
mod vfs;
mod devfs;
mod partitions;
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

pub static UART: Mutex<LazyInitialised<UARTDevice>> = Mutex::from(LazyInitialised::uninit());

#[allow(unused)]
fn kprint_dump<T>(ptr: *const T, bytes: usize, uart: &mut UARTDevice){
    let arr = unsafe{core::slice::from_raw_parts(core::mem::transmute::<_, *mut u32>(ptr), bytes/core::mem::size_of::<u32>())};
    for e in arr{ write!(uart, "0x{:02X}",  *e).unwrap(); }  
}

pub const unsafe fn from_utf8_unchecked(v: &[u8]) -> &str {
    // SAFETY: the caller must guarantee that the bytes `v` are valid UTF-8.
    // Also relies on `&str` and `&[u8]` having the same layout.
    core::mem::transmute(v)
}

static TERMINAL: Mutex<LazyInitialised<Terminal<'static>>> = Mutex::from(LazyInitialised::uninit());
struct Terminal<'a>{
    fb: &'a mut dyn FrameBuffer,
    cursor_pos: (usize, usize),
    cursor_char: char,
    color: Pixel
}
impl Debug for Terminal<'_>{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Terminal").field("cursor_pos", &self.cursor_pos).field("cursor_char", &self.cursor_char).field("color", &self.color).finish()
    }
}

impl<'a> Write for Terminal<'a>{
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().for_each(|c|self.write_char(c));
        Ok(())
    }
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
        if self.cursor_pos.0 >= self.fb.get_cols()-1 {
            self.cursor_pos.0 = 0; 
            self.cursor_down(); 
            for x in 0..self.fb.get_cols(){self.fb.write_char(x, self.cursor_pos.1, ' ', self.color);}
            return;
        }
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
                for x in 0..self.fb.get_cols(){self.fb.write_char(x, self.cursor_pos.1, ' ', self.color);}
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
    writeln!(UART.lock(), "Hello, world!").unwrap();

    
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
    vfs::VFS_ROOT.lock().set(Rc::new(RefCell::new(VFSNode::new_root())));
    let dev_folder = vfs::VFSNode::new_folder(vfs::VFS_ROOT.lock().clone(), "dev");
    let dfs = Rc::new(RefCell::new(devfs::DevFS::new()));
    (*dev_folder).borrow_mut().mountpoint = Some(dfs.clone());

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
            fb = Some(unsafe{ &mut *((&mut uo) as *mut Vga<Color256, Unblanked>) as &mut dyn FrameBuffer});
        }
    }
    let fb = fb.unwrap();

    fb.fill(0, 0, fb.get_width(), fb.get_height(), Pixel{r: 0, g: 0, b: 0});    
    TERMINAL.lock().set(Terminal::new(fb, Pixel{r: 0x0, g: 0xa8, b: 0x54 }));
    
    writeln!(UART.lock(), "If you see this then that means the framebuffer subsystem didn't instantly crash the kernel :)").unwrap();
    writeln!(TERMINAL.lock(), "Hello, world!").unwrap();

       
    if let Some(primary_ata_bus) = unsafe{ ATABus::primary_x86() }{
        let ata_ref = Rc::new(RefCell::new(primary_ata_bus));
        // NOTE: master device is not necessarilly the device from which the os was booted
    
        if unsafe{(*ata_ref).borrow_mut().identify(ATADevice::MASTER).is_some()}{
            let master_dev = Rc::new(RefCell::new(ATADeviceFile{bus: ata_ref.clone(), bus_device: ATADevice::MASTER}));
            (*dfs).borrow_mut().add_device_file(master_dev.clone());
            for part_number in 0..4{
                if let Some(part_dev) = partitions::MBRPartitionFile::from(master_dev.clone(), part_number.try_into().unwrap()){
                    (*dfs).borrow_mut().add_device_file(Rc::new(RefCell::new(part_dev)));
                }
            }
        }

        if unsafe{(*ata_ref).borrow_mut().identify(ATADevice::SLAVE).is_some()}{
            let slave_dev = Rc::new(RefCell::new(ATADeviceFile{bus: ata_ref.clone(), bus_device: ATADevice::SLAVE}));
            (*dfs).borrow_mut().add_device_file(slave_dev.clone());
            for part_number in 0..4{
                if let Some(part_dev) = partitions::MBRPartitionFile::from(slave_dev.clone(), part_number.try_into().unwrap()){
                    (*dfs).borrow_mut().add_device_file(Rc::new(RefCell::new(part_dev)));
                }
            }
        }
    }

    if let Some(secondary_ata_bus) = unsafe{ ATABus::secondary_x86() }{
        let ata_ref = Rc::new(RefCell::new(secondary_ata_bus));
        // NOTE: master device is not necessarilly the device from which the os was booted
    
        if unsafe{(*ata_ref).borrow_mut().identify(ATADevice::MASTER).is_some()}{
            let master_dev = Rc::new(RefCell::new(ATADeviceFile{bus: ata_ref.clone(), bus_device: ATADevice::MASTER}));
            (*dfs).borrow_mut().add_device_file(master_dev.clone());
            for part_number in 0..4{
                if let Some(part_dev) = partitions::MBRPartitionFile::from(master_dev.clone(), part_number.try_into().unwrap()){
                    (*dfs).borrow_mut().add_device_file(Rc::new(RefCell::new(part_dev)));
                }
            }
        }

        if unsafe{(*ata_ref).borrow_mut().identify(ATADevice::SLAVE).is_some()}{
            let slave_dev = Rc::new(RefCell::new(ATADeviceFile{bus: ata_ref.clone(), bus_device: ATADevice::SLAVE}));
            (*dfs).borrow_mut().add_device_file(slave_dev.clone());
            for part_number in 0..4{
                if let Some(part_dev) = partitions::MBRPartitionFile::from(slave_dev.clone(), part_number.try_into().unwrap()){
                    (*dfs).borrow_mut().add_device_file(Rc::new(RefCell::new(part_dev)));
                }
            }
        }
    }

    
    let mut ps2 = unsafe { ps2_8042::PS2Device::x86_default() };

    let mut cur_dir = vfs::Path::try_from("/").unwrap();
    write!(TERMINAL.lock(), "{} # ", cur_dir).unwrap();

    let mut ignore_inc_x: bool; 
    // Basically an ad-hoc ArrayString (arrayvec crate)
    let mut cmd_buf: [u8; 80] = [b' '; 80]; 
    let mut buf_ind = 0; // Also length of buf, a.k.a portion of buf used
    'big_loop: loop {
        ignore_inc_x = false;
        let b = unsafe { ps2.read_packet() };

        if b.typ == KeyboardPacketType::KEY_RELEASED && b.special_keys.ESC { break; }
        if b.typ == KeyboardPacketType::KEY_RELEASED { continue; }

        if b.special_keys.UP_ARROW {
            TERMINAL.lock().visual_cursor_up();
        } else if b.special_keys.DOWN_ARROW{
            TERMINAL.lock().visual_cursor_down();
        } else if b.special_keys.RIGHT_ARROW {
            TERMINAL.lock().visual_cursor_right();
        }else if b.special_keys.LEFT_ARROW {
            TERMINAL.lock().visual_cursor_left();
        }

        let mut c = match b.char_codepoint {
            Some(v) => v,
            None => continue
        };

        if b.special_keys.any_shift() { c = b.shift_codepoint().unwrap(); }

        TERMINAL.lock().write_char(c);
        if c == '\r' { ignore_inc_x = true; if buf_ind > 0 { buf_ind -= 1; } }
        if c == '\n' {
            let bufs = unsafe{from_utf8_unchecked(&cmd_buf[..buf_ind])}.trim();
            buf_ind = 0; // Flush buffer
            let mut splat = bufs.split_inclusive(' ');
            if let Some(cmnd) = splat.next(){
                // Handle shell built ins
                if cmnd.contains("puts"){
                    while let Some(arg) = splat.next(){
                        write!(TERMINAL.lock(), "{}", arg).unwrap();
                    }
                    TERMINAL.lock().write_char('\n');
                }else if cmnd.contains("whoareyou"){
                    writeln!(TERMINAL.lock(), "Ron").unwrap();
                }else if cmnd.contains("help"){
                    writeln!(TERMINAL.lock(), "puts whoareyou rmdir mkdir hexdump ls cd clear exit help").unwrap();
                }else if cmnd.contains("clear"){
                    TERMINAL.lock().clear();
                }else if cmnd.contains("ls"){
                    for subnode in (*cur_dir.get_node().expect("Shell path should be valid at all times!")).borrow().get_children(){
                        write!(TERMINAL.lock(), "{} ", subnode.get_name()).unwrap();
                        if let Node::File(f) = subnode{
                            write!(TERMINAL.lock(), "(size: {} kb) ", (*f).borrow().get_size() as f32 / 1024.0).unwrap();
                        }
                    }
                    writeln!(TERMINAL.lock()).unwrap();                
                }else if cmnd.contains("hexdump"){
                    if let (Some(offset_str), Some(arg)) = (splat.next(), splat.next()){
                        if let Ok(offset) = offset_str.trim().parse::<usize>(){
                            let mut found = None;
                            for subnode in (*cur_dir.get_node().expect("Shell path should be valid at all times!")).borrow().get_children(){
                               if subnode.get_name() == arg{
                                   found = Some(subnode);
                                   break;
                               }
                            }   
                        
                            if let Some(Node::File(file)) = found{
                                if let Some(data) = (*file).borrow().read(offset, 16){
                                    for e in data{
                                        write!(TERMINAL.lock(), "0x{:02X} ", e).unwrap();
                                    }
                                }else{
                                    write!(TERMINAL.lock(), "Couldn't read file!").unwrap();
                                }
                            }else{
                                write!(TERMINAL.lock(), "File not found!").unwrap();
                            }
                        }else{
                            write!(TERMINAL.lock(), "Bad offset!").unwrap();
                        }
                        writeln!(TERMINAL.lock()).unwrap();                
                    }
                }else if cmnd.contains("cd"){
                    if let Some(name) = splat.next(){
                        let name = name.trim();
                        let old_dir = cur_dir.clone();
                        if name == ".."{
                            cur_dir.del_last();
                        }else if name.starts_with("/"){
                            if let Ok(new_dir) = name.try_into(){
                                cur_dir = new_dir;
                            }else{
                                writeln!(TERMINAL.lock(), "Invalid cd path!").unwrap();
                            }
                        }else{
                            cur_dir.append(name);
                        }
                        if cur_dir.get_node().is_none(){
                            writeln!(TERMINAL.lock(), "Invalid cd path: {}!", cur_dir).unwrap();
                            cur_dir = old_dir;
                        }
                    }
                }else if cmnd.contains("mkdir"){
                    while let Some(name) = splat.next(){
                       VFSNode::new_folder(cur_dir.get_node().expect("Shell path should be valid at all times!"), name);
                    }
                }else if cmnd.contains("rmdir"){
                    while let Some(name) = splat.next(){
                        let cur_node = cur_dir.get_node().expect("Shell path should be valid at all times!");
                        // Empty folder check
                        if let Some(child_to_sacrifice) = VFSNode::find_folder(cur_node.clone(), name){
                            if (*child_to_sacrifice).borrow().get_children().len() != 0 {
                                writeln!(TERMINAL.lock(), "Folder: \"{}\", is non-empty!", name).unwrap();
                                break;
                            }
                        }else{
                            writeln!(TERMINAL.lock(), "Folder: \"{}\", does not exist!", name).unwrap();
                            continue;
                        }
                        ////
          
                        if !VFSNode::del_folder(cur_node, name){ writeln!(TERMINAL.lock(), "Couldn't delete folder: \"{}\"!", name).unwrap(); }
                    }
                }else if cmnd.contains("elp"){
                    writeln!(TERMINAL.lock(), "NOPERS, no elp!").unwrap();
                }else if cmnd.contains("exit"){
                    break 'big_loop;
                }

            }

            write!(TERMINAL.lock(), "{} # ", cur_dir).unwrap();
            continue;
        }

        if buf_ind < cmd_buf.len() { 
            cmd_buf[buf_ind] = c as u8; 
            if !ignore_inc_x { buf_ind += 1; }
        }

    }

    writeln!(UART.lock(), "Heap usage: {} bytes", allocator::ALLOCATOR.lock().get_heap_size()).unwrap();
    // Shutdown
    writeln!(UART.lock(), "\nIt's now safe to turn off your computer!").unwrap();

    let width = TERMINAL.lock().fb.get_width();
    let height = TERMINAL.lock().fb.get_height();
    let cols = TERMINAL.lock().fb.get_cols();
    let rows = TERMINAL.lock().fb.get_rows();
    TERMINAL.lock().fb.fill(0, 0, width, height, Pixel{r: 0, g: 0, b: 0});
    let s = "It's now safe to turn off your computer!";
    s.chars().enumerate().for_each(|(ind, c)|{TERMINAL.lock().fb.write_char(ind+cols/2-s.len()/2, (rows-1)/2, c, Pixel{r: 0xff ,g: 0xff, b: 0x55});});
    
    loop{}
}
