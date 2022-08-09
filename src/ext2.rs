use core::{cell::RefCell, str::from_utf8, convert::TryInto};

use alloc::{rc::Rc, vec, vec::Vec, borrow::ToOwned};
use packed_struct::prelude::PackedStruct;

use crate::{vfs::{IFile, self, IFolder}, UART};

#[derive(PackedStruct)]
#[packed_struct(endian = "lsb")] // ext2 is little endian (https://wiki.osdev.org/Ext2#Basic_Concepts)
pub struct Ext2SuperBlock {
    no_of_inodes: u32,
    no_of_blocks: u32,
    superuser_reserved_blocks: u32,
    unallocated_blocks: u32,
    unallocated_inodes: u32,
    superblock_block_number: u32,
    block_size_log2_minus_10: u32,
    fragment_size_log2_minus_10: u32,
    blocks_per_block_group: u32,
    fragments_per_block_group: u32,
    inodes_per_block_group: u32,
    last_mount_unix_timestamp: u32,
    last_written_unix_timestamp: u32,
    mounts_since_last_check: u16,
    mounts_allowed_before_check: u16,
    ext2_signature: u16,
    fs_state: u16,
    on_error: u16,
    minor_version: u16,
    last_check_unix_timestamp: u32,
    max_interval_between_checks: u32,
    os_id: u32,
    major_version: u32,
    uid_for_reserved_blocks: u16,
    gid_for_reserved_blocks: u16
}

#[derive(PackedStruct)]
#[packed_struct(endian = "lsb")] // ext2 is little endian (https://wiki.osdev.org/Ext2#Basic_Concepts)
pub struct Ext2ExtendedSuperblock {
    first_non_reserved_inode_in_fs: u32,
    inode_size: u16,
    block_group_of_this_superblock: u16,
    optional_features: u32, // Features that you could implement but don't have to to read or write
    required_features: u32, // Features that you must implement to write/read
    write_features: u32, // Features that are required for writing but not reading
    fs_id: [u8; 16],
    volume_name: [u8; 16],
    last_mounted: [u8; 64],
    compression_algorithm: u32,
    blocks_to_preallocate_for_files: u8,
    blocks_to_preallocate_for_directories: u8,
    _unused1: u16,
    journal_id: [u8; 16],
    journal_inode: u32,
    journal_device: u32,
    head_of_orphan_inode_list: u32
}


#[derive(PackedStruct)]
#[packed_struct(endian = "lsb")] // ext2 is little endian (https://wiki.osdev.org/Ext2#Basic_Concepts)
pub struct Ext2BlockGroupDescriptor {
    block_addr_for_block_usage_bitmap: u32,
    block_addr_for_inode_usage_bitmap: u32,
    block_addr_for_inode_table: u32,
    unallocated_blocks_in_group: u16,
    unallocated_inodes_in_group: u16,
    directories_in_group: u16,
    // 14 unused bytes
    _unused1: u64,
    _unused2: u32,
    _unused3: u16, 
}

#[derive(PackedStruct)]
#[packed_struct(endian = "lsb")] // ext2 is little endian (https://wiki.osdev.org/Ext2#Basic_Concepts)
pub struct Ext2RawInode {
    type_and_perm: u16,
    user_id: u16,
    low32_size: u32,
    last_access_unix_timestamp: u32,
    creation_unix_timestamp: u32,
    last_modif_unix_timestamp: u32,
    deletion_unix_timestamp: u32,
    group_id: u16,
    hard_links_to_inode: u16,
    disk_sectors_used: u32,
    flags: u32,
    os_value_1: u32,
    direct_block_pointers: [u32; 12],
    singly_indirect_block_pointer: u32,
    doubly_indirect_block_pointer: u32,
    triply_indirect_block_pointer: u32,
    generation_number: u32,
    ext2_majorv1_extended_attribute_block: u32,
    ext2_majorv1_upper32_size: u32,
    block_addr_of_fragment: u32,
    os_value_2: [u32; 3],
}

#[derive(PackedStruct)]
#[packed_struct(endian = "lsb")] // ext2 is little endian (https://wiki.osdev.org/Ext2#Basic_Concepts)
pub struct Ext2DirectoryEntryHeader {
    inode_addr: u32,
    entry_size: u16,
    name_length_low8: u8,
    entry_type: u8
}

impl Ext2RawInode {

    fn get_value_from_u32_array_as_le_bytes(bytes: &[u8], index: usize) -> Option<u32> {
        use core::mem::size_of;
        Some(u32::from_le_bytes(bytes[index*size_of::<u32>()..(index+1)*size_of::<u32>()].try_into().ok()?))
    }

    fn set_value_to_u32_array_as_le_bytes(bytes: &mut [u8], index: usize, val: u32) -> Option<()>{
        use core::mem::size_of;
        let bytes_to_write = u32::to_le_bytes(val);
        let mut iter = bytes_to_write.iter();
        for byte_ind in index*size_of::<u32>()..(index+1)*size_of::<u32>(){
            bytes[byte_ind] = *iter.next()?;
        }
        Some(())
    }



    pub fn read_data_block_pointer(&self, mut block_number: usize, fs: &Ext2FS) -> Option<u32> {
        // TODO: Test all posibilites of this function!!!
        // Direct data
        if block_number <= 11 {
            return Some(self.direct_block_pointers[block_number]);
        }

        let pointers_per_block = fs.get_block_size() as usize/core::mem::size_of::<u32>();

        let read_pointer_from_block = Self::get_value_from_u32_array_as_le_bytes;

        // Singly indirect data
        block_number -= 12;
        if block_number < pointers_per_block {
            let singly_indirect_block_index = block_number; // Index of pointer to data block
            let mut singly_indirect_block = fs.read_block(self.singly_indirect_block_pointer)?;

            return read_pointer_from_block(&mut singly_indirect_block, singly_indirect_block_index);
        }

        // Doubly indirect data
        block_number -= pointers_per_block;
        if block_number < pointers_per_block*pointers_per_block{
            let doubly_indirect_block_index = block_number/pointers_per_block; // Index of pointer to singly indirect block
            let singly_indirect_block_index = block_number%pointers_per_block; // Index of pointer to data block

            let doubly_indirect_block = fs.read_block(self.doubly_indirect_block_pointer)?;
            let mut singly_indirect_block = fs.read_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?)?;
            return read_pointer_from_block(&mut singly_indirect_block, singly_indirect_block_index);
        }

        // Triply indirect data
        block_number -= pointers_per_block*pointers_per_block;
        if block_number < pointers_per_block*pointers_per_block*pointers_per_block{
            let triply_indirect_block_index = block_number/(pointers_per_block*pointers_per_block); // Index of pointer to doubly indirect block
            let doubly_indirect_block_index = (block_number%(pointers_per_block*pointers_per_block))/pointers_per_block; // Index of pointer to singly indirect data block
            let singly_indirect_block_index = (block_number%(pointers_per_block*pointers_per_block))%pointers_per_block; // Index of pointer to data block

            let triply_indirect_block = fs.read_block(self.triply_indirect_block_pointer)?;
            let doubly_indirect_block = fs.read_block(read_pointer_from_block(&triply_indirect_block, triply_indirect_block_index)?)?;
            let mut singly_indirect_block = fs.read_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?)?;
            return read_pointer_from_block(&mut singly_indirect_block, singly_indirect_block_index);
        }
        None
    }

    pub fn dealloc_data_block_pointer(&mut self, mut block_number: usize, fs: &mut Ext2FS) -> Option<()> {
        // TODO: Test all posibilites of this function!!!
        // Direct data
        if block_number <= 11 {
            self.direct_block_pointers[block_number] = 0;
            return Some(());
        }

        let pointers_per_block = fs.get_block_size() as usize/core::mem::size_of::<u32>();

        let read_pointer_from_block = Self::get_value_from_u32_array_as_le_bytes;
        let write_pointer_to_block = Self::set_value_to_u32_array_as_le_bytes;

        // Singly indirect data
        block_number -= 12;
        if block_number < pointers_per_block {
            let singly_indirect_block_index = block_number; // Index of pointer to data block
            let mut singly_indirect_block = fs.read_block(self.singly_indirect_block_pointer)?;

            // Deallocate data block
            fs.dealloc_block(read_pointer_from_block(&mut singly_indirect_block, singly_indirect_block_index)?)?;
            // Override with null data block
            write_pointer_to_block(&mut singly_indirect_block, singly_indirect_block_index, 0)?;
            fs.write_block(self.singly_indirect_block_pointer, &singly_indirect_block)?;
            return Some(());
        }

        // Doubly indirect data
        block_number -= pointers_per_block;
        if block_number < pointers_per_block*pointers_per_block{
            let doubly_indirect_block_index = block_number/pointers_per_block; // Index of pointer to singly indirect block
            let singly_indirect_block_index = block_number%pointers_per_block; // Index of pointer to data block

            let doubly_indirect_block = fs.read_block(self.doubly_indirect_block_pointer)?;
            let mut singly_indirect_block = fs.read_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?)?;
            // Deallocate data block
            fs.dealloc_block(read_pointer_from_block(&singly_indirect_block, singly_indirect_block_index)?)?;
            // Override with null data block
            write_pointer_to_block(&mut singly_indirect_block, singly_indirect_block_index, 0)?;
            fs.write_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?, &singly_indirect_block)?;
            return Some(());
        }

        // Triply indirect data
        block_number -= pointers_per_block*pointers_per_block;
        if block_number < pointers_per_block*pointers_per_block*pointers_per_block{
            let triply_indirect_block_index = block_number/(pointers_per_block*pointers_per_block); // Index of pointer to doubly indirect block
            let doubly_indirect_block_index = (block_number%(pointers_per_block*pointers_per_block))/pointers_per_block; // Index of pointer to singly indirect data block
            let singly_indirect_block_index = (block_number%(pointers_per_block*pointers_per_block))%pointers_per_block; // Index of pointer to data block

            let triply_indirect_block = fs.read_block(self.triply_indirect_block_pointer)?;
            let doubly_indirect_block = fs.read_block(read_pointer_from_block(&triply_indirect_block, triply_indirect_block_index)?)?;
            let mut singly_indirect_block = fs.read_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?)?;
            // Deallocate data block
            fs.dealloc_block(read_pointer_from_block(&singly_indirect_block, singly_indirect_block_index)?)?;
            // Override with null data block
            write_pointer_to_block(&mut singly_indirect_block, singly_indirect_block_index, 0)?;
            fs.write_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?, &singly_indirect_block)?;
            return Some(());
        }
        None
    }

    pub fn shrink_data_structure_to_fit(&mut self, fs: &mut Ext2FS) {
        let read_pointer_from_block = Self::get_value_from_u32_array_as_le_bytes;
        let write_pointer_to_block = Self::set_value_to_u32_array_as_le_bytes;

        if let Some(singly_indirect_block) = fs.read_block(self.singly_indirect_block_pointer){
            if singly_indirect_block.into_iter().all(|v| v == 0) { // Empty block
                fs.dealloc_block(self.singly_indirect_block_pointer);
                self.singly_indirect_block_pointer = 0;
            }
        }

        if let Some(mut doubly_indirect_block) = fs.read_block(self.doubly_indirect_block_pointer){
            for i in (0..fs.get_block_size()/(core::mem::size_of::<u32>() as u32)).rev() {
                if let Some(singly_indirect_block_pointer) = read_pointer_from_block(&doubly_indirect_block, i as usize){
                    if let Some(singly_indirect_block) = fs.read_block(singly_indirect_block_pointer){
                        if singly_indirect_block.iter().all(|v| *v == 0) { // Empty block
                            fs.dealloc_block(singly_indirect_block_pointer);
                            write_pointer_to_block(&mut doubly_indirect_block, i as usize, 0);
                        }else{
                            break; // If this block is not empty then none of the rest should be since they are allocated sequentially
                        }
                    }
                }    
            }

            if doubly_indirect_block.iter().all(|v| *v == 0) { // Empty block
                fs.dealloc_block(self.doubly_indirect_block_pointer);
                self.doubly_indirect_block_pointer = 0;
            }else{
                fs.write_block(self.doubly_indirect_block_pointer, &doubly_indirect_block);
            }
        }

        if let Some(mut triply_indirect_block) = fs.read_block(self.triply_indirect_block_pointer) {
            'big_loop: for i in 0..fs.get_block_size()/(core::mem::size_of::<u32>() as u32) {
                if let Some(doubly_indirect_block_pointer) = read_pointer_from_block(&triply_indirect_block, i as usize){
                    if let Some(mut doubly_indirect_block) = fs.read_block(doubly_indirect_block_pointer) {
                        for j in (0..fs.get_block_size()/(core::mem::size_of::<u32>() as u32)).rev() {
                            if let Some(singly_indirect_block_pointer) = read_pointer_from_block(&doubly_indirect_block, j as usize){
                                if let Some(singly_indirect_block) = fs.read_block(singly_indirect_block_pointer){
                                    if singly_indirect_block.iter().all(|v| *v == 0) { // Empty block
                                        fs.dealloc_block(singly_indirect_block_pointer);
                                        write_pointer_to_block( &mut doubly_indirect_block, j as usize, 0);
                                    }else{
                                        break 'big_loop; // If this block is not empty then none of the rest should be
                                    }
                                }
                            }
                        }

                        if doubly_indirect_block.iter().all(|v| *v == 0) {
                            fs.dealloc_block(doubly_indirect_block_pointer);
                            write_pointer_to_block(&mut triply_indirect_block, i as usize, 0);
                        }else{
                            fs.write_block(doubly_indirect_block_pointer, &doubly_indirect_block);
                        }
                    }
                }
            }

            if triply_indirect_block.iter().all(|v| *v == 0) { // Empty block
                fs.dealloc_block(self.triply_indirect_block_pointer);
                self.triply_indirect_block_pointer = 0;
            }else{
                fs.write_block(self.triply_indirect_block_pointer, &triply_indirect_block);
            }
        }
    }



    pub fn read_raw_data_block(&self, block_number: usize, fs: &Ext2FS) -> Option<Vec<u8>> {
        return fs.read_block(self.read_data_block_pointer(block_number, fs)?);
    }

    pub fn write_raw_data_block(&self, block_number: usize, data: &[u8], fs: &mut Ext2FS) -> Option<()> {
        return fs.write_block(self.read_data_block_pointer(block_number, fs)?, data);
    }



    pub fn read_bytes(&self, offset: usize, len: usize, e2fs: &Ext2FS) -> Option<Vec<u8>> {
        if offset+len > self.get_size() { return None; }
        let starting_block_index = offset/(e2fs.get_block_size() as usize);
        let starting_block_offset = offset%(e2fs.get_block_size() as usize);
        let mut res: Vec<u8> = Vec::with_capacity(len);

        let first_block = self.read_raw_data_block(starting_block_index, e2fs)?;
        for e in &first_block[starting_block_offset..] { res.push(*e); }

        let extra_block = if len%e2fs.get_block_size() as usize != 0 { 1 } else { 0 };
        for block_ind in 1..(len/e2fs.get_block_size() as usize+extra_block) {
            res.append(
                &mut self.read_raw_data_block(starting_block_index+block_ind, e2fs)?
            );
        }

        while res.len() > len { res.pop(); }
        Some(res)
    }

    pub fn write_bytes(&self, offset: usize, data: &[u8], e2fs: &mut Ext2FS) -> Option<usize> {
        if offset+data.len() > self.get_size() { return None; }
        let starting_block_addr = offset/(e2fs.get_block_size() as usize);
        let offset_in_starting_block = offset%(e2fs.get_inode_size() as usize);
        let mut iter = data.iter();

        let extra_block = if data.len()%e2fs.get_block_size() as usize != 0 { 1 } else { 0 };
        let mut ind = offset_in_starting_block;
        for block_ind in 0..(data.len()/e2fs.get_block_size() as usize+extra_block) {
            // No need to read sectors that we know will be completly overriden
            let mut v = if (block_ind == (data.len()/e2fs.get_block_size() as usize+extra_block-1) && extra_block == 1) || (block_ind == 0 && offset_in_starting_block != 0) {
                self.read_raw_data_block(starting_block_addr+block_ind, e2fs)?
            }else{
                vec![0u8; e2fs.get_block_size() as usize]
            };

            while let Some(a) = iter.next() {
                if ind >= v.len() { break; }
                v[ind] = *a; 
                ind += 1; 
            }
            ind = 0;

            self.write_raw_data_block(starting_block_addr+block_ind, &v, e2fs);
        }

        Some(data.len())
    }



    pub fn shrink_by(&mut self, nbytes: usize, e2fs: &mut Ext2FS) -> Option<()> {
        let mut blocks_to_remove = nbytes/e2fs.get_block_size() as usize;
        {
        let bytes_to_remove_from_last_block = nbytes%e2fs.get_block_size() as usize;
        let bytes_used_in_last_block = self.get_size()%e2fs.get_block_size() as usize;
        if bytes_used_in_last_block <= bytes_to_remove_from_last_block {
            blocks_to_remove += 1;
        }
        }

        let last_data_block_index = self.get_size()/e2fs.get_block_size() as usize;
        use core::fmt::Write;
        writeln!(UART.lock(), "Removing block pointers!").unwrap();
        for i in 0..blocks_to_remove {
            self.dealloc_data_block_pointer(last_data_block_index-i, e2fs)?;
        }
        writeln!(UART.lock(), "Shrinking inode!").unwrap();
        self.shrink_data_structure_to_fit(e2fs);
        writeln!(UART.lock(), "Done!").unwrap();
        self.set_size(self.get_size()-nbytes);
        Some(())
    }

    pub fn resize(&mut self, new_size: usize, e2fs: &mut Ext2FS) -> Option<()> {
        if new_size == self.get_size() { return Some(()); }
        if new_size < self.get_size() {
            self.shrink_by(self.get_size()-new_size, e2fs)
        }else{
            None
        }
    }

    pub fn as_vfs_node(self, fs: Rc<RefCell<Ext2FS>>, inode_addr: u32) -> Option<vfs::Node> {
        if self.type_and_perm & 0xF000 == 0x4000 { 
            return Some(vfs::Node::Folder(Rc::new(RefCell::new(Ext2Folder{inode: self, fs})) as Rc<RefCell<dyn IFolder>>));
        }
        if self.type_and_perm & 0xF000 == 0x8000 {
            return Some(vfs::Node::File(Rc::new(RefCell::new(Ext2File{inode: self, inode_addr, fs})) as Rc<RefCell<dyn IFile>>));
        }
        None
    }

    pub fn get_size(&self) -> usize {
        // FIXME: Handle larger files
        self.low32_size as usize
    }

    fn set_size(&mut self, new_size: usize) {
        self.low32_size = new_size as u32;
    }

}

pub struct Ext2File {
    inode: Ext2RawInode,
    inode_addr: u32,
    fs: Rc<RefCell<Ext2FS>>,
}

impl vfs::IFile for Ext2File{
    fn read(&self, offset: usize, len: usize) -> Option<Vec<u8>> {
        self.inode.read_bytes(offset, len, &*self.fs.borrow())
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> Option<usize> {
        self.inode.write_bytes(offset, data, &mut *self.fs.borrow_mut())
    }

    fn get_size(&self) -> usize {
        self.inode.get_size()
    }

    fn resize(&mut self, new_size: usize) -> Option<()>{
        self.inode.resize(new_size, &mut *self.fs.borrow_mut())?;
        self.fs.borrow_mut().write_inode(self.inode_addr, &self.inode)?;
        Some(())
    }
}

pub struct Ext2Folder {
    inode: Ext2RawInode, 
    fs: Rc<RefCell<Ext2FS>>
}

impl IFolder for Ext2Folder {
    fn get_children(&self) -> Vec<(alloc::string::String, vfs::Node)> {
        let raw_data = self.inode.read_bytes(0, self.inode.low32_size as usize, &*self.fs.borrow());
        let mut res = Vec::new();
        if let Some(raw_data) = raw_data {
            let mut cur_ind = 0;
            while cur_ind < raw_data.len(){
                let entry = Ext2DirectoryEntryHeader::unpack(raw_data[cur_ind..cur_ind+core::mem::size_of::<Ext2DirectoryEntryHeader>()].try_into().expect("Reading directory entry should always work!")).expect("Parsing directory entry should always work!");
                cur_ind += Ext2FS::get_directory_entry_header_size();
                
                let name: &str = from_utf8(&raw_data[cur_ind..cur_ind+entry.name_length_low8 as usize]).expect("Ext2 inode name in directory entry should be valid utf-8!");
                cur_ind += entry.entry_size as usize-Ext2FS::get_directory_entry_header_size();

                let inode = self.fs.borrow().read_inode(entry.inode_addr).expect("Inode in directory entry should be valid!");
                res.push((name.to_owned(), inode.as_vfs_node(self.fs.clone(), entry.inode_addr).expect("Inodes should be parsable as vfs nodes!")))
            }
        }
        res
    }
}

pub struct Ext2FS {
    backing_device: Rc<RefCell<dyn IFile>>,
    pub sb: Ext2SuperBlock,
    pub extended_sb: Option<Ext2ExtendedSuperblock>,
}


impl Ext2FS {
    pub fn new(backing_dev: Rc<RefCell<dyn IFile>>) -> Option<Ext2FS>{
        // The Superblock is always located at byte 1024 from the beginning of the volume and is exactly 1024 bytes in length.
        // Source: https://wiki.osdev.org/Ext2#Locating_the_Superblock

        let sb_data: Vec<u8> = backing_dev.borrow().read(1024, core::mem::size_of::<Ext2SuperBlock>())?;
        let sb = Ext2SuperBlock::unpack(sb_data.as_slice().try_into().ok()?).ok()?;

        if sb.inodes_per_block_group < 1 { return None; }

        let mut extended_sb = None;
        if sb.major_version >= 1{
            let extended_sb_data: Vec<u8> = backing_dev.borrow().read(1024+core::mem::size_of::<Ext2SuperBlock>(), core::mem::size_of::<Ext2ExtendedSuperblock>())?;
            extended_sb = Some(Ext2ExtendedSuperblock::unpack(extended_sb_data.as_slice().try_into().ok()?).ok()?);
        }
        Some(Ext2FS{
            backing_device: backing_dev,
            sb: sb,
            extended_sb: extended_sb
        })
    }

    fn read(&self, addr: u32, size: usize) -> Option<Vec<u8>>{
        (*self.backing_device).borrow().read(addr as usize, size)
    }

    fn write(&mut self, addr: u32, data: &[u8]) -> Option<usize> {
        use core::fmt::Write;
        let res = (*self.backing_device).borrow_mut().write(addr as usize, data);

        let written_data = self.read(addr, data.len())?;
        let mut is_sane = true;
        let mut bad_offset = 0;
        for i in 0..data.len(){
            if *written_data.get(i)? != data[i] {
                is_sane = false;
                break;
            }
            bad_offset += 1;
        }
        
        if !is_sane {
            writeln!(UART.lock(), "Sanity check in write failed, written data was not read back, addr: {}, offset in data: {}!", addr, bad_offset).unwrap();
        }
        res
    }

    pub fn read_block(&self, number: u32) -> Option<Vec<u8>> {
        // NOTE: There should be no reason to read the first block, because it is either unused ( 1024-bytes before the superblock ), or it just contains the superblock along with the first 1024-bytes of unused space if block size is > 1024, but other than that it's unused
        // Also 0 is used in block pointers in inodes to denote invalid/unused pointers to blocks
        if number == 0 { return None; }
        self.read(self.get_block_size()*number, self.get_block_size() as usize)  
    }

    pub fn write_block(&mut self, number: u32, data: &[u8]) -> Option<()> {
        assert!(data.len() == self.get_block_size() as usize, "The amount of data to write to a block must be the same as the size of the block!");
        if number == 0 { return None; }
        let bytes_written = self.write(self.get_block_size()*number, data)?;
        if bytes_written < self.get_block_size() as usize { return None; }
        Some(())
    }  
    
    pub fn dealloc_block(&mut self, number: u32) -> Option<()> {
        // TODO: Test to make sure we don't leak blocks
        let block_group_descriptor_index = number/self.sb.blocks_per_block_group;
        let offset_in_block_group = number%self.sb.blocks_per_block_group;
        
        let mut descriptor = self.read_block_group_descriptor(block_group_descriptor_index)?;
        if u32::from(descriptor.unallocated_blocks_in_group) == self.sb.blocks_per_block_group {
            // All blocks in the descriptor are already unallocated
            panic!("Block: {} should exist in block group no. {} in the block group descriptor table, however that descriptor reported no allocated blocks!", number, block_group_descriptor_index);
        }
        let mut bitmap = self.read_block(descriptor.block_addr_for_block_usage_bitmap)?;
        let mut val_to_edit = bitmap[(offset_in_block_group/8) as usize];
        // create a mask of all ones except a zero at the location of the block to deallocate, by anding this mask with the current value we mark the block as deallocated while leaving other blocks in the same state
        val_to_edit &= !(1u8<<(offset_in_block_group%8)); 
        bitmap[(offset_in_block_group/8) as usize] = val_to_edit;
        // Write modified bitmap
        self.write_block(descriptor.block_addr_for_block_usage_bitmap,&bitmap);
        descriptor.unallocated_blocks_in_group += 1;
        // Write modified descriptor
        self.write_block_group_descriptor(block_group_descriptor_index, &mut descriptor)?;
        Some(())
    }

    pub fn alloc_block(&mut self, block_group_descriptor_index: u32) -> Option<u32> {
        let mut descriptor =  self.read_block_group_descriptor(block_group_descriptor_index)?;
        if descriptor.unallocated_blocks_in_group == 0 { return None; }

        let mut bitmap = self.read_block(descriptor.block_addr_for_block_usage_bitmap)?;
        let found = bitmap.iter().cloned().enumerate().find(|(_, val)| *val != 0xff)?;
        let mut found_bit = 0;
        while (found.1 >> found_bit) & 1 != 0 {found_bit += 1;}
        let free_block_in_blockgroup =   found.0*8+found_bit;

        let block_index_to_allocate = free_block_in_blockgroup as u32 + block_group_descriptor_index*self.sb.blocks_per_block_group;
        self.write_block(block_index_to_allocate as u32, &vec![0; self.get_block_size() as usize])?;
        bitmap[found.0] |= 1 << found_bit; // Mark as allocated
        self.write_block(descriptor.block_addr_for_block_usage_bitmap, &bitmap)?; // Update bitmap
        descriptor.unallocated_blocks_in_group -= 1;
        self.write_block_group_descriptor(block_group_descriptor_index, &mut descriptor)?; // Update descriptor

        return Some(block_index_to_allocate);
    }

    pub fn read_inode(&self, inode_addr: u32) -> Option<Ext2RawInode> {
        // Inode indexing starts at 1
        let block_group_descriptor_index = (inode_addr-1)/self.sb.inodes_per_block_group;
        let block_group_descriptor = self.read_block_group_descriptor(block_group_descriptor_index)?;
        let inode_table_addr = block_group_descriptor.block_addr_for_inode_table*self.get_block_size();
        let inode_index_in_table = (inode_addr-1)%self.sb.inodes_per_block_group;
        // Inode size in list is self.get_inode_size() but only core::mem::size_of::<Ext2Inode>() bytes of the entire thing are useful for us
        let raw_inode = self.read(inode_table_addr+inode_index_in_table*self.get_inode_size() as u32, core::mem::size_of::<Ext2RawInode>())?;
        Ext2RawInode::unpack(raw_inode.as_slice().try_into().ok()?).ok()
    }

    pub fn write_inode(&mut self, inode_addr: u32, raw_inode: &Ext2RawInode) -> Option<()> {
        // Inode indexing starts at 1
        let block_group_descriptor_index = (inode_addr-1)/self.sb.inodes_per_block_group;
        let block_group_descriptor = self.read_block_group_descriptor(block_group_descriptor_index)?;
        let inode_table_addr = block_group_descriptor.block_addr_for_inode_table*self.get_block_size();
        let inode_index_in_table = (inode_addr-1)%self.sb.inodes_per_block_group;
        // Inode size in list is self.get_inode_size() but only core::mem::size_of::<Ext2Inode>() bytes of the entire thing are useful for us
        self.write(inode_table_addr+inode_index_in_table*self.get_inode_size() as u32, &raw_inode.pack().ok()?)?;
        Some(())
    }

    pub fn read_block_group_descriptor(&self, block_group_descriptor_index: u32) -> Option<Ext2BlockGroupDescriptor> {
        let offset_of_descriptor_in_table = block_group_descriptor_index * Self::get_block_group_descriptor_size() as u32;

        // The block group descriptor table is located in the block immediately following the Superblock.
        // Source: https://wiki.osdev.org/Ext2#Block_Group_Descriptor_Table

        // The Superblock is always located at byte 1024 from the beginning of the volume and is exactly 1024 bytes in length.
        // Source: https://wiki.osdev.org/Ext2#Locating_the_Superblock

        let table_addr = ((1024 + 1024)/self.get_block_size())*self.get_block_size(); // Find the block that's at 2048 bytes ( a.k.a immediatly after the superblock which is 1024 bytes in length and located AT byte 1024, so the first byte of the superblock is byte number 1024 and the last is 2048 )
        let raw_descriptor: Vec<u8> = self.read(table_addr+offset_of_descriptor_in_table, core::mem::size_of::<Ext2BlockGroupDescriptor>())?;
        Ext2BlockGroupDescriptor::unpack(raw_descriptor.as_slice().try_into().ok()?).ok()
    }

    pub fn write_block_group_descriptor(&mut self, block_group_descriptor_index: u32, descriptor: &mut Ext2BlockGroupDescriptor) -> Option<()> {
        let offset_of_descriptor_in_table = block_group_descriptor_index * Self::get_block_group_descriptor_size() as u32;

        // The block group descriptor table is located in the block immediately following the Superblock.
        // Source: https://wiki.osdev.org/Ext2#Block_Group_Descriptor_Table

        // The Superblock is always located at byte 1024 from the beginning of the volume and is exactly 1024 bytes in length.
        // Source: https://wiki.osdev.org/Ext2#Locating_the_Superblock

        let table_addr = ((1024 + 1024)/self.get_block_size())*self.get_block_size(); // Find the byte-address of the block that's 2048 bytes (a.k.a immediatly after the superblock which is 1024 bytes in length and located AT byte 1024)
        self.write(table_addr+offset_of_descriptor_in_table, &descriptor.pack().ok()?)?;
        Some(())
    }

    pub fn get_block_size(&self) -> u32 {
        2u32.pow(self.sb.block_size_log2_minus_10+10)
    }

    // These are used for indexing in arrays instead of core::mem::size_of, so that if rust decides to add padding it doesn't mess up array indexing
    pub fn get_directory_entry_header_size() -> usize {
        8
    }

    pub fn get_block_group_descriptor_size() -> usize {
        32
    }

    pub fn get_inode_size(&self) -> usize {
        // Inodes have a fixed size of either 128 for major version 0 Ext2 file systems, or as dictated by the field in the Superblock for major version 1 file systems
        // Source: https://wiki.osdev.org/Ext2#Inodes
        if self.sb.major_version >= 1{
            self.extended_sb.as_ref().expect("Extended superblock should exist when ext2 major version >= 1!").inode_size.into()
        }else{
            128
        }
    }
}