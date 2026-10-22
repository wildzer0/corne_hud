//! Custom key labels from the config (e.g. "Step over" for F10 on DEBUG).

use crate::config::LabelConfig;
use crate::tr_format;
use crate::vial::legend::{Legend, Text};
use crate::vial::names::parse_keycode;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub text: String,
    pub description: Option<String>,
}

/// Application name as compared: lower case, without `.exe`.
pub fn app_key(name: &str) -> String {
    let n = name.trim().to_lowercase();
    n.strip_suffix(".exe").map(str::to_string).unwrap_or(n)
}

/// Labels of one kind: generic ones, and per application.
#[derive(Debug, Default)]
struct Table<K> {
    any: HashMap<K, Label>,
    by_app: HashMap<(String, K), Label>,
}

impl<K: std::hash::Hash + Eq + Clone> Table<K> {
    fn insert(&mut self, apps: &[String], key: K, label: Label) {
        if apps.is_empty() {
            self.any.insert(key, label);
            return;
        }
        for a in apps {
            self.by_app.insert((app_key(a), key.clone()), label.clone());
        }
    }

    /// The application's label first, then the generic one.
    fn get(&self, app: Option<&str>, key: &K) -> Option<&Label> {
        app.and_then(|a| self.by_app.get(&(a.to_string(), key.clone()))).or_else(|| self.any.get(key))
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.any.is_empty() && self.by_app.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct Labels {
    by_position: Table<(Option<u8>, u8, u8)>,
    by_layer_code: Table<(u8, u16)>,
    by_code: Table<u16>,
    /// Applications that have labels of their own.
    apps: Vec<String>,
    /// Application in the foreground (see [`Labels::set_app`]).
    app: Option<String>,
}

impl Labels {
    pub fn from_config(entries: &[LabelConfig]) -> (Self, Vec<String>) {
        let mut labels = Labels::default();
        let mut errors = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            let n = i + 1;
            if e.label.trim().is_empty() {
                errors.push(tr_format!("etichetta {n}: 'label' vuoto", "label {n}: empty 'label'"));
                continue;
            }
            let apps: Vec<String> = e.app.as_ref().map(|a| a.names()).unwrap_or_default();
            if e.app.is_some() && apps.iter().all(|a| a.trim().is_empty()) {
                errors.push(tr_format!("etichetta {n} ({}): 'app' vuoto", "label {n} ({}): empty 'app'", e.label));
                continue;
            }
            let label = Label { text: e.label.clone(), description: e.description.clone() };
            match (e.row, e.col, &e.keycode) {
                (Some(row), Some(col), None) => labels.by_position.insert(&apps, (e.layer, row, col), label),
                (None, None, Some(name)) => match parse_keycode(name) {
                    Some(code) => match e.layer {
                        Some(layer) => labels.by_layer_code.insert(&apps, (layer, code), label),
                        None => labels.by_code.insert(&apps, code, label),
                    },
                    None => {
                        errors.push(tr_format!(
                            "etichetta {n} ({}): keycode '{name}' non riconosciuto",
                            "label {n} ({}): unknown keycode '{name}'",
                            e.label
                        ));
                        continue;
                    }
                },
                _ => {
                    errors.push(tr_format!(
                        "etichetta {n} ({}): indica 'keycode' oppure 'row' e 'col' (con 'layer' facoltativo)",
                        "label {n} ({}): give 'keycode', or 'row' and 'col' ('layer' is optional)",
                        e.label
                    ));
                    continue;
                }
            }
            for a in apps.iter().map(|a| app_key(a)) {
                if !labels.apps.contains(&a) {
                    labels.apps.push(a);
                }
            }
        }
        (labels, errors)
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.by_position.is_empty() && self.by_layer_code.is_empty() && self.by_code.is_empty()
    }

    /// Sets the foreground application. Returns true when the labels shown change.
    pub fn set_app(&mut self, app: Option<&str>) -> bool {
        let key = app.map(app_key).filter(|a| self.apps.contains(a));
        if key == self.app {
            return false;
        }
        self.app = key;
        true
    }

    /// Some labels depend on the foreground application.
    pub fn has_app_labels(&self) -> bool {
        !self.apps.is_empty()
    }

    /// The foreground application, when it has labels of its own.
    pub fn active_app(&self) -> Option<&str> {
        self.app.as_deref()
    }

    pub fn lookup(&self, layer: u8, row: u8, col: u8, code: u16, code_layer: u8) -> Option<&Label> {
        let app = self.app.as_deref();
        self.by_position
            .get(app, &(Some(layer), row, col))
            .or_else(|| self.by_position.get(app, &(None, row, col)))
            .or_else(|| self.by_layer_code.get(app, &(code_layer, code)))
            .or_else(|| self.by_code.get(app, &code))
    }

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
            app: None,
        }
    }

    #[test]
    fn labels_per_application() {
        use crate::config::AppNames;
        let code_only = LabelConfig {
            app: Some(AppNames::One("Code.exe".into())),
            ..entry(Some(4), None, Some("KC_F10"), "Step over")
        };
        let both = LabelConfig {
            app: Some(AppNames::Many(vec!["idea64".into(), "pycharm".into()])),
            ..entry(Some(4), None, Some("KC_F10"), "Step (JetBrains)")
        };
        let (mut labels, errors) = Labels::from_config(&[code_only, both, entry(None, None, Some("KC_F10"), "F10")]);
        assert!(errors.is_empty(), "{errors:?}");
        let text = |l: &Labels| l.lookup(4, 1, 3, 0x43, 4).map(|l| l.text.clone());
        // no app, or one without labels: the generic label
        assert_eq!(text(&labels), Some("F10".into()));
        assert!(!labels.set_app(Some("firefox")));
        assert_eq!(text(&labels), Some("F10".into()));
        // names compare without case and ".exe"
        assert!(labels.set_app(Some("code")));
        assert_eq!(labels.active_app(), Some("code"));
        assert_eq!(text(&labels), Some("Step over".into()));
        assert!(labels.set_app(Some("PyCharm.exe")));
        assert_eq!(text(&labels), Some("Step (JetBrains)".into()));
        // other layers keep the generic label
        assert_eq!(labels.lookup(3, 0, 3, 0x43, 3).map(|l| l.text.as_str()), Some("F10"));
        assert!(labels.set_app(None));
        // an empty app name is an error
        let bad = LabelConfig { app: Some(AppNames::One(" ".into())), ..entry(None, None, Some("KC_A"), "x") };
        assert_eq!(Labels::from_config(&[bad]).1.len(), 1);
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
