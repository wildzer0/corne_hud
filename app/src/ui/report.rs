//! Statistics window (a separate OS window): layers, hands, fingers, keys
//! never used, CSV export, reset.

use super::Layout;
use crate::clock;
use crate::i18n::{tr, Lang, Msg};
use crate::labels::Labels;
use crate::stats::{self, Stats, FINGERS};
use crate::vial::legend::{legend, LegendContext};
use egui::{RichText, ViewportBuilder, ViewportId};
use std::path::PathBuf;

/// Viewport of the statistics window.
const VIEWPORT: &str = "hud-stats";

#[derive(Default)]
pub struct ReportWindow {
    pub open: bool,
    confirm_reset: bool,
    message: Option<String>,
}

pub struct ReportInput<'a> {
    pub lang: Lang,
    pub layout: Option<&'a Layout>,
    pub positions: &'a [(u8, u8)],
    pub layer_names: &'a [String],
    pub labels: &'a Labels,
    pub days: u32,
    pub csv_path: PathBuf,
}

fn pct(n: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        n as f32 / total as f32
    }
}

impl ReportWindow {
    pub fn show(&mut self, ctx: &egui::Context, mut stats: Option<&mut Stats>, input: &ReportInput) {
        if !self.open {
            return;
        }
        let lang = input.lang;
        let builder = ViewportBuilder::default()
            .with_title(format!("Keyboard HUD · {}", tr(lang, Msg::StatsTitle)))
            .with_inner_size([520.0, 620.0]);
        ctx.show_viewport_immediate(ViewportId::from_hash_of(VIEWPORT), builder, |ui, _class| {
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                self.open = false;
            }
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.contents(ui, stats.as_deref_mut(), input));
            });
        });
    }

    fn contents(&mut self, ui: &mut egui::Ui, stats: Option<&mut Stats>, input: &ReportInput) {
        let lang = input.lang;
        ui.heading(tr(lang, Msg::StatsTitle));
        let (Some(stats), Some(layout)) = (stats, input.layout) else {
            ui.label(tr(lang, Msg::StatsDisabled));
            return;
        };
        let today = clock::today();
        let keymap = &layout.keymap;
        let r = stats::report(stats, keymap, input.positions, input.days, today);
        ui.label(format!(
            "{} {} {}: {} · {}: {}",
            tr(lang, Msg::StatsPeriod),
            input.days,
            tr(lang, Msg::Days),
            r.total,
            tr(lang, Msg::StatsAllTime),
            r.all_time
        ));
        ui.label(RichText::new(tr(lang, Msg::StatsPrivacy)).small().weak());
        ui.separator();

        ui.strong(tr(lang, Msg::StatsLayers));
        for (l, n) in r.per_layer.iter().enumerate() {
            let name = input.layer_names.get(l).cloned().unwrap_or_else(|| format!("L{l}"));
            ui.horizontal(|ui| {
                ui.add_sized([70.0, 18.0], egui::Label::new(name));
                ui.add(egui::ProgressBar::new(pct(*n, r.total)).text(format!("{n}")).desired_width(330.0));
            });
        }
        ui.separator();

        ui.strong(tr(lang, Msg::StatsHands));
        let hands = [(Msg::Left, r.hands[0]), (Msg::Right, r.hands[1])];
        for (m, n) in hands {
            ui.horizontal(|ui| {
                ui.add_sized([70.0, 18.0], egui::Label::new(tr(lang, m)));
                let p = pct(n, r.total);
                ui.add(egui::ProgressBar::new(p).text(format!("{:.0}%", p * 100.0)).desired_width(330.0));
            });
        }
        ui.add_space(4.0);
        ui.strong(tr(lang, Msg::StatsFingers));
        egui::Grid::new("fingers").striped(true).show(ui, |ui| {
            ui.label("");
            ui.label(tr(lang, Msg::Left));
            ui.label(tr(lang, Msg::Right));
            ui.end_row();
            for (i, f) in FINGERS.iter().enumerate() {
                ui.label(f.name());
                for h in 0..2 {
                    let n = r.fingers[h][i];
                    ui.label(format!("{n} ({:.0}%)", pct(n, r.total) * 100.0));
                }
                ui.end_row();
            }
        });
        ui.separator();

        ui.strong(format!("{} ({} {})", tr(lang, Msg::StatsUnused), input.days, tr(lang, Msg::Days)));
        let ctx = LegendContext { layer_names: input.layer_names, custom: &layout.definition.custom };
        let mut current = None;
        let mut line = String::new();
        let flush = |ui: &mut egui::Ui, layer: Option<u8>, line: &mut String| {
            if let Some(l) = layer {
                let name = input.layer_names.get(l as usize).cloned().unwrap_or_else(|| format!("L{l}"));
                ui.label(format!("{name}: {line}"));
            }
            line.clear();
        };
        for &((layer, row, col), code) in &r.unused {
            if current != Some(layer) {
                flush(ui, current, &mut line);
                current = Some(layer);
            }
            let mut leg = legend(code, &ctx);
            input.labels.apply(&mut leg, layer, row, col, code, layer);
            if !line.is_empty() {
                line.push_str("  ");
            }
            line.push_str(&leg.tap.text);
        }
        flush(ui, current, &mut line);
        if r.unused.is_empty() {
            ui.label("—");
        }
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button(tr(lang, Msg::StatsExport)).clicked() {
                let text = stats::csv(stats, keymap, input.positions, input.layer_names, input.days, today);
                self.message = Some(match std::fs::write(&input.csv_path, text) {
                    Ok(()) => format!("{}: {}", tr(lang, Msg::StatsExported), input.csv_path.display()),
                    Err(e) => format!("{}: {e}", input.csv_path.display()),
                });
            }
            if self.confirm_reset {
                if ui.button(RichText::new(tr(lang, Msg::StatsResetConfirm)).color(egui::Color32::RED)).clicked() {
                    stats.reset();
                    self.confirm_reset = false;
                    self.message = Some(tr(lang, Msg::StatsResetDone).to_string());
                }
                if ui.button(tr(lang, Msg::Cancel)).clicked() {
                    self.confirm_reset = false;
                }
            } else if ui.button(tr(lang, Msg::StatsReset)).clicked() {
                self.confirm_reset = true;
            }
        });
        if let Some(m) = &self.message {
            ui.label(RichText::new(m).small());
        }
        ui.label(RichText::new(stats.path().display().to_string()).small().weak());
    }
}
