//! Search: where is "è", "redo", "F10" or "{"? Finds the layer and key, and
//! the sequence to press (e.g. "Fn1 + D").

use crate::labels::Labels;
use crate::model::Keymap;
use crate::vial::keycode::{
    decode, qmk_name, Keycode, LayerOp, KC_NO, KC_TRNS, TRI_LAYER_ADJUST, TRI_LAYER_LOWER, TRI_LAYER_UPPER,
};
use crate::vial::legend::{legend, Legend, LegendContext};

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
    pub code: u16,
    /// What the key shows (label or legend).
    pub shown: String,
    /// Keys to press, e.g. "Fn1 + D".
    pub sequence: String,
    pub score: u32,
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Score of `query` against one candidate text.
fn score_text(query: &str, text: &str, exact: u32, prefix: u32, contains: u32) -> u32 {
    let t = norm(text);
    if t.is_empty() {
        0
    } else if t == query {
        exact
    } else if t.starts_with(query) {
        prefix
    } else if query.chars().count() >= 2 && t.contains(query) {
        contains
    } else {
        0
    }
}

fn legend_with_label(code: u16, ctx: &LegendContext, labels: &Labels, pos: (u8, u8, u8)) -> (Legend, Option<String>) {
    let mut leg = legend(code, ctx);
    let label = labels.lookup(pos.0, pos.1, pos.2, code, pos.0).map(|l| l.text.clone());
    labels.apply(&mut leg, pos.0, pos.1, pos.2, code, pos.0);
    (leg, label)
}

/// How a layer is reached from the default layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Reach {
    Hold,
    OneShot,
    Toggle,
}

/// Name of the key at a default-layer position, as printed on it.
fn key_name(keymap: &Keymap, ctx: &LegendContext, labels: &Labels, row: u8, col: u8) -> String {
    let code = keymap.get(0, row, col);
    let (leg, _) = legend_with_label(code, ctx, labels, (0, row, col));
    match decode(code) {
        Keycode::Layer { op: LayerOp::TriLayerLower, .. } => "Fn1".into(),
        Keycode::Layer { op: LayerOp::TriLayerUpper, .. } => "Fn2".into(),
        _ if leg.tap.text.is_empty() => format!("{},{}", row, col),
        _ => leg.tap.text,
    }
}

/// Best key on the default layer that reaches `target`, and how.
fn activator(keymap: &Keymap, ctx: &LegendContext, labels: &Labels, target: u8) -> Option<(String, Reach)> {
    let mut best: Option<((u8, u8), Reach)> = None;
    for row in 0..keymap.rows() {
        for col in 0..keymap.cols() {
            let reach = match decode(keymap.get(0, row, col)) {
                Keycode::Layer { op, layer } if layer == target => match op {
                    LayerOp::Momentary | LayerOp::TriLayerLower | LayerOp::TriLayerUpper | LayerOp::TapToggle => {
                        Reach::Hold
                    }
                    LayerOp::OneShot => Reach::OneShot,
                    LayerOp::Toggle | LayerOp::To => Reach::Toggle,
                    LayerOp::DefaultLayer | LayerOp::PersistentDefault => continue,
                },
                Keycode::LayerTap { layer, .. } | Keycode::LayerMod { layer, .. } if layer == target => Reach::Hold,
                _ => continue,
            };
            if best.is_none_or(|(_, r)| reach < r) {
                best = Some(((row, col), reach));
            }
        }
    }
    best.map(|((r, c), reach)| (key_name(keymap, ctx, labels, r, c), reach))
}

/// Sequence to press `row,col` on `layer`, e.g. "Fn1 + D", "1×ACC, poi E".
pub fn sequence(keymap: &Keymap, ctx: &LegendContext, labels: &Labels, layer: u8, row: u8, col: u8) -> String {
    let key = key_name(keymap, ctx, labels, row, col);
    if layer == 0 {
        return key;
    }
    match activator(keymap, ctx, labels, layer) {
        Some((act, Reach::Hold)) => format!("{act} + {key}"),
        Some((act, Reach::OneShot)) => format!("{act}, poi {key}"),
        Some((act, Reach::Toggle)) => format!("{act} (blocca), poi {key}"),
        None if layer == TRI_LAYER_ADJUST => {
            let lower = activator(keymap, ctx, labels, TRI_LAYER_LOWER);
            let upper = activator(keymap, ctx, labels, TRI_LAYER_UPPER);
            match (lower, upper) {
                (Some((l, Reach::Hold)), Some((u, Reach::Hold))) => format!("{l} + {u} + {key}"),
                _ => format!("layer {layer} + {key}"),
            }
        }
        None => {
            let name = ctx.layer_names.get(layer as usize).cloned().unwrap_or_else(|| format!("L{layer}"));
            format!("layer {name} + {key}")
        }
    }
}

/// Finds keys matching `query` on every layer, best first.
pub fn search(query: &str, keymap: &Keymap, positions: &[(u8, u8)], ctx: &LegendContext, labels: &Labels) -> Vec<Hit> {
    let q = norm(query);
    if q.is_empty() {
        return Vec::new();
    }
    let q_kc = format!("kc_{q}");
    let mut hits = Vec::new();
    for layer in 0..keymap.layers() {
        for &(row, col) in positions {
            let code = keymap.get(layer, row, col);
            if code == KC_NO || code == KC_TRNS {
                continue;
            }
            let (leg, label) = legend_with_label(code, ctx, labels, (layer, row, col));
            let name = qmk_name(code);
            let n = norm(&name);
            let mut score = [
                label.as_deref().map_or(0, |l| score_text(&q, l, 100, 70, 50)),
                score_text(&q, &leg.tap.symbol, 100, 60, 40),
                score_text(&q, &leg.tap.text, 100, 60, 40),
                if n == q || n == q_kc { 95 } else { score_text(&q, &name, 0, 55, 35) },
                score_text(&q, &leg.tooltip, 0, 30, 25),
            ]
            .into_iter()
            .max()
            .unwrap_or(0);
            if score == 0 {
                continue;
            }
            // same match on several layers: prefer the easier to reach
            score = score * 10 + (9 - layer.min(9)) as u32;
            let shown = if leg.tap.text.is_empty() { name } else { leg.tap.text.clone() };
            hits.push(Hit {
                layer,
                row,
                col,
                code,
                shown,
                sequence: sequence(keymap, ctx, labels, layer, row, col),
                score,
            });
        }
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then((a.layer, a.row, a.col).cmp(&(b.layer, b.row, b.col))));
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vial::definition::Definition;

    struct Fixture {
        keymap: Keymap,
        positions: Vec<(u8, u8)>,
        def: Definition,
        names: Vec<String>,
        labels: Labels,
    }

    fn fixture() -> Fixture {
        let def = Definition::from_json_str(crate::hid::mock::MOCK_VIAL_JSON).unwrap();
        let keymap = crate::vial::vil::Vil::parse(crate::hid::mock::MOCK_VIL, 8, 7).unwrap().keymap;
        let positions = def
            .visible_keys(0)
            .iter()
            .filter_map(|k| match k.role() {
                Some(crate::kle::KeyRole::Matrix { row, col }) => Some((row, col)),
                _ => None,
            })
            .collect();
        let cfg = crate::config::Config::parse(include_str!("../../keyboard-hud.example.toml")).unwrap();
        let (labels, _) = Labels::from_config(&cfg.labels);
        let names = ["BASE", "SYM", "NAV", "SYS", "DEBUG", "ACC"].map(String::from).to_vec();
        Fixture { keymap, positions, def, names, labels }
    }

    fn first(f: &Fixture, q: &str) -> Hit {
        let ctx = LegendContext { layer_names: &f.names, custom: &f.def.custom };
        search(q, &f.keymap, &f.positions, &ctx, &f.labels)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("no hit for {q}"))
    }

    #[test]
    fn finds_symbols_letters_and_accents() {
        let f = fixture();
        // '(' is on SYM, physical D
        let h = first(&f, "(");
        assert_eq!((h.layer, h.row, h.col), (1, 1, 3));
        assert_eq!(h.sequence, "Fn1 + D");
        // '{' on SYM, physical A
        assert_eq!(first(&f, "{").sequence, "Fn1 + A");
        // è on ACC (one-shot), physical E
        let h = first(&f, "è");
        assert_eq!((h.layer, h.sequence.as_str()), (5, "1×ACC, poi E"));
        // a plain letter is on BASE
        assert_eq!(first(&f, "q").sequence, "Q");
    }

    #[test]
    fn finds_custom_and_semantic_names() {
        let f = fixture();
        // Redo: custom keycode on NAV, physical B
        let h = first(&f, "redo");
        assert_eq!((h.layer, h.sequence.as_str()), (2, "Fn2 + B"));
        // Step over: semantic label for F10 on DEBUG (toggled with TG)
        let h = first(&f, "step over");
        assert_eq!((h.layer, h.row, h.col), (4, 1, 3));
        assert!(h.sequence.starts_with("⇄ DEBUG (blocca), poi"), "{}", h.sequence);
        // F10 by name: the easiest is on SYS (Fn1 + Fn2)
        let h = first(&f, "F10");
        assert_eq!(h.layer, 3);
        assert_eq!(h.sequence, "Fn1 + Fn2 + E");
    }

    #[test]
    fn empty_and_unknown_queries() {
        let f = fixture();
        let ctx = LegendContext { layer_names: &f.names, custom: &f.def.custom };
        assert!(search("   ", &f.keymap, &f.positions, &ctx, &f.labels).is_empty());
        assert!(search("zzzqqq", &f.keymap, &f.positions, &ctx, &f.labels).is_empty());
    }
}
