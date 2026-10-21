//! Header: active layers (held / locked / one-shot), modifiers, Caps Lock /
//! Caps Word, QWERTY / Colemak-DH, host OS.

use super::theme::{mix, text_on, Palette};
use crate::hid::protocol::{flags, HudState, OsKind};
use crate::i18n::{tr, Lang, Msg};
use crate::model::{mod_level, LayerHold, ModLevel, Modifier, MODIFIERS};
use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

#[derive(Clone, Copy)]
enum ChipStyle {
    Off,
    /// Solid fill: active.
    Filled(Color32),
    /// Pale fill: weak (transient) state.
    Soft(Color32),
    /// Pulsing outline: one-shot waiting for the next key.
    Pulse(Color32),
}

pub struct HeaderView<'a> {
    pub lang: Lang,
    /// Highest layer shown, and the classified active layers (highest first).
    pub layer: u8,
    pub layers: &'a [(u8, LayerHold)],
    pub layer_name: &'a dyn Fn(u8) -> String,
    pub layer_color: &'a dyn Fn(u8) -> Option<Color32>,
    pub state: Option<&'a HudState>,
    pub compact: bool,
    /// Seconds, for the one-shot pulse.
    pub time: f64,
}

/// Padlock icon (no font has a reliable glyph for it), `h` tall, centered on `center`.
fn draw_lock(painter: &Painter, center: Pos2, h: f32, color: Color32) {
    let body = Rect::from_center_size(center + Vec2::new(0.0, h * 0.18), Vec2::new(h * 0.72, h * 0.52));
    painter.rect_filled(body, CornerRadius::same((h * 0.1) as u8), color);
    // shackle: half circle above the body
    let r = h * 0.22;
    let c = Pos2::new(center.x, body.top());
    let points: Vec<Pos2> = (0..=12)
        .map(|i| {
            let a = std::f32::consts::PI * (1.0 + i as f32 / 12.0);
            c + Vec2::new(r * a.cos(), r * a.sin() * 1.2)
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(h * 0.11, color)));
}

/// Text suffix for one-shot layers.
const ONE_SHOT_MARK: &str = "1×";

/// Lays chips out left to right on one row.
struct Chips<'p> {
    painter: &'p Painter,
    palette: &'p Palette,
    x: f32,
    y: f32,
    size: f32,
    gap: f32,
    time: f64,
}

impl Chips<'_> {
    fn add(&mut self, text: &str, style: ChipStyle) {
        self.add_with(text, style, false);
    }

    /// Adds a chip, optionally followed by a padlock inside it.
    fn add_with(&mut self, text: &str, style: ChipStyle, lock: bool) {
        let (painter, palette, size) = (self.painter, self.palette, self.size);
        let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(size), palette.text);
        let pad = Vec2::new(size * 0.55, size * 0.25);
        let icon = if lock { size * 0.95 } else { 0.0 };
        let rect = Rect::from_min_size(
            Pos2::new(self.x, self.y - galley.size().y / 2.0 - pad.y),
            galley.size() + pad * 2.0 + Vec2::new(icon, 0.0),
        );
        let radius = CornerRadius::same((rect.height() / 2.0) as u8);
        let (fill, stroke, text_color) = match style {
            ChipStyle::Off => (Color32::TRANSPARENT, Stroke::new(1.0, palette.chip_off), palette.chip_off),
            ChipStyle::Filled(c) => (c, Stroke::NONE, text_on(c)),
            ChipStyle::Soft(c) => (c.gamma_multiply(0.35), Stroke::new(1.0, c.gamma_multiply(0.7)), palette.text),
            ChipStyle::Pulse(c) => {
                let a = 0.55 + 0.45 * ((self.time * 6.0).sin() as f32 + 1.0) / 2.0;
                (c.gamma_multiply(0.15), Stroke::new(2.0, c.gamma_multiply(a)), palette.text)
            }
        };
        painter.rect(rect, radius, fill, stroke, StrokeKind::Inside);
        let text = painter.layout_no_wrap(text.to_string(), FontId::proportional(size), text_color);
        let text_right = rect.min.x + pad.x + text.size().x;
        painter.galley(rect.min + pad, text, text_color);
        if lock {
            draw_lock(painter, Pos2::new(text_right + icon * 0.55, rect.center().y), size * 0.85, text_color);
        }
        self.x += rect.width() + self.gap;
    }

    fn space(&mut self) {
        self.x += self.gap;
    }
}

fn mod_label(m: Modifier, os: OsKind) -> &'static str {
    match (m, os) {
        (Modifier::Ctrl, _) => "Ctrl",
        (Modifier::Shift, _) => "Shift",
        (Modifier::Alt, OsKind::MacOs) => "Opt",
        (Modifier::Alt, _) => "Alt",
        (Modifier::Gui, OsKind::MacOs) => "Cmd",
        (Modifier::Gui, OsKind::Windows) => "Win",
        (Modifier::Gui, OsKind::Linux) => "Super",
        (Modifier::Gui, OsKind::Auto) => "Gui",
    }
}

fn os_name(os: OsKind) -> &'static str {
    match os {
        OsKind::Windows => "Windows",
        OsKind::MacOs => "macOS",
        OsKind::Linux => "Linux",
        OsKind::Auto => "?",
    }
}

/// Draws the header in `rect` (one row). Returns true while something animates.
pub fn draw(painter: &Painter, rect: Rect, view: &HeaderView, palette: &Palette) -> bool {
    let y = rect.center().y;
    let big = if view.compact { 16.0 } else { 20.0 };
    let small = if view.compact { 11.0 } else { 12.5 };
    let gap = if view.compact { 5.0 } else { 7.0 };
    let mut x = rect.left();
    let mut animating = false;

    // main layer: name in its color, with the lock / one-shot mark
    let main_how = view.layers.iter().find(|(l, _)| *l == view.layer).map(|(_, h)| *h);
    let color = (view.layer_color)(view.layer).map(|c| palette.readable(c)).unwrap_or(palette.neutral_layer);
    let mut name = (view.layer_name)(view.layer);
    if main_how == Some(LayerHold::OneShot) {
        name = format!("{name} {ONE_SHOT_MARK}");
        animating = true;
    }
    let r = painter.text(Pos2::new(x, y), Align2::LEFT_CENTER, name, FontId::proportional(big), color);
    x = r.right() + gap * 2.0;
    if main_how == Some(LayerHold::Locked) {
        draw_lock(painter, Pos2::new(r.right() + big * 0.55, y), big * 0.8, color);
        x += big * 0.9;
    }
    let mut chips = Chips { painter, palette, x, y, size: small, gap, time: view.time };

    // other active layers below the main one (e.g. DEBUG locked while holding Fn1)
    for &(l, how) in view.layers.iter().filter(|(l, _)| *l != view.layer) {
        let c = (view.layer_color)(l).unwrap_or(palette.neutral_layer);
        let mut text = (view.layer_name)(l);
        if how == LayerHold::OneShot {
            text = format!("{text} {ONE_SHOT_MARK}");
        }
        let style = if how == LayerHold::OneShot { ChipStyle::Pulse(c) } else { ChipStyle::Soft(c) };
        chips.add_with(&text, style, how == LayerHold::Locked);
        animating |= how == LayerHold::OneShot;
    }

    let Some(s) = view.state else { return animating };
    chips.space();

    for m in MODIFIERS {
        let level = mod_level(s, m);
        let style = match level {
            ModLevel::Off => ChipStyle::Off,
            ModLevel::Weak => ChipStyle::Soft(palette.accent),
            ModLevel::OneShot => ChipStyle::Pulse(palette.accent),
            ModLevel::OneShotLocked | ModLevel::Held => ChipStyle::Filled(palette.accent),
        };
        animating |= level == ModLevel::OneShot;
        if view.compact && level == ModLevel::Off {
            continue;
        }
        chips.add_with(mod_label(m, s.active_os), style, level == ModLevel::OneShotLocked);
    }
    chips.space();

    let lang = view.lang;
    if s.flag(flags::CAPS_WORD) {
        chips.add(tr(lang, Msg::CapsWord), ChipStyle::Filled(palette.caps_word));
    }
    if s.flag(flags::CAPS_LOCK) {
        chips.add(tr(lang, Msg::CapsLock), ChipStyle::Filled(palette.text));
    }
    let neutral = ChipStyle::Soft(mix(palette.chip_off, palette.text, 0.5));
    if s.flag(flags::COLEMAK) {
        chips.add(tr(lang, Msg::Colemak), ChipStyle::Soft(palette.colemak));
    } else if !view.compact {
        chips.add(tr(lang, Msg::Qwerty), neutral);
    }
    if !view.compact {
        let how = if s.os_mode == OsKind::Auto { tr(lang, Msg::OsAuto) } else { tr(lang, Msg::OsForced) };
        chips.add(&format!("{} · {how}", os_name(s.active_os)), neutral);
    }
    animating
}
