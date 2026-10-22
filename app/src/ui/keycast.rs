//! Keycast for videos and presentations: shortcuts pressed (Ctrl+Shift+P,
//! Alt+Tab, F5, Esc...) shown for a moment in a corner of the screen.
//! Never text: a key that types a character shows only with Ctrl, Alt or
//! Gui held. Nothing is recorded: entries vanish after a few seconds.

use super::header::mod_label;
use super::theme::Palette;
use crate::hid::protocol::OsKind;
use crate::model::{Modifier, MODIFIERS};
use crate::vial::keycode::{decode, Keycode};
use crate::vial::legend::{legend, LegendContext};
use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How long an entry stays (the last part fades out).
pub const SHOW_TIME: Duration = Duration::from_millis(2500);
const FADE_TIME: Duration = Duration::from_millis(500);
/// A modifier may reach the state a little after its key (mod-taps decide on
/// the next key): a key still held is checked again for this long.
const PENDING_TIME: Duration = Duration::from_millis(400);
const MAX_ENTRIES: usize = 4;

/// Keys worth showing alone: F-keys, Esc, Tab, navigation, editing, media.
/// Enter, Space and Backspace are left out: they come with typing.
fn shown_alone(code: u16) -> bool {
    matches!(code,
        0x29 | 0x2B            // Esc, Tab
        | 0x3A..=0x45          // F1-F12
        | 0x46..=0x52          // PrintScreen, ScrollLock, Pause, Insert, Home, PgUp, Del, End, PgDn, arrows
        | 0x68..=0x73          // F13-F24
        | 0xA5..=0xBE) // system and media keys
}

/// Ctrl, Alt or Gui among `mods` (5-bit form, side ignored).
fn has_command_mod(mods: u8) -> bool {
    mods & (0x01 | 0x04 | 0x08) != 0
}

/// "Ctrl+Shift" from 5-bit mods, with the OS names (Cmd, Opt...).
fn mods_text(mods: u8, os: OsKind) -> String {
    let bit = |m: Modifier| match m {
        Modifier::Ctrl => 0x01,
        Modifier::Shift => 0x02,
        Modifier::Alt => 0x04,
        Modifier::Gui => 0x08,
    };
    MODIFIERS.iter().filter(|&&m| mods & bit(m) != 0).map(|&m| mod_label(m, os)).collect::<Vec<_>>().join("+")
}

/// What a press of `code` shows with `held` mods (8-bit QMK mask): a
/// shortcut, a key worth showing alone, or nothing (text, modifiers,
/// layer keys, mod-taps).
pub fn describe(code: u16, held: u8, os: OsKind, ctx: &LegendContext) -> Option<String> {
    let held = (held | held >> 4) & 0x0F;
    let (mods, kc) = match decode(code) {
        Keycode::Basic(c) if !(0xE0..=0xE7).contains(&c) => (held, c),
        Keycode::Mods { mods, kc } => (held | (mods.0 & 0x0F), kc as u16),
        _ => return None,
    };
    if !has_command_mod(mods) && !shown_alone(kc) {
        return None;
    }
    let key = legend(kc, ctx).tap.text;
    if mods == 0 {
        Some(key)
    } else {
        Some(format!("{}+{key}", mods_text(mods, os)))
    }
}

struct Entry {
    text: String,
    count: u32,
    at: Instant,
}

struct Pending {
    pos: (u8, u8),
    code: u16,
    at: Instant,
}

#[derive(Default)]
pub struct Keycast {
    entries: VecDeque<Entry>,
    pending: Vec<Pending>,
}

impl Keycast {
    fn push(&mut self, text: String, now: Instant) {
        match self.entries.back_mut() {
            Some(last) if last.text == text && now.duration_since(last.at) < SHOW_TIME => {
                last.count += 1;
                last.at = now;
            }
            _ => {
                self.entries.push_back(Entry { text, count: 1, at: now });
                while self.entries.len() > MAX_ENTRIES {
                    self.entries.pop_front();
                }
            }
        }
    }

    /// A new state: `presses` are the keys just pressed with their keycode,
    /// `pressed` tells which keys are still down, `held` the mods now.
    pub fn update(
        &mut self,
        presses: &[((u8, u8), u16)],
        pressed: impl Fn(u8, u8) -> bool,
        held: u8,
        os: OsKind,
        ctx: &LegendContext,
        now: Instant,
    ) {
        // keys waiting for a modifier: shown if it arrived while they are held
        let mut shown = Vec::new();
        self.pending.retain(|p| {
            if !pressed(p.pos.0, p.pos.1) || now.duration_since(p.at) > PENDING_TIME {
                return false;
            }
            match describe(p.code, held, os, ctx).filter(|_| has_command_mod((held | held >> 4) & 0x0F)) {
                Some(text) => {
                    shown.push(text);
                    false
                }
                None => true,
            }
        });
        for &(pos, code) in presses {
            match describe(code, held, os, ctx) {
                Some(text) => shown.push(text),
                None if matches!(decode(code), Keycode::Basic(c) if !(0xE0..=0xE7).contains(&c)) => {
                    self.pending.push(Pending { pos, code, at: now })
                }
                None => {}
            }
        }
        for text in shown {
            self.push(text, now);
        }
        self.entries.retain(|e| now.duration_since(e.at) < SHOW_TIME);
    }

    pub fn is_empty(&self, now: Instant) -> bool {
        self.entries.iter().all(|e| now.duration_since(e.at) >= SHOW_TIME)
    }

    /// Draws the entries in a row inside `rect`, aligned left, centered or
    /// right (`align` 0, 1, 2), newest last. Returns true while fading.
    pub fn draw(&self, painter: &Painter, rect: Rect, align: u8, palette: &Palette, now: Instant) -> bool {
        let size = 20.0;
        let (pad, gap) = (Vec2::new(12.0, 6.0), 8.0);
        let items: Vec<(String, f32)> = self
            .entries
            .iter()
            .filter_map(|e| {
                let age = now.duration_since(e.at);
                let left = SHOW_TIME.checked_sub(age)?;
                let alpha = (left.as_secs_f32() / FADE_TIME.as_secs_f32()).min(1.0);
                let text = if e.count > 1 { format!("{} ×{}", e.text, e.count) } else { e.text.clone() };
                Some((text, alpha))
            })
            .collect();
        let galleys: Vec<_> = items
            .iter()
            .map(|(t, alpha)| {
                painter.layout_no_wrap(t.clone(), FontId::proportional(size), palette.text.gamma_multiply(*alpha))
            })
            .collect();
        let width: f32 = galleys.iter().map(|g| g.size().x + 2.0 * pad.x + gap).sum::<f32>() - gap;
        let mut x = match align {
            0 => rect.left(),
            1 => rect.center().x - width / 2.0,
            _ => rect.right() - width,
        };
        let mut fading = false;
        for (g, (_, alpha)) in galleys.into_iter().zip(&items) {
            fading |= *alpha < 1.0;
            let chip =
                Rect::from_min_size(Pos2::new(x, rect.center().y - g.size().y / 2.0 - pad.y), g.size() + 2.0 * pad);
            let fade = |c: Color32| c.gamma_multiply(*alpha);
            painter.rect(
                chip,
                CornerRadius::same((chip.height() / 2.0) as u8),
                fade(palette.background),
                Stroke::new(1.0, fade(palette.key_border)),
                StrokeKind::Inside,
            );
            painter.galley(chip.min + pad, g, fade(palette.text));
            x = chip.right() + gap;
        }
        fading
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> LegendContext<'static> {
        LegendContext { layer_names: &[], custom: &[], tap_dances: &[], macros: &[] }
    }

    #[test]
    fn only_shortcuts_and_special_keys() {
        let c = ctx();
        let d = |code, held| describe(code, held, OsKind::Windows, &c);
        // plain text: never
        assert_eq!(d(0x06, 0), None); // C
        assert_eq!(d(0x06, 0x02), None); // Shift+C is a capital letter
        assert_eq!(d(0x28, 0), None); // Enter
                                      // shortcuts, left or right mods
        assert_eq!(d(0x06, 0x01).as_deref(), Some("Ctrl+C"));
        assert_eq!(d(0x13, 0x01 | 0x20).as_deref(), Some("Ctrl+Shift+P"));
        assert_eq!(d(0x2B, 0x04).as_deref(), Some("Alt+Tab"));
        // special keys alone, with Shift too
        assert_eq!(d(0x3E, 0).as_deref(), Some("F5"));
        assert_eq!(d(0x3E, 0x02).as_deref(), Some("Shift+F5"));
        // a key with its own mods: C(KC_Z)
        assert_eq!(d(0x011D, 0).as_deref(), Some("Ctrl+Z"));
        // modifiers and layer keys: nothing
        assert_eq!(d(0xE0, 0), None);
        assert_eq!(d(0x5221, 0), None);
        // macOS names
        assert_eq!(describe(0x06, 0x08, OsKind::MacOs, &c).as_deref(), Some("Cmd+C"));
    }

    #[test]
    fn late_modifier_and_repeats() {
        let c = ctx();
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let mut k = Keycast::default();
        // C pressed, Ctrl (a mod-tap) reaches the state a frame later
        k.update(&[((1, 3), 0x06)], |_, _| true, 0, OsKind::Linux, &c, ms(0));
        assert!(k.is_empty(ms(0)));
        k.update(&[], |_, _| true, 0x01, OsKind::Linux, &c, ms(30));
        assert_eq!(k.entries.back().map(|e| e.text.as_str()), Some("Ctrl+C"));
        // the same shortcut again: one entry, counted
        k.update(&[((1, 3), 0x06)], |_, _| true, 0x01, OsKind::Linux, &c, ms(200));
        assert_eq!((k.entries.len(), k.entries[0].count), (1, 2));
        // a letter released without a modifier: never shown
        let mut k = Keycast::default();
        k.update(&[((1, 3), 0x06)], |_, _| true, 0, OsKind::Linux, &c, ms(0));
        k.update(&[], |_, _| false, 0x01, OsKind::Linux, &c, ms(30));
        assert!(k.is_empty(ms(30)));
        // gone after a while
        let mut k = Keycast::default();
        k.update(&[((0, 0), 0x3E)], |_, _| true, 0, OsKind::Linux, &c, ms(0));
        assert!(!k.is_empty(ms(100)));
        assert!(k.is_empty(ms(3000)));
    }
}
