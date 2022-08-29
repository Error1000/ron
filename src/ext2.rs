use core::{cell::RefCell, str::from_utf8, convert::TryInto, mem::size_of};

use alloc::{rc::Rc, vec, vec::Vec, borrow::ToOwned};
use packed_struct::prelude::PackedStruct;

use crate::{vfs::{IFile, self, IFolder}, UART};

#[derive(PackedStruct, Debug)]
#[packed_struct(endian = "lsb")] // ext2 is little endian (https://wiki.osdev.org/Ext2#Basic_Concepts)
pub struct Ext2SuperBlock {
    max_no_of_inodes: u32,
    max_no_of_blocks: u32,
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

impl Ext2ExtendedSuperblock{
    fn has_unrecognised_required_features(&self) -> bool {
        self.required_features & 0x000F != self.required_features
    }

    fn has_unrecognised_write_required_features(&self) -> bool {
        self.write_features & 0x0007 != self.write_features
    }



    fn has_required_feature_compression(&self) -> bool {
        self.required_features & 0x0001 != 0
    }

    fn has_required_feature_directory_entry_type_field(&self) -> bool {
        self.required_features & 0x0002 != 0
    }

    fn has_required_feature_replay_journal(&self) -> bool {
        self.required_features & 0x0004 != 0
    }

    fn has_required_feature_journal_device(&self) -> bool {
        self.required_features & 0x0008 != 0
    }



    fn has_write_required_feature_sparse(&self) -> bool {
        self.write_features & 0x0001 != 0
    }

    fn has_write_required_feature_64bit_file_size(&self) -> bool {
        self.write_features & 0x0002 != 0
    }

    fn has_write_required_feature_directory_contents_binary_tree(&self) -> bool {
        self.write_features & 0x0004 != 0
    }
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

impl Default for Ext2RawInode {
    fn default() -> Self {
        Self { 
            type_and_perm: 0, 
            user_id: 0, 
            low32_size: 0, 
            last_access_unix_timestamp: 0, 
            creation_unix_timestamp: 0, 
            last_modif_unix_timestamp: 0, 
            deletion_unix_timestamp: 0, 
            group_id: 0, 
            hard_links_to_inode: 0, 
            disk_sectors_used: 0, 
            flags: 0, 
            os_value_1: 0, 
            direct_block_pointers: [0; 12], 
            singly_indirect_block_pointer: 0, 
            doubly_indirect_block_pointer: 0, 
            triply_indirect_block_pointer: 0, 
            generation_number: 0, 
            ext2_majorv1_extended_attribute_block: 0, 
            ext2_majorv1_upper32_size: 0, 
            block_addr_of_fragment: 0, 
            os_value_2: [0; 3]
        }
    }
}

#[derive(PackedStruct, Debug)]
#[packed_struct(endian = "lsb")] // ext2 is little endian (https://wiki.osdev.org/Ext2#Basic_Concepts)
pub struct Ext2DirectoryEntryHeader {
    inode_addr: u32,
    entry_size: u16,
    name_length_low8: u8,
    entry_type: u8
}

impl Default for Ext2DirectoryEntryHeader {
    fn default() -> Self {
        Self { 
            inode_addr: 0, 
            entry_size: Ext2FS::get_directory_entry_header_size() as u16, 
            name_length_low8: 0, 
            entry_type: 0 
        }
    }
}


impl Ext2RawInode {

    fn read_value_from_u32_array_as_le_bytes(bytes: &[u8], index: usize) -> Option<u32> {
        Some(u32::from_le_bytes(bytes[index*size_of::<u32>()..(index+1)*size_of::<u32>()].try_into().ok()?))
    }

    fn write_value_to_u32_array_as_le_bytes(bytes: &mut [u8], index: usize, val: u32) -> Option<()>{
        let bytes_to_write = u32::to_le_bytes(val);
        let mut iter = bytes_to_write.iter();
        for byte_ind in index*size_of::<u32>()..(index+1)*size_of::<u32>(){
            bytes[byte_ind] = *iter.next()?;
        }
        Some(())
    }

    // Last block meaning the block after the last full block
    // Note: If there are no allocated blocks the function returns None
    fn get_last_allocated_data_block_number(&self, fs: &Ext2FS) -> Option<usize> {
        if self.get_size() == 0 { return None; }
        // Indexing starts at 0
        Some((self.get_size()-1) / fs.get_block_size() as usize)
    }


    // Resturns pointer to data block #block_number
    // This allows treating the hierarchical underlying structure as a flat structure
    fn read_data_block_pointer(&self, mut data_block_number: usize, fs: &Ext2FS) -> Option<u32> {
        // TODO: Test all posibilites of this function!!!
        // Direct data
        if data_block_number <= 11 {
            return Some(self.direct_block_pointers[data_block_number]);
        }

        let pointers_per_block = fs.get_block_size() as usize/core::mem::size_of::<u32>();

        let read_pointer_from_block = Self::read_value_from_u32_array_as_le_bytes;

        // Singly indirect data
        data_block_number -= 12;
        if data_block_number < pointers_per_block {
            let singly_indirect_block_index = data_block_number; // Index of pointer to data block
            let mut singly_indirect_block = fs.read_block(self.singly_indirect_block_pointer)?;

            return read_pointer_from_block(&mut singly_indirect_block, singly_indirect_block_index);
        }

        // Doubly indirect data
        data_block_number -= pointers_per_block;
        if data_block_number < pointers_per_block*pointers_per_block{
            let doubly_indirect_block_index = data_block_number/pointers_per_block; // Index of pointer to singly indirect block
            let singly_indirect_block_index = data_block_number%pointers_per_block; // Index of pointer to data block

            let doubly_indirect_block = fs.read_block(self.doubly_indirect_block_pointer)?;
            let mut singly_indirect_block = fs.read_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?)?;
            return read_pointer_from_block(&mut singly_indirect_block, singly_indirect_block_index);
        }

        // Triply indirect data
        data_block_number -= pointers_per_block*pointers_per_block;
        if data_block_number < pointers_per_block*pointers_per_block*pointers_per_block{
            let triply_indirect_block_index = data_block_number/(pointers_per_block*pointers_per_block); // Index of pointer to doubly indirect block
            let doubly_indirect_block_index = (data_block_number%(pointers_per_block*pointers_per_block))/pointers_per_block; // Index of pointer to singly indirect data block
            let singly_indirect_block_index = (data_block_number%(pointers_per_block*pointers_per_block))%pointers_per_block; // Index of pointer to data block

            let triply_indirect_block = fs.read_block(self.triply_indirect_block_pointer)?;
            let doubly_indirect_block = fs.read_block(read_pointer_from_block(&triply_indirect_block, triply_indirect_block_index)?)?;
            let mut singly_indirect_block = fs.read_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?)?;
            return read_pointer_from_block(&mut singly_indirect_block, singly_indirect_block_index);
        }
        None
    }


    // Writes pointer over the pointer pointing to #block_number in the hierarchichal data structure
    // NOTE: Will deallocate block to avoid data leaks
    fn write_data_block_pointer(&mut self, mut data_block_number: usize, pointer: u32, fs: &mut Ext2FS) -> Option<()> {
        // TODO: Test all posibilites of this function!!!
        // Direct data
        if data_block_number <= 11 {
            // Deallocate data block if it is allocated, so ignore the result of deallocation
            fs.dealloc_block(self.direct_block_pointers[data_block_number] as u32);
            self.direct_block_pointers[data_block_number] = pointer;
            return Some(());
        }

        let pointers_per_block = fs.get_block_size() as usize/core::mem::size_of::<u32>();

        let read_pointer_from_block = Self::read_value_from_u32_array_as_le_bytes;
        let write_pointer_to_block = Self::write_value_to_u32_array_as_le_bytes;

        // Singly indirect data
        data_block_number -= 12;
        if data_block_number < pointers_per_block {
            let singly_indirect_block_index = data_block_number; // Index of pointer to data block
            let mut singly_indirect_block = fs.read_block(self.singly_indirect_block_pointer)?;

            // Deallocate data block if it is allocated, so ignore the result of deallocation
            fs.dealloc_block(read_pointer_from_block(&mut singly_indirect_block, singly_indirect_block_index)?);
            // Override with null data block
            write_pointer_to_block(&mut singly_indirect_block, singly_indirect_block_index, pointer)?;
            fs.write_block(self.singly_indirect_block_pointer, &singly_indirect_block)?;
            return Some(());
        }

        // Doubly indirect data
        data_block_number -= pointers_per_block;
        if data_block_number < pointers_per_block*pointers_per_block{
            let doubly_indirect_block_index = data_block_number/pointers_per_block; // Index of pointer to singly indirect block
            let singly_indirect_block_index = data_block_number%pointers_per_block; // Index of pointer to data block

            let doubly_indirect_block = fs.read_block(self.doubly_indirect_block_pointer)?;
            let mut singly_indirect_block = fs.read_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?)?;
            // Deallocate data block if it is allocated, so ignore the result of deallocation
            fs.dealloc_block(read_pointer_from_block(&singly_indirect_block, singly_indirect_block_index)?);
            // Override with null data block
            write_pointer_to_block(&mut singly_indirect_block, singly_indirect_block_index, pointer)?;
            fs.write_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?, &singly_indirect_block)?;
            return Some(());
        }

        // Triply indirect data
        data_block_number -= pointers_per_block*pointers_per_block;
        if data_block_number < pointers_per_block*pointers_per_block*pointers_per_block{
            let triply_indirect_block_index = data_block_number/(pointers_per_block*pointers_per_block); // Index of pointer to doubly indirect block
            let doubly_indirect_block_index = (data_block_number%(pointers_per_block*pointers_per_block))/pointers_per_block; // Index of pointer to singly indirect data block
            let singly_indirect_block_index = (data_block_number%(pointers_per_block*pointers_per_block))%pointers_per_block; // Index of pointer to data block

            let triply_indirect_block = fs.read_block(self.triply_indirect_block_pointer)?;
            let doubly_indirect_block = fs.read_block(read_pointer_from_block(&triply_indirect_block, triply_indirect_block_index)?)?;
            let mut singly_indirect_block = fs.read_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?)?;
            // Deallocate data block if it is allocated, so ignore the result of deallocation
            fs.dealloc_block(read_pointer_from_block(&singly_indirect_block, singly_indirect_block_index)?);
            // Override with null data block
            write_pointer_to_block(&mut singly_indirect_block, singly_indirect_block_index, pointer)?;
            fs.write_block(read_pointer_from_block(&doubly_indirect_block, doubly_indirect_block_index)?, &singly_indirect_block)?;
            return Some(());
        }
        None
    }



    
    pub fn read_data_block(&self, data_block_number: usize, fs: &Ext2FS) -> Option<Vec<u8>> {
        return fs.read_block(self.read_data_block_pointer(data_block_number, fs)?);
    }

    pub fn write_data_block(&self, data_block_number: usize, data: &[u8], fs: &mut Ext2FS) -> Option<()> {
        return fs.write_block(self.read_data_block_pointer(data_block_number, fs)?, data);
    }

    pub fn dealloc_data_block(&mut self, data_block_number: usize, fs: &mut Ext2FS) -> Option<()> {
        // NOTE: Write will deallocate on it's own
        self.write_data_block_pointer(data_block_number, 0, fs)
    }

    // NOTE: Will deallocate block if allocated and allocate a new one
    pub fn alloc_data_block(&mut self, data_block_number: usize, fs: &mut Ext2FS) -> Option<()> {
        // Get descriptor of last block in file and try to put new block there, if that fails, try descriptors next to it, until one succeds or all fails
        // If there are no allocated blocks use descriptor 0 and ones next to it.
        let get_appropriate_descriptor_index = || {
            let last_data_block_number = 
                if let Some(val) = self.get_last_allocated_data_block_number(fs) { val } else { return Some(0); };
            
            fs.get_descriptor_index_of_block_number(self.read_data_block_pointer(last_data_block_number, fs)?)
        };
        let descriptor_index = get_appropriate_descriptor_index()?; // Avoid borrowing fs twice
        let new_block_pointer = fs.alloc_block_close_to(descriptor_index)?;
        self.write_data_block_pointer(data_block_number, new_block_pointer, fs)?;
        Some(())
    }



    pub fn shrink_data_structure_to_fit(&mut self, fs: &mut Ext2FS) {
        let read_pointer_from_block = Self::read_value_from_u32_array_as_le_bytes;
        let write_pointer_to_block = Self::write_value_to_u32_array_as_le_bytes;

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
                            // NOTE: This might not support holes in inodes, depending on how holes are implemented
                            break; // If this block is not empty then none of the rest should be since they are sequential
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
                                        // NOTE: This might not support holes in inodes, depending on how holes are implemented
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

    pub fn grow_data_structure_by(&mut self, nblocks: usize, fs: &mut Ext2FS) -> Option<()> {
        use core::convert::TryFrom;
        let mut new_block_size: isize = isize::try_from(self.get_size()/fs.get_block_size() as usize).ok()? + if self.get_size()%fs.get_block_size() as usize != 0 { 1 } else { 0 } + isize::try_from(nblocks).ok()?;
        let write_pointer_to_block = Self::write_value_to_u32_array_as_le_bytes;
       
        new_block_size -= 12;
        if new_block_size <= 0 { return Some(()); }

        // IDK just allocate the indirect blocks next to the data blocks?
        // TODO: Is this really a good approach to this?, idk, i haven't profiled or tested anything jsut guessing
        let get_appropriate_descriptor_index = || {

            let last_data_block_number = if let Some(val) = self.get_last_allocated_data_block_number(fs) { val } else { return Some(0); };
            fs.get_descriptor_index_of_block_number(self.read_data_block_pointer( last_data_block_number, fs)?)
        };

        let descriptor_index = get_appropriate_descriptor_index()?; // Avoid borrowing fs twice


        if self.singly_indirect_block_pointer == 0 {
            self.singly_indirect_block_pointer = fs.alloc_block_close_to(descriptor_index)?;
        }
        new_block_size -= fs.get_block_size() as isize/size_of::<u32>() as isize;
    
        if new_block_size <= 0 { return Some(());}



        if self.doubly_indirect_block_pointer == 0 {
            self.doubly_indirect_block_pointer = fs.alloc_block_close_to(descriptor_index)?;
        }

        let mut doubly_indirect_block = fs.read_block(self.doubly_indirect_block_pointer)?;
        let mut ind = 0;
        while new_block_size > 0 && ind < fs.get_block_size()/size_of::<u32>() as u32 {
            write_pointer_to_block( &mut doubly_indirect_block, ind as usize, fs.alloc_block_close_to(descriptor_index)?)?;
            new_block_size -= fs.get_block_size() as isize/size_of::<u32>() as isize;
            ind += 1;
        }
        fs.write_block(self.doubly_indirect_block_pointer, &doubly_indirect_block)?;
        if new_block_size <= 0 { return Some(()); }



        if self.triply_indirect_block_pointer == 0 {
            self.triply_indirect_block_pointer = fs.alloc_block_close_to(descriptor_index)?;
        }
        
        let mut triply_indirect_block = fs.read_block(self.triply_indirect_block_pointer)?;
        let mut ind = 0;
        while new_block_size > 0 && ind < fs.get_block_size()/size_of::<u32>() as u32 {
            let doubly_indirect_block_pointer = fs.alloc_block_close_to(descriptor_index)?;
            write_pointer_to_block( &mut triply_indirect_block, ind as usize, doubly_indirect_block_pointer)?;
            {
                let mut doubly_indirect_block = fs.read_block(self.doubly_indirect_block_pointer)?;
                let mut j_ind = 0;
                while new_block_size > 0 && j_ind < fs.get_block_size()/size_of::<u32>() as u32 {
                    write_pointer_to_block( &mut doubly_indirect_block, j_ind as usize, fs.alloc_block_close_to(descriptor_index)?)?;
                    new_block_size -= fs.get_block_size() as isize/size_of::<u32>() as isize;
                    j_ind += 1;
                }
                fs.write_block(self.doubly_indirect_block_pointer, &doubly_indirect_block)?;
            }
                        
            ind += 1;
        }
        fs.write_block(self.triply_indirect_block_pointer, &triply_indirect_block)?;
        if new_block_size <= 0 { return Some(()); }
        None
    }




    pub fn read_bytes(&self, offset: usize, len: usize, e2fs: &Ext2FS) -> Option<Vec<u8>> {
        if offset+len > self.get_size() { return None; }
        let starting_block_index = offset/(e2fs.get_block_size() as usize);
        let starting_block_offset = offset%(e2fs.get_block_size() as usize);
        let mut res: Vec<u8> = Vec::with_capacity(len);

        let first_block = self.read_data_block(starting_block_index, e2fs)?;
        for e in &first_block[starting_block_offset..] { res.push(*e); }

        let extra_block = if len%e2fs.get_block_size() as usize != 0 { 1 } else { 0 };
        for block_ind in 1..(len/e2fs.get_block_size() as usize+extra_block) {
            res.append(
                &mut self.read_data_block(starting_block_index+block_ind, e2fs)?
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
        let mut bytes_written = 0;

        let extra_block = if data.len()%e2fs.get_block_size() as usize != 0 { 1 } else { 0 };
        let mut ind = offset_in_starting_block;
        for block_ind in 0..(data.len()/e2fs.get_block_size() as usize+extra_block) {
            // No need to read sectors that we know will be completly overriden
            let mut v = if (block_ind == (data.len()/e2fs.get_block_size() as usize+extra_block-1) && extra_block == 1) || (block_ind == 0 && offset_in_starting_block != 0) {
                self.read_data_block(starting_block_addr+block_ind, e2fs)?
            }else{
                vec![0u8; e2fs.get_block_size() as usize]
            };

            loop {
                if ind >= v.len() { break; }
                let byte_to_write = if let Some(val) = iter.next() { val } else { break; };
                v[ind] = *byte_to_write; 
                ind += 1; 
                bytes_written += 1;
            }
            ind = 0;

            self.write_data_block(starting_block_addr+block_ind, &v, e2fs)?;
        }

        Some(bytes_written)
    }


    pub fn shrink_by(&mut self, nbytes: usize, e2fs: &mut Ext2FS) -> Option<()> {
        if nbytes > self.get_size() { return None; }
        if nbytes == 0 { return Some(()); }
        let mut blocks_to_remove = 0;
        // Calculate blocks to remove
        {
            let mut bytes_to_remove = nbytes;
            // NOTE: get_size() is never 0, because of the initial ifs
            let bytes_used_in_last_nonempty_block = if self.get_size()%e2fs.get_block_size() as usize == 0 { e2fs.get_block_size() as usize } else { self.get_size()%e2fs.get_block_size() as usize };
        
            if bytes_to_remove >= bytes_used_in_last_nonempty_block {
                // Calculation to remove last block
                blocks_to_remove += 1;
                bytes_to_remove -= bytes_used_in_last_nonempty_block;

                // Calculation to remove the rest of the blocks
                blocks_to_remove += bytes_to_remove/e2fs.get_block_size() as usize;

                // bytes_to_remove%e2fs.get_block_size() is the number of bytes
                // that we would need to be removed from the first block, but that first block will also contain 
                // data that we don't want to remove, so we just don't remove that block, and instead 
                // simply set the size so as to ignore those bytes in the first block
            }
        }

        let last_allocated_data_block_number = self.get_last_allocated_data_block_number(e2fs).expect("File must have allocated blocks if shrinking, we checked for it!");
        for i in 0..blocks_to_remove {
            // writeln!(UART.lock(),"Deallocating block: {}", last_allocated_data_block_number-i).unwrap();
            self.dealloc_data_block(last_allocated_data_block_number-i, e2fs)?;
        }
        self.shrink_data_structure_to_fit(e2fs);
        self.set_size(self.get_size()-nbytes);
        Some(())
    }

    pub fn grow_by(&mut self, nbytes: usize, e2fs: &mut Ext2FS) -> Option<()> {
        if nbytes == 0 { return Some(()); }
        let mut blocks_to_add = 0;
        // Calculate blocks to add
        {
            let mut bytes_to_add = nbytes;
            // Here last block means the block after the last completly-full block, or if no blocks are full, then block 0
            let bytes_used_in_last_block = self.get_size()%e2fs.get_block_size() as usize;
            if bytes_used_in_last_block == 0 {
                // Make sure the last block exists
                self.grow_data_structure_by(1, e2fs)?; // In case the last block doesn't exist and it would overflow in a non-allocated part of the inode strucutre, a.k.a if the indirect blocks don't exist
                self.alloc_data_block(self.get_last_allocated_data_block_number(e2fs).map(|last_block_n|last_block_n+1).unwrap_or(0)/* if no blocks allocated, then the last block is the first block, block 0, and it doesn't exist, so this is definetly needed */, e2fs)?;
            }
            let bytes_available_in_last_block = e2fs.get_block_size() as usize - bytes_used_in_last_block;

            if bytes_to_add > bytes_available_in_last_block {
                // Calculation for adding in the bytes from the last block
                bytes_to_add -= bytes_available_in_last_block;

                // Calculation of how many blocks we actually need
                blocks_to_add += bytes_to_add/e2fs.get_block_size() as usize + if bytes_to_add%e2fs.get_block_size() as usize != 0 { 1 } else { 0 };
            }
        }
        self.grow_data_structure_by(blocks_to_add, e2fs)?;

        let last_allocated_data_block_number = self.get_last_allocated_data_block_number(e2fs).unwrap_or(0)/* block 0 gets allocated above if it doesn't exist, so it's ok to skip it */; 
        for i in 1..=blocks_to_add {
            // writeln!(UART.lock(),"Allocating block: {}", last_allocated_data_block_number+i).unwrap();
            self.alloc_data_block(last_allocated_data_block_number+i, e2fs)?;
        }

        self.set_size(self.get_size()+nbytes);
        Some(())
    }

    pub fn resize(&mut self, new_size: usize, e2fs: &mut Ext2FS) -> Option<()> {
        if new_size == self.get_size() { return Some(()); }
        if new_size < self.get_size() {
            self.shrink_by(self.get_size()-new_size, e2fs)
        }else{
            self.grow_by(new_size-self.get_size(), e2fs)
        }
    }

    pub fn as_vfs_node(self, fs: Rc<RefCell<Ext2FS>>, inode_addr: u32) -> Option<vfs::Node> {
        if self.type_and_perm & 0xF000 == 0x4000 { 
            return Some(vfs::Node::Folder(Rc::new(RefCell::new(Ext2Folder{inode: self, inode_addr, fs})) as Rc<RefCell<dyn IFolder>>));
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
        // FIXME: Can the size of a disk sector in ext2 ever be anything else than 512 bytes?
        self.disk_sectors_used = new_size as u32/512 + if new_size as u32%512 != 0 { 1 } else { 0 };
    }

}

pub struct Ext2File {
    inode: Ext2RawInode,
    inode_addr: u32,
    fs: Rc<RefCell<Ext2FS>>,
}

impl vfs::IFile for Ext2File {
    fn read(&self, offset: u64, len: usize) -> Option<Vec<u8>> {
        self.inode.read_bytes(offset as usize, len, &*self.fs.borrow())
    }

    fn write(&mut self, offset: u64, data: &[u8]) -> Option<usize> {
        self.inode.write_bytes(offset as usize, data, &mut *self.fs.borrow_mut())
    }

    fn get_size(&self) -> u64 {
        self.inode.get_size() as u64
    }

    fn resize(&mut self, new_size: u64) -> Option<()>{
        self.inode.resize(new_size as usize, &mut *self.fs.borrow_mut())?;
        self.fs.borrow_mut().write_inode(self.inode_addr, &self.inode)?;
        Some(())
    }
}

pub struct Ext2Folder {
    inode: Ext2RawInode, 
    inode_addr: u32,
    fs: Rc<RefCell<Ext2FS>>
}

impl Ext2Folder {
    fn get_entries(&self) -> Vec<(usize, Ext2DirectoryEntryHeader, alloc::string::String)> {
        let raw_data = self.inode.read_bytes(0, self.inode.get_size() as usize, &*self.fs.borrow());
        let raw_data = if let Some(val) = raw_data { val } else { return Vec::new(); };
        let mut cur_ind = 0;

        let mut res = Vec::new();
        while cur_ind < raw_data.len() {
            let start_ind = cur_ind;
            let entry = Ext2DirectoryEntryHeader::unpack(raw_data[cur_ind..cur_ind+core::mem::size_of::<Ext2DirectoryEntryHeader>()].try_into().expect("Reading directory entry should always work!")).expect("Parsing directory entry should always work!");
            cur_ind += Ext2FS::get_directory_entry_header_size();
        
            if entry.inode_addr == 0 {
                // Entries with inode addr 0 are supposed to be skipped
                // Source: https://www.nongnu.org/ext2-doc/ext2.html#linked-directory-entry-structure
                cur_ind += entry.entry_size as usize-Ext2FS::get_directory_entry_header_size();
                continue;
            }

            let name: &str = from_utf8(&raw_data[cur_ind..cur_ind+entry.name_length_low8 as usize]).expect("Ext2 inode name in directory entry should be valid utf-8!");
            cur_ind += entry.entry_size as usize-Ext2FS::get_directory_entry_header_size();
            res.push((start_ind, entry, name.to_owned()))   
        }  
        res
    }

    fn write_entry_header_to_buffer(&mut self, raw_data: &mut [u8], entry: &(usize, Ext2DirectoryEntryHeader, alloc::string::String)) -> Option<()> {
        let mut indx = entry.0;
        for byte in entry.1.pack().ok()? {
            raw_data[indx] = byte;
            indx += 1;
        }
        Some(())
    }

    fn write_entry_string_to_buffer(&mut self, raw_data: &mut [u8], entry: &(usize, Ext2DirectoryEntryHeader, alloc::string::String)) -> Option<()> {
        let mut indx = entry.0+Ext2FS::get_directory_entry_header_size();
        for byte in entry.2.bytes() {
            raw_data[indx] = byte;
            indx += 1;
        }
        Some(())
    }

}


impl IFolder for Ext2Folder {
    fn get_children(&self) -> Vec<(alloc::string::String, vfs::Node)> {
        self.get_entries().into_iter().map(|(_, entry, name)|{
            let child_inode = self.fs.borrow().read_inode(entry.inode_addr).expect("Inode in directory should be readable!");
            (name, child_inode.as_vfs_node(self.fs.clone(), entry.inode_addr).expect("Inodes should be parsable as vfs nodes!"))
        }).collect()
    }

    fn unlink_or_delete_empty_child(&mut self, child_name: &str) -> Option<()> {
        let mut child = None; 
        let mut last = None;
        for e in self.get_entries() {
            if e.2 == child_name {
                child = Some(e);
                break;
            }
            last = Some(e);
        }
        let child = child?;
        let mut last = last?;

        { 
            // Update inode that is being unlinked/deleted
            let mut child_inode = self.fs.borrow().read_inode(child.1.inode_addr).expect("Inode in directory entry should be valid!");

            if child_inode.hard_links_to_inode >= 1 {
                child_inode.hard_links_to_inode -= 1; 

                // If inode is no longer hard linked to fs then try to fully deallocate it
                if child_inode.hard_links_to_inode == 0 {
                    if child_inode.get_size() != 0 { 
                        child_inode.hard_links_to_inode = 1;
                        return None; 
                    }

                    self.fs.borrow_mut().dealloc_inode(child.1.inode_addr)?;
                }
            }

            // NOTE: Technically this is unecessary and kind of wierd if we just deallocated the inode, because then we don't need to update the inode since it's deallocated, but it makes the logic simpler to understand
            self.fs.borrow_mut().write_inode(child.1.inode_addr, &child_inode)?;
        }

        // Delete entry, by updating last entry to point past this entry
        // FIXME: This "leaks" the entry currently, though it is possible to clean it up later

        last.1.entry_size += child.1.entry_size;


        let mut raw_data = self.inode.read_bytes(0, self.inode.get_size() as usize, &*self.fs.borrow())?;
        
        // Write updated last entry to raw data
        self.write_entry_header_to_buffer(&mut raw_data, &last);


        // Update directory entries
        // NOTE: No need to change(shrink) inode(directory) size, so no need to update inode(directory), since we just "leak" the entry the size of the inode shouldn't change
        assert!(self.inode.get_size() == raw_data.len());
        if self.inode.write_bytes(0, &raw_data, &mut *self.fs.borrow_mut())? != raw_data.len() { return None; }

        Some(())
    }

    fn create_empty_child(&mut self, name: &str, typ: vfs::NodeType) -> Option<vfs::Node> {
        // First create the inode
        //---------------------------------

        let mut new_child = Ext2RawInode::default();
        let mut entries = self.get_entries();
        let last_entry: &mut (usize, Ext2DirectoryEntryHeader, alloc::string::String) = entries.last_mut()?;

        let get_appropriate_descriptor_index = || -> Option<u32> {
            Some(Ext2FS::get_descriptor_index_of_inode_addr(&self.fs.borrow(), last_entry.1.inode_addr))
        };
        let descriptor_index = get_appropriate_descriptor_index().unwrap_or(0); // Avoid borrowing fs twice
        let new_child_inode_addr = self.fs.borrow_mut().alloc_inode_close_to(descriptor_index)?;

        new_child.type_and_perm = match typ {
            // FIXME: For now, since we don't deal with permissions, we just create an inode with all permissions
            vfs::NodeType::File => 0x8000 | 0x1FF,
            vfs::NodeType::Folder => 0x4000 | 0x1FF,
        };

        new_child.hard_links_to_inode = 1;

        self.fs.borrow_mut().write_inode(new_child_inode_addr, &new_child)?;
        
        // We don't need to mutate new_child anymore, and name is only ever used as bytes from here on
        let new_child = new_child;

        // Then create a new directory entry
        //-----------------------------------

        let mut raw_data = self.inode.read_bytes(0, self.inode.get_size() as usize, &*self.fs.borrow())?;

        let mut new_entry_header = {
            let mut entry_type = 0;
            if let Some(esb) = &self.fs.borrow().extended_sb {
                if esb.has_required_feature_directory_entry_type_field() {
                    entry_type = match typ {
                        vfs::NodeType::File => 1,
                        vfs::NodeType::Folder => 2,
                    };
                }
            }

            Ext2DirectoryEntryHeader{
                inode_addr: new_child_inode_addr,
                entry_size: name.len() as u16 + Ext2FS::get_directory_entry_header_size() as u16,
                name_length_low8: name.len() as u8,
                entry_type
            }
        };

        let new_entry_first_byte: usize = {
            // Test to see if the entry could fit in the free space of the last entry in the list
            // And if so shrink the last entry and put the new entry there, otherwise put the new entry after the last entry
            // So the new entry will always become the new last entry

            let mut actual_space_used_by_last_entry = Ext2FS::get_directory_entry_header_size() + usize::from(last_entry.1.name_length_low8);
            // Comply with the requirement that entries must be 4-byte aligned when calculating if there is enough free space and when updating the size of the last entry if there is enough free space
            // https://www.nongnu.org/ext2-doc/ext2.html#directory
            if actual_space_used_by_last_entry % 4 != 0 {
                actual_space_used_by_last_entry += 4 - (actual_space_used_by_last_entry % 4);
            }

            let free_space_in_last_entry = last_entry.1.entry_size as usize - actual_space_used_by_last_entry;

            if free_space_in_last_entry >= new_entry_header.entry_size as usize {
                // Shrink the last entry
                last_entry.1.entry_size = actual_space_used_by_last_entry as u16;

                // Write the updated last entry header to buffer
                self.write_entry_header_to_buffer(&mut raw_data, last_entry)?;
            }

            // This is fine since the last entry is either pointing to the end of the block, so the new entry will NOT
            // span a block boundry and it will be 4-byte aligned
            // Or we just shrunk it because the new entry would fit in the current block, and since we shrunk it to a multiple of 4, 
            // the new entry will be 4-byte aligned and NOT span a block boundry
            last_entry.0 + usize::from(last_entry.1.entry_size)
        };

        // Grow new entry to the end of the current block
        let location_of_new_entry_end_in_block = (new_entry_first_byte+new_entry_header.entry_size as usize)%(self.fs.borrow().get_block_size() as usize);
        // Note location_of_new_entry_end_in_block points one past the end of the entry, because new_entry_first_byte+new_entry.entry_size points one past the end of the entry
        // This is correct, since if the last byte is byte 0 of the current block, then we only want to grow by 1023 bytes, but 1024-0 = 1024, but 1024-1 = 1023, 
        // so location_of_new_entry_end_in_block pointing one past the end is correct
        let space_to_grow_by = self.fs.borrow().get_block_size() as usize - location_of_new_entry_end_in_block;
        new_entry_header.entry_size += space_to_grow_by as u16;



        // Then write the new entry to disk
        //----------------------------------

        raw_data.resize(new_entry_first_byte+usize::from(new_entry_header.entry_size), 0);

        // Resize inode(directory) to fit new entry
        self.inode.resize(new_entry_first_byte+usize::from(new_entry_header.entry_size), &mut *self.fs.borrow_mut())?;

        // Update inode(directory), to update its size
        self.fs.borrow_mut().write_inode(self.inode_addr, &self.inode)?;


        // Write new entry
        let new_entry = (new_entry_first_byte, new_entry_header, name.to_owned());
        self.write_entry_header_to_buffer(&mut raw_data, &new_entry)?;
        self.write_entry_string_to_buffer(&mut raw_data, &new_entry)?;
        
        // Update directory entries
        assert!(self.inode.get_size() == raw_data.len());
        self.inode.write_bytes(0, &raw_data, &mut *self.fs.borrow_mut())?;

        Some(new_child.as_vfs_node(self.fs.clone(), new_child_inode_addr).expect("New child inode should be valid!"))
    }

}

pub struct Ext2FS {
    backing_device: Rc<RefCell<dyn IFile>>,
    pub sb: Ext2SuperBlock,
    pub extended_sb: Option<Ext2ExtendedSuperblock>,
    read_only: bool
}


impl Ext2FS {
    pub fn new(backing_dev: Rc<RefCell<dyn IFile>>, mut read_only: bool) -> Option<Ext2FS>{
        // The Superblock is always located at byte 1024 from the beginning of the volume and is exactly 1024 bytes in length.
        // Source: https://wiki.osdev.org/Ext2#Locating_the_Superblock

        let sb_data: Vec<u8> = backing_dev.borrow().read(1024, core::mem::size_of::<Ext2SuperBlock>())?;
        let sb = Ext2SuperBlock::unpack(sb_data.as_slice().try_into().ok()?).ok()?;
        if sb.inodes_per_block_group < 1 { return None; }

        let mut extended_sb = None;
        if sb.major_version >= 1{
            let extended_sb_data: Vec<u8> = backing_dev.borrow().read(1024+core::mem::size_of::<Ext2SuperBlock>() as u64, core::mem::size_of::<Ext2ExtendedSuperblock>())?;
            let esb: Ext2ExtendedSuperblock = Ext2ExtendedSuperblock::unpack(extended_sb_data.as_slice().try_into().ok()?).ok()?;
                use core::fmt::Write;

            if esb.has_unrecognised_required_features() {
                writeln!(UART.lock(), "ERROR: Ext2FS has unrecognised required features!").unwrap();
                return None;
            }

            if esb.has_required_feature_compression() {
                writeln!(UART.lock(), "ERROR: Ext2FS has compression, which is not supported!").unwrap();
                return None;
            }

            if esb.has_required_feature_journal_device() {
                writeln!(UART.lock(), "ERROR: Ext2FS has a journal device, which is not supported!").unwrap();
                return None;
            }

            if esb.has_required_feature_replay_journal() {
                writeln!(UART.lock(), "ERROR: Ext2FS requires a journal replay, which is not supported!").unwrap();
                return None;
            }


            
            if esb.has_unrecognised_write_required_features() {
                writeln!(UART.lock(), "WARNING: Ext2FS has unrecognised write-required features, mounting as read-only!").unwrap();
                read_only = true;
            }

            if esb.has_write_required_feature_directory_contents_binary_tree() {
                writeln!(UART.lock(), "WARNING: Ext2FS uses a binary tree to store directory contents, which is not supported, mounting as read-only!").unwrap();
                read_only = true;       
            }

            // NOTE: We support (at least in theory) 64-bit file sizes, directory entry type field and sparse superblocks and group descriptor tables 
            // Actual level of support
            // 64-bit file sizes: full support, but not really tested
            // directory entry type field: full support, but not really tested
            // sparse superblocks and group descriptor tables: lol no support, but i don't think it actually matters unless the filesystem gets corrupted so honestly it's more of an optional feature anyways
            // nonetheless i should probs FIXME add support for sparse superblocks and group descriptor tables, to comply with the spec

            extended_sb = Some(esb);
        }
        Some(Ext2FS{
            backing_device: backing_dev,
            sb: sb,
            extended_sb: extended_sb,
            read_only
        })
    }

    fn read(&self, addr: u32, size: usize) -> Option<Vec<u8>>{
        (*self.backing_device).borrow().read(addr as u64, size)
    }


    // Returns Ok(()) if sanity check passes or Err(None) if it did not and was not able to determine the offending byte
    // or Err(Some(index)) if the sanity check did not pass and it was able to determine the offending byte
    fn sanity_check_write(&self, addr: u32, written_data: &[u8]) -> Result<(), Option<usize>> {
        // Read back the written data
        let read_back_data = self.read(addr, written_data.len()).ok_or(None)?;
        let mut is_sane = true;
        let mut bad_offset = 0;
        // And check that the read data and written data is the same
        for i in 0..written_data.len() {
            if *read_back_data.get(i).ok_or(i)? != written_data[i] {
                bad_offset = i;
                is_sane = false;
                break;
            }
        }

        if !is_sane {
            use core::fmt::Write;
            writeln!(UART.lock(), "Sanity check in write failed, written data was not read back, addr: {}, offset in data: {}!", addr, bad_offset).unwrap();
            return Err(Some(bad_offset));
        }else{
            return Ok(());
        }
    }

    fn write(&mut self, addr: u32, data: &[u8]) -> Option<usize> {
        if self.read_only { return None; }
        let res = (*self.backing_device).borrow_mut().write(addr as u64, data);
        
        #[cfg(debug_assertions)]
        self.sanity_check_write(addr, data).ok()?;
        
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
        if self.sb.unallocated_blocks == self.sb.max_no_of_blocks { return None; }
        // TODO: Test to make sure we don't leak blocks
        let block_group_descriptor_index = self.get_descriptor_index_of_block_number(number)?;
        let offset_in_block_group = self.get_descriptor_subindex_of_block_number(number)?;
        
        let mut descriptor = self.read_block_group_descriptor(block_group_descriptor_index)?;
        if u32::from(descriptor.unallocated_blocks_in_group) == self.sb.blocks_per_block_group {
            // All blocks in the descriptor are already unallocated
            panic!("Block: {} should exist in block group no. {} in the block group descriptor table, however that descriptor reported no allocated blocks!", number, block_group_descriptor_index);
        }
        let mut bitmap = self.read_block(descriptor.block_addr_for_block_usage_bitmap)?;
        let mut val_to_edit = bitmap[(offset_in_block_group/8) as usize];
        // Test if block is already deallocated
        if val_to_edit & (1 << (offset_in_block_group%8)) == 0 { return None; }

        // Create a mask of all ones except a zero at the location of the block to deallocate, by anding this mask with the current value we mark the block as deallocated while leaving other blocks in the same state
        val_to_edit &= !(1 << (offset_in_block_group%8)); 
        bitmap[(offset_in_block_group/8) as usize] = val_to_edit;

        // Update bitmap
        self.write_block(descriptor.block_addr_for_block_usage_bitmap,&bitmap);

        // Update descriptor
        descriptor.unallocated_blocks_in_group += 1;
        self.write_block_group_descriptor(block_group_descriptor_index, &mut descriptor)?;

        // Update superblock
        self.sb.unallocated_blocks += 1;
        self.flush_super_blocks();

        Some(())
    }

    pub fn alloc_block(&mut self, block_group_descriptor_index: u32) -> Option<u32> {
        if self.sb.unallocated_blocks == 0 { return None; }

        let mut descriptor =  self.read_block_group_descriptor(block_group_descriptor_index)?;
        if descriptor.unallocated_blocks_in_group == 0 { return None; }

        let mut bitmap = self.read_block(descriptor.block_addr_for_block_usage_bitmap)?;

        let found_loc_and_byte = bitmap.iter().cloned().enumerate().find(|(_, val)| *val != 0xff)?;
        let mut found_bit = 0;
        while (found_loc_and_byte.1 >> found_bit) & 1 != 0 /* 0 == free */ {found_bit += 1;}
        let free_block_in_blockgroup =   found_loc_and_byte.0*8 + found_bit;

        let block_pointer_to_allocate = free_block_in_blockgroup as u32 + block_group_descriptor_index*self.sb.blocks_per_block_group + self.get_number_of_special_blocks() as u32;

        bitmap[found_loc_and_byte.0] |= 1 << found_bit; // Mark as allocated

        // Update bitmap
        self.write_block(descriptor.block_addr_for_block_usage_bitmap, &bitmap)?; 

        // Update descriptor
        descriptor.unallocated_blocks_in_group -= 1;
        self.write_block_group_descriptor(block_group_descriptor_index, &descriptor)?;

        // Update super block
        self.sb.unallocated_blocks -= 1;
        self.flush_super_blocks();
    
        // Zero out new block
        self.write_block(block_pointer_to_allocate as u32, &vec![0; self.get_block_size() as usize])?;
        
        return Some(block_pointer_to_allocate);
    }

    pub fn alloc_block_close_to(&mut self, mut block_group_descriptor_index: u32) -> Option<u32> {
        let new_block_pointer;
        loop {
            if let Some(ptr) = self.alloc_block(block_group_descriptor_index) {
                new_block_pointer = ptr;
                break; 
            }
            block_group_descriptor_index += 1;
            if block_group_descriptor_index > self.get_number_of_block_groups() { return None; }
        }
        Some(new_block_pointer)
    }

    pub fn read_inode(&self, inode_addr: u32) -> Option<Ext2RawInode> {
        // Inode indexing starts at 1
        let block_group_descriptor_index = self.get_descriptor_index_of_inode_addr(inode_addr);
        let block_group_descriptor = self.read_block_group_descriptor(block_group_descriptor_index)?;
        let inode_table_addr = block_group_descriptor.block_addr_for_inode_table*self.get_block_size();
        let inode_index_in_table = self.get_descriptor_subindex_of_inode_addr(inode_addr);
        // Inode size in list is self.get_inode_size() but only core::mem::size_of::<Ext2Inode>() bytes of the entire thing are useful for us
        let raw_inode = self.read(inode_table_addr+inode_index_in_table*self.get_inode_size() as u32, core::mem::size_of::<Ext2RawInode>())?;
        Ext2RawInode::unpack(raw_inode.as_slice().try_into().ok()?).ok()
    }

    pub fn write_inode(&mut self, inode_addr: u32, raw_inode: &Ext2RawInode) -> Option<()> {
        // Inode indexing starts at 1
        let block_group_descriptor_index = self.get_descriptor_index_of_inode_addr(inode_addr);
        let block_group_descriptor = self.read_block_group_descriptor(block_group_descriptor_index)?;
        let inode_table_addr = block_group_descriptor.block_addr_for_inode_table*self.get_block_size();
        let inode_index_in_table = self.get_descriptor_subindex_of_inode_addr(inode_addr);
        // Inode size in list is self.get_inode_size() but only core::mem::size_of::<Ext2Inode>() bytes of the entire thing are useful for us
        self.write(inode_table_addr+inode_index_in_table*self.get_inode_size() as u32, &raw_inode.pack().ok()?)?;
        Some(())
    }

    // WARNING: DOES NOT DEALLOCATE INODE'S DATA, WILL LEAK IF GIVEN THE OPPORTUNITY
    // TODO: This is inconsistent with write_data_block_pointer, which will deallocate data, if needed, instead of leaking it
    pub fn dealloc_inode(&mut self, inode_addr: u32) -> Option<()> {
        if self.sb.unallocated_inodes == self.sb.max_no_of_inodes { return None; }
        let block_group_descriptor_index = self.get_descriptor_index_of_inode_addr(inode_addr);
        let offset_in_block_group = self.get_descriptor_subindex_of_inode_addr(inode_addr);

        let mut descriptor = self.read_block_group_descriptor(block_group_descriptor_index)?;
        // If we have already deallocated all inodes
        if u32::from(descriptor.unallocated_inodes_in_group) == self.sb.inodes_per_block_group {
            return None;
        }

        let mut allocation_bitmap = self.read_block(descriptor.block_addr_for_inode_usage_bitmap)?;
        let mut val_to_edit = allocation_bitmap[offset_in_block_group as usize/8];    
        
        // Test if inode is already deallocated
        if val_to_edit & (1 << (offset_in_block_group%8)) == 0 {
            use core::fmt::Write;
            writeln!(UART.lock(), "ERROR: Inode {} is already deallocated according to block group bitmap!", inode_addr).unwrap();
            return None;
        }
    
        // Create a mask of all ones except a zero at the location of the inode to deallocate, by anding this mask with the current value we mark the inode as deallocated while leaving other inodes in the same state
        val_to_edit &= !(1 << (offset_in_block_group%8));
        allocation_bitmap[offset_in_block_group as usize/8] = val_to_edit;
        
        // Update bitmap
        self.write_block(descriptor.block_addr_for_inode_usage_bitmap, &allocation_bitmap)?;
        
        // Update descriptor
        descriptor.unallocated_inodes_in_group += 1;
        self.write_block_group_descriptor(block_group_descriptor_index, &descriptor)?;

        // Update superblock
        self.sb.unallocated_inodes += 1;
        self.flush_super_blocks();

        Some(())
    }

    pub fn alloc_inode(&mut self, block_group_descriptor_index:  u32) -> Option<u32> {
        if self.sb.unallocated_inodes == 0 { return None; }
        
        let mut descriptor = self.read_block_group_descriptor(block_group_descriptor_index)?;
        // If we have already allocated all of the inodes
        if descriptor.unallocated_inodes_in_group == 0 { return None; }

        let mut allocation_bitmap = self.read_block(descriptor.block_addr_for_inode_usage_bitmap)?;

        let found_loc_and_byte = allocation_bitmap.iter().cloned().enumerate().find(|(_, val)| *val != 0xff)?;
        let mut found_bit = 0;
        while (found_loc_and_byte.1 >> found_bit) & 1 != 0 /* 0 == free */ {found_bit += 1;}

        let free_inode_index_in_table = found_loc_and_byte.0*8+found_bit;
        let inode_addr_to_allocate = free_inode_index_in_table as u32 + self.sb.inodes_per_block_group as u32*block_group_descriptor_index + 1;

        if inode_addr_to_allocate == 1 {
            // Maybe don't
            return None;
        }
        allocation_bitmap[found_loc_and_byte.0] |= 1 << found_bit; // Mark as allocated

        // Update bitmap
        self.write_block(descriptor.block_addr_for_inode_usage_bitmap, &allocation_bitmap)?; 

        // Update descriptor
        descriptor.unallocated_inodes_in_group -= 1;
        self.write_block_group_descriptor(block_group_descriptor_index, &descriptor)?;
        
        // Update superblock
        self.sb.unallocated_inodes -= 1;
        self.flush_super_blocks();

        // Zero out new inode
        self.write_inode(inode_addr_to_allocate, &Ext2RawInode::default())?;

        Some(inode_addr_to_allocate)
    }

    pub fn alloc_inode_close_to(&mut self, mut block_group_descriptor_index: u32) -> Option<u32> {
        let new_inode_addr;
        loop {
            if let Some(addr) = self.alloc_inode(block_group_descriptor_index) {
                new_inode_addr = addr;
                break; 
            }
            block_group_descriptor_index += 1;
            if block_group_descriptor_index > self.get_number_of_block_groups() { return None; }
        }
        Some(new_inode_addr)
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

    pub fn write_block_group_descriptor(&mut self, block_group_descriptor_index: u32, descriptor: &Ext2BlockGroupDescriptor) -> Option<()> {
        let offset_of_descriptor_in_table = block_group_descriptor_index * Self::get_block_group_descriptor_size() as u32;

        // The block group descriptor table is located in the block immediately following the Superblock.
        // Source: https://wiki.osdev.org/Ext2#Block_Group_Descriptor_Table

        // The Superblock is always located at byte 1024 from the beginning of the volume and is exactly 1024 bytes in length.
        // Source: https://wiki.osdev.org/Ext2#Locating_the_Superblock

        let table_addr = ((1024 + 1024)/self.get_block_size())*self.get_block_size(); // Find the byte-address of the block that's 2048 bytes (a.k.a immediatly after the superblock which is 1024 bytes in length and located AT byte 1024)
        self.write(table_addr+offset_of_descriptor_in_table, &descriptor.pack().ok()?)?;
        Some(())
    }

    pub fn flush_super_blocks(&mut self) -> Option<()> {
        // Update superblock
        self.write(1024, &self.sb.pack().ok()?)?; 
                
        // Update extended superblock
        if let Some(esb) = &self.extended_sb{
            self.write(1024+Self::get_super_block_size() as u32, &esb.pack().ok()?)?; 
        }
        Some(())
    }
    
    // Maps block numbers and inode addresses to block groups indecies and offsets(subindicies)
    pub fn get_descriptor_index_of_block_number(&self, block_number: u32) -> Option<u32> {
        if block_number < self.get_number_of_special_blocks() as u32 { return None; }
        Some((block_number-self.get_number_of_special_blocks() as u32)/self.sb.blocks_per_block_group)
    }

    pub fn get_descriptor_subindex_of_block_number(&self, block_number: u32) -> Option<u32> {
        if block_number < self.get_number_of_special_blocks() as u32 { return None; }
        Some((block_number-self.get_number_of_special_blocks() as u32)%self.sb.blocks_per_block_group)
    }

    pub fn get_descriptor_index_of_inode_addr(&self, inode_addr: u32) -> u32 {
        (inode_addr-1)/self.sb.inodes_per_block_group
    }

    pub fn get_descriptor_subindex_of_inode_addr(&self, inode_addr: u32) -> u32 {
        (inode_addr-1)%self.sb.inodes_per_block_group
    }



    fn get_number_of_block_groups(&self) -> u32 {
        assert!(self.sb.max_no_of_blocks/self.sb.blocks_per_block_group + if self.sb.max_no_of_blocks%self.sb.blocks_per_block_group != 0 { 1 } else { 0 } 
            == self.sb.max_no_of_inodes/self.sb.inodes_per_block_group + if self.sb.max_no_of_inodes%self.sb.inodes_per_block_group != 0 { 1 } else { 0 });
        return self.sb.max_no_of_blocks/self.sb.blocks_per_block_group + if self.sb.max_no_of_blocks%self.sb.blocks_per_block_group != 0 { 1 } else { 0 };
    }

    fn get_number_of_special_blocks(&self) -> usize {
        let initial_blocks = (1024/*reserved space*/+1024/*superblock*/)/self.get_block_size() as usize + if (1024+1024)%self.get_block_size() as usize != 0 { 1 } else { 0 };
        
        // Size of the block group table in blocks
        let block_group_table_blocks = Self::get_block_group_descriptor_size() * self.get_number_of_block_groups() as usize/self.get_block_size() as usize + if (Self::get_block_group_descriptor_size() * self.get_number_of_block_groups() as usize)%self.get_block_size() as usize != 0 { 1 } else { 0 };
        return initial_blocks+block_group_table_blocks;
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

    pub fn get_super_block_size() -> usize {
        84
    }

    pub fn get_extended_super_block_size() -> usize {
        1024-Self::get_super_block_size()
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

impl Drop for Ext2FS {
    fn drop(&mut self) {
        // FIXME: Figure out why drop isn't called
        self.flush_super_blocks();
    }
}