//! Diagnostics window (a separate OS window): keyboard, firmware, link
//! quality, files. Everything can be copied as text for a bug report.

use crate::i18n::{tr, Lang, Msg};
use egui::{RichText, ViewportBuilder, ViewportId};
use std::time::Duration;

/// Viewport of the diagnostics window.
const VIEWPORT: &str = "hud-diagnostics";

/// A titled group of `label: value` rows.
pub struct Section {
    pub title: String,
    pub rows: Vec<(String, String)>,
}

impl Section {
    pub fn new(title: &str) -> Self {
        Section { title: title.to_string(), rows: Vec::new() }
    }

    pub fn row(&mut self, label: &str, value: impl Into<String>) {
        self.rows.push((label.to_string(), value.into()));
    }
}

/// The sections as plain text.
pub fn as_text(sections: &[Section]) -> String {
    let mut out = String::new();
    for s in sections {
        out.push_str(&format!("[{}]\n", s.title));
        for (label, value) in &s.rows {
            out.push_str(&format!("{label}: {value}\n"));
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// "850 ms", "12 s", "5 min", "3 h".
pub fn short_duration(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        0 => format!("{} ms", d.as_millis()),
        1..=119 => format!("{s} s"),
        120..=7199 => format!("{} min", s / 60),
        _ => format!("{} h", s / 3600),
    }
}

pub enum Action {
    None,
    Copied,
    OpenFolder,
}

#[derive(Default)]
pub struct DiagnosticsWindow {
    pub open: bool,
}

impl DiagnosticsWindow {
    pub fn show(&mut self, ctx: &egui::Context, lang: Lang, sections: &[Section]) -> Action {
        if !self.open {
            return Action::None;
        }
        let mut action = Action::None;
        let builder = ViewportBuilder::default()
            .with_title(format!("Keyboard HUD · {}", tr(lang, Msg::DiagTitle)))
            .with_inner_size([480.0, 560.0]);
        ctx.show_viewport_immediate(ViewportId::from_hash_of(VIEWPORT), builder, |ui, _class| {
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                self.open = false;
            }
            egui::CentralPanel::default().show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, Msg::DiagCopy)).clicked() {
                        ui.ctx().copy_text(as_text(sections));
                        action = Action::Copied;
                    }
                    if ui.button(tr(lang, Msg::DiagOpenFolder)).clicked() {
                        action = Action::OpenFolder;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, s) in sections.iter().enumerate() {
                        ui.label(RichText::new(&s.title).strong());
                        egui::Grid::new(("diag", i)).num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                            for (label, value) in &s.rows {
                                ui.label(RichText::new(label).weak());
                                ui.label(value);
                                ui.end_row();
                            }
                        });
                        ui.add_space(10.0);
                    }
                });
            });
        });
        // live figures: refresh while open
        ctx.request_repaint_after(Duration::from_millis(500));
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_durations() {
        let mut s = Section::new("Tastiera");
        s.row("Stato", "collegata");
        s.row("Latenza", "1.2 ms");
        assert_eq!(as_text(&[s]), "[Tastiera]\nStato: collegata\nLatenza: 1.2 ms");
        assert_eq!(short_duration(Duration::from_millis(850)), "850 ms");
        assert_eq!(short_duration(Duration::from_secs(12)), "12 s");
        assert_eq!(short_duration(Duration::from_secs(300)), "5 min");
        assert_eq!(short_duration(Duration::from_secs(3 * 3600 + 5)), "3 h");
    }
}
