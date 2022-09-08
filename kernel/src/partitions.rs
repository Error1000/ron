use core::{cell::RefCell, convert::TryFrom};

use alloc::{rc::Rc, vec::Vec};

use crate::{ata, vfs::IFile};

pub struct MBRPartitionNumber(u8);
pub mod mbr {
    use super::MBRPartitionNumber;

    pub const PART_0: MBRPartitionNumber = MBRPartitionNumber(0);
    pub const PART_1: MBRPartitionNumber = MBRPartitionNumber(1);
    pub const PART_2: MBRPartitionNumber = MBRPartitionNumber(2);
    pub const PART_3: MBRPartitionNumber = MBRPartitionNumber(3);
}

impl TryFrom<usize> for MBRPartitionNumber {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(mbr::PART_0),
            1 => Ok(mbr::PART_1),
            2 => Ok(mbr::PART_2),
            3 => Ok(mbr::PART_3),
            _ => Err(()),
        }
    }
}
pub struct MBRPartitionFile {
    device: Rc<RefCell<dyn IFile>>,
    partition_offset: u64,
    partition_size: u64,
    partiton_number: MBRPartitionNumber,
}

impl MBRPartitionFile {
    pub fn from(
        device_file: Rc<RefCell<dyn IFile>>,
        partition_number: MBRPartitionNumber,
    ) -> Option<Self> {
        let part_data_offset = u64::from(partition_number.0) * 16 + (0x1fe - 16 * 4);
        if let Some(part_data) = device_file.borrow().read(part_data_offset, 16) {
            // If SYSTEM_ID/partition type is 0 then the partition is unused
            if part_data[4] == 0x0 {
                return None;
            }
            Some(Self {
                device: device_file.clone(),
                partition_offset: u32::from_le_bytes([
                    part_data[8],
                    part_data[9],
                    part_data[10],
                    part_data[11],
                ]) as u64
                    * ata::SECTOR_SIZE_IN_BYTES as u64,
                partition_size: u32::from_le_bytes([
                    part_data[12],
                    part_data[13],
                    part_data[14],
                    part_data[15],
                ]) as u64
                    * ata::SECTOR_SIZE_IN_BYTES as u64,
                partiton_number: partition_number,
            })
        } else {
            None
        }
    }

    pub fn get_offset(&self) -> u64 {
        self.partition_offset
    }
}

impl IFile for MBRPartitionFile {
    fn read(&self, offset: u64, len: usize) -> Option<Vec<u8>> {
        if offset + len as u64 > self.partition_size {
            return None;
        }
        (*self.device)
            .borrow()
            .read(offset + self.partition_offset, len)
    }

    fn write(&mut self, offset: u64, data: &[u8]) -> Option<usize> {
        if offset + data.len() as u64 > self.partition_size {
            return None;
        }
        (*self.device)
            .borrow_mut()
            .write(offset + self.partition_offset, data)
    }

    fn get_size(&self) -> u64 {
        self.partition_size
    }

    fn resize(&mut self, _new_size: u64) -> Option<()> {
        None
    }
}
