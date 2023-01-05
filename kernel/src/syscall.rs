use core::{
    convert::{TryFrom, TryInto},
    ptr::null_mut
};

use alloc::{vec::Vec, string::String, borrow::ToOwned, collections::{BTreeMap, VecDeque}};
use rlibc::sys::O_RDONLY;
use rlibc::sys::SyscallNumber;

use crate::{
    hio::{KeyboardPacketType, standard_usa_qwerty},
    process::{FdMapping, ProcessData, ProcessNode, Emulator, Process, ProcessState, ProcessPipe},
    ps2_8042::KEYBOARD_INPUT,
    vfs::{self, Path},
    virtmem::{self, UserPointer, VirtualMemory},
    UART, allocator::{ProgramBasicAlloc, self, BasicAlloc}, scheduler, emulator::CpuAction, elf::{ElfFile, elf_header}, terminal::TERMINAL,
};

/* TODO: Add errno to program
mod errno {
    pub const EIDK_FIGURE_IT_OUT_YOURSELF: isize = -1;
    pub const EACCESS: isize = -2;
    pub const EBADFD: isize = -3;
    pub const EOUTSIDE_ACCESSIBLE_ADDRESS_SPACE: isize = -4;
    pub const EINVAL: isize = -5;
    pub const EISDIR: isize = -6;
}*/

pub fn syscall_entry_point(emu: &mut Emulator, proc_data: &mut ProcessData) -> CpuAction {
    // Source: man syscall
    let syscall_number = emu.read_reg(17 /* a7 */);
    let Ok(syscall_number) = SyscallNumber::try_from(syscall_number as usize) else {
        use core::fmt::Write;
        let _ = writeln!(UART.lock(), "Syscall number {}, is not implemented, ignoring!", syscall_number);
        return CpuAction::NONE;
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
        SyscallNumber::Exit => exit( proc_data, argument_1() as usize),

        SyscallNumber::Read => {
            if let Some(val) = read(
                emu,
                proc_data,
                argument_1() as usize,
                unsafe { UserPointer::<[u8]>::from_mem(argument_2()) },
                argument_3() as usize,
            ){
                return_value(val as i64 as u64, emu);
            }else{
                return CpuAction::REPEAT_INSTRUCTION;
            }
        }

        SyscallNumber::Write => {
            let val = write(
                emu,
                proc_data,
                argument_1() as usize,
                unsafe { virtmem::UserPointer::<[u8]>::from_mem(argument_2()) },
                argument_3() as usize,
            );
            return_value(val as i64 as u64, emu);
        }

        SyscallNumber::Open => {
            let val =
                open(emu, proc_data, unsafe { virtmem::UserPointer::<[u8]>::from_mem(argument_1()) }, argument_2() as usize);
            return_value(val as u64, emu);
        }

        SyscallNumber::Close => {
            let val = close(proc_data, argument_1() as usize);
            return_value(val as u64, emu);
        }

        SyscallNumber::Malloc => {
            let val = malloc(emu, proc_data, argument_1() as usize);
            return_value(val as u64, emu);
        }

        SyscallNumber::Free => free(emu, proc_data, argument_1()),

        SyscallNumber::LSeek => {
            let val = lseek(proc_data, argument_1() as usize, argument_2() as i64, argument_3() as usize);
            return_value(val as u64, emu);
        }

        SyscallNumber::Realloc => {
            let val = realloc(emu, proc_data, argument_1(), argument_2() as usize);
            return_value(val as u64, emu);
        }

        SyscallNumber::Getcwd => {
            let val =
                getcwd(emu, proc_data, unsafe { virtmem::UserPointer::<[u8]>::from_mem(argument_1()) }, argument_2() as usize);
            return_value(val as u64, emu);
        }

        SyscallNumber::Getenv => {
            let val = getenv(emu, proc_data, unsafe { virtmem::UserPointer::<[u8]>::from_mem(argument_1()) });
            return_value(val as u64, emu);
        }

        SyscallNumber::Fchdir => {
            let val = fchdir(proc_data, argument_1() as usize);
            return_value(val as u64, emu);
        }

        SyscallNumber::Dup => {
            let val = dup(proc_data, argument_1() as usize);
            return_value(val as u64, emu);
        }

        SyscallNumber::Dup2 => {
            let val = dup2(proc_data, argument_1() as usize, argument_2() as usize);
            return_value(val as u64, emu);
        }

        SyscallNumber::Fork => {
            let val = fork(emu, proc_data);
            return_value(val as u64, emu);
        }

        SyscallNumber::Waitpid => {
            let val = waitpid(emu, proc_data, argument_1() as isize, unsafe{virtmem::UserPointer::<usize>::from_mem(argument_2())}, argument_3() as usize );
            if let Some(val) = val{
                return_value(val as u64, emu);
            }else{
                // Waitpid will cause the scheduler to stop ticking the program until the change happens, however once the change does happen
                // the instruction that it *would* execute would be the one after the syscall
                // so the syscall will not be able to return the wait information
                // so instead we just make the process repeat the instruction
                // so that when the change happens it just calls syscall again, which will call waitpid again
                // and give waitpid a chance to actually return the data structure with the information of the wait call 
                // to make sure that once waitpid returns it doesn't get called again we 
                // make waitpid return an Option which is None only if it is to be called again
                return CpuAction::REPEAT_INSTRUCTION;
            }
        }

        SyscallNumber::Fexecve => {
            let res = fexecve(emu, proc_data, argument_1() as usize, unsafe{virtmem::UserPointer::<[u64]>::from_mem(argument_2())}, unsafe{virtmem::UserPointer::<[u64]>::from_mem(argument_3())});
            match res {
                Ok(_) => return CpuAction::REPEAT_INSTRUCTION, // Don't advance to the next instruction as we want to start executing the new program from the first instruction
                Err(val) => return_value(val as u64, emu)
            }
        }

        SyscallNumber::Execve => {
            let res = execve(emu, proc_data, unsafe{virtmem::UserPointer::<[u8]>::from_mem(argument_1())}, unsafe{virtmem::UserPointer::<[u64]>::from_mem(argument_2())}, unsafe{virtmem::UserPointer::<[u64]>::from_mem(argument_3())});
            match res {
                Ok(_) => return CpuAction::REPEAT_INSTRUCTION, // Don't advance to the next instruction as we want to start executing the new program from the first instruction
                Err(val) => return_value(val as u64, emu)
            }
        }

        SyscallNumber::Execvpe => {
            let res = execvpe(emu, proc_data, unsafe{virtmem::UserPointer::<[u8]>::from_mem(argument_1())}, unsafe{virtmem::UserPointer::<[u64]>::from_mem(argument_2())}, unsafe{virtmem::UserPointer::<[u64]>::from_mem(argument_3())});
            match res {
                Ok(_) => return CpuAction::REPEAT_INSTRUCTION, // Don't advance to the next instruction as we want to start executing the new program from the first instruction
                Err(val) => return_value(val as u64, emu)
            }
        }

        SyscallNumber::Pipe => {
            let res = pipe(emu, proc_data, unsafe{virtmem::UserPointer::<[core::ffi::c_int]>::from_mem(argument_1())});
            return_value(res as u64, emu)
        }

        SyscallNumber::MaxValue => (),
    }

    return CpuAction::NONE;
}

fn exit(proc_data: &mut ProcessData, exit_code: usize) {
    if proc_data.parent_pid == None {
        proc_data.state = ProcessState::TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED{exit_code};
    } else {
        proc_data.state = ProcessState::TERMINATED_NORMALLY_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT{exit_code};
    }
}

fn write(emu: &mut Emulator, proc_data: &mut ProcessData, fd: usize, user_buf: UserPointer<[u8]>, count: usize) -> i32 {
    let Some(buf) = user_buf.try_as_ref(&mut emu.memory, count) else { return -1 };
    let Some(Some(node_mapping)) = proc_data.fd_mappings.get(fd).cloned() else { return -1 };

    match node_mapping {
        FdMapping::Regular(node_index) => {
            // If the node_index exists then so does the node
            let node = proc_data.open_nodes[node_index].as_mut().unwrap();

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

                let Some(inc) = (*f).borrow_mut().write(node.cursor, buf) else {
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

        FdMapping::PipeReadEnd(_) => return -1, // Can't write to read end of pipe

        FdMapping::PipeWriteEnd(pipe_index) => {
            let mut pipes = scheduler::PIPES.lock();
            let pipe = pipes[pipe_index].as_mut().unwrap();
            if pipe.readers_count == 0 { return -1; }
            pipe.buf.extend(buf);
            return count as i32;
        }

        FdMapping::Stdout | crate::process::FdMapping::Stderr => {
            use crate::TERMINAL;
            use core::fmt::Write;
            let Ok(str_buf) = core::str::from_utf8(buf) else {
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


fn read(emu: &mut Emulator, proc_data: &mut ProcessData, fd: usize, user_buf: UserPointer<[u8]>, count: usize) -> Option<i32> {
    let Some(buf) = user_buf.try_as_mut(&mut emu.memory, count) else { return Some(-1) };
    let Some(Some(node_mapping)) = proc_data.fd_mappings.get(fd).cloned() else { return Some(-1) };

    match node_mapping {
        FdMapping::Regular(node_index) => {
            // If the node_index exists then so does the node
            let node = proc_data.open_nodes[node_index].as_mut().unwrap();

            // Check for read access
            if node.flags & rlibc::sys::O_RDONLY == 0 {
                return Some(-1);
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
                return Some(index as i32);
            } else {
                return Some(-1); // Can't read directory
            }
        }

        FdMapping::PipeWriteEnd(_) => return Some(-1), // Can't read from write end of pipe

        FdMapping::PipeReadEnd(pipe_index) => {
            let mut pipes = scheduler::PIPES.lock();
            let pipe = pipes[pipe_index].as_mut().unwrap();

            let mut index = 0;
            while index < count {
                let Some(byte) = pipe.buf.pop_front() else { break; };
                buf[index] = byte;
                index += 1;
            }
            
            if index == 0 { // No data left to read
               if pipe.writers_count == 0 {
                 // No more writers, so EOF
                 return Some(0); 
               } else {
                 // Wait for writer
                 proc_data.state = ProcessState::WAITING_FOR_READ_PIPE { pipe_index: pipe_index };
                 return None; // Will cause read to be repeated until something is read effectively emulating a block on read
               }
            }

            return Some(index as i32);
        }

        FdMapping::Stdin => {
            loop {
                // We only allow applications to read one character from stdin at a time to stop the keyboard from being hogged by applications
                // FIXME: Implement better drivers
                let packet = unsafe { KEYBOARD_INPUT.lock().read_packet() };
                if packet.packet_type == KeyboardPacketType::KeyReleased {
                    continue;
                }

                let Ok(c) = standard_usa_qwerty::parse_key(packet.key, packet.modifiers) else { continue; };
                buf[0] = if let Ok(val) = c.try_into() { val } else { continue };
                TERMINAL.lock().recive_key(packet.key, packet.modifiers);
                return Some(1);
            }
        }

        FdMapping::Stdout | FdMapping::Stderr => return Some(-1), // Can't read stdout or stderr
    }
}

fn open(emu: &mut Emulator, proc_data: &mut ProcessData, pathname: virtmem::UserPointer<[u8]>, flags: usize) -> isize {
    let Some(path) = virtmem::cstr_user_pointer_to_str(pathname, &emu.memory) else { return -1 };
    let path = if let Ok(val) = vfs::Path::try_from(path) { val } else { 
        // Maybe the pathname is relative to cwd
        let mut current = proc_data.cwd.clone();
        current.append_str(path);
        current
    };

    // Get directory containing file
    let Some(parent_node) = path.clone().del_last().get_node() else { return -1 };
    let parent_node = if let vfs::Node::Folder(val) = parent_node { val } else { return -1 };

    let node = {
        let search_result = (*parent_node).borrow_mut().get_children().into_iter().find(|child| child.0 == path.last());
        if let Some((_, mut node)) = search_result {
            // O_TRUNC
            // If the file already exists and is a regular file and the
            //   access mode allows writing (i.e., is O_RDWR or O_WRONLY)
            //   it will be truncated to length 0.
            // Source: man open
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

    // FIXME: This is non-compliant
    // NOTE: Consider changing it so it actually uses the lowest fd number, instead of using a fd number that is known to be free
    proc_data.open_nodes.push(Some(ProcessNode { vfs_node: node, cursor: 0, flags, path, reference_count: 1 }));
    let node_index = proc_data.open_nodes.len() - 1;
    proc_data.fd_mappings.push(Some(FdMapping::Regular(node_index)));

    // -1 because we just added a new one
    proc_data.fd_mappings.len() as isize - 1
}

// source: man close
pub fn close(proc_data: &mut ProcessData, fd: usize) -> isize {
    let Some(Some(fd_mapping)) = proc_data.fd_mappings.get(fd).cloned() else { return -1 };

    match fd_mapping {
        FdMapping::Regular(node_index) => {
            // NOTE: If the fd mapping exists then so must the node so it's fine to unwrap
            let node = proc_data.open_nodes[node_index].as_mut().unwrap();
            if node.reference_count == 1 {
                proc_data.open_nodes[node_index] = None;
            } else if node.reference_count > 1 {
                node.reference_count -= 1;
            }
        }

        FdMapping::PipeReadEnd(pipe_index) => {
            let mut pipes = scheduler::PIPES.lock();
            // NOTE: If a FdMapping exists then the pipe must exist
            let pipe = pipes[pipe_index].as_mut().unwrap();

            if pipe.readers_count == 1 && pipe.writers_count == 0 {
                pipes[pipe_index] = None;

                // Drain None's if it wouldn't affect the location of Some's
                while pipes.last().map(|val|val.is_none()).unwrap_or(false) {
                    pipes.pop();
                }
                
                pipes.shrink_to_fit();
            } else if pipe.readers_count >= 1 { // Pipes can still exist with 0 writes, this is to allow readers to finish reading all the contents of the pipe
                pipe.readers_count -= 1;
            }
        }

        FdMapping::PipeWriteEnd(pipe_index) => {
            let mut pipes = scheduler::PIPES.lock();
            // NOTE: If a FdMapping exists then the pipe must exist
            let pipe = pipes[pipe_index].as_mut().unwrap();
            if pipe.writers_count == 1 && pipe.readers_count == 0 {
                pipes[pipe_index] = None;
    
                // Drain None's if it wouldn't affect the location of Some's
                while pipes.last().map(|val|val.is_none()).unwrap_or(false) {
                    pipes.pop();
                }
            } else if pipe.writers_count >= 1 { // Pipes can still exist with 0 readers, this is to make it so that a FdMapping to a pipe is always valid to keep the system simple
                pipe.writers_count -= 1;
            } 

        }
        FdMapping::Stdin | FdMapping::Stdout | FdMapping::Stderr => {} // No need to close stdin, stdout or stderr since they are not actual nodes
    }

    proc_data.fd_mappings[fd] = None;

    // Drain None's if it wouldn't affect the indices of elements that are Some
    // Note: We cannot use retain or drain_filter because we need to keep the index of fds that are Some, which removing all None using retain or drain_filter will not do, for ex. on the vector: Some None Some
    while proc_data.fd_mappings.last().map(|val|val.is_none()).unwrap_or(false) {
        proc_data.fd_mappings.pop();
    }

    while proc_data.open_nodes.last().map(|val|val.is_none()).unwrap_or(false) {
        proc_data.open_nodes.pop();
    }

    return 0;
}

fn lseek(proc_data: &mut ProcessData, fd: usize, offset: i64, whence: usize) -> i64 {
    let Some(Some(node_mapping)) = proc_data.fd_mappings.get(fd).cloned() else { return -1 };

    match node_mapping {
        FdMapping::Regular(node_index) => {
            let node = proc_data.open_nodes[node_index].as_mut().unwrap();
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

        // It is not possible to apply lseek(2) to a pipe.
        // Source: man pipe
        FdMapping::PipeReadEnd(_) | FdMapping::PipeWriteEnd(_) => return -1,
    }
}

fn malloc(emu: &mut Emulator, proc_data: &mut ProcessData, size: usize) -> u64 {
    // We also allocate size_of::<usize>() bytes more than we are requested to, to store the size of the allocation
    let Ok(allocation_info) = core::alloc::Layout::from_size_align(size + core::mem::size_of::<usize>(), 8) else {
        return virtmem::USERSPACE_NULL_PTR;
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
    let virtual_allocation_ptr = proc_data.virtual_allocator.alloc(allocation_info) as u64;
    if virtual_allocation_ptr == virtmem::USERSPACE_NULL_PTR { return virtmem::USERSPACE_NULL_PTR; }

    // Create mapping
    emu.memory.add_region(virtual_allocation_ptr, physical_allocation);

    return virtual_allocation_ptr+core::mem::size_of::<usize>() as u64;
}

fn free(emu: &mut Emulator, proc_data: &mut ProcessData, virtual_ptr: u64) {

    // Translate pointer from user space
    let Some(mapped_alloc) = emu.memory.try_map_mut(virtual_ptr) else {
        return;
    };

    let size = usize::from_le_bytes(mapped_alloc.0.backing_storage[0..core::mem::size_of::<usize>()].try_into().unwrap());
    let Ok(allocation_info) = core::alloc::Layout::from_size_align(size, 8) else{
        return;
    };

    let alloc_region_index = mapped_alloc.1.region_index;
    emu.memory.remove_region(alloc_region_index);

    proc_data.virtual_allocator.dealloc((virtual_ptr-core::mem::size_of::<usize>() as u64) as *mut u8, allocation_info);
}

fn realloc(emu: &mut Emulator, proc_data: &mut ProcessData, virtual_ptr: u64, new_size: usize) -> u64 {
    // Translate pointer from user space
    let Some(mapped_alloc) = emu.memory.try_map_mut(virtual_ptr) else {
        return virtmem::USERSPACE_NULL_PTR;
    };

    let data: Vec<u8, &'static ProgramBasicAlloc> = mapped_alloc.0.backing_storage.clone();
    free(emu, proc_data, virtual_ptr);
    let new_virtual_ptr = malloc(emu, proc_data, new_size);
    // Translate pointer from user space
    let Some(new_mapped_alloc) = emu.memory.try_map_mut(new_virtual_ptr) else {
        return virtmem::USERSPACE_NULL_PTR;
    };
    new_mapped_alloc.0.backing_storage = data;
    return new_virtual_ptr;
}

fn getcwd(emu: &mut Emulator, proc_data: &mut ProcessData, virtual_ptr: virtmem::UserPointer<[u8]>, buf_size: usize) -> u64 {
    // On failure, these functions return NULL
    // Source: man getcwd

    // If the length of the absolute pathname of the current working
    // directory, including the terminating null byte, exceeds size
    // bytes, NULL is returned
    // Source: man getcwd
    if proc_data.cwd.len() + 1 > buf_size {
        return virtmem::USERSPACE_NULL_PTR;
    }

    let Some(buf) = virtual_ptr.try_as_mut(&mut emu.memory, buf_size) else {
        return virtmem::USERSPACE_NULL_PTR;
    };

    for (i, c) in proc_data.cwd.chars().enumerate() {
        buf[i] = c as u8;
    }

    buf[buf.len() - 1] = b'\0';
    return virtual_ptr.get_inner();
}

fn fchdir(proc_data: &mut ProcessData, fd: usize) -> isize {
    let Some(Some(node_mapping)) = proc_data.fd_mappings.get(fd).cloned() else {
        return -1;
    };

    match node_mapping {
        FdMapping::Regular(node_index) => {
            let node = proc_data.open_nodes[node_index].as_mut().unwrap();
            proc_data.cwd = node.path.clone();
            return 0;
        }
        FdMapping::Stdin | FdMapping::Stdout | FdMapping::Stderr | FdMapping::PipeReadEnd(_) | FdMapping::PipeWriteEnd(_) => return -1,
    }
}

fn getenv(emu: &mut Emulator, proc_data: &mut ProcessData, virtual_ptr: virtmem::UserPointer<[u8]>) -> u64 {
    // The getenv() function returns a pointer to the value in the
    // environment, or NULL if there is no match.
    // Source: man getenv

    let Some(key) = virtmem::cstr_user_pointer_to_str(virtual_ptr, &emu.memory) else {
        return virtmem::USERSPACE_NULL_PTR;
    };

    // Lookup the environment variable
    let Some(res) = proc_data.env.get(key) else {
        return virtmem::USERSPACE_NULL_PTR;
    };

    return *res;
}

fn dup(proc_data: &mut ProcessData, oldfd: usize) -> isize {
    let Some(Some(node_mapping)) = proc_data.fd_mappings.get_mut(oldfd).cloned() else {
        return -1;
    };

    match node_mapping {
        FdMapping::Regular(node_index) => {
            // NOTE: If the node_index exists then the node must exist
            let node = proc_data.open_nodes[node_index].as_mut().unwrap();
            node.reference_count += 1;
        }

        FdMapping::PipeReadEnd(pipe_index) => {
            let mut pipes = scheduler::PIPES.lock();
            let pipe = pipes[pipe_index].as_mut().unwrap();
            pipe.readers_count += 1;
        }

        FdMapping::PipeWriteEnd(pipe_index) => {
            let mut pipes = scheduler::PIPES.lock();
            let pipe = pipes[pipe_index].as_mut().unwrap();
            pipe.writers_count += 1;
        }

        FdMapping::Stdin | FdMapping::Stdout | FdMapping::Stderr => {} // No need to update ref count of stdin, stdout or stderr since they are handled specially anyways
    }

    // FIXME: This is non-compliant
    // NOTE: Consider changing it so it uses the lowest fd number available instead of a fd number that is known to be free
    proc_data.fd_mappings.push(Some(node_mapping));
    // -1 because we just added the new one
    proc_data.fd_mappings.len() as isize - 1
}

fn dup2(proc_data: &mut ProcessData, oldfd: usize, newfd: usize) -> isize {
    let Some(Some(node_mapping)) = proc_data.fd_mappings.get_mut(oldfd).cloned() else {
        return -1;
    };

    close(proc_data, newfd);

    match node_mapping {
        FdMapping::Regular(node_index) => {
            // NOTE: If the fd mapping exists then the node must exist
            let node = proc_data.open_nodes[node_index].as_mut().unwrap();
            node.reference_count += 1;
        }

        FdMapping::PipeReadEnd(pipe_index) => {
            let mut pipes = scheduler::PIPES.lock();
            let pipe = pipes[pipe_index].as_mut().unwrap();
            pipe.readers_count += 1;
        }

        FdMapping::PipeWriteEnd(pipe_index) => {
            let mut pipes = scheduler::PIPES.lock();
            let pipe = pipes[pipe_index].as_mut().unwrap();
            pipe.writers_count += 1;
        }

        FdMapping::Stdin | FdMapping::Stdout | FdMapping::Stderr => {} // No need to update ref count of stdin, stdout or stderr since they are handled specially anyways
    }

    if newfd > proc_data.fd_mappings.len() {
        proc_data.fd_mappings.resize(newfd+1, None);
    }

    proc_data.fd_mappings[newfd] = Some(node_mapping);
    newfd as isize
}

fn fork(emu: &mut Emulator, proc_data: &mut ProcessData) -> usize {
    match proc_data.state {
        ProcessState::RUNNING_NEW_CHILD_JUST_FORKED => {
            // We are the child and we are executing the fork again because we got cloned before our parent could get past the syscall instruction
            proc_data.state = ProcessState::RUNNING;
            0
        }

        _ => {
            // We are the parent
            let mut child = Process::new(emu.clone(), proc_data.clone());

            // Since pipes are global we need to go through all the pipe fds and increment their reference counts
            // as cloning the proc_data is equivalent to creating new fds that point to the same pipe with dup
            // The reason this is fine for regular files is because files are cloned when we clone, but
            // pipes are not as they are global
            let mut pipes = scheduler::PIPES.lock();

            for fd_mapping in child.data.fd_mappings.iter() {
                match fd_mapping.clone() {
                    Some(FdMapping::PipeReadEnd(pipe_index)) => {
                        pipes[pipe_index].as_mut().unwrap().readers_count += 1;
                    }
                    
                    Some(FdMapping::PipeWriteEnd(pipe_index)) => {
                        pipes[pipe_index].as_mut().unwrap().writers_count += 1;
                    }

                    _ => ()
                }
            }

            // Since our tick hasn't ended we haven't advanced past the syscall instruction
            // so the child we just made is still on the same instruction as us, which is syscall fork
            // So when we tick it, it will also call fork, to prevent it from cloning itself again, we use the just_forked flag
            // to inform it that it is a new child and that it's call to fork should return 0
            child.data.parent_pid = proc_data.pid;
            child.data.state = ProcessState::RUNNING_NEW_CHILD_JUST_FORKED;
            scheduler::new_task(child)
        }
    } 
}

fn waitpid(emu: &mut Emulator, proc_data: &mut ProcessData, pid: isize, wstatus: UserPointer<usize>, options: usize) -> Option<isize> {
    // RETURN VALUE
    // waitpid(): on success, returns the process ID of the child whose
    // state has changed; if WNOHANG was specified and one or more
    // child(ren) specified by pid exist, but have not yet changed
    // state, then 0 is returned.  On failure, -1 is returned.
    // source: man waitpid

    // NOTE: options (like WUNTRACED) are irrelevant right now since we don't support STOPPING and RESUMING
    // FIXME: Support waiting for stopping and resuming once we have signals

    if let ProcessState::FINISHED_WAITING_FOR_CHILD_PROCESS(info) = proc_data.state {
        proc_data.state = ProcessState::RUNNING;
        match info {
            Some(info) => {
                let Some(wstatus_ptr) = wstatus.try_as_ptr(&mut emu.memory) else {
                    return Some(-1); // wstatus is pointing to an address that is not mapped
                };

                if wstatus_ptr != null_mut() {
                    match info.action {
                        crate::process::WaitAction::EXITED { exit_code } => unsafe{ *wstatus_ptr = 0b00_00000000 | exit_code & 0b11111111; },
                        crate::process::WaitAction::TERMINATED_BY_SIGNAL { signal } => unsafe{ *wstatus_ptr = 0b01_00000000 | (u8::from(signal.signal_type) as usize) & 0b11111111; },
                    }
                }

                return Some(info.cpid as isize);
            }

            None => return Some(-1) // The child pid that we were waiting for is invalid or there is no child to wait for
        }
    }else{

        if pid == -1 { // Wait for any child process
            proc_data.state = ProcessState::WAITING_FOR_CHILD_PROCESS{cpid: None};
            return None; // Tell the cpu to repeat syscall so once the scheduler tells us that the child received an update we can return
        } else if pid > 0 { // Wait for a specific child process
            proc_data.state = ProcessState::WAITING_FOR_CHILD_PROCESS{cpid: Some(pid as usize)};
            return None; // Tell the cpu to repeat syscall so once the scheduler tells us that the child received an update we can return
        }

        return Some(-1);
    }
}



// FIXME: Support elf interpreters
mod exec_internal {

use super::*;

// Internal function that accepts internal args and env formats, used to carry out the actual replacement of the running program with the new program
// NOTE: Supports interpreter scripts
pub fn exec(emu: &mut Emulator, proc_data: &mut ProcessData, node: vfs::Node, node_path: vfs::Path, args: Vec<String>, envs: BTreeMap<String, String>, recursion_depth: usize) -> Result<(), isize> {
    if recursion_depth > 100 {
        // Too many recursive calls
        // This can happen if for example the interpreter of a script file is itself
        return  Err(-1);
    }

    let file = if let vfs::Node::File(f) = &node {
        f.borrow()
    }else{
        // fd was not a file
        return Err(-1);
    };

    let file_size = file.get_size() as usize;

    // We don't need the node anymore so shadow it with ()
    #[allow(unused)]
    let program_node = ();

    let Some(file_bytes) = file.read(0, file_size) else {
        // Couldn't read file
        return Err(-1);
    };

    if let Some(elf) = ElfFile::from_bytes(&file_bytes) {
        if elf.header.instruction_set != elf_header::InstructionSet::RiscV {
            return Err(-1);
        }

        if elf.header.elf_type != elf_header::ElfType::EXECUTABLE {
            return Err(-1);
        }

        // Time to replace ourselves with this new executable

        // First reset ourselves, past this point returning -1 is useless as the program would crash anyways
        // FIXME: Don't crash the program if we fail to load the new program
        emu.memory.clear_regions();
        emu.reset_registers(elf.header.program_entry);

        let lower_virt_addr = Process::load_elf_into_virtual_memory(&elf, &file_bytes, &mut emu.memory)
        .ok_or_else(|| {
            exit( proc_data, 0xDED);
            return -1isize; // We need to return something so just return -1 even if it doesn't matter
        })?; // Return -1 if we can't expand and map the elf into virtual memory
        
        const PROGRAM_STACK_SIZE: u64 = 8 * 1024;
        let mut program_stack = Vec::new_in(&allocator::PROGRAM_ALLOCATOR);
        program_stack.clear();
        program_stack.resize(PROGRAM_STACK_SIZE as usize, 0u8);

        // Add 8kb of stack space at the end of the virtual address space
        let did_create_stack_region =  emu.memory.add_region(
            u64::MAX - (PROGRAM_STACK_SIZE) + 1,     /* +1 because the address itself is included in the region */
            program_stack, // NOTE: We don't use [] because that would allocate 1MB on the stack, then move it to the heap, which might overflow the stack
        );
        if did_create_stack_region.is_none() { // We failed to add a stack region
            exit( proc_data, 0xDED);
            return Err(-1); // We need to return something so just return -1 even if it doesn't matter
        }

        // Create virtual allocator for the heap, this manages the locations of allocations on the heap in the virtual space
        // Or just generally the location of segments in virtual space, this can't be done for some segments like the elf regions and the stack
        // as they require certain addresses
        proc_data.virtual_allocator = BasicAlloc::from(lower_virt_addr as *mut u8, (u64::MAX - (PROGRAM_STACK_SIZE + lower_virt_addr)) as usize, true);


        let Some(args_ptrs_array_virtual_ptr) = Process::load_args_into_virtual_memory(
            args.iter().map(|arg|arg.as_str()), 
            args.len(), 
            &mut emu.memory, 
            &mut proc_data.virtual_allocator
        ) else {
            exit( proc_data, 0xDED);
            return Err(-1); // We need to return something so just return -1 even if it doesn't matter
        };


        let Some(prog_env) = Process::load_env_into_virtual_memory(
            envs.iter().map(|(key, value)| (key.as_str(), value.as_str())), 
            &mut emu.memory, 
            &mut proc_data.virtual_allocator
        ) else {
            exit( proc_data, 0xDED);
            return Err(-1); // We need to return something so just return -1 even if it doesn't matter
        };

        proc_data.env = prog_env;
        

        // Setup argc and argv
        emu.write_reg(10, args.len() as u64); // argc
        emu.write_reg(11, args_ptrs_array_virtual_ptr as u64); // argv

        // We succeeded
        return Ok(());
    } else { 
        // Maybe it's a interpreter script file
        // Check for #!
        if file_bytes[0] == b'#' && file_bytes[1] == b'!' {
            // Read until first '\n'  to read the first line
            let Some(first_line) = file_bytes.split(|&byte| byte == b'\n').next() else {
                return Err(-1);
            };
            
            let Ok(first_line)  = core::str::from_utf8(first_line) else {
                return Err(-1); // Error out if the first line is not valid utf-8
            };

            let first_line = first_line[2..].trim(); // Ignore #! and extraneous whitespace

            let (interpreter, opt_arg) = if let Some((interpreter, opt_arg)) = first_line.split_once(' ') {
                // We found a space so there is an optional argument
                // NOTE: We parse the optional argument as everything after the interpreter so it may contain whitespace itself
                (interpreter, Some(opt_arg))
            }else{
                // The entire line is just the interpreter
                (first_line, None)
            };
            
            let Ok(interpreter_path) = vfs::Path::try_from(interpreter) else { 
                // Error out if interpreter is not an absolute path, as we will not accept searching for it in the PATH environment variable
                return Err(-1) 
            };
        
            let Some(interpreter_node) = interpreter_path.clone().get_node() else { return Err(-1) }; // Error out if the path is valid but the file doesn't exist
        
            // Now we must modify the args so that they contain opt_arg and the scripts's path
            // NOTE: The environment stays the same, so the interpreter will get the environment provided to exec if the file is a script

            
            //  If the pathname argument of execve() specifies an interpreter
            //  script, then interpreter will be invoked with the following
            //  arguments:

            //  interpreter [optional-arg] pathname arg...
            // Source: man execve
            
            let mut new_args = Vec::new();
            new_args.push(interpreter_path.clone().into_inner()); // the first argument for the interpreter is it's path
            if let Some(interpreter_arg) = opt_arg { new_args.push(interpreter_arg.to_owned()); } // If the opt_arg exists the push it
            new_args.push(node_path.into_inner()); // path to the file(script) provided to exec


            // Finally append the arguments provided to exec, ignoring the first one which would have been argv[0]

            // arg...  is the series of words pointed
            // to by the argv argument of execve(), starting at argv[1].  Note
            // that there is no way to get the argv[0] that was passed to the
            // execve() call.
            // Source: man execve

            new_args.extend(args.into_iter().skip(1));

            return exec(emu, proc_data, interpreter_node, interpreter_path, new_args, envs, recursion_depth+1);
        }else{
            // It's not an elf or a shell script, so error out
            return Err(-1);
        }
    }
}


// Transform a user provided argv into an internal args representation
pub fn parse_exec_args(virtual_memory: &impl VirtualMemory, argv: virtmem::UserPointer<[u64]>) -> Option<Vec<String>> {
    let mut parsed_args = Vec::new();
    // argv is a pointer to pointers, the end is found by finding the first null pointer
    let args_ref = {
        let mut argv_ptr = argv.try_as_ptr(virtual_memory)?; // Error if the argv pointer is not in mapped virtual space
        let mut args_len = 0;
        while unsafe{*argv_ptr} != virtmem::USERSPACE_NULL_PTR {
            // NOTE: count is in units of T, so add 1 goes to the next pointer
            // Source: https://doc.rust-lang.org/std/primitive.pointer.html#method.add
            argv_ptr = unsafe{argv_ptr.add(1)}; 
            args_len += 1;
        }
        argv.try_as_ref(virtual_memory, args_len)? // Error if we can't create a slice from it
    };

    for &virtual_arg_ptr in args_ref {
        let arg = unsafe{ virtmem::UserPointer::<[u8]>::from_mem(virtual_arg_ptr) };
        let Some(arg) = virtmem::cstr_user_pointer_to_str(arg, virtual_memory) else {
            // Couldn't parse part of argv as str, maybe it contains invalid utf8?
            return None;
        };
        parsed_args.push(arg.to_owned());
    }
    Some(parsed_args)
}

// Transform a user provided envp into an internal env representation
pub fn parse_exec_env(virtual_memory: &impl VirtualMemory, envp: virtmem::UserPointer<[u64]>) -> Option<BTreeMap<String, String>> {
    let mut parsed_env = BTreeMap::new();
    let envs_ref = {
        let mut envs_ptr = envp.try_as_ptr(virtual_memory)?; // Error if the argv pointer is not in mapped virtual space
        let mut envs_len = 0;
        while unsafe{*envs_ptr} != 0 {
            // NOTE: count is in units of T, so add 1 goes to the next pointer
            // Source: https://doc.rust-lang.org/std/primitive.pointer.html#method.add
            envs_ptr = unsafe{envs_ptr.add(1)};
            envs_len += 1;
        }
        envp.try_as_ref(virtual_memory, envs_len)? // Error if we can't create a slice
    };

    for &virtual_env_ptr in envs_ref {
        let env_str = unsafe{ virtmem::UserPointer::<[u8]>::from_mem(virtual_env_ptr) };
        let env_str = virtmem::cstr_user_pointer_to_str(env_str, virtual_memory);
        let Some(env_str) = env_str else {
            // Couldn't parse part of envp as str, maybe it contains invalid utf8?
            return None;
        };

        let Some((name, value)) = env_str.split_once('=') else {
            // Malformed env variable
            return None;
        };

        parsed_env.insert(name.to_owned(), value.to_owned());
    }

    Some(parsed_env)
}

// FIXME: This accepts a null envp to mean inherit current program's environment, the behavior of a null envp is unspecified as far as i'm aware, so this is NON-STANDRD
// envp quirk refers to the fact that it accepts null envp
pub fn parse_argv_and_envp_with_envp_quirk(virtual_memory: &impl VirtualMemory, proc_data: &ProcessData, argv: virtmem::UserPointer<[u64]>, envp: virtmem::UserPointer<[u64]>) -> Option<(Vec<String>, BTreeMap<String, String>)> {
    // Parse argv and envp

    // WARNING: At the time of writing this comment, if argv is null 
    //          then when parse_exec_args calls try_as_ptr it will try to map null 
    //          which is not mapped in virtual space, so it will simply fail 
    //          and return None which will cause us to return -1 due to ok_or_else
    let parsed_args = parse_exec_args(virtual_memory, argv)?;

    let parsed_env = if envp.get_inner() != virtmem::USERSPACE_NULL_PTR {
        parse_exec_env(virtual_memory, envp)
    }else{  
        // If we get a null pointer, then reuse our current environment
        // WARNING: NON-STANDARD
        let mut env = BTreeMap::new();
        for (name, &virtual_pointer_to_val) in &proc_data.env {
            let val = unsafe{ virtmem::UserPointer::<[u8]>::from_mem(virtual_pointer_to_val)};
            let Some(val) = virtmem::cstr_user_pointer_to_str(val, virtual_memory) else {
                // *This program's* environment is malformed, invalid utf8?
                return None;
            };

            env.insert(name.clone(), val.to_owned());
        }

        Some(env)
    }?; // Error out if parse_exec_env fails to parse the environment provided

    return Some((parsed_args, parsed_env));
}

}

fn fexecve(emu: &mut Emulator, proc_data: &mut ProcessData, fd: usize, argv: virtmem::UserPointer<[u64]>, envp: virtmem::UserPointer<[u64]>) -> Result<(), isize> {
    let Some(Some(FdMapping::Regular(node_index))) = proc_data.fd_mappings.get(fd).cloned() else{
        // Causes invalid fds and fds that map to stdin, stdout and stderr to error out
        return Err(-1);
    };
    
    // NOTE: If the node_index exists then the node must exist
    let node = proc_data.open_nodes[node_index].as_mut().unwrap().clone();

    // The file descriptor fd must be opened read-only
    //    (O_RDONLY) or with the O_PATH flag and the caller must have permission
    //    to execute the file that it refers to.
    // source: freebsd man fexecve
    if node.flags & O_RDONLY == 0 {
        return Err(-1);
    }

    let (parsed_args, parsed_env) = exec_internal::parse_argv_and_envp_with_envp_quirk(&emu.memory, proc_data, argv, envp)
    .ok_or_else(|| -1isize)?;
  
    // Finally now that we have the parsed args and environment do the exec
    exec_internal::exec(emu, proc_data, node.vfs_node, node.path, parsed_args, parsed_env, 0)
}


fn execve(emu: &mut Emulator, proc_data: &mut ProcessData, pathname: virtmem::UserPointer<[u8]>, argv: virtmem::UserPointer<[u64]>, envp: virtmem::UserPointer<[u64]>) -> Result<(), isize> {
    let Some(path) = virtmem::cstr_user_pointer_to_str(pathname, &emu.memory) else { return Err(-1) };
    let Ok(path) = vfs::Path::try_from(path) else { 
        // NOTE: execve does not allow CWD relative paths or searching in the PATH environment variable so error out
        return Err(-1) 
    };

    let Some(node) = path.clone().get_node() else { return Err(-1) };

    let (parsed_args, parsed_env) = exec_internal::parse_argv_and_envp_with_envp_quirk(&emu.memory, proc_data, argv, envp)
    .ok_or_else(|| -1isize)?;

    // Finally now that we have the parsed args and environment do the exec
    exec_internal::exec(emu, proc_data, node, path, parsed_args, parsed_env, 0)
}


fn execvpe(emu: &mut Emulator, proc_data: &mut ProcessData, file: virtmem::UserPointer<[u8]>, argv: virtmem::UserPointer<[u64]>, envp: virtmem::UserPointer<[u64]>) -> Result<(), isize> {
    // Same as execve but if path lookup fails for file then it looks in the PATH environment variable of the current program

    let Some(file) = virtmem::cstr_user_pointer_to_str(file, &emu.memory) else { return Err(-1) };
    let (path, node) = if let Some(path) = vfs::Path::try_from(file).ok() {
        (Some(path.clone()), path.get_node())
    }else{
        (None, None)
    };

    // If both the path and hte node are valid after just trying the file as an absolute path then return them else do PATH environment variable lookup
    let (path, node) = if let (Some(path), Some(node)) = (path, node) { (path, node) } else {
        // NOTE: execvpe does not allow CWD relative paths, but does allow lookup in the PATH environment variable
        // NOTE: We only get here if we couldn't parse the file as an absolute path

        // Get *our* PATH environment variable
        let Some(&path_value) = proc_data.env.get("PATH") else {
            return Err(-1); // file is not an absolute path and PATH variable doesn't exist
        };
        
        let path_value = unsafe{virtmem::UserPointer::<[u8]>::from_mem(path_value)};
        let Some(path_value) = virtmem::cstr_user_pointer_to_str(path_value, &emu.memory) else {
            return Err(-1); // PATH environment variable value is not a valid utf-8 string
        };

        let mut found = None;

        // PATH environment variable has the format /a/b:/c/d:/l/m
        for path in path_value.split(':') {
            let Ok(mut path) = Path::try_from(path) else {
                continue; // Ignore malformed paths in the PATH environment variable
            };

            // Try it out
            path.append_str(file);

            if let Some(node) = path.clone().get_node() {
                found = Some((path, node));
                break;
            }
        }

        let Some(found) = found else {
            return Err(-1); // If we couldn't find the file even after PATH lookup then error out
        };

        found
    };

    let (parsed_args, parsed_env) = exec_internal::parse_argv_and_envp_with_envp_quirk(&emu.memory, proc_data, argv, envp)
    .ok_or_else(|| -1isize)?;

    exec_internal::exec(emu, proc_data, node, path, parsed_args, parsed_env, 0)
}

fn pipe(emu: &mut Emulator, proc_data: &mut ProcessData, fds: virtmem::UserPointer<[core::ffi::c_int]>) -> isize {
    let Some(fds) = fds.try_as_mut(&mut emu.memory, 2) else { return -1; }; // Error out if ptr is not mapped in the virtual space
    let mut pipes = scheduler::PIPES.lock();
    let pipe_index = pipes.len();
    pipes.push(Some(ProcessPipe{buf: VecDeque::new(), readers_count: 1, writers_count: 1}));

    // FIXME: This is non-compliant
    // NOTE: Consider using the lowest free fd instead of a fd that is known to be free
    let read_fd = proc_data.fd_mappings.len();
    proc_data.fd_mappings.push(Some(FdMapping::PipeReadEnd(pipe_index)));
    let write_fd = proc_data.fd_mappings.len();
    proc_data.fd_mappings.push(Some(FdMapping::PipeWriteEnd(pipe_index)));
    
    fds[0] = read_fd as core::ffi::c_int;
    fds[1] = write_fd as core::ffi::c_int;

    return 0;
}