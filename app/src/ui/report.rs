//! Statistics window (a separate OS window): layers, hands, fingers, keys
//! never used, CSV export, reset.

use super::Layout;
use crate::clock;
use crate::i18n::{tr, Lang, Msg};
use crate::labels::Labels;
use crate::stats::{self, Stats, FINGERS, ROWS};
use crate::vial::legend::legend;
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

/// Bars, one per day, with the date and count on hover.
fn daily_chart(ui: &mut egui::Ui, daily: &[(i64, u64)], lang: Lang) {
    let max = daily.iter().map(|d| d.1).max().unwrap_or(0).max(1);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(400.0, 60.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let w = rect.width() / daily.len().max(1) as f32;
    let color = ui.visuals().selection.bg_fill;
    let mut hovered = None;
    for (i, &(day, n)) in daily.iter().enumerate() {
        let h = rect.height() * n as f32 / max as f32;
        let x = rect.left() + i as f32 * w;
        let bar =
            egui::Rect::from_min_max(egui::pos2(x + 1.0, rect.bottom() - h), egui::pos2(x + w - 1.0, rect.bottom()));
        painter.rect_filled(bar, 1.0, color);
        if response.hover_pos().is_some_and(|p| p.x >= x && p.x < x + w) {
            hovered = Some((day, n));
        }
    }
    painter.line_segment([rect.left_bottom(), rect.right_bottom()], ui.visuals().widgets.noninteractive.bg_stroke);
    if let Some((day, n)) = hovered {
        response.on_hover_text(format!("{}: {n} {}", clock::date_string(day), tr(lang, Msg::Presses)));
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
        ui.add_space(4.0);
        ui.strong(tr(lang, Msg::StatsRows));
        for (row, n) in ROWS.iter().zip(r.rows) {
            ui.horizontal(|ui| {
                ui.add_sized([70.0, 18.0], egui::Label::new(row.name()));
                let p = pct(n, r.total);
                ui.add(egui::ProgressBar::new(p).text(format!("{:.0}%", p * 100.0)).desired_width(330.0));
            });
        }
        ui.label(format!("{}: {:.2}", tr(lang, Msg::StatsEffort), r.avg_effort));
        ui.separator();

        let ctx = layout.legend_context(input.layer_names);
        let layer_name = |l: u8| input.layer_names.get(l as usize).cloned().unwrap_or_else(|| format!("L{l}"));
        // what the key does on that layer: a transparent key is the one below
        let key_text = |layer: u8, row: u8, col: u8| {
            let (code, from) = keymap.resolve(layer, (1 << layer) | 1, row, col);
            let mut leg = legend(code, &ctx);
            input.labels.apply(&mut leg, layer, row, col, code, from);
            leg.tap.text
        };
        ui.strong(tr(lang, Msg::StatsTop));
        egui::Grid::new("top").striped(true).show(ui, |ui| {
            for &((layer, row, col), n) in &r.top {
                ui.label(key_text(layer, row, col));
                ui.label(layer_name(layer));
                ui.label(crate::search::sequence(keymap, &ctx, input.labels, layer, row, col));
                ui.label(format!("{n}"));
                ui.end_row();
            }
        });
        ui.separator();

        ui.strong(tr(lang, Msg::StatsDaily));
        daily_chart(ui, &r.daily, lang);
        ui.separator();

        ui.strong(tr(lang, Msg::StatsSuggestions));
        let mut any = false;
        let left = pct(r.hands[0], r.total);
        if r.total >= 500 && (left - 0.5).abs() > 0.1 {
            let (hand, share) = if left > 0.5 { (Msg::Left, left) } else { (Msg::Right, 1.0 - left) };
            ui.label(format!("• {} {:.0}%", tr(lang, hand), share * 100.0));
            any = true;
        }
        for s in &r.suggestions {
            let place = |(row, col): (u8, u8)| {
                let (_, f) = stats::finger_of(row, col, keymap.rows());
                format!("{}, {}", f.name(), stats::row_of(row, keymap.rows()).name())
            };
            let target = match keymap.get(s.layer, s.to.0, s.to.1) {
                crate::vial::keycode::KC_NO => tr(lang, Msg::StatsFreeSpot).to_string(),
                _ => format!("{} ({} {})", key_text(s.layer, s.to.0, s.to.1), s.to_presses, tr(lang, Msg::Presses)),
            };
            ui.label(format!(
                "• {}: {} ({}, {} {}) ↔ {target} ({}): −{:.1}%",
                layer_name(s.layer),
                key_text(s.layer, s.from.0, s.from.1),
                place(s.from),
                s.from_presses,
                tr(lang, Msg::Presses),
                place(s.to),
                s.saving_pct
            ));
            any = true;
        }
        if !any {
            ui.label(tr(lang, Msg::StatsNoSuggestions));
        }
        ui.label(RichText::new(tr(lang, Msg::StatsSuggestionHint)).small().weak());
        ui.separator();

        ui.strong(format!("{} ({} {})", tr(lang, Msg::StatsUnused), input.days, tr(lang, Msg::Days)));
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
