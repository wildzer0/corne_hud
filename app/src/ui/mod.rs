//! egui application: header with layer and connection status, keyboard view.

pub mod keyboard;
pub mod theme;

use crate::hid::protocol::{HudInfo, HudState};
use crate::hid::worker::{Event, Status, Worker};
use crate::i18n::{tr, Lang, Msg};
use crate::kle::KleKey;
use crate::model::{highest_layer, Keymap};
use crate::vial::definition::Definition;
use crate::vial::legend::LegendContext;
use egui::{Align2, Color32, FontId, Key, Rect, Sense, Vec2, ViewportCommand};
use theme::{Palette, DEFAULT_LAYERS};

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

pub enum Mode {
    /// Talking to a keyboard (real or simulated).
    Live { worker: Worker, mock: bool },
    /// Files from disk only.
    Offline,
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
    opacity: f32,
    screenshot: Option<Screenshot>,
}

/// Development aid: capture the window to a PPM file, then quit.
pub struct Screenshot {
    pub path: std::path::PathBuf,
    pub at: std::time::Instant,
    requested: bool,
}

impl Screenshot {
    pub fn new(path: std::path::PathBuf, delay: std::time::Duration) -> Self {
        Screenshot { path, at: std::time::Instant::now() + delay, requested: false }
    }
}

fn write_ppm(path: &std::path::Path, image: &egui::ColorImage) -> std::io::Result<()> {
    let [w, h] = image.size;
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in &image.pixels {
        // composite on black: the window itself is transparent
        let [r, g, b, _] = p.to_array();
        out.extend_from_slice(&[r, g, b]);
    }
    std::fs::write(path, out)
}

impl HudApp {
    pub fn new(mode: Mode, offline: Option<Layout>, start_layer: u8, screenshot: Option<Screenshot>) -> Self {
        HudApp {
            lang: Lang::It,
            mode,
            status: None,
            live: None,
            info: None,
            offline,
            state: None,
            browse_layer: start_layer,
            layer_names: DEFAULT_LAYERS.iter().map(|(n, _)| n.to_string()).collect(),
            layer_colors: DEFAULT_LAYERS.iter().map(|(_, c)| *c).collect(),
            opacity: 0.85,
            screenshot,
        }
    }

    fn drain_events(&mut self) {
        let Mode::Live { worker, .. } = &self.mode else { return };
        while let Ok(event) = worker.events.try_recv() {
            match event {
                Event::Status(s) => {
                    if !matches!(s, Status::Connected { .. } | Status::NoHudFirmware { .. }) {
                        self.live = None;
                        self.state = None;
                        self.info = None;
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
                Event::State(s) => self.state = Some(s),
            }
        }
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
                let key = Key::from_name(&n.to_string());
                if key.is_some_and(|k| i.key_pressed(k)) && n <= max {
                    self.browse_layer = n;
                }
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
}

impl eframe::App for HudApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if let Some(shot) = &mut self.screenshot {
            if !shot.requested && std::time::Instant::now() >= shot.at {
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
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let palette = Palette::dark(self.opacity);
        let full = ui.max_rect();
        let response = ui.interact(full, ui.scope_id().with("background"), Sense::click_and_drag());
        if response.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 12.0, palette.background);

        // live data wins; otherwise the files loaded from disk
        let live_state = self.live.is_some().then_some(self.state.as_ref()).flatten().copied();
        let layout = self.live.as_ref().or(self.offline.as_ref());

        let (layer, active_mask, browsing) = match (&live_state, layout) {
            (Some(s), _) => {
                let mask = s.active_mask();
                (highest_layer(mask), mask, false)
            }
            (None, Some(l)) => {
                let layer = self.browse_layer.min(l.keymap.layers().saturating_sub(1));
                (layer, (1 << layer) | 1, true)
            }
            (None, None) => (0, 1, false),
        };

        // header: layer name in its color, status on the right
        let header = Rect::from_min_size(full.min + Vec2::new(16.0, 10.0), Vec2::new(full.width() - 32.0, 26.0));
        let name = self.layer_name(layer);
        let color = self.layer_color(layer).unwrap_or(palette.neutral_layer);
        let badge = painter.text(header.left_center(), Align2::LEFT_CENTER, &name, FontId::proportional(20.0), color);
        let mut sub = self.status_line();
        if browsing && layout.is_some() {
            sub = format!("{sub} · {}", tr(self.lang, Msg::BrowseHint));
        }
        painter.text(
            badge.right_center() + Vec2::new(14.0, 1.0),
            Align2::LEFT_CENTER,
            sub,
            FontId::proportional(13.0),
            palette.text_dim,
        );

        let body = Rect::from_min_max(full.min + Vec2::new(14.0, 44.0), full.max - Vec2::new(14.0, 12.0));
        let mut browse_layers = None;
        match layout {
            Some(l) => {
                let legends = LegendContext { layer_names: &self.layer_names, custom: &l.definition.custom };
                let colors = |n: u8| self.layer_color(n);
                let view = keyboard::KeyboardView {
                    keys: &l.keys,
                    keymap: &l.keymap,
                    layer,
                    active_mask,
                    state: live_state.as_ref(),
                    legends: &legends,
                    layer_color: &colors,
                };
                let hover = response.hover_pos();
                if let Some(tip) = keyboard::draw(&painter, body, &view, &palette, hover) {
                    response.on_hover_text(tip);
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

        if let Some(layers) = browse_layers {
            self.handle_browse_input(&ctx, layers);
        }

        if matches!(self.mode, Mode::Live { .. }) {
            // events arrive from the HID thread, which also wakes the UI
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}
