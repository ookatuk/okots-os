use core::ptr::NonNull;

#[derive(Debug)]
pub struct GopData {
    pub ptr: NonNull<u32>,
    pub w: usize,
    pub h: usize,
    pub stride: usize,
}

impl GopData {
    pub unsafe fn draw_pixel(&self, x: usize, y: usize, color: u32) {
        if x < self.w && y < self.h {
            let offset = y * self.stride + x;
            unsafe{self.ptr.add(offset).write_volatile(color)};
        }
    }
}

unsafe impl Send for GopData {}
unsafe impl Sync for GopData {}
