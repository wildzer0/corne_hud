//! Training window (a separate OS window): the question, a hint after a
//! while, the score at the end. The keys come from the HUD's state, see
//! `HudApp::train_presses`.

use crate::i18n::{tr, Lang, Msg};
use crate::tr_format;
use crate::training::{Outcome, Session};
use crate::vial::legend::{legend, LegendContext};
use egui::{Color32, RichText, ViewportBuilder, ViewportId};
use std::time::{Duration, Instant};

/// Viewport of the training window.
const VIEWPORT: &str = "hud-training";
/// How long "Right!" or "No" stays.
const FEEDBACK: Duration = Duration::from_millis(900);

pub struct TrainingInput<'a> {
    pub lang: Lang,
    pub legends: &'a LegendContext<'a>,
    /// How to type the current question, e.g. "Lower + D".
    pub hint: Option<String>,
    /// Live state from a HUD firmware, not paused.
    pub available: bool,
}

pub enum Action {
    None,
    Start,
}

#[derive(Default)]
pub struct TrainingWindow {
    pub open: bool,
    pub session: Option<Session>,
    feedback: Option<(Outcome, Instant)>,
}

fn seconds(d: Duration) -> String {
    format!("{:.1} s", d.as_secs_f32())
}

impl TrainingWindow {
    /// A round is going on: the HUD stays on screen.
    pub fn active(&self) -> bool {
        self.open && self.session.as_ref().is_some_and(|s| !s.finished())
    }

    pub fn feedback(&mut self, outcome: Outcome, now: Instant) {
        if outcome != Outcome::Ignored {
            self.feedback = Some((outcome, now));
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, input: &TrainingInput) -> Action {
        if !self.open {
            return Action::None;
        }
        let lang = input.lang;
        let now = Instant::now();
        let mut action = Action::None;
        let builder = ViewportBuilder::default()
            .with_title(format!("Keyboard HUD · {}", tr(lang, Msg::TrainTitle)))
            .with_inner_size([420.0, 460.0]);
        ctx.show_viewport_immediate(ViewportId::from_hash_of(VIEWPORT), builder, |ui, _class| {
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                self.open = false;
                self.session = None;
            }
            egui::CentralPanel::default().show(ui, |ui| {
                let running = self.session.as_ref().is_some_and(|s| !s.finished());
                if !running {
                    ui.label(tr(lang, Msg::TrainIntro));
                    ui.add_space(8.0);
                    if let Some(s) = self.session.as_ref().filter(|s| !s.answers.is_empty()) {
                        let sum = s.summary();
                        ui.heading(tr_format!(
                            "{} su {} al primo colpo",
                            "{} of {} at the first try",
                            sum.clean,
                            sum.answered
                        ));
                        ui.label(tr_format!("Tempo medio: {}", "Average time: {}", seconds(sum.average)));
                        if !sum.hardest.is_empty() {
                            ui.add_space(6.0);
                            ui.label(RichText::new(tr(lang, Msg::TrainHardest)).strong());
                            for a in &sum.hardest {
                                let leg = legend(a.target.code, input.legends);
                                let what = leg.tooltip.lines().next().unwrap_or_default().to_string();
                                ui.label(format!("{} · {what}", leg.tap.text));
                            }
                        }
                        ui.add_space(8.0);
                    } else if self.session.is_some() {
                        ui.label(tr(lang, Msg::TrainEmpty));
                    }
                    let start = if self.session.is_some() { Msg::TrainAgain } else { Msg::TrainStart };
                    if ui.add_enabled(input.available, egui::Button::new(tr(lang, start))).clicked() {
                        action = Action::Start;
                    }
                    if !input.available {
                        ui.colored_label(Color32::from_rgb(0xE0, 0x55, 0x55), tr(lang, Msg::TrainUnavailable));
                    }
                    return;
                }
                let Some(s) = self.session.as_mut() else { return };
                let Some(target) = s.current() else { return };
                ui.label(
                    RichText::new(tr_format!("Domanda {} di {}", "Question {} of {}", s.answers.len() + 1, s.total()))
                        .weak(),
                );
                ui.add_space(10.0);
                let leg = legend(target.code, input.legends);
                let big = if leg.tap.symbol.is_empty() { &leg.tap.text } else { &leg.tap.symbol };
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(tr(lang, Msg::TrainPress)).weak());
                    ui.label(RichText::new(big).size(64.0).strong());
                    ui.label(leg.tooltip.lines().next().unwrap_or_default());
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(seconds(s.elapsed(now)));
                    if s.wrong() > 0 {
                        ui.label(tr_format!("· {} errori", "· {} wrong", s.wrong()));
                    }
                });
                if s.hint_due(now) {
                    if let Some(h) = &input.hint {
                        ui.label(RichText::new(format!("{}: {h}", tr(lang, Msg::TrainHowTo))).strong());
                    }
                }
                match self.feedback.filter(|(_, t)| now.duration_since(*t) < FEEDBACK) {
                    Some((Outcome::Correct, _)) => {
                        ui.colored_label(Color32::from_rgb(0x4C, 0xC0, 0x6A), tr(lang, Msg::TrainRight));
                    }
                    Some((Outcome::Wrong, _)) => {
                        ui.colored_label(Color32::from_rgb(0xE0, 0x55, 0x55), tr(lang, Msg::TrainWrong));
                    }
                    _ => {
                        ui.label("");
                    }
                }
                ui.add_space(10.0);
                let (mut skip, mut stop) = (false, false);
                ui.horizontal(|ui| {
                    skip = ui.button(tr(lang, Msg::TrainSkip)).clicked();
                    stop = ui.button(tr(lang, Msg::TrainStop)).clicked();
                });
                if skip {
                    s.skip(now);
                }
                if stop {
                    self.session = None;
                }
            });
        });
        if self.active() {
            // the timer and the hint
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        action
    }
}
