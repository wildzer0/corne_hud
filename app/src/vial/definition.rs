//! Vial keyboard definition (`vial.json`): matrix size, KLE geometry and
//! custom keycode names. In live mode it is read compressed (xz) from the
//! keyboard; in offline mode from disk.

use crate::kle::{self, KleError, KleKey};
use serde::Deserialize;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CustomKeycode {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, rename = "shortName")]
    pub short_name: String,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub rows: u8,
    pub cols: u8,
    /// All KLE keys, including every layout option alternative.
    pub keys: Vec<KleKey>,
    /// Bit width of each layout option group.
    pub option_widths: Vec<u32>,
    pub custom: Vec<CustomKeycode>,
}

#[derive(Debug)]
pub enum DefinitionError {
    Json(serde_json::Error),
    Xz(String),
    Missing(&'static str),
    Invalid(&'static str),
    Kle(KleError),
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DefinitionError::Json(e) => write!(f, "JSON non valido: {e}"),
            DefinitionError::Xz(e) => write!(f, "decompressione xz fallita: {e}"),
            DefinitionError::Missing(field) => write!(f, "campo mancante: {field}"),
            DefinitionError::Invalid(field) => write!(f, "valore non valido: {field}"),
            DefinitionError::Kle(e) => write!(f, "layout KLE: {e}"),
        }
    }
}

impl std::error::Error for DefinitionError {}

impl Definition {
    pub fn from_json_str(text: &str) -> Result<Self, DefinitionError> {
        let v: Value = serde_json::from_str(text).map_err(DefinitionError::Json)?;
        Self::from_value(&v)
    }

    /// Parses the xz-compressed definition returned by `vial_get_def`.
    /// Trailing padding after the xz stream (the last 32-byte page) is ignored.
    #[cfg(test)]
    pub fn from_xz(data: &[u8]) -> Result<Self, DefinitionError> {
        Self::from_json_str(&Self::xz_to_json(data)?)
    }

    /// Decompresses the definition to its JSON text (kept as is in the cache).
    pub fn xz_to_json(data: &[u8]) -> Result<String, DefinitionError> {
        // an xz stream ends with the "YZ" footer magic: drop zero padding after it
        let end = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        let data = if data[..end].ends_with(b"YZ") { &data[..end] } else { data };
        let mut out = Vec::new();
        let mut input = std::io::BufReader::new(data);
        lzma_rs::xz_decompress(&mut input, &mut out).map_err(|e| DefinitionError::Xz(e.to_string()))?;
        String::from_utf8(out).map_err(|_| DefinitionError::Invalid("utf-8"))
    }

    pub fn from_value(v: &Value) -> Result<Self, DefinitionError> {
        let matrix = v.get("matrix").ok_or(DefinitionError::Missing("matrix"))?;
        let dim = |field: &'static str| -> Result<u8, DefinitionError> {
            let n = matrix.get(field).and_then(Value::as_u64).ok_or(DefinitionError::Missing(field))?;
            u8::try_from(n).ok().filter(|n| (1..=32).contains(n)).ok_or(DefinitionError::Invalid(field))
        };
        let rows = dim("rows")?;
        let cols = dim("cols")?;
        let layouts = v.get("layouts").ok_or(DefinitionError::Missing("layouts"))?;
        let keymap = layouts.get("keymap").ok_or(DefinitionError::Missing("layouts.keymap"))?;
        let keys = kle::parse(keymap).map_err(DefinitionError::Kle)?;
        let option_widths = layouts.get("labels").map(kle::option_widths).unwrap_or_default();
        let custom = match v.get("customKeycodes") {
            Some(c) => serde_json::from_value(c.clone()).map_err(DefinitionError::Json)?,
            None => Vec::new(),
        };
        Ok(Definition {
            name: v.get("name").and_then(Value::as_str).unwrap_or("Keyboard").to_string(),
            rows,
            cols,
            keys,
            option_widths,
            custom,
        })
    }

    /// Keys visible with the given VIA layout options value.
    pub fn visible_keys(&self, layout_options: u32) -> Vec<KleKey> {
        let choices = kle::decode_options(layout_options, &self.option_widths);
        kle::filter_by_options(&self.keys, &choices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIAL_JSON: &str = include_str!("../../data/mock/vial.json");

    #[test]
    fn parses_corne_definition() {
        let d = Definition::from_json_str(VIAL_JSON).unwrap();
        assert_eq!((d.rows, d.cols), (8, 7));
        assert_eq!(d.name, "Corne v4");
        assert_eq!(d.custom.len(), 19);
        assert_eq!(d.custom[0].short_name, "à");
        assert_eq!(d.custom[18].short_name, "HUD");
        assert_eq!(d.visible_keys(0).len(), 46);
    }

    #[test]
    fn rejects_malformed_definitions() {
        assert!(Definition::from_json_str("").is_err());
        assert!(Definition::from_json_str("{}").is_err());
        assert!(Definition::from_json_str(r#"{"matrix":{"rows":0,"cols":7},"layouts":{"keymap":[]}}"#).is_err());
        assert!(Definition::from_json_str(r#"{"matrix":{"rows":8,"cols":7},"layouts":{"keymap":{}}}"#).is_err());
        assert!(Definition::from_xz(&[0u8; 64]).is_err());
        assert!(Definition::from_xz(&[]).is_err());
    }

    #[test]
    fn xz_round_trip_with_padding() {
        let mut compressed = Vec::new();
        lzma_rs::xz_compress(&mut VIAL_JSON.as_bytes(), &mut compressed).unwrap();
        // the keyboard sends whole 32-byte pages
        compressed.resize(compressed.len().div_ceil(32) * 32, 0);
        let d = Definition::from_xz(&compressed).unwrap();
        assert_eq!(d.custom.len(), 19);
    }
}
