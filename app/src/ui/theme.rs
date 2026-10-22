//! Colors and default layer appearance.

use crate::config::{parse_color, LayerConfig};
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

/// Layer names and colors: the config wins, defaults fill the gaps.
pub fn layer_table(config: &[LayerConfig]) -> (Vec<String>, Vec<Option<Color32>>) {
    let n = config.len().max(DEFAULT_LAYERS.len());
    (0..n)
        .map(|i| {
            let default = DEFAULT_LAYERS.get(i);
            match config.get(i) {
                Some(c) => {
                    let name = if c.name.is_empty() {
                        default.map(|d| d.0.to_string()).unwrap_or_else(|| format!("L{i}"))
                    } else {
                        c.name.clone()
                    };
                    (name, parse_color(&c.color).map(|(r, g, b)| Color32::from_rgb(r, g, b)))
                }
                None => default.map(|(n, c)| (n.to_string(), *c)).unwrap_or_else(|| (format!("L{i}"), None)),
            }
        })
        .unzip()
}

pub struct Palette {
    pub background: Color32,
    pub key: Color32,
    pub key_empty: Color32,
    pub key_border: Color32,
    pub pressed: Color32,
    pub pressed_text: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub neutral_layer: Color32,
    pub accent: Color32,
    pub chip_off: Color32,
    pub caps_word: Color32,
    pub colemak: Color32,
    /// Something is wrong (half of the keyboard disconnected).
    pub warning: Color32,
    /// Light theme: layer colors used as text are darkened to stay readable.
    darken_text: f32,
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
            pressed_text: Color32::from_rgb(20, 20, 24),
            text: Color32::from_rgb(235, 236, 240),
            text_dim: Color32::from_rgb(120, 122, 132),
            neutral_layer: Color32::from_rgb(150, 152, 165),
            accent: Color32::from_rgb(255, 170, 40),
            chip_off: Color32::from_rgb(70, 72, 82),
            caps_word: Color32::from_rgb(70, 110, 255),
            colemak: Color32::from_rgb(170, 90, 255),
            warning: Color32::from_rgb(235, 70, 60),
            darken_text: 0.0,
        }
    }

    pub fn light(opacity: f32) -> Self {
        let a = (opacity.clamp(0.0, 1.0) * 255.0) as u8;
        Palette {
            background: Color32::from_rgba_unmultiplied(236, 237, 241, a),
            key: Color32::from_rgb(252, 252, 253),
            key_empty: Color32::from_rgb(214, 216, 222),
            key_border: Color32::from_rgb(170, 174, 186),
            pressed: Color32::from_rgb(40, 44, 56),
            pressed_text: Color32::from_rgb(245, 245, 250),
            text: Color32::from_rgb(28, 30, 36),
            text_dim: Color32::from_rgb(130, 134, 146),
            neutral_layer: Color32::from_rgb(90, 94, 108),
            accent: Color32::from_rgb(225, 120, 0),
            chip_off: Color32::from_rgb(190, 194, 204),
            caps_word: Color32::from_rgb(40, 80, 230),
            colemak: Color32::from_rgb(140, 60, 230),
            warning: Color32::from_rgb(200, 40, 35),
            darken_text: 0.4,
        }
    }

    /// `color` adjusted for use as text on the background.
    pub fn readable(&self, color: Color32) -> Color32 {
        mix(color, Color32::BLACK, self.darken_text)
    }
}

/// Black or white text, whichever reads better on `bg`.
pub fn text_on(bg: Color32) -> Color32 {
    let l = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if l > 150.0 {
        Color32::from_rgb(20, 20, 24)
    } else {
        Color32::WHITE
    }
}

/// Mixes `a` towards `b` by `t` (0..1).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_table_merges_config_and_defaults() {
        let cfg = vec![
            LayerConfig { name: "Lettere".into(), color: "".into() },
            LayerConfig { name: "".into(), color: "#FF0000".into() },
        ];
        let (names, colors) = layer_table(&cfg);
        assert_eq!(names.len(), 6);
        assert_eq!(names[0], "Lettere");
        assert_eq!(names[1], "SYM");
        assert_eq!(colors[1], Some(Color32::from_rgb(255, 0, 0)));
        assert_eq!(names[4], "DEBUG");
        assert_eq!(colors[4], DEFAULT_LAYERS[4].1);
        assert_eq!(colors[0], None);
    }
}
