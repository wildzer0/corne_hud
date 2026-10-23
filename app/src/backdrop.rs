//! Is what lies behind a part of the screen light or dark? For the heads-up
//! marker, so that it stands out on any wallpaper or window.
//!
//! A few pixels around the marker are read from the screen, averaged into
//! one brightness and thrown away: nothing is kept or saved.
//! - Windows: GDI (`BitBlt` from the screen);
//! - Linux/X11: `GetImage` on the root window;
//! - Wayland and macOS: not possible without special permissions, the
//!   result is `None` and the marker uses colours readable everywhere.

use std::time::{Duration, Instant};

const CHECK_EVERY: Duration = Duration::from_secs(2);
/// Brightness thresholds, apart so that a background near the middle does
/// not make the marker flicker.
const LIGHT_ABOVE: f32 = 0.6;
const DARK_BELOW: f32 = 0.45;

/// A rectangle on screen in physical pixels: x, y, width, height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl ScreenRect {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

pub struct Backdrop {
    imp: imp::Imp,
    last: Option<(Instant, ScreenRect)>,
    light: Option<bool>,
}

impl Backdrop {
    // `Imp` holds state on Linux only: `default()` works everywhere
    #[allow(clippy::default_constructed_unit_structs)]
    pub fn new() -> Self {
        Backdrop { imp: imp::Imp::default(), last: None, light: None }
    }

    /// Is the screen in `area`, outside `hole`, light? Read again every
    /// 2 s, or at once when the area moves. `None` when it cannot be read.
    pub fn light(&mut self, area: ScreenRect, hole: ScreenRect, now: Instant) -> Option<bool> {
        let due = self.last.is_none_or(|(t, a)| a != area || now.duration_since(t) >= CHECK_EVERY);
        if due {
            self.last = Some((now, area));
            let brightness = self.imp.grab(area).and_then(|px| brightness(&px, area, hole));
            self.light = brightness.map(|b| decide(self.light, b));
        }
        self.light
    }

    /// Forget the last reading: the next call reads the screen again.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

/// Light or dark, keeping the previous answer in the band between the thresholds.
fn decide(previous: Option<bool>, brightness: f32) -> bool {
    if brightness >= LIGHT_ABOVE {
        true
    } else if brightness <= DARK_BELOW {
        false
    } else {
        previous.unwrap_or(brightness >= (LIGHT_ABOVE + DARK_BELOW) / 2.0)
    }
}

/// Average brightness (0..1) of the RGB pixels of `area` outside `hole`.
fn brightness(pixels: &[[u8; 3]], area: ScreenRect, hole: ScreenRect) -> Option<f32> {
    if pixels.len() != (area.w * area.h).max(0) as usize {
        return None;
    }
    let (mut sum, mut n) = (0.0f32, 0u32);
    for (i, p) in pixels.iter().enumerate() {
        let (x, y) = (area.x + i as i32 % area.w, area.y + i as i32 / area.w);
        if hole.contains(x, y) {
            continue;
        }
        // perceived brightness (Rec. 601), close enough on sRGB values
        sum += (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0;
        n += 1;
    }
    (n > 0).then(|| sum / n as f32)
}

#[cfg(windows)]
mod imp {
    use super::ScreenRect;
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
    };

    #[derive(Default)]
    pub struct Imp;

    impl Imp {
        /// The pixels of `r`, row by row, as RGB.
        pub fn grab(&mut self, r: ScreenRect) -> Option<Vec<[u8; 3]>> {
            if r.w <= 0 || r.h <= 0 {
                return None;
            }
            let mut bgra = vec![0u8; (r.w * r.h * 4) as usize];
            // SAFETY: plain GDI calls; every object created is released, and
            // GetDIBits writes at most w*h 32-bit pixels into a buffer that size
            let ok = unsafe {
                let screen = GetDC(std::ptr::null_mut());
                if screen.is_null() {
                    return None;
                }
                let mem = CreateCompatibleDC(screen);
                let bitmap = CreateCompatibleBitmap(screen, r.w, r.h);
                let old = SelectObject(mem, bitmap);
                // without CAPTUREBLT: only what is drawn, as the user sees it
                let mut ok = BitBlt(mem, 0, 0, r.w, r.h, screen, r.x, r.y, SRCCOPY) != 0;
                SelectObject(mem, old);
                if ok {
                    let mut info: BITMAPINFO = std::mem::zeroed();
                    info.bmiHeader = BITMAPINFOHEADER {
                        biSize: size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: r.w,
                        biHeight: -r.h, // top-down
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB,
                        ..std::mem::zeroed()
                    };
                    let rows =
                        GetDIBits(mem, bitmap, 0, r.h as u32, bgra.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
                    ok = rows == r.h;
                }
                DeleteObject(bitmap);
                DeleteDC(mem);
                ReleaseDC(std::ptr::null_mut(), screen);
                ok
            };
            ok.then(|| bgra.chunks_exact(4).map(|p| [p[2], p[1], p[0]]).collect())
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::ScreenRect;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder, Window};
    use x11rb::rust_connection::RustConnection;

    struct X {
        conn: RustConnection,
        root: Window,
    }

    /// The X connection is opened on first use; without X, or on Wayland
    /// (where XWayland shows only its own windows), it is not tried again.
    #[derive(Default)]
    pub struct Imp {
        x: Option<X>,
        failed: bool,
    }

    fn connect() -> Option<X> {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return None;
        }
        std::env::var_os("DISPLAY")?;
        let (conn, screen) = x11rb::connect(None).ok()?;
        // the pixel decoding below expects 32-bit BGRX
        if conn.setup().image_byte_order != ImageOrder::LSB_FIRST {
            return None;
        }
        let root = conn.setup().roots.get(screen)?.root;
        Some(X { conn, root })
    }

    impl Imp {
        /// The pixels of `r`, row by row, as RGB.
        pub fn grab(&mut self, r: ScreenRect) -> Option<Vec<[u8; 3]>> {
            if self.x.is_none() && !self.failed {
                self.x = connect();
                self.failed = self.x.is_none();
            }
            let x = self.x.as_ref()?;
            let (px, py) = (i16::try_from(r.x).ok()?, i16::try_from(r.y).ok()?);
            let (w, h) = (u16::try_from(r.w).ok()?, u16::try_from(r.h).ok()?);
            // fails (BadMatch) when the area leaves the screen: no answer this time
            let reply = x.conn.get_image(ImageFormat::Z_PIXMAP, x.root, px, py, w, h, !0).ok()?.reply().ok()?;
            if !matches!(reply.depth, 24 | 32) || reply.data.len() != w as usize * h as usize * 4 {
                return None;
            }
            Some(reply.data.chunks_exact(4).map(|p| [p[2], p[1], p[0]]).collect())
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod imp {
    use super::ScreenRect;

    #[derive(Default)]
    pub struct Imp;

    impl Imp {
        pub fn grab(&mut self, _r: ScreenRect) -> Option<Vec<[u8; 3]>> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_skips_the_hole() {
        let area = ScreenRect { x: 10, y: 20, w: 3, h: 3 };
        let hole = ScreenRect { x: 11, y: 21, w: 1, h: 1 };
        // white all around, black in the middle (the marker itself)
        let mut px = vec![[255u8; 3]; 9];
        px[4] = [0; 3];
        assert_eq!(brightness(&px, area, hole), Some(1.0));
        let all = ScreenRect { x: 0, y: 0, w: 100, h: 100 };
        assert_eq!(brightness(&px, area, all), None);
        assert_eq!(brightness(&px[..8], area, hole), None);
    }

    #[test]
    fn hysteresis() {
        assert!(decide(None, 0.9));
        assert!(!decide(Some(true), 0.2));
        // in the band: the previous answer stays
        assert!(decide(Some(true), 0.5));
        assert!(!decide(Some(false), 0.55));
        assert!(!decide(None, 0.5));
    }
}
