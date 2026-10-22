//! What changed between two backups (`.vil` contents), in words: one line
//! per change, e.g. "NAV · tasto B (riga 2, colonna 5): Redo → Ctrl+Y".

use crate::vial::definition::CustomKeycode;
use crate::vial::dynamic::{Combo, KeyOverride, TapDance};
use crate::vial::keycode::{Mods5, KC_NO, KC_TRNS};
use crate::vial::legend::{legend, LegendContext};
use crate::vial::names::vial_code;
use serde_json::Value;

/// Names of the QMK settings (vial-qmk `quantum/qmk_settings.c`).
fn setting_name(id: u64) -> String {
    let name = match id {
        1 => "Grave Esc override",
        2 => "combo term",
        3 => "auto shift",
        4 => "auto shift timeout",
        5 => "one-shot tap toggle",
        6 => "one-shot timeout",
        7 => "tapping term",
        9 => "mouse key delay",
        10 => "mouse key interval",
        11 => "mouse key move delta",
        12 => "mouse key max speed",
        13 => "mouse key time to max",
        14 => "mouse wheel delay",
        15 => "mouse wheel interval",
        16 => "mouse wheel max speed",
        17 => "mouse wheel time to max",
        18 => "tap code delay",
        19 => "tap hold caps delay",
        20 => "tapping toggle",
        21 => "magic",
        22 => "permissive hold",
        23 => "hold on other key press",
        24 => "retro tapping",
        25 => "quick tap term",
        26 => "chordal hold",
        27 => "flow tap term",
        _ => return format!("impostazione QMK {id}"),
    };
    name.to_string()
}

/// Keycode from a `.vil` value: number, vial-gui name or hex string.
fn code(v: &Value) -> u16 {
    match v {
        Value::Number(n) => n.as_u64().and_then(|n| u16::try_from(n).ok()).unwrap_or(0),
        Value::String(s) => vial_code(s).unwrap_or(0),
        _ => 0,
    }
}

fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn tap_dances(v: &Value) -> Vec<TapDance> {
    list(v, "tap_dance")
        .iter()
        .map(|e| {
            let at = |i: usize| e.get(i).map_or(0, code);
            let term = e.get(4).and_then(Value::as_u64).unwrap_or(0) as u16;
            TapDance { on_tap: at(0), on_hold: at(1), on_double_tap: at(2), on_tap_hold: at(3), term_ms: term }
        })
        .collect()
}

fn combos(v: &Value) -> Vec<Combo> {
    list(v, "combo")
        .iter()
        .map(|e| {
            let at = |i: usize| e.get(i).map_or(0, code);
            Combo { keys: [at(0), at(1), at(2), at(3)], output: at(4) }
        })
        .collect()
}

fn key_overrides(v: &Value) -> Vec<KeyOverride> {
    list(v, "key_override")
        .iter()
        .map(|e| {
            let int = |k: &str| e.get(k).and_then(Value::as_u64).unwrap_or(0);
            KeyOverride {
                trigger: e.get("trigger").map_or(0, code),
                replacement: e.get("replacement").map_or(0, code),
                layers: int("layers") as u16,
                trigger_mods: int("trigger_mods") as u8,
                negative_mods: int("negative_mod_mask") as u8,
                suppressed_mods: int("suppressed_mods") as u8,
                options: int("options") as u8,
            }
        })
        .collect()
}

/// Everything needed to put changes into words.
pub struct Context<'a> {
    pub layer_names: &'a [String],
    pub custom: &'a [CustomKeycode],
}

impl Context<'_> {
    fn layer(&self, l: usize) -> String {
        self.layer_names.get(l).cloned().unwrap_or_else(|| format!("L{l}"))
    }

    fn layers(&self, mask: u16) -> String {
        if mask == 0xFFFF {
            return "tutti i layer".into();
        }
        let names: Vec<String> = (0..16).filter(|l| mask & (1 << l) != 0).map(|l| self.layer(l)).collect();
        if names.is_empty() {
            "nessun layer".into()
        } else {
            names.join(", ")
        }
    }
}

/// A keycode in words, e.g. "Tab (tieni Ctrl)", "trasparente".
fn key(c: u16, ctx: &LegendContext) -> String {
    match c {
        KC_NO => "nessuno".into(),
        KC_TRNS => "trasparente".into(),
        _ => {
            let l = legend(c, ctx);
            let tap = if l.tap.text.is_empty() { l.tooltip.clone() } else { l.tap.text.clone() };
            match l.hold {
                Some(h) => format!("{tap} (tieni {})", h.text),
                None => tap,
            }
        }
    }
}

fn combo_text(c: &Combo, ctx: &LegendContext) -> String {
    let keys: Vec<String> = c.inputs().map(|k| key(k, ctx)).collect();
    format!("{} insieme → {}", keys.join(" + "), key(c.output, ctx))
}

fn override_text(o: &KeyOverride, ctx: &LegendContext, names: &Context) -> String {
    let off = if o.is_active() { "" } else { " (disattivato)" };
    format!(
        "{} + {} → {} ({}){off}",
        Mods5(o.mods5()).display(),
        key(o.trigger, ctx),
        key(o.replacement, ctx),
        names.layers(o.layers)
    )
}

fn tap_dance_text(t: &TapDance, ctx: &LegendContext) -> String {
    let parts: Vec<String> = t
        .actions()
        .iter()
        .filter(|(_, c)| *c != 0)
        .map(|(a, c)| format!("{} {}", a.describe(), key(*c, ctx)))
        .collect();
    parts.join(", ")
}

fn macro_text(m: &Value, ctx: &LegendContext) -> String {
    let steps: Vec<String> = m
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(|a| {
            let a = a.as_array()?;
            let tag = a.first()?.as_str()?;
            let rest = &a[1..];
            let keys = || rest.iter().map(|k| key(code(k), ctx)).collect::<Vec<_>>().join(" ");
            Some(match tag {
                "text" => format!("\"{}\"", rest.first()?.as_str()?),
                "tap" => keys(),
                "down" => format!("giù {}", keys()),
                "up" => format!("su {}", keys()),
                "delay" => format!("pausa {} ms", rest.first()?.as_u64()?),
                other => other.to_string(),
            })
        })
        .collect();
    if steps.is_empty() {
        "vuota".into()
    } else {
        steps.join(", ")
    }
}

/// A short summary for the first backup of a keyboard.
pub fn summary(cur: &Value) -> String {
    let layers = list(cur, "layout").len();
    let combos = combos(cur).iter().filter(|c| c.is_active()).count();
    let tds = tap_dances(cur).iter().filter(|t| !t.is_empty()).count();
    let kos = key_overrides(cur).iter().filter(|o| o.is_active()).count();
    let macros = list(cur, "macro").iter().filter(|m| m.as_array().is_some_and(|a| !a.is_empty())).count();
    format!("Primo backup: {layers} layer, {combos} combo, {tds} tap dance, {kos} key override, {macros} macro")
}

/// Changes from `prev` to `cur`, one line each, in a stable order.
pub fn describe(prev: &Value, cur: &Value, names: &Context) -> Vec<String> {
    let (td_prev, td_cur) = (tap_dances(prev), tap_dances(cur));
    let ctx_prev = LegendContext { layer_names: names.layer_names, custom: names.custom, tap_dances: &td_prev };
    let ctx_cur = LegendContext { layer_names: names.layer_names, custom: names.custom, tap_dances: &td_cur };
    let mut out = Vec::new();

    // keymap
    let (lp, lc) = (list(prev, "layout"), list(cur, "layout"));
    if lp.len() != lc.len() {
        out.push(format!("Numero di layer: {} → {}", lp.len(), lc.len()));
    }
    let cell = |layout: &[Value], l: usize, r: usize, c: usize| layout.get(l)?.get(r)?.get(c).cloned();
    for (l, rows) in lc.iter().enumerate() {
        for (r, cols) in rows.as_array().map(Vec::as_slice).unwrap_or(&[]).iter().enumerate() {
            for (c, now) in cols.as_array().map(Vec::as_slice).unwrap_or(&[]).iter().enumerate() {
                if now.as_i64() == Some(-1) {
                    continue;
                }
                let Some(before) = cell(lp, l, r, c) else { continue };
                let (a, b) = (code(&before), code(now));
                if a == b {
                    continue;
                }
                // where: the key as printed on the base layer
                let base = cell(lc, 0, r, c).map_or(0, |v| code(&v));
                let place = match base {
                    KC_NO | KC_TRNS => format!("riga {r}, colonna {c}"),
                    _ => format!("tasto {} (riga {r}, colonna {c})", key(base, &ctx_cur)),
                };
                out.push(format!("{} · {place}: {} → {}", names.layer(l), key(a, &ctx_prev), key(b, &ctx_cur)));
            }
        }
    }

    // encoders: [layer][index] = [ccw, cw]
    let (ep, ec) = (list(prev, "encoder_layout"), list(cur, "encoder_layout"));
    for (l, encs) in ec.iter().enumerate() {
        for (i, dirs) in encs.as_array().map(Vec::as_slice).unwrap_or(&[]).iter().enumerate() {
            let Some(before) = ep.get(l).and_then(|e| e.get(i)) else { continue };
            for (d, label) in [(0, "antiorario"), (1, "orario")] {
                let (a, b) = (before.get(d).map_or(0, code), dirs.get(d).map_or(0, code));
                if a != b {
                    out.push(format!(
                        "{} · encoder {i}, senso {label}: {} → {}",
                        names.layer(l),
                        key(a, &ctx_prev),
                        key(b, &ctx_cur)
                    ));
                }
            }
        }
    }

    // combos, tap dances and key overrides by slot
    let (cp, cc) = (combos(prev), combos(cur));
    for i in 0..cp.len().max(cc.len()) {
        let (a, b) = (cp.get(i).copied().unwrap_or_default(), cc.get(i).copied().unwrap_or_default());
        match (a.is_active(), b.is_active()) {
            (false, true) => out.push(format!("Combo aggiunta: {}", combo_text(&b, &ctx_cur))),
            (true, false) => out.push(format!("Combo rimossa: {}", combo_text(&a, &ctx_prev))),
            (true, true) if a != b => out.push(format!(
                "Combo modificata: {} diventa {}",
                combo_text(&a, &ctx_prev),
                combo_text(&b, &ctx_cur)
            )),
            _ => {}
        }
    }
    for i in 0..td_prev.len().max(td_cur.len()) {
        let (a, b) = (td_prev.get(i).copied().unwrap_or_default(), td_cur.get(i).copied().unwrap_or_default());
        match (a.is_empty(), b.is_empty()) {
            (true, false) => out.push(format!("Tap dance {i} aggiunto: {}", tap_dance_text(&b, &ctx_cur))),
            (false, true) => out.push(format!("Tap dance {i} rimosso: {}", tap_dance_text(&a, &ctx_prev))),
            (false, false) if a != b => {
                let mut what = Vec::new();
                for ((action, x), (_, y)) in a.actions().into_iter().zip(b.actions()) {
                    if x != y {
                        what.push(format!("{} {} → {}", action.describe(), key(x, &ctx_prev), key(y, &ctx_cur)));
                    }
                }
                if a.term_ms != b.term_ms {
                    what.push(format!("tapping term {} → {} ms", a.term_ms, b.term_ms));
                }
                out.push(format!("Tap dance {i}: {}", what.join(", ")));
            }
            _ => {}
        }
    }
    let (kp, kc) = (key_overrides(prev), key_overrides(cur));
    for i in 0..kp.len().max(kc.len()) {
        let (a, b) = (kp.get(i).copied().unwrap_or_default(), kc.get(i).copied().unwrap_or_default());
        let used = |o: &KeyOverride| o.trigger != 0 || o.replacement != 0;
        match (used(&a), used(&b)) {
            (false, true) => out.push(format!("Key override aggiunto: {}", override_text(&b, &ctx_cur, names))),
            (true, false) => out.push(format!("Key override rimosso: {}", override_text(&a, &ctx_prev, names))),
            (true, true) if KeyOverride { options: a.options ^ 0x80, ..a } == b => {
                let what = if b.is_active() { "riattivato" } else { "disattivato" };
                out.push(format!(
                    "Key override {what}: {}",
                    override_text(&b, &ctx_cur, names).replace(" (disattivato)", "")
                ));
            }
            (true, true) if a != b => out.push(format!(
                "Key override modificato: {} diventa {}",
                override_text(&a, &ctx_prev, names),
                override_text(&b, &ctx_cur, names)
            )),
            _ => {}
        }
    }

    // macros by slot
    let (mp, mc) = (list(prev, "macro"), list(cur, "macro"));
    for i in 0..mp.len().max(mc.len()) {
        let (a, b) = (mp.get(i), mc.get(i));
        if a != b {
            let text = |m: Option<&Value>, ctx| m.map_or("vuota".into(), |m| macro_text(m, ctx));
            out.push(format!("Macro {i}: {} → {}", text(a, &ctx_prev), text(b, &ctx_cur)));
        }
    }

    // alt repeat keys: just which slots changed
    let (ap, ac) = (list(prev, "alt_repeat_key"), list(cur, "alt_repeat_key"));
    for i in 0..ap.len().max(ac.len()) {
        if ap.get(i) != ac.get(i) {
            out.push(format!("Alt repeat key {i} modificato"));
        }
    }

    // QMK settings
    let empty = serde_json::Map::new();
    let (sp, sc) = (
        prev.get("settings").and_then(Value::as_object).unwrap_or(&empty),
        cur.get("settings").and_then(Value::as_object).unwrap_or(&empty),
    );
    let mut ids: Vec<u64> = sp.keys().chain(sc.keys()).filter_map(|k| k.parse().ok()).collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let k = id.to_string();
        let show = |v: Option<&Value>| v.map_or("—".into(), |v| v.to_string());
        if sp.get(&k) != sc.get(&k) {
            out.push(format!("{}: {} → {}", setting_name(id), show(sp.get(&k)), show(sc.get(&k))));
        }
    }

    if prev.get("layout_options") != cur.get("layout_options") {
        out.push("Opzioni di layout cambiate".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn names() -> Vec<String> {
        ["BASE", "SYM", "NAV"].map(String::from).to_vec()
    }

    fn base() -> Value {
        json!({
            "uid": 1,
            "layout": [
                [[0x04, 0x05, -1], [0x29, 0x2A, -1]],
                [[0x01, 0x01, -1], [0x01, 0x01, -1]],
            ],
            "encoder_layout": [[[0x81, 0x80]], [[0x01, 0x01]]],
            "macro": [[], []],
            "tap_dance": [[0, 0, 0, 0, 200]],
            "combo": [[0, 0, 0, 0, 0]],
            "key_override": [{"trigger": 0x2A, "replacement": 0x4C, "layers": 0xFFFF, "trigger_mods": 0x22,
                              "negative_mod_mask": 0, "suppressed_mods": 0x22, "options": 0x87}],
            "alt_repeat_key": [],
            "settings": {"7": 200},
            "layout_options": 0,
        })
    }

    #[test]
    fn nothing_changed() {
        let n = names();
        assert!(describe(&base(), &base(), &Context { layer_names: &n, custom: &[] }).is_empty());
    }

    #[test]
    fn every_kind_of_change() {
        let n = names();
        let ctx = Context { layer_names: &n, custom: &[] };
        let mut cur = base();
        cur["layout"][1][0][1] = json!(0x011D); // SYM, on B: Ctrl+Z
        cur["layout"][0][1][0] = json!(0x2129); // BASE: Esc becomes Esc / Ctrl
        cur["encoder_layout"][0][0][1] = json!(0x0081);
        cur["combo"][0] = json!([0x04, 0x05, 0, 0, 0x2B]);
        cur["tap_dance"][0] = json!([0x04, 0xE0, 0, 0, 200]);
        cur["key_override"][0]["options"] = json!(0x07);
        cur["macro"][1] = json!([["text", "ciao"], ["tap", 0x28]]);
        cur["settings"]["7"] = json!(180);
        let lines = describe(&base(), &cur, &ctx);
        let expected = [
            "BASE · tasto Esc (tieni Ctrl) (riga 1, colonna 0): Esc → Esc (tieni Ctrl)",
            "SYM · tasto B (riga 0, colonna 1): trasparente → Ctrl+Z",
            "BASE · encoder 0, senso orario: Vol+ → Vol−",
            "Combo aggiunta: A + B insieme → Tab",
            "Tap dance 0 aggiunto: tocca A, tieni Ctrl",
            "Key override disattivato: Shift + Bksp → Del (tutti i layer)",
            "Macro 1: vuota → \"ciao\", Enter",
            "tapping term: 200 → 180",
        ];
        assert_eq!(lines, expected);
    }

    #[test]
    fn first_backup_summary() {
        assert_eq!(summary(&base()), "Primo backup: 2 layer, 0 combo, 0 tap dance, 1 key override, 0 macro");
    }
}
