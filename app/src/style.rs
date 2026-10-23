//! Styles: visual themes, one `.toml` file each in `styles/` next to the
//! configuration (the file name is the style name). Four are built in; a
//! file with the same name replaces one. A style does not choose between
//! dark and light: it has colors for both and follows the theme.

use crate::config::parse_color;
use crate::i18n::pick;
use crate::paths;
use crate::tr_format;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

pub const DEFAULT_STYLE: &str = "predefinito";

const BUILT_IN: [(&str, &str); 4] = [
    ("predefinito", include_str!("../data/styles/predefinito.toml")),
    ("minimal", include_str!("../data/styles/minimal.toml")),
    ("vetro", include_str!("../data/styles/vetro.toml")),
    ("alto-contrasto", include_str!("../data/styles/alto-contrasto.toml")),
];

/// Palette colors a style can set.
pub const COLOR_NAMES: [&str; 14] = [
    "background",
    "key",
    "key_empty",
    "key_border",
    "pressed",
    "pressed_text",
    "text",
    "text_dim",
    "neutral_layer",
    "accent",
    "chip_off",
    "caps_word",
    "colemak",
    "warning",
];

/// Color name → "#RRGGBB".
pub type StyleColors = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Style {
    /// Opacity of the keys, 0.2–1 (the legends stay opaque).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_opacity: Option<f32>,
    /// Corner radius, 0–0.5 of the key size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_radius: Option<f32>,
    /// Key border in pixels, 0 = none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_width: Option<f32>,
    /// Legend size, 0.7–1.5.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend_scale: Option<f32>,
    /// Corners of the HUD window, in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_radius: Option<f32>,
    /// Applied when the style is chosen, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_opacity: Option<f32>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub dark: StyleColors,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub light: StyleColors,
}

/// Shape values, resolved: what the drawing code uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub key_opacity: f32,
    pub key_radius: f32,
    pub border_width: f32,
    pub legend_scale: f32,
    pub window_radius: f32,
}

impl Default for Look {
    fn default() -> Self {
        Look { key_opacity: 1.0, key_radius: 0.0, border_width: 1.0, legend_scale: 1.0, window_radius: 12.0 }
    }
}

/// Accepted ranges of the key shape, also offered by the settings window.
pub const KEY_OPACITY: std::ops::RangeInclusive<f32> = 0.2..=1.0;
pub const KEY_RADIUS: std::ops::RangeInclusive<f32> = 0.0..=0.5;

pub fn clamp_key_opacity(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(*KEY_OPACITY.start(), *KEY_OPACITY.end())
    } else {
        1.0
    }
}

pub fn clamp_key_radius(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(*KEY_RADIUS.start(), *KEY_RADIUS.end())
    } else {
        0.0
    }
}

impl Style {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut s: Style = toml::from_str(text).map_err(|e| e.to_string())?;
        for (section, colors) in [("dark", &s.dark), ("light", &s.light)] {
            for (name, value) in colors {
                if !COLOR_NAMES.contains(&name.as_str()) {
                    return Err(tr_format!(
                        "[{section}] {name}: colore sconosciuto",
                        "[{section}] {name}: unknown color"
                    ));
                }
                if parse_color(value).is_none() {
                    return Err(tr_format!(
                        "[{section}] {name} = \"{value}\": serve \"#RRGGBB\"",
                        "[{section}] {name} = \"{value}\": \"#RRGGBB\" expected"
                    ));
                }
            }
        }
        let fix = |v: &mut Option<f32>, f: &dyn Fn(f32) -> f32| *v = v.map(f);
        fix(&mut s.key_opacity, &clamp_key_opacity);
        fix(&mut s.key_radius, &clamp_key_radius);
        fix(&mut s.border_width, &|v| if v.is_finite() { v.clamp(0.0, 6.0) } else { 1.0 });
        fix(&mut s.legend_scale, &|v| if v.is_finite() { v.clamp(0.7, 1.5) } else { 1.0 });
        fix(&mut s.window_radius, &|v| if v.is_finite() { v.clamp(0.0, 40.0) } else { 12.0 });
        fix(&mut s.window_opacity, &|v| if v.is_finite() { v.clamp(0.2, 1.0) } else { 0.85 });
        Ok(s)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    /// Shape values, with the key opacity and radius set elsewhere (menu,
    /// config) winning over the style's.
    pub fn look_with(&self, key_opacity: Option<f32>, key_radius: Option<f32>) -> Look {
        let look = self.look();
        Look {
            key_opacity: key_opacity.unwrap_or(look.key_opacity),
            key_radius: key_radius.unwrap_or(look.key_radius),
            ..look
        }
    }

    /// Shape values, missing ones from the defaults.
    pub fn look(&self) -> Look {
        let d = Look::default();
        Look {
            key_opacity: self.key_opacity.unwrap_or(d.key_opacity),
            key_radius: self.key_radius.unwrap_or(d.key_radius),
            border_width: self.border_width.unwrap_or(d.border_width),
            legend_scale: self.legend_scale.unwrap_or(d.legend_scale),
            window_radius: self.window_radius.unwrap_or(d.window_radius),
        }
    }

    pub fn colors(&self, dark: bool) -> &StyleColors {
        if dark {
            &self.dark
        } else {
            &self.light
        }
    }
}

pub fn dir() -> PathBuf {
    paths::base_dir().join("styles")
}

fn file(name: &str) -> PathBuf {
    dir().join(format!("{name}.toml"))
}

fn built_in(name: &str) -> Option<&'static str> {
    BUILT_IN.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

/// Style names, built-in and from the folder, sorted.
pub fn names() -> Vec<String> {
    let mut names: Vec<String> = BUILT_IN.iter().map(|(n, _)| n.to_string()).collect();
    if let Ok(entries) = std::fs::read_dir(dir()) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "toml") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Loads a style: the user's file first, then the built-in one.
pub fn load(name: &str) -> Result<Style, String> {
    match std::fs::read_to_string(file(name)) {
        Ok(text) => Style::parse(&text).map_err(|e| format!("{}: {e}", file(name).display())),
        Err(_) => match built_in(name) {
            Some(text) => Style::parse(text),
            None => {
                Err(tr_format!("stile «{name}» non trovato in {}", "style “{name}” not found in {}", dir().display()))
            }
        },
    }
}

/// Writes `style` as a new file `<base>.toml`, `<base>-2.toml`... never
/// over an existing one. Returns the name used.
pub fn save_new(base: &str, style: &Style) -> std::io::Result<String> {
    std::fs::create_dir_all(dir())?;
    let name = (1..)
        .map(|i| if i == 1 { base.to_string() } else { format!("{base}-{i}") })
        .find(|n| !file(n).exists() && built_in(n).is_none())
        .unwrap_or_else(|| base.to_string());
    let header = pick(
        "# Stile di Keyboard HUD: ogni voce è facoltativa, i colori sono \"#RRGGBB\".\n",
        "# Keyboard HUD style: every entry is optional, colors are \"#RRGGBB\".\n",
    );
    std::fs::write(file(&name), format!("{header}{}", style.to_toml()))?;
    Ok(name)
}

/// Watches the file of the style in use: changes apply without a restart.
pub struct StyleWatcher {
    name: String,
    mtime: Option<SystemTime>,
    last_check: Instant,
}

impl StyleWatcher {
    pub fn new(name: &str) -> Self {
        StyleWatcher { name: name.to_string(), mtime: mtime(name), last_check: Instant::now() }
    }

    /// Checks at most once per second; `Some` when the file changed.
    pub fn poll(&mut self) -> Option<Result<Style, String>> {
        if self.last_check.elapsed() < Duration::from_secs(1) {
            return None;
        }
        self.last_check = Instant::now();
        let m = mtime(&self.name);
        if m == self.mtime {
            return None;
        }
        self.mtime = m;
        Some(load(&self.name))
    }
}

fn mtime(name: &str) -> Option<SystemTime> {
    std::fs::metadata(file(name)).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_styles_parse() {
        for (name, text) in BUILT_IN {
            let s = Style::parse(text).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(s.key_radius.is_some(), "{name}");
        }
        assert_eq!(Style::parse(built_in("vetro").unwrap()).unwrap().look().key_opacity, 0.5);
    }

    #[test]
    fn invalid_values() {
        assert!(Style::parse("nope = 1").is_err());
        assert!(Style::parse("[dark]\nkey = \"rosso\"").is_err());
        assert!(Style::parse("[dark]\nsfondo = \"#000000\"").is_err());
        let s = Style::parse("key_radius = 3.0\nkey_opacity = 0.0\nlegend_scale = 9").unwrap();
        assert_eq!((s.key_radius, s.key_opacity, s.legend_scale), (Some(0.5), Some(0.2), Some(1.5)));
        // missing values: defaults
        assert_eq!(Style::default().look(), Look::default());
        // menu or config values win over the style
        let vetro = Style::parse(built_in("vetro").unwrap()).unwrap();
        let look = vetro.look_with(Some(0.9), None);
        assert_eq!((look.key_opacity, look.key_radius), (0.9, 0.22));
    }

    #[test]
    fn round_trip() {
        let mut s = Style::parse(built_in("alto-contrasto").unwrap()).unwrap();
        s.window_opacity = Some(0.7);
        assert_eq!(Style::parse(&s.to_toml()).unwrap(), s);
    }
}
