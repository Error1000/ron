use core::{convert::TryFrom, cell::RefCell};

use alloc::{rc::Rc, vec::Vec};

use crate::{vfs::IFile, ata};

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum MBRPartitionNumber{
  PART_0 = 0, PART_1 = 1, PART_2 = 2, PART_3 = 3
}

impl TryFrom<usize> for MBRPartitionNumber{
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value{
          0 => Ok(MBRPartitionNumber::PART_0),
          1 => Ok(MBRPartitionNumber::PART_1),
          2 => Ok(MBRPartitionNumber::PART_2),
          3 => Ok(MBRPartitionNumber::PART_3),
          _ => Err(())
        }
    }
}
pub struct MBRPartitionFile{
  device: Rc<RefCell<dyn IFile>>,
  partition_offset: usize,
  partition_size: usize,
  partiton_number: MBRPartitionNumber
}

impl MBRPartitionFile{
  pub fn from(device_file: Rc<RefCell<dyn IFile>>, partition_number: MBRPartitionNumber) -> Option<Self>{
    let part_data_offset = (partition_number as usize)*16 + (0x1fe-16*4);
    if let Some(part_data) = device_file.borrow().read(part_data_offset, 16){
      // If SYSTEM_ID/partition type is 0 then the partition is unused
      if part_data[4] == 0x0 { return None; } 
      Some(Self{
        device: device_file.clone(),
        partition_offset: ((part_data[8] as u32) << 0 | (part_data[9] as u32) << 8 | (part_data[10] as u32) << 16 | (part_data[11] as u32) << 24) as usize * ata::SECTOR_SIZE_IN_BYTES,
        partition_size: ((part_data[12] as u32) << 0 | (part_data[13] as u32) << 8 | (part_data[14] as u32) << 16 | (part_data[15] as u32) << 24) as usize * ata::SECTOR_SIZE_IN_BYTES,
        partiton_number: partition_number,
      })
    }else { None }
  }

  pub fn get_offset(&self) -> usize{
    self.partition_offset
  }
}

impl IFile for MBRPartitionFile{
    fn read(&self, offset: usize, len: usize) -> Option<Vec<u8>> {
      if offset+len >= self.partition_size{ return None; }
      (*self.device).borrow().read(offset+self.partition_offset, len)
    }

    fn write(&mut self, offset: usize, data: &[u8]) {
      if offset+data.len() >= self.partition_size{ return; }
      (*self.device).borrow_mut().write(offset+self.partition_offset, data);
    }

    fn get_size(&self) -> usize {
        self.partition_size
    }
}
