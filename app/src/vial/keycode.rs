//! Keycode decoding (QMK keycode set used by Vial protocol 6).
//!
//! Range bases are kept as constants for fast decoding; a unit test checks
//! them against the ranges generated from vial-qmk's hjson files.

use super::names;

pub const KC_NO: u16 = 0x0000;
pub const KC_TRNS: u16 = 0x0001;

const QK_MODS: u16 = 0x0100;
const QK_MODS_MAX: u16 = 0x1FFF;
const QK_MOD_TAP: u16 = 0x2000;
const QK_MOD_TAP_MAX: u16 = 0x3FFF;
const QK_LAYER_TAP: u16 = 0x4000;
const QK_LAYER_TAP_MAX: u16 = 0x4FFF;
const QK_LAYER_MOD: u16 = 0x5000;
const QK_LAYER_MOD_MAX: u16 = 0x51FF;
const QK_TO: u16 = 0x5200;
const QK_MOMENTARY: u16 = 0x5220;
const QK_DEF_LAYER: u16 = 0x5240;
const QK_TOGGLE_LAYER: u16 = 0x5260;
const QK_ONE_SHOT_LAYER: u16 = 0x5280;
const QK_ONE_SHOT_MOD: u16 = 0x52A0;
const QK_LAYER_TAP_TOGGLE: u16 = 0x52C0;
const QK_PERSISTENT_DEF_LAYER: u16 = 0x52E0;
const QK_TAP_DANCE: u16 = 0x5700;
const QK_MACRO: u16 = 0x7700;
pub const QK_TRI_LAYER_LOWER: u16 = 0x7C77;
pub const QK_TRI_LAYER_UPPER: u16 = 0x7C78;
pub const QK_KB_0: u16 = 0x7E00;
const QK_KB_MAX: u16 = 0x7E3F;

/// Layers used by `TL_LOWR` / `TL_UPPR` (QMK defaults, same as the keymap).
pub const TRI_LAYER_LOWER: u8 = 1;
pub const TRI_LAYER_UPPER: u8 = 2;
pub const TRI_LAYER_ADJUST: u8 = 3;

/// 5-bit QMK modifier set as encoded in keycodes: bits 0-3 = Ctrl, Shift,
/// Alt, Gui; bit 4 = right-hand side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mods5(pub u8);

impl Mods5 {
    pub fn is_right(self) -> bool {
        self.0 & 0x10 != 0
    }

    /// Human readable form, e.g. `Ctrl+Shift`.
    pub fn display(self) -> String {
        let side = if self.is_right() { "R" } else { "" };
        let m = self.0 & 0x0F;
        let named = match m {
            0x0F => Some("Hyper"),
            0x07 => Some("Meh"),
            _ => None,
        };
        if let Some(n) = named {
            return format!("{side}{n}");
        }
        let parts: Vec<&str> = [(0x01, "Ctrl"), (0x02, "Shift"), (0x04, "Alt"), (0x08, "Gui")]
            .into_iter()
            .filter(|(bit, _)| m & bit != 0)
            .map(|(_, n)| n)
            .collect();
        format!("{side}{}", parts.join("+"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerOp {
    To,
    Momentary,
    DefaultLayer,
    Toggle,
    OneShot,
    TapToggle,
    PersistentDefault,
    TriLayerLower,
    TriLayerUpper,
}

impl LayerOp {
    pub fn prefix(self) -> &'static str {
        match self {
            LayerOp::To => "TO",
            LayerOp::Momentary => "MO",
            LayerOp::DefaultLayer => "DF",
            LayerOp::Toggle => "TG",
            LayerOp::OneShot => "OSL",
            LayerOp::TapToggle => "TT",
            LayerOp::PersistentDefault => "PDF",
            LayerOp::TriLayerLower => "TL_LOWR",
            LayerOp::TriLayerUpper => "TL_UPPR",
        }
    }
}

/// Structured view of a 16-bit keycode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keycode {
    No,
    Transparent,
    /// Basic HID keycode (0x04-0xFF) or any other code with a name in the QMK table.
    Basic(u16),
    /// Modifiers + basic key, e.g. `C(KC_Z)`.
    Mods {
        mods: Mods5,
        kc: u8,
    },
    /// Mod-tap, e.g. `LCTL_T(KC_TAB)`.
    ModTap {
        mods: Mods5,
        kc: u8,
    },
    LayerTap {
        layer: u8,
        kc: u8,
    },
    LayerMod {
        layer: u8,
        mods: Mods5,
    },
    Layer {
        op: LayerOp,
        layer: u8,
    },
    OneShotMod(Mods5),
    TapDance(u8),
    Macro(u8),
    /// Keyboard-level custom keycode (`QK_KB_n`, Vial `USERnn`).
    Custom(u8),
    Unknown(u16),
}

pub fn decode(code: u16) -> Keycode {
    match code {
        KC_NO => Keycode::No,
        KC_TRNS => Keycode::Transparent,
        0x0002..=0x00FF => Keycode::Basic(code),
        QK_MODS..=QK_MODS_MAX => Keycode::Mods { mods: Mods5((code >> 8) as u8 & 0x1F), kc: code as u8 },
        QK_MOD_TAP..=QK_MOD_TAP_MAX => Keycode::ModTap { mods: Mods5((code >> 8) as u8 & 0x1F), kc: code as u8 },
        QK_LAYER_TAP..=QK_LAYER_TAP_MAX => Keycode::LayerTap { layer: ((code >> 8) & 0x0F) as u8, kc: code as u8 },
        QK_LAYER_MOD..=QK_LAYER_MOD_MAX => {
            Keycode::LayerMod { layer: ((code >> 5) & 0x0F) as u8, mods: Mods5(code as u8 & 0x1F) }
        }
        0x5200..=0x52FF => {
            let layer = (code & 0x1F) as u8;
            let op = match code & 0xFFE0 {
                QK_TO => LayerOp::To,
                QK_MOMENTARY => LayerOp::Momentary,
                QK_DEF_LAYER => LayerOp::DefaultLayer,
                QK_TOGGLE_LAYER => LayerOp::Toggle,
                QK_ONE_SHOT_LAYER => LayerOp::OneShot,
                QK_ONE_SHOT_MOD => return Keycode::OneShotMod(Mods5(layer)),
                QK_LAYER_TAP_TOGGLE => LayerOp::TapToggle,
                QK_PERSISTENT_DEF_LAYER => LayerOp::PersistentDefault,
                _ => return Keycode::Unknown(code),
            };
            Keycode::Layer { op, layer }
        }
        QK_TAP_DANCE..=0x57FF => Keycode::TapDance(code as u8),
        QK_MACRO..=0x77FF => Keycode::Macro(code as u8),
        QK_TRI_LAYER_LOWER => Keycode::Layer { op: LayerOp::TriLayerLower, layer: TRI_LAYER_LOWER },
        QK_TRI_LAYER_UPPER => Keycode::Layer { op: LayerOp::TriLayerUpper, layer: TRI_LAYER_UPPER },
        QK_KB_0..=QK_KB_MAX => Keycode::Custom((code - QK_KB_0) as u8),
        _ if names::qmk_by_code(code).is_some() => Keycode::Basic(code),
        _ => Keycode::Unknown(code),
    }
}

/// Canonical QMK-style name, e.g. `LCTL_T(KC_TAB)`, `MO(1)`, `QK_KB_3`.
pub fn qmk_name(code: u16) -> String {
    fn basic(kc: u8) -> String {
        names::qmk_by_code(kc as u16).map(|e| e.short_name().to_string()).unwrap_or_else(|| format!("{kc:#04x}"))
    }
    fn mods_fn(m: Mods5, suffix: &str) -> String {
        let side = if m.is_right() { "R" } else { "L" };
        let name = match m.0 & 0x0F {
            0x01 => "CTL",
            0x02 => "SFT",
            0x04 => "ALT",
            0x08 => "GUI",
            0x03 => "CS",
            0x05 => "CA",
            0x09 => "CG",
            0x06 => "SA",
            0x0C => "AG",
            0x0A => "SG",
            0x0D => "CAG",
            0x07 => return format!("MEH{suffix}"),
            0x0F => return if suffix.is_empty() { "HYPR".into() } else { "ALL_T".into() },
            _ => return format!("MOD{:#04x}{suffix}", m.0),
        };
        format!("{side}{name}{suffix}")
    }
    match decode(code) {
        Keycode::No => "KC_NO".into(),
        Keycode::Transparent => "KC_TRNS".into(),
        Keycode::Basic(c) => {
            names::qmk_by_code(c).map(|e| e.short_name().to_string()).unwrap_or_else(|| format!("{c:#06x}"))
        }
        Keycode::Mods { mods, kc } => format!("{}({})", mods_fn(mods, ""), basic(kc)),
        Keycode::ModTap { mods, kc } => {
            let f = mods_fn(mods, "_T");
            format!("{f}({})", basic(kc))
        }
        Keycode::LayerTap { layer, kc } => format!("LT({layer}, {})", basic(kc)),
        Keycode::LayerMod { layer, mods } => format!("LM({layer}, {})", mods.display()),
        Keycode::Layer { op: op @ (LayerOp::TriLayerLower | LayerOp::TriLayerUpper), .. } => op.prefix().into(),
        Keycode::Layer { op, layer } => format!("{}({layer})", op.prefix()),
        Keycode::OneShotMod(m) => format!("OSM({})", m.display()),
        Keycode::TapDance(n) => format!("TD({n})"),
        Keycode::Macro(n) => format!("M{n}"),
        Keycode::Custom(n) => format!("QK_KB_{n}"),
        Keycode::Unknown(c) => format!("{c:#06x}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range_start(define: &str) -> u16 {
        names::QMK_RANGES.iter().find(|r| r.2 == define).unwrap_or_else(|| panic!("{define} missing")).0
    }

    #[test]
    fn constants_match_generated_ranges() {
        assert_eq!(range_start("QK_MODS"), QK_MODS);
        assert_eq!(range_start("QK_MOD_TAP"), QK_MOD_TAP);
        assert_eq!(range_start("QK_LAYER_TAP"), QK_LAYER_TAP);
        assert_eq!(range_start("QK_LAYER_MOD"), QK_LAYER_MOD);
        assert_eq!(range_start("QK_TO"), QK_TO);
        assert_eq!(range_start("QK_MOMENTARY"), QK_MOMENTARY);
        assert_eq!(range_start("QK_DEF_LAYER"), QK_DEF_LAYER);
        assert_eq!(range_start("QK_TOGGLE_LAYER"), QK_TOGGLE_LAYER);
        assert_eq!(range_start("QK_ONE_SHOT_LAYER"), QK_ONE_SHOT_LAYER);
        assert_eq!(range_start("QK_ONE_SHOT_MOD"), QK_ONE_SHOT_MOD);
        assert_eq!(range_start("QK_LAYER_TAP_TOGGLE"), QK_LAYER_TAP_TOGGLE);
        assert_eq!(range_start("QK_PERSISTENT_DEF_LAYER"), QK_PERSISTENT_DEF_LAYER);
        assert_eq!(range_start("QK_TAP_DANCE"), QK_TAP_DANCE);
        assert_eq!(range_start("QK_MACRO"), QK_MACRO);
        assert_eq!(range_start("QK_KB"), QK_KB_0);
        assert_eq!(names::qmk_code("QK_TRI_LAYER_LOWER"), Some(QK_TRI_LAYER_LOWER));
        assert_eq!(names::qmk_code("TL_UPPR"), Some(QK_TRI_LAYER_UPPER));
    }

    #[test]
    fn decodes_keymap_examples() {
        assert_eq!(decode(0x0000), Keycode::No);
        assert_eq!(decode(0x0001), Keycode::Transparent);
        assert_eq!(decode(0x0004), Keycode::Basic(0x04));
        // LCTL_T(KC_TAB)
        assert_eq!(decode(0x212B), Keycode::ModTap { mods: Mods5(0x01), kc: 0x2B });
        // C(KC_Z), C(S(KC_F5)), A(KC_F12)
        assert_eq!(decode(0x011D), Keycode::Mods { mods: Mods5(0x01), kc: 0x1D });
        assert_eq!(decode(0x033E), Keycode::Mods { mods: Mods5(0x03), kc: 0x3E });
        assert_eq!(decode(0x0445), Keycode::Mods { mods: Mods5(0x04), kc: 0x45 });
        // TG(4), OSL(5), MO(1), TL_LOWR, TL_UPPR
        assert_eq!(decode(0x5264), Keycode::Layer { op: LayerOp::Toggle, layer: 4 });
        assert_eq!(decode(0x5285), Keycode::Layer { op: LayerOp::OneShot, layer: 5 });
        assert_eq!(decode(0x5221), Keycode::Layer { op: LayerOp::Momentary, layer: 1 });
        assert_eq!(decode(0x7C77), Keycode::Layer { op: LayerOp::TriLayerLower, layer: 1 });
        assert_eq!(decode(0x7C78), Keycode::Layer { op: LayerOp::TriLayerUpper, layer: 2 });
        // LT(2, KC_SPC), OSM(MOD_LSFT)
        assert_eq!(decode(0x422C), Keycode::LayerTap { layer: 2, kc: 0x2C });
        assert_eq!(decode(0x52A2), Keycode::OneShotMod(Mods5(0x02)));
        // custom keycodes: ACC_A, HUD_TOG
        assert_eq!(decode(0x7E00), Keycode::Custom(0));
        assert_eq!(decode(0x7E12), Keycode::Custom(18));
        // named quantum keycodes: QK_REP, CW_TOGG, QK_BOOT
        assert_eq!(decode(0x7C79), Keycode::Basic(0x7C79));
        assert_eq!(decode(0x7C73), Keycode::Basic(0x7C73));
        assert_eq!(decode(0x7C00), Keycode::Basic(0x7C00));
        assert_eq!(decode(0x7FFF), Keycode::Unknown(0x7FFF));
    }

    #[test]
    fn canonical_names() {
        assert_eq!(qmk_name(0x212B), "LCTL_T(KC_TAB)");
        assert_eq!(qmk_name(0x011D), "LCTL(KC_Z)");
        assert_eq!(qmk_name(0x033E), "LCS(KC_F5)");
        assert_eq!(qmk_name(0x5264), "TG(4)");
        assert_eq!(qmk_name(0x7C77), "TL_LOWR");
        assert_eq!(qmk_name(0x7E12), "QK_KB_18");
        assert_eq!(qmk_name(0x0029), "KC_ESC");
    }

    #[test]
    fn every_code_decodes_without_panic() {
        for code in 0..=u16::MAX {
            let _ = decode(code);
            let _ = qmk_name(code);
        }
    }

    #[test]
    fn mods_display() {
        assert_eq!(Mods5(0x01).display(), "Ctrl");
        assert_eq!(Mods5(0x03).display(), "Ctrl+Shift");
        assert_eq!(Mods5(0x14).display(), "RAlt");
        assert_eq!(Mods5(0x07).display(), "Meh");
    }
}
