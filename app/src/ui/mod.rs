//! egui application: overlay window, header with live indicators, keyboard
//! view, context menu.

pub mod header;
pub mod keyboard;
pub mod theme;

use crate::config::{Config, ConfigWatcher, Reload, ThemeChoice, ViewMode, WindowState};
use crate::hid::protocol::{caps, HudInfo, HudState};
use crate::hid::worker::{Event, PauseReason, Status, Worker};
use crate::i18n::{tr, Lang, Msg};
use crate::kle::KleKey;
use crate::logging;
use crate::model::{classify_layers, highest_layer, Keymap};
use crate::vial::definition::Definition;
use crate::vial::legend::LegendContext;
use egui::{Align2, Color32, FontId, Key, Rect, Sense, Vec2, ViewportCommand};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use theme::{layer_table, Palette};

/// Window size at scale 1.0, per view mode.
pub fn base_size(mode: ViewMode) -> Vec2 {
    match mode {
        ViewMode::Full => Vec2::new(900.0, 400.0),
        ViewMode::Compact => Vec2::new(620.0, 230.0),
    }
}

/// Press animation: instant on, fade out over this time after release.
const FADE: Duration = Duration::from_millis(200);
/// Two HUD_TOG taps within this time are a double tap.
const DOUBLE_TAP: Duration = Duration::from_millis(300);
const NOTICE_TIME: Duration = Duration::from_secs(4);
const SAVE_DELAY: Duration = Duration::from_secs(1);

/// DejaVu Sans as a fallback font: it has the compact key symbols
/// (⌫ ⏎ ⇥ ⇧ ⇪ ⌦ ␣ ⇄ ⟳) missing from egui's default fonts.
pub fn install_fonts(ctx: &egui::Context) {
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
    ctx.add_font(FontInsert::new(
        "dejavu-sans",
        egui::FontData::from_static(include_bytes!("../../data/fonts/DejaVuSans.ttf")),
        vec![InsertFontFamily { family: egui::FontFamily::Proportional, priority: FontPriority::Lowest }],
    ));
}

/// What is shown on screen: geometry + keymap, live or loaded from disk.
pub struct Layout {
    pub definition: Definition,
    pub keys: Vec<KleKey>,
    pub keymap: Keymap,
}

impl Layout {
    pub fn new(definition: Definition, layout_options: u32, keymap: Keymap) -> Self {
        let keys = definition.visible_keys(layout_options);
        Layout { definition, keys, keymap }
    }
}

pub enum Mode {
    /// Talking to a keyboard (real or simulated).
    Live { worker: Worker, mock: bool },
    /// Files from disk only.
    Offline,
}

/// Everything the app needs at startup besides the keyboard.
pub struct Startup {
    pub mode: Mode,
    pub offline: Option<Layout>,
    pub start_layer: u8,
    pub screenshot: Option<Screenshot>,
    pub config: Config,
    pub watcher: ConfigWatcher,
    pub config_error: Option<String>,
    pub window: WindowState,
    pub window_path: PathBuf,
    /// `--verbose` wins over the configured log level.
    pub verbose: bool,
}

/// Development aid: capture the window to a PPM file, then quit.
pub struct Screenshot {
    pub path: PathBuf,
    pub at: Instant,
    requested: bool,
}

impl Screenshot {
    pub fn new(path: PathBuf, delay: Duration) -> Self {
        Screenshot { path, at: Instant::now() + delay, requested: false }
    }
}

fn write_ppm(path: &std::path::Path, image: &egui::ColorImage) -> std::io::Result<()> {
    let [w, h] = image.size;
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in &image.pixels {
        // composited on black: the window itself is transparent
        let [r, g, b, _] = p.to_array();
        out.extend_from_slice(&[r, g, b]);
    }
    std::fs::write(path, out)
}

/// Press highlight per matrix position.
#[derive(Default)]
struct PressFade {
    matrix: [u8; 8],
    released: [[Option<Instant>; 8]; 8],
}

impl PressFade {
    fn update(&mut self, matrix: &[u8; 8], now: Instant) {
        for ((old, new), times) in self.matrix.iter().zip(matrix).zip(self.released.iter_mut()) {
            let released = old & !new;
            for (c, t) in times.iter_mut().enumerate() {
                if released & (1 << c) != 0 {
                    *t = Some(now);
                }
            }
        }
        self.matrix = *matrix;
    }

    fn clear(&mut self) {
        *self = PressFade::default();
    }

    fn level(&self, row: u8, col: u8, now: Instant) -> f32 {
        let (r, c) = (row as usize, col as usize);
        if r >= 8 || c >= 8 {
            return 0.0;
        }
        if self.matrix[r] & (1 << c) != 0 {
            return 1.0;
        }
        match self.released[r][c] {
            Some(t) => 1.0 - (now.duration_since(t).as_secs_f32() / FADE.as_secs_f32()).min(1.0),
            None => 0.0,
        }
    }

    fn animating(&self, now: Instant) -> bool {
        self.released.iter().flatten().flatten().any(|t| now.duration_since(*t) < FADE)
    }
}

/// HUD_TOG: one tap toggles the HUD, a double tap toggles click-through.
#[derive(Default)]
struct HudToggle {
    count: Option<u8>,
    pending: Option<Instant>,
}

enum TapAction {
    None,
    Single,
    Double,
}

impl HudToggle {
    fn on_count(&mut self, count: u8, now: Instant) -> TapAction {
        let Some(old) = self.count.replace(count) else { return TapAction::None };
        match count.wrapping_sub(old) {
            0 => TapAction::None,
            1 if self.pending.is_some_and(|t| now.duration_since(t) <= DOUBLE_TAP) => {
                self.pending = None;
                TapAction::Double
            }
            1 => {
                self.pending = Some(now);
                TapAction::None
            }
            _ => {
                // two taps between polls
                self.pending = None;
                TapAction::Double
            }
        }
    }

    /// A single tap is confirmed once the double-tap window has passed.
    fn poll(&mut self, now: Instant) -> TapAction {
        match self.pending {
            Some(t) if now.duration_since(t) > DOUBLE_TAP => {
                self.pending = None;
                TapAction::Single
            }
            _ => TapAction::None,
        }
    }

    fn reset(&mut self) {
        *self = HudToggle::default();
    }
}

pub struct HudApp {
    lang: Lang,
    mode: Mode,
    status: Option<Status>,
    live: Option<Layout>,
    info: Option<HudInfo>,
    offline: Option<Layout>,
    state: Option<HudState>,
    browse_layer: u8,
    layer_names: Vec<String>,
    layer_colors: Vec<Option<Color32>>,

    config: Config,
    watcher: ConfigWatcher,
    verbose: bool,
    window: WindowState,
    window_path: PathBuf,
    window_dirty: Option<Instant>,

    opacity: f32,
    scale: f32,
    view: ViewMode,
    theme: ThemeChoice,
    dark: Option<bool>,
    click_through: bool,
    visible: bool,

    fade: PressFade,
    hud_toggle: HudToggle,
    notice: Option<(String, Instant)>,
    screenshot: Option<Screenshot>,
    started: Instant,
}

impl HudApp {
    pub fn new(ctx: &egui::Context, s: Startup) -> Self {
        let a = &s.config.appearance;
        let (layer_names, layer_colors) = layer_table(&s.config.layers);
        let mut app = HudApp {
            lang: Lang::It,
            mode: s.mode,
            status: None,
            live: None,
            info: None,
            offline: s.offline,
            state: None,
            browse_layer: s.start_layer,
            layer_names,
            layer_colors,
            opacity: s.window.opacity.unwrap_or(a.opacity),
            scale: s.window.scale.unwrap_or(a.scale),
            view: s.window.mode.unwrap_or(a.mode),
            theme: s.window.theme.unwrap_or(a.theme),
            config: s.config,
            watcher: s.watcher,
            verbose: s.verbose,
            window: s.window,
            window_path: s.window_path,
            window_dirty: None,
            dark: None,
            click_through: false,
            visible: true,
            fade: PressFade::default(),
            hud_toggle: HudToggle::default(),
            notice: None,
            screenshot: s.screenshot,
            started: Instant::now(),
        };
        if let Some(e) = s.config_error {
            app.notify(format!("{}: {e}", tr(app.lang, Msg::ConfigError)));
        }
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        app.apply_poll_rate();
        app
    }

    fn notify(&mut self, text: String) {
        log::info!("{text}");
        self.notice = Some((text, Instant::now()));
    }

    fn worker(&self) -> Option<&Worker> {
        match &self.mode {
            Mode::Live { worker, .. } => Some(worker),
            Mode::Offline => None,
        }
    }

    fn is_mock(&self) -> bool {
        matches!(self.mode, Mode::Live { mock: true, .. })
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        let Mode::Live { worker, .. } = &self.mode else { return };
        let events: Vec<Event> = worker.events.try_iter().collect();
        let now = Instant::now();
        for event in events {
            match event {
                Event::Status(s) => {
                    match &s {
                        Status::Connected { .. } | Status::NoHudFirmware { .. } => {}
                        // keep the last layout on screen while paused, but no stale state
                        Status::Paused(_) => self.forget_state(),
                        _ => {
                            self.live = None;
                            self.info = None;
                            self.forget_state();
                        }
                    }
                    self.status = Some(s);
                }
                Event::Layout { definition, layout_options, keymap, info } => {
                    self.live = Some(Layout::new(definition, layout_options, keymap));
                    self.info = info;
                }
                Event::Keymap(km) => {
                    if let Some(l) = &mut self.live {
                        l.keymap = km;
                    }
                }
                Event::State(s) => {
                    self.fade.update(&s.matrix, now);
                    if let TapAction::Double = self.hud_toggle.on_count(s.hud_toggle_count, now) {
                        self.set_click_through(ctx, !self.click_through);
                    }
                    self.state = Some(s);
                }
            }
        }
        // without a way to turn it off again, click-through must not stay on
        if self.click_through && !self.click_through_available() {
            self.set_click_through(ctx, false);
        }
    }

    fn forget_state(&mut self) {
        self.state = None;
        self.fade.clear();
        self.hud_toggle.reset();
    }

    fn click_through_available(&self) -> bool {
        matches!(self.status, Some(Status::Connected { .. }))
            && self.info.is_some_and(|i| i.capabilities & caps::HUD_TOG != 0)
    }

    fn set_click_through(&mut self, ctx: &egui::Context, on: bool) {
        if on == self.click_through || (on && !self.click_through_available()) {
            return;
        }
        self.click_through = on;
        ctx.send_viewport_cmd(ViewportCommand::MousePassthrough(on));
        self.notify(tr(self.lang, if on { Msg::ClickThroughOn } else { Msg::ClickThroughOff }).to_string());
    }

    fn set_visible(&mut self, ctx: &egui::Context, visible: bool) {
        if visible == self.visible {
            return;
        }
        self.visible = visible;
        ctx.send_viewport_cmd(ViewportCommand::Visible(visible));
        self.apply_poll_rate();
        log::info!("{}", if visible { "HUD visibile" } else { tr(self.lang, Msg::HudHidden) });
    }

    fn apply_poll_rate(&self) {
        if let Some(w) = self.worker() {
            let p = &self.config.polling;
            w.set_poll_hz(if self.visible { p.visible_hz } else { p.hidden_hz });
            w.set_timeout_ms(p.timeout_ms);
        }
    }

    fn apply_config(&mut self, ctx: &egui::Context, config: Config) {
        if config.device != self.config.device {
            log::warn!("le impostazioni [device] valgono dal prossimo avvio");
        }
        let (names, colors) = layer_table(&config.layers);
        self.layer_names = names;
        self.layer_colors = colors;
        // appearance values edited in the file win over the remembered ones
        let (old, new) = (&self.config.appearance, &config.appearance);
        if new.opacity != old.opacity {
            self.opacity = new.opacity;
        }
        if new.theme != old.theme {
            self.theme = new.theme;
        }
        let resize = new.scale != old.scale || new.mode != old.mode;
        if resize {
            self.scale = new.scale;
            self.view = new.mode;
        }
        if !self.verbose {
            logging::set_level(logging::parse_level(&config.log.level));
        }
        self.config = config;
        self.apply_poll_rate();
        if resize {
            self.resize(ctx);
        }
        self.notify(tr(self.lang, Msg::ConfigReloaded).to_string());
    }

    fn resize(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(base_size(self.view) * self.scale));
        self.mark_dirty();
    }

    fn mark_dirty(&mut self) {
        self.window_dirty = Some(Instant::now());
    }

    fn save_window(&mut self) {
        self.window.opacity = Some(self.opacity);
        self.window.scale = Some(self.scale);
        self.window.mode = Some(self.view);
        self.window.theme = Some(self.theme);
        self.window.save(&self.window_path);
        self.window_dirty = None;
    }

    fn track_position(&mut self, ctx: &egui::Context) {
        if let Some(r) = ctx.input(|i| i.viewport().outer_rect) {
            let moved = |old: Option<f32>, new: f32| old.is_none_or(|o| (o - new).abs() > 0.5);
            if moved(self.window.x, r.min.x) || moved(self.window.y, r.min.y) {
                self.window.x = Some(r.min.x);
                self.window.y = Some(r.min.y);
                self.mark_dirty();
            }
        }
        if self.window_dirty.is_some_and(|t| t.elapsed() >= SAVE_DELAY) {
            self.save_window();
        }
    }

    fn apply_theme(&mut self, ctx: &egui::Context) -> bool {
        let dark = match self.theme {
            ThemeChoice::Dark => true,
            ThemeChoice::Light => false,
            ThemeChoice::System => ctx.system_theme().is_none_or(|t| t == egui::Theme::Dark),
        };
        if self.dark != Some(dark) {
            ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
            self.dark = Some(dark);
        }
        dark
    }

    fn layer_name(&self, layer: u8) -> String {
        self.layer_names.get(layer as usize).cloned().unwrap_or_else(|| format!("L{layer}"))
    }

    fn layer_color(&self, layer: u8) -> Option<Color32> {
        self.layer_colors.get(layer as usize).copied().flatten()
    }

    fn status_line(&self) -> String {
        let t = |m| tr(self.lang, m);
        match (&self.mode, &self.status) {
            (Mode::Offline, _) => t(Msg::Offline).to_string(),
            (_, Some(Status::Paused(PauseReason::Vial))) => t(Msg::PausedVial).to_string(),
            (_, Some(Status::Paused(PauseReason::Manual))) => t(Msg::PausedManual).to_string(),
            (Mode::Live { mock: true, .. }, _) => t(Msg::Mock).to_string(),
            (_, None | Some(Status::Searching)) => t(Msg::Searching).to_string(),
            (_, Some(Status::Connected { product })) => format!("{} · {product}", t(Msg::Connected)),
            (_, Some(Status::NoHudFirmware { product })) => format!("{product}: {}", t(Msg::NoHudFirmware)),
            (_, Some(Status::PermissionDenied(path))) => {
                format!("{} ({path}). {}", t(Msg::PermissionDenied), t(Msg::UdevHint))
            }
            (_, Some(Status::Error(e))) => format!("{}: {e}", t(Msg::Error)),
        }
    }

    fn handle_browse_input(&mut self, ctx: &egui::Context, layers: u8) {
        let max = layers.saturating_sub(1);
        ctx.input(|i| {
            for n in 0..=9u8 {
                if Key::from_name(&n.to_string()).is_some_and(|k| i.key_pressed(k)) && n <= max {
                    self.browse_layer = n;
                }
            }
            if i.modifiers.command {
                return; // Ctrl + wheel zooms
            }
            // one layer per wheel event, whatever its size
            let scroll: f32 = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::MouseWheel { delta, .. } => Some(delta.y.signum()),
                    _ => None,
                })
                .sum();
            if scroll < 0.0 && self.browse_layer < max {
                self.browse_layer += 1;
            } else if scroll > 0.0 && self.browse_layer > 0 {
                self.browse_layer -= 1;
            }
        });
    }

    fn handle_zoom(&mut self, ctx: &egui::Context) {
        let zoom = ctx.input(|i| i.zoom_delta());
        if (zoom - 1.0).abs() > f32::EPSILON {
            let scale = (self.scale * zoom).clamp(0.5, 2.0);
            if (scale - self.scale).abs() > 0.001 {
                self.scale = scale;
                self.resize(ctx);
            }
        }
    }

    fn context_menu(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let ctx = ui.ctx().clone();
        ui.set_min_width(220.0);
        ui.horizontal(|ui| {
            ui.label(tr(lang, Msg::MenuOpacity));
            let mut pct = (self.opacity * 100.0).round();
            if ui.add(egui::Slider::new(&mut pct, 20.0..=100.0).suffix("%")).changed() {
                self.opacity = pct / 100.0;
                self.mark_dirty();
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr(lang, Msg::MenuScale));
            let mut pct = (self.scale * 100.0).round();
            let r = ui.add(egui::Slider::new(&mut pct, 50.0..=200.0).suffix("%"));
            if r.changed() {
                self.scale = pct / 100.0;
            }
            // resizing while dragging would move the slider under the pointer
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                self.resize(&ctx);
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr(lang, Msg::MenuView));
            let before = self.view;
            ui.selectable_value(&mut self.view, ViewMode::Full, tr(lang, Msg::ViewFull));
            ui.selectable_value(&mut self.view, ViewMode::Compact, tr(lang, Msg::ViewCompact));
            if self.view != before {
                self.resize(&ctx);
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr(lang, Msg::MenuTheme));
            let before = self.theme;
            ui.selectable_value(&mut self.theme, ThemeChoice::System, tr(lang, Msg::ThemeSystem));
            ui.selectable_value(&mut self.theme, ThemeChoice::Dark, tr(lang, Msg::ThemeDark));
            ui.selectable_value(&mut self.theme, ThemeChoice::Light, tr(lang, Msg::ThemeLight));
            if self.theme != before {
                self.mark_dirty();
            }
        });
        ui.separator();
        let available = self.click_through_available();
        let mut ct = self.click_through;
        let hint = tr(lang, if available { Msg::ClickThroughHint } else { Msg::ClickThroughUnavailable });
        let r = ui
            .add_enabled(available, egui::Checkbox::new(&mut ct, tr(lang, Msg::MenuClickThrough)))
            .on_hover_text(hint)
            .on_disabled_hover_text(hint);
        if r.changed() {
            self.set_click_through(&ctx, ct);
            ui.close();
        }
        if let Some(w) = self.worker() {
            let mut paused = w.is_paused();
            if ui.checkbox(&mut paused, tr(lang, Msg::MenuPause)).changed() {
                w.set_paused(paused);
            }
        }
        ui.separator();
        if ui.button(tr(lang, Msg::MenuQuit)).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

impl eframe::App for HudApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events(ctx);
        let now = Instant::now();
        if let TapAction::Single = self.hud_toggle.poll(now) {
            self.set_visible(ctx, !self.visible);
        }
        match self.watcher.poll() {
            Reload::Unchanged => {}
            Reload::Changed(c) => self.apply_config(ctx, *c),
            Reload::Error(e) => {
                let path = self.watcher.path().display().to_string();
                self.notify(format!("{} ({path}): {e}", tr(self.lang, Msg::ConfigError)));
            }
        }
        self.track_position(ctx);

        if let Some(shot) = &mut self.screenshot {
            if !shot.requested && now >= shot.at {
                shot.requested = true;
                let (path, ctx2) = (shot.path.clone(), ctx.clone());
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |image| {
                    if let Err(e) = write_ppm(&path, &image) {
                        log::error!("{}: {e}", path.display());
                    }
                    ctx2.send_viewport_cmd(ViewportCommand::Close);
                    ctx2.request_repaint();
                })));
            }
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        // a pending single tap must be confirmed even if no new state arrives
        if self.hud_toggle.pending.is_some() {
            ctx.request_repaint_after(DOUBLE_TAP);
        }
        // keep checking the config file
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_window();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = Instant::now();
        let dark = self.apply_theme(&ctx);
        let palette = if dark { Palette::dark(self.opacity) } else { Palette::light(self.opacity) };
        let compact = self.view == ViewMode::Compact;

        let full = ui.max_rect();
        let response = ui.interact(full, ui.scope_id().with("background"), Sense::click_and_drag());
        if response.drag_started() && !self.click_through {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if !self.click_through {
            self.handle_zoom(&ctx);
        }
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 12.0, palette.background);

        // live data wins; otherwise the files loaded from disk
        let live_state = self.live.as_ref().and(self.state);
        let layout = self.live.as_ref().or(self.offline.as_ref());

        let (layer, active_mask, browsing) = match (&live_state, layout) {
            (Some(s), _) => {
                let mask = s.active_mask();
                (highest_layer(mask), mask, false)
            }
            (None, Some(l)) => {
                let layer = self.browse_layer.min(l.keymap.layers().saturating_sub(1));
                (layer, (1 << layer) | 1, self.live.is_none())
            }
            (None, None) => (0, 1, false),
        };
        let layers = match (&live_state, layout) {
            (Some(s), Some(l)) => classify_layers(&l.keymap, s),
            _ => Vec::new(),
        };

        let pad = if compact { 10.0 } else { 16.0 };
        let header_h = if compact { 22.0 } else { 28.0 };
        let footer_h = if compact { 14.0 } else { 18.0 };
        let header_rect =
            Rect::from_min_size(full.min + Vec2::new(pad, 8.0), Vec2::new(full.width() - 2.0 * pad, header_h));
        let names = |l: u8| self.layer_name(l);
        let colors = |l: u8| self.layer_color(l);
        let animating_header = header::draw(
            &painter,
            header_rect,
            &header::HeaderView {
                lang: self.lang,
                layer,
                layers: &layers,
                layer_name: &names,
                layer_color: &colors,
                state: live_state.as_ref(),
                compact,
                time: self.started.elapsed().as_secs_f64(),
            },
            &palette,
        );

        let body = Rect::from_min_max(
            full.min + Vec2::new(pad - 2.0, 8.0 + header_h + 6.0),
            full.max - Vec2::new(pad - 2.0, footer_h + 4.0),
        );
        let mut browse_layers = None;
        match layout {
            Some(l) => {
                let legends = LegendContext { layer_names: &self.layer_names, custom: &l.definition.custom };
                let fade = &self.fade;
                let highlight = |r: u8, c: u8| if live_state.is_some() { fade.level(r, c, now) } else { 0.0 };
                let view = keyboard::KeyboardView {
                    keys: &l.keys,
                    keymap: &l.keymap,
                    layer,
                    active_mask,
                    highlight: &highlight,
                    legends: &legends,
                    layer_color: &colors,
                };
                let hover = if self.click_through { None } else { response.hover_pos() };
                if let Some(tip) = keyboard::draw(&painter, body, &view, &palette, hover) {
                    response.clone().on_hover_text(tip);
                }
                browse_layers = browsing.then(|| l.keymap.layers());
            }
            None => {
                painter.text(
                    body.center(),
                    Align2::CENTER_CENTER,
                    tr(self.lang, Msg::NoLayout),
                    FontId::proportional(15.0),
                    palette.text_dim,
                );
            }
        }

        // footer: notice, otherwise connection status
        let notice = self.notice.as_ref().filter(|(_, t)| t.elapsed() < NOTICE_TIME).map(|(s, _)| s.clone());
        let show_status = !compact || !matches!(self.status, Some(Status::Connected { .. })) || self.is_mock();
        let mut footer = notice.clone().unwrap_or_else(|| if show_status { self.status_line() } else { String::new() });
        if notice.is_none() && browsing && layout.is_some() {
            footer = format!("{footer} · {}", tr(self.lang, Msg::BrowseHint));
        }
        let color = if notice.is_some() { palette.accent } else { palette.text_dim };
        painter.text(
            full.left_bottom() + Vec2::new(pad, -(footer_h / 2.0 + 4.0)),
            Align2::LEFT_CENTER,
            footer,
            FontId::proportional(if compact { 10.5 } else { 12.0 }),
            color,
        );

        if let Some(n) = browse_layers {
            self.handle_browse_input(&ctx, n);
        }
        if !self.click_through {
            response.context_menu(|ui| self.context_menu(ui));
        }

        if self.fade.animating(now) || animating_header {
            ctx.request_repaint();
        } else if notice.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_fade() {
        let t0 = Instant::now();
        let mut f = PressFade::default();
        let mut m = [0u8; 8];
        m[1] = 0b100;
        f.update(&m, t0);
        assert_eq!(f.level(1, 2, t0), 1.0);
        f.update(&[0; 8], t0);
        assert!((f.level(1, 2, t0 + Duration::from_millis(100)) - 0.5).abs() < 0.01);
        assert_eq!(f.level(1, 2, t0 + Duration::from_millis(250)), 0.0);
        assert!(f.animating(t0 + Duration::from_millis(50)));
        assert!(!f.animating(t0 + Duration::from_millis(300)));
        assert_eq!(f.level(9, 9, t0), 0.0);
    }

    #[test]
    fn hud_toggle_taps() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let mut h = HudToggle::default();
        assert!(matches!(h.on_count(7, t0), TapAction::None)); // baseline
                                                               // single tap, confirmed after the double-tap window
        assert!(matches!(h.on_count(8, ms(0)), TapAction::None));
        assert!(matches!(h.poll(ms(100)), TapAction::None));
        assert!(matches!(h.poll(ms(400)), TapAction::Single));
        // double tap
        assert!(matches!(h.on_count(9, ms(1000)), TapAction::None));
        assert!(matches!(h.on_count(10, ms(1200)), TapAction::Double));
        assert!(matches!(h.poll(ms(1600)), TapAction::None));
        // two taps between polls, across the u8 wrap
        h.reset();
        h.on_count(254, ms(2000));
        assert!(matches!(h.on_count(0, ms(2016)), TapAction::Double));
    }
}
