#![no_std]
#![no_main]
#![feature(bench_black_box)]
#![feature(abi_efiapi)]
#![feature(default_alloc_error_handler)]
#![feature(lang_items)]
#![allow(dead_code)]

extern crate alloc;
extern crate rlibc_rust;

use core::cell::RefCell;
use core::cmp::min;
use core::convert::{TryFrom, TryInto};
use core::fmt::Debug;
use core::fmt::Write;

use alloc::borrow::ToOwned;
use alloc::rc::Rc;
use alloc::string::String;
use ata::{ATABus, ATADevice, ATADeviceFile};
use primitives::{LazyInitialised, Mutex};
use program::Program;
use vfs::{IFile, IFolder, Node, RootFSNode};
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
        while !$cond {
            core::hint::spin_loop()
        }
    };
}

trait X86Default {
    unsafe fn x86_default() -> Self;
}

#[panic_handler]
fn panic(p: &::core::panic::PanicInfo) -> ! {
    let mut s = String::new();
    let written = write!(s, "Ron: {}", p).is_ok(); // FIXME: Crashes on virtualbox and real hardware but not on qemu?
    if !UART.is_locked() {
        writeln!(UART.lock()).unwrap();
        if !written {
            writeln!(
                UART.lock(),
                "Bad panic, panic info cannot be formatted correctly, maybe OOM?"
            )
            .unwrap();
        } else {
            writeln!(UART.lock(), "{}", &s).unwrap();
        }
    }
    if !TERMINAL.is_locked() {
        let mut lock = TERMINAL.lock();
        lock.write_char('\n');
        if !written {
            "Bad panic, panic info cannot be formatted correctly, maybe OOM?\n"
                .chars()
                .for_each(|c| lock.write_char(c));
        } else {
            s.chars().for_each(|c| lock.write_char(c));
            lock.write_char('\n');
        }
    }
    loop {}
}

mod allocator;
mod ata;
mod char_device;
mod devfs;
mod efi;
mod elf;
mod emulator;
mod ext2;
mod framebuffer;
mod hio;
mod multiboot;
mod partitions;
mod primitives;
mod program;
mod ps2_8042;
mod syscall;
mod uart_16550;
mod vfs;
mod vga;
mod virtmem;


pub static UART: Mutex<LazyInitialised<UARTDevice>> = Mutex::from(LazyInitialised::uninit());

#[allow(unused)]
fn kprint_dump<T>(ptr: *const T, bytes: usize, uart: &mut UARTDevice) {
    let arr = unsafe {
        core::slice::from_raw_parts(
            core::mem::transmute::<_, *mut u32>(ptr),
            bytes / core::mem::size_of::<u32>(),
        )
    };
    for e in arr {
        write!(uart, "0x{:02X}", *e).unwrap();
    }
}

pub const unsafe fn from_utf8_unchecked(v: &[u8]) -> &str {
    // SAFETY: the caller must guarantee that the bytes `v` are valid UTF-8.
    // Also relies on `&str` and `&[u8]` having the same layout.
    core::mem::transmute(v)
}

static TERMINAL: Mutex<LazyInitialised<Terminal<'static>>> = Mutex::from(LazyInitialised::uninit());
struct Terminal<'a> {
    fb: &'a mut dyn FrameBuffer,
    cursor_pos: (usize, usize),
    cursor_char: char,
    color: Pixel,
}
impl Debug for Terminal<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Terminal")
            .field("cursor_pos", &self.cursor_pos)
            .field("cursor_char", &self.cursor_char)
            .field("color", &self.color)
            .finish()
    }
}

impl<'a> Write for Terminal<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().for_each(|c| self.write_char(c));
        Ok(())
    }
}

impl<'a> Terminal<'a> {
    fn new(fb: &'a mut dyn FrameBuffer, color: Pixel) -> Self {
        Terminal {
            fb,
            cursor_pos: (0, 0),
            cursor_char: ' ',
            color,
        }
    }
    fn clear(&mut self) {
        for i in 0..self.fb.get_height() {
            for j in 0..self.fb.get_width() {
                self.fb.set_pixel(j, i, Pixel { r: 0, g: 0, b: 0 });
            }
        }
        self.cursor_pos = (0, 0);
    }
    fn cursor_up(&mut self) {
        if self.cursor_pos.1 == 0 {
            return;
        }
        self.cursor_pos.1 -= 1;
    }
    fn cursor_down(&mut self) {
        if self.cursor_pos.1 >= self.fb.get_rows() - 1 {
            self.cursor_pos.1 = 0;
        } else {
            self.cursor_pos.1 += 1;
        }
    }
    fn cursor_right(&mut self) {
        if self.cursor_pos.0 >= self.fb.get_cols() - 1 {
            self.cursor_pos.0 = 0;
            self.cursor_down();
            for x in 0..self.fb.get_cols() {
                self.fb.write_char(x, self.cursor_pos.1, ' ', self.color);
            }
            return;
        }
        self.cursor_pos.0 += 1;
    }
    fn cursor_left(&mut self) {
        if self.cursor_pos.0 == 0 {
            return;
        }
        self.cursor_pos.0 -= 1;
    }

    pub fn visual_cursor_up(&mut self) {
        self.erase_visual_cursor();
        self.cursor_up();
        self.update_visual_cursor();
    }
    pub fn visual_cursor_left(&mut self) {
        self.erase_visual_cursor();
        self.cursor_left();
        self.update_visual_cursor();
    }
    pub fn visual_cursor_right(&mut self) {
        self.erase_visual_cursor();
        self.cursor_right();
        self.update_visual_cursor();
    }

    pub fn visual_cursor_down(&mut self) {
        self.erase_visual_cursor();
        self.cursor_down();
        self.update_visual_cursor();
    }

    fn update_visual_cursor(&mut self) {
        self.fb
            .write_char(self.cursor_pos.0, self.cursor_pos.1, '_', self.color);
    }

    fn erase_visual_cursor(&mut self) {
        self.fb.write_char(
            self.cursor_pos.0,
            self.cursor_pos.1,
            self.cursor_char,
            self.color,
        );
    }

    fn write_char(&mut self, c: char) {
        self.erase_visual_cursor(); // erase current cursor
        match c {
            '\n' => {
                self.cursor_down();
                for x in 0..self.fb.get_cols() {
                    self.fb.write_char(x, self.cursor_pos.1, ' ', self.color);
                }
                self.cursor_pos.0 = 0;
            }
            '\r' => {
                self.cursor_left(); // Go to char
                self.erase_visual_cursor();
            }
            c => {
                self.fb
                    .write_char(self.cursor_pos.0, self.cursor_pos.1, c, self.color);
                self.cursor_right();
            }
        }
        self.update_visual_cursor();
    }
}

// reg1 and reg2 are used for multiboot
#[no_mangle]
pub extern "C" fn main(r1: u32, r2: u32) -> ! {
    unsafe {
        UART.lock().set(UARTDevice::x86_default());
    }
    UART.lock().init();

    let multiboot_data = multiboot::init(r1 as usize, r2 as usize);
    writeln!(UART.lock(), "Hello, world!").unwrap();

    let mut efi_system_table_ptr = 0usize;
    let mut i = 0;
    loop {
        let id = multiboot_data[i];
        let mut len = multiboot_data[i + 1];
        if len % 8 != 0 {
            len += 8 - len % 8;
        }
        len /= core::mem::size_of::<u32>() as u32;
        if id == 0 && len == 2 {
            break;
        }

        if id == 0xB || id == 0xC {
            for j in i + 2..i + 2 + (core::mem::size_of::<usize>() / core::mem::size_of::<u32>()) {
                efi_system_table_ptr |= (multiboot_data[j] as usize) << ((j - i - 2) * 32);
                // FIXME: assumes little endian
            }
        }
        i += len as usize;
    }
    // FIXME: Don't hardcode the starting location of the heap
    // Stack size: 1mb, executable size (as of 22 may 2022): ~4mb, so starting the heap at 8mb should be a safe bet.
    allocator::ALLOCATOR
        .lock()
        .init((8 * 1024 * 1024) as *mut u8, 4 * 1024 * 1024);

    vfs::VFS_ROOT
        .lock()
        .set(Rc::new(RefCell::new(RootFSNode::new_root())));

    let dev_folder = vfs::RootFSNode::new_folder(vfs::VFS_ROOT.lock().clone(), "dev");
    let dfs = Rc::new(RefCell::new(devfs::DevFS::new()));
    (*dev_folder).borrow_mut().mountpoint = Some(dfs.clone() as Rc<RefCell<dyn IFolder>>);

    let vga;
    let mut fb: Option<&mut dyn framebuffer::FrameBuffer>;
    let o;
    let mut uo;
    fb = framebuffer::try_setup_efi_framebuffer(
        efi_system_table_ptr as *mut efi::EfiSystemTable,
        800,
        600,
    )
    .map(|x| x as &mut dyn framebuffer::FrameBuffer);
    if fb.is_none() {
        vga = unsafe { Vga::x86_default() };
        o = framebuffer::try_setup_vga_framebuffer(vga, 800, 600);
        if o.is_some() {
            uo = o.unwrap();
            fb = Some(unsafe {
                &mut *((&mut uo) as *mut Vga<Color256, Unblanked>) as &mut dyn FrameBuffer
            });
        }
    }
    let fb = fb.unwrap();

    fb.fill(
        0,
        0,
        fb.get_width(),
        fb.get_height(),
        Pixel { r: 0, g: 0, b: 0 },
    );
    TERMINAL.lock().set(Terminal::new(
        fb,
        Pixel {
            r: 0x0,
            g: 0xa8,
            b: 0x54,
        },
    ));

    writeln!(UART.lock(), "If you see this then that means the framebuffer subsystem didn't instantly crash the kernel :)").unwrap();
    writeln!(TERMINAL.lock(), "Hello, world!").unwrap();

    if let Some(primary_ata_bus) = unsafe { ATABus::primary_x86() } {
        let ata_ref = Rc::new(RefCell::new(primary_ata_bus));
        // NOTE: master device is not necessarilly the device from which the os was booted

        if unsafe {
            (*ata_ref)
                .borrow_mut()
                .identify(ATADevice::MASTER)
                .is_some()
        } {
            let master_dev = Rc::new(RefCell::new(ATADeviceFile {
                bus: ata_ref.clone(),
                bus_device: ATADevice::MASTER,
            }));
            (*dfs).borrow_mut().add_device_file(
                master_dev.clone() as Rc<RefCell<dyn IFile>>,
                "hda".to_owned(),
            );
            for part_number in 0..4 {
                if let Some(part_dev) = partitions::MBRPartitionFile::from(
                    master_dev.clone() as Rc<RefCell<dyn IFile>>,
                    part_number.try_into().unwrap(),
                ) {
                    let mut part_dev_name = String::new();
                    write!(part_dev_name, "hdap{}", part_number + 1).unwrap();
                    writeln!(
                        TERMINAL.lock(),
                        "Found partition {}, with offset in bytes from begining of: {}",
                        part_dev_name,
                        part_dev.get_offset()
                    )
                    .unwrap();
                    (*dfs).borrow_mut().add_device_file(
                        Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>,
                        part_dev_name,
                    );
                }
            }
        }

        if unsafe { (*ata_ref).borrow_mut().identify(ATADevice::SLAVE).is_some() } {
            let slave_dev = Rc::new(RefCell::new(ATADeviceFile {
                bus: ata_ref.clone(),
                bus_device: ATADevice::SLAVE,
            }));
            (*dfs).borrow_mut().add_device_file(
                slave_dev.clone() as Rc<RefCell<dyn IFile>>,
                "hdb".to_owned(),
            );
            for part_number in 0..4 {
                if let Some(part_dev) = partitions::MBRPartitionFile::from(
                    slave_dev.clone() as Rc<RefCell<dyn IFile>>,
                    part_number.try_into().unwrap(),
                ) {
                    let mut part_dev_name = String::new();
                    write!(part_dev_name, "hdbp{}", part_number + 1).unwrap();
                    writeln!(
                        TERMINAL.lock(),
                        "Found partition {}, with offset in bytes from begining of: {}",
                        part_dev_name,
                        part_dev.get_offset()
                    )
                    .unwrap();
                    (*dfs).borrow_mut().add_device_file(
                        Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>,
                        part_dev_name,
                    );
                }
            }
        }
    }

    if let Some(secondary_ata_bus) = unsafe { ATABus::secondary_x86() } {
        let ata_ref = Rc::new(RefCell::new(secondary_ata_bus));
        // NOTE: master device is not necessarilly the device from which the os was booted

        if unsafe {
            (*ata_ref)
                .borrow_mut()
                .identify(ATADevice::MASTER)
                .is_some()
        } {
            let master_dev = Rc::new(RefCell::new(ATADeviceFile {
                bus: ata_ref.clone(),
                bus_device: ATADevice::MASTER,
            }));
            (*dfs).borrow_mut().add_device_file(
                master_dev.clone() as Rc<RefCell<dyn IFile>>,
                "hdc".to_owned(),
            );
            for part_number in 0..4 {
                if let Some(part_dev) = partitions::MBRPartitionFile::from(
                    master_dev.clone() as Rc<RefCell<dyn IFile>>,
                    part_number.try_into().unwrap(),
                ) {
                    let mut part_dev_name = String::new();
                    write!(part_dev_name, "hdcp{}", part_number + 1).unwrap();
                    writeln!(
                        TERMINAL.lock(),
                        "Found partition {}, with offset in bytes from begining of: {}",
                        part_dev_name,
                        part_dev.get_offset()
                    )
                    .unwrap();
                    (*dfs).borrow_mut().add_device_file(
                        Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>,
                        part_dev_name,
                    );
                }
            }
        }

        if unsafe { (*ata_ref).borrow_mut().identify(ATADevice::SLAVE).is_some() } {
            let slave_dev = Rc::new(RefCell::new(ATADeviceFile {
                bus: ata_ref.clone(),
                bus_device: ATADevice::SLAVE,
            }));
            (*dfs).borrow_mut().add_device_file(
                slave_dev.clone() as Rc<RefCell<dyn IFile>>,
                "hdd".to_owned(),
            );
            for part_number in 0..4 {
                if let Some(part_dev) = partitions::MBRPartitionFile::from(
                    slave_dev.clone() as Rc<RefCell<dyn IFile>>,
                    part_number.try_into().unwrap(),
                ) {
                    let mut part_dev_name = String::new();
                    write!(part_dev_name, "hddp{}", part_number + 1).unwrap();
                    writeln!(
                        TERMINAL.lock(),
                        "Found partition {}, with offset in bytes from begining of: {}",
                        part_dev_name,
                        part_dev.get_offset()
                    )
                    .unwrap();
                    (*dfs).borrow_mut().add_device_file(
                        Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>,
                        part_dev_name,
                    );
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

        if b.typ == KeyboardPacketType::KeyReleased && b.special_keys.esc {
            break;
        }
        if b.typ == KeyboardPacketType::KeyReleased {
            continue;
        }

        if b.special_keys.up_arrow {
            TERMINAL.lock().visual_cursor_up();
        } else if b.special_keys.down_arrow {
            TERMINAL.lock().visual_cursor_down();
        } else if b.special_keys.right_arrow {
            TERMINAL.lock().visual_cursor_right();
        } else if b.special_keys.left_arrow {
            TERMINAL.lock().visual_cursor_left();
        }

        let mut c = match b.char_codepoint {
            Some(v) => v,
            None => continue,
        };

        if b.special_keys.any_shift() {
            c = b.shift_codepoint().unwrap();
        }

        TERMINAL.lock().write_char(c);
        if c == '\r' {
            ignore_inc_x = true;
            if buf_ind > 0 {
                buf_ind -= 1;
            }
        }
        if c == '\n' {
            let bufs = unsafe { from_utf8_unchecked(&cmd_buf[..buf_ind]) }.trim();
            buf_ind = 0; // Flush buffer
            let mut splat = bufs.split_inclusive(' ');
            if let Some(cmnd) = splat.next() {
                // Handle shell built ins
                if cmnd.contains("puts") {
                    let mut puts_output: String = String::new();
                    let mut redirect: Option<String> = None;
                    while let Some(arg) = splat.next() {
                        if arg.trim().starts_with('>') {
                            redirect = Some(arg.trim()[1..].to_owned());
                            continue;
                        }

                        if let Some(ref mut redir) = redirect {
                            redir.push_str(arg);
                        } else {
                            puts_output.push_str(arg);
                        }
                    }

                    if let Some(redir_str) = redirect {
                        let path = if redir_str.starts_with('/') {
                            vfs::Path::try_from(redir_str).ok()
                        } else {
                            let mut actual_dir = cur_dir.clone();
                            actual_dir.push_str(redir_str.as_str());
                            Some(actual_dir)
                        };
                        if let Some(node) = path.map(|path| path.get_node()) {
                            if let Some(Node::File(file)) = node {
                                if (*file)
                                    .borrow_mut()
                                    .resize(puts_output.len() as u64)
                                    .is_some()
                                {
                                    if (*file)
                                        .borrow_mut()
                                        .write(0, puts_output.as_bytes())
                                        .is_none()
                                    {
                                        writeln!(TERMINAL.lock(), "Couldn't write to file!")
                                            .unwrap();
                                    }
                                } else {
                                    writeln!(TERMINAL.lock(), "Couldn't resize file!").unwrap();
                                }
                            } else {
                                writeln!(TERMINAL.lock(), "Redirect path should be valid!")
                                    .unwrap();
                            }
                        }
                    } else {
                        write!(TERMINAL.lock(), "{}", puts_output).unwrap();
                    };

                    writeln!(TERMINAL.lock()).unwrap();
                } else if cmnd.contains("whoareyou") {
                    writeln!(TERMINAL.lock(), "Ron").unwrap();
                } else if cmnd.contains("help") {
                    writeln!(TERMINAL.lock(), "puts whoareyou rmrootfsdir mkrootfsdir rm touch mount.ext2 umount free hexdump cat ls cd clear exit help").unwrap();
                } else if cmnd.contains("clear") {
                    TERMINAL.lock().clear();
                } else if cmnd.contains("free") {
                    let heap_used = ALLOCATOR.lock().get_heap_used();
                    let heap_max = ALLOCATOR.lock().get_heap_max();
                    writeln!(
                        TERMINAL.lock(),
                        "{} bytes of {} bytes used on heap, that's {}% !",
                        heap_used,
                        heap_max,
                        heap_used as f32 / heap_max as f32 * 100.0
                    )
                    .unwrap();
                } else if cmnd.contains("mount.ext2") {
                    if let (Some(file), Some(mntpoint)) = (splat.next(), splat.next()) {
                        let mut file_node = vfs::Path::try_from(file.trim());
                        if !file.starts_with("/") {
                            let mut actual_node = cur_dir.clone();
                            actual_node.push_str(file);
                            file_node = Ok(actual_node);
                        }
                        let file_node = if let Ok(val) = file_node {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Malformed source path: \"{}\"!", file)
                                .unwrap();
                            continue;
                        };
                        let file_node = if let Some(val) = file_node.get_node() {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Source path: \"{}\" does not exist!", file)
                                .unwrap();
                            continue;
                        };
                        let file_node = if let vfs::Node::File(val) = file_node {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Source path: \"{}\" is not a file!", file)
                                .unwrap();
                            continue;
                        };
                        let e2fs = ext2::Ext2FS::new(file_node, false);
                        let e2fs = if let Some(val) = e2fs {
                            val
                        } else {
                            writeln!(
                                TERMINAL.lock(),
                                "Source file does not contain a valid ext2 fs!"
                            )
                            .unwrap();
                            continue;
                        };
                        let e2fs = Rc::new(RefCell::new(e2fs));

                        let root_inode = (*e2fs)
                            .borrow_mut()
                            .read_inode(2)
                            .expect("Root inode should exist!")
                            .as_vfs_node(e2fs.clone(), 2)
                            .expect("Root inode should be parsable in vfs!")
                            .expect_folder();
                        let mut mntpoint_node = vfs::Path::try_from(mntpoint.trim());
                        if !mntpoint.starts_with("/") {
                            let mut actual_node = cur_dir.clone();
                            actual_node.push_str(mntpoint);
                            mntpoint_node = Ok(actual_node);
                        }
                        let mntpoint_node = if let Ok(val) = mntpoint_node {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Malformed mountpoint path!").unwrap();
                            continue;
                        };
                        let mntpoint_node = if let Some(val) = mntpoint_node.get_rootfs_node() {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Mountpoint should exist in vfs!").unwrap();
                            continue;
                        };
                        (*mntpoint_node).borrow_mut().mountpoint = Some(root_inode);
                    } else {
                        writeln!(TERMINAL.lock(), "Not enough arguments!").unwrap();
                    }
                } else if cmnd.contains("umount") {
                    if let Some(mntpoint) = splat.next() {
                        let mut mntpoint_node = vfs::Path::try_from(mntpoint.trim());
                        if !mntpoint.starts_with("/") {
                            let mut actual_node = cur_dir.clone();
                            actual_node.push_str(mntpoint);
                            mntpoint_node = Ok(actual_node);
                        }
                        let mntpoint_node = if let Ok(val) = mntpoint_node {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Malformed mountpoint path!").unwrap();
                            continue;
                        };
                        let mntpoint_node = if let Some(val) = mntpoint_node.get_rootfs_node() {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Mountpoint should exist in vfs!").unwrap();
                            continue;
                        };
                        (*mntpoint_node).borrow_mut().mountpoint = None;
                    } else {
                        writeln!(TERMINAL.lock(), "Not enough arguments!").unwrap();
                    }
                } else if cmnd.contains("ls") {
                    for subnode in (*cur_dir
                        .get_node()
                        .expect("Shell path should be valid at all times!")
                        .expect_folder())
                    .borrow()
                    .get_children()
                    {
                        write!(TERMINAL.lock(), "{} ", subnode.0).unwrap();
                        if let Node::File(f) = subnode.1 {
                            write!(
                                TERMINAL.lock(),
                                "(size: {} kb) ",
                                (*f).borrow().get_size() as f32 / 1024.0
                            )
                            .unwrap();
                        }
                    }
                    writeln!(TERMINAL.lock()).unwrap();
                } else if cmnd.contains("hexdump") {
                    if let (Some(offset_str), Some(file_str)) = (splat.next(), splat.next()) {
                        if let Ok(offset) = offset_str.trim().parse::<usize>() {
                            let arg_path = if file_str.starts_with('/') {
                                vfs::Path::try_from(file_str)
                            } else {
                                let mut actual_dir = cur_dir.clone();
                                actual_dir.push_str(file_str);
                                Ok(actual_dir)
                            };

                            let node = arg_path.map(|path| path.get_node());
                            let node = if let Ok(val) = node {
                                val
                            } else {
                                writeln!(TERMINAL.lock(), "Invalid path!").unwrap();
                                continue;
                            };
                            let node = if let Some(val) = node {
                                val
                            } else {
                                writeln!(TERMINAL.lock(), "Path doesn't exist!").unwrap();
                                continue;
                            };

                            if let Node::File(file) = node {
                                if let Some(data) = (*file).borrow().read(
                                    offset as u64,
                                    min(16, (*file).borrow().get_size() as usize),
                                ) {
                                    for e in data.iter() {
                                        write!(TERMINAL.lock(), "0x{:02X} ", e).unwrap();
                                    }
                                } else {
                                    write!(TERMINAL.lock(), "Couldn't read file!").unwrap();
                                }
                            } else {
                                write!(TERMINAL.lock(), "Path should be a file!").unwrap();
                            }
                        } else {
                            write!(TERMINAL.lock(), "Bad offset!").unwrap();
                        }
                    } else {
                        write!(TERMINAL.lock(), "Not enough arguments!").unwrap();
                    }

                    writeln!(TERMINAL.lock()).unwrap();
                } else if cmnd.contains("cat") {
                    if let Some(file_str) = splat.next() {
                        let arg_path = if file_str.starts_with('/') {
                            vfs::Path::try_from(file_str)
                        } else {
                            let mut actual_dir = cur_dir.clone();
                            actual_dir.push_str(file_str);
                            Ok(actual_dir)
                        };
                        let node = arg_path.map(|path| path.get_node());
                        let node = if let Ok(val) = node {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Invalid path!").unwrap();
                            continue;
                        };
                        let node = if let Some(val) = node {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Path doesn't exist!").unwrap();
                            continue;
                        };
                        if let Node::File(file) = node {
                            writeln!(
                                TERMINAL.lock(),
                                "File size: {} bytes!",
                                (*file).borrow().get_size()
                            )
                            .unwrap();
                            if let Some(data) = (*file)
                                .borrow()
                                .read(0, (*file).borrow().get_size() as usize)
                            {
                                for e in data.iter() {
                                    write!(
                                        TERMINAL.lock(),
                                        "{}",
                                        if e.is_ascii() && !e.is_ascii_control() {
                                            *e as char
                                        } else {
                                            ' '
                                        }
                                    )
                                    .unwrap();
                                }
                            } else {
                                write!(TERMINAL.lock(), "Couldn't read file!").unwrap();
                            }
                        } else {
                            write!(TERMINAL.lock(), "Path should be a file!").unwrap();
                        }
                    }
                    writeln!(TERMINAL.lock()).unwrap();
                } else if cmnd.contains("touch") {
                    while let Some(name) = splat.next() {
                        let arg_path = if name.starts_with('/') {
                            vfs::Path::try_from(name)
                        } else {
                            let mut actual_dir = cur_dir.clone();
                            actual_dir.push_str(name);
                            Ok(actual_dir)
                        };
                        let mut arg_path = if let Ok(val) = arg_path {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Bad path!").unwrap();
                            continue;
                        };
                        let name = arg_path.last().to_owned();
                        arg_path.del_last();

                        let node = if let Some(val) = arg_path.get_node() {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Non-existant path!").unwrap();
                            continue;
                        };
                        if let Node::Folder(folder) = node {
                            if folder
                                .borrow_mut()
                                .create_empty_child(&name, vfs::NodeType::File)
                                .is_none()
                            {
                                writeln!(TERMINAL.lock(), "Failed to touch file!").unwrap();
                            }
                        }
                    }
                } else if cmnd.contains("cd") {
                    if let Some(name) = splat.next() {
                        let name = name.trim();
                        let old_dir = cur_dir.clone();
                        if name == ".." {
                            cur_dir.del_last();
                        } else if name.starts_with("/") {
                            if let Ok(new_dir) = name.try_into() {
                                cur_dir = new_dir;
                            } else {
                                writeln!(TERMINAL.lock(), "Invalid cd path!").unwrap();
                            }
                        } else {
                            cur_dir.push_str(name);
                        }
                        if cur_dir.get_node().is_none() {
                            writeln!(TERMINAL.lock(), "Invalid cd path: {}!", cur_dir).unwrap();
                            cur_dir = old_dir;
                        }
                    }
                } else if cmnd.contains("mkrootfsdir") {
                    while let Some(name) = splat.next() {
                        RootFSNode::new_folder(
                            cur_dir
                                .get_rootfs_node()
                                .expect("Shell path should be valid at all times!"),
                            name,
                        );
                    }
                } else if cmnd.contains("rmrootfsdir") {
                    while let Some(name) = splat.next() {
                        let cur_node = cur_dir
                            .get_rootfs_node()
                            .expect("Shell path should be valid at all times!");
                        // Empty folder check
                        if let Some(child_to_sacrifice) =
                            RootFSNode::find_folder(cur_node.clone(), name)
                        {
                            if (*child_to_sacrifice).borrow().get_children().len() != 0 {
                                writeln!(TERMINAL.lock(), "Folder: \"{}\", is non-empty!", name)
                                    .unwrap();
                                break;
                            }
                        } else {
                            writeln!(TERMINAL.lock(), "Folder: \"{}\", does not exist!", name)
                                .unwrap();
                            continue;
                        }
                        ////

                        if !RootFSNode::del_folder(cur_node, name) {
                            writeln!(TERMINAL.lock(), "Couldn't delete folder: \"{}\"!", name)
                                .unwrap();
                        }
                    }
                } else if cmnd.contains("rm") {
                    while let Some(name) = splat.next() {
                        let arg_path = if name.starts_with('/') {
                            vfs::Path::try_from(name)
                        } else {
                            let mut actual_dir = cur_dir.clone();
                            actual_dir.push_str(name);
                            Ok(actual_dir)
                        };
                        let mut arg_path = if let Ok(val) = arg_path {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Bad path!").unwrap();
                            continue;
                        };
                        let file_name = arg_path.last().to_owned();
                        arg_path.del_last();

                        let node = if let Some(val) = arg_path.get_node() {
                            val
                        } else {
                            writeln!(TERMINAL.lock(), "Non-existant path!").unwrap();
                            continue;
                        };
                        if let Node::Folder(folder) = node {
                            let child = if let Some(val) = folder
                                .borrow_mut()
                                .get_children()
                                .into_iter()
                                .find(|child| child.0 == file_name)
                            {
                                val.1
                            } else {
                                writeln!(TERMINAL.lock(), "File doesn't exist in folder!").unwrap();
                                continue;
                            };
                            let child = if let Node::File(f) = child {
                                f
                            } else {
                                writeln!(TERMINAL.lock(), "Not a file!").unwrap();
                                continue;
                            };

                            writeln!(TERMINAL.lock(), "Removing the data from \"{}\"!", name)
                                .unwrap();
                            if child.borrow_mut().resize(0).is_none() {
                                writeln!(TERMINAL.lock(), "Failed to remove the data!").unwrap();
                            } else {
                                writeln!(TERMINAL.lock(), "Deleting/unlinking file!").unwrap();
                                if folder
                                    .borrow_mut()
                                    .unlink_or_delete_empty_child(&name)
                                    .is_none()
                                {
                                    writeln!(TERMINAL.lock(), "Failed to delete/unlink file!")
                                        .unwrap();
                                }
                            }
                        }
                    }
                } else if cmnd.contains("elp") {
                    writeln!(TERMINAL.lock(), "NOPERS, no elp!").unwrap();
                } else if cmnd.contains("exit") {
                    break 'big_loop;
                } else {
                    let executable_path = if cmnd.starts_with('/') {
                        vfs::Path::try_from(cmnd)
                    } else if cmnd.starts_with('.') {
                        let mut actual_dir = cur_dir.clone();
                        actual_dir.push_str(cmnd);
                        Ok(actual_dir)
                    } else {
                        Err(())
                    };
                    let executable_path = if let Ok(val) = executable_path {
                        val
                    } else {
                        writeln!(TERMINAL.lock(), "Unrecognised command!").unwrap();
                        continue;
                    };
                    let node = executable_path.get_node();
                    let node = if let Some(val) = node {
                        val
                    } else {
                        writeln!(TERMINAL.lock(), "Invalid executable path!").unwrap();
                        continue;
                    };
                    if let Node::File(executable) = node {
                        let contents = executable
                            .borrow()
                            .read(0, executable.borrow().get_size() as usize);
                        let contents = if let Some(res) = contents {
                            res
                        } else {
                            writeln!(TERMINAL.lock(), "Failed to read executable!").unwrap();
                            continue;
                        };
                        {
                            let elf = elf::ElfFile::from_bytes(&contents);
                            let elf = if let Some(res) = elf {
                                res
                            } else {
                                writeln!(TERMINAL.lock(), "Executable is not an elf file!")
                                    .unwrap();
                                continue;
                            };

                            writeln!(
                                UART.lock(),
                                "Program entry point: {}",
                                elf.header.program_entry
                            )
                            .unwrap();
                            writeln!(
                                UART.lock(),
                                "Number of parsed program headers in elf: {}",
                                elf.program_headers.len()
                            )
                            .unwrap();
                            for header in elf.program_headers {
                                writeln!(UART.lock(), "Found program header in elf file, type: {:?}, in-file offset: {}, in-file size: {}, virtual offset: {}, virtual size: {}, flags: {:?}", header.segment_type, header.segment_file_offset, header.segment_file_size, header.segment_virtual_address, header.segment_virtual_size, header.flags).unwrap();
                            }
                        }
                        let mut program = if let Some(p) = Program::from_elf(&contents) {
                            p
                        } else {
                            writeln!(TERMINAL.lock(), "Failed to load elf file into program!")
                                .unwrap();
                            continue;
                        };
                        program.run();
                        writeln!(UART.lock(), "CPU state after program ended: {:?}", program)
                            .unwrap();
                    } else {
                        writeln!(TERMINAL.lock(), "Executable path is not a file!").unwrap();
                    }
                }
            }

            write!(TERMINAL.lock(), "{} # ", cur_dir).unwrap();
            continue;
        }

        if buf_ind < cmd_buf.len() {
            cmd_buf[buf_ind] = c as u8;
            if !ignore_inc_x {
                buf_ind += 1;
            }
        }
    }

    writeln!(
        UART.lock(),
        "Heap usage: {} bytes",
        allocator::ALLOCATOR.lock().get_heap_used()
    )
    .unwrap();

    // Shutdown
    writeln!(UART.lock(), "\nIt's now safe to turn off your computer!").unwrap();

    let width = TERMINAL.lock().fb.get_width();
    let height = TERMINAL.lock().fb.get_height();
    let cols = TERMINAL.lock().fb.get_cols();
    let rows = TERMINAL.lock().fb.get_rows();
    TERMINAL
        .lock()
        .fb
        .fill(0, 0, width, height, Pixel { r: 0, g: 0, b: 0 });
    let s = "It's now safe to turn off your computer!";
    s.chars().enumerate().for_each(|(ind, c)| {
        TERMINAL.lock().fb.write_char(
            ind + cols / 2 - s.len() / 2,
            (rows - 1) / 2,
            c,
            Pixel {
                r: 0xff,
                g: 0xff,
                b: 0x55,
            },
        );
    });

    loop {}
}
