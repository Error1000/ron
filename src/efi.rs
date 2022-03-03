use core::ffi::c_void;


#[repr(C)]
pub struct EfiTableHeader{
    pub signature: u64,
    pub rev: u32,
    pub table_size: u32,
    pub crc: u32,
    _reserved: u32
}

type EfiStatus = usize;


pub enum EfiGraphicsPixelFormat{
    Rgb8bit,
    Bgr8bit,
    BitMask,
    BltOnly,
    FormatMax
}

pub struct EfiPixelBitMask{
    red_mask: u32,
    green_mask: u32,
    blue_mask: u32,
    reserved_mask: u32
}
#[repr(C)]
pub struct EfiGopModeInfo{
    pub version: u32,
    pub horz_res: u32,
    pub vert_res: u32,
    pub pix_format: EfiGraphicsPixelFormat,
    pub pix_info: EfiPixelBitMask,
    pub pix_per_scan_line: u32,
}
#[repr(C)]
pub struct EfiGopMode<'a>{
    pub max_mode: u32,
    pub mode: u32,
    pub info: &'a  EfiGopModeInfo,
    pub size_of_info: usize,
    pub framebuffer_base: u64,
    pub framebuffer_size: usize
}

#[repr(C)]
pub struct EfiGop<'a>{
    pub query_mode: extern "efiapi" fn(this: &EfiGop, mode_numer: u32, size_of_info: &mut usize, info: *mut *const EfiGopModeInfo) -> EfiStatus,
    pub set_mode: extern "efiapi" fn(this: &EfiGop, mode_number: u32) -> EfiStatus,
    blt: *const c_void,
    pub mode: &'a EfiGopMode<'a>
}

#[repr(C)]
#[derive(Default)]
pub struct EfiTime{
    pub yr: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
    _pad1: u8,
    pub nanosec: u32,
    pub tz: i16,
    pub daylight: u8,
    _pad2: u8
}

#[repr(C)]
pub struct EfiRuntimeServices{
    pub hdr: EfiTableHeader,
    pub get_time: extern "efiapi" fn(efi_time: *mut EfiTime, capabilities: *const c_void) -> EfiStatus,
}


#[repr(C)]
pub struct EfiBootServices{
    pub hdr: EfiTableHeader,
    _we_dont_care_about_the_first_couple_of_function_pointers_for_now_dont_worry_about_it: [*const c_void; 37],
    pub locate_protocol: extern "efiapi" fn(guid: *const u128, optional: *const c_void, interface: *mut *const EfiGop) -> EfiStatus,
    pub install_multiple_protocol_interfaces: u32,
    uninstall_multiple_protocol_interfaces: *const c_void,
    calculate_crc32: *const c_void,
    copy_mem: *const c_void,
    set_mem: *const c_void,
    create_event_ex: *const c_void,
}

#[repr(C)]
pub struct EfiSystemTable{
    pub hdr: EfiTableHeader, /* 24 bytes */
    pub firmware_vendor: *const u16,
    pub firmware_rev: u32,
    
    console_in: *const c_void,
    simple_in_interface: *const c_void,

    console_out: *const c_void,
    simple_out_interface: *const c_void,

    std_err: *const c_void,
    simple_err_out_protocol: *const c_void,

    pub runtime_services: &'static mut EfiRuntimeServices,
    pub boot_services: &'static mut EfiBootServices,

    config_table_no_of_enteries: usize,
    config_table: *const c_void
}