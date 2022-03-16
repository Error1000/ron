use core::{ptr, ffi, slice};

use crate::{efi::{EfiGopMode, self}, vga::{Vga, MixedRegisterState, Color256, self, VgaMode, Unblanked}};

#[derive(Clone, Copy)]
pub struct Pixel{
    pub r: u8,
    pub g: u8,
    pub b: u8
}

impl Pixel{
    pub fn from_u32_rgb(val: u32) -> Pixel{
        Pixel{b: ((val&0xFF) >> 0) as u8, g: ((val&0xFF00) >> 8) as u8, r: ((val&0xFF0000) >> 16) as u8 }
    }

    pub fn from_u32_bgr(val: u32) -> Pixel{
        Pixel{r: ((val&0xFF) >> 0) as u8, g: ((val&0xFF00) >> 8) as u8, b: ((val&0xFF0000) >> 16) as u8 }
    }
}

pub trait FrameBuffer {
    fn get_width(&self) -> usize;
    fn get_height(&self) -> usize;
    fn set_pixel(&mut self, x: usize, y: usize, pixel: Pixel) -> Option<(i16, i16, i16)>;
    fn fill(&mut self, x1: usize, y1: usize, x2: usize, y2: usize, pixel: Pixel){
        for y in y1..y2{
            for x in x1..x2{
                self.set_pixel(x, y, pixel);
            }
        }
    }
}

impl<'a> FrameBuffer for EfiGopMode<'a>{
    fn get_width(&self) -> usize {
        self.info.horz_res as usize
    }

    fn get_height(&self) -> usize {
        self.info.vert_res as usize
    }

    #[inline(always)]
    fn set_pixel(&mut self, x: usize, y: usize, pixel: Pixel) -> Option<(i16, i16, i16)> {
       let fb_ptr = unsafe{slice::from_raw_parts_mut(self.framebuffer_base as *mut u32, self.get_width()*self.get_height())};
       if x > self.get_width() { return None }
       if y > self.get_height() { return None }
       match self.info.pix_format{
        efi::EfiGraphicsPixelFormat::RgbR8bit => { fb_ptr[y*self.get_width() + x] = ((pixel.r as u32) << 24 | (pixel.g as u32) << 16 | (pixel.b as u32) << 8).to_be(); return Some((0, 0, 0))},
        efi::EfiGraphicsPixelFormat::BgrR8bit => { fb_ptr[y*self.get_width() + x] = ((pixel.b as u32) << 24 | (pixel.g as u32) << 16 | (pixel.r as u32) << 8).to_be(); return Some((0, 0, 0))},
        efi::EfiGraphicsPixelFormat::BitMask => return None,
        efi::EfiGraphicsPixelFormat::BltOnly => return None,
        efi::EfiGraphicsPixelFormat::FormatMax => return None
      };
    }

}
impl<STATE: MixedRegisterState> FrameBuffer for Vga<Color256, STATE>{
    fn get_width(&self) -> usize {
        320
    }

    fn get_height(&self) -> usize {
        200
    }

    #[inline(always)]
    fn set_pixel(&mut self, x: usize, y: usize, pixel: Pixel) -> Option<(i16, i16, i16)> {
        let mut best_ind = 0;
        let mut best_err = i32::MAX;
        let mut best_color = Pixel{r: 0, g: 0, b: 0};

        for (i, c)  in vga::FANCY_PALETTE.iter().enumerate(){
            let err = (c.0 as i32*4 - pixel.r as i32).abs().pow(2) + (c.1 as i32*4 - pixel.g as i32).abs().pow(2) + (c.2 as i32*4 - pixel.b as i32).abs().pow(2);
            if err < best_err{
                best_ind = i;
                best_err = err;
                best_color = Pixel{r: c.0, g: c.1, b: c.2};
            }
        }
        unsafe{ self.write(x, y, best_ind as u8); }
        Some((best_color.r as i16 - pixel.r as i16, best_color.g as i16 - pixel.r as i16, best_color.b as i16 - pixel.b as i16))
    }
}


pub fn try_setup_efi_framebuffer(efi_table: *mut efi::EfiSystemTable, _desired_res_w: u32, _desired_res_h: u32) -> Option<&'static mut impl FrameBuffer>{
    if efi_table == ptr::null_mut(){ return None }
    let efi_table = unsafe{&mut *efi_table};
    if unsafe{core::mem::transmute::<_, *const ffi::c_void>(&*efi_table.boot_services)} == ptr::null() { return None }

    let mut gop: *mut efi::EfiGop = ptr::null_mut();
    let res =  (efi_table.boot_services.locate_protocol)(&efi::GOP_GUID, ptr::null(), &mut gop);
    if (res as isize) < 0{  return None;  }
    
    let mut gop = unsafe{&mut *gop};
      
    let mut info: *const efi::EfiGopModeInfo = ptr::null();
    let mut size_of_info: usize = 0;
    let mut _num_modes: usize = 0;
        
    let res = (gop.query_mode)(gop, if gop.mode as *const EfiGopMode == ptr::null() {0} else {gop.mode.mode}, &mut size_of_info, &mut info );
    if (res as isize) == -19 /* EFI_NOT_STARTED */ {
        (gop.set_mode)(&mut gop, 0);
    } else if (res as isize) < 0{  return None }
    else{
        _num_modes = gop.mode.max_mode as usize;
    }

    // FIXME: Seems to have problems on qemu ia32 uefi, and x64 real hardware
    // For now just keeping the default mode seems to fix the issue
    /* 
    let mut best_mode_ind = 0;
    let mut best_mode_err = i64::MAX;
       for i in 0..num_modes as u32{
           (gop.query_mode)(&gop, i as u32, &mut size_of_info, &mut info);
           let info = unsafe{&*info};
           let err = ((info.horz_res*info.vert_res) as i64 - (desired_res_h*desired_res_w) as i64).abs();
           if err < best_mode_err {
               best_mode_err = err;
               best_mode_ind = i;
           }
       }
    (gop.set_mode)(&mut gop, best_mode_ind);*/
    Some(gop.mode)
}


pub fn try_setup_vga_framebuffer<MODE: VgaMode + 'static>(vga: Vga<MODE, Unblanked>, _desired_res_w: u32, _desired_res_h: u32) -> Option<Vga<Color256, Unblanked>>{
    let vga = unsafe{vga.blank_screen()};
    Some(unsafe{vga.set_mode::<Color256>().unblank_screen()})
}
