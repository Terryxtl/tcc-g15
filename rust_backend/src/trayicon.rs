//! Tray icon rendering. Consolas temperatures, either stacked GPU-over-CPU
//! like `GUI/QGaugeTrayIcon.py` or a selected reading, plus G-mode corner dots.

use std::ffi::c_void;
use std::ptr::null_mut;

use crate::colors::{self, Rgb};
use crate::win32::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconMode {
    Max,
    Gpu,
    Cpu,
    Both,
}

impl TrayIconMode {
    pub const ALL: [TrayIconMode; 4] = [
        TrayIconMode::Max,
        TrayIconMode::Gpu,
        TrayIconMode::Cpu,
        TrayIconMode::Both,
    ];

    pub fn key(self) -> &'static str {
        match self {
            TrayIconMode::Max => "Max",
            TrayIconMode::Gpu => "GPU",
            TrayIconMode::Cpu => "CPU",
            TrayIconMode::Both => "Both",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TrayIconMode::Max => "Max",
            TrayIconMode::Gpu => "GPU",
            TrayIconMode::Cpu => "CPU",
            TrayIconMode::Both => "GPU + CPU",
        }
    }

    pub fn from_key(key: &str) -> Option<TrayIconMode> {
        TrayIconMode::ALL
            .into_iter()
            .find(|mode| mode.key().eq_ignore_ascii_case(key))
    }
}

/// What to draw.
pub struct TrayIconSpec {
    /// Icon edge length in pixels (16 at 100% DPI).
    pub size: i32,
    pub gpu_temp: Option<u32>,
    pub cpu_temp: Option<u32>,
    pub gpu_limits: (u32, u32),
    pub cpu_limits: (u32, u32),
    pub g_mode: bool,
    pub mode: TrayIconMode,
}

/// A top-down BGRA bitmap.
pub struct IconBitmap {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
}

impl IconBitmap {
    /// Wraps these pixels in an `HICON`. The caller owns the icon and must
    /// release it with `DestroyIcon`.
    pub fn to_hicon(&self) -> Option<HICON> {
        let size = self.width;
        let color = create_dib(size, self.height)?;
        unsafe {
            let count = (size * self.height) as usize * 4;
            std::ptr::copy_nonoverlapping(self.pixels.as_ptr(), color.bits as *mut u8, count);
        }

        // The AND mask: 1 bit per pixel, set where the icon is transparent.
        let mask_stride = ((size + 15) / 16 * 2) as usize;
        let mut mask_bits = vec![0u8; mask_stride * self.height as usize];
        for y in 0..self.height as usize {
            for x in 0..size as usize {
                let alpha = self.pixels[(y * size as usize + x) * 4 + 3];
                if alpha < 128 {
                    mask_bits[y * mask_stride + x / 8] |= 0x80 >> (x % 8);
                }
            }
        }
        let mask =
            unsafe { CreateBitmap(size, self.height, 1, 1, mask_bits.as_ptr() as *const c_void) };
        if mask.is_null() {
            // `color` is dropped here, and `Dib::drop` releases the bitmap.
            return None;
        }

        let color_bitmap = color.into_bitmap();
        let info = ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color_bitmap,
        };
        let icon = unsafe { CreateIconIndirect(&info) };
        unsafe {
            DeleteObject(mask);
            DeleteObject(color_bitmap);
        }
        if icon.is_null() {
            None
        } else {
            Some(icon)
        }
    }

    /// Composites the icon over `background` and writes a 32-bit BMP.
    /// Used by `tcc-backend icon-preview` to eyeball the rendering.
    pub fn write_bmp(&self, path: &std::path::Path, background: Rgb) -> std::io::Result<()> {
        let width = self.width as usize;
        let height = self.height as usize;
        let stride = width * 4;
        let mut pixels = vec![0u8; stride * height];
        for y in 0..height {
            for x in 0..width {
                let index = (y * width + x) * 4;
                let alpha = self.pixels[index + 3] as u32;
                for channel in 0..3 {
                    let foreground = self.pixels[index + channel] as u32;
                    let background = match channel {
                        0 => background.2 as u32,
                        1 => background.1 as u32,
                        _ => background.0 as u32,
                    };
                    pixels[index + channel] =
                        ((foreground * alpha + background * (255 - alpha)) / 255) as u8;
                }
                pixels[index + 3] = 255;
            }
        }

        let mut file = Vec::with_capacity(54 + pixels.len());
        let header_size = 14u32 + 40;
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&(header_size + pixels.len() as u32).to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&header_size.to_le_bytes());
        file.extend_from_slice(&40u32.to_le_bytes());
        file.extend_from_slice(&self.width.to_le_bytes());
        // BMP pixel rows are stored bottom-up.
        file.extend_from_slice(&self.height.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&32u16.to_le_bytes());
        file.extend_from_slice(&BI_RGB.to_le_bytes());
        file.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
        file.extend_from_slice(&0i32.to_le_bytes());
        file.extend_from_slice(&0i32.to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        for row in (0..height).rev() {
            file.extend_from_slice(&pixels[row * stride..(row + 1) * stride]);
        }
        std::fs::write(path, file)
    }
}

struct Dib {
    bitmap: HBITMAP,
    bits: *mut c_void,
}

impl Dib {
    /// Relinquishes ownership of the bitmap so the caller can destroy it.
    fn into_bitmap(mut self) -> HBITMAP {
        let bitmap = self.bitmap;
        self.bitmap = null_mut();
        bitmap
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        if !self.bitmap.is_null() {
            unsafe { DeleteObject(self.bitmap) };
        }
    }
}

fn create_dib(width: i32, height: i32) -> Option<Dib> {
    let screen = unsafe { GetDC(null_mut()) };
    if screen.is_null() {
        return None;
    }
    let memory = unsafe { CreateCompatibleDC(screen) };
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER::new(width, -height, 32),
        bmiColors: [0],
    };
    let mut bits: *mut c_void = null_mut();
    let bitmap =
        unsafe { CreateDIBSection(memory, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0) };
    unsafe {
        DeleteDC(memory);
        ReleaseDC(null_mut(), screen);
    }
    if bitmap.is_null() || bits.is_null() {
        return None;
    }
    Some(Dib { bitmap, bits })
}

/// Renders the selected tray mode; `Both` preserves the Python icon's layout.
pub fn draw(spec: &TrayIconSpec) -> Option<IconBitmap> {
    let size = spec.size.max(8);
    let screen = unsafe { GetDC(null_mut()) };
    if screen.is_null() {
        return None;
    }
    let memory = unsafe { CreateCompatibleDC(screen) };
    unsafe { ReleaseDC(null_mut(), screen) };
    if memory.is_null() {
        return None;
    }

    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER::new(size, -size, 32),
        bmiColors: [0],
    };
    let mut bits: *mut c_void = null_mut();
    let bitmap =
        unsafe { CreateDIBSection(memory, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0) };
    if bitmap.is_null() || bits.is_null() {
        unsafe { DeleteDC(memory) };
        return None;
    }
    let old_bitmap = unsafe { SelectObject(memory, bitmap) };

    let black = unsafe { CreateSolidBrush(0) };
    let full = RECT::new(0, 0, size, size);
    unsafe { FillRect(memory, &full, black) };

    let single = single_value(
        spec.mode,
        spec.gpu_temp,
        spec.cpu_temp,
        spec.gpu_limits,
        spec.cpu_limits,
    );
    let font_height = if spec.mode == TrayIconMode::Both {
        // QFont("Consolas", SIZE//2) on a 1×-DPR pixmap (96 DPI).
        let point = (size / 2).max(6);
        -(point * 96 / 72).max(8)
    } else {
        // Use the full icon for one reading, reducing the font for 3+ digits.
        let digits = single.map_or(2, |(value, _)| value.to_string().len().max(2)) as i32;
        -(size * 7 / 4 / digits).max(1)
    };
    let face = to_wide("Consolas");
    let font = unsafe {
        CreateFontW(
            font_height,
            0,
            0,
            0,
            FW_NORMAL,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_TT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            ANTIALIASED_QUALITY as u32,
            DEFAULT_PITCH as u32,
            face.as_ptr(),
        )
    };
    let old_font = unsafe { SelectObject(memory, font) };
    unsafe {
        SetBkMode(memory, TRANSPARENT);
        SetTextAlign(memory, TA_LEFT | TA_BASELINE);
    }

    if spec.mode == TrayIconMode::Both {
        draw_val(memory, spec.gpu_temp, spec.gpu_limits, size / 2 - 1);
        draw_val(memory, spec.cpu_temp, spec.cpu_limits, size);
    } else if let Some((value, limits)) = single {
        let text = to_wide(&value.to_string());
        let mut rect = full;
        unsafe {
            SetTextAlign(memory, TA_LEFT);
            SetTextColor(memory, colors::colorref(colors::heat_color(value, limits)));
            DrawTextW(
                memory,
                text.as_ptr(),
                (text.len() - 1) as i32,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
        }
    }

    if spec.g_mode {
        let white = unsafe { CreateSolidBrush(colors::colorref(colors::WHITE)) };
        let dots: &[(i32, i32)] = if size > 16 {
            &[
                (0, 0),
                (0, size - 1),
                (size - 1, 0),
                (size - 1, size - 1),
                (0, 1),
                (0, size - 2),
                (size - 1, 1),
                (size - 1, size - 2),
            ]
        } else {
            &[(0, 0), (0, size - 1), (size - 1, 0), (size - 1, size - 1)]
        };
        for (x, y) in dots {
            let dot = RECT::new(*x, *y, x + 1, y + 1);
            unsafe { FillRect(memory, &dot, white) };
        }
        unsafe { DeleteObject(white) };
    }

    let mut pixels = unsafe {
        std::slice::from_raw_parts(bits as *const u8, (size * size * 4) as usize).to_vec()
    };
    // GDI leaves alpha at zero: recover it from the colour drawn over black.
    for pixel in pixels.chunks_exact_mut(4) {
        let (b, g, r) = (pixel[0] as u32, pixel[1] as u32, pixel[2] as u32);
        let alpha = b.max(g).max(r);
        if alpha == 0 {
            continue;
        }
        for channel in pixel.iter_mut().take(3) {
            *channel = ((*channel as u32 * 255) / alpha).min(255) as u8;
        }
        pixel[3] = alpha as u8;
    }

    unsafe {
        SelectObject(memory, old_font);
        SelectObject(memory, old_bitmap);
        DeleteObject(font);
        DeleteObject(black);
        DeleteObject(bitmap);
        DeleteDC(memory);
    }

    Some(IconBitmap {
        width: size,
        height: size,
        pixels,
    })
}

/// `QPainter.drawText(x, y, text)`: `y` is the baseline, `x` is 2 or -1.
fn draw_val(hdc: HDC, value: Option<u32>, limits: (u32, u32), baseline_y: i32) {
    let Some(value) = value else { return };
    let x = if value < 100 { 2 } else { -1 };
    let text = value.to_string();
    let wide = to_wide(&text);
    unsafe {
        SetTextColor(hdc, colors::colorref(colors::heat_color(value, limits)));
        TextOutW(hdc, x, baseline_y, wide.as_ptr(), text.len() as i32);
    }
}

fn single_value(
    mode: TrayIconMode,
    gpu_temp: Option<u32>,
    cpu_temp: Option<u32>,
    gpu_limits: (u32, u32),
    cpu_limits: (u32, u32),
) -> Option<(u32, (u32, u32))> {
    match mode {
        TrayIconMode::Both => None,
        TrayIconMode::Gpu => gpu_temp.map(|value| (value, gpu_limits)),
        TrayIconMode::Cpu => cpu_temp.map(|value| (value, cpu_limits)),
        TrayIconMode::Max => match (gpu_temp, cpu_temp) {
            (Some(gpu), Some(cpu)) if cpu > gpu => Some((cpu, cpu_limits)),
            (Some(gpu), _) => Some((gpu, gpu_limits)),
            (None, Some(cpu)) => Some((cpu, cpu_limits)),
            (None, None) => None,
        },
    }
}

/// Icon size the shell uses for the notification area, in physical pixels.
pub fn best_size() -> i32 {
    let metric = unsafe { GetSystemMetrics(SM_CXSMICON) };
    let dpi = unsafe { GetDpiForSystem() };
    let from_dpi = if dpi > 0 { 16 * dpi as i32 / 96 } else { 0 };
    metric.max(from_dpi).max(16)
}

/// Small helper so callers can build a spec from live readings.
pub fn spec_for(
    size: i32,
    gpu_temp: Option<u32>,
    cpu_temp: Option<u32>,
    gpu_limits: (u32, u32),
    cpu_limits: (u32, u32),
    g_mode: bool,
    mode: TrayIconMode,
) -> TrayIconSpec {
    TrayIconSpec {
        size,
        gpu_temp,
        cpu_temp,
        gpu_limits,
        cpu_limits,
        g_mode,
        mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GPU_LIMITS: (u32, u32) = (72, 85);
    const CPU_LIMITS: (u32, u32) = (85, 95);

    #[test]
    fn tray_mode_keys_round_trip() {
        for mode in TrayIconMode::ALL {
            assert_eq!(TrayIconMode::from_key(mode.key()), Some(mode));
        }
        assert_eq!(TrayIconMode::from_key("gpu"), Some(TrayIconMode::Gpu));
        assert_eq!(TrayIconMode::from_key("unknown"), None);
    }

    #[test]
    fn single_value_mode_picks_the_requested_or_hottest_reading() {
        assert_eq!(
            single_value(
                TrayIconMode::Gpu,
                Some(70),
                Some(90),
                GPU_LIMITS,
                CPU_LIMITS
            ),
            Some((70, GPU_LIMITS))
        );
        assert_eq!(
            single_value(
                TrayIconMode::Cpu,
                Some(70),
                Some(90),
                GPU_LIMITS,
                CPU_LIMITS
            ),
            Some((90, CPU_LIMITS))
        );
        assert_eq!(
            single_value(
                TrayIconMode::Max,
                Some(70),
                Some(90),
                GPU_LIMITS,
                CPU_LIMITS
            ),
            Some((90, CPU_LIMITS))
        );
        assert_eq!(
            single_value(
                TrayIconMode::Max,
                Some(80),
                Some(70),
                GPU_LIMITS,
                CPU_LIMITS
            ),
            Some((80, GPU_LIMITS))
        );
        assert_eq!(
            single_value(TrayIconMode::Max, None, Some(70), GPU_LIMITS, CPU_LIMITS),
            Some((70, CPU_LIMITS))
        );
        assert_eq!(
            single_value(TrayIconMode::Max, None, None, GPU_LIMITS, CPU_LIMITS),
            None
        );
        assert_eq!(
            single_value(
                TrayIconMode::Both,
                Some(80),
                Some(70),
                GPU_LIMITS,
                CPU_LIMITS
            ),
            None
        );
    }

    fn opaque_pixels(icon: &IconBitmap) -> usize {
        icon.pixels.chunks(4).filter(|pixel| pixel[3] > 0).count()
    }

    fn render(mode: TrayIconMode, gpu: Option<u32>, cpu: Option<u32>) -> IconBitmap {
        draw(&spec_for(16, gpu, cpu, GPU_LIMITS, CPU_LIMITS, false, mode)).expect("icon")
    }

    #[test]
    fn rendered_modes_show_the_selected_temperature() {
        let gpu = render(TrayIconMode::Gpu, Some(59), Some(90));
        let cpu = render(TrayIconMode::Cpu, Some(59), Some(90));
        let max = render(TrayIconMode::Max, Some(59), Some(90));
        let both = render(TrayIconMode::Both, Some(59), Some(90));

        for icon in [&gpu, &cpu, &max, &both] {
            assert!(opaque_pixels(icon) > 0);
        }
        assert_ne!(gpu.pixels, cpu.pixels);
        assert_ne!(gpu.pixels, both.pixels);
        assert_ne!(cpu.pixels, both.pixels);
        assert_eq!(max.pixels, cpu.pixels);
        assert_eq!(
            gpu.pixels,
            render(TrayIconMode::Gpu, Some(59), Some(99)).pixels
        );
        assert_eq!(
            cpu.pixels,
            render(TrayIconMode::Cpu, Some(99), Some(90)).pixels
        );
        assert_eq!(
            render(TrayIconMode::Max, Some(90), Some(59)).pixels,
            render(TrayIconMode::Gpu, Some(90), Some(59)).pixels
        );
    }

    #[test]
    fn rendered_max_preserves_the_selected_sensors_color_limits() {
        // The same reading is red for GPU and yellow for CPU.
        let gpu = render(TrayIconMode::Gpu, Some(90), Some(90));
        let cpu = render(TrayIconMode::Cpu, Some(90), Some(90));
        assert_ne!(gpu.pixels, cpu.pixels);
        assert_eq!(
            render(TrayIconMode::Max, Some(90), Some(89)).pixels,
            gpu.pixels
        );
        assert_eq!(
            render(TrayIconMode::Max, Some(89), Some(90)).pixels,
            cpu.pixels
        );
    }

    #[test]
    fn rendered_modes_handle_missing_readings() {
        assert_eq!(opaque_pixels(&render(TrayIconMode::Gpu, None, Some(90))), 0);
        assert_eq!(opaque_pixels(&render(TrayIconMode::Cpu, Some(90), None)), 0);
        assert_eq!(
            render(TrayIconMode::Max, None, Some(90)).pixels,
            render(TrayIconMode::Cpu, None, Some(90)).pixels
        );
        assert_eq!(
            render(TrayIconMode::Max, Some(90), None).pixels,
            render(TrayIconMode::Gpu, Some(90), None).pixels
        );
        for mode in TrayIconMode::ALL {
            assert_eq!(opaque_pixels(&render(mode, None, None)), 0);
        }
    }

    #[test]
    fn python_layout_draws_both_temps() {
        let spec = spec_for(
            16,
            Some(59),
            Some(88),
            GPU_LIMITS,
            CPU_LIMITS,
            false,
            TrayIconMode::Both,
        );
        let icon = draw(&spec).expect("icon");
        assert!(opaque_pixels(&icon) > 0);
    }

    #[test]
    fn empty_readings_stay_transparent() {
        let spec = spec_for(
            16,
            None,
            None,
            GPU_LIMITS,
            CPU_LIMITS,
            false,
            TrayIconMode::Both,
        );
        let icon = draw(&spec).expect("icon");
        assert_eq!(opaque_pixels(&icon), 0);
    }
}
