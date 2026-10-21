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
        },
    }
}
