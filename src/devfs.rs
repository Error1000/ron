use core::cell::{RefCell};

use alloc::{vec::Vec, string::String, rc::Rc, borrow::ToOwned};

use crate::{ata::{ATABus, ATADevice, self}, vfs::{self, INode, IFile, Node}};



pub struct ATADeviceFile{
    pub bus: Rc<RefCell<ATABus>>,
    pub bus_device: ATADevice
}


impl INode for ATADeviceFile{
    fn get_name(&self) -> String {
      let mut s = String::new();
      use core::fmt::Write;
      write!(s, "disk-{}", match self.bus_device{ ATADevice::MASTER => "master", ATADevice::SLAVE => "slave"}).unwrap();
      s
    }
}

impl IFile for ATADeviceFile{
    fn read(&self, mut offset: usize, len: usize) -> Option<Vec<u8>> {
      let offset_in_sector = offset % ata::SECTOR_SIZE_IN_BYTES;
      offset /= ata::SECTOR_SIZE_IN_BYTES;
      let lba = ata::LBA28{hi: ((offset>>16)&0xFF) as u8, mid: ((offset>>8)&0xFF) as u8, low: (offset&0xFF) as u8};
      unsafe{ self.bus.borrow_mut().read_sector(self.bus_device, lba) }
      .map(|val|{
        let mut v = Vec::with_capacity(len);
        for e in &val[offset_in_sector/core::mem::size_of::<u16>()..(offset_in_sector+len)/core::mem::size_of::<u16>()]{
            v.push(((e>>8)&0xFF) as u8);
            v.push((e&0xFF) as u8);
        }
        v
      })
    }

    fn write(&mut self, mut offset: usize, data: &[u8]) {
       let offset_in_sector = offset & ata::SECTOR_SIZE_IN_BYTES;
       offset /= ata::SECTOR_SIZE_IN_BYTES;
       let lba = ata::LBA28{hi: ((offset>>16)&0xFF) as u8, mid: ((offset>>8)&0xFF) as u8, low: (offset&0xFF) as u8};
       let mut v = unsafe{ self.bus.borrow_mut().read_sector(self.bus_device, lba) }.expect("Reading device should work!");
       let mut i = data.iter();
       let mut ind = 0;
       while let (Some(a), Some(b)) = (i.next(), i.next()){
          v[offset_in_sector+ind] = ((*a as u16) << 8) | (*b as u16);
          ind += 1;
       }
       unsafe{ self.bus.borrow_mut().write_sector(self.bus_device, lba, &v) }
    }

}
pub struct DevFS{
  disk_devices: Vec<Rc<RefCell<dyn IFile>>>
}

impl DevFS{
  pub fn new() -> Self{
    Self{
      disk_devices: Vec::new()
    }
  }

  pub fn add_device_file(&mut self, dev: Rc<RefCell<dyn IFile>>) {
    self.disk_devices.push(dev)
  }
}

impl vfs::INode for DevFS{
    fn get_name(&self) -> String {
        "dev".to_owned()
    }
}

impl vfs::IFolder for DevFS{
    fn get_children(&self) -> Vec<Node> {
       let mut v = Vec::<Node>::new();
       for c in &self.disk_devices{
         v.push(Node::File(c.clone()))
       }
       v
    }
}