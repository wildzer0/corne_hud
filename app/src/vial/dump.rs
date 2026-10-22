//! Snapshot of everything Vial stores in the keyboard, written as a `.vil`
//! that vial-gui can load (*File → Load saved layout*).
//!
//! Keycodes are written as numbers: vial-gui's `Keycode.deserialize` accepts
//! them as they are, and they need no name table.

use super::dynamic::{Combo, KeyOverride, TapDance};
use crate::model::Keymap;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// Alt repeat key entry (`vial_alt_repeat_key_entry_t`, 6 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AltRepeat {
    pub keycode: u16,
    pub alt_keycode: u16,
    pub allowed_mods: u8,
    pub options: u8,
}

impl AltRepeat {
    pub fn from_bytes(b: &[u8]) -> Self {
        AltRepeat {
            keycode: u16::from_le_bytes([b[0], b[1]]),
            alt_keycode: u16::from_le_bytes([b[2], b[3]]),
            allowed_mods: b[4],
            options: b[5],
        }
    }
}

/// One step of a macro, as vial-gui saves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroAction {
    Text(String),
    Tap(Vec<u16>),
    Down(Vec<u16>),
    Up(Vec<u16>),
    Delay(u32),
}

impl MacroAction {
    fn to_json(&self) -> Value {
        let keys = |tag: &str, k: &[u16]| {
            let mut v = vec![json!(tag)];
            v.extend(k.iter().map(|&c| json!(c)));
            Value::Array(v)
        };
        match self {
            MacroAction::Text(t) => json!(["text", t]),
            MacroAction::Tap(k) => keys("tap", k),
            MacroAction::Down(k) => keys("down", k),
            MacroAction::Up(k) => keys("up", k),
            MacroAction::Delay(ms) => json!(["delay", ms]),
        }
    }
}

/// Size in bytes of each QMK setting (`protos` in vial-qmk's
/// `quantum/qmk_settings.c`): the answer does not say, and the rest of the
/// buffer still holds the request. Unknown ids are not backed up.
pub fn qmk_setting_width(qsid: u16) -> Option<usize> {
    match qsid {
        1 | 3 | 5 | 20 | 22 | 23 | 24 | 26 => Some(1),
        2 | 4 | 6 | 7 | 9..=19 | 25 | 27 => Some(2),
        21 => Some(4),
        _ => None,
    }
}

const SS_QMK_PREFIX: u8 = 1;
const SS_TAP: u8 = 1;
const SS_DOWN: u8 = 2;
const SS_UP: u8 = 3;
const SS_DELAY: u8 = 4;
const EXT_TAP: u8 = 5;
const EXT_DOWN: u8 = 6;
const EXT_UP: u8 = 7;

/// Splits the macro buffer into `count` macros and decodes them, following
/// `dynamic_keymap_macro_send` in vial-qmk.
pub fn decode_macros(buffer: &[u8], count: usize) -> Vec<Vec<MacroAction>> {
    let mut macros = Vec::with_capacity(count);
    let mut rest = buffer;
    for _ in 0..count {
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        macros.push(decode_macro(&rest[..end]));
        rest = rest.get(end + 1..).unwrap_or(&[]);
    }
    macros
}

fn decode_macro(bytes: &[u8]) -> Vec<MacroAction> {
    let mut out = Vec::new();
    let mut text = Vec::new();
    let flush = |text: &mut Vec<u8>, out: &mut Vec<MacroAction>| {
        if !text.is_empty() {
            out.push(MacroAction::Text(String::from_utf8_lossy(text).into_owned()));
            text.clear();
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != SS_QMK_PREFIX {
            text.push(bytes[i]);
            i += 1;
            continue;
        }
        flush(&mut text, &mut out);
        let Some(&kind) = bytes.get(i + 1) else { break };
        let action = match kind {
            SS_TAP | SS_DOWN | SS_UP => {
                let Some(&kc) = bytes.get(i + 2) else { break };
                i += 3;
                key_action(kind, kc as u16)
            }
            EXT_TAP | EXT_DOWN | EXT_UP => {
                let (Some(&lo), Some(&hi)) = (bytes.get(i + 2), bytes.get(i + 3)) else { break };
                i += 4;
                // 0xFF01 stands for 0x0100: a zero byte would end the macro
                let raw = u16::from_le_bytes([lo, hi]);
                let kc = if raw > 0xFF00 { (raw & 0xFF) << 8 } else { raw };
                key_action(kind - (EXT_TAP - SS_TAP), kc)
            }
            SS_DELAY => {
                let (Some(&d0), Some(&d1)) = (bytes.get(i + 2), bytes.get(i + 3)) else { break };
                i += 4;
                MacroAction::Delay((d0 as u32).saturating_sub(1) + (d1 as u32).saturating_sub(1) * 255)
            }
            _ => break,
        };
        out.push(action);
    }
    flush(&mut text, &mut out);
    out
}

fn key_action(kind: u8, kc: u16) -> MacroAction {
    match kind {
        SS_DOWN => MacroAction::Down(vec![kc]),
        SS_UP => MacroAction::Up(vec![kc]),
        _ => MacroAction::Tap(vec![kc]),
    }
}

/// Everything a `.vil` holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Dump {
    pub uid: u64,
    pub vial_protocol: u32,
    pub via_protocol: u16,
    pub layout_options: u32,
    /// Raw keymap (as Vial sees it, without the HUD's Colemak remap).
    pub keymap: Keymap,
    /// Matrix positions that exist in the layout; the others are written as -1.
    pub present: Vec<(u8, u8)>,
    /// `[layer][encoder] = [counter-clockwise, clockwise]`.
    pub encoders: Vec<Vec<[u16; 2]>>,
    pub macros: Vec<Vec<MacroAction>>,
    /// Every entry, empty ones included (their index matters).
    pub tap_dances: Vec<TapDance>,
    pub combos: Vec<Combo>,
    pub key_overrides: Vec<KeyOverride>,
    pub alt_repeat: Vec<AltRepeat>,
    /// QMK settings by id.
    pub settings: BTreeMap<u16, u32>,
}

impl Dump {
    pub fn to_vil(&self) -> Value {
        let km = &self.keymap;
        let layout: Vec<Value> = (0..km.layers())
            .map(|l| {
                let rows: Vec<Value> = (0..km.rows())
                    .map(|r| {
                        let cols: Vec<Value> = (0..km.cols())
                            .map(|c| if self.present.contains(&(r, c)) { json!(km.get(l, r, c)) } else { json!(-1) })
                            .collect();
                        Value::Array(cols)
                    })
                    .collect();
                Value::Array(rows)
            })
            .collect();
        let encoders: Vec<Value> = self
            .encoders
            .iter()
            .map(|layer| Value::Array(layer.iter().map(|e| json!([e[0], e[1]])).collect()))
            .collect();
        let macros: Vec<Value> =
            self.macros.iter().map(|m| Value::Array(m.iter().map(MacroAction::to_json).collect())).collect();
        let tap_dance: Vec<Value> = self
            .tap_dances
            .iter()
            .map(|t| json!([t.on_tap, t.on_hold, t.on_double_tap, t.on_tap_hold, t.term_ms]))
            .collect();
        let combo: Vec<Value> =
            self.combos.iter().map(|c| json!([c.keys[0], c.keys[1], c.keys[2], c.keys[3], c.output])).collect();
        let key_override: Vec<Value> = self
            .key_overrides
            .iter()
            .map(|o| {
                json!({
                    "trigger": o.trigger,
                    "replacement": o.replacement,
                    "layers": o.layers,
                    "trigger_mods": o.trigger_mods,
                    "negative_mod_mask": o.negative_mods,
                    "suppressed_mods": o.suppressed_mods,
                    "options": o.options,
                })
            })
            .collect();
        let alt_repeat: Vec<Value> = self
            .alt_repeat
            .iter()
            .map(|a| {
                json!({
                    "keycode": a.keycode,
                    "alt_keycode": a.alt_keycode,
                    "allowed_mods": a.allowed_mods,
                    "options": a.options,
                })
            })
            .collect();
        let settings: Map<String, Value> = self.settings.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        json!({
            "version": 1,
            "uid": self.uid,
            "layout": layout,
            "encoder_layout": encoders,
            "layout_options": self.layout_options,
            "macro": macros,
            "vial_protocol": self.vial_protocol,
            "via_protocol": self.via_protocol,
            "tap_dance": tap_dance,
            "combo": combo,
            "key_override": key_override,
            "alt_repeat_key": alt_repeat,
            "settings": settings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vial::dynamic::Dynamic;
    use crate::vial::vil::Vil;

    #[test]
    fn macros_from_buffer() {
        let buf = [
            b'h', b'i', // text
            1, 1, 0x04, // tap A
            1, 4, 101, 1, // delay 100 ms
            1, 5, 0x29, 0x02, // tap Shift+F... 16-bit 0x0229
            1, 5, 0x02, 0xFF, // 0xFF02 stands for 0x0200
            1, 2, 0xE1, 1, 3, 0xE1, // down / up LShift
            0,    // end of macro 0
            0,    // macro 1 empty
            b'x', 0, // macro 2
            0, 0, 0, // padding
        ];
        let m = decode_macros(&buf, 4);
        assert_eq!(
            m[0],
            vec![
                MacroAction::Text("hi".into()),
                MacroAction::Tap(vec![0x04]),
                MacroAction::Delay(100),
                MacroAction::Tap(vec![0x0229]),
                MacroAction::Tap(vec![0x0200]),
                MacroAction::Down(vec![0xE1]),
                MacroAction::Up(vec![0xE1]),
            ]
        );
        assert!(m[1].is_empty());
        assert_eq!(m[2], vec![MacroAction::Text("x".into())]);
        assert!(m[3].is_empty());
        // delays above 254 ms use the second byte
        assert_eq!(decode_macro(&[1, 4, 6, 3]), vec![MacroAction::Delay(5 + 2 * 255)]);
        // truncated: no panic
        assert_eq!(decode_macro(&[1, 5, 0x29]), vec![]);
    }

    #[test]
    fn vil_round_trip() {
        // the user's .vil, through Vil + Dynamic, back to .vil and again
        let text = include_str!("../../data/mock/corne.vil");
        let v: Value = serde_json::from_str(text).unwrap();
        let vil = Vil::parse(text, 8, 7).unwrap();
        let present: Vec<(u8, u8)> = (0..8u8)
            .flat_map(|r| (0..7u8).map(move |c| (r, c)))
            .filter(|&(r, c)| v["layout"][0][r as usize][c as usize] != json!(-1))
            .collect();
        let d = Dynamic::from_vil(&v);
        let dump = Dump {
            uid: vil.uid.unwrap(),
            vial_protocol: 6,
            via_protocol: 9,
            layout_options: 0,
            keymap: vil.keymap.clone(),
            present,
            encoders: vec![vec![[0x0081, 0x0080]; 4]; 6],
            macros: vec![Vec::new(); 16],
            tap_dances: d.tap_dances.clone(),
            combos: vec![Combo::default(); 32],
            key_overrides: d.key_overrides.clone(),
            alt_repeat: vec![AltRepeat::default(); 32],
            settings: BTreeMap::from([(2, 50), (7, 200)]),
        };
        let out = dump.to_vil();
        let again = Vil::parse(&out.to_string(), 8, 7).unwrap();
        assert_eq!(again.keymap, vil.keymap);
        assert_eq!(again.dynamic, d);
        assert_eq!(out["layout"][0][3], json!([-1, -1, -1, 0xE2, 0x7C77, 0x2C, -1]));
        assert_eq!(out["macro"].as_array().unwrap().len(), 16);
        assert_eq!(out["settings"]["7"], json!(200));
        assert_eq!(out["tap_dance"][0], json!([0, 0, 0, 0, 200]));
    }
}
