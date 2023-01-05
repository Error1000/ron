#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![feature(default_alloc_error_handler)]
#![feature(lang_items)]
#![feature(allocator_api)]
#![allow(dead_code)]

extern crate alloc;
extern crate rlibc;

use core::cell::RefCell;
use core::cmp::min;
use core::convert::{TryFrom, TryInto};
use core::fmt::Write;

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use allocator::PROGRAM_ALLOCATOR;
use ata::{ATABus, ATADevice, ATADeviceFile};
use char_device::CharDevice;
use hio::{KeyboardKey, standard_usa_qwerty};
use primitives::{LazyInitialised, Mutex};
use process::Process;
use ps2_8042::KEYBOARD_INPUT;
use terminal::{Terminal, TERMINAL};
use vfs::{IFile, IFolder, Node, RootFSNode};
use vga::{Color256, Unblanked};

use crate::allocator::ALLOCATOR;
use crate::framebuffer::{FrameBuffer, Pixel};
use crate::hio::KeyboardPacketType;
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
            writeln!(UART.lock(), "Bad panic, panic info cannot be formatted correctly, maybe OOM?").unwrap();
        } else {
            writeln!(UART.lock(), "{}", &s).unwrap();
        }
    }
    if !TERMINAL.is_locked() {
        let mut lock = TERMINAL.lock();
        lock.write_char('\n');
        if !written {
            "Bad panic, panic info cannot be formatted correctly, maybe OOM?\n".chars().for_each(|c| lock.write_char(c));
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
mod process;
mod scheduler;
mod ps2_8042;
mod syscall;
mod terminal;
mod uart_16550;
mod vfs;
mod vga;
mod virtmem;

pub static UART: Mutex<LazyInitialised<UARTDevice>> = Mutex::from(LazyInitialised::uninit());

#[allow(unused)]
fn kprint_dump<T>(ptr: *const T, bytes: usize, uart: &mut UARTDevice) {
    let arr =
        unsafe { core::slice::from_raw_parts(core::mem::transmute::<_, *mut u32>(ptr), bytes / core::mem::size_of::<u32>()) };
    for e in arr {
        write!(uart, "0x{:02X}", *e).unwrap();
    }
}

pub const unsafe fn from_utf8_unchecked(v: &[u8]) -> &str {
    // SAFETY: the caller must guarantee that the bytes `v` are valid UTF-8.
    // Also relies on `&str` and `&[u8]` having the same layout.
    core::mem::transmute(v)
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
    // Stack size: 2mb, executable size (as of 17 sep 2022): ~6mb, so starting the heap at 8mb should be a safe bet.
    allocator::ALLOCATOR.lock().init((8 * 1024 * 1024) as *mut u8, 8 * 1024 * 1024);
    allocator::PROGRAM_ALLOCATOR.0.lock().init((16 * 1024 * 1024) as *mut u8, 240 * 1024 * 1024);

    vfs::VFS_ROOT.lock().set(Rc::new(RefCell::new(RootFSNode::new_root())));

    let dev_folder = vfs::RootFSNode::new_folder(vfs::VFS_ROOT.lock().clone(), "dev");
    let dfs = Rc::new(RefCell::new(devfs::DevFS::new()));
    (*dev_folder).borrow_mut().mountpoint = Some(dfs.clone() as Rc<RefCell<dyn IFolder>>);

    let vga;
    let mut fb: Option<&mut dyn framebuffer::FrameBuffer>;
    let o;
    let mut uo;
    fb = framebuffer::try_setup_efi_framebuffer(efi_system_table_ptr as *mut efi::EfiSystemTable, 800, 600)
        .map(|x| x as &mut dyn framebuffer::FrameBuffer);
    if fb.is_none() {
        vga = unsafe { Vga::x86_default() };
        o = framebuffer::try_setup_vga_framebuffer(vga, 800, 600);
        if o.is_some() {
            uo = o.unwrap();
            fb = Some(unsafe { &mut *((&mut uo) as *mut Vga<Color256, Unblanked>) as &mut dyn FrameBuffer });
        }
    }
    let fb = fb.unwrap();

    fb.fill(0, 0, fb.get_width(), fb.get_height(), Pixel { r: 0, g: 0, b: 0 });
    TERMINAL.lock().set(Terminal::new(fb, Pixel { r: 0x0, g: 0xa8, b: 0x54 }));

    writeln!(UART.lock(), "If you see this then that means the framebuffer subsystem didn't instantly crash the kernel :)")
        .unwrap();
    writeln!(TERMINAL.lock(), "Hello, world!").unwrap();

    if let Some(primary_ata_bus) = unsafe { ATABus::primary_x86() } {
        let ata_ref = Rc::new(RefCell::new(primary_ata_bus));
        // NOTE: master device is not necessarilly the device from which the os was booted

        if unsafe { (*ata_ref).borrow_mut().identify(ATADevice::MASTER).is_some() } {
            let master_dev = Rc::new(RefCell::new(ATADeviceFile { bus: ata_ref.clone(), bus_device: ATADevice::MASTER }));
            (*dfs).borrow_mut().add_device_file(master_dev.clone() as Rc<RefCell<dyn IFile>>, "hda".to_owned());
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
                    (*dfs)
                        .borrow_mut()
                        .add_device_file(Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>, part_dev_name);
                }
            }
        }

        if unsafe { (*ata_ref).borrow_mut().identify(ATADevice::SLAVE).is_some() } {
            let slave_dev = Rc::new(RefCell::new(ATADeviceFile { bus: ata_ref.clone(), bus_device: ATADevice::SLAVE }));
            (*dfs).borrow_mut().add_device_file(slave_dev.clone() as Rc<RefCell<dyn IFile>>, "hdb".to_owned());
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
                    (*dfs)
                        .borrow_mut()
                        .add_device_file(Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>, part_dev_name);
                }
            }
        }
    }

    if let Some(secondary_ata_bus) = unsafe { ATABus::secondary_x86() } {
        let ata_ref = Rc::new(RefCell::new(secondary_ata_bus));
        // NOTE: master device is not necessarily the device from which the os was booted

        if unsafe { (*ata_ref).borrow_mut().identify(ATADevice::MASTER).is_some() } {
            let master_dev = Rc::new(RefCell::new(ATADeviceFile { bus: ata_ref.clone(), bus_device: ATADevice::MASTER }));
            (*dfs).borrow_mut().add_device_file(master_dev.clone() as Rc<RefCell<dyn IFile>>, "hdc".to_owned());
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
                    (*dfs)
                        .borrow_mut()
                        .add_device_file(Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>, part_dev_name);
                }
            }
        }

        if unsafe { (*ata_ref).borrow_mut().identify(ATADevice::SLAVE).is_some() } {
            let slave_dev = Rc::new(RefCell::new(ATADeviceFile { bus: ata_ref.clone(), bus_device: ATADevice::SLAVE }));
            (*dfs).borrow_mut().add_device_file(slave_dev.clone() as Rc<RefCell<dyn IFile>>, "hdd".to_owned());
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
                    (*dfs)
                        .borrow_mut()
                        .add_device_file(Rc::new(RefCell::new(part_dev)) as Rc<RefCell<dyn IFile>>, part_dev_name);
                }
            }
        }
    }

    scheduler::init();


    KEYBOARD_INPUT.lock().set(unsafe { ps2_8042::PS2Device::x86_default() });

    let mut cur_dir = vfs::Path::try_from("/").unwrap();
    write!(TERMINAL.lock(), "{} # ", cur_dir).unwrap();

    let mut ignore_inc_x: bool;
    // Basically an ad-hoc ArrayString (arrayvec crate)
    let mut cmd_buf: [u8; 2048] = [b' '; 2048];
    let mut buf_ind = 0; // Also length of buf, a.k.a portion of buf used
    'big_loop: loop {
        ignore_inc_x = false;
        let packet = unsafe { KEYBOARD_INPUT.lock().read_packet() };

        if packet.packet_type == KeyboardPacketType::KeyReleased && packet.key == KeyboardKey::Escape {
            break;
        }

        if packet.packet_type == KeyboardPacketType::KeyReleased {
            continue;
        }

        if packet.key == KeyboardKey::UpArrow {
            TERMINAL.lock().visual_cursor_up();
        } else if packet.key == KeyboardKey::DownArrow {
            TERMINAL.lock().visual_cursor_down();
        } else if packet.key == KeyboardKey::RightArrow {
            TERMINAL.lock().visual_cursor_right();
        } else if packet.key == KeyboardKey::LeftArrow {
            TERMINAL.lock().visual_cursor_left();
        }

        if packet.key == KeyboardKey::Backspace {
            ignore_inc_x = true;
            if buf_ind > 0 {
                buf_ind -= 1;
            }
        }

        TERMINAL.lock().recive_key(packet.key, packet.modifiers);

        let Ok(c) = standard_usa_qwerty::parse_key(packet.key, packet.modifiers) else { continue; };

        if c == '\n' {
            let bufs = unsafe { from_utf8_unchecked(&cmd_buf[..buf_ind]) }.trim();
            buf_ind = 0; // Flush buffer
            let mut splat = bufs.split_inclusive(' ');
            if let Some(cmnd) = splat.next() {
                // Handle shell built ins
                if cmnd.starts_with("puts") {
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
                            actual_dir.append_str(redir_str.as_str());
                            Some(actual_dir)
                        };
                        if let Some(node) = path.map(|path| path.get_node()) {
                            if let Some(Node::File(file)) = node {
                                if (*file).borrow_mut().resize(puts_output.len() as u64).is_some() {
                                    if (*file).borrow_mut().write(0, puts_output.as_bytes()).is_none() {
                                        writeln!(TERMINAL.lock(), "Couldn't write to file!").unwrap();
                                    }
                                } else {
                                    writeln!(TERMINAL.lock(), "Couldn't resize file!").unwrap();
                                }
                            } else {
                                writeln!(TERMINAL.lock(), "Redirect path should be valid!").unwrap();
                            }
                        }
                    } else {
                        write!(TERMINAL.lock(), "{}", puts_output).unwrap();
                    };

                    writeln!(TERMINAL.lock()).unwrap();
                } else if cmnd.starts_with("whoareyou") {
                    writeln!(TERMINAL.lock(), "Ron").unwrap();
                } else if cmnd.starts_with("help") {
                    writeln!(
                        TERMINAL.lock(),
                        "puts whoareyou rmrootfsdir mkrootfsdir rm touch mount.ext2 umount free hexdump ls cd clear exit help"
                    )
                    .unwrap();
                } else if cmnd.starts_with("clear") {
                    TERMINAL.lock().clear();
                } else if cmnd.starts_with("free") {
                    let kernel_heap_used = ALLOCATOR.lock().get_heap_used();
                    let program_heap_used = PROGRAM_ALLOCATOR.0.lock().get_heap_used();
                    let kernel_heap_max = ALLOCATOR.lock().get_heap_max();
                    let program_heap_max = PROGRAM_ALLOCATOR.0.lock().get_heap_max();
                    writeln!(
                        TERMINAL.lock(),
                        "{} bytes of {} bytes used on heap, that's {}% !",
                        kernel_heap_used+program_heap_used,
                        kernel_heap_max+program_heap_max,
                        (kernel_heap_used+program_heap_used) as f32 / (kernel_heap_max+program_heap_max) as f32 * 100.0
                    )
                    .unwrap();

                    writeln!(TERMINAL.lock(), "Breakdown: {}% used of kernel heap, and {}% of program heap!", (kernel_heap_used as f32/kernel_heap_max as f32) * 100.0, (program_heap_used as f32/program_heap_max as f32)*100.0).unwrap();
                } else if cmnd.starts_with("mount.ext2") {
                    if let (Some(file), Some(mntpoint)) = (splat.next(), splat.next()) {
                        let mut file_node = vfs::Path::try_from(file.trim());
                        if !file.starts_with("/") {
                            let mut actual_node = cur_dir.clone();
                            actual_node.append_str(file);
                            file_node = Ok(actual_node);
                        }
                        let Ok(file_node) = file_node else {
                            writeln!(TERMINAL.lock(), "Malformed source path: \"{}\"!", file).unwrap();
                            continue;
                        };

                        let Some(file_node) = file_node.get_node() else {
                            writeln!(TERMINAL.lock(), "Source path: \"{}\" does not exist!", file).unwrap();
                            continue;
                        };

                        let vfs::Node::File(file_node) = file_node else {
                            writeln!(TERMINAL.lock(), "Source path: \"{}\" is not a file!", file).unwrap();
                            continue;
                        };

                        let Some(e2fs) = ext2::Ext2FS::new(file_node, false) else {
                            writeln!(TERMINAL.lock(), "Source file does not contain a valid ext2 fs!").unwrap();
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
                            actual_node.append_str(mntpoint);
                            mntpoint_node = Ok(actual_node);
                        }

                        let Ok(mntpoint_node) = mntpoint_node else {
                            writeln!(TERMINAL.lock(), "Malformed mountpoint path!").unwrap();
                            continue;
                        };

                        let Some(mntpoint_node)= mntpoint_node.get_rootfs_node() else {
                            writeln!(TERMINAL.lock(), "Mountpoint should exist in vfs!").unwrap();
                            continue;
                        };
                        (*mntpoint_node).borrow_mut().mountpoint = Some(root_inode);
                    } else {
                        writeln!(TERMINAL.lock(), "Not enough arguments!").unwrap();
                    }
                } else if cmnd.starts_with("umount") {
                    if let Some(mntpoint) = splat.next() {
                        let mut mntpoint_node = vfs::Path::try_from(mntpoint.trim());
                        if !mntpoint.starts_with("/") {
                            let mut actual_node = cur_dir.clone();
                            actual_node.append_str(mntpoint);
                            mntpoint_node = Ok(actual_node);
                        }

                        let Ok(mntpoint_node) = mntpoint_node else {
                            writeln!(TERMINAL.lock(), "Malformed mountpoint path!").unwrap();
                            continue;
                        };

                        let Some(mntpoint_node) = mntpoint_node.get_rootfs_node() else {
                            writeln!(TERMINAL.lock(), "Mountpoint should exist in vfs!").unwrap();
                            continue;
                        };

                        (*mntpoint_node).borrow_mut().mountpoint = None;
                    } else {
                        writeln!(TERMINAL.lock(), "Not enough arguments!").unwrap();
                    }
                } else if cmnd.starts_with("ls") {
                    for subnode in (*cur_dir.get_node().expect("Shell path should be valid at all times!").expect_folder())
                        .borrow()
                        .get_children()
                    {
                        write!(TERMINAL.lock(), "{} ", subnode.0).unwrap();
                        if let Node::File(f) = subnode.1 {
                            write!(TERMINAL.lock(), "(size: {} kb) ", (*f).borrow().get_size() as f32 / 1024.0).unwrap();
                        }
                    }
                    writeln!(TERMINAL.lock()).unwrap();
                } else if cmnd.starts_with("hexdump") {
                    if let (Some(offset_str), Some(file_str)) = (splat.next(), splat.next()) {
                        if let Ok(offset) = offset_str.trim().parse::<usize>() {
                            let arg_path = if file_str.starts_with('/') {
                                vfs::Path::try_from(file_str)
                            } else {
                                let mut actual_dir = cur_dir.clone();
                                actual_dir.append_str(file_str);
                                Ok(actual_dir)
                            };

                            let node = arg_path.map(|path| path.get_node());
                            let Ok(node)= node else {
                                writeln!(TERMINAL.lock(), "Invalid path!").unwrap();
                                continue;
                            };
                            let Some(node) = node else {
                                writeln!(TERMINAL.lock(), "Path doesn't exist!").unwrap();
                                continue;
                            };

                            if let Node::File(file) = node {
                                if let Some(data) =
                                    (*file).borrow().read(offset as u64, min(16, (*file).borrow().get_size() as usize))
                                {
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
                } else if cmnd.starts_with("touch") {
                    while let Some(name) = splat.next() {
                        let arg_path = if name.starts_with('/') {
                            vfs::Path::try_from(name)
                        } else {
                            let mut actual_dir = cur_dir.clone();
                            actual_dir.append_str(name);
                            Ok(actual_dir)
                        };
                        let Ok(mut arg_path) = arg_path else {
                            writeln!(TERMINAL.lock(), "Bad path!").unwrap();
                            continue;
                        };
                        let name = arg_path.last().to_owned();
                        arg_path.del_last();

                        let Some(node) = arg_path.get_node() else {
                            writeln!(TERMINAL.lock(), "Non-existant path!").unwrap();
                            continue;
                        };
                        if let Node::Folder(folder) = node {
                            if folder.borrow_mut().create_empty_child(&name, vfs::NodeType::File).is_none() {
                                writeln!(TERMINAL.lock(), "Failed to touch file!").unwrap();
                            }
                        }
                    }
                } else if cmnd.starts_with("cd") {
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
                            cur_dir.append_str(name);
                        }
                        if cur_dir.get_node().is_none() {
                            writeln!(TERMINAL.lock(), "Invalid cd path: {}!", cur_dir).unwrap();
                            cur_dir = old_dir;
                        }
                    }
                } else if cmnd.starts_with("mkrootfsdir") {
                    while let Some(name) = splat.next() {
                        RootFSNode::new_folder(
                            cur_dir.get_rootfs_node().expect("Shell path should be valid at all times!"),
                            name,
                        );
                    }
                } else if cmnd.starts_with("rmrootfsdir") {
                    while let Some(name) = splat.next() {
                        let cur_node = cur_dir.get_rootfs_node().expect("Shell path should be valid at all times!");
                        // Empty folder check
                        if let Some(child_to_sacrifice) = RootFSNode::find_folder(cur_node.clone(), name) {
                            if (*child_to_sacrifice).borrow().get_children().len() != 0 {
                                writeln!(TERMINAL.lock(), "Folder: \"{}\", is non-empty!", name).unwrap();
                                break;
                            }
                        } else {
                            writeln!(TERMINAL.lock(), "Folder: \"{}\", does not exist!", name).unwrap();
                            continue;
                        }
                        ////

                        if !RootFSNode::del_folder(cur_node, name) {
                            writeln!(TERMINAL.lock(), "Couldn't delete folder: \"{}\"!", name).unwrap();
                        }
                    }
                } else if cmnd.starts_with("rm") {
                    while let Some(name) = splat.next() {
                        let arg_path = if name.starts_with('/') {
                            vfs::Path::try_from(name)
                        } else {
                            let mut actual_dir = cur_dir.clone();
                            actual_dir.append_str(name);
                            Ok(actual_dir)
                        };
                        let Ok(mut arg_path) = arg_path else {
                            writeln!(TERMINAL.lock(), "Bad path!").unwrap();
                            continue;
                        };
                        let file_name = arg_path.last().to_owned();
                        arg_path.del_last();

                        let Some(node) = arg_path.get_node() else {
                            writeln!(TERMINAL.lock(), "Non-existant path!").unwrap();
                            continue;
                        };
                        
                        if let Node::Folder(folder) = node {
                            let Some((_, child)) = folder.borrow_mut().get_children().into_iter().find(|child| child.0 == file_name) else {
                                writeln!(TERMINAL.lock(), "File doesn't exist in folder!").unwrap();
                                continue;
                            };
                            let Node::File(child) = child else {
                                writeln!(TERMINAL.lock(), "Not a file!").unwrap();
                                continue;
                            };

                            writeln!(TERMINAL.lock(), "Removing the data from \"{}\"!", name).unwrap();
                            if child.borrow_mut().resize(0).is_none() {
                                writeln!(TERMINAL.lock(), "Failed to remove the data!").unwrap();
                            } else {
                                writeln!(TERMINAL.lock(), "Deleting/unlinking file!").unwrap();
                                if folder.borrow_mut().unlink_or_delete_empty_child(&name).is_none() {
                                    writeln!(TERMINAL.lock(), "Failed to delete/unlink file!").unwrap();
                                }
                            }
                        }
                    }
                } else if cmnd.starts_with("elp") {
                    writeln!(TERMINAL.lock(), "NOPERS, no elp!").unwrap();
                } else if cmnd.starts_with("exit") {
                    break 'big_loop;
                } else {
                    let executable_path = if cmnd.starts_with('/') {
                        vfs::Path::try_from(cmnd)
                    } else if cmnd.starts_with('.') {
                        let mut actual_dir = cur_dir.clone();
                        actual_dir.append_str(cmnd);
                        Ok(actual_dir)
                    } else {
                        Err(())
                    };

                    let Ok(executable_path) = executable_path else {
                        writeln!(TERMINAL.lock(), "Unrecognised command!").unwrap();
                        continue;
                    };

                    let Some(node) = executable_path.get_node() else {
                        writeln!(TERMINAL.lock(), "Invalid executable path!").unwrap();
                        continue;
                    };
                    
                    if let Node::File(executable) = node {
                        writeln!(TERMINAL.lock(), "Loading program, please wait ...").unwrap();
                        let Some(contents) = executable.borrow().read(0, executable.borrow().get_size() as usize) else {
                            writeln!(TERMINAL.lock(), "Failed to read executable!").unwrap();
                            continue;
                        };

                        writeln!(TERMINAL.lock(), "Parsing program, please wait ...").unwrap();
                        {
                            let Some(elf) = elf::ElfFile::from_bytes(&contents) else {
                                writeln!(TERMINAL.lock(), "Executable is not an elf file!").unwrap();
                                continue;
                            };

                            writeln!(UART.lock(), "Program entry point: {}", elf.header.program_entry).unwrap();
                            writeln!(UART.lock(), "Number of parsed program headers in elf: {}", elf.program_headers.len())
                                .unwrap();
                        }

                        let mut program_env = BTreeMap::new();
                        program_env.insert("HOME", "/");

                        let program =
                            if let Some(p) = Process::from_elf(&contents, &bufs.split(' ').collect::<Vec<&str>>(), cur_dir.clone(), &program_env) {
                                p
                            } else {
                                writeln!(TERMINAL.lock(), "Failed to load elf file into program!").unwrap();
                                continue;
                            };
                        scheduler::new_task(program);

                        writeln!(TERMINAL.lock(), "Program loaded!").unwrap();
                    } else {
                        writeln!(TERMINAL.lock(), "Executable path is not a file!").unwrap();
                    }
                }
            }

            // Wait until all processes finish executing
            while scheduler::tick() {}

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

    writeln!(UART.lock(), "Kernel heap usage: {} bytes", allocator::ALLOCATOR.lock().get_heap_used()).unwrap();
    writeln!(UART.lock(), "Program heap usage: {} bytes", allocator::PROGRAM_ALLOCATOR.0.lock().get_heap_used()).unwrap();

    // Shutdown
    writeln!(UART.lock(), "\nIt's now safe to turn off your computer!").unwrap();

    let width = TERMINAL.lock().fb.get_width();
    let height = TERMINAL.lock().fb.get_height();
    let cols = TERMINAL.lock().fb.get_cols();
    let rows = TERMINAL.lock().fb.get_rows();
    TERMINAL.lock().fb.fill(0, 0, width, height, Pixel { r: 0, g: 0, b: 0 });
    let s = "It's now safe to turn off your computer!";
    s.chars().enumerate().for_each(|(ind, c)| {
        TERMINAL.lock().fb.write_char(ind + cols / 2 - s.len() / 2, (rows - 1) / 2, c, Pixel { r: 0xff, g: 0xff, b: 0x55 });
    });

    loop {}
}
