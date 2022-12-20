use core::{
    convert::{TryFrom, TryInto},
    ptr::null_mut,
    slice,
};

use rlibc::sys::SyscallNumber;

use crate::{
    emulator::Riscv64Cpu,
    hio::KeyboardPacketType,
    program::{ProgramData, ProgramFileDescriptor},
    ps2_8042::KEYBOARD_INPUT,
    vfs,
    virtmem::{self, LittleEndianVirtualMemory, UserPointer, VirtualMemory},
    UART,
};

type Emulator = Riscv64Cpu<LittleEndianVirtualMemory>;

/* TODO: Add errno to program
mod errno {
    pub const EIDK_FIGURE_IT_OUT_YOURSELF: isize = -1;
    pub const EACCESS: isize = -2;
    pub const EBADFD: isize = -3;
    pub const EOUTSIDE_ACCESSIBLE_ADDRESS_SPACE: isize = -4;
    pub const EINVAL: isize = -5;
    pub const EISDIR: isize = -6;
}*/

pub fn syscall_linux_abi_entry_point(emu: &mut Emulator, prog_data: &mut ProgramData) {
    // Source: man syscall
    let syscall_number = emu.read_reg(17 /* a7 */);
    let syscall_number = if let Ok(val) = SyscallNumber::try_from(syscall_number as usize) {
        val
    } else {
        use core::fmt::Write;
        let _ = writeln!(UART.lock(), "Syscall number {}, is not implemented, ignoring!", syscall_number);
        return;
    };

    // a0-a5
    let argument_1 = || emu.read_reg(10);
    let argument_2 = || emu.read_reg(11);
    let argument_3 = || emu.read_reg(12);
    let argument_4 = || emu.read_reg(13);
    let argument_5 = || emu.read_reg(14);
    let argument_6 = || emu.read_reg(15);

    let return_value = |val, emu: &mut Emulator| emu.write_reg(10, val);

    match syscall_number {
        SyscallNumber::Exit => exit(emu, argument_1() as usize),

        SyscallNumber::Read => {
            let val = read(
                emu,
                prog_data,
                argument_1() as usize,
                unsafe { UserPointer::<[u8]>::from_mem(argument_2()) },
                argument_3() as usize,
            );
            return_value(val as i64 as u64, emu)
        }

        // SAFETY: Argument 2 comes from the program itself so it is in its address space
        SyscallNumber::Write => {
            let val = write(
                emu,
                prog_data,
                argument_1() as usize,
                unsafe { virtmem::UserPointer::<[u8]>::from_mem(argument_2()) },
                argument_3() as usize,
            );
            return_value(val as i64 as u64, emu)
        }

        // SAFETY: Argument 1 comes from the program itself so it is in its address space
        SyscallNumber::Open => {
            let val =
                open(emu, prog_data, unsafe { virtmem::UserPointer::<[u8]>::from_mem(argument_1()) }, argument_2() as usize);
            return_value(val as u64, emu)
        }

        SyscallNumber::Close => {
            let val = close(prog_data, argument_1() as usize);
            return_value(val as u64, emu)
        }

        SyscallNumber::Malloc => {
            let val = malloc(emu, prog_data, argument_1() as usize);
            return_value(val as u64, emu)
        }

        SyscallNumber::Free => free(emu, prog_data, argument_1()),

        SyscallNumber::LSeek => {
            let val = lseek(prog_data, argument_1() as usize, argument_2() as i64, argument_3() as usize);
            return_value(val as u64, emu)
        }

        SyscallNumber::Realloc => {
            let val = realloc(emu, prog_data, argument_1(), argument_2() as usize);
            return_value(val as u64, emu)
        }

        SyscallNumber::MaxValue => (),
    }
}

fn exit(emu: &mut Emulator, exit_number: usize) {
    use core::fmt::Write;
    writeln!(UART.lock(), "exit({}) called!", exit_number).unwrap();
    emu.halted = true;
}

fn write(emu: &mut Emulator, prog_data: &mut ProgramData, fd: usize, user_buf: UserPointer<[u8]>, count: usize) -> i32 {
    let buf = if let Some(val) = user_buf.try_as_ref(&mut emu.memory, count) { val } else { return -1 };

    match fd {
        rlibc::sys::STDIN_FILENO => return -1,

        rlibc::sys::STDOUT_FILENO | rlibc::sys::STDERR_FILENO => {
            use crate::TERMINAL;
            use core::fmt::Write;
            let str_buf = if let Ok(val) = core::str::from_utf8(buf) {
                val
            } else {
                return -1;
            };

            let res = write!(TERMINAL.lock(), "{}", str_buf);
            if res.is_err() {
                return -1;
            }
            return count as i32;
        }

        fd => {
            let fd = fd - 3; /* 0, 1, and 2 fds a special */
            if let Some(Some(node_fd)) = prog_data.open_fds.get_mut(fd) {
                // Check for write access
                if node_fd.flags & rlibc::sys::O_WRONLY == 0 {
                    return -1;
                }

                if let vfs::Node::File(f) = node_fd.vfs_node.clone() {
                    if node_fd.flags & rlibc::sys::O_APPEND != 0 {
                        // Before each write, the file offset is positioned at the end of the file
                        // Source: man open
                        node_fd.cursor = (*f).borrow().get_size();
                    }

                    // Make sure file is big enough
                    if node_fd.cursor + buf.len() as u64 > (*f).borrow().get_size() {
                        if (*f).borrow_mut().resize(node_fd.cursor + buf.len() as u64).is_none() {
                            return -1;
                        }
                    }

                    let inc = if let Some(amnt) = (*f).borrow_mut().write(node_fd.cursor, buf) {
                        amnt
                    } else {
                        return -1;
                    };
                    node_fd.cursor += inc as u64;
                    return inc as i32;
                } else {
                    return -1;
                }
            } else {
                return -1;
            }
        }
    }
}

fn read(emu: &mut Emulator, prog_data: &mut ProgramData, fd: usize, user_buf: UserPointer<[u8]>, count: usize) -> i32 {
    let buf = if let Some(val) = user_buf.try_as_ref(&mut emu.memory, count) { val } else { return -1 };

    match fd {
        rlibc::sys::STDOUT_FILENO | rlibc::sys::STDERR_FILENO => return -1,
        rlibc::sys::STDIN_FILENO => {
            // FIXME: Allow character echoing in the console
            loop {
                // We only allow applications to read one character from stdin at a time to stop the keyboard from being hogged by applications
                let packet = unsafe { KEYBOARD_INPUT.lock().read_packet() };
                if packet.typ == KeyboardPacketType::KeyReleased {
                    continue;
                }
                // FIXME: This shouldn't be here
                if packet.special_keys.any_ctrl() && packet.char_codepoint == Some('c') {
                    use crate::TERMINAL;
                    use core::fmt::Write;
                    write!(TERMINAL.lock(), "^C").unwrap();
                    emu.halted = true;
                }
                let c = if packet.special_keys.any_shift() { packet.shift_codepoint() } else { packet.char_codepoint };
                let c = if let Some(val) = c { val } else { continue };
                buf[0] = if let Ok(val) = c.try_into() { val } else { continue };
                return 1;
            }
        }

        fd => {
            let fd = fd - 3;
            if let Some(Some(node_fd)) = prog_data.open_fds.get_mut(fd) {
                // Check for read access
                if node_fd.flags & rlibc::sys::O_RDONLY == 0 {
                    return -1;
                }

                if let vfs::Node::File(f) = node_fd.vfs_node.clone() {
                    let mut index: u64 = 0;
                    while index < buf.len() as u64 {
                        // Make sure we don't pass file boundaries
                        if node_fd.cursor + index > (*f).borrow().get_size() {
                            break;
                        }

                        // TODO: Try to read more than 1 byte at a time
                        buf[index as usize] = if let Some(val) = (*f).borrow().read(node_fd.cursor + index, 1) {
                            val[0]
                        } else {
                            break;
                        };
                        index += 1;
                    }

                    node_fd.cursor += index;
                    return index as i32;
                } else {
                    return -1;
                }
            } else {
                return -1;
            }
        }
    }
}

fn open(emu: &mut Emulator, prog_data: &mut ProgramData, pathname: virtmem::UserPointer<[u8]>, flags: usize) -> isize {
    let buf = {
        let buf = if let Some(val) = pathname.try_as_ptr(&mut emu.memory) { val } else { return -1 };
        let buf_len = unsafe { rlibc::cstr::strlen(buf as *const core::ffi::c_char) };
        unsafe { slice::from_raw_parts(buf, buf_len as usize) }
    };

    let path = if let Ok(val) = core::str::from_utf8(buf) { val } else { return -1 };
    let path = if let Ok(val) = vfs::Path::try_from(path) { val } else { return -1 };

    // Get directory containing file
    let parent_node = if let Some(val) = path.clone().del_last().get_node() { val } else { return -1 };
    let parent_node = if let vfs::Node::Folder(val) = parent_node { val } else { return -1 };

    let node = {
        let search_result = (*parent_node).borrow_mut().get_children().into_iter().find(|child| child.0 == path.last());
        if let Some(val) = search_result {
            let mut node = val.1;

            // man open
            // If the file already exists and is a regular file and the
            //   access mode allows writing (i.e., is O_RDWR or O_WRONLY)
            //   it will be truncated to length 0.
            if (flags & rlibc::sys::O_TRUNC != 0) && (flags & rlibc::sys::O_WRONLY != 0) {
                if let vfs::Node::File(f) = &mut node {
                    if f.borrow_mut().resize(0).is_none() {
                        return -1;
                    }
                }
            }

            node
        } else {
            if flags & rlibc::sys::O_CREAT != 0 {
                // FIXME: Deal with permissions
                // Create file
                if let Some(val) = (*parent_node).borrow_mut().create_empty_child(path.last(), vfs::NodeType::File) {
                    val
                } else {
                    return -1;
                }
            } else {
                return -1;
            }
        }
    };

    prog_data.open_fds.push(Some(ProgramFileDescriptor { vfs_node: node, cursor: 0, flags }));
    (prog_data.open_fds.len() as isize - 1) + 3 /* 0, 1, and 2 fds a special */
}

// source: man close
fn close(prog_data: &mut ProgramData, fd: usize) -> isize {
    let fd = fd - 3; /* 0, 1, and 2 fds a special */
    if let Some(node) = prog_data.open_fds.get_mut(fd) {
        *node = None;
        // Drain None's if possible
        // Note: We cannot use retain or drain_filter because we need to keep the index of fds that are Some, which removing all None using retain or drain_filter will not do, for ex. on the vector: Some None Some
        while prog_data.open_fds.last().map(|elem| elem.is_none()).unwrap_or(false) {
            prog_data.open_fds.pop();
        }
        return 0;
    } else {
        return -1;
    }
}

fn lseek(prog_data: &mut ProgramData, fd: usize, offset: i64, whence: usize) -> i64 {
    match fd {
        rlibc::sys::STDIN_FILENO | rlibc::sys::STDOUT_FILENO | rlibc::sys::STDERR_FILENO => return -1,

        _ => {
            let fd = fd - 3;
            if let Some(Some(node)) = prog_data.open_fds.get_mut(fd) {
                match whence {
                    rlibc::sys::SEEK_SET => {
                        node.cursor = offset as u64;
                        return node.cursor as i64;
                    }

                    rlibc::sys::SEEK_CUR => {
                        node.cursor = ((node.cursor as i64) + offset) as u64;
                        return node.cursor as i64;
                    }

                    rlibc::sys::SEEK_END => {
                        if let vfs::Node::File(f) = &node.vfs_node {
                            node.cursor = ((f.borrow().get_size() as i64) + offset) as u64;
                            return node.cursor as i64;
                        } else {
                            return -1;
                        }
                    }

                    _ => return -1,
                }
            } else {
                return -1;
            }
        }
    }
}

fn malloc(emu: &mut Emulator, prog_data: &mut ProgramData, size: usize) -> u64 {
    let userspace_null_ptr: u64 = null_mut::<u8>() as u64;

    // We also allocate size_of::<usize>() bytes more than we are requested to, to store the size of the allocation
    let allocation_info = core::alloc::Layout::from_size_align(size + core::mem::size_of::<usize>(), 8);
    let allocation_info = if let Ok(val) = allocation_info {
        val
    } else {
        return userspace_null_ptr;
    };

    let heap = emu.memory.try_map_mut(prog_data.heap_virtual_start_addr);
    let heap = if let Some(val) = heap {
        val
    } else {
        return userspace_null_ptr;
    };
    let heap = heap.0;
    let mut heap_increment = 2;

    loop {
        // Make sure allocator is in sync with the actual location of the heap
        // This is because the actual location may change as the mapping changes/grows
        unsafe { prog_data.program_alloc.update_base(heap.backing_storage.as_mut_ptr()) };

        // Try to allocate
        let res = prog_data.program_alloc.alloc(allocation_info);
        if res != null_mut() {
            // Yay, we did it! :)

            unsafe {
                *(res as *mut usize) = size;
            }
            let res = unsafe { res.add(core::mem::size_of::<usize>()) };

            // But we still need to "reverse map" the returned pointer to user space
            let offset_in_heap = unsafe { (res as *mut u8).sub(heap.backing_storage.as_mut_ptr() as usize) };

            return heap.try_reverse_map(offset_in_heap as usize).unwrap_or(userspace_null_ptr);
        } else {
            // Well maybe we just need to grow the heap

            // Make sure we don't surpass virtual size limits
            if (heap.len() + heap_increment) as u64 >= prog_data.max_virtual_heap_size {
                return userspace_null_ptr;
            }

            // The best approach would be to increase by one byte each time, which would never over-allocate, but would take a lot of computation
            // A faster approach is to double the amount that we increase the heap by each time, starting at 2 bytes, this could over-allocate but is not really likely to
            heap.backing_storage.resize(heap.len() + heap_increment, 0);
            // Let allocator know that it's buffer just increased in size
            prog_data.program_alloc.grow_heap_space(heap_increment);
            heap_increment *= 2;
        }
    }
}

fn free(emu: &mut Emulator, prog_data: &mut ProgramData, virtual_ptr: u64) {
    // Translate pointer from user space
    let mapped_alloc = emu.memory.try_map_mut(virtual_ptr);
    let mapped_alloc = if let Some(val) = mapped_alloc {
        val
    } else {
        return;
    };

    let alloc_ptr =
        unsafe { mapped_alloc.0.backing_storage.as_mut_ptr().add(mapped_alloc.1).sub(core::mem::size_of::<usize>()) };

    // Make sure allocator is in sync with the actual location of the heap
    // This is because the actual location may change as the mapping changes/grows
    let heap = emu.memory.try_map_mut(prog_data.heap_virtual_start_addr);
    let heap = if let Some(val) = heap {
        val
    } else {
        return;
    };
    let heap = heap.0;
    unsafe { prog_data.program_alloc.update_base(heap.backing_storage.as_mut_ptr()) };

    // Get allocation info
    let alloc_size = unsafe { *(alloc_ptr as *mut usize) }; // Get size from before the allocation, since allocations made by malloc contain the size before the allocation

    let allocation_info = core::alloc::Layout::from_size_align(alloc_size + core::mem::size_of::<usize>(), 8);
    let allocation_info = if let Ok(val) = allocation_info {
        val
    } else {
        return;
    };

    // Deallocate
    prog_data.program_alloc.dealloc(alloc_ptr, allocation_info);
}


fn realloc(emu: &mut Emulator, prog_data: &mut ProgramData, virtual_ptr: u64, new_size: usize) -> u64 {
    let userspace_null_ptr: u64 = null_mut::<u8>() as u64;

    // Translate pointer from user space
    let mapped_alloc = emu.memory.try_map_mut(virtual_ptr);
    let mapped_alloc = if let Some(val) = mapped_alloc {
        val
    } else {
        return userspace_null_ptr;
    };

    // Get actual physical ptr
    let alloc_ptr =
    unsafe { mapped_alloc.0.backing_storage.as_mut_ptr().add(mapped_alloc.1).sub(core::mem::size_of::<usize>()) };

    // Make sure allocator is in sync with the actual location of the heap
    // This is because the actual location may change as the mapping changes/grows
    let heap = emu.memory.try_map_mut(prog_data.heap_virtual_start_addr);
    let heap = if let Some(val) = heap {
        val
    } else {
        return userspace_null_ptr;
    };
    let heap = heap.0;
    unsafe { prog_data.program_alloc.update_base(heap.backing_storage.as_mut_ptr()) };

    // Get allocation info
    let alloc_size = unsafe { *(alloc_ptr as *mut usize) }; // Get size from before the allocation, since allocations made by malloc contain the size before the allocation

    let allocation_info = core::alloc::Layout::from_size_align(alloc_size + core::mem::size_of::<usize>(), 8);
    let allocation_info = if let Ok(val) = allocation_info {
        val
    } else {
        return userspace_null_ptr;
    };

    // Reallocate
    prog_data.program_alloc.realloc(alloc_ptr, allocation_info, new_size) as u64
}