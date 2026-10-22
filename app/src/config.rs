//! Configuration (`keyboard-hud.toml`, reloaded on change) and remembered
//! window state (`keyboard-hud-state.toml`, written by the app so that the
//! user's commented config file is never rewritten).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    System,
    Dark,
    Light,
}

/// Interface language: the system's (Italian or English) or a fixed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageChoice {
    #[default]
    Auto,
    It,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    #[default]
    Full,
    Compact,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceConfig {
    /// Vendor ID, e.g. "0x4653". Empty = any keyboard answering GET_INFO.
    pub vid: Option<String>,
    pub pid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub opacity: f32,
    pub scale: f32,
    pub theme: ThemeChoice,
    pub mode: ViewMode,
    /// Initial window position (logical pixels); the last position is remembered anyway.
    pub x: Option<f32>,
    pub y: Option<f32>,
    /// While dragging, the HUD snaps to screen edges and middle closer than this (0 = off).
    pub snap_distance: f32,
    /// Gap between the HUD and the screen edges, for snapping and quick positions.
    pub screen_margin: f32,
    /// Icon in the system tray.
    pub tray: bool,
    /// Style (file name in `styles/`, without `.toml`); empty = the default one.
    pub style: String,
    pub language: LanguageChoice,
    /// Opacity of the keys, 0.2–1; unset = from the style.
    pub key_opacity: Option<f32>,
    /// Corner radius of the keys, 0–0.5 of their size; unset = from the style.
    pub key_radius: Option<f32>,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            opacity: 0.85,
            scale: 1.0,
            theme: ThemeChoice::System,
            mode: ViewMode::Full,
            x: None,
            y: None,
            snap_distance: 16.0,
            screen_margin: 8.0,
            tray: true,
            style: String::new(),
            language: LanguageChoice::Auto,
            key_opacity: None,
            key_radius: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayerConfig {
    pub name: String,
    /// "#RRGGBB", or empty for a neutral layer.
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Polling {
    /// While the HUD is visible (or in heads-up mode).
    pub visible_hz: u32,
    /// While hidden.
    pub hidden_hz: u32,
    pub timeout_ms: u64,
}

impl Default for Polling {
    fn default() -> Self {
        Polling { visible_hz: 60, hidden_hz: 10, timeout_ms: 50 }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LogConfig {
    /// error, warn, info, debug
    pub level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig { level: "info".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HeadsUp {
    pub enabled: bool,
    /// A modifier must be held this long before the HUD appears.
    pub modifier_delay_ms: u64,
    /// The HUD fades out this long after the keyboard is back to the base state.
    pub hide_delay_ms: u64,
    /// Opacity while faded out (0 = invisible).
    pub hidden_opacity: f32,
    /// What a locked layer (TG, Layer Lock) brings up.
    pub locked_layer: ShownIn,
    /// What a pending one-shot (OSL, one-shot mods) brings up.
    pub oneshot: ShownIn,
    /// What a modifier held past `modifier_delay_ms` brings up.
    pub modifiers: ShownIn,
}

impl Default for HeadsUp {
    fn default() -> Self {
        HeadsUp {
            enabled: true,
            modifier_delay_ms: 400,
            hide_delay_ms: 800,
            hidden_opacity: 0.0,
            locked_layer: ShownIn::Hud,
            oneshot: ShownIn::Hud,
            modifiers: ShownIn::Hud,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShownIn {
    /// The whole HUD stays on screen.
    #[default]
    Hud,
    /// Only the mini indicator.
    Indicator,
}

/// Screen corner, edge middle or centre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScreenPlace {
    TopLeft,
    Top,
    #[default]
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl ScreenPlace {
    /// `(column, row)`, 0..=2 each.
    pub fn cell(self) -> (u8, u8) {
        match self {
            ScreenPlace::TopLeft => (0, 0),
            ScreenPlace::Top => (1, 0),
            ScreenPlace::TopRight => (2, 0),
            ScreenPlace::Left => (0, 1),
            ScreenPlace::Center => (1, 1),
            ScreenPlace::Right => (2, 1),
            ScreenPlace::BottomLeft => (0, 2),
            ScreenPlace::Bottom => (1, 2),
            ScreenPlace::BottomRight => (2, 2),
        }
    }
}

/// Mini indicator shown while the HUD is hidden.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IndicatorConfig {
    pub enabled: bool,
    pub position: ScreenPlace,
}

impl Default for IndicatorConfig {
    fn default() -> Self {
        IndicatorConfig { enabled: true, position: ScreenPlace::TopRight }
    }
}

/// Shortcuts shown on screen, for videos and presentations.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeycastConfig {
    pub enabled: bool,
    pub position: ScreenPlace,
}

impl Default for KeycastConfig {
    fn default() -> Self {
        KeycastConfig { enabled: false, position: ScreenPlace::Bottom }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BackupConfig {
    pub enabled: bool,
    /// Folder; empty = `backups` next to the config. Relative paths too.
    pub dir: String,
    /// Backups kept per keyboard.
    pub keep: usize,
}

impl Default for BackupConfig {
    fn default() -> Self {
        BackupConfig { enabled: true, dir: String::new(), keep: 30 }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StatsConfig {
    pub enabled: bool,
    /// Counter file; empty = `keyboard-hud-stats.json` next to the config.
    pub file: String,
    /// Report: keys never used in the last N days.
    pub unused_days: u32,
}

impl Default for StatsConfig {
    fn default() -> Self {
        StatsConfig { enabled: true, file: String::new(), unused_days: 30 }
    }
}

/// Custom key label, by position (`layer` + `row` + `col`) or by `keycode`
/// (optionally limited to one `layer`).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LabelConfig {
    pub layer: Option<u8>,
    pub row: Option<u8>,
    pub col: Option<u8>,
    pub keycode: Option<String>,
    pub label: String,
    /// Longer description shown in the tooltip.
    pub description: Option<String>,
    /// Only while this application (or one of these) is in the foreground.
    pub app: Option<AppNames>,
}

/// `app = "code"` or `app = ["code", "idea64"]`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum AppNames {
    One(String),
    Many(Vec<String>),
}

impl AppNames {
    pub fn names(&self) -> Vec<String> {
        match self {
            AppNames::One(n) => vec![n.clone()],
            AppNames::Many(v) => v.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub device: DeviceConfig,
    pub appearance: Appearance,
    pub headsup: HeadsUp,
    pub indicator: IndicatorConfig,
    pub keycast: KeycastConfig,
    pub layers: Vec<LayerConfig>,
    pub labels: Vec<LabelConfig>,
    pub stats: StatsConfig,
    pub backup: BackupConfig,
    pub polling: Polling,
    pub log: LogConfig,
}

pub fn parse_hex_id(s: &str) -> Option<u16> {
    let s = s.trim();
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    u16::from_str_radix(hex, 16).ok()
}

/// "#RRGGBB" → (r, g, b).
pub fn parse_color(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut c: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        c.sanitize();
        Ok(c)
    }

    fn sanitize(&mut self) {
        let a = &mut self.appearance;
        a.opacity = if a.opacity.is_finite() { a.opacity.clamp(0.2, 1.0) } else { 0.85 };
        a.scale = if a.scale.is_finite() { a.scale.clamp(0.5, 2.0) } else { 1.0 };
        a.snap_distance = if a.snap_distance.is_finite() { a.snap_distance.clamp(0.0, 200.0) } else { 16.0 };
        a.screen_margin = if a.screen_margin.is_finite() { a.screen_margin.clamp(0.0, 200.0) } else { 8.0 };
        a.key_opacity = a.key_opacity.map(crate::style::clamp_key_opacity);
        a.key_radius = a.key_radius.map(crate::style::clamp_key_radius);
        a.style = a.style.trim().to_string();
        let p = &mut self.polling;
        p.visible_hz = p.visible_hz.clamp(1, 250);
        p.hidden_hz = p.hidden_hz.clamp(1, 250);
        p.timeout_ms = p.timeout_ms.clamp(5, 1000);
        let h = &mut self.headsup;
        h.hidden_opacity = if h.hidden_opacity.is_finite() { h.hidden_opacity.clamp(0.0, 1.0) } else { 0.0 };
        h.modifier_delay_ms = h.modifier_delay_ms.min(10_000);
        h.hide_delay_ms = h.hide_delay_ms.min(60_000);
        self.stats.unused_days = self.stats.unused_days.clamp(1, 3650);
        self.backup.keep = self.backup.keep.clamp(1, 1000);
    }

    pub fn vid(&self) -> Option<u16> {
        self.device.vid.as_deref().and_then(parse_hex_id)
    }

    pub fn pid(&self) -> Option<u16> {
        self.device.pid.as_deref().and_then(parse_hex_id)
    }
}

/// Watches the config file and reloads it when its modification time changes.
pub struct ConfigWatcher {
    path: PathBuf,
    mtime: Option<SystemTime>,
    last_check: Instant,
}

pub enum Reload {
    Unchanged,
    Changed(Box<Config>),
    Error(String),
}

impl ConfigWatcher {
    /// Loads the initial config (defaults when the file is missing or invalid).
    pub fn new(path: PathBuf) -> (Self, Config, Option<String>) {
        let mtime = mtime(&path);
        let (config, error) = match std::fs::read_to_string(&path) {
            Ok(text) => match Config::parse(&text) {
                Ok(c) => (c, None),
                Err(e) => (Config::default(), Some(e)),
            },
            Err(_) => (Config::default(), None),
        };
        (ConfigWatcher { path, mtime, last_check: Instant::now() }, config, error)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Checks at most once per second.
    pub fn poll(&mut self) -> Reload {
        if self.last_check.elapsed() < Duration::from_secs(1) {
            return Reload::Unchanged;
        }
        self.last_check = Instant::now();
        let m = mtime(&self.path);
        if m == self.mtime {
            return Reload::Unchanged;
        }
        self.mtime = m;
        match std::fs::read_to_string(&self.path) {
            Ok(text) => match Config::parse(&text) {
                Ok(c) => Reload::Changed(Box::new(c)),
                Err(e) => Reload::Error(e),
            },
            // file removed: back to defaults
            Err(_) => Reload::Changed(Box::default()),
        }
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Window state remembered between runs.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct WindowState {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub scale: Option<f32>,
    pub opacity: Option<f32>,
    pub mode: Option<ViewMode>,
    pub theme: Option<ThemeChoice>,
    pub headsup: Option<bool>,
    pub combos: Option<bool>,
    pub style: Option<String>,
    pub language: Option<LanguageChoice>,
    pub keycast: Option<bool>,
    pub key_opacity: Option<f32>,
    pub key_radius: Option<f32>,
}

impl WindowState {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        let text = match toml::to_string(self) {
            Ok(t) => t,
            Err(e) => return log::warn!("stato della finestra: {e}"),
        };
        let header = "# Scritto da Keyboard HUD: posizione e aspetto dell'ultima sessione.\n";
        if let Err(e) = std::fs::write(path, format!("{header}{text}")) {
            log::warn!("{}: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../../keyboard-hud.example.toml");

    #[test]
    fn example_config_parses() {
        let c = Config::parse(EXAMPLE).unwrap();
        assert_eq!(c.layers.len(), 6);
        assert_eq!(c.layers[1].name, "SYM");
        assert_eq!(parse_color(&c.layers[1].color), Some((0x00, 0xB9, 0xD7)));
        assert_eq!(c.polling.visible_hz, 60);
        assert!(c.headsup.enabled);
        assert_eq!(c.headsup.locked_layer, ShownIn::Hud);
        assert_eq!(c.headsup.oneshot, ShownIn::Hud);
        assert_eq!(c.headsup.modifiers, ShownIn::Hud);
        assert_eq!(c.indicator.position, ScreenPlace::TopRight);
        assert!(!c.keycast.enabled);
        assert_eq!(c.appearance.language, LanguageChoice::Auto);
        assert_eq!(c.keycast.position, ScreenPlace::Bottom);
        assert_eq!(c.headsup.modifier_delay_ms, 400);
        assert!(c.labels.iter().any(|l| l.label == "Step over" && l.keycode.as_deref() == Some("KC_F10")));
        assert!(c
            .labels
            .iter()
            .all(|l| crate::vial::names::parse_keycode(l.keycode.as_deref().unwrap_or("KC_NO")).is_some()));
    }

    #[test]
    fn defaults_and_clamping() {
        let c = Config::parse("").unwrap();
        assert_eq!(c, Config::default());
        let c = Config::parse("[appearance]\nopacity = 5.0\nscale = 0.1\n[polling]\nvisible_hz = 0").unwrap();
        assert_eq!((c.appearance.opacity, c.appearance.scale, c.polling.visible_hz), (1.0, 0.5, 1));
        let c = Config::parse("[appearance]\nkey_opacity = 0.0\nkey_radius = 2.0\nstyle = \" vetro \"").unwrap();
        assert_eq!((c.appearance.key_opacity, c.appearance.key_radius), (Some(0.2), Some(0.5)));
        assert_eq!(c.appearance.style, "vetro");
    }

    #[test]
    fn invalid_config_is_an_error() {
        assert!(Config::parse("[appearance]\nopacity = \"tanta\"").is_err());
        assert!(Config::parse("[nope]\nx = 1").is_err());
        assert!(Config::parse("not toml at all [").is_err());
    }

    #[test]
    fn ids_and_colors() {
        assert_eq!(parse_hex_id("0x4653"), Some(0x4653));
        assert_eq!(parse_hex_id("0004"), Some(4));
        assert_eq!(parse_hex_id("xyz"), None);
        assert_eq!(parse_color("#FF8000"), Some((255, 128, 0)));
        assert_eq!(parse_color(""), None);
        assert_eq!(parse_color("#12345"), None);
    }

    #[test]
    fn window_state_round_trip() {
        let s = WindowState {
            x: Some(10.0),
            y: Some(20.5),
            scale: Some(1.25),
            mode: Some(ViewMode::Compact),
            ..Default::default()
        };
        let text = toml::to_string(&s).unwrap();
        assert_eq!(toml::from_str::<WindowState>(&text).unwrap(), s);
    }
}
