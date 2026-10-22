//! Screen geometry: the work area of a monitor, snapping to its edges while
//! dragging, quick positions and keeping the HUD on screen.
//! Everything is in egui points, like `ViewportInfo::outer_rect`.

use egui::{Pos2, Rect, Vec2};

/// Where to place the HUD in the work area: column and row, 0..=2 each
/// (left/top, middle, right/bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub col: u8,
    pub row: u8,
}

/// Work area of the monitor nearest to `at`, without the taskbar.
/// `at` and the result are in physical pixels.
#[cfg(windows)]
pub fn monitor_work_area(at: Pos2) -> Option<Rect> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    let p = POINT { x: at.x.round() as i32, y: at.y.round() as i32 };
    // SAFETY: plain Win32 calls; GetMonitorInfoW only writes the struct it is given, sized by cbSize
    unsafe {
        let monitor = MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let w = info.rcWork;
        Some(Rect::from_min_max(Pos2::new(w.left as f32, w.top as f32), Pos2::new(w.right as f32, w.bottom as f32)))
    }
}

#[cfg(not(windows))]
pub fn monitor_work_area(_at: Pos2) -> Option<Rect> {
    None
}

/// Guess of the monitor area when the system only tells its size: monitors
/// side by side, all of that size. Includes panels and taskbars.
pub fn guess_monitor_area(window: Rect, monitor: Vec2) -> Option<Rect> {
    if monitor.x <= 0.0 || monitor.y <= 0.0 {
        return None;
    }
    let c = window.center();
    let min = Pos2::new((c.x / monitor.x).floor() * monitor.x, (c.y / monitor.y).floor() * monitor.y);
    Some(Rect::from_min_size(min, monitor))
}

/// Position of a window of `size` at `anchor`, `margin` away from the edges.
pub fn anchored(anchor: Anchor, size: Vec2, area: Rect, margin: f32) -> Pos2 {
    let place = |i: u8, lo: f32, hi: f32, len: f32| match i {
        0 => lo + margin,
        1 => (lo + hi - len) / 2.0,
        _ => hi - margin - len,
    };
    Pos2::new(
        place(anchor.col, area.left(), area.right(), size.x),
        place(anchor.row, area.top(), area.bottom(), size.y),
    )
}

/// Snaps `window` to the edges or the middle of `area` when it is closer than
/// `distance`, each axis on its own. Returns the new top-left corner.
pub fn snap(window: Rect, area: Rect, distance: f32, margin: f32) -> Pos2 {
    if distance <= 0.0 {
        return window.min;
    }
    let axis = |pos: f32, len: f32, lo: f32, hi: f32| {
        [lo + margin, (lo + hi - len) / 2.0, hi - margin - len]
            .into_iter()
            .map(|t| (t, (t - pos).abs()))
            .filter(|&(_, d)| d <= distance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(pos, |(t, _)| t)
    };
    Pos2::new(
        axis(window.left(), window.width(), area.left(), area.right()),
        axis(window.top(), window.height(), area.top(), area.bottom()),
    )
}

/// Moves `window` back inside `area` (keeping `margin` from the edges when it
/// fits). A window larger than the area sticks to its top-left corner.
pub fn keep_inside(window: Rect, area: Rect, margin: f32) -> Pos2 {
    let axis = |pos: f32, len: f32, lo: f32, hi: f32| {
        let m = if len + 2.0 * margin <= hi - lo { margin } else { 0.0 };
        if pos + len > hi - m {
            (hi - m - len).max(lo)
        } else if pos < lo + m {
            lo + m
        } else {
            pos
        }
    };
    let inside = |pos: f32, len: f32, lo: f32, hi: f32| pos >= lo && pos + len <= hi;
    // already fully on screen: leave it where the user put it, even in the margin
    if inside(window.left(), window.width(), area.left(), area.right())
        && inside(window.top(), window.height(), area.top(), area.bottom())
    {
        return window.min;
    }
    Pos2::new(
        axis(window.left(), window.width(), area.left(), area.right()),
        axis(window.top(), window.height(), area.top(), area.bottom()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1040.0))
    }

    fn win(x: f32, y: f32) -> Rect {
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(600.0, 300.0))
    }

    #[test]
    fn snaps_to_nearest_edge_or_middle() {
        // near the left edge and the bottom edge
        assert_eq!(snap(win(14.0, 725.0), area(), 16.0, 8.0), Pos2::new(8.0, 732.0));
        // near the horizontal middle (660), far from any vertical target
        assert_eq!(snap(win(650.0, 400.0), area(), 16.0, 8.0), Pos2::new(660.0, 400.0));
        // too far: unchanged
        assert_eq!(snap(win(100.0, 400.0), area(), 16.0, 8.0), Pos2::new(100.0, 400.0));
        // disabled
        assert_eq!(snap(win(10.0, 10.0), area(), 0.0, 8.0), Pos2::new(10.0, 10.0));
    }

    #[test]
    fn anchors() {
        let size = Vec2::new(600.0, 300.0);
        assert_eq!(anchored(Anchor { col: 0, row: 0 }, size, area(), 8.0), Pos2::new(8.0, 8.0));
        assert_eq!(anchored(Anchor { col: 1, row: 1 }, size, area(), 8.0), Pos2::new(660.0, 370.0));
        assert_eq!(anchored(Anchor { col: 2, row: 2 }, size, area(), 8.0), Pos2::new(1312.0, 732.0));
    }

    #[test]
    fn keeps_inside() {
        // on screen, even inside the margin: untouched
        assert_eq!(keep_inside(win(2.0, 2.0), area(), 8.0), Pos2::new(2.0, 2.0));
        // off the right and bottom edges
        assert_eq!(keep_inside(win(1500.0, 900.0), area(), 8.0), Pos2::new(1312.0, 732.0));
        // on a monitor that is gone
        assert_eq!(keep_inside(win(-3000.0, 100.0), area(), 8.0), Pos2::new(8.0, 100.0));
        // larger than the area
        let big = Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::new(2000.0, 300.0));
        assert_eq!(keep_inside(big, area(), 8.0).x, 0.0);
    }

    #[test]
    fn guesses_monitor() {
        let m = Vec2::new(1920.0, 1080.0);
        assert_eq!(guess_monitor_area(win(2500.0, 100.0), m).map(|r| r.min), Some(Pos2::new(1920.0, 0.0)));
        assert_eq!(guess_monitor_area(win(0.0, 0.0), Vec2::ZERO), None);
    }
}
