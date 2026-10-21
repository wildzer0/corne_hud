//! Custom key labels from the config (e.g. "Step over" for F10 on DEBUG).

use crate::config::LabelConfig;
use crate::vial::legend::{Legend, Text};
use crate::vial::names::parse_keycode;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub text: String,
    pub description: Option<String>,
}

/// Lookup tables, most specific first: position, keycode on a layer, keycode.
#[derive(Debug, Default)]
pub struct Labels {
    by_position: HashMap<(Option<u8>, u8, u8), Label>,
    by_layer_code: HashMap<(u8, u16), Label>,
    by_code: HashMap<u16, Label>,
}

impl Labels {
    /// Builds the tables; invalid entries are skipped and described in the returned errors.
    pub fn from_config(entries: &[LabelConfig]) -> (Self, Vec<String>) {
        let mut labels = Labels::default();
        let mut errors = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            let n = i + 1;
            if e.label.trim().is_empty() {
                errors.push(format!("etichetta {n}: 'label' vuoto"));
                continue;
            }
            let label = Label { text: e.label.clone(), description: e.description.clone() };
            match (e.row, e.col, &e.keycode) {
                (Some(row), Some(col), None) => {
                    labels.by_position.insert((e.layer, row, col), label);
                }
                (None, None, Some(name)) => match parse_keycode(name) {
                    Some(code) => {
                        match e.layer {
                            Some(layer) => labels.by_layer_code.insert((layer, code), label),
                            None => labels.by_code.insert(code, label),
                        };
                    }
                    None => errors.push(format!("etichetta {n} ({}): keycode '{name}' non riconosciuto", e.label)),
                },
                _ => errors.push(format!(
                    "etichetta {n} ({}): indica 'keycode' oppure 'row' e 'col' (con 'layer' facoltativo)",
                    e.label
                )),
            }
        }
        (labels, errors)
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.by_position.is_empty() && self.by_layer_code.is_empty() && self.by_code.is_empty()
    }

    /// Label for the key at `(layer, row, col)` showing `code`, which comes
    /// from `code_layer` (different from `layer` for transparent keys).
    pub fn lookup(&self, layer: u8, row: u8, col: u8, code: u16, code_layer: u8) -> Option<&Label> {
        self.by_position
            .get(&(Some(layer), row, col))
            .or_else(|| self.by_position.get(&(None, row, col)))
            .or_else(|| self.by_layer_code.get(&(code_layer, code)))
            .or_else(|| self.by_code.get(&code))
    }

    /// Applies the matching label (if any) to `legend`.
    pub fn apply(&self, legend: &mut Legend, layer: u8, row: u8, col: u8, code: u16, code_layer: u8) {
        if let Some(l) = self.lookup(layer, row, col, code, code_layer) {
            let what = l.description.clone().unwrap_or_else(|| l.text.clone());
            legend.tooltip = format!("{what}\n{}", legend.tooltip);
            legend.tap = Text { symbol: l.text.clone(), text: l.text.clone() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(layer: Option<u8>, pos: Option<(u8, u8)>, keycode: Option<&str>, label: &str) -> LabelConfig {
        LabelConfig {
            layer,
            row: pos.map(|p| p.0),
            col: pos.map(|p| p.1),
            keycode: keycode.map(str::to_string),
            label: label.into(),
            description: None,
        }
    }

    #[test]
    fn precedence_and_errors() {
        let (labels, errors) = Labels::from_config(&[
            entry(Some(4), None, Some("KC_F10"), "Step over"),
            entry(None, None, Some("KC_F10"), "F10 ovunque"),
            entry(Some(4), Some((1, 3)), None, "Qui"),
            entry(None, None, Some("KC_BOGUS"), "x"),
            entry(None, Some((0, 0)), Some("KC_A"), "ambiguo"),
            entry(None, None, Some("KC_A"), " "),
        ]);
        assert_eq!(errors.len(), 3, "{errors:?}");
        let text = |l: Option<&Label>| l.map(|l| l.text.clone());
        // position wins
        assert_eq!(text(labels.lookup(4, 1, 3, 0x43, 4)), Some("Qui".into()));
        // keycode on its layer
        assert_eq!(text(labels.lookup(4, 1, 4, 0x43, 4)), Some("Step over".into()));
        // keycode anywhere else
        assert_eq!(text(labels.lookup(3, 0, 3, 0x43, 3)), Some("F10 ovunque".into()));
        assert_eq!(labels.lookup(0, 0, 1, 0x14, 0), None);
    }

    #[test]
    fn example_config_labels_are_valid() {
        let cfg = crate::config::Config::parse(include_str!("../../keyboard-hud.example.toml")).unwrap();
        let (labels, errors) = Labels::from_config(&cfg.labels);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(!labels.is_empty());
        // SM_RUN (QK_KB_15) on DEBUG
        assert_eq!(labels.lookup(4, 0, 3, 0x7E0F, 4).map(|l| l.text.as_str()), Some("Run"));
        // C(KC_DOT) on DEBUG
        assert_eq!(labels.lookup(4, 2, 5, 0x0137, 4).map(|l| l.text.as_str()), Some("Quick fix"));
    }
}
