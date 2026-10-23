//! Settings window (a separate OS window), in the spirit of VS Code: every
//! setting of `keyboard-hud.toml` with its description, a search box, layers
//! and labels, and the file itself as text. The file stays the source of
//! truth: each change rewrites only what it touches (`config_edit`),
//! comments included.

use super::theme::DEFAULT_LAYERS;
use crate::config::{self, lookup, Config, LabelConfig};
use crate::config_edit::Edit;
use crate::i18n::{tr, Lang, Msg};
use crate::style::{self, Look};
use crate::vial::names::parse_keycode;
use egui::{RichText, ViewportBuilder, ViewportId};
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::path::Path;

/// Viewport of the settings window.
const VIEWPORT: &str = "hud-settings";

/// A choice: the value in the file and its label (`None` = the value itself).
type Choices = &'static [(&'static str, Option<Msg>)];

const THEMES: Choices =
    &[("system", Some(Msg::ThemeSystem)), ("dark", Some(Msg::ThemeDark)), ("light", Some(Msg::ThemeLight))];
const LANGUAGES: Choices =
    &[("auto", Some(Msg::LanguageAuto)), ("it", Some(Msg::LangItalian)), ("en", Some(Msg::LangEnglish))];
const MODES: Choices = &[("full", Some(Msg::ViewFull)), ("compact", Some(Msg::ViewCompact))];
const SHOWN_IN: Choices = &[("hud", Some(Msg::ShownHud)), ("indicator", Some(Msg::ShownIndicator))];
const PLACES: Choices = &[
    ("top-left", Some(Msg::PlaceTopLeft)),
    ("top", Some(Msg::PlaceTop)),
    ("top-right", Some(Msg::PlaceTopRight)),
    ("left", Some(Msg::PlaceLeft)),
    ("center", Some(Msg::PlaceCenter)),
    ("right", Some(Msg::PlaceRight)),
    ("bottom-left", Some(Msg::PlaceBottomLeft)),
    ("bottom", Some(Msg::PlaceBottom)),
    ("bottom-right", Some(Msg::PlaceBottomRight)),
];
const LOG_LEVELS: Choices = &[("error", None), ("warn", None), ("info", None), ("debug", None)];

pub enum Kind {
    Toggle,
    /// A fraction, shown as a percentage.
    Percent(RangeInclusive<f32>),
    /// Optional percentage: unset = the style's value.
    StylePercent(RangeInclusive<f32>, fn(&Look) -> f32),
    Float(RangeInclusive<f32>, &'static str),
    Int(RangeInclusive<i64>, &'static str),
    Choice(Choices),
    /// Name of a style in `styles/`; empty = the default one.
    Style,
    Text,
    /// Optional hexadecimal USB ID.
    HexId,
}

pub struct Setting {
    pub key: &'static str,
    pub label: Msg,
    pub hint: Msg,
    pub kind: Kind,
    /// Read only at startup.
    pub restart: bool,
}

const fn s(key: &'static str, label: Msg, hint: Msg, kind: Kind) -> Setting {
    Setting { key, label, hint, kind, restart: false }
}

const fn range_i64(r: RangeInclusive<u64>) -> RangeInclusive<i64> {
    *r.start() as i64..=*r.end() as i64
}

pub struct Group {
    pub title: Msg,
    pub settings: &'static [Setting],
}

/// Every setting the window offers, in the order of the example file.
/// Layers and labels (lists of tables) are edited in the text for now.
pub static GROUPS: &[Group] = &[
    Group {
        title: Msg::GroupAppearance,
        settings: &[
            s("appearance.opacity", Msg::MenuOpacity, Msg::SetOpacityHint, Kind::Percent(config::OPACITY)),
            s("appearance.scale", Msg::MenuScale, Msg::SetScaleHint, Kind::Percent(config::SCALE)),
            s("appearance.theme", Msg::MenuTheme, Msg::SetThemeHint, Kind::Choice(THEMES)),
            s("appearance.language", Msg::MenuLanguage, Msg::SetLanguageHint, Kind::Choice(LANGUAGES)),
            s("appearance.mode", Msg::MenuView, Msg::SetModeHint, Kind::Choice(MODES)),
            s("appearance.style", Msg::MenuStyle, Msg::SetStyleHint, Kind::Style),
            s(
                "appearance.key_opacity",
                Msg::MenuKeyOpacity,
                Msg::SetKeyOpacityHint,
                Kind::StylePercent(style::KEY_OPACITY, |l| l.key_opacity),
            ),
            s(
                "appearance.key_radius",
                Msg::MenuKeyRadius,
                Msg::SetKeyRadiusHint,
                Kind::StylePercent(style::KEY_RADIUS, |l| l.key_radius),
            ),
            s("appearance.combos", Msg::MenuCombos, Msg::SetCombosHint, Kind::Toggle),
            s("appearance.hide_in_fullscreen", Msg::SetFullscreen, Msg::SetFullscreenHint, Kind::Toggle),
            s("appearance.tray", Msg::SetTray, Msg::SetTrayHint, Kind::Toggle),
            s("appearance.snap_distance", Msg::SetSnap, Msg::SetSnapHint, Kind::Float(config::SCREEN_DISTANCE, " px")),
            s(
                "appearance.screen_margin",
                Msg::SetMargin,
                Msg::SetMarginHint,
                Kind::Float(config::SCREEN_DISTANCE, " px"),
            ),
        ],
    },
    Group {
        title: Msg::GroupHeadsUp,
        settings: &[
            s("headsup.enabled", Msg::SetEnabled, Msg::SetHeadsUpHint, Kind::Toggle),
            s(
                "headsup.layer_delay_ms",
                Msg::SetLayerDelay,
                Msg::SetLayerDelayHint,
                Kind::Int(range_i64(config::MODIFIER_DELAY_MS), " ms"),
            ),
            s(
                "headsup.modifier_delay_ms",
                Msg::SetModifierDelay,
                Msg::SetModifierDelayHint,
                Kind::Int(range_i64(config::MODIFIER_DELAY_MS), " ms"),
            ),
            s(
                "headsup.hide_delay_ms",
                Msg::SetHideDelay,
                Msg::SetHideDelayHint,
                Kind::Int(range_i64(config::HIDE_DELAY_MS), " ms"),
            ),
            s(
                "headsup.hidden_opacity",
                Msg::SetHiddenOpacity,
                Msg::SetHiddenOpacityHint,
                Kind::Percent(config::HIDDEN_OPACITY),
            ),
            s("headsup.marker", Msg::SetMarker, Msg::SetMarkerHint, Kind::Toggle),
            s(
                "headsup.marker_opacity",
                Msg::SetMarkerOpacity,
                Msg::SetMarkerOpacityHint,
                Kind::Percent(config::MARKER_OPACITY),
            ),
            s("headsup.locked_layer", Msg::SetLockedLayer, Msg::SetLockedLayerHint, Kind::Choice(SHOWN_IN)),
            s("headsup.oneshot", Msg::SetOneshot, Msg::SetOneshotHint, Kind::Choice(SHOWN_IN)),
            s("headsup.modifiers", Msg::SetModifiers, Msg::SetModifiersHint, Kind::Choice(SHOWN_IN)),
        ],
    },
    Group {
        title: Msg::GroupUpdate,
        settings: &[s("update.auto_check", Msg::UpdateAuto, Msg::UpdateAutoHint, Kind::Toggle)],
    },
    Group {
        title: Msg::GroupIndicator,
        settings: &[
            s("indicator.enabled", Msg::SetEnabled, Msg::SetIndicatorHint, Kind::Toggle),
            s("indicator.position", Msg::SetPosition, Msg::SetPositionHint, Kind::Choice(PLACES)),
        ],
    },
    Group {
        title: Msg::GroupKeycast,
        settings: &[
            s("keycast.enabled", Msg::SetEnabled, Msg::KeycastHint, Kind::Toggle),
            s("keycast.position", Msg::SetPosition, Msg::SetPositionHint, Kind::Choice(PLACES)),
        ],
    },
    Group {
        title: Msg::GroupStats,
        settings: &[
            s("stats.enabled", Msg::SetEnabled, Msg::SetStatsHint, Kind::Toggle),
            s("stats.file", Msg::SetStatsFile, Msg::SetStatsFileHint, Kind::Text),
            s(
                "stats.unused_days",
                Msg::SetUnusedDays,
                Msg::SetUnusedDaysHint,
                Kind::Int(*config::UNUSED_DAYS.start() as i64..=*config::UNUSED_DAYS.end() as i64, ""),
            ),
        ],
    },
    Group {
        title: Msg::GroupBackup,
        settings: &[
            s("backup.enabled", Msg::SetEnabled, Msg::SetBackupHint, Kind::Toggle),
            s("backup.dir", Msg::SetBackupDir, Msg::SetBackupDirHint, Kind::Text),
            s(
                "backup.keep",
                Msg::SetBackupKeep,
                Msg::SetBackupKeepHint,
                Kind::Int(*config::BACKUP_KEEP.start() as i64..=*config::BACKUP_KEEP.end() as i64, ""),
            ),
        ],
    },
    Group {
        title: Msg::GroupPolling,
        settings: &[
            s(
                "polling.visible_hz",
                Msg::SetVisibleHz,
                Msg::SetVisibleHzHint,
                Kind::Int(*config::POLL_HZ.start() as i64..=*config::POLL_HZ.end() as i64, " Hz"),
            ),
            s(
                "polling.hidden_hz",
                Msg::SetHiddenHz,
                Msg::SetHiddenHzHint,
                Kind::Int(*config::POLL_HZ.start() as i64..=*config::POLL_HZ.end() as i64, " Hz"),
            ),
            s(
                "polling.timeout_ms",
                Msg::SetTimeout,
                Msg::SetTimeoutHint,
                Kind::Int(range_i64(config::TIMEOUT_MS), " ms"),
            ),
        ],
    },
    Group {
        title: Msg::GroupDevice,
        settings: &[
            Setting { key: "device.vid", label: Msg::SetVid, hint: Msg::SetVidHint, kind: Kind::HexId, restart: true },
            Setting { key: "device.pid", label: Msg::SetPid, hint: Msg::SetPidHint, kind: Kind::HexId, restart: true },
        ],
    },
    Group {
        title: Msg::GroupLog,
        settings: &[s("log.level", Msg::SetLogLevel, Msg::SetLogLevelHint, Kind::Choice(LOG_LEVELS))],
    },
];

/// Does the setting match the search (label, description or key)?
fn matches(setting: &Setting, lang: Lang, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty()
        || setting.key.contains(&q)
        || tr(lang, setting.label).to_lowercase().contains(&q)
        || tr(lang, setting.hint).to_lowercase().contains(&q)
}

/// Does the label match the search (text, description, keycode, apps)?
fn label_matches(l: &LabelConfig, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    let apps = l.app.as_ref().map(|a| a.names().join(" ")).unwrap_or_default();
    [l.label.as_str(), l.description.as_deref().unwrap_or(""), l.keycode.as_deref().unwrap_or(""), &apps]
        .iter()
        .any(|t| t.to_lowercase().contains(&q))
}

fn as_f32(v: Option<&toml::Value>) -> Option<f32> {
    match v? {
        toml::Value::Float(f) => Some(*f as f32),
        toml::Value::Integer(i) => Some(*i as f32),
        _ => None,
    }
}

fn float(v: f32) -> toml::Value {
    toml::Value::Float(v as f64)
}

fn text(s: &str) -> toml::Value {
    toml::Value::String(s.to_string())
}

fn hex_color(c: egui::Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// `code, idea64` → one name or a list; empty → no app.
fn apps_value(t: &str) -> Option<toml::Value> {
    let names: Vec<&str> = t.split(',').map(str::trim).filter(|n| !n.is_empty()).collect();
    match names.as_slice() {
        [] => None,
        [one] => Some(text(one)),
        many => Some(toml::Value::Array(many.iter().map(|n| text(n)).collect())),
    }
}

/// What a label is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Keycode,
    Position,
}

fn target(l: &LabelConfig) -> Target {
    if l.keycode.is_some() || (l.row.is_none() && l.col.is_none()) {
        Target::Keycode
    } else {
        Target::Position
    }
}

/// What is wrong with a label, if anything: the same rules as `Labels::from_config`.
fn label_problem(l: &LabelConfig) -> Option<Msg> {
    if l.label.trim().is_empty() {
        return Some(Msg::LabelEmpty);
    }
    match (l.row, l.col, &l.keycode) {
        (Some(_), Some(_), None) => None,
        (None, None, Some(k)) if parse_keycode(k).is_some() => None,
        (None, None, _) => Some(Msg::LabelBadKeycode),
        _ => Some(Msg::LabelBadTarget),
    }
}

pub enum Action {
    None,
    Edits(Vec<Edit>),
    OpenFile,
    /// The whole text of the file, edited in the window.
    SaveText(String),
}

/// What the window shows besides the settings themselves.
pub struct SettingsInput<'a> {
    pub lang: Lang,
    pub config: &'a Config,
    pub path: &'a Path,
    /// The file does not parse: read-only until it is fixed.
    pub error: Option<&'a str>,
    pub styles: &'a [String],
    /// Key shape of the style in use, for the values left to the style.
    pub style_look: Look,
    /// Layer names in use (config, then defaults).
    pub layer_names: &'a [String],
    /// Grows whenever the file is read or written: the text page reloads it.
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    /// One of `GROUPS`.
    Group(usize),
    Layers,
    Labels,
    Text,
}

impl Default for Page {
    fn default() -> Self {
        Page::Group(0)
    }
}

/// The file as text, edited in the window.
#[derive(Default)]
struct TextPage {
    content: String,
    /// As last read from disk, with `\n` only (the text editor's way).
    original: String,
    /// The file on disk has `\r\n` line endings: saved the same way.
    crlf: bool,
    revision: Option<u64>,
    /// Parse error of the content being edited.
    problem: Option<String>,
}

impl TextPage {
    fn dirty(&self) -> bool {
        self.content != self.original
    }

    /// Reads the file again unless there are changes not saved.
    fn sync(&mut self, path: &Path, revision: u64) {
        if self.revision == Some(revision) || self.dirty() {
            return;
        }
        self.revision = Some(revision);
        let text = std::fs::read_to_string(path).unwrap_or_default();
        self.crlf = text.contains("\r\n");
        self.original = text.replace("\r\n", "\n");
        self.content = self.original.clone();
        self.check();
    }

    fn check(&mut self) {
        self.problem = Config::parse(&self.content).err();
    }
}

#[derive(Default)]
pub struct SettingsWindow {
    pub open: bool,
    query: String,
    page: Page,
    focus_search: bool,
    /// A change being dragged (slider, number, color), saved when the mouse is released.
    draft: Option<(String, Vec<Edit>)>,
    /// Text fields being edited.
    texts: HashMap<String, String>,
    /// Text field holding an invalid value.
    invalid: Option<String>,
    defaults: Option<toml::Value>,
    text: TextPage,
}

impl SettingsWindow {
    pub fn show_window(&mut self) {
        self.open = true;
        self.focus_search = true;
    }

    /// `--settings-page`: layers, labels or text.
    pub fn open_page(&mut self, name: Option<&str>) {
        self.page = match name {
            Some("layers") => Page::Layers,
            Some("labels") => Page::Labels,
            Some("text") => Page::Text,
            _ => Page::Group(0),
        };
    }

    pub fn show(&mut self, ctx: &egui::Context, input: &SettingsInput) -> Action {
        if !self.open {
            return Action::None;
        }
        let lang = input.lang;
        let mut action = Action::None;
        let current = input.config.to_value();
        let defaults = self.defaults.get_or_insert_with(|| Config::default().to_value()).clone();
        let builder = ViewportBuilder::default()
            .with_title(format!("Keyboard HUD · {}", tr(lang, Msg::SettingsTitle)))
            .with_inner_size([760.0, 640.0])
            .with_min_inner_size([480.0, 320.0]);
        ctx.show_viewport_immediate(ViewportId::from_hash_of(VIEWPORT), builder, |ui, _class| {
            let vctx = ui.ctx().clone();
            let (close, esc) = vctx.input(|i| (i.viewport().close_requested(), i.key_pressed(egui::Key::Escape)));
            if close {
                self.open = false;
            }
            if vctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Comma)) {
                action = Action::OpenFile;
            }
            egui::Panel::top("settings-top").show(ui, |ui| self.top_bar(ui, input, esc, &mut action));
            let searching = !self.query.trim().is_empty();
            egui::Panel::left("settings-pages").resizable(false).exact_size(170.0).show(ui, |ui| {
                ui.add_space(6.0);
                // while searching, the groups without results are greyed out
                let hits: Vec<bool> =
                    GROUPS.iter().map(|g| g.settings.iter().any(|s| matches(s, lang, &self.query))).collect();
                let mut page = |ui: &mut egui::Ui, p: Page, title: Msg, hits: bool| {
                    let selected = !searching && self.page == p;
                    if ui.add_enabled(!searching || hits, egui::Button::selectable(selected, tr(lang, title))).clicked()
                    {
                        self.page = p;
                        self.query.clear();
                    }
                };
                for (i, g) in GROUPS.iter().enumerate() {
                    page(ui, Page::Group(i), g.title, hits[i]);
                }
                ui.separator();
                page(ui, Page::Layers, Msg::GroupLayers, true);
                page(ui, Page::Labels, Msg::GroupLabels, true);
                ui.separator();
                page(ui, Page::Text, Msg::GroupText, true);
            });
            egui::CentralPanel::default().show(ui, |ui| {
                if !searching && self.page == Page::Text {
                    self.text_page(ui, input, &mut action);
                    return;
                }
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.add_enabled_ui(input.error.is_none(), |ui| {
                        if searching {
                            self.search_results(ui, input, &current, &defaults, &mut action);
                        } else {
                            match self.page {
                                Page::Layers => self.layers_page(ui, input, &mut action),
                                Page::Labels => self.labels_page(ui, input, None, &mut action),
                                Page::Text => {}
                                Page::Group(i) => {
                                    let g = &GROUPS[i.min(GROUPS.len() - 1)];
                                    ui.heading(tr(lang, g.title));
                                    ui.add_space(4.0);
                                    for s in g.settings {
                                        self.row(ui, s, input, &current, &defaults, &mut action);
                                    }
                                }
                            }
                        }
                    });
                });
            });
            // a drag (slider, number, color picker) is saved once the mouse is released
            if matches!(action, Action::None) && !vctx.input(|i| i.pointer.any_down()) {
                if let Some((_, edits)) = self.draft.take() {
                    action = Action::Edits(edits);
                }
            }
        });
        action
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, input: &SettingsInput, esc: bool, action: &mut Action) {
        let lang = input.lang;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text(tr(lang, Msg::SettingsSearch))
                    .desired_width(ui.available_width() - 150.0),
            );
            if std::mem::take(&mut self.focus_search) {
                r.request_focus();
            }
            if esc && r.has_focus() {
                self.query.clear();
            }
            if ui
                .button(format!("{{ }}  {}", tr(lang, Msg::SettingsOpenText)))
                .on_hover_text(tr(lang, Msg::SettingsOpenTextHint))
                .clicked()
            {
                *action = Action::OpenFile;
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(input.path.display().to_string()).monospace().weak().small());
            match input.error {
                None => ui.label(RichText::new(format!("✔ {}", tr(lang, Msg::SettingsValid))).weak().small()),
                Some(_) => ui.label(
                    RichText::new(format!("⚠ {}", tr(lang, Msg::SettingsInvalid)))
                        .small()
                        .color(ui.visuals().warn_fg_color),
                ),
            };
        });
        if let Some(e) = input.error {
            ui.label(RichText::new(e).monospace().small().color(ui.visuals().error_fg_color));
        }
        ui.add_space(4.0);
    }

    fn search_results(
        &mut self,
        ui: &mut egui::Ui,
        input: &SettingsInput,
        current: &toml::Value,
        defaults: &toml::Value,
        action: &mut Action,
    ) {
        let lang = input.lang;
        let mut any = false;
        for g in GROUPS {
            let hits: Vec<&Setting> = g.settings.iter().filter(|s| matches(s, lang, &self.query)).collect();
            if hits.is_empty() {
                continue;
            }
            any = true;
            ui.heading(tr(lang, g.title));
            for s in hits {
                self.row(ui, s, input, current, defaults, action);
            }
            ui.add_space(8.0);
        }
        let q = self.query.clone();
        if input.config.labels.iter().any(|l| label_matches(l, &q)) {
            any = true;
            self.labels_page(ui, input, Some(&q), action);
        }
        if !any {
            ui.label(RichText::new(tr(lang, Msg::SettingsNoMatch)).weak());
        }
    }

    /// One setting: title, key, description, control, reset.
    fn row(
        &mut self,
        ui: &mut egui::Ui,
        setting: &Setting,
        input: &SettingsInput,
        current: &toml::Value,
        defaults: &toml::Value,
        action: &mut Action,
    ) {
        let lang = input.lang;
        let cur = lookup(current, setting.key);
        let default = lookup(defaults, setting.key);
        let modified = cur != default;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr(lang, setting.label)).strong());
            if modified {
                ui.label(RichText::new("●").color(ui.visuals().selection.bg_fill))
                    .on_hover_text(tr(lang, Msg::SettingsModified));
            }
            ui.label(RichText::new(setting.key).monospace().weak().small());
            if setting.restart {
                ui.label(RichText::new(tr(lang, Msg::SettingsRestart)).italics().weak().small());
            }
            if modified {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("↺").on_hover_text(tr(lang, Msg::SettingsReset)).clicked() {
                        // the default value written out keeps the comment next to the key
                        *action = Action::Edits(vec![Edit::Key(setting.key, default.cloned())]);
                        self.texts.remove(setting.key);
                    }
                });
            }
        });
        ui.label(RichText::new(tr(lang, setting.hint)).weak());
        if let Some(edit) = self.control(ui, setting, input, cur) {
            *action = Action::Edits(vec![edit]);
        }
    }

    /// The value being dragged, otherwise the one in the file.
    fn shown<'a>(&'a self, id: &str, cur: Option<&'a toml::Value>) -> Option<&'a toml::Value> {
        match &self.draft {
            Some((d, edits)) if d == id => match edits.first() {
                Some(Edit::Key(_, v) | Edit::Field { value: v, .. }) => v.as_ref(),
                _ => cur,
            },
            _ => cur,
        }
    }

    /// Sliders, numbers, colors: kept as a draft, saved when the mouse is released.
    fn track(&mut self, id: &str, r: &egui::Response, edits: Vec<Edit>) {
        if r.changed() {
            self.draft = Some((id.to_string(), edits));
        }
    }

    /// A text field saved when it loses the focus (Enter, Tab, a click
    /// elsewhere); the new text if it changed.
    fn text_field(&mut self, ui: &mut egui::Ui, id: &str, value: &str, hint: &str, width: f32) -> Option<String> {
        let editing = self.invalid.as_deref() == Some(id);
        let buf = self.texts.entry(id.to_string()).or_insert_with(|| value.to_string());
        // a fixed size: in a grid the available width is the column's, from the previous frame
        let size = egui::vec2(width, ui.spacing().interact_size.y);
        let r = ui.add_sized(size, egui::TextEdit::singleline(buf).hint_text(hint));
        if r.lost_focus() {
            let t = buf.trim().to_string();
            return (t != value || editing).then_some(t);
        }
        if !r.has_focus() && !editing {
            // follow the file while not being edited
            value.clone_into(buf);
        }
        None
    }

    fn control(
        &mut self,
        ui: &mut egui::Ui,
        setting: &Setting,
        input: &SettingsInput,
        cur: Option<&toml::Value>,
    ) -> Option<Edit> {
        let lang = input.lang;
        let key = setting.key;
        let set = |v: Option<toml::Value>| vec![Edit::Key(key, v)];
        match &setting.kind {
            Kind::Toggle => {
                let mut on = cur.and_then(toml::Value::as_bool).unwrap_or(false);
                ui.checkbox(&mut on, "").changed().then_some(Edit::Key(key, Some(toml::Value::Boolean(on))))
            }
            Kind::Percent(range) => {
                let mut pct = (as_f32(self.shown(key, cur)).unwrap_or(*range.start()) * 100.0).round();
                let r = ui.add(
                    egui::Slider::new(&mut pct, range.start() * 100.0..=range.end() * 100.0)
                        .suffix("%")
                        .fixed_decimals(0),
                );
                self.track(key, &r, set(Some(float(pct / 100.0))));
                None
            }
            Kind::StylePercent(range, from_style) => {
                let value = as_f32(self.shown(key, cur));
                let mut edit = None;
                ui.horizontal(|ui| {
                    let mut by_style = value.is_none();
                    if ui.checkbox(&mut by_style, tr(lang, Msg::SettingsFromStyle)).changed() {
                        let v = from_style(&input.style_look);
                        edit = Some(Edit::Key(key, (!by_style).then(|| float(v))));
                    }
                    let mut pct = (value.unwrap_or_else(|| from_style(&input.style_look)) * 100.0).round();
                    let r = ui.add_enabled(
                        value.is_some(),
                        egui::Slider::new(&mut pct, range.start() * 100.0..=range.end() * 100.0)
                            .suffix("%")
                            .fixed_decimals(0),
                    );
                    self.track(key, &r, set(Some(float(pct / 100.0))));
                });
                edit
            }
            Kind::Float(range, suffix) => {
                let mut v = as_f32(self.shown(key, cur)).unwrap_or(*range.start());
                let r = ui.add(
                    egui::DragValue::new(&mut v)
                        .range(range.clone())
                        .suffix(*suffix)
                        .speed(0.5)
                        .max_decimals(1)
                        .update_while_editing(false),
                );
                self.track(key, &r, set(Some(float(v))));
                None
            }
            Kind::Int(range, suffix) => {
                let mut v = self.shown(key, cur).and_then(toml::Value::as_integer).unwrap_or(*range.start());
                let speed = ((range.end() - range.start()) as f64 / 500.0).max(0.1);
                let r = ui.add(
                    egui::DragValue::new(&mut v)
                        .range(range.clone())
                        .suffix(*suffix)
                        .speed(speed)
                        .update_while_editing(false),
                );
                self.track(key, &r, set(Some(toml::Value::Integer(v))));
                None
            }
            Kind::Choice(choices) => {
                let value = cur.and_then(toml::Value::as_str).unwrap_or("");
                let label = |v: &str| {
                    choices
                        .iter()
                        .find(|(c, _)| *c == v)
                        .and_then(|(_, m)| m.map(|m| tr(lang, m).to_string()))
                        .unwrap_or_else(|| v.to_string())
                };
                let mut edit = None;
                egui::ComboBox::from_id_salt(key).selected_text(label(value)).width(220.0).show_ui(ui, |ui| {
                    for (c, _) in choices.iter() {
                        if ui.selectable_label(*c == value, label(c)).clicked() && *c != value {
                            edit = Some(Edit::Key(key, Some(text(c))));
                        }
                    }
                });
                edit
            }
            Kind::Style => {
                let value = cur.and_then(toml::Value::as_str).filter(|v| !v.is_empty()).unwrap_or(style::DEFAULT_STYLE);
                let mut edit = None;
                egui::ComboBox::from_id_salt(key).selected_text(value).width(220.0).show_ui(ui, |ui| {
                    for name in input.styles {
                        if ui.selectable_label(name == value, name).clicked() && name != value {
                            edit = Some(Edit::Key(key, Some(text(name))));
                        }
                    }
                });
                edit
            }
            Kind::Text => {
                let value = cur.and_then(toml::Value::as_str).unwrap_or("").to_string();
                self.text_field(ui, key, &value, "", 320.0).map(|t| Edit::Key(key, Some(text(&t))))
            }
            Kind::HexId => {
                let value = cur.and_then(toml::Value::as_str).unwrap_or("").to_string();
                let edit = match self.text_field(ui, key, &value, "0x4653", 160.0) {
                    Some(t) if !t.is_empty() && config::parse_hex_id(&t).is_none() => {
                        self.invalid = Some(key.to_string());
                        None
                    }
                    Some(t) => {
                        self.invalid = None;
                        Some(Edit::Key(key, (!t.is_empty()).then(|| text(&t))))
                    }
                    None => None,
                };
                if self.invalid.as_deref() == Some(key) {
                    ui.label(RichText::new(tr(lang, Msg::SettingsInvalidId)).color(ui.visuals().error_fg_color));
                }
                edit
            }
        }
    }

    /// `[[layers]]`: name and color of each layer, in firmware order.
    fn layers_page(&mut self, ui: &mut egui::Ui, input: &SettingsInput, action: &mut Action) {
        let lang = input.lang;
        ui.heading(tr(lang, Msg::GroupLayers));
        ui.label(RichText::new(tr(lang, Msg::LayersHint)).weak());
        ui.add_space(8.0);
        let layers = &input.config.layers;
        egui::Grid::new("layers").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
            for (i, l) in layers.iter().enumerate() {
                ui.label(RichText::new(i.to_string()).monospace().weak());
                let id = format!("layers.{i}.name");
                let fallback = DEFAULT_LAYERS.get(i).map_or_else(|| format!("L{i}"), |d| d.0.to_string());
                if let Some(t) = self.text_field(ui, &id, &l.name, &fallback, 180.0) {
                    *action = Action::Edits(vec![Edit::Field {
                        list: "layers",
                        index: i,
                        field: "name",
                        value: Some(text(&t)),
                    }]);
                }
                let id = format!("layers.{i}.color");
                let shown = match self.shown(&id, None) {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => l.color.clone(),
                };
                ui.horizontal(|ui| {
                    let color = config::parse_color(&shown);
                    let mut neutral = color.is_none();
                    if ui.checkbox(&mut neutral, tr(lang, Msg::LayerNeutral)).changed() {
                        let c = if neutral {
                            String::new()
                        } else {
                            let default = DEFAULT_LAYERS.get(i).and_then(|d| d.1);
                            hex_color(default.unwrap_or(egui::Color32::from_rgb(0x80, 0x80, 0x80)))
                        };
                        *action = Action::Edits(vec![Edit::Field {
                            list: "layers",
                            index: i,
                            field: "color",
                            value: Some(text(&c)),
                        }]);
                    }
                    if let Some((r, g, b)) = color {
                        let mut rgb = [r, g, b];
                        let resp = egui::widgets::color_picker::color_edit_button_srgb(ui, &mut rgb);
                        let c = hex_color(egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                        self.track(
                            &id,
                            &resp,
                            vec![Edit::Field { list: "layers", index: i, field: "color", value: Some(text(&c)) }],
                        );
                        ui.label(RichText::new(c).monospace().weak().small());
                    }
                });
                if i + 1 == layers.len() {
                    // only the last one: the others would change number
                    if ui.small_button("✕").on_hover_text(tr(lang, Msg::LayerRemove)).clicked() {
                        *action = Action::Edits(vec![Edit::Remove { list: "layers", index: i }]);
                        self.texts.retain(|k, _| !k.starts_with("layers."));
                    }
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
        });
        ui.add_space(8.0);
        if ui.button(format!("+  {}", tr(lang, Msg::LayerAdd))).clicked() {
            let i = layers.len();
            let (name, color) = DEFAULT_LAYERS.get(i).map_or_else(
                || (format!("L{i}"), String::new()),
                |d| (d.0.to_string(), d.1.map(hex_color).unwrap_or_default()),
            );
            *action = Action::Edits(vec![Edit::Push {
                list: "layers",
                fields: vec![("name", text(&name)), ("color", text(&color))],
            }]);
        }
    }

    /// `[[labels]]`, all of them or those matching `filter`.
    fn labels_page(&mut self, ui: &mut egui::Ui, input: &SettingsInput, filter: Option<&str>, action: &mut Action) {
        let lang = input.lang;
        ui.heading(tr(lang, Msg::GroupLabels));
        if filter.is_none() {
            ui.label(RichText::new(tr(lang, Msg::LabelsHint)).weak());
            ui.add_space(8.0);
        }
        let labels = &input.config.labels;
        if labels.is_empty() {
            ui.label(RichText::new(tr(lang, Msg::LabelsNone)).weak());
        }
        for (i, l) in labels.iter().enumerate() {
            if filter.is_some_and(|q| !label_matches(l, q)) {
                continue;
            }
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some(edits) = self.label_card(ui, input, i, l) {
                    *action = Action::Edits(edits);
                }
            });
            ui.add_space(6.0);
        }
        if filter.is_none() && ui.button(format!("+  {}", tr(lang, Msg::LabelAdd))).clicked() {
            *action = Action::Edits(vec![Edit::Push {
                list: "labels",
                fields: vec![("keycode", text("")), ("label", text(""))],
            }]);
        }
    }

    fn label_card(&mut self, ui: &mut egui::Ui, input: &SettingsInput, i: usize, l: &LabelConfig) -> Option<Vec<Edit>> {
        let lang = input.lang;
        let field =
            |field: &'static str, value: Option<toml::Value>| Edit::Field { list: "labels", index: i, field, value };
        let mut out = None;
        egui::Grid::new(("label", i)).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(tr(lang, Msg::LabelText));
            ui.horizontal(|ui| {
                if let Some(t) = self.text_field(ui, &format!("labels.{i}.label"), &l.label, "Step over", 200.0) {
                    out = Some(vec![field("label", Some(text(&t)))]);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").on_hover_text(tr(lang, Msg::LabelDelete)).clicked() {
                        out = Some(vec![Edit::Remove { list: "labels", index: i }]);
                        self.texts.retain(|k, _| !k.starts_with("labels."));
                    }
                });
            });
            ui.end_row();

            ui.label(tr(lang, Msg::LabelDescription));
            let desc = l.description.clone().unwrap_or_default();
            if let Some(t) = self.text_field(ui, &format!("labels.{i}.description"), &desc, "", 360.0) {
                out = Some(vec![field("description", (!t.is_empty()).then(|| text(&t)))]);
            }
            ui.end_row();

            ui.label(tr(lang, Msg::LabelTarget));
            ui.horizontal(|ui| {
                let now = target(l);
                let mut chosen = now;
                ui.radio_value(&mut chosen, Target::Keycode, tr(lang, Msg::LabelByKeycode));
                ui.radio_value(&mut chosen, Target::Position, tr(lang, Msg::LabelByPosition));
                if chosen != now {
                    out = Some(match chosen {
                        Target::Keycode => {
                            vec![field("row", None), field("col", None), field("keycode", Some(text("")))]
                        }
                        Target::Position => vec![
                            field("keycode", None),
                            field("row", Some(toml::Value::Integer(0))),
                            field("col", Some(toml::Value::Integer(0))),
                        ],
                    });
                    self.texts.remove(&format!("labels.{i}.keycode"));
                }
                ui.add_space(8.0);
                match now {
                    Target::Keycode => {
                        let code = l.keycode.clone().unwrap_or_default();
                        if let Some(t) =
                            self.text_field(ui, &format!("labels.{i}.keycode"), &code, "KC_F10, C(KC_DOT)", 200.0)
                        {
                            out = Some(vec![field("keycode", Some(text(&t)))]);
                        }
                    }
                    Target::Position => {
                        for (name, msg, v) in [("row", Msg::LabelRow, l.row), ("col", Msg::LabelCol, l.col)] {
                            ui.label(tr(lang, msg));
                            let id = format!("labels.{i}.{name}");
                            let mut n = self
                                .shown(&id, None)
                                .and_then(toml::Value::as_integer)
                                .unwrap_or(v.unwrap_or(0) as i64);
                            let r = ui
                                .add(egui::DragValue::new(&mut n).range(0..=31).speed(0.1).update_while_editing(false));
                            self.track(&id, &r, vec![field(name, Some(toml::Value::Integer(n)))]);
                        }
                    }
                }
            });
            ui.end_row();

            ui.label(tr(lang, Msg::LabelLayer));
            let name = |n: u8| {
                let layer = input.layer_names.get(n as usize).cloned().unwrap_or_else(|| format!("L{n}"));
                format!("{n} · {layer}")
            };
            let selected = l.layer.map_or_else(|| tr(lang, Msg::LabelAllLayers).to_string(), name);
            egui::ComboBox::from_id_salt(("label-layer", i)).selected_text(selected).width(200.0).show_ui(ui, |ui| {
                if ui.selectable_label(l.layer.is_none(), tr(lang, Msg::LabelAllLayers)).clicked() && l.layer.is_some()
                {
                    out = Some(vec![field("layer", None)]);
                }
                for n in 0..input.layer_names.len().max(1) as u8 {
                    if ui.selectable_label(l.layer == Some(n), name(n)).clicked() && l.layer != Some(n) {
                        out = Some(vec![field("layer", Some(toml::Value::Integer(n as i64)))]);
                    }
                }
            });
            ui.end_row();

            ui.label(tr(lang, Msg::LabelApp)).on_hover_text(tr(lang, Msg::LabelAppHint));
            let apps = l.app.as_ref().map(|a| a.names().join(", ")).unwrap_or_default();
            ui.horizontal(|ui| {
                if let Some(t) = self.text_field(ui, &format!("labels.{i}.app"), &apps, "code, idea64", 200.0) {
                    out = Some(vec![field("app", apps_value(&t))]);
                }
                ui.label(RichText::new(tr(lang, Msg::LabelAppHint)).weak().small());
            });
            ui.end_row();
        });
        if let Some(problem) = label_problem(l) {
            ui.label(RichText::new(format!("⚠ {}", tr(lang, problem))).color(ui.visuals().warn_fg_color));
        }
        out
    }

    /// The file as text: edited here, saved only when it is valid.
    fn text_page(&mut self, ui: &mut egui::Ui, input: &SettingsInput, action: &mut Action) {
        let lang = input.lang;
        self.text.sync(input.path, input.revision);
        ui.heading(tr(lang, Msg::GroupText));
        ui.label(RichText::new(tr(lang, Msg::TextHint)).weak());
        ui.add_space(4.0);
        let dirty = self.text.dirty();
        let save_key = ui.ctx().input(|i| i.modifiers.command && i.key_pressed(egui::Key::S));
        ui.horizontal(|ui| {
            let can_save = dirty && self.text.problem.is_none();
            if ui.add_enabled(can_save, egui::Button::new(tr(lang, Msg::TextSave))).clicked() || (save_key && can_save)
            {
                let text =
                    if self.text.crlf { self.text.content.replace('\n', "\r\n") } else { self.text.content.clone() };
                *action = Action::SaveText(text);
                // what is saved becomes the original: no reload over it
                self.text.original = self.text.content.clone();
            }
            if ui.add_enabled(dirty, egui::Button::new(tr(lang, Msg::TextRevert))).clicked() {
                self.text.content = self.text.original.clone();
                self.text.check();
            }
            match (&self.text.problem, dirty) {
                (Some(_), _) => ui.label(
                    RichText::new(format!("⚠ {}", tr(lang, Msg::TextInvalid))).color(ui.visuals().warn_fg_color),
                ),
                (None, true) => ui.label(RichText::new(tr(lang, Msg::TextUnsaved)).weak()),
                (None, false) => ui.label(RichText::new(format!("✔ {}", tr(lang, Msg::TextValid))).weak()),
            };
        });
        if let Some(p) = &self.text.problem {
            ui.label(RichText::new(p).monospace().small().color(ui.visuals().error_fg_color));
        }
        ui.add_space(4.0);
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
            ui.fonts_mut(|f| f.layout_job(highlight(buf.as_str(), ui.visuals())))
        };
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let r = ui.add(
                egui::TextEdit::multiline(&mut self.text.content)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(30)
                    .layouter(&mut layouter),
            );
            if r.changed() {
                self.text.check();
            }
        });
    }
}

/// Where the comment of a TOML line starts: the first `#` outside a string.
fn comment_start(line: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (Some('"'), '\\') if !escaped => {
                escaped = true;
                continue;
            }
            (Some(q), c) if c == q && !escaped => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, '#') => return Some(i),
            _ => {}
        }
        escaped = false;
    }
    None
}

/// TOML as in an editor: monospace, no wrapping, comments dimmed, tables
/// in the accent color.
fn highlight(text: &str, visuals: &egui::Visuals) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let font = egui::FontId::monospace(13.0);
    let format = |color| TextFormat { font_id: font.clone(), color, ..Default::default() };
    let (plain, weak, table) = (visuals.text_color(), visuals.weak_text_color(), visuals.selection.bg_fill);
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    for line in text.split_inclusive('\n') {
        let split = comment_start(line).unwrap_or(line.len());
        let (code, comment) = line.split_at(split);
        let color = if code.trim_start().starts_with('[') { table } else { plain };
        job.append(code, 0.0, format(color));
        job.append(comment, 0.0, format(weak));
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key offered exists in the configuration, so a change from the
    /// window is never rejected as an unknown field.
    #[test]
    fn keys_exist_in_the_config() {
        let mut full = Config::default();
        full.appearance.key_opacity = Some(0.5);
        full.appearance.key_radius = Some(0.1);
        full.device.vid = Some("0x4653".into());
        full.device.pid = Some("0x0004".into());
        let tree = full.to_value();
        for g in GROUPS {
            for s in g.settings {
                assert!(lookup(&tree, s.key).is_some(), "{}", s.key);
            }
        }
    }

    /// Choices are values the parser accepts.
    #[test]
    fn choices_parse() {
        for g in GROUPS {
            for s in g.settings {
                if let Kind::Choice(choices) = &s.kind {
                    for (c, _) in choices.iter() {
                        let (table, key) = s.key.split_once('.').unwrap();
                        let text = format!("[{table}]\n{key} = \"{c}\"\n");
                        assert!(Config::parse(&text).is_ok(), "{text}");
                    }
                }
            }
        }
    }

    #[test]
    fn search() {
        let opacity = &GROUPS[0].settings[0];
        assert!(matches(opacity, Lang::It, "opac"));
        assert!(matches(opacity, Lang::En, "OPACITY"));
        assert!(matches(opacity, Lang::It, "appearance.op"));
        assert!(matches(opacity, Lang::It, ""));
        assert!(!matches(opacity, Lang::It, "polling"));
        let l = LabelConfig { keycode: Some("KC_F10".into()), label: "Step over".into(), ..Default::default() };
        assert!(label_matches(&l, "step"));
        assert!(label_matches(&l, "kc_f10"));
        assert!(!label_matches(&l, "rename"));
    }

    #[test]
    fn comments() {
        assert_eq!(comment_start("# tutto"), Some(0));
        assert_eq!(comment_start("scale = 1.0   # 0.5"), Some(14));
        assert_eq!(comment_start("color = \"#00B9D7\"   # azzurro"), Some(20));
        assert_eq!(comment_start("label = 'a#b'"), None);
        assert_eq!(comment_start("label = \"a\\\"#\""), None);
        let job = highlight("[a]\nx = \"#1\" # c\n", &egui::Visuals::dark());
        assert_eq!(job.text, "[a]\nx = \"#1\" # c\n");
    }

    #[test]
    fn apps_and_problems() {
        assert_eq!(apps_value(" , "), None);
        assert_eq!(apps_value("code"), Some(text("code")));
        assert_eq!(apps_value("idea64, pycharm64"), Some(toml::Value::Array(vec![text("idea64"), text("pycharm64")])));
        let mut l = LabelConfig { keycode: Some("KC_F10".into()), label: "Step".into(), ..Default::default() };
        assert_eq!(label_problem(&l), None);
        assert_eq!(target(&l), Target::Keycode);
        l.keycode = Some("".into());
        assert_eq!(label_problem(&l), Some(Msg::LabelBadKeycode));
        l = LabelConfig { row: Some(1), col: Some(2), label: "X".into(), ..Default::default() };
        assert_eq!((label_problem(&l), target(&l)), (None, Target::Position));
        l.col = None;
        assert_eq!(label_problem(&l), Some(Msg::LabelBadTarget));
        l.label = " ".into();
        assert_eq!(label_problem(&l), Some(Msg::LabelEmpty));
    }

    /// What the labels page writes for a new label and a switch to a position
    /// gives a valid label once filled in.
    #[test]
    fn label_edits_give_valid_labels() {
        use crate::config_edit::apply;
        let push = Edit::Push { list: "labels", fields: vec![("keycode", text("")), ("label", text(""))] };
        let out = apply("", &[push]).unwrap();
        let field = |field, value| Edit::Field { list: "labels", index: 0, field, value };
        let out = apply(&out, &[field("label", Some(text("Debug")))]).unwrap();
        let switch = [
            field("keycode", None),
            field("row", Some(toml::Value::Integer(0))),
            field("col", Some(toml::Value::Integer(3))),
        ];
        let out = apply(&out, &switch).unwrap();
        let c = Config::parse(&out).unwrap();
        assert_eq!(label_problem(&c.labels[0]), None);
        let (labels, errors) = crate::labels::Labels::from_config(&c.labels);
        assert!(errors.is_empty() && !labels.is_empty(), "{errors:?}");
    }
}
