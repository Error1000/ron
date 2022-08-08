use core::{mem, cell::RefCell};
use alloc::{rc::Rc, vec::Vec};
use packed_struct::prelude::PackedStruct;

use crate::{virtmem::KernPointer, vfs::IFile};

#[derive(PackedStruct, PartialEq)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "1")]
struct ATAStatus{
    #[packed_field(bits = "0")]
    ata_err: bool,
    #[packed_field(bits = "1")]
    ata_idx: bool,
    #[packed_field(bits = "2")]
    ata_corrected: bool,
    #[packed_field(bits = "3")]
    ata_data_request: bool,
    #[packed_field(bits = "4")]
    ata_overlapped_mode_service_request: bool,
    #[packed_field(bits = "5")]
    ata_drive_fault_error: bool,
    #[packed_field(bits = "6")]
    ata_ready: bool,
    #[packed_field(bits = "7")]
    ata_busy: bool
}

struct IORegistersLBA28 {
    pub data: KernPointer<u16>,
    err_features: KernPointer<u8>,
    pub sector_count: KernPointer<u8>,
    pub address_low: KernPointer<u8>,
    pub address_mid: KernPointer<u8>,
    pub address_hi: KernPointer<u8>,
    pub drive_sel: KernPointer<u8>,
    stat_command: KernPointer<u8>
}

impl IORegistersLBA28{
    unsafe fn new(base: KernPointer<u8>) -> Self {
        IORegistersLBA28{
            data: mem::transmute::<KernPointer<u8>, _>(base),
            err_features: base.offset(1),
            sector_count: base.offset(2),
            address_low: base.offset(3),
            address_mid: base.offset(4),
            address_hi: base.offset(5),
            drive_sel: base.offset(6),
            stat_command: base.offset(7)
        }
    }

    unsafe fn read_error(&self) -> u8 {
        self.err_features.read()
    }

    unsafe fn write_features(&mut self, d: u8) {
        self.err_features.write(d)
    }

    unsafe fn read_status(&self) -> ATAStatus {
        ATAStatus::unpack(&[self.stat_command.read()]).expect("Reading and parsing ata status register should always work")
    }

    unsafe fn write_command(&mut self, d: u8) {
        self.stat_command.write(d)
    }
}

struct ControlRegistersLBA28 {
    alt_stat_device_ctrl: KernPointer<u8>,
    drive_addr: KernPointer<u8>
}

impl ControlRegistersLBA28 {
    unsafe fn new(base: KernPointer<u8>) -> Self {
        ControlRegistersLBA28{
            alt_stat_device_ctrl: base,
            drive_addr: base.offset(1)
        }
    }

    unsafe fn read_alt_stat(&self) -> u8 {
        self.alt_stat_device_ctrl.read()
    }

    unsafe fn write_device_ctrl(&mut self, d: u8) {
        self.alt_stat_device_ctrl.write(d)
    }

    unsafe fn read_drive_addr(&self) -> u8 {
        self.drive_addr.read()
    }
}

pub const SECTOR_SIZE_IN_BYTES: usize = 256*core::mem::size_of::<u16>();
type Sector = [u16; SECTOR_SIZE_IN_BYTES/core::mem::size_of::<u16>()];

#[derive(Clone, Copy)]
pub enum ATADevice {MASTER, SLAVE}

#[derive(Clone, Copy)]
enum BUSType{Primary, Secondary}

impl BUSType {
    pub fn into_str(self) -> &'static str{
        match self{
            BUSType::Primary => "primary",
            BUSType::Secondary => "secondary"
        }    
    }
}

pub struct ATABus {
    io: IORegistersLBA28,
    control: ControlRegistersLBA28,
    master_sector_count: Option<u32>,
    slave_sector_count: Option<u32>,
    bus_type: BUSType
}

#[derive(Clone, Copy)]

pub struct LBA28 {
    pub low: u8,
    pub mid: u8,
    pub hi: u8
}

impl From<u32> for LBA28 {
    fn from(v: u32) -> Self {
        let data = v.to_le_bytes();
        LBA28{
            low: data[0],
            mid: data[1],
            hi: data[2]
        }
    }
}

impl Into<u32> for LBA28 {
    fn into(self) -> u32 {
        u32::from_le_bytes([self.low, self.mid, self.hi, 0])
    }
}
impl ATADevice {
    pub fn into_str(self) -> &'static str{
        match self{
            ATADevice::MASTER => "master",
            ATADevice::SLAVE => "slave"
        }    
    }
}

#[allow(unused)]
mod ata_command {
    pub const NOP: u8 = 0x00;
    pub const READ_SECTORS: u8 = 0x20;
    pub const WRITE_SECTORS: u8 = 0x30;
    pub const READ_DMA: u8 = 0xC8;
    pub const WRITE_DMA: u8 = 0xCA;
    pub const STANDBY_IMMEDIATE: u8 = 0xE0;
    pub const IDLE_IMMEDIATE: u8 = 0xE1;
    pub const STANDBY: u8 = 0xE2;
    pub const IDLE: u8 = 0xE3;
    pub const READ_BUFFER: u8 = 0xE4;
    pub const CHECK_POWER_MODE: u8 = 0xE5;
    pub const SLEEP: u8 = 0xE6;
    pub const WRITE_BUFFER: u8 = 0xE8;
    pub const IDENTIYFY_DEVICE: u8 = 0xEC;
    pub const SET_FEATURES: u8 = 0xEF;
}

impl ATABus{
    pub unsafe fn primary_x86() -> Option<Self> {
        ATABus::new(KernPointer::<u8>::from_port(0x1F0), KernPointer::<u8>::from_port(0x3F6), BUSType::Primary)
    }

    pub unsafe fn secondary_x86() -> Option<Self> {
        ATABus::new(KernPointer::<u8>::from_port(0x170), KernPointer::<u8>::from_port(0x376), BUSType::Secondary)
    }

    unsafe fn new(io_base: KernPointer<u8>, cntrl_base: KernPointer<u8>, typ: BUSType) -> Option<Self>{
        let bus = ATABus{
            io: IORegistersLBA28::new(io_base),
            control: ControlRegistersLBA28::new(cntrl_base),
            master_sector_count: None,
            slave_sector_count: None,
            bus_type: typ
        };
        // IO bus has pull-up resitors so 0xFF, which is normally an invalid value anyway, probs indicates no drives on the bus
        if bus.io.read_status() == ATAStatus::unpack(&[0xFF]).ok()? {
            None
        }else{
            Some(bus)
        }
    }
     
    pub unsafe fn get_sector_count(&mut self, device: ATADevice) -> Option<u32>{
        match device{
            ATADevice::MASTER => {
                if let Some(sector_count) = self.master_sector_count{
                    return Some(sector_count);
                }else if let Some(id) = self.identify(device){
                    self.master_sector_count = Some(
                        u32::from_le_bytes([id[60].to_le_bytes()[0], id[60].to_le_bytes()[1], id[61].to_le_bytes()[0], id[61].to_le_bytes()[1] ])
                    );
                    return self.master_sector_count;
                }
            },
            ATADevice::SLAVE => {
                if let Some(sector_count) = self.slave_sector_count{
                    return Some(sector_count);
                }else if let Some(id) = self.identify(device){
                    self.slave_sector_count = Some(
                        u32::from_le_bytes([id[60].to_le_bytes()[0], id[60].to_le_bytes()[1], id[61].to_le_bytes()[0], id[61].to_le_bytes()[1] ])
                    );
                    return self.slave_sector_count;
                }
            }
        }
        None
    }
    
    pub unsafe fn identify(&mut self, device: ATADevice) -> Option<Sector> {
        self.io.drive_sel.write(match device{
            ATADevice::MASTER => 0xA0,
            ATADevice::SLAVE => 0xB0
        });
        self.io.address_hi.write(0);
        self.io.address_mid.write(0);
        self.io.address_low.write(0);
        self.io.write_command(ata_command::IDENTIYFY_DEVICE); // IDENTIFY
        if self.io.read_status() == ATAStatus::unpack(&[0x0]).ok()? { return None; }
        wait_for!(self.io.read_status().ata_busy == false); // BSY clears
        wait_for!({
            let status = self.io.read_status();
            status.ata_data_request || status.ata_err
        }); // DRQ or ERR sets
        if self.io.read_status().ata_err { return None; } // ERR
        let mut a = [0u16; 256];
        a.iter_mut().for_each(|e| *e = self.io.data.read());
        Some(a)
    }

    pub unsafe fn read_sector(&mut self, device: ATADevice, sector_lba: LBA28) -> Option<Sector> {
        self.io.drive_sel.write(match device{
            ATADevice::MASTER => 0xE0,
            ATADevice::SLAVE => 0xF0
        } | (sector_lba.hi >> 4) );
        self.io.write_features(0); // No features
        self.io.sector_count.write(1); // Read one sector
        self.io.address_low.write(sector_lba.low);
        self.io.address_mid.write(sector_lba.mid);
        self.io.address_hi.write(sector_lba.hi);
        self.io.write_command(ata_command::READ_SECTORS); // Read sectors command
        
        wait_for!(self.io.read_status().ata_busy == false); // BSY clears
        wait_for!({
            let status = self.io.read_status();
            status.ata_data_request || status.ata_err
        }); // DRQ or ERR sets
        if self.io.read_status().ata_err { return None; } // ERR

        let mut a = [0u16; 256];
        a.iter_mut().for_each(|e| *e = self.io.data.read());
        Some(a)
    }

    pub unsafe fn write_sector(&mut self, device: ATADevice, sector_lba: LBA28, data: &Sector) {
        self.io.drive_sel.write(match device{
            ATADevice::MASTER => 0xE0,
            ATADevice::SLAVE => 0xF0
        } | (sector_lba.hi >> 4) );
        self.io.write_features(0); // No features
        self.io.sector_count.write(1); // Read one sector
        self.io.address_low.write(sector_lba.low);
        self.io.address_mid.write(sector_lba.mid);
        self.io.address_hi.write(sector_lba.hi);
        self.io.write_command(ata_command::WRITE_SECTORS); // Write sectors command

        wait_for!(self.io.read_status().ata_busy == false); // BSY clears
        wait_for!({
            let status = self.io.read_status();
            status.ata_data_request || status.ata_err
        }); // DRQ or ERR sets
        if self.io.read_status().ata_err { return; } // ERR

        data.iter().for_each(|e| self.io.data.write(*e));

    }
}


pub struct ATADeviceFile{
    pub bus: Rc<RefCell<ATABus>>,
    pub bus_device: ATADevice
}


impl IFile for ATADeviceFile{
    fn read(&self, offset_in_bytes: usize, len: usize) -> Option<Vec<u8>> {
      let offset_in_first_sector = offset_in_bytes % SECTOR_SIZE_IN_BYTES;
      let offset_of_first_sector = offset_in_bytes / SECTOR_SIZE_IN_BYTES;
      let mut res: Vec<u8> = Vec::with_capacity(len);

      // Deal with first block
      let first_block_lba = LBA28{hi: ((offset_of_first_sector>>16)&0xFF) as u8, mid: ((offset_of_first_sector>>8)&0xFF) as u8, low: (offset_of_first_sector&0xFF) as u8};
      let first_block = unsafe{ (*self.bus).borrow_mut().read_sector(self.bus_device, first_block_lba) }?;

      let mut skip_first_byte = offset_in_first_sector%2 == 1;
      for e in &first_block[offset_in_first_sector/2..] {                 
          if skip_first_byte {
            res.push(((e >> 8)&0xFF) as u8); 
            skip_first_byte = false;
            continue;
          } 
          res.push((e&0xFF) as u8);
          res.push(((e >> 8)&0xFF) as u8); 
      }

      // Read continually, until the end is included in res, overreading if necessary
      let extra_block = if len%SECTOR_SIZE_IN_BYTES != 0 { 1 } else { 0 };
      for sector_indx in 1..((len/SECTOR_SIZE_IN_BYTES)+extra_block){
        if res.len() >= len { break; }
        let offset = offset_of_first_sector+sector_indx;
        let lba = LBA28{hi: ((offset>>16)&0xFF) as u8, mid: ((offset>>8)&0xFF) as u8, low: (offset&0xFF) as u8};
        res.append(
        &mut unsafe{ (*self.bus).borrow_mut().read_sector(self.bus_device, lba) }
        .map(|val|{
            let mut v = Vec::with_capacity(SECTOR_SIZE_IN_BYTES);
            for e in &val {
                v.extend(e.to_le_bytes());
            }
            v
        })?
        );
      }
      // Get rid of overread bytes
      while res.len() > len { res.pop(); }
      assert!(res.len() == len, "The amount of bytes read from disk should be the same as the amount requested!"); 
      Some(res)
    }

    fn write(&mut self, offset_in_bytes: usize, data: &[u8]) -> Option<usize> {
       let offset_in_first_sector = offset_in_bytes % SECTOR_SIZE_IN_BYTES;
       let offset_of_first_sector = offset_in_bytes / SECTOR_SIZE_IN_BYTES;
       let mut iter = data.iter();
       
       let extra_block = if data.len()%SECTOR_SIZE_IN_BYTES != 0 { 1 } else { 0 };

       let mut ind = offset_in_first_sector;
       for sector_indx in 0..(data.len()/SECTOR_SIZE_IN_BYTES+extra_block){
         let offset = offset_of_first_sector+sector_indx;
         let lba = LBA28{hi: ((offset>>16)&0xFF) as u8, mid: ((offset>>8)&0xFF) as u8, low: (offset&0xFF) as u8};
         
         // No need to read sectors that we know will be completly overriden
         let mut v = if sector_indx == (data.len()/SECTOR_SIZE_IN_BYTES+extra_block-1) && extra_block == 1 { 
            unsafe{ (*self.bus).borrow_mut().read_sector(self.bus_device, lba) }.expect("Reading device should work!")
         } else {
            [0u16; SECTOR_SIZE_IN_BYTES/core::mem::size_of::<u16>()] 
         };

         while let (Some(a), Some(b)) = (iter.next(), iter.next()) {
            if ind >= v.len() { break; }
            v[ind] = u16::from_le_bytes([*a, *b]);
            ind += 1;
         }
         ind = 0;

         unsafe{ (*self.bus).borrow_mut().write_sector(self.bus_device, lba, &v) }
        }
        Some(data.len())
    }

    fn resize(&mut self, _new_size: usize) -> Option<()> {
        None
    }

    fn get_size(&self) -> usize{
      let mut ata_bus = (*self.bus).borrow_mut();
      let sector_count = unsafe{ ata_bus.get_sector_count(self.bus_device) }.expect("Rading device should work!");
      (sector_count as usize) * SECTOR_SIZE_IN_BYTES
    }

}