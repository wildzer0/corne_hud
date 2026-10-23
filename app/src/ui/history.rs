//! Backup history window (a separate OS window): two backups side by side,
//! the keys that changed marked on the keyboard, every other change in
//! words. A key can be put back through the key editor.

use super::keyboard::{self, KeyboardView};
use super::theme::Palette;
use crate::backup::{self, BackupFile};
use crate::changes::{self, KeyChange};
use crate::i18n::{tr, Lang, Msg};
use crate::kle::KleKey;
use crate::labels::Labels;
use crate::vial::definition::CustomKeycode;
use crate::vial::legend::LegendContext;
use crate::vial::vil::Vil;
use egui::{Color32, RichText, Sense, Vec2, ViewportBuilder, ViewportId};
use std::path::{Path, PathBuf};

/// Viewport of the history window.
const VIEWPORT: &str = "hud-history";

pub struct HistoryInput<'a> {
    pub lang: Lang,
    pub dir: &'a Path,
    /// Key geometry and matrix size, when a layout is known.
    pub keys: Option<&'a [KleKey]>,
    pub rows: u8,
    pub cols: u8,
    pub layer_names: &'a [String],
    pub layer_color: &'a dyn Fn(u8) -> Option<Color32>,
    pub custom: &'a [CustomKeycode],
    pub labels: &'a Labels,
    pub palette: &'a Palette,
    /// A key can be written now.
    pub can_restore: bool,
}

pub enum Action {
    None,
    OpenFolder,
    /// Put this key back as it was in the older backup.
    Restore(KeyChange),
}

/// The differences between the two chosen backups.
struct Diff {
    pair: (PathBuf, PathBuf),
    keys: Vec<(KeyChange, String)>,
    others: Vec<String>,
    /// Keymap of the newer backup, to draw.
    after: Option<Vil>,
    same_keyboard: bool,
}

#[derive(Default)]
pub struct HistoryWindow {
    pub open: bool,
    files: Vec<BackupFile>,
    /// Indexes in `files` (newest first).
    before: usize,
    after: usize,
    /// Layer drawn; `None` = the first one with changes.
    layer: Option<u8>,
    diff: Option<Diff>,
}

impl HistoryWindow {
    /// Opens the window on the two latest backups.
    pub fn open(&mut self, dir: &Path) {
        *self = HistoryWindow { open: true, ..HistoryWindow::default() };
        self.reload(dir);
    }

    fn reload(&mut self, dir: &Path) {
        self.files = backup::history(dir);
        self.after = 0;
        // the previous backup of the same keyboard, if there is one
        let uid = self.files.first().and_then(|f| f.uid);
        self.before = self.files.iter().skip(1).position(|f| f.uid == uid).map_or(1, |i| i + 1);
        self.diff = None;
        self.layer = None;
    }

    fn compute(&mut self, input: &HistoryInput) {
        let (Some(b), Some(a)) = (self.files.get(self.before), self.files.get(self.after)) else {
            self.diff = None;
            return;
        };
        let pair = (b.path.clone(), a.path.clone());
        if self.diff.as_ref().is_some_and(|d| d.pair == pair) {
            return;
        }
        let names = changes::Context { layer_names: input.layer_names, custom: input.custom };
        let keys = changes::key_changes(&b.content, &a.content)
            .into_iter()
            .map(|k| {
                let text = changes::describe_key(&k, &b.content, &a.content, &names);
                (k, text)
            })
            .collect();
        let others = changes::describe_others(&b.content, &a.content, &names);
        let after = Vil::parse(&a.content.to_string(), input.rows, input.cols).ok();
        self.diff = Some(Diff { pair, keys, others, after, same_keyboard: b.uid == a.uid });
        self.layer = None;
    }

    pub fn show(&mut self, ctx: &egui::Context, input: &HistoryInput) -> Action {
        if !self.open {
            return Action::None;
        }
        let lang = input.lang;
        let mut action = Action::None;
        let builder = ViewportBuilder::default()
            .with_title(format!("Keyboard HUD · {}", tr(lang, Msg::HistTitle)))
            .with_inner_size([640.0, 680.0]);
        ctx.show_viewport_immediate(ViewportId::from_hash_of(VIEWPORT), builder, |ui, _class| {
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                self.open = false;
            }
            egui::CentralPanel::default().show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, Msg::HistRefresh)).clicked() {
                        self.reload(input.dir);
                    }
                    if ui.button(tr(lang, Msg::HistOpenFolder)).on_hover_text(input.dir.display().to_string()).clicked()
                    {
                        action = Action::OpenFolder;
                    }
                });
                ui.separator();
                match self.files.len() {
                    0 => {
                        ui.label(tr(lang, Msg::HistNone));
                        return;
                    }
                    1 => {
                        ui.label(tr(lang, Msg::HistNeedTwo));
                        ui.label(RichText::new(changes::summary(&self.files[0].content)).weak());
                        return;
                    }
                    _ => {}
                }
                self.pickers(ui, lang);
                self.compute(input);
                let Some(diff) = &self.diff else { return };
                if !diff.same_keyboard {
                    ui.colored_label(Color32::from_rgb(0xE0, 0x9A, 0x30), tr(lang, Msg::HistOtherKeyboard));
                }
                if diff.keys.is_empty() && diff.others.is_empty() {
                    ui.add_space(8.0);
                    ui.label(tr(lang, Msg::HistSame));
                    return;
                }
                let changed_layers: Vec<u8> = {
                    let mut l: Vec<u8> = diff.keys.iter().map(|(k, _)| k.layer).collect();
                    l.dedup();
                    l
                };
                let shown = self.layer.or(changed_layers.first().copied());
                if !changed_layers.is_empty() {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for &l in &changed_layers {
                            let n = diff.keys.iter().filter(|(k, _)| k.layer == l).count();
                            let name = input.layer_names.get(l as usize).cloned().unwrap_or_else(|| format!("L{l}"));
                            if ui.selectable_label(shown == Some(l), format!("{name} ({n})")).clicked() {
                                self.layer = Some(l);
                            }
                        }
                    });
                }
                if let (Some(layer), Some(vil), Some(keys)) = (shown, &diff.after, input.keys) {
                    draw_layer(ui, input, vil, keys, layer, &diff.keys);
                }
                egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                    if !diff.keys.is_empty() {
                        ui.label(RichText::new(tr(lang, Msg::HistKeys)).strong());
                        for (k, text) in &diff.keys {
                            ui.horizontal(|ui| {
                                let r = ui
                                    .add_enabled(
                                        input.can_restore,
                                        egui::Button::new(tr(lang, Msg::HistRestore)).small(),
                                    )
                                    .on_hover_text(tr(lang, Msg::HistRestoreHint))
                                    .on_disabled_hover_text(tr(lang, Msg::HistRestoreUnavailable));
                                if r.clicked() {
                                    action = Action::Restore(*k);
                                }
                                ui.label(text);
                            });
                        }
                        ui.add_space(8.0);
                    }
                    if !diff.others.is_empty() {
                        ui.label(RichText::new(tr(lang, Msg::HistOthers)).strong());
                        for line in &diff.others {
                            ui.label(line);
                        }
                    }
                });
            });
        });
        action
    }

    /// The two backups to compare.
    fn pickers(&mut self, ui: &mut egui::Ui, lang: Lang) {
        let several = self.files.iter().any(|f| f.uid != self.files[0].uid);
        let label = |f: &BackupFile| {
            if several {
                let file = f.path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                let keyboard = file.rsplit_once('_').map_or(file, |(k, _)| k);
                format!("{} · {keyboard}", f.when)
            } else {
                f.when.clone()
            }
        };
        egui::Grid::new("history-pickers").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            for (title, index, id) in [
                (Msg::HistBefore, &mut self.before, "history-before"),
                (Msg::HistAfter, &mut self.after, "history-after"),
            ] {
                ui.label(tr(lang, title));
                let current = self.files.get(*index).map(label).unwrap_or_default();
                egui::ComboBox::from_id_salt(id).width(320.0).selected_text(current).show_ui(ui, |ui| {
                    for (i, f) in self.files.iter().enumerate() {
                        ui.selectable_value(index, i, label(f));
                    }
                });
                ui.end_row();
            }
        });
    }
}

/// The newer backup's `layer`, changed keys marked.
fn draw_layer(
    ui: &mut egui::Ui,
    input: &HistoryInput,
    vil: &Vil,
    keys: &[KleKey],
    layer: u8,
    changed: &[(KeyChange, String)],
) {
    let width = ui.available_width();
    let (response, painter) = ui.allocate_painter(Vec2::new(width, (width * 0.42).min(260.0)), Sense::hover());
    let legends = LegendContext {
        layer_names: input.layer_names,
        custom: input.custom,
        tap_dances: &vil.dynamic.tap_dances,
        macros: &vil.dynamic.macros,
    };
    let mark = |r: u8, c: u8| {
        if changed.iter().any(|(k, _)| k.layer == layer && k.row == r && k.col == c) {
            1.0
        } else {
            0.0
        }
    };
    let none = |_: u8, _: u8| 0.0;
    let no_override = |_: u16| None;
    let view = KeyboardView {
        keys,
        keymap: &vil.keymap,
        layer,
        active_mask: (1 << layer) | 1,
        highlight: &none,
        legends: &legends,
        labels: input.labels,
        layer_color: input.layer_color,
        mark: &mark,
        heat: None,
        key_override: &no_override,
        combos: None,
    };
    painter.rect_filled(response.rect, 6.0, input.palette.background);
    if let Some(tip) = keyboard::draw(&painter, response.rect.shrink(6.0), &view, input.palette, response.hover_pos()) {
        response.on_hover_text(tip);
    }
}
