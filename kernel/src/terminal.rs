use core::fmt::{Debug, Write};

use crate::{
    char_device::CharDevice,
    framebuffer::{FrameBuffer, Pixel},
    primitives::{LazyInitialised, Mutex},
};

pub static TERMINAL: Mutex<LazyInitialised<Terminal<'static>>> = Mutex::from(LazyInitialised::uninit());

pub struct Terminal<'a> {
    pub fb: &'a mut dyn FrameBuffer,
    cursor_pos: (usize, usize),
    cursor_char: char,
    color: Pixel,
}

impl Debug for Terminal<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Terminal")
            .field("cursor_pos", &self.cursor_pos)
            .field("cursor_char", &self.cursor_char)
            .field("color", &self.color)
            .finish()
    }
}

impl<'a> Write for Terminal<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().for_each(|c| self.write_char(c));
        Ok(())
    }
}

impl<'a> Terminal<'a> {
    pub fn new(fb: &'a mut dyn FrameBuffer, color: Pixel) -> Self {
        Terminal { fb, cursor_pos: (0, 0), cursor_char: ' ', color }
    }

    pub fn clear(&mut self) {
        for i in 0..self.fb.get_height() {
            for j in 0..self.fb.get_width() {
                self.fb.set_pixel(j, i, Pixel { r: 0, g: 0, b: 0 });
            }
        }
        self.cursor_pos = (0, 0);
    }

    pub fn cursor_up(&mut self) {
        if self.cursor_pos.1 == 0 {
            return;
        }
        self.cursor_pos.1 -= 1;
    }

    pub fn cursor_down(&mut self) {
        if self.cursor_pos.1 >= self.fb.get_rows() - 1 {
            self.cursor_pos.1 = 0;
        } else {
            self.cursor_pos.1 += 1;
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_pos.0 >= self.fb.get_cols() - 1 {
            self.cursor_pos.0 = 0;
            self.cursor_down();
            for x in 0..self.fb.get_cols() {
                self.fb.write_char(x, self.cursor_pos.1, ' ', self.color);
            }
            return;
        }
        self.cursor_pos.0 += 1;
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_pos.0 == 0 {
            return;
        }
        self.cursor_pos.0 -= 1;
    }

    pub fn visual_cursor_up(&mut self) {
        self.erase_visual_cursor();
        self.cursor_up();
        self.update_visual_cursor();
    }

    pub fn visual_cursor_left(&mut self) {
        self.erase_visual_cursor();
        self.cursor_left();
        self.update_visual_cursor();
    }

    pub fn visual_cursor_right(&mut self) {
        self.erase_visual_cursor();
        self.cursor_right();
        self.update_visual_cursor();
    }

    pub fn visual_cursor_down(&mut self) {
        self.erase_visual_cursor();
        self.cursor_down();
        self.update_visual_cursor();
    }

    fn update_visual_cursor(&mut self) {
        self.fb.write_char(self.cursor_pos.0, self.cursor_pos.1, '_', self.color);
    }

    fn erase_visual_cursor(&mut self) {
        self.fb.write_char(self.cursor_pos.0, self.cursor_pos.1, self.cursor_char, self.color);
    }

    pub fn write_char(&mut self, c: char) {
        self.erase_visual_cursor(); // erase current cursor
        match c {
            '\n' => {
                self.cursor_down();
                for x in 0..self.fb.get_cols() {
                    self.fb.write_char(x, self.cursor_pos.1, ' ', self.color);
                }
                self.cursor_pos.0 = 0;
            }
            '\r' => {
                self.cursor_left(); // Go to char
                self.erase_visual_cursor();
            }
            c => {
                self.fb.write_char(self.cursor_pos.0, self.cursor_pos.1, c, self.color);
                self.cursor_right();
            }
        }
        self.update_visual_cursor();
    }
}
