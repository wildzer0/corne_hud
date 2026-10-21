//! User-facing strings, centralized so that more languages can be added.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    It,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    Searching,
    Connected,
    NoHudFirmware,
    PermissionDenied,
    UdevHint,
    Offline,
    Mock,
    NoLayout,
    BrowseHint,
    Error,
    PausedVial,
    PausedManual,
    MenuOpacity,
    MenuScale,
    MenuView,
    ViewFull,
    ViewCompact,
    MenuTheme,
    ThemeSystem,
    ThemeDark,
    ThemeLight,
    MenuClickThrough,
    ClickThroughHint,
    ClickThroughUnavailable,
    ClickThroughOn,
    ClickThroughOff,
    MenuPause,
    MenuQuit,
    ConfigError,
    ConfigReloaded,
    OsAuto,
    OsForced,
    CapsLock,
    CapsWord,
    Qwerty,
    Colemak,
    HudHidden,
}

pub fn tr(lang: Lang, msg: Msg) -> &'static str {
    match lang {
        Lang::It => match msg {
            Msg::Searching => "Ricerca della tastiera…",
            Msg::Connected => "Collegata",
            Msg::NoHudFirmware => "Firmware senza protocollo HUD: keymap letto via VIA, nessuno stato live",
            Msg::PermissionDenied => "Permesso negato sul dispositivo HID",
            Msg::UdevHint => "Serve una regola udev (una tantum): avvia con --print-udev-rule",
            Msg::Offline => "Modalità offline",
            Msg::Mock => "Tastiera simulata",
            Msg::NoLayout => "Nessun layout: collega la tastiera o avvia con --offline",
            Msg::BrowseHint => "Rotellina o tasti 0–9 per sfogliare i layer",
            Msg::Error => "Errore",
            Msg::PausedVial => "In pausa: Vial è aperto",
            Msg::PausedManual => "In pausa",
            Msg::MenuOpacity => "Opacità",
            Msg::MenuScale => "Scala",
            Msg::MenuView => "Vista",
            Msg::ViewFull => "Completa",
            Msg::ViewCompact => "Compatta",
            Msg::MenuTheme => "Tema",
            Msg::ThemeSystem => "Sistema",
            Msg::ThemeDark => "Scuro",
            Msg::ThemeLight => "Chiaro",
            Msg::MenuClickThrough => "Click-through",
            Msg::ClickThroughHint => "I clic passano alle finestre sotto. Per disattivarlo: doppio tocco di HUD_TOG",
            Msg::ClickThroughUnavailable => {
                "Serve una tastiera collegata con il tasto HUD_TOG, per poterlo disattivare"
            }
            Msg::ClickThroughOn => "Click-through attivo (doppio tocco di HUD_TOG per disattivarlo)",
            Msg::ClickThroughOff => "Click-through disattivato",
            Msg::MenuPause => "Pausa",
            Msg::MenuQuit => "Esci",
            Msg::ConfigError => "Errore nella configurazione",
            Msg::ConfigReloaded => "Configurazione ricaricata",
            Msg::OsAuto => "auto",
            Msg::OsForced => "forzato",
            Msg::CapsLock => "CAPS",
            Msg::CapsWord => "Caps Word",
            Msg::Qwerty => "QWERTY",
            Msg::Colemak => "Colemak-DH",
            Msg::HudHidden => "HUD nascosto",
        },
    }
}
