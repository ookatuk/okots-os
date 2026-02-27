use core::num::NonZeroUsize;
use core::ptr::NonNull;
use uefi::boot::ScopedProtocol;
use uefi::proto::console::gop::{BltOp, GraphicsOutput, PixelFormat};
use uefi_raw::protocol::console::PixelBitmask;
use crate::{log_debug, log_info, result};
use crate::{ALLOW_RATIOS};
use crate::util::result::{Error, ErrorType};

fn scale_f32_to_mask(intensity: f32, mask: u32) -> u32 {
    if mask == 0 { return 0; }

    let width = mask.count_ones();
    let shift = mask.trailing_zeros();

    let max_val = (1 << width) - 1;

    let val = libm::round((intensity * max_val as f32) as f64) as u32;
    val << shift
}

#[derive(Copy, Clone, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Color {
            r,
            g,
            b
        }
    }

    pub fn from_rgb(data: u32) -> Self {
        let bytes = data.to_be_bytes();
        Self::new(
            bytes[1] as f32 / 255.0,
            bytes[2] as f32 / 255.0,
            bytes[3] as f32 / 255.0,
        )
    }

    pub fn get(&self, format: PixelFormat, mask: Option<PixelBitmask>) -> Option<result::Result<u32>> {
        let r = self.r.clamp(0.0, 1.0);
        let g = self.g.clamp(0.0, 1.0);
        let b = self.b.clamp(0.0, 1.0);

        Some(Ok(match format {
            PixelFormat::Rgb => {
                let r8 = libm::round((r * 255f32) as f64) as u32;
                let g8 = libm::round((g * 255f32) as f64) as u32;
                let b8 = libm::round((b * 255f32) as f64) as u32;
                r8 | (g8 << 8) | (b8 << 16)
            }
            PixelFormat::Bgr => {
                let r8 = libm::round((r * 255f32) as f64) as u32;
                let g8 = libm::round((g * 255f32) as f64) as u32;
                let b8 = libm::round((b * 255f32) as f64) as u32;
                b8 | (g8 << 8) | (r8 << 16)
            }
            PixelFormat::Bitmask => {
                if let Some(m) = mask {
                    let r_part = scale_f32_to_mask(r, m.red);
                    let g_part = scale_f32_to_mask(g, m.green);
                    let b_part = scale_f32_to_mask(b, m.blue);
                    r_part | g_part | b_part
                } else {
                    return Some(Error::new(
                        ErrorType::InvalidData,
                        Some("pixel format is bitmask. but mast is none")
                    ).raise());
                }
            }
            PixelFormat::BltOnly => return None,
        }))
    }
}

#[derive(Debug)]
pub struct GopData {
    pub ptr: Option<NonNull<u32>>,
    pub gop: ScopedProtocol<GraphicsOutput>,
    pub w: NonZeroUsize,
    pub h: NonZeroUsize,
    pub stride: NonZeroUsize,
    pub format: PixelFormat,
    pub mask: Option<PixelBitmask>,
}

impl GopData {
    pub fn new(mut gop: ScopedProtocol<GraphicsOutput>) -> result::Result<Self> {
        let mut data = Self {
            ptr: None,
            w: NonZeroUsize::new(1).unwrap(),
            h: NonZeroUsize::new(1).unwrap(),
            stride: NonZeroUsize::new(1).unwrap(),
            gop,
            format: PixelFormat::BltOnly,
            mask: None,
        };

        data.update()?;

        Ok(data)
    }

    pub fn update(&mut self) -> result::Result {
        let info = self.gop.current_mode_info();
        let (w, h) = info.resolution();
        let ptr = self.gop.frame_buffer().as_mut_ptr() as *mut u32;
        let format = info.pixel_format();

        if format != PixelFormat::BltOnly && ptr.is_null() {
            return Error::new(
                ErrorType::UefiBroken,
                Some("It is specified as writable directly, but there is no pointer."),
            ).raise();
        }

        self.ptr = NonNull::new(ptr);
        self.w = NonZeroUsize::new(w).unwrap();
        self.h = NonZeroUsize::new(h).unwrap();
        self.stride = NonZeroUsize::new(info.stride()).unwrap();
        self.format = format;
        self.mask = info.pixel_bitmask();

        Ok(())
    }

    #[inline]
    pub unsafe fn draw_pixel(&mut self, x: usize, y: usize, color: Color) -> result::Result {
        let (w, h) = {
            (self.w.get(), self.h.get())
        };

        if x < w && y < h {
            return self.draw_pixel_unchecked(x, y, color);
        }

        Error::new(
            ErrorType::InvalidData,
            Some("pixel out of bounds"),
        ).raise()
    }

    pub unsafe fn draw_pixel_unchecked(&mut self, x: usize, y: usize, color: Color) -> result::Result {
        let raw_color = color.get(self.format, self.mask);

        if let Some(raw_res) = raw_color {
            let raw = raw_res?;

            let ptr = self.ptr.unwrap();
            let offset = y * self.stride.get() + x;
            unsafe { ptr.as_ptr().add(offset).write_volatile(raw) };
        } else {
            let blt_pixel = uefi::proto::console::gop::BltPixel::new(
                (color.r * 255f32) as u8,
                (color.g * 255f32) as u8,
                (color.b * 255f32) as u8,
            );

            self.gop.blt(BltOp::VideoFill {
                color: blt_pixel,
                dest: (x, y),
                dims: (1, 1),
            }).map_err(|e| Error::from_uefi(e, Some("Blt failed")))?;
        };

        Ok(())
    }

    pub unsafe fn draw_line(&mut self, x0: isize, y0: isize, x1: isize, y1: isize, color: Color) -> result::Result {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        let mut curr_x = x0;
        let mut curr_y = y0;

        loop {
            self.draw_pixel(curr_x as usize, curr_y as usize, color)?;

            if curr_x == x1 && curr_y == y1 { break; }

            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                curr_x += sx;
            }
            if e2 <= dx {
                err += dx;
                curr_y += sy;
            }
        }
        Ok(())
    }

    pub unsafe fn draw_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Color) -> result::Result {
        let raw_color = if let Some(raw_res) = color.get(self.format, self.mask) {
            raw_res?
        } else {
            let blt_pixel = uefi::proto::console::gop::BltPixel::new(
                (color.r * 255.0) as u8, (color.g * 255.0) as u8, (color.b * 255.0) as u8
            );
            return self.gop.blt(BltOp::VideoFill {
                color: blt_pixel,
                dest: (x, y),
                dims: (w, h),
            }).map_err(|e| Error::from_uefi(e, Some("Blt fill failed")));
        };

        let ptr = self.ptr.unwrap_or_else(|| core::hint::unreachable_unchecked()).as_ptr();
        let stride = self.stride.get();

        for py in y..(y + h) {
            let row_ptr = ptr.add(py * stride + x);

            for px in 0..w {
                row_ptr.add(px).write_volatile(raw_color);
            }
        }

        Ok(())
    }

    pub unsafe fn clear(&mut self, color: Color) -> result::Result {
        let raw_color = if let Some(c) = color.get(self.format, self.mask) {
            c?
        } else {
            // BltOnly の場合は UEFI の機能で一括塗り
            return self.draw_rect(0, 0, self.w.get(), self.h.get(), color);
        };

        let color64 = ((raw_color as u64) << 32) | (raw_color as u64);
        let ptr = self.ptr.unwrap_or_else(|| core::hint::unreachable_unchecked()).as_ptr();
        let w = self.w.get();
        let h = self.h.get();
        let stride = self.stride.get();

        if w == stride {
            let total_pixels = w * h;
            core::arch::asm!(
            "rep stosq",
            inout("rcx") total_pixels / 2 => _,
            inout("rdi") ptr => _,
            in("rax") color64,
            );
            if total_pixels % 2 != 0 {
                ptr.add(total_pixels - 1).write_volatile(raw_color);
            }
        } else {
            for y in 0..h {
                let row_ptr = ptr.add(y * stride);

                core::arch::asm!(
                "rep stosq",
                inout("rcx") w / 2 => _,
                inout("rdi") row_ptr as usize => _,
                in("rax") color64,
                );

                if w % 2 != 0 {
                    row_ptr.add(w - 1).write_volatile(raw_color);
                }
            }
        }
        Ok(())
    }
}

unsafe impl Send for GopData {}
unsafe impl Sync for GopData {}

pub fn get_gop(mut gop: ScopedProtocol<GraphicsOutput>) -> result::Result<GopData> {
    let mut target: Option<(usize, usize, uefi::proto::console::gop::Mode)> = None;

    log_debug!("kernel", "gop", "getting good gop modes...");

    for mode in gop.modes() {
        let info = mode.info();
        let (w, h) = info.resolution();

        if let Some((level, _)) = ALLOW_RATIOS.iter().enumerate().find(|&(_, &(rw, rh))| w * rh == h * rw) {

            let is_better = if let Some((best_w, best_level, _)) = target {
                // レベルが低い（優先度が高い）か、同じレベルで幅が広い場合
                level < best_level || (level == best_level && w > best_w)
            } else {
                true
            };

            if is_better {
                target = Some((w, level, mode));
            }
        }
    }

    log_debug!("kernel", "gop", "setting goog modes...");

    if let Some((_, _, mode)) = target {
        Error::try_raise(gop.set_mode(&mode), Some("Failed to set video mode"))?;
    }

    log_debug!("kernel", "gop", "creating gop struct...");

    let info = gop.current_mode_info();
    let (w, h) = info.resolution();

    log_info!("kernel", "gop", "found good gop mode. w: {}, h: {}", w, h);

    GopData::new(gop)
}