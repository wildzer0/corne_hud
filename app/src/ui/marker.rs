//! Heads-up marker: while the HUD is faded out, a small copy of the app
//! icon (a split keyboard with column stagger) stays where the HUD shows up.
//! The two coloured keys are Fn1 and Fn2, the keys that bring the HUD up.

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Vec2};

/// Height of the marker (points).
pub const HEIGHT: f32 = 24.0;
/// Space between the marker and the bottom of the HUD (points).
pub const MARGIN: f32 = 6.0;
/// Halo around the keys, which keeps them readable on busy backgrounds.
const HALO: f32 = 0.75;

/// The keys of `packaging/icon/make_icon.py`, in its units: x, y and, for
/// the home row of the inner columns, the side (0 = Fn1, 1 = Fn2). The keys
/// are a little smaller than in the icon: at this size the gaps would vanish.
const KEY: f32 = 46.0;
const STEP: f32 = 64.0;
const SIZE: Vec2 = Vec2::new(400.0, 252.0);

fn keys() -> Vec<(f32, f32, Option<usize>)> {
    let half = 3.0 * STEP - 10.0;
    let stagger = [18.0, 0.0, 8.0]; // outer, middle, inner column
    let mut out = Vec::new();
    for side in 0..2 {
        let left = if side == 0 { 0.0 } else { half + 44.0 };
        for c in 0..3 {
            let col = if side == 0 { c } else { 2 - c };
            for r in 0..3 {
                let fn_key = (r == 1 && col == 2).then_some(side);
                out.push((left + c as f32 * STEP, stagger[col] + r as f32 * STEP, fn_key));
            }
        }
        for t in 0..2 {
            let i = if side == 0 { t + 1 } else { t };
            out.push((left + i as f32 * STEP, 3.0 * STEP + 14.0, None));
        }
    }
    out
}

/// Where the marker goes in a HUD of rectangle `hud`.
pub fn rect(hud: Rect) -> Rect {
    let size = SIZE * (HEIGHT / SIZE.y);
    Rect::from_center_size(Pos2::new(hud.center().x, hud.bottom() - MARGIN - HEIGHT / 2.0), size)
}

/// Colours of the marker: on a light background (`Some(true)`) dark keys,
/// otherwise light ones; `None` when the background is not known.
pub struct Colors {
    pub light_background: Option<bool>,
    /// Fn1 and Fn2 (the colours of the layers they bring up).
    pub fn_keys: [Color32; 2],
}

pub fn draw(painter: &Painter, at: Rect, colors: &Colors) {
    let light = Color32::from_rgb(0xE8, 0xEA, 0xF0);
    let dark = Color32::from_rgb(0x22, 0x25, 0x2E);
    let (key, halo) = match colors.light_background {
        Some(true) => (dark, light.gamma_multiply(0.35)),
        Some(false) => (light, dark.gamma_multiply(0.35)),
        // readable on both: light keys with a stronger dark halo
        None => (light, dark.gamma_multiply(0.6)),
    };
    let scale = at.height() / SIZE.y;
    let side = KEY * scale;
    let radius = |extra: f32| CornerRadius::same(((side + 2.0 * extra) * 0.22).round() as u8);
    let keys = keys();
    let rect_of = |&(x, y, _): &(f32, f32, Option<usize>)| {
        Rect::from_min_size(at.min + Vec2::new(x, y) * scale, Vec2::splat(side))
    };
    for k in &keys {
        painter.rect_filled(rect_of(k).expand(HALO), radius(HALO), halo);
    }
    for k in &keys {
        let fill = k.2.map_or(key, |i| colors.fn_keys[i]);
        painter.rect_filled(rect_of(k), radius(0.0), fill);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_fits_its_box() {
        let keys = keys();
        assert_eq!(keys.len(), 22);
        assert_eq!(keys.iter().filter(|k| k.2.is_some()).count(), 2);
        let right = keys.iter().map(|k| k.0 + KEY).fold(0.0, f32::max);
        let bottom = keys.iter().map(|k| k.1 + KEY).fold(0.0, f32::max);
        assert_eq!((right, bottom), (SIZE.x, SIZE.y));
    }
}
