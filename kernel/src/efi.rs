use core::ffi::c_void;

//  gnu-efi, inc/efiprot.h, line 840 { 0x9042a9de, 0x23dc, 0x4a38, {0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a } }
// Rando internet forum: 0deh, 0a9h, 42h,90h,0dch,023h,38h,04ah,96h,0fbh,7ah,0deh,0d0h,80h,51h,6ah

pub const GOP_GUID: u128 = 0xdea94290dc23384a96fb7aded080516a_u128.to_be();

#[repr(C)]
pub struct EfiTableHeader {
    pub signature: u64,
    pub rev: u32,
    pub table_size: u32,
    pub crc: u32,
    _reserved: u32,
}

type EfiStatus = usize;

#[derive(Clone, Copy)]
#[repr(C)]
pub enum EfiGraphicsPixelFormat {
    RgbR8bit = 0,  // 4 bytes in the order: red green blue reserved, in big endian
    BgrR8bit = 1,  // 4 bytes in the order: blue green red reserved, in big endian
    BitMask = 2,   // use mask for the pixel format
    BltOnly = 3,   // no framebuffer, must use blit function from uefi
    FormatMax = 4, // no format above this, including this is valid
}

#[repr(C)]
pub struct EfiPixelBitMask {
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    reserved_mask: u32,
}

#[repr(C)]
pub struct EfiGopModeInfo {
    pub version: u32,
    pub horz_res: u32,
    pub vert_res: u32,
    pub pix_format: EfiGraphicsPixelFormat,
    pub pix_mask: EfiPixelBitMask,
    pub pix_per_scan_line: u32,
}

#[repr(C)]
pub struct EfiGopMode<'a> {
    pub max_mode: u32,
    pub mode: u32,
    pub info: &'a EfiGopModeInfo,
    pub size_of_info: usize,
    pub framebuffer_base: u64, /* yes even on 32-bit efi */
    pub framebuffer_size: usize,
}

#[repr(C)]
pub struct EfiGop<'a> {
    pub query_mode: extern "efiapi" fn(
        this: &EfiGop,
        mode_numer: u32,
        size_of_info: &mut usize,
        info: *mut *const EfiGopModeInfo,
    ) -> EfiStatus,
    pub set_mode: extern "efiapi" fn(this: &mut EfiGop, mode_number: u32) -> EfiStatus,
    blt: *const c_void,
    pub mode: &'a mut EfiGopMode<'a>,
}

#[repr(C)]
#[derive(Default)]
pub struct EfiTime {
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
    _pad2: u8,
}

#[repr(C)]
pub struct EfiRuntimeServices {
    pub hdr: EfiTableHeader,
    pub get_time:
        extern "efiapi" fn(efi_time: *mut EfiTime, capabilities: *const c_void) -> EfiStatus,
}

#[repr(C)]
pub struct EfiBootServices {
    pub hdr: EfiTableHeader,
    _we_dont_care_about_the_first_couple_of_function_pointers_for_now_dont_worry_about_it:
        [*const c_void; 37],
    pub locate_protocol: extern "efiapi" fn(
        guid: *const u128,
        optional: *const c_void,
        interface: *mut *mut EfiGop,
    ) -> EfiStatus,
    pub install_multiple_protocol_interfaces: u32,
    uninstall_multiple_protocol_interfaces: *const c_void,
    calculate_crc32: *const c_void,
    copy_mem: *const c_void,
    set_mem: *const c_void,
    create_event_ex: *const c_void,
}

#[repr(C)]
pub struct EfiSystemTable {
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
    config_table: *const c_void,
}
