//! Colors and default layer appearance.

use egui::Color32;

/// Default layer names and colors, matching the firmware palette
/// (SYM cyan, NAV green, SYS neutral, DEBUG red, ACC yellow).
pub const DEFAULT_LAYERS: [(&str, Option<Color32>); 6] = [
    ("BASE", None),
    ("SYM", Some(Color32::from_rgb(0, 185, 215))),
    ("NAV", Some(Color32::from_rgb(60, 200, 90))),
    ("SYS", None),
    ("DEBUG", Some(Color32::from_rgb(230, 60, 60))),
    ("ACC", Some(Color32::from_rgb(235, 200, 40))),
];

pub struct Palette {
    pub background: Color32,
    pub key: Color32,
    pub key_empty: Color32,
    pub key_border: Color32,
    pub pressed: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub neutral_layer: Color32,
}

impl Palette {
    pub fn dark(opacity: f32) -> Self {
        let a = (opacity.clamp(0.0, 1.0) * 255.0) as u8;
        Palette {
            background: Color32::from_rgba_unmultiplied(18, 18, 22, a),
            key: Color32::from_rgb(48, 50, 58),
            key_empty: Color32::from_rgb(28, 29, 34),
            key_border: Color32::from_rgb(78, 82, 94),
            pressed: Color32::from_rgb(240, 240, 250),
            text: Color32::from_rgb(235, 236, 240),
            text_dim: Color32::from_rgb(120, 122, 132),
            neutral_layer: Color32::from_rgb(130, 132, 145),
        }
    }
}

/// Mixes `a` towards `b` by `t` (0..1).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}
