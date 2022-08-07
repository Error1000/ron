use core::{cell::RefCell, str::from_utf8, convert::TryInto, mem::size_of};

use alloc::{rc::Rc, vec, vec::Vec, borrow::ToOwned};
use packed_struct::prelude::PackedStruct;

use crate::{vfs::{IFile, self, IFolder}};

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
pub struct Ext2ExtendedSuperblock{
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
pub struct Ext2BlockGroupDescriptor{
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
pub struct Ext2RawInode{
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
    ext2_majorv1_custom: u32,
    block_addr_of_fragment: u32,
    os_value_2: [u32; 3],
}

#[derive(PackedStruct)]
#[packed_struct(endian = "lsb")] // ext2 is little endian (https://wiki.osdev.org/Ext2#Basic_Concepts)
pub struct Ext2DirectoryEntry {
    inode_addr: u32,
    entry_size: u16,
    name_length_low8: u8,
    entry_type: u8
}

impl Ext2RawInode {

    fn get_value_from_u32_array_as_le_bytes(bytes: &[u8], index: usize) -> Option<u32> {
        Some(u32::from_le_bytes(bytes[index*size_of::<u32>()..(index+1)*size_of::<u32>()].try_into().ok()?))
    }

    pub fn get_data_block_pointer(&self, mut block_number: usize, fs: &Ext2FS) -> Option<u32>{
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
            // direct_pointer_block is a block of tightly packed block pointers
            let block_of_direct_pointers = fs.read_block(self.singly_indirect_block_pointer)?;
            return read_pointer_from_block(&block_of_direct_pointers, block_number);
        }

        // Doubly indirect data
        block_number -= pointers_per_block;
        if block_number < pointers_per_block*pointers_per_block{
            let singly_indirect_block_index = block_number/pointers_per_block;
            let direct_block_index = block_number%pointers_per_block;
            let block_of_singly_indirect_pointers = fs.read_block(self.doubly_indirect_block_pointer)?;
            let block_of_direct_pointers = fs.read_block(read_pointer_from_block(&block_of_singly_indirect_pointers, singly_indirect_block_index)?)?;
            return read_pointer_from_block(&block_of_direct_pointers, direct_block_index);
        }

        // Triply indirect data
        block_number -= pointers_per_block*pointers_per_block;
        if block_number < pointers_per_block*pointers_per_block*pointers_per_block{
            let doubly_indirect_block_index = block_number/(pointers_per_block*pointers_per_block);
            let singly_indirect_block_index = (block_number%(pointers_per_block*pointers_per_block))/pointers_per_block;
            let direct_block_index = (block_number%(pointers_per_block*pointers_per_block))%pointers_per_block;
            let block_of_doubly_indirect_pointers = fs.read_block(self.triply_indirect_block_pointer)?;
            let block_of_singly_indirect_pointers = fs.read_block(read_pointer_from_block(&block_of_doubly_indirect_pointers, doubly_indirect_block_index)?)?;
            let block_of_direct_pointers = fs.read_block(read_pointer_from_block(&block_of_singly_indirect_pointers, singly_indirect_block_index)?)?;
            return read_pointer_from_block(&block_of_direct_pointers, direct_block_index);
        }
        None
    }

    pub fn read_raw_data_block(&self, block_number: usize, fs: &Ext2FS) -> Option<Vec<u8>> {
        return fs.read_block(self.get_data_block_pointer(block_number, fs)?);
    }

    pub fn write_raw_data_block(&self, block_number: usize, data: &[u8], fs: &mut Ext2FS) -> Option<()> {
        return fs.write_block(self.get_data_block_pointer(block_number, fs)?, data);
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
            let mut v = if block_ind == (data.len()/e2fs.get_block_size() as usize+extra_block-1) && extra_block == 1 {
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

    pub fn as_vfs_node(self, fs: Rc<RefCell<Ext2FS>>) -> Option<vfs::Node> {
        if self.type_and_perm & 0xF000 == 0x4000 { 
            return Some(vfs::Node::Folder(Rc::new(RefCell::new(Ext2Folder{inode: self, fs})) as Rc<RefCell<dyn IFolder>>));
        }
        if self.type_and_perm & 0xF000 == 0x8000 {
            return Some(vfs::Node::File(Rc::new(RefCell::new(Ext2File{inode: self, fs})) as Rc<RefCell<dyn IFile>>));
        }
        None
    }

    pub fn get_size(&self) -> usize {
        // FIXME: Handle larger files
        self.low32_size as usize
    }
}

pub struct Ext2File {
    inode: Ext2RawInode,
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
                let entry = Ext2DirectoryEntry::unpack(raw_data[cur_ind..cur_ind+size_of::<Ext2DirectoryEntry>()].try_into().expect("Reading directory entry should always work!")).expect("Parsing directory entry should always work!");
                cur_ind += core::mem::size_of::<Ext2DirectoryEntry>();
                let name: &str = from_utf8(&raw_data[cur_ind..cur_ind+entry.name_length_low8 as usize]).expect("Ext2 inode name in directory entry should be valid utf-8!");
                cur_ind += entry.entry_size as usize-core::mem::size_of::<Ext2DirectoryEntry>();

                let inode = self.fs.borrow().get_inode(entry.inode_addr).expect("Inode in directory entry should be valid!");
                res.push((name.to_owned(), inode.as_vfs_node(self.fs.clone()).expect("Inodes should be parsable as vfs nodes!")))
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

    fn write(&mut self, addr: u32, data: &[u8]) {
        (*self.backing_device).borrow_mut().write(addr as usize, data);
    }

    pub fn read_block(&self, number: u32) -> Option<Vec<u8>> {
        // NOTE: There should be no reason to read the first block, because it either unused ( 1024-bytes before the superblock ), or it just contains the superblock, but other than that it's unused
        // Also 0 is used in block pointers in inodes to denote invalid/unused pointers to blocks
        if number == 0 { return None; }
        self.read(self.get_block_size()*number, self.get_block_size() as usize)  
    }

    pub fn write_block(&mut self, number: u32, data: &[u8]) -> Option<()> {
        assert!(data.len() == self.get_block_size() as usize, "The amount of data to write to a block must be the same as the size of the block!");
        if number == 0 { return None; }
        // FIXME: Write should return more about wheter it succeeds or not, right now we just assume it does.
        self.write(self.get_block_size()*number, data);
        Some(())
    }  
    

    pub fn get_inode(&self, inode_addr: u32) -> Option<Ext2RawInode> {
        // Inode indexing starts at 1
        let block_group_descriptor_index = (inode_addr-1)/self.sb.inodes_per_block_group;
        let block_group_descriptor = self.get_block_group_descriptor(block_group_descriptor_index)?;
        let inode_table_addr = block_group_descriptor.block_addr_for_inode_table*self.get_block_size();
        let inode_index_in_table = (inode_addr-1)%self.sb.inodes_per_block_group;
        // Inode size in list is self.get_inode_size() but only core::mem::size_of::<Ext2Inode>() bytes of the entire thing are useful for us
        let raw_inode = self.read(inode_table_addr+inode_index_in_table*self.get_inode_size() as u32, core::mem::size_of::<Ext2RawInode>())?;
        Ext2RawInode::unpack(raw_inode.as_slice().try_into().ok()?).ok()
    }

    pub fn get_block_group_descriptor(&self, block_group_descriptor_index: u32) -> Option<Ext2BlockGroupDescriptor> {
        let offset_of_descriptor_in_table = block_group_descriptor_index * core::mem::size_of::<Ext2BlockGroupDescriptor>() as u32;

        // The block group descriptor table is located in the block immediately following the Superblock.
        // Source: https://wiki.osdev.org/Ext2#Block_Group_Descriptor_Table

        // The Superblock is always located at byte 1024 from the beginning of the volume and is exactly 1024 bytes in length.
        // Source: https://wiki.osdev.org/Ext2#Locating_the_Superblock

        let table_addr = ((1024 + 1024)/self.get_block_size())*self.get_block_size(); // Find the block that's at 2048 bytes ( a.k.a immediatly after the superblock which is 1024 bytes in length and located AT byte 1024, so the first byte of the superblock is byte number 1024 and the last is 2048 )
        let raw_descriptor: Vec<u8> = self.read(table_addr+offset_of_descriptor_in_table, core::mem::size_of::<Ext2BlockGroupDescriptor>())?;
        Ext2BlockGroupDescriptor::unpack(raw_descriptor.as_slice().try_into().ok()?).ok()
    }

    pub fn get_block_size(&self) -> u32 {
        2u32.pow(self.sb.block_size_log2_minus_10+10)
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