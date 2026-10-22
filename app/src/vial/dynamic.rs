//! Vial "dynamic entries": tap dances, combos and key overrides, configured
//! in Vial and stored in the keyboard's EEPROM. Read over Raw HID
//! (`vial_dynamic_entry_op`) or from a `.vil` backup.
//!
//! Every entry is 10 bytes, little-endian, laid out as the structs in
//! vial-qmk's `quantum/vial.h`.

use super::dump::MacroAction;
use super::names::vial_code;
use crate::i18n::pick;
use serde_json::Value;

/// Size of one entry on the wire.
pub const ENTRY_SIZE: usize = 10;

/// Key override options (`vial_ko_*` in vial.h).
const KO_ONE_MOD: u8 = 1 << 3;
const KO_ENABLED: u8 = 1 << 7;

fn words(w: [u16; 5]) -> [u8; ENTRY_SIZE] {
    let mut b = [0u8; ENTRY_SIZE];
    for (chunk, v) in b.chunks_exact_mut(2).zip(w) {
        chunk.copy_from_slice(&v.to_le_bytes());
    }
    b
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TapDance {
    pub on_tap: u16,
    pub on_hold: u16,
    pub on_double_tap: u16,
    pub on_tap_hold: u16,
    /// Tapping term in ms (0 = keyboard default).
    pub term_ms: u16,
}

impl TapDance {
    pub fn from_bytes(b: &[u8; ENTRY_SIZE]) -> Self {
        let w = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
        TapDance { on_tap: w(0), on_hold: w(2), on_double_tap: w(4), on_tap_hold: w(6), term_ms: w(8) }
    }

    pub fn to_bytes(self) -> [u8; ENTRY_SIZE] {
        words([self.on_tap, self.on_hold, self.on_double_tap, self.on_tap_hold, self.term_ms])
    }

    pub fn is_empty(&self) -> bool {
        self.actions().iter().all(|&(_, code)| code == 0)
    }

    /// The four actions with their kind, in Vial's order.
    pub fn actions(&self) -> [(TapAction, u16); 4] {
        [
            (TapAction::Tap, self.on_tap),
            (TapAction::Hold, self.on_hold),
            (TapAction::DoubleTap, self.on_double_tap),
            (TapAction::TapHold, self.on_tap_hold),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapAction {
    Tap,
    Hold,
    DoubleTap,
    TapHold,
}

impl TapAction {
    pub fn describe(self) -> &'static str {
        match self {
            TapAction::Tap => pick("tocca", "tap"),
            TapAction::Hold => pick("tieni", "hold"),
            TapAction::DoubleTap => pick("doppio tocco", "double tap"),
            TapAction::TapHold => pick("tocca e tieni", "tap and hold"),
        }
    }
}

/// Keys pressed together (up to four) that send `output`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Combo {
    pub keys: [u16; 4],
    pub output: u16,
}

impl Combo {
    pub fn from_bytes(b: &[u8; ENTRY_SIZE]) -> Self {
        let w = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
        Combo { keys: [w(0), w(2), w(4), w(6)], output: w(8) }
    }

    pub fn to_bytes(self) -> [u8; ENTRY_SIZE] {
        let [a, b, c, d] = self.keys;
        words([a, b, c, d, self.output])
    }

    /// Input keycodes actually used.
    pub fn inputs(&self) -> impl Iterator<Item = u16> + '_ {
        self.keys.iter().copied().filter(|&k| k != 0)
    }

    /// A combo fires only with two keys or more and an output.
    pub fn is_active(&self) -> bool {
        self.output != 0 && self.inputs().count() >= 2
    }
}

/// `trigger` with `trigger_mods` down sends `replacement` instead.
/// Modifier masks use the 8-bit HID layout (bits 0-3 left Ctrl, Shift, Alt,
/// Gui; bits 4-7 the right ones).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyOverride {
    pub trigger: u16,
    pub replacement: u16,
    /// Layers where the override applies, bit per layer.
    pub layers: u16,
    pub trigger_mods: u8,
    pub negative_mods: u8,
    pub suppressed_mods: u8,
    pub options: u8,
}

impl KeyOverride {
    pub fn from_bytes(b: &[u8; ENTRY_SIZE]) -> Self {
        let w = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
        KeyOverride {
            trigger: w(0),
            replacement: w(2),
            layers: w(4),
            trigger_mods: b[6],
            negative_mods: b[7],
            suppressed_mods: b[8],
            options: b[9],
        }
    }

    pub fn to_bytes(self) -> [u8; ENTRY_SIZE] {
        let mut b = words([self.trigger, self.replacement, self.layers, 0, 0]);
        b[6..].copy_from_slice(&[self.trigger_mods, self.negative_mods, self.suppressed_mods, self.options]);
        b
    }

    pub fn is_active(&self) -> bool {
        self.options & KO_ENABLED != 0 && self.trigger != 0
    }

    /// Whether the override fires with `mods` held on `layer` (same rules as
    /// QMK's `key_override_matches_active_modifiers`).
    pub fn matches(&self, mods: u8, layer: u8) -> bool {
        if !self.is_active() || layer >= 16 || self.layers & (1 << layer) == 0 {
            return false;
        }
        if self.negative_mods & mods != 0 {
            return false;
        }
        if self.trigger_mods == 0 {
            return true;
        }
        if self.options & KO_ONE_MOD != 0 {
            return self.trigger_mods & mods != 0;
        }
        // every required mod, on either side when both sides are listed
        let one_side = |m: u8| (m & 0x0F) | (m >> 4);
        one_side(self.trigger_mods & mods) == one_side(self.trigger_mods)
    }

    /// Modifiers to hold, as a 5-bit QMK set for display (right side only
    /// when no left mod is listed).
    pub fn mods5(&self) -> u8 {
        let (left, right) = (self.trigger_mods & 0x0F, self.trigger_mods >> 4);
        if left != 0 {
            left
        } else {
            right | 0x10
        }
    }
}

/// Everything configured in Vial, beyond the keymap.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dynamic {
    /// Indexed by tap dance number (`TD(n)`), empty entries included.
    pub tap_dances: Vec<TapDance>,
    /// Active combos only.
    pub combos: Vec<Combo>,
    /// Enabled key overrides only.
    pub key_overrides: Vec<KeyOverride>,
    /// Dynamic macros, indexed by number (`M0`...). Read through VIA, not
    /// with the other entries.
    pub macros: Vec<Vec<MacroAction>>,
}

impl Dynamic {
    /// Builds from the raw entries, dropping unused combos and overrides.
    pub fn new(tap_dances: Vec<TapDance>, combos: Vec<Combo>, key_overrides: Vec<KeyOverride>) -> Self {
        Dynamic {
            tap_dances,
            combos: combos.into_iter().filter(Combo::is_active).collect(),
            key_overrides: key_overrides.into_iter().filter(KeyOverride::is_active).collect(),
            macros: Vec::new(),
        }
    }

    pub fn tap_dance(&self, n: u8) -> Option<&TapDance> {
        self.tap_dances.get(n as usize).filter(|t| !t.is_empty())
    }

    /// Replacement for `code` with `mods` held on `layer`, if an override fires.
    pub fn override_for(&self, code: u16, mods: u8, layer: u8) -> Option<&KeyOverride> {
        self.key_overrides.iter().find(|o| o.trigger == code && o.matches(mods, layer))
    }

    /// Reads the `tap_dance`, `combo`, `key_override` and `macro` sections of a `.vil`
    /// (missing sections are empty). Unknown keycode names become `KC_NO`.
    pub fn from_vil(v: &Value) -> Self {
        let code = |v: &Value| -> u16 {
            match v {
                Value::String(s) => vial_code(s).unwrap_or(0),
                Value::Number(n) => n.as_u64().and_then(|n| u16::try_from(n).ok()).unwrap_or(0),
                _ => 0,
            }
        };
        let list = |key: &str| v.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
        let int = |e: &Value, k: &str| e.get(k).and_then(Value::as_u64).unwrap_or(0);

        let tap_dances = list("tap_dance")
            .iter()
            .map(|e| {
                let a = e.as_array().map(Vec::as_slice).unwrap_or(&[]);
                let at = |i: usize| a.get(i).map_or(0, code);
                let term = a.get(4).and_then(Value::as_u64).and_then(|n| u16::try_from(n).ok()).unwrap_or(0);
                TapDance { on_tap: at(0), on_hold: at(1), on_double_tap: at(2), on_tap_hold: at(3), term_ms: term }
            })
            .collect();
        let combos = list("combo")
            .iter()
            .map(|e| {
                let a = e.as_array().map(Vec::as_slice).unwrap_or(&[]);
                let at = |i: usize| a.get(i).map_or(0, code);
                Combo { keys: [at(0), at(1), at(2), at(3)], output: at(4) }
            })
            .collect();
        let key_overrides = list("key_override")
            .iter()
            .map(|e| KeyOverride {
                trigger: e.get("trigger").map_or(0, code),
                replacement: e.get("replacement").map_or(0, code),
                layers: int(e, "layers") as u16,
                trigger_mods: int(e, "trigger_mods") as u8,
                negative_mods: int(e, "negative_mod_mask") as u8,
                suppressed_mods: int(e, "suppressed_mods") as u8,
                options: int(e, "options") as u8,
            })
            .collect();
        let macros = list("macro")
            .iter()
            .map(|m| {
                m.as_array().map_or(&[][..], Vec::as_slice).iter().filter_map(|a| macro_action(a, &code)).collect()
            })
            .collect();
        Dynamic { macros, ..Dynamic::new(tap_dances, combos, key_overrides) }
    }
}

/// One macro step as vial-gui saves it: `["text", "..."]`, `["tap", keys...]`,
/// `["down", ...]`, `["up", ...]`, `["delay", ms]`.
fn macro_action(v: &Value, code: &dyn Fn(&Value) -> u16) -> Option<MacroAction> {
    let a = v.as_array()?;
    let keys = || a[1..].iter().map(code).collect();
    Some(match a.first()?.as_str()? {
        "text" => MacroAction::Text(a.get(1)?.as_str()?.to_string()),
        "tap" => MacroAction::Tap(keys()),
        "down" => MacroAction::Down(keys()),
        "up" => MacroAction::Up(keys()),
        "delay" => MacroAction::Delay(a.get(1)?.as_u64()? as u32),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIFT_BKSP_DEL: KeyOverride = KeyOverride {
        trigger: 0x2A,
        replacement: 0x4C,
        layers: 0xFFFF,
        trigger_mods: 0x22,
        negative_mods: 0,
        suppressed_mods: 0x22,
        options: 0x87,
    };

    #[test]
    fn entries_from_bytes() {
        let td = TapDance::from_bytes(&[0x04, 0, 0xE0, 0, 0x29, 0, 0, 0, 200, 0]);
        assert_eq!((td.on_tap, td.on_hold, td.on_double_tap, td.on_tap_hold, td.term_ms), (4, 0xE0, 0x29, 0, 200));
        let c = Combo::from_bytes(&[0x0D, 0, 0x0E, 0, 0, 0, 0, 0, 0x29, 0]);
        assert_eq!((c.inputs().collect::<Vec<_>>(), c.output), (vec![0x0D, 0x0E], 0x29));
        let ko = KeyOverride::from_bytes(&[0x2A, 0, 0x4C, 0, 0xFF, 0xFF, 0x22, 0, 0x22, 0x87]);
        assert_eq!(ko, SHIFT_BKSP_DEL);
        assert_eq!(TapDance::from_bytes(&td.to_bytes()), td);
        assert_eq!(Combo::from_bytes(&c.to_bytes()), c);
        assert_eq!(KeyOverride::from_bytes(&ko.to_bytes()), ko);
    }

    #[test]
    fn key_override_modifiers() {
        let ko = SHIFT_BKSP_DEL;
        // either shift is enough when both sides are listed
        assert!(ko.matches(0x02, 0));
        assert!(ko.matches(0x20, 3));
        assert!(ko.matches(0x03, 0)); // extra mods are fine without a negative mask
        assert!(!ko.matches(0x01, 0));
        assert!(!ko.matches(0, 0));
        // Ctrl+Shift, all required
        let cs = KeyOverride { trigger_mods: 0x03, options: KO_ENABLED, ..ko };
        assert!(!cs.matches(0x02, 0));
        assert!(cs.matches(0x03, 0));
        assert!(!cs.matches(0x12, 0)); // right Ctrl does not count: only the left side is listed
        let both = KeyOverride { trigger_mods: 0x33, ..cs };
        assert!(both.matches(0x12, 0)); // right Ctrl + left Shift
                                        // one mod is enough
        assert!(KeyOverride { options: KO_ENABLED | KO_ONE_MOD, ..cs }.matches(0x02, 0));
        // negative mask and layers
        assert!(!KeyOverride { negative_mods: 0x04, ..ko }.matches(0x06, 0));
        assert!(!KeyOverride { layers: 0b10, ..ko }.matches(0x02, 0));
        // disabled
        assert!(!KeyOverride { options: 0x07, ..ko }.matches(0x02, 0));
        assert_eq!(ko.mods5(), 0x02);
        assert_eq!(KeyOverride { trigger_mods: 0x40, ..ko }.mods5(), 0x14);
    }

    #[test]
    fn vil_sections() {
        let d = Dynamic::from_vil(&serde_json::from_str(include_str!("../../data/mock/corne.vil")).unwrap());
        assert_eq!(d.tap_dances.len(), 32);
        assert!(d.combos.is_empty());
        assert_eq!(d.key_overrides, vec![SHIFT_BKSP_DEL]);

        let v = serde_json::json!({
            "tap_dance": [["KC_A", "KC_LCTRL", "KC_ESCAPE", "KC_NO", 180]],
            "combo": [["KC_J", "KC_K", "KC_NO", "KC_NO", "KC_ESCAPE"], ["KC_J", "KC_NO", "KC_NO", "KC_NO", "KC_X"]],
            "macro": [[["text", "hi"], ["tap", "KC_ENTER"]], [["down", "KC_LCTRL"], ["tap", 6], ["up", "KC_LCTRL"], ["delay", 30]], []],
        });
        let d = Dynamic::from_vil(&v);
        let td = d.tap_dance(0).unwrap();
        assert_eq!((td.on_tap, td.on_hold, td.on_double_tap, td.term_ms), (0x04, 0xE0, 0x29, 180));
        assert_eq!(d.tap_dance(1), None);
        assert_eq!(d.macros.len(), 3);
        assert_eq!(d.macros[0], vec![MacroAction::Text("hi".into()), MacroAction::Tap(vec![0x28])]);
        assert_eq!(
            d.macros[1],
            vec![
                MacroAction::Down(vec![0xE0]),
                MacroAction::Tap(vec![0x06]),
                MacroAction::Up(vec![0xE0]),
                MacroAction::Delay(30)
            ]
        );
        // a combo needs two keys
        assert_eq!(d.combos, vec![Combo { keys: [0x0D, 0x0E, 0, 0], output: 0x29 }]);
    }
}
