use core::{
    convert::{TryFrom, TryInto},
    ptr::null_mut,
    slice,
    str::from_utf8_unchecked
};

use alloc::vec::Vec;
use rlibc::cstr::strlen;
use rlibc::sys::SyscallNumber;

use crate::{
    emulator::Riscv64Cpu,
    hio::KeyboardPacketType,
    program::{FdMapping, ProgramData, ProgramNode},
    ps2_8042::KEYBOARD_INPUT,
    vfs,
    virtmem::{self, LittleEndianVirtualMemory, UserPointer, VirtualMemory},
    UART, allocator::{ProgramBasicAlloc, self},
};

type Emulator = Riscv64Cpu<LittleEndianVirtualMemory<&'static ProgramBasicAlloc>>;

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

        SyscallNumber::Getcwd => {
            let val =
                getcwd(emu, prog_data, unsafe { virtmem::UserPointer::<[u8]>::from_mem(argument_1()) }, argument_2() as usize);
            return_value(val as u64, emu)
        }

        SyscallNumber::Getenv => {
            let val = getenv(emu, prog_data, unsafe { virtmem::UserPointer::<[u8]>::from_mem(argument_1()) });
            return_value(val as u64, emu)
        }

        SyscallNumber::Fchdir => {
            let val = fchdir(prog_data, argument_1() as usize);
            return_value(val as u64, emu)
        }

        SyscallNumber::Dup => {
            let val = dup(prog_data, argument_1() as usize);
            return_value(val as u64, emu)
        }

        SyscallNumber::Dup2 => {
            let val = dup2(prog_data, argument_1() as usize, argument_2() as usize);
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
    let node_mapping = if let Some(Some(val)) = prog_data.fd_mapping.get(fd).copied() { val } else { return -1 };

    match node_mapping {
        FdMapping::Index(node_index) => {
            // If the node_index exists then so does the node
            let node = prog_data.open_nodes[node_index].as_mut().unwrap();

            // Check for write access
            if node.flags & rlibc::sys::O_WRONLY == 0 {
                return -1;
            }

            if let vfs::Node::File(f) = node.vfs_node.clone() {
                if node.flags & rlibc::sys::O_APPEND != 0 {
                    // Before each write, the file offset is positioned at the end of the file
                    // Source: man open
                    node.cursor = (*f).borrow().get_size();
                }

                // Make sure file is big enough
                if node.cursor + buf.len() as u64 > (*f).borrow().get_size() {
                    if (*f).borrow_mut().resize(node.cursor + buf.len() as u64).is_none() {
                        return -1;
                    }
                }

                let inc = if let Some(amnt) = (*f).borrow_mut().write(node.cursor, buf) {
                    amnt
                } else {
                    return -1;
                };

                node.cursor += inc as u64;

                return inc as i32;
            } else {
                // Can't write to directory
                return -1;
            }
        }

        FdMapping::Stdin => return -1, // Can't write to stdin

        FdMapping::Stdout | crate::program::FdMapping::Stderr => {
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
    }
}

fn read(emu: &mut Emulator, prog_data: &mut ProgramData, fd: usize, user_buf: UserPointer<[u8]>, count: usize) -> i32 {
    let buf = if let Some(val) = user_buf.try_as_ref(&mut emu.memory, count) { val } else { return -1 };
    let node_mapping = if let Some(Some(val)) = prog_data.fd_mapping.get(fd).copied() { val } else { return -1 };

    match node_mapping {
        FdMapping::Index(node_index) => {
            // If the node_index exists then so does the node
            let node = prog_data.open_nodes[node_index].as_mut().unwrap();

            // Check for read access
            if node.flags & rlibc::sys::O_RDONLY == 0 {
                return -1;
            }

            if let vfs::Node::File(f) = node.vfs_node.clone() {
                let mut index: u64 = 0;
                while index < buf.len() as u64 {
                    // Make sure we don't pass file boundaries
                    if node.cursor + index > (*f).borrow().get_size() {
                        break;
                    }

                    // TODO: Try to read more than 1 byte at a time
                    buf[index as usize] = if let Some(val) = (*f).borrow().read(node.cursor + index, 1) {
                        val[0]
                    } else {
                        break;
                    };
                    index += 1;
                }

                node.cursor += index;
                return index as i32;
            } else {
                return -1; // Can't read directory
            }
        }

        FdMapping::Stdin => {
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

        FdMapping::Stdout | FdMapping::Stderr => return -1, // Can't read stdout or stderr
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

    // TODO: Consider changing it so it actually uses the lowest fd number, instead of using a fd number that is known to be free
    prog_data.open_nodes.push(Some(ProgramNode { vfs_node: node, cursor: 0, flags, path, reference_count: 1 }));
    let node_index = prog_data.open_nodes.len() - 1;
    prog_data.fd_mapping.push(Some(FdMapping::Index(node_index)));

    // -1 because we just added a new one
    prog_data.fd_mapping.len() as isize - 1
}

// source: man close
fn close(prog_data: &mut ProgramData, fd: usize) -> isize {
    let node_mapping = if let Some(Some(val)) = prog_data.fd_mapping.get(fd).copied() { val } else { return -1 };
    match node_mapping {
        FdMapping::Index(node_index) => {
            // NOTE: If the node_index exists then so must the node so it's fine to unwrap
            let node = prog_data.open_nodes[node_index].as_mut().unwrap();
            if node.reference_count > 1 {
                node.reference_count -= 1;
            } else {
                prog_data.open_nodes[node_index] = None;
            }
        }
        FdMapping::Stdin | FdMapping::Stdout | FdMapping::Stderr => {} // No need to close stdin, stdout or stderr since they are not actual nodes
    }

    prog_data.fd_mapping[fd] = None;

    // Drain None's if possible
    // Note: We cannot use retain or drain_filter because we need to keep the index of fds that are Some, which removing all None using retain or drain_filter will not do, for ex. on the vector: Some None Some
    while prog_data.fd_mapping.last().map(|elem| elem.is_none()).unwrap_or(false) {
        prog_data.fd_mapping.pop();
    }

    while prog_data.open_nodes.last().map(|elem| elem.is_none()).unwrap_or(false) {
        prog_data.open_nodes.pop();
    }

    return 0;
}

fn lseek(prog_data: &mut ProgramData, fd: usize, offset: i64, whence: usize) -> i64 {
    let node_mapping = if let Some(Some(val)) = prog_data.fd_mapping.get(fd).copied() { val } else { return -1 };

    match node_mapping {
        FdMapping::Index(node_index) => {
            let node = prog_data.open_nodes[node_index].as_mut().unwrap();
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
        }

        FdMapping::Stdin | FdMapping::Stdout | FdMapping::Stderr => return -1,
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

    // Try to allocate physical space
    let mut physical_allocation = Vec::new_in(&allocator::PROGRAM_ALLOCATOR);
    physical_allocation.clear();
    physical_allocation.resize(allocation_info.size(), 0u8);

    // Add size of allocation to beginning for usage by the free syscall
    for (index, byte) in allocation_info.size().to_le_bytes().iter().enumerate() {
        physical_allocation[index] = *byte;
    }

    // Allocate virtual space
    let virtual_allocation_ptr = prog_data.virtual_allocator.alloc(allocation_info) as u64;
    if virtual_allocation_ptr == userspace_null_ptr { return userspace_null_ptr; }

    // Create mapping
    emu.memory.add_region(virtual_allocation_ptr, physical_allocation);

    return virtual_allocation_ptr+core::mem::size_of::<usize>() as u64;
}

fn free(emu: &mut Emulator, prog_data: &mut ProgramData, virtual_ptr: u64) {

    // Translate pointer from user space
    let mapped_alloc = emu.memory.try_map_mut(virtual_ptr);
    let mapped_alloc = if let Some(val) = mapped_alloc {
        val
    } else {
        return;
    };

    let size = usize::from_le_bytes(mapped_alloc.0.backing_storage[0..core::mem::size_of::<usize>()].try_into().unwrap());
    let allocation_info = core::alloc::Layout::from_size_align(size, 8);
    let allocation_info = if let Ok(val) = allocation_info {
        val
    }else{
        return;
    };

    let alloc_region_index = mapped_alloc.1.region_index;
    emu.memory.remove_region(alloc_region_index);

    prog_data.virtual_allocator.dealloc((virtual_ptr-core::mem::size_of::<usize>() as u64) as *mut u8, allocation_info);
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

    let data: Vec<u8, &'static ProgramBasicAlloc> = mapped_alloc.0.backing_storage.clone();
    free(emu, prog_data, virtual_ptr);
    let new_virtual_ptr = malloc(emu, prog_data, new_size);
    // Translate pointer from user space
    let new_mapped_alloc = emu.memory.try_map_mut(new_virtual_ptr);
    let new_mapped_alloc = if let Some(val) = new_mapped_alloc {
        val
    } else {
        return userspace_null_ptr;
    };
    new_mapped_alloc.0.backing_storage = data;
    return new_virtual_ptr;
}

fn getcwd(emu: &mut Emulator, prog_data: &mut ProgramData, virtual_ptr: virtmem::UserPointer<[u8]>, buf_size: usize) -> u64 {
    // On failure, these functions return NULL
    // Source: man getcwd

    let userspace_null_ptr: u64 = null_mut::<u8>() as u64;

    // If the length of the absolute pathname of the current working
    // directory, including the terminating null byte, exceeds size
    // bytes, NULL is returned
    // Source: man getcwd
    if prog_data.cwd.len() + 1 > buf_size {
        return userspace_null_ptr;
    }

    let buf = if let Some(val) = virtual_ptr.try_as_ref(&mut emu.memory, buf_size) {
        val
    } else {
        return userspace_null_ptr;
    };

    for (i, c) in prog_data.cwd.chars().enumerate() {
        buf[i] = c as u8;
    }

    buf[buf.len() - 1] = b'\0';
    return virtual_ptr.get_inner();
}

fn fchdir(prog_data: &mut ProgramData, fd: usize) -> isize {
    prog_data.cwd = if let Some(Some(node_mapping)) = prog_data.fd_mapping.get(fd).copied() {
        match node_mapping {
            FdMapping::Index(node_index) => {
                let node = prog_data.open_nodes[node_index].as_mut().unwrap();
                node.path.clone()
            }
            FdMapping::Stdin | FdMapping::Stdout | FdMapping::Stderr => return -1,
        }
    } else {
        return -1;
    };

    return 0;
}

fn getenv(emu: &mut Emulator, prog_data: &mut ProgramData, virtual_ptr: virtmem::UserPointer<[u8]>) -> u64 {
    let userspace_null_ptr: u64 = null_mut::<u8>() as u64;

    // The getenv() function returns a pointer to the value in the
    // environment, or NULL if there is no match.
    // Source: man getenv
    let key_ptr = if let Some(val) = virtual_ptr.try_as_ptr(&mut emu.memory) {
        val
    } else {
        return userspace_null_ptr;
    };

    let key = unsafe {
        from_utf8_unchecked(
            if let Some(val) = virtual_ptr.try_as_ref(&mut emu.memory, strlen(key_ptr as *const core::ffi::c_char) as usize) {
                val
            } else {
                return userspace_null_ptr;
            },
        )
    };

    // Lookup the environment variable
    let res = prog_data.env.get(key);
    let res = if let Some(val) = res {
        val
    } else {
        return userspace_null_ptr;
    };

    return *res;
}

fn dup(prog_data: &mut ProgramData, oldfd: usize) -> isize {
    let node_mapping = if let Some(Some(val)) = prog_data.fd_mapping.get_mut(oldfd).copied() {
        val
    } else {
        return -1;
    };

    match node_mapping {
        FdMapping::Index(node_index) => {
            // NOTE: If the node_index exists then the node must exist
            let node = prog_data.open_nodes[node_index].as_mut().unwrap();
            node.reference_count += 1;
        }

        FdMapping::Stdin | FdMapping::Stdout | FdMapping::Stderr => {} // No need to update ref count of stdin, stdout or stderr since they are handled specially anyways
    }

    prog_data.fd_mapping.push(Some(node_mapping));
    // -1 because we just added the new one
    prog_data.fd_mapping.len() as isize - 1
}

fn dup2(prog_data: &mut ProgramData, oldfd: usize, newfd: usize) -> isize {
    use core::fmt::Write;
    writeln!(UART.lock(), "dup2({}, {}) called!", oldfd, newfd);

    let node_mapping = if let Some(Some(val)) = prog_data.fd_mapping.get_mut(oldfd).copied() {
        val
    } else {
        return -1;
    };

    close(prog_data, newfd);

    match node_mapping {
        FdMapping::Index(node_index) => {
            // NOTE: If the node_index exists then the node must exist
            let node = prog_data.open_nodes[node_index].as_mut().unwrap();
            node.reference_count += 1;
        }

        FdMapping::Stdin | FdMapping::Stdout | FdMapping::Stderr => {} // No need to update ref count of stdin, stdout or stderr since they are handled specially anyways
    }

    if newfd > prog_data.fd_mapping.len() {
        prog_data.fd_mapping.resize(newfd+1, None);
    }

    prog_data.fd_mapping[newfd] = Some(node_mapping);
    newfd as isize
}