//! Icon in the system tray (notification area, menu bar) with a small menu.
//! On Linux it goes through D-Bus (StatusNotifierItem, `ksni`), without GTK:
//! KDE and most panels show it, GNOME needs the AppIndicator extension.

use crate::i18n::{tr, Lang, Msg};
use std::sync::{Mutex, Once};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ToggleHud,
    TogglePause,
    ToggleAutostart,
    Backups,
    Stats,
    Settings,
    Quit,
}

enum Raw {
    Menu(MenuId),
    LeftClick,
}

/// Events arrive on other threads; the handlers can be set only once per
/// process, so they feed this queue and wake the UI.
static QUEUE: Mutex<Vec<Raw>> = Mutex::new(Vec::new());
static WAKE: Mutex<Option<egui::Context>> = Mutex::new(None);
static HANDLERS: Once = Once::new();

fn push(raw: Raw) {
    if let Ok(mut q) = QUEUE.lock() {
        q.push(raw);
    }
    if let Some(ctx) = WAKE.lock().ok().and_then(|c| c.clone()) {
        ctx.request_repaint();
    }
}

pub struct Tray {
    _icon: TrayIcon,
    show: MenuItem,
    pause: CheckMenuItem,
    autostart: CheckMenuItem,
    backups: MenuItem,
    stats: MenuItem,
    settings: MenuItem,
    quit: MenuItem,
    /// Last state shown, to touch the menu only on changes.
    shown: Option<(bool, bool, bool)>,
    lang: Lang,
}

impl Tray {
    pub fn new(ctx: &egui::Context, lang: Lang) -> Result<Self, String> {
        if let Ok(mut w) = WAKE.lock() {
            *w = Some(ctx.clone());
        }
        HANDLERS.call_once(|| {
            MenuEvent::set_event_handler(Some(|e: MenuEvent| push(Raw::Menu(e.id))));
            TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                    push(Raw::LeftClick);
                }
            }));
        });
        let t = |m| tr(lang, m);
        let title = MenuItem::new(format!("Keyboard HUD {}", env!("CARGO_PKG_VERSION")), false, None);
        let show = MenuItem::new(t(Msg::TrayHide), true, None);
        let pause = CheckMenuItem::new(t(Msg::MenuPause), true, false, None);
        let autostart = CheckMenuItem::new(t(Msg::MenuAutostart), true, false, None);
        let backups = MenuItem::new(t(Msg::MenuBackups), true, None);
        let stats = MenuItem::new(t(Msg::MenuStats), true, None);
        let settings = MenuItem::new(format!("{}…", t(Msg::SettingsTitle)), true, None);
        let quit = MenuItem::new(t(Msg::MenuQuit), true, None);
        let menu = Menu::new();
        menu.append_items(&[
            &title,
            &PredefinedMenuItem::separator(),
            &show,
            &pause,
            &PredefinedMenuItem::separator(),
            &backups,
            &stats,
            &settings,
            &autostart,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|e| e.to_string())?;
        let icon = Icon::from_rgba(include_bytes!("../packaging/icon/icon-64.rgba").to_vec(), 64, 64)
            .map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("Keyboard HUD")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Tray { _icon: tray, show, pause, autostart, backups, stats, settings, quit, shown: None, lang })
    }

    /// Actions requested since the last call.
    pub fn poll(&self) -> Vec<Action> {
        let raw = QUEUE.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
        raw.into_iter()
            .filter_map(|r| match r {
                Raw::LeftClick => Some(Action::ToggleHud),
                Raw::Menu(id) => [
                    (self.show.id(), Action::ToggleHud),
                    (self.pause.id(), Action::TogglePause),
                    (self.autostart.id(), Action::ToggleAutostart),
                    (self.backups.id(), Action::Backups),
                    (self.stats.id(), Action::Stats),
                    (self.settings.id(), Action::Settings),
                    (self.quit.id(), Action::Quit),
                ]
                .into_iter()
                .find(|(i, _)| **i == id)
                .map(|(_, a)| a),
            })
            .collect()
    }

    /// Mirrors the app state in the menu.
    pub fn sync(&mut self, visible: bool, paused: bool, autostart: bool) {
        let now = (visible, paused, autostart);
        if self.shown == Some(now) {
            return;
        }
        self.shown = Some(now);
        self.show.set_text(tr(self.lang, if visible { Msg::TrayHide } else { Msg::TrayShow }));
        self.pause.set_checked(paused);
        self.autostart.set_checked(autostart);
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        if let Ok(mut w) = WAKE.lock() {
            *w = None;
        }
    }
}
