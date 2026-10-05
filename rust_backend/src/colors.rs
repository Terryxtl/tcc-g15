//! Palette from `GUI/AppColors.py`.

pub type Rgb = (u8, u8, u8);

pub const DARK_GREY: Rgb = (0x20, 0x20, 0x20);
pub const GREY: Rgb = (0x50, 0x50, 0x50);
pub const WHITE: Rgb = (0xDD, 0xDD, 0xDD);
pub const GREEN: Rgb = (0x34, 0xA8, 0x53);
pub const YELLOW: Rgb = (0xF1, 0xC2, 0x32);
pub const RED: Rgb = (0xF4, 0x43, 0x36);
pub const BLUE: Rgb = (0x1A, 0x64, 0x97);
pub const BORDER: Rgb = (0x80, 0x80, 0x80);

/// GDI stores colors as `0x00BBGGRR`.
pub fn colorref((r, g, b): Rgb) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

/// Bars turn yellow and then red as the reading approaches the limits.
pub fn heat_color(value: u32, limits: (u32, u32)) -> Rgb {
    if value >= limits.1 {
        RED
    } else if value >= limits.0 {
        YELLOW
    } else {
        GREEN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_limits_map_to_the_expected_colors() {
        assert_eq!(heat_color(60, (72, 85)), GREEN);
        assert_eq!(heat_color(72, (72, 85)), YELLOW);
        assert_eq!(heat_color(84, (72, 85)), YELLOW);
        assert_eq!(heat_color(85, (72, 85)), RED);
    }

    #[test]
    fn colorref_is_bgr() {
        assert_eq!(colorref((0xFF, 0x00, 0x00)), 0x0000_00FF);
        assert_eq!(colorref((0x1A, 0x64, 0x97)), 0x0097_641A);
    }
}
