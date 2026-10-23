//! Settings window (a separate OS window), in the spirit of VS Code: every
//! setting of `keyboard-hud.toml` with its description, a search box, and the
//! file itself one click away. The file stays the source of truth: each
//! change rewrites only its key (`config_edit`), comments included.

use crate::config::{self, lookup, Config};
use crate::config_edit::Edit;
use crate::i18n::{tr, Lang, Msg};
use crate::style::{self, Look};
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
            s("headsup.locked_layer", Msg::SetLockedLayer, Msg::SetLockedLayerHint, Kind::Choice(SHOWN_IN)),
            s("headsup.oneshot", Msg::SetOneshot, Msg::SetOneshotHint, Kind::Choice(SHOWN_IN)),
            s("headsup.modifiers", Msg::SetModifiers, Msg::SetModifiersHint, Kind::Choice(SHOWN_IN)),
        ],
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
    Group { title: Msg::GroupLayers, settings: &[] },
];

/// Does the setting match the search (label, description or key)?
fn matches(setting: &Setting, lang: Lang, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty()
        || setting.key.contains(&q)
        || tr(lang, setting.label).to_lowercase().contains(&q)
        || tr(lang, setting.hint).to_lowercase().contains(&q)
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

pub enum Action {
    None,
    Edit(Edit),
    OpenFile,
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
}

#[derive(Default)]
pub struct SettingsWindow {
    pub open: bool,
    query: String,
    group: usize,
    focus_search: bool,
    /// Value being dragged or typed, saved on release.
    draft: Option<(&'static str, toml::Value)>,
    /// Text fields being edited.
    texts: HashMap<&'static str, String>,
    /// Text fields holding an invalid value.
    invalid: Option<&'static str>,
    defaults: Option<toml::Value>,
}

impl SettingsWindow {
    pub fn show_window(&mut self) {
        self.open = true;
        self.focus_search = true;
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
            .with_inner_size([720.0, 600.0])
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
            egui::Panel::top("settings-top").show(ui, |ui| {
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
                        action = Action::OpenFile;
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(input.path.display().to_string()).monospace().weak().small());
                    match input.error {
                        None => ui.label(RichText::new(format!("✔ {}", tr(lang, Msg::SettingsValid))).weak().small()),
                        Some(e) => ui
                            .label(
                                RichText::new(format!("⚠ {}", tr(lang, Msg::SettingsInvalid)))
                                    .small()
                                    .color(ui.visuals().warn_fg_color),
                            )
                            .on_hover_text(e),
                    };
                });
                if let Some(e) = input.error {
                    ui.label(RichText::new(e).monospace().small().color(ui.visuals().error_fg_color));
                }
                ui.add_space(4.0);
            });
            let searching = !self.query.trim().is_empty();
            egui::Panel::left("settings-groups").resizable(false).exact_size(170.0).show(ui, |ui| {
                ui.add_space(6.0);
                for (i, g) in GROUPS.iter().enumerate() {
                    let hits = GROUPS[i].settings.iter().filter(|s| matches(s, lang, &self.query)).count();
                    let enabled = !searching || hits > 0;
                    let selected = !searching && self.group == i;
                    let r = ui.add_enabled(enabled, egui::Button::selectable(selected, tr(lang, g.title)));
                    if r.clicked() {
                        self.group = i;
                        self.query.clear();
                    }
                }
            });
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.add_enabled_ui(input.error.is_none(), |ui| {
                        if searching {
                            let mut any = false;
                            for g in GROUPS {
                                let hits: Vec<&Setting> =
                                    g.settings.iter().filter(|s| matches(s, lang, &self.query)).collect();
                                if hits.is_empty() {
                                    continue;
                                }
                                any = true;
                                ui.heading(tr(lang, g.title));
                                for s in hits {
                                    self.row(ui, s, input, &current, &defaults, &mut action);
                                }
                                ui.add_space(8.0);
                            }
                            if !any {
                                ui.label(RichText::new(tr(lang, Msg::SettingsNoMatch)).weak());
                            }
                        } else {
                            let g = &GROUPS[self.group.min(GROUPS.len() - 1)];
                            ui.heading(tr(lang, g.title));
                            ui.add_space(4.0);
                            if g.settings.is_empty() {
                                self.text_only(ui, input, &mut action);
                            }
                            for s in g.settings {
                                self.row(ui, s, input, &current, &defaults, &mut action);
                            }
                        }
                    });
                });
            });
        });
        action
    }

    /// Layers and labels: a summary, and the way to the text.
    fn text_only(&self, ui: &mut egui::Ui, input: &SettingsInput, action: &mut Action) {
        let lang = input.lang;
        ui.label(tr(lang, Msg::SettingsTextOnly));
        ui.add_space(6.0);
        ui.label(format!("{}: {}", tr(lang, Msg::SettingsLayers), input.config.layers.len()));
        for (i, l) in input.config.layers.iter().enumerate() {
            ui.horizontal(|ui| {
                let color = config::parse_color(&l.color)
                    .map_or(ui.visuals().weak_text_color(), |(r, g, b)| egui::Color32::from_rgb(r, g, b));
                ui.label(RichText::new("■").color(color));
                ui.label(format!("{i} · {}", l.name));
            });
        }
        ui.label(format!("{}: {}", tr(lang, Msg::SettingsLabels), input.config.labels.len()));
        ui.add_space(6.0);
        if ui.button(format!("{{ }}  {}", tr(lang, Msg::SettingsOpenText))).clicked() {
            *action = Action::OpenFile;
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
                        *action = Action::Edit((setting.key, default.cloned()));
                        self.texts.remove(setting.key);
                    }
                });
            }
        });
        ui.label(RichText::new(tr(lang, setting.hint)).weak());
        if let Some(edit) = self.control(ui, setting, input, cur) {
            *action = Action::Edit(edit);
        }
    }

    /// The value being dragged, otherwise the one in the file.
    fn shown<'a>(&'a self, key: &str, cur: Option<&'a toml::Value>) -> Option<&'a toml::Value> {
        match &self.draft {
            Some((k, v)) if *k == key => Some(v),
            _ => cur,
        }
    }

    /// Sliders and number fields: saved when let go, not at every step.
    fn track(&mut self, key: &'static str, r: &egui::Response, value: toml::Value) -> Option<Edit> {
        if r.changed() {
            self.draft = Some((key, value));
        }
        let done = r.drag_stopped() || (r.changed() && !r.dragged());
        match self.draft.take() {
            Some((k, v)) if k == key && done => Some((key, Some(v))),
            other => {
                self.draft = other;
                None
            }
        }
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
        match &setting.kind {
            Kind::Toggle => {
                let mut on = cur.and_then(toml::Value::as_bool).unwrap_or(false);
                ui.checkbox(&mut on, "").changed().then_some((key, Some(toml::Value::Boolean(on))))
            }
            Kind::Percent(range) => {
                let mut pct = (as_f32(self.shown(key, cur)).unwrap_or(*range.start()) * 100.0).round();
                let r = ui.add(
                    egui::Slider::new(&mut pct, range.start() * 100.0..=range.end() * 100.0)
                        .suffix("%")
                        .fixed_decimals(0),
                );
                self.track(key, &r, float(pct / 100.0))
            }
            Kind::StylePercent(range, from_style) => {
                let set = as_f32(self.shown(key, cur));
                let mut edit = None;
                ui.horizontal(|ui| {
                    let mut by_style = set.is_none();
                    if ui.checkbox(&mut by_style, tr(lang, Msg::SettingsFromStyle)).changed() {
                        let v = from_style(&input.style_look);
                        edit = Some((key, (!by_style).then(|| float(v))));
                    }
                    let mut pct = (set.unwrap_or_else(|| from_style(&input.style_look)) * 100.0).round();
                    let r = ui.add_enabled(
                        set.is_some(),
                        egui::Slider::new(&mut pct, range.start() * 100.0..=range.end() * 100.0)
                            .suffix("%")
                            .fixed_decimals(0),
                    );
                    if edit.is_none() {
                        edit = self.track(key, &r, float(pct / 100.0));
                    }
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
                self.track(key, &r, float(v))
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
                self.track(key, &r, toml::Value::Integer(v))
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
                            edit = Some((key, Some(text(c))));
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
                            edit = Some((key, Some(text(name))));
                        }
                    }
                });
                edit
            }
            Kind::Text | Kind::HexId => {
                let hex = matches!(setting.kind, Kind::HexId);
                let value = cur.and_then(toml::Value::as_str).unwrap_or("").to_string();
                let invalid = self.invalid == Some(key);
                let buf = self.texts.entry(key).or_insert_with(|| value.clone());
                let r = ui.add(egui::TextEdit::singleline(buf).desired_width(320.0).hint_text(if hex {
                    "0x4653"
                } else {
                    ""
                }));
                let mut edit = None;
                if r.lost_focus() {
                    let t = buf.trim().to_string();
                    if hex && !t.is_empty() && config::parse_hex_id(&t).is_none() {
                        self.invalid = Some(key);
                    } else {
                        if invalid {
                            self.invalid = None;
                        }
                        if t != value {
                            edit = Some((key, if hex && t.is_empty() { None } else { Some(text(&t)) }));
                        }
                    }
                } else if !r.has_focus() && !invalid {
                    // follow the file while not being edited
                    *buf = value;
                }
                if self.invalid == Some(key) {
                    ui.label(RichText::new(tr(lang, Msg::SettingsInvalidId)).color(ui.visuals().error_fg_color));
                }
                edit
            }
        }
    }
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
    }
}
