//! Vial layout backup (`.vil`), used by the offline mode.
//!
//! `layout[layer][row][col]` holds vial-gui keycode names (`"KC_LSHIFT"`,
//! `"LCTL_T(KC_TAB)"`, `"USER03"`, `"0x7e12"`), integers, or `-1` for
//! matrix positions without a key.

use super::names::vial_code;
use crate::model::Keymap;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Vil {
    pub uid: Option<u64>,
    pub layout_options: u32,
    pub keymap: Keymap,
    /// Names that could not be decoded (shown as `KC_NO`).
    pub unknown: Vec<String>,
}

#[derive(Debug)]
pub enum VilError {
    Json(serde_json::Error),
    Invalid(String),
}

impl fmt::Display for VilError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VilError::Json(e) => write!(f, "file .vil non valido: {e}"),
            VilError::Invalid(msg) => write!(f, "file .vil non valido: {msg}"),
        }
    }
}

impl std::error::Error for VilError {}

impl Vil {
    pub fn parse(text: &str, rows: u8, cols: u8) -> Result<Self, VilError> {
        let v: Value = serde_json::from_str(text).map_err(VilError::Json)?;
        let layers =
            v.get("layout").and_then(Value::as_array).ok_or_else(|| VilError::Invalid("manca 'layout'".into()))?;
        if layers.is_empty() || layers.len() > 32 {
            return Err(VilError::Invalid(format!("numero di layer non valido: {}", layers.len())));
        }
        let mut keymap = Keymap::new(layers.len() as u8, rows, cols);
        let mut unknown = Vec::new();
        for (l, layer) in layers.iter().enumerate() {
            let layer_rows = layer.as_array().ok_or_else(|| VilError::Invalid(format!("layer {l} non è una lista")))?;
            for (r, row) in layer_rows.iter().enumerate().take(rows as usize) {
                let cells = row.as_array().ok_or_else(|| VilError::Invalid(format!("layer {l} riga {r}")))?;
                for (c, cell) in cells.iter().enumerate().take(cols as usize) {
                    let code = match cell {
                        Value::Number(n) => match n.as_i64() {
                            Some(-1) => None,
                            Some(n) => u16::try_from(n).ok(),
                            None => None,
                        },
                        Value::String(s) => {
                            let code = vial_code(s);
                            if code.is_none() {
                                unknown.push(s.clone());
                            }
                            code
                        }
                        _ => None,
                    };
                    keymap.set(l as u8, r as u8, c as u8, code.unwrap_or(0));
                }
            }
        }
        Ok(Vil {
            uid: v.get("uid").and_then(Value::as_u64),
            layout_options: v.get("layout_options").and_then(Value::as_i64).map(|o| o.max(0) as u32).unwrap_or(0),
            keymap,
            unknown,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIL: &str = include_str!("../../data/mock/corne.vil");

    #[test]
    fn parses_corne_vil() {
        let vil = Vil::parse(VIL, 8, 7).unwrap();
        assert!(vil.unknown.is_empty(), "unknown names: {:?}", vil.unknown);
        assert_eq!(vil.keymap.layers(), 6);
        assert_eq!(vil.layout_options, 0);
        // the uid in the file is VIAL_KEYBOARD_UID read as little-endian u64
        assert_eq!(vil.uid, Some(u64::from_le_bytes([0x89, 0x36, 0x2A, 0xC7, 0xFA, 0xD8, 0x89, 0x45])));
        let k = &vil.keymap;
        assert_eq!(k.get(0, 0, 0), 0x5264); // TG(4)
        assert_eq!(k.get(0, 1, 0), 0x212B); // LCTL_T(KC_TAB)
        assert_eq!(k.get(0, 3, 4), 0x7C77); // TL_LOWR
        assert_eq!(k.get(0, 7, 3), 0x7C79); // QK_REP
        assert_eq!(k.get(0, 5, 6), 0x5285); // OSL(5)
        assert_eq!(k.get(0, 4, 0), 0x002A); // Backspace (right half mirrored)
        assert_eq!(k.get(0, 3, 0), 0); // -1 (no key)
        assert_eq!(k.get(4, 1, 1), 0x023E); // S(KC_F5)
        assert_eq!(k.get(5, 0, 3), 0x7E01); // ACC_EG
        assert_eq!(k.get(1, 1, 6), 0x0231); // KC_PIPE, saved as LSFT(KC_BSLASH)
    }

    #[test]
    fn rejects_malformed_files() {
        assert!(Vil::parse("", 8, 7).is_err());
        assert!(Vil::parse("{}", 8, 7).is_err());
        assert!(Vil::parse(r#"{"layout": []}"#, 8, 7).is_err());
        assert!(Vil::parse(r#"{"layout": [5]}"#, 8, 7).is_err());
        let v = Vil::parse(r#"{"layout": [[["KC_A", "BOGUS", 70000, {}]]]}"#, 8, 7).unwrap();
        assert_eq!(v.keymap.get(0, 0, 0), 4);
        assert_eq!(v.keymap.get(0, 0, 1), 0);
        assert_eq!(v.keymap.get(0, 0, 2), 0);
        assert_eq!(v.unknown, vec!["BOGUS".to_string()]);
    }
}
