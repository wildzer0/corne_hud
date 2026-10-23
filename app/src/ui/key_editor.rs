//! "Change key" window (a separate OS window): picks a keycode for one key
//! and asks the worker to write it to the keyboard. What may be written and
//! what deserves a warning is decided by `keymap_guard`.

use crate::hid::worker::KeyWrite;
use crate::i18n::{tr, Lang, Msg};
use crate::keymap_guard::{Refusal, Warning, QK_BOOT};
use crate::tr_format;
use crate::vial::keycode::{qmk_name, KC_NO, KC_TRNS, QK_KB_0};
use crate::vial::legend::{legend, LegendContext};
use crate::vial::names::{parse_keycode, QMK_KEYCODES};
use egui::{Color32, RichText, ViewportBuilder, ViewportId};

/// Viewport of the key editor window.
const VIEWPORT: &str = "hud-key-editor";
/// Results listed at most.
const MAX_RESULTS: usize = 60;

/// Layer keycodes offered for every layer: MO, TG, TO, OSL, TT, DF.
const LAYER_BASES: [u16; 6] = [0x5220, 0x5260, 0x5200, 0x5280, 0x52C0, 0x5240];
const TAP_DANCE_BASE: u16 = 0x5700;
const MACRO_BASE: u16 = 0x7700;

/// One keycode the editor can offer.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub code: u16,
    /// e.g. `KC_ESC`, `MO(2)`, `USER05`.
    pub name: String,
    /// Other names it is searched by (QMK aliases).
    pub aliases: Vec<String>,
    /// What it does, e.g. "Esc (KC_ESC)".
    pub description: String,
}

/// The keycodes to choose from: QMK's named ones, the layer keys of every
/// layer, tap dances, macros and the keyboard's own keycodes.
pub fn catalog(layers: u8, tap_dances: usize, macros: usize, ctx: &LegendContext) -> Vec<Candidate> {
    let describe = |code: u16| legend(code, ctx).tooltip.lines().next().unwrap_or_default().to_string();
    let mut out: Vec<Candidate> = Vec::new();
    let mut add = |code: u16, name: String, aliases: Vec<String>| {
        if code != QK_BOOT && !out.iter().any(|c| c.code == code) {
            out.push(Candidate { code, name, aliases, description: describe(code) });
        }
    };
    add(KC_NO, "KC_NO".into(), vec!["XXXXXXX".into()]);
    add(KC_TRNS, "KC_TRNS".into(), vec!["_______".into()]);
    for &(code, key, _, aliases) in QMK_KEYCODES {
        // the keyboard's own keycodes come with their names below
        if (QK_KB_0..QK_KB_0 + 64).contains(&code) {
            continue;
        }
        let aliases: Vec<String> = aliases.iter().filter(|a| !a.starts_with('!')).map(|a| a.to_string()).collect();
        let mut names = vec![key.to_string()];
        names.extend(aliases);
        add(code, qmk_name(code), names);
    }
    for layer in 0..layers.min(32) {
        for base in LAYER_BASES {
            let code = base + layer as u16;
            add(code, qmk_name(code), Vec::new());
        }
    }
    for n in 0..tap_dances.min(256) {
        add(TAP_DANCE_BASE + n as u16, format!("TD({n})"), Vec::new());
    }
    for n in 0..macros.min(256) {
        add(MACRO_BASE + n as u16, format!("M{n}"), Vec::new());
    }
    for (n, c) in ctx.custom.iter().enumerate().take(64) {
        let names = [c.name.clone(), c.short_name.clone(), format!("QK_KB_{n}")];
        add(QK_KB_0 + n as u16, format!("USER{n:02}"), names.into_iter().filter(|s| !s.is_empty()).collect());
    }
    out
}

/// Candidates matching `query`, best first. A keycode typed by hand that is
/// not in the catalog (`C(KC_Z)`, `LT1(KC_SPC)`...) comes first.
pub fn matches<'a>(catalog: &'a [Candidate], query: &str) -> (Option<u16>, Vec<&'a Candidate>) {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (None, Vec::new());
    }
    let typed = parse_keycode(query.trim()).filter(|&c| c != QK_BOOT);
    let score = |c: &Candidate| -> u32 {
        let names = std::iter::once(&c.name).chain(&c.aliases).map(|n| n.to_lowercase());
        let mut best = 0;
        for n in names {
            let bare = n.strip_prefix("kc_").unwrap_or(&n).to_string();
            best = best.max(match () {
                _ if n == q || bare == q => 100,
                _ if n.starts_with(&q) || bare.starts_with(&q) => 60,
                _ if n.contains(&q) => 30,
                _ => 0,
            });
        }
        let d = c.description.to_lowercase();
        best.max(match () {
            _ if d == q => 90,
            _ if d.starts_with(&q) => 50,
            _ if q.chars().count() >= 2 && d.contains(&q) => 20,
            _ => 0,
        })
    };
    let mut hits: Vec<(u32, &Candidate)> = catalog.iter().map(|c| (score(c), c)).filter(|(s, _)| *s > 0).collect();
    // stable: equal scores keep the catalog order (basic keys first)
    hits.sort_by_key(|h| std::cmp::Reverse(h.0));
    let typed = typed.filter(|t| !hits.first().is_some_and(|(s, c)| *s >= 100 && c.code == *t));
    (typed, hits.into_iter().take(MAX_RESULTS).map(|(_, c)| c).collect())
}

/// Where the key is and what it holds now.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
    /// Keycode on that layer (not resolved through transparent keys).
    pub shown: u16,
    /// e.g. "NAV · riga 2, colonna 4".
    pub place: String,
}

/// What the window asks the app to do.
pub enum Action {
    None,
    Write(KeyWrite),
}

/// The checks for the selected keycode, done by the app (it has the keymap).
pub type Check<'a> = &'a dyn Fn(u16) -> Result<Vec<Warning>, Refusal>;

#[derive(Default)]
pub struct KeyEditor {
    pub target: Option<Target>,
    catalog: Vec<Candidate>,
    query: String,
    selected: Option<u16>,
    confirmed: bool,
    /// A write was sent, the answer has not come yet.
    pub writing: Option<KeyWrite>,
    pub error: Option<String>,
    focus_query: bool,
}

pub fn warning_text(lang: Lang, w: Warning, layer_name: &dyn Fn(u8) -> String) -> String {
    match w {
        Warning::LayerUnreachable(l) => tr_format!(
            "Il layer {} non sarà più raggiungibile da nessun tasto.",
            "Layer {} will no longer be reachable from any key.",
            layer_name(l)
        ),
        Warning::LastBootloader => tr(lang, Msg::EditKeyLastBoot).to_string(),
        Warning::LastHudToggle => tr(lang, Msg::EditKeyLastHudTog).to_string(),
        Warning::EmptyOnBase => tr(lang, Msg::EditKeyEmptyBase).to_string(),
    }
}

pub fn refusal_text(lang: Lang, r: Refusal) -> &'static str {
    match r {
        Refusal::Bootloader => tr(lang, Msg::EditKeyBootloader),
        Refusal::Unchanged => tr(lang, Msg::EditKeyUnchanged),
    }
}

impl KeyEditor {
    pub fn open(&mut self, target: Target, catalog: Vec<Candidate>) {
        *self = KeyEditor { target: Some(target), catalog, focus_query: true, ..KeyEditor::default() };
    }

    /// Development aid: searches for `query` and picks the first result.
    pub fn search_and_pick(&mut self, query: &str) {
        self.query = query.to_string();
        let (typed, hits) = matches(&self.catalog, query);
        self.selected = typed.or_else(|| hits.first().map(|c| c.code));
    }

    pub fn close(&mut self) {
        *self = KeyEditor::default();
    }

    pub fn is_open(&self) -> bool {
        self.target.is_some()
    }

    /// `available`: connected and not paused. `backups`: automatic backups on.
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        lang: Lang,
        legends: &LegendContext,
        available: bool,
        backups: bool,
        check: Check,
        layer_name: &dyn Fn(u8) -> String,
    ) -> Action {
        let Some(target) = self.target.clone() else { return Action::None };
        let mut action = Action::None;
        let mut close = false;
        let builder = ViewportBuilder::default()
            .with_title(format!("Keyboard HUD · {}", tr(lang, Msg::EditKeyTitle)))
            .with_inner_size([460.0, 560.0]);
        ctx.show_viewport_immediate(ViewportId::from_hash_of(VIEWPORT), builder, |ui, _class| {
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                close = true;
            }
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading(&target.place);
                let now = legend(target.shown, legends);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr(lang, Msg::EditKeyNow)).weak());
                    ui.strong(&now.tap.text);
                    ui.label(qmk_name(target.shown));
                });
                ui.separator();
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(tr(lang, Msg::EditKeySearch))
                        .desired_width(f32::INFINITY),
                );
                if std::mem::take(&mut self.focus_query) {
                    r.request_focus();
                }
                if r.changed() {
                    self.confirmed = false;
                }
                ui.label(RichText::new(tr(lang, Msg::EditKeyExamples)).small().weak());
                let (typed, hits) = matches(&self.catalog, &self.query);
                egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
                    if let Some(code) = typed {
                        let text = format!("{}  ·  {}", qmk_name(code), tr(lang, Msg::EditKeyTyped));
                        if ui.selectable_label(self.selected == Some(code), text).clicked() {
                            self.selected = Some(code);
                            self.confirmed = false;
                        }
                    }
                    for c in &hits {
                        let text = if c.description.is_empty() || c.description == c.name {
                            c.name.clone()
                        } else {
                            format!("{}  ·  {}", c.name, c.description)
                        };
                        if ui.selectable_label(self.selected == Some(c.code), text).clicked() {
                            self.selected = Some(c.code);
                            self.confirmed = false;
                        }
                    }
                    if typed.is_none() && hits.is_empty() && !self.query.trim().is_empty() {
                        ui.label(RichText::new(tr(lang, Msg::EditKeyNoMatch)).weak());
                    }
                });
                ui.separator();
                let warn = Color32::from_rgb(0xE0, 0x9A, 0x30);
                let bad = Color32::from_rgb(0xE0, 0x55, 0x55);
                let verdict = self.selected.map(|code| (code, check(code)));
                if let Some((code, _)) = verdict {
                    let new = legend(code, legends);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr(lang, Msg::EditKeyNew)).weak());
                        ui.strong(&new.tap.text);
                        ui.label(qmk_name(code));
                    });
                    ui.label(RichText::new(new.tooltip).small());
                }
                let mut can_write = available && self.writing.is_none();
                match &verdict {
                    None => can_write = false,
                    Some((_, Err(r))) => {
                        ui.colored_label(bad, refusal_text(lang, *r));
                        can_write = false;
                    }
                    Some((_, Ok(warnings))) => {
                        for &w in warnings {
                            ui.colored_label(warn, format!("⚠ {}", warning_text(lang, w, layer_name)));
                        }
                        if !warnings.is_empty() {
                            ui.checkbox(&mut self.confirmed, tr(lang, Msg::EditKeyConfirm));
                            can_write &= self.confirmed;
                        }
                    }
                }
                if !available {
                    ui.colored_label(bad, tr(lang, Msg::EditKeyUnavailable));
                } else if !backups {
                    ui.label(RichText::new(tr(lang, Msg::EditKeyNoBackup)).small().weak());
                }
                if let Some(e) = &self.error {
                    ui.colored_label(bad, e);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(can_write, egui::Button::new(tr(lang, Msg::EditKeyWrite))).clicked() {
                        if let Some((code, _)) = verdict {
                            let w = KeyWrite {
                                layer: target.layer,
                                row: target.row,
                                col: target.col,
                                code,
                                shown: target.shown,
                            };
                            self.writing = Some(w);
                            self.error = None;
                            action = Action::Write(w);
                        }
                    }
                    if ui.button(tr(lang, Msg::EditKeyCancel)).clicked() {
                        close = true;
                    }
                    if self.writing.is_some() {
                        ui.spinner();
                        ui.label(tr(lang, Msg::EditKeyWriting));
                    }
                });
            });
        });
        if close {
            self.close();
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vial::definition::CustomKeycode;

    fn ctx<'a>(names: &'a [String], custom: &'a [CustomKeycode]) -> LegendContext<'a> {
        LegendContext { layer_names: names, custom, tap_dances: &[], macros: &[] }
    }

    #[test]
    fn catalog_and_search() {
        let names = vec!["BASE".to_string(), "SYM".to_string(), "NAV".to_string()];
        let custom = vec![CustomKeycode { name: "HUD_TOG".into(), title: "HUD".into(), short_name: "HUD".into() }];
        let cat = catalog(3, 2, 1, &ctx(&names, &custom));
        // no bootloader, every layer key, tap dances, macros, custom keycodes
        assert!(!cat.iter().any(|c| c.code == QK_BOOT));
        for name in ["MO(2)", "TG(1)", "OSL(0)", "TD(1)", "M0", "USER00", "KC_ESC", "KC_NO", "KC_TRNS"] {
            assert!(cat.iter().any(|c| c.name == name), "{name}");
        }
        assert!(!cat.iter().any(|c| c.name == "TD(2)" || c.name == "MO(3)"));
        // codes are unique
        let mut codes: Vec<u16> = cat.iter().map(|c| c.code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), cat.len());

        let (typed, hits) = matches(&cat, "esc");
        assert_eq!(typed, None);
        assert_eq!(hits[0].code, 0x29);
        let (_, hits) = matches(&cat, "hud_tog");
        assert_eq!(hits[0].code, QK_KB_0);
        let (_, hits) = matches(&cat, "mo(2)");
        assert_eq!(hits[0].code, 0x5222);
        // typed by hand: not in the catalog, offered first
        let (typed, _) = matches(&cat, "C(KC_Z)");
        assert_eq!(typed, Some(0x011D));
        let (typed, _) = matches(&cat, "LCTL_T(KC_A)");
        assert_eq!(typed, Some(0x2104));
        // the bootloader cannot be typed either
        assert_eq!(matches(&cat, "QK_BOOT").0, None);
        assert_eq!(matches(&cat, "  ").1.len(), 0);
    }
}
