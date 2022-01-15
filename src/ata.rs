use core::mem;
use crate::{virtmem::{KernPointer, PointerLike}, X86Default};

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
    unsafe fn new(base: KernPointer<u8>) -> Self{
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

    unsafe fn write_features(&mut self, d: u8){
        self.err_features.write(d)
    }

    unsafe fn read_status(&self) -> u8{
        self.stat_command.read()
    }

    unsafe fn write_command(&mut self, d: u8){
        self.stat_command.write(d)
    }
}

struct ControlRegistersLBA28 {
    alt_stat_device_ctrl: KernPointer<u8>,
    drive_addr: KernPointer<u8>
}

impl ControlRegistersLBA28 {
    unsafe fn new(base: KernPointer<u8>) -> Self{
        ControlRegistersLBA28{
            alt_stat_device_ctrl: base,
            drive_addr: base.offset(1)
        }
    }

    unsafe fn read_alt_stat(&self) -> u8{
        self.alt_stat_device_ctrl.read()
    }

    unsafe fn write_device_ctrl(&mut self, d: u8){
        self.alt_stat_device_ctrl.write(d)
    }

    unsafe fn read_drive_addr(&self) -> u8{
        self.drive_addr.read()
    }
}
type Sector = [u16; 256];

pub enum ATADevice {MASTER, SLAVE}
pub struct ATABus{
    io: IORegistersLBA28,
    control: ControlRegistersLBA28
}

pub struct LBA28{
    pub low: u8,
    pub mid: u8,
    pub hi: u8
}

impl ATABus{
    pub unsafe fn new(io_base: KernPointer<u8>, cntrl_base: KernPointer<u8>) -> Option<Self>{
        let bus = ATABus{
            io: IORegistersLBA28::new(io_base),
            control: ControlRegistersLBA28::new(cntrl_base)
        };
        // IO bus has pull-up resitors so 0xFF, which is normally and invalid value anyway, probs indicates no drives on the bus
        if bus.io.read_status() == 0xFF {
            None
        }else{
            Some(bus)
        }
    }
    
    pub unsafe fn identify(&mut self, device: ATADevice) -> Option<Sector> {
        self.io.drive_sel.write(match device{
            ATADevice::MASTER => 0xA0,
            ATADevice::SLAVE => 0xB0
        });

        self.io.address_hi.write(0);
        self.io.address_mid.write(0);
        self.io.address_low.write(0);
        self.io.write_command(0xEC); // IDENTIFY
        if self.io.read_status() == 0x0 { return None; }
        wait_for!(self.io.read_status() & (1 << 7) == 0); // BSY clears
        wait_for!(self.io.read_status() & (1 << 3) != 0 || self.io.read_status() & (1 << 0) != 0); // DRQ or ERR sets
        if self.io.read_status() & (1 << 0) == 1 { return None; } // ERR
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
        self.io.write_command(0x20); // Read sectors command
        
        wait_for!(self.io.read_status() & (1 << 7) == 0); // BSY clears
        wait_for!(self.io.read_status() & (1 << 3) != 0 || self.io.read_status() & (1 << 0) != 0); // DRQ or ERR sets
        if self.io.read_status() & (1 << 0) == 1 { return None; } // ERR

        let mut a = [0u16; 256];
        a.iter_mut().for_each(|e| *e = self.io.data.read());
        Some(a)
    }

    pub unsafe fn write_sector(&mut self, device: ATADevice, sector_lba: LBA28, data: Sector) {
        self.io.drive_sel.write(match device{
            ATADevice::MASTER => 0xE0,
            ATADevice::SLAVE => 0xF0
        } | (sector_lba.hi >> 4) );
        self.io.write_features(0); // No features
        self.io.sector_count.write(1); // Read one sector
        self.io.address_low.write(sector_lba.low);
        self.io.address_mid.write(sector_lba.mid);
        self.io.address_hi.write(sector_lba.hi);
        self.io.write_command(0x30); // Write sectors command

        wait_for!(self.io.read_status() & (1 << 7) == 0); // BSY clears
        wait_for!(self.io.read_status() & (1 << 3) != 0 || self.io.read_status() & (1 << 0) != 0); // DRQ or ERR sets
        // if self.io.read_status() & (1 << 0) == 1 { return None; } // ERR

        data.iter().for_each(|e| self.io.data.write(*e));

    }
}

impl X86Default for ATABus{
    unsafe fn x86_default() -> Self {
       ATABus::new(KernPointer::<u8>::from_port(0x1F0), KernPointer::<u8>::from_port(0x3F6)).unwrap()
    }
}