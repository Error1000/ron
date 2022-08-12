#![no_std]
#![no_main]
#![feature(bench_black_box)]
#![feature(abi_efiapi)]
#![feature(default_alloc_error_handler)]
#![feature(lang_items)]
#![allow(dead_code)]

extern crate alloc;

use core::cmp::min;
use core::fmt::Debug;
use core::cell::RefCell;
use core::convert::{TryFrom, TryInto};
use core::fmt::Write;

use alloc::borrow::ToOwned;
use alloc::rc::Rc;
use alloc::string::String;
use ata::{ATADevice, ATADeviceFile, ATABus};
use vfs::{VFSNode, IFolder, Node, IFile};
use primitives::{Mutex, LazyInitialised};
use vga::{Color256, Unblanked};

use crate::allocator::ALLOCATOR;
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
    let written = write!(s, "Ron: {}", p).is_ok(); // FIXME: Crashes on virtualbox and real hardware but not on qemu?
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
mod ext2;
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
    // NOTE: Don't use from_ne_bytes as it causes a call to memset (don't know if directly or indirectly), causing recursion, leading to a stack overflow
    // Endianness dosen't matter
    let mut c_size = 0usize;
    for i in 0..core::mem::size_of::<usize>()/core::mem::size_of::<u8>() {
        c_size |= (c as usize) << i;
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
    unsafe{ UART.lock().set(UARTDevice::x86_default()); }
    UART.lock().init();

    let multiboot_data = multiboot::init(r1 as usize, r2 as usize);
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
                efi_system_table_ptr |= (multiboot_data[j] as usize) << ((j-i-2)*32); // FIXME: assumes little endian
            }
        }
        i += len as usize;
    }
    // FIXME: Don't hardcode the starting location of the heap
    // Stack size: 1mb, executable size (as of 22 may 2022): ~4mb, so starting the heap at 8mb should be a safe bet.
    allocator::ALLOCATOR.lock().init((8*1024*1024) as *mut u8, 1*1024*1024);
    vfs::VFS_ROOT.lock().set(Rc::new(RefCell::new(VFSNode::new_root())));

    let dev_folder = vfs::VFSNode::new_folder(vfs::VFS_ROOT.lock().clone(), "dev");
    let dfs = Rc::new(RefCell::new(devfs::DevFS::new()));
    (*dev_folder).borrow_mut().mountpoint = Some(dfs.clone() as Rc<RefCell<dyn IFolder>>);

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
            (*dfs).borrow_mut().add_device_file(master_dev.clone() as Rc<RefCell<dyn IFile>>, "hda".to_owned());
            for part_number in 0..4{
                if let Some(part_dev) = partitions::MBRPartitionFile::from(master_dev.clone() as Rc<RefCell<dyn IFile>>, part_number.try_into().unwrap()){
                    let mut part_dev_name = String::new();
                    write!(part_dev_name, "hdap{}", part_number+1).unwrap();
                    writeln!(TERMINAL.lock(), "Found partition {}, with offset in bytes from begining of: {}", part_dev_name, part_dev.get_offset()).unwrap();
                    (*dfs).borrow_mut().add_device_file(Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>, part_dev_name);
                }
            }
        }

        if unsafe{(*ata_ref).borrow_mut().identify(ATADevice::SLAVE).is_some()}{
            let slave_dev = Rc::new(RefCell::new(ATADeviceFile{bus: ata_ref.clone(), bus_device: ATADevice::SLAVE}));
            (*dfs).borrow_mut().add_device_file(slave_dev.clone() as Rc<RefCell<dyn IFile>>, "hdb".to_owned());
            for part_number in 0..4{
                if let Some(part_dev) = partitions::MBRPartitionFile::from(slave_dev.clone() as Rc<RefCell<dyn IFile>>, part_number.try_into().unwrap()){
                    let mut part_dev_name = String::new();
                    write!(part_dev_name, "hdbp{}", part_number+1).unwrap();
                    writeln!(TERMINAL.lock(), "Found partition {}, with offset in bytes from begining of: {}", part_dev_name, part_dev.get_offset()).unwrap();
                    (*dfs).borrow_mut().add_device_file(Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>, part_dev_name);
                }
            }
        }
    }

    if let Some(secondary_ata_bus) = unsafe{ ATABus::secondary_x86() }{
        let ata_ref = Rc::new(RefCell::new(secondary_ata_bus));
        // NOTE: master device is not necessarilly the device from which the os was booted
    
        if unsafe{(*ata_ref).borrow_mut().identify(ATADevice::MASTER).is_some()}{
            let master_dev = Rc::new(RefCell::new(ATADeviceFile{bus: ata_ref.clone(), bus_device: ATADevice::MASTER}));
            (*dfs).borrow_mut().add_device_file(master_dev.clone() as Rc<RefCell<dyn IFile>>, "hdc".to_owned());
            for part_number in 0..4{
                if let Some(part_dev) = partitions::MBRPartitionFile::from(master_dev.clone() as Rc<RefCell<dyn IFile>>, part_number.try_into().unwrap()){
                    let mut part_dev_name = String::new();
                    write!(part_dev_name, "hdcp{}", part_number+1).unwrap();
                    writeln!(TERMINAL.lock(), "Found partition {}, with offset in bytes from begining of: {}", part_dev_name, part_dev.get_offset()).unwrap();
                    (*dfs).borrow_mut().add_device_file(Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>, part_dev_name);
                }
            }
        }

        if unsafe{(*ata_ref).borrow_mut().identify(ATADevice::SLAVE).is_some()}{
            let slave_dev = Rc::new(RefCell::new(ATADeviceFile{bus: ata_ref.clone(), bus_device: ATADevice::SLAVE}));
            (*dfs).borrow_mut().add_device_file(slave_dev.clone() as Rc<RefCell<dyn IFile>>, "hdd".to_owned());
            for part_number in 0..4{
                if let Some(part_dev) = partitions::MBRPartitionFile::from(slave_dev.clone() as Rc<RefCell<dyn IFile>>, part_number.try_into().unwrap()){
                    let mut part_dev_name = String::new();
                    write!(part_dev_name, "hddp{}", part_number+1).unwrap();
                    writeln!(TERMINAL.lock(), "Found partition {}, with offset in bytes from begining of: {}", part_dev_name, part_dev.get_offset()).unwrap();
                    (*dfs).borrow_mut().add_device_file(Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>, part_dev_name);
                }
            }
        }
    }

    let mut ps2 = unsafe { ps2_8042::PS2Device::x86_default() };

    let mut cur_dir = vfs::Path::try_from("/").unwrap();
    write!(TERMINAL.lock(), "{} # ", cur_dir).unwrap();

    let mut ignore_inc_x: bool; 
    // Basically an ad-hoc ArrayString (arrayvec crate)
    let mut cmd_buf: [u8; 2048] = [b' '; 2048]; 
    let mut buf_ind = 0; // Also length of buf, a.k.a portion of buf used
    'big_loop: loop {
        ignore_inc_x = false;
        let b = unsafe { ps2.read_packet() };

        if b.typ == KeyboardPacketType::KeyReleased && b.special_keys.esc { break; }
        if b.typ == KeyboardPacketType::KeyReleased { continue; }

        if b.special_keys.up_arrow {
            TERMINAL.lock().visual_cursor_up();
        } else if b.special_keys.down_arrow{
            TERMINAL.lock().visual_cursor_down();
        } else if b.special_keys.right_arrow {
            TERMINAL.lock().visual_cursor_right();
        }else if b.special_keys.left_arrow {
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
                    let mut puts_output: String = String::new();
                    let mut redirect: Option<String> = None;
                    while let Some(arg) = splat.next() {
                        if arg.trim().starts_with('>'){ redirect = Some(arg.trim()[1..].to_owned()); continue; }
                        
                        if let Some(ref mut redir) = redirect{
                            redir.push_str(arg);
                        }else{
                            puts_output.push_str(arg);
                        }
                    }

                    if let Some(redir_str) = redirect {
                        let path = if redir_str.starts_with('/'){
                            vfs::Path::try_from(redir_str).ok()
                        }else{
                            let mut actual_dir = cur_dir.clone();
                            actual_dir.push_str(redir_str.as_str());
                            Some(actual_dir)
                        };
                        if let Some(node) = path.map(|path| path.get_node()) {
                    
                            if let Some(Node::File(file)) = node {
                                if (*file).borrow_mut().resize(puts_output.len()).is_some(){
                                    if (*file).borrow_mut().write(0, puts_output.as_bytes()).is_none() {
                                        writeln!(TERMINAL.lock(), "Couldn't write to file!").unwrap();
                                    }
                                }else{
                                    writeln!(TERMINAL.lock(), "Couldn't resize file!").unwrap();
                                }
                            }else{
                                writeln!(TERMINAL.lock(), "Redirect path should be valid!").unwrap();
                            }
                        }

                    } else { write!(TERMINAL.lock(), "{}", puts_output).unwrap(); };



                    writeln!(TERMINAL.lock()).unwrap();
                }else if cmnd.contains("whoareyou"){
                    writeln!(TERMINAL.lock(), "Ron").unwrap();
                }else if cmnd.contains("help"){
                    writeln!(TERMINAL.lock(), "puts whoareyou rmvfsdir mkvfsdir mount.ext2 umount free hexdump cat ls cd clear exit help").unwrap();
                }else if cmnd.contains("clear"){
                    TERMINAL.lock().clear();
                }else if cmnd.contains("free"){
                    let heap_used = ALLOCATOR.lock().get_heap_used();
                    let heap_max = ALLOCATOR.lock().get_heap_max();
                    writeln!(TERMINAL.lock(), "{} bytes of {} bytes used on heap, that's {}% !", heap_used, heap_max, heap_used as f32/heap_max as f32 * 100.0).unwrap();
                }else if cmnd.contains("mount.ext2"){
                    if let (Some(file), Some(mntpoint)) = (splat.next(), splat.next()){
                        let mut file_node = vfs::Path::try_from(file.trim());
                        if !file.starts_with("/"){
                            let mut actual_node = cur_dir.clone();
                            actual_node.push_str(file);
                            file_node = Ok(actual_node);
                        }
                        let file_node = if let Ok(val) = file_node { val } else { writeln!(TERMINAL.lock(), "Malformed source path: \"{}\"!", file).unwrap(); continue; };
                        let file_node = if let Some(val) = file_node.get_node() { val } else { writeln!(TERMINAL.lock(), "Source path: \"{}\" does not exist!", file).unwrap(); continue; };
                        let file_node = if let vfs::Node::File(val) = file_node { val } else { writeln!(TERMINAL.lock(), "Source path: \"{}\" is not a file!", file).unwrap(); continue;};
                        let e2fs = ext2::Ext2FS::new(file_node);
                        let e2fs = if let Some(val) = e2fs { val } else { writeln!(TERMINAL.lock(), "Source file does not contain a valid ext2 fs!").unwrap(); continue; };
                        let e2fs = Rc::new(RefCell::new(e2fs));
                        let root_inode = (*e2fs).borrow_mut().read_inode(2).expect("Root inode should exist!").as_vfs_node(e2fs.clone(), 2).expect("Root inode should be parsable in vfs!").expect_folder();
                        let mut mntpoint_node = vfs::Path::try_from(mntpoint.trim());
                        if !mntpoint.starts_with("/") {
                            let mut actual_node = cur_dir.clone();
                            actual_node.push_str(mntpoint);
                            mntpoint_node = Ok(actual_node);
                        }
                        let mntpoint_node = if let Ok(val) = mntpoint_node { val } else { writeln!(TERMINAL.lock(), "Malformed mountpoint path!").unwrap(); continue;};
                        let mntpoint_node = if let Some(val) = mntpoint_node.get_vfs_node() { val } else { writeln!(TERMINAL.lock(), "Mountpoint should exist in vfs!").unwrap(); continue; };
                        (*mntpoint_node).borrow_mut().mountpoint = Some(root_inode);
                    }else{
                        writeln!(TERMINAL.lock(), "Not enough arguments!").unwrap();                
                    }
                }else if cmnd.contains("umount"){
                    if let Some(mntpoint) = splat.next(){
                        let mut mntpoint_node = vfs::Path::try_from(mntpoint.trim());
                        if !mntpoint.starts_with("/"){
                            let mut actual_node = cur_dir.clone();
                            actual_node.push_str(mntpoint);
                            mntpoint_node = Ok(actual_node);
                        }
                        let mntpoint_node = if let Ok(val) = mntpoint_node { val } else { writeln!(TERMINAL.lock(), "Malformed mountpoint path!").unwrap(); continue;};
                        let mntpoint_node = if let Some(val) = mntpoint_node.get_vfs_node() { val } else { writeln!(TERMINAL.lock(), "Mountpoint should exist in vfs!").unwrap(); continue; };
                        (*mntpoint_node).borrow_mut().mountpoint = None;
                    }else{
                        writeln!(TERMINAL.lock(), "Not enough arguments!").unwrap();                
                    }
                }else if cmnd.contains("ls"){
                    for subnode in (*cur_dir.get_node().expect("Shell path should be valid at all times!").expect_folder()).borrow().get_children(){
                        write!(TERMINAL.lock(), "{} ", subnode.0).unwrap();
                        if let Node::File(f) = subnode.1{
                            write!(TERMINAL.lock(), "(size: {} kb) ", (*f).borrow().get_size() as f32 / 1024.0).unwrap();
                        }
                    }
                    writeln!(TERMINAL.lock()).unwrap();                
                }else if cmnd.contains("hexdump"){
                    if let (Some(offset_str), Some(file_str)) = (splat.next(), splat.next()){
                        if let Ok(offset) = offset_str.trim().parse::<usize>(){

                            let arg_path = if file_str.starts_with('/'){
                                vfs::Path::try_from(file_str)
                            }else{
                                let mut actual_dir = cur_dir.clone();
                                actual_dir.push_str(file_str);
                                Ok(actual_dir)
                            };

                            let node = arg_path.map(|path|path.get_node());
                            let node = if let Ok(val) = node { val } else { writeln!(TERMINAL.lock(), "Invalid path!").unwrap(); continue; };
                            let node = if let Some(val) = node { val } else { writeln!(TERMINAL.lock(), "Path doesn't exist!").unwrap(); continue; };

                            if let Node::File(file) = node {
                                if let Some(data) = (*file).borrow().read(offset, min(16, (*file).borrow().get_size())){
                                    for e in data.iter() {
                                        write!(TERMINAL.lock(), "0x{:02X} ", e).unwrap();
                                    }
                                }else{
                                    write!(TERMINAL.lock(), "Couldn't read file!").unwrap();
                                }
                            }else{
                                write!(TERMINAL.lock(), "Path should be a file!").unwrap();
                            }
                        }else{
                            write!(TERMINAL.lock(), "Bad offset!").unwrap();
                        }
                    }else{
                        write!(TERMINAL.lock(), "Not enough arguments!").unwrap();
                    }

                    writeln!(TERMINAL.lock()).unwrap();
                }else if cmnd.contains("cat"){
                    if let Some(file_str) = splat.next(){
                        let arg_path = if file_str.starts_with('/'){
                            vfs::Path::try_from(file_str)
                        }else{
                            let mut actual_dir = cur_dir.clone();
                            actual_dir.push_str(file_str);
                            Ok(actual_dir)
                        };
                        let node = arg_path.map(|path|path.get_node());
                        let node = if let Ok(val) = node { val } else { writeln!(TERMINAL.lock(), "Invalid path!").unwrap(); continue; };
                        let node = if let Some(val) = node { val } else { writeln!(TERMINAL.lock(), "Path doesn't exist!").unwrap(); continue; };
                        if let Node::File(file) = node {
                            writeln!(TERMINAL.lock(), "File size: {} bytes!", (*file).borrow().get_size()).unwrap();
                            if let Some(data) = (*file).borrow().read(0, (*file).borrow().get_size()){
                                for e in data.iter() {
                                    write!(TERMINAL.lock(), "{}", if e.is_ascii() && !e.is_ascii_control() { *e as char} else { ' ' }).unwrap();
                                }
                            }else{
                                write!(TERMINAL.lock(), "Couldn't read file!").unwrap();
                            }
                        }else{
                            write!(TERMINAL.lock(), "Path should be a file!").unwrap();
                        }
                    }
                    writeln!(TERMINAL.lock()).unwrap();
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
                            cur_dir.push_str(name);
                        }
                        if cur_dir.get_node().is_none(){
                            writeln!(TERMINAL.lock(), "Invalid cd path: {}!", cur_dir).unwrap();
                            cur_dir = old_dir;
                        }
                    }
                }else if cmnd.contains("mkvfsdir"){
                    while let Some(name) = splat.next(){
                       VFSNode::new_folder(cur_dir.get_vfs_node().expect("Shell path should be valid at all times!"), name);
                    }
                }else if cmnd.contains("rmvfsdir"){
                    while let Some(name) = splat.next(){
                        let cur_node = cur_dir.get_vfs_node().expect("Shell path should be valid at all times!");
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

    writeln!(UART.lock(), "Heap usage: {} bytes", allocator::ALLOCATOR.lock().get_heap_used()).unwrap();
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
