//! Search box over the keyboard: results with layer and key sequence; the
//! keyboard shows the selected result's layer with the matching keys marked.

use super::theme::Palette;
use crate::i18n::{tr, Lang, Msg};
use crate::labels::Labels;
use crate::model::Keymap;
use crate::search::{search, Hit};
use crate::vial::dynamic::Dynamic;
use crate::vial::legend::LegendContext;
use egui::{Align2, Color32, Key, RichText, Vec2};

const MAX_SHOWN: usize = 6;

#[derive(Default)]
pub struct SearchPanel {
    pub open: bool,
    query: String,
    searched: Option<String>,
    hits: Vec<Hit>,
    selected: usize,
    focus: bool,
}

impl SearchPanel {
    pub fn open(&mut self) {
        self.open = true;
        self.focus = true;
    }

    pub fn open_with(&mut self, query: String) {
        self.query = query;
        self.open();
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn selected(&self) -> Option<&Hit> {
        self.open.then(|| self.hits.get(self.selected)).flatten()
    }

    /// Mark for the keyboard: 1 for the selected result, 0.4 for the others on its layer.
    pub fn mark(&self, layer: u8, row: u8, col: u8) -> f32 {
        let Some(sel) = self.selected() else { return 0.0 };
        let is = |h: &Hit| h.layer == layer && ((h.row, h.col) == (row, col) || h.also.contains(&(row, col)));
        if is(sel) {
            1.0
        } else if self.hits.iter().take(MAX_SHOWN).any(is) {
            0.4
        } else {
            0.0
        }
    }

    /// Re-runs the search when the query (or the keymap) changed.
    pub fn refresh(
        &mut self,
        keymap: &Keymap,
        positions: &[(u8, u8)],
        ctx: &LegendContext,
        labels: &Labels,
        dynamic: &Dynamic,
        force: bool,
    ) {
        if self.open && (force || self.searched.as_deref() != Some(self.query.as_str())) {
            self.hits = search(&self.query, keymap, positions, ctx, labels, dynamic);
            self.selected = 0;
            self.searched = Some(self.query.clone());
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, lang: Lang, palette: &Palette, layer_name: &dyn Fn(u8) -> String) {
        if !self.open {
            return;
        }
        let (up, down, enter, esc) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
            )
        });
        if esc {
            self.close();
            return;
        }
        let shown = self.hits.len().min(MAX_SHOWN);
        if down && self.selected + 1 < shown {
            self.selected += 1;
        }
        if up && self.selected > 0 {
            self.selected -= 1;
        }
        if enter {
            self.close();
            return;
        }
        egui::Area::new(egui::Id::unique("hud-search"))
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -24.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    // fits small HUDs too (low scale, compact view)
                    ui.set_width((ctx.content_rect().width() - 40.0).clamp(120.0, 360.0));
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text(tr(lang, Msg::SearchHint))
                            .desired_width(f32::INFINITY),
                    );
                    if self.focus {
                        edit.request_focus();
                        self.focus = false;
                    }
                    if self.query.trim().is_empty() {
                        return;
                    }
                    if self.hits.is_empty() {
                        ui.label(RichText::new(tr(lang, Msg::SearchNoResults)).color(palette.text_dim));
                        return;
                    }
                    for (i, h) in self.hits.iter().take(MAX_SHOWN).enumerate() {
                        let text = format!("{}  ·  {}  ·  {}", h.shown, layer_name(h.layer), h.sequence);
                        let mut rich = RichText::new(text);
                        if i == self.selected {
                            rich = rich.strong().color(palette.accent);
                        }
                        if ui.selectable_label(i == self.selected, rich).clicked() {
                            self.selected = i;
                        }
                    }
                    if self.hits.len() > MAX_SHOWN {
                        let more = format!("+{} …", self.hits.len() - MAX_SHOWN);
                        ui.label(RichText::new(more).color(Color32::GRAY).small());
                    }
                });
            });
    }
}
