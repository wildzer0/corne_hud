//! User-facing strings, centralized: every message has its Italian and
//! English text side by side, so a missing translation does not compile.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    It,
    En,
}

/// Language of the texts built far from the UI (legends, search, changelog).
static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_current(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == Lang::En as u8 {
        Lang::En
    } else {
        Lang::It
    }
}

/// `format!` with an Italian and an English template, in the current language.
#[macro_export]
macro_rules! tr_format {
    ($it:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        match $crate::i18n::current() {
            $crate::i18n::Lang::It => format!($it $(, $arg)*),
            $crate::i18n::Lang::En => format!($en $(, $arg)*),
        }
    };
}

/// `it` or `en`, in the current language: for texts built far from the UI.
pub fn pick(it: &'static str, en: &'static str) -> &'static str {
    match current() {
        Lang::It => it,
        Lang::En => en,
    }
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
    PausedForeign,
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
    MenuBackups,
    BackupSaved,
    TrayShow,
    TrayHide,
    LabelsFor,
    BackupFirst,
    BackupChanges,
    MenuAutostart,
    AutostartHint,
    AutostartStale,
    AutostartOn,
    AutostartOff,
    AutostartError,
    MenuQuit,
    MenuClose,
    ConfigError,
    ConfigReloaded,
    OsAuto,
    OsForced,
    CapsLock,
    CapsWord,
    Qwerty,
    Colemak,
    HudHidden,
    MenuHeadsUp,
    HeadsUpHint,
    MenuSearch,
    MenuPosition,
    PositionUnavailable,
    MenuHeatmap,
    MenuCombos,
    CombosHint,
    NoCombos,
    HeatOff,
    HeatLayer,
    HeatAll,
    MenuStats,
    SearchHint,
    SearchNoResults,
    StatsTitle,
    StatsDisabled,
    StatsPeriod,
    StatsAllTime,
    StatsPrivacy,
    StatsLayers,
    StatsHands,
    StatsFingers,
    StatsUnused,
    StatsRows,
    StatsTop,
    StatsDaily,
    StatsEffort,
    StatsSuggestions,
    StatsNoSuggestions,
    StatsSuggestionHint,
    StatsFreeSpot,
    Presses,
    StatsExport,
    StatsExported,
    StatsReset,
    StatsResetConfirm,
    StatsResetDone,
    Cancel,
    Days,
    Left,
    Right,
    LabelErrors,
    Pinned,
    Unpinned,
    BadgeNoKeyboard,
    BadgePermission,
    BadgeNoHud,
    BadgePausedVial,
    BadgePausedForeign,
    BadgeError,
    NumLock,
    SplitLost,
    MenuStyle,
    MenuKeyOpacity,
    MenuKeyRadius,
    MenuSaveStyle,
    SaveStyleHint,
    MenuStylesFolder,
    StyleSaved,
    MenuDiagnostics,
    DiagTitle,
    DiagKeyboard,
    DiagStatus,
    DiagDefinition,
    DiagFirmware,
    DiagMatrix,
    DiagLink,
    DiagPolls,
    DiagLatency,
    DiagErrors,
    DiagConnections,
    DiagConnectedFor,
    DiagLive,
    DiagKeymapVersion,
    DiagOs,
    DiagSplit,
    DiagSplitOk,
    DiagSplitUnknown,
    DiagFiles,
    DiagConfig,
    DiagLog,
    DiagStyles,
    DiagStats,
    DiagBackups,
    DiagLastBackup,
    DiagApp,
    DiagVersion,
    DiagSystem,
    DiagCopy,
    DiagCopied,
    DiagOpenFolder,
    DiagNone,
    DiagOff,
    DiagAgo,
    DiagLast,
    MenuKeycast,
    KeycastHint,
    MenuLanguage,
    LanguageAuto,
}

pub fn tr(lang: Lang, msg: Msg) -> &'static str {
    let (it, en) = match msg {
        Msg::Searching => ("Ricerca della tastiera…", "Looking for the keyboard…"),
        Msg::Connected => ("Collegata", "Connected"),
        Msg::NoHudFirmware => ("Firmware senza protocollo HUD: keymap letto via VIA, nessuno stato live", "Firmware without the HUD protocol: keymap read via VIA, no live state"),
        Msg::PermissionDenied => ("Permesso negato sul dispositivo HID", "Permission denied on the HID device"),
        Msg::UdevHint => ("Serve una regola udev (una tantum): avvia con --print-udev-rule", "A udev rule is needed (once): run with --print-udev-rule"),
        Msg::Offline => ("Modalità offline", "Offline mode"),
        Msg::Mock => ("Tastiera simulata", "Simulated keyboard"),
        Msg::NoLayout => ("Nessun layout: collega la tastiera o avvia con --offline", "No layout: connect the keyboard or run with --offline"),
        Msg::BrowseHint => ("Rotellina o tasti 0–9 per sfogliare i layer", "Wheel or keys 0–9 to browse the layers"),
        Msg::Error => ("Errore", "Error"),
        Msg::PausedVial => ("In pausa: Vial è aperto", "Paused: Vial is open"),
        Msg::PausedManual => ("In pausa", "Paused"),
        Msg::PausedForeign => ("In pausa: un altro programma usa la tastiera (Vial nel browser?)", "Paused: another program is using the keyboard (Vial in a browser?)"),
        Msg::MenuOpacity => ("Opacità", "Opacity"),
        Msg::MenuScale => ("Scala", "Scale"),
        Msg::MenuView => ("Vista", "View"),
        Msg::ViewFull => ("Completa", "Full"),
        Msg::ViewCompact => ("Compatta", "Compact"),
        Msg::MenuTheme => ("Tema", "Theme"),
        Msg::ThemeSystem => ("Sistema", "System"),
        Msg::ThemeDark => ("Scuro", "Dark"),
        Msg::ThemeLight => ("Chiaro", "Light"),
        Msg::MenuClickThrough => ("Click-through", "Click-through"),
        Msg::ClickThroughHint => ("I clic passano alle finestre sotto. Per disattivarlo: doppio tocco di HUD_TOG", "Clicks go to the windows below. To turn it off: double tap HUD_TOG"),
        Msg::ClickThroughUnavailable => ("Serve una tastiera collegata con il tasto HUD_TOG, per poterlo disattivare", "Needs a connected keyboard with the HUD_TOG key, to turn it off again"),
        Msg::ClickThroughOn => ("Click-through attivo (doppio tocco di HUD_TOG per disattivarlo)", "Click-through on (double tap HUD_TOG to turn it off)"),
        Msg::ClickThroughOff => ("Click-through disattivato", "Click-through off"),
        Msg::MenuPause => ("Pausa", "Pause"),
        Msg::MenuBackups => ("Backup della tastiera…", "Keyboard backups…"),
        Msg::BackupSaved => ("Backup della configurazione salvato", "Configuration backup saved"),
        Msg::TrayShow => ("Mostra l'HUD", "Show the HUD"),
        Msg::TrayHide => ("Nascondi l'HUD", "Hide the HUD"),
        Msg::LabelsFor => ("etichette per", "labels for"),
        Msg::BackupFirst => ("primo backup di questa tastiera", "first backup of this keyboard"),
        Msg::BackupChanges => ("modifiche (dettagli in CHANGELOG.md)", "changes (details in CHANGELOG.md)"),
        Msg::MenuAutostart => ("Avvia con il sistema", "Start with the system"),
        Msg::AutostartHint => ("Avvia l'HUD all'accesso, solo per questo utente", "Starts the HUD at login, for this user only"),
        Msg::AutostartStale => ("Registrato per un altro percorso o con altre opzioni: spunta per aggiornarlo a questo eseguibile", "Registered for another path or with other options: tick to update it to this executable"),
        Msg::AutostartOn => ("Avvio con il sistema attivato", "Start with the system on"),
        Msg::AutostartOff => ("Avvio con il sistema disattivato", "Start with the system off"),
        Msg::AutostartError => ("Avvio con il sistema non modificabile", "Start with the system cannot be changed"),
        Msg::MenuQuit => ("Esci", "Quit"),
        Msg::MenuClose => ("Chiudi il menu (anche Esc o un clic sull'HUD)", "Close the menu (also Esc or a click on the HUD)"),
        Msg::ConfigError => ("Errore nella configurazione", "Error in the configuration"),
        Msg::ConfigReloaded => ("Configurazione ricaricata", "Configuration reloaded"),
        Msg::OsAuto => ("auto", "auto"),
        Msg::OsForced => ("forzato", "forced"),
        Msg::CapsLock => ("CAPS", "CAPS"),
        Msg::CapsWord => ("Caps Word", "Caps Word"),
        Msg::Qwerty => ("QWERTY", "QWERTY"),
        Msg::Colemak => ("Colemak-DH", "Colemak-DH"),
        Msg::HudHidden => ("HUD nascosto", "HUD hidden"),
        Msg::MenuHeadsUp => ("Heads-up (compare da solo)", "Heads-up (shows up by itself)"),
        Msg::HeadsUpHint => ("L'HUD compare con un layer diverso da BASE, un one-shot o un modificatore tenuto", "The HUD shows up with a layer other than BASE, a one-shot or a held modifier"),
        Msg::MenuSearch => ("Cerca… (Ctrl+F)", "Search… (Ctrl+F)"),
        Msg::MenuPosition => ("Posizione", "Position"),
        Msg::PositionUnavailable => ("Il sistema non permette all'app di spostare la finestra", "The system does not let the app move the window"),
        Msg::MenuHeatmap => ("Heatmap", "Heatmap"),
        Msg::MenuCombos => ("Mostra le combo", "Show combos"),
        Msg::CombosHint => ("Le combo configurate in Vial, sopra i tasti da premere insieme", "The combos configured in Vial, over the keys to press together"),
        Msg::NoCombos => ("Nessuna combo configurata in Vial", "No combos configured in Vial"),
        Msg::HeatOff => ("No", "Off"),
        Msg::HeatLayer => ("Layer", "Layer"),
        Msg::HeatAll => ("Tutti", "All"),
        Msg::MenuStats => ("Statistiche…", "Statistics…"),
        Msg::SearchHint => ("Cerca un tasto: è, redo, F10, {", "Find a key: é, redo, F10, {"),
        Msg::SearchNoResults => ("Nessun tasto trovato", "No key found"),
        Msg::StatsTitle => ("Statistiche", "Statistics"),
        Msg::StatsDisabled => ("Statistiche disattivate (o nessun layout caricato)", "Statistics off (or no layout loaded)"),
        Msg::StatsPeriod => ("Pressioni negli ultimi", "Presses in the last"),
        Msg::StatsAllTime => ("in totale", "all time"),
        Msg::StatsPrivacy => ("Solo contatori per tasto, layer e giorno: niente testo, sequenze o tempi.", "Only counters per key, layer and day: no text, sequences or timings."),
        Msg::StatsLayers => ("Utilizzo dei layer", "Layer usage"),
        Msg::StatsHands => ("Mani", "Hands"),
        Msg::StatsFingers => ("Dita", "Fingers"),
        Msg::StatsUnused => ("Tasti mai usati", "Keys never used"),
        Msg::StatsRows => ("Righe", "Rows"),
        Msg::StatsTop => ("Tasti più usati", "Most used keys"),
        Msg::StatsDaily => ("Attività giornaliera", "Daily activity"),
        Msg::StatsEffort => ("Fatica media per pressione (1 = indice sulla home row)", "Average effort per press (1 = index finger on the home row)"),
        Msg::StatsSuggestions => ("Suggerimenti", "Suggestions"),
        Msg::StatsNoSuggestions => ("Nessun suggerimento: i tasti più usati sono già in posizioni comode", "No suggestions: the most used keys are already in comfortable spots"),
        Msg::StatsSuggestionHint => ("Stima con un modello semplice (dito, riga, colonne di allungamento): un'idea da valutare, non una regola", "An estimate from a simple model (finger, row, stretch columns): an idea to consider, not a rule"),
        Msg::StatsFreeSpot => ("posto libero", "free spot"),
        Msg::Presses => ("pressioni", "presses"),
        Msg::StatsExport => ("Esporta CSV", "Export CSV"),
        Msg::StatsExported => ("Esportato", "Exported"),
        Msg::StatsReset => ("Azzera…", "Reset…"),
        Msg::StatsResetConfirm => ("Conferma: azzera tutto", "Confirm: reset everything"),
        Msg::StatsResetDone => ("Statistiche azzerate", "Statistics reset"),
        Msg::Cancel => ("Annulla", "Cancel"),
        Msg::Days => ("giorni", "days"),
        Msg::Left => ("sinistra", "left"),
        Msg::Right => ("destra", "right"),
        Msg::LabelErrors => ("Etichette non valide", "Invalid labels"),
        Msg::Pinned => ("HUD fissato (tocca HUD_TOG per tornare all'heads-up)", "HUD pinned (tap HUD_TOG to go back to heads-up)"),
        Msg::Unpinned => ("Heads-up", "Heads-up"),
        Msg::BadgeNoKeyboard => ("Tastiera non trovata", "Keyboard not found"),
        Msg::BadgePermission => ("Tastiera: permessi mancanti", "Keyboard: missing permissions"),
        Msg::BadgeNoHud => ("Firmware senza HUD", "Firmware without HUD"),
        Msg::BadgePausedVial => ("In pausa · Vial", "Paused · Vial"),
        Msg::BadgePausedForeign => ("In pausa · altro programma", "Paused · another program"),
        Msg::BadgeError => ("Errore della tastiera", "Keyboard error"),
        Msg::NumLock => ("Bloc Num", "Num Lock"),
        Msg::SplitLost => ("Altra metà scollegata", "Other half disconnected"),
        Msg::MenuStyle => ("Stile", "Style"),
        Msg::MenuKeyOpacity => ("Opacità tasti", "Key opacity"),
        Msg::MenuKeyRadius => ("Arrotondamento", "Rounding"),
        Msg::MenuSaveStyle => ("Salva come stile", "Save as style"),
        Msg::SaveStyleHint => ("Scrive un nuovo file in styles/ con tutti i valori attuali, colori compresi: un modello da modificare", "Writes a new file in styles/ with every current value, colors included: a template to edit"),
        Msg::MenuStylesFolder => ("Cartella stili", "Styles folder"),
        Msg::StyleSaved => ("Stile salvato", "Style saved"),
        Msg::MenuDiagnostics => ("Diagnostica…", "Diagnostics…"),
        Msg::DiagTitle => ("Diagnostica", "Diagnostics"),
        Msg::DiagKeyboard => ("Tastiera", "Keyboard"),
        Msg::DiagStatus => ("Stato", "Status"),
        Msg::DiagDefinition => ("Definizione Vial", "Vial definition"),
        Msg::DiagFirmware => ("Protocollo HUD", "HUD protocol"),
        Msg::DiagMatrix => ("Matrice", "Matrix"),
        Msg::DiagLink => ("Collegamento", "Link"),
        Msg::DiagPolls => ("Letture al secondo", "Reads per second"),
        Msg::DiagLatency => ("Latenza", "Latency"),
        Msg::DiagErrors => ("Errori", "Errors"),
        Msg::DiagConnections => ("Connessioni", "Connections"),
        Msg::DiagConnectedFor => ("Collegata da", "Connected for"),
        Msg::DiagLive => ("Stato della tastiera", "Keyboard state"),
        Msg::DiagKeymapVersion => ("Versione del keymap", "Keymap version"),
        Msg::DiagOs => ("Sistema operativo", "Operating system"),
        Msg::DiagSplit => ("Altra metà", "Other half"),
        Msg::DiagSplitOk => ("collegata", "connected"),
        Msg::DiagSplitUnknown => ("mai vista (tastiera non split?)", "never seen (not a split keyboard?)"),
        Msg::DiagFiles => ("File", "Files"),
        Msg::DiagConfig => ("Configurazione", "Configuration"),
        Msg::DiagLog => ("Log", "Log"),
        Msg::DiagStyles => ("Stili", "Styles"),
        Msg::DiagStats => ("Statistiche", "Statistics"),
        Msg::DiagBackups => ("Backup", "Backups"),
        Msg::DiagLastBackup => ("Ultimo backup", "Last backup"),
        Msg::DiagApp => ("App", "App"),
        Msg::DiagVersion => ("Versione", "Version"),
        Msg::DiagSystem => ("Sistema", "System"),
        Msg::DiagCopy => ("Copia tutto", "Copy all"),
        Msg::DiagCopied => ("Diagnostica copiata negli appunti", "Diagnostics copied to the clipboard"),
        Msg::DiagOpenFolder => ("Apri la cartella dei file", "Open the files folder"),
        Msg::DiagNone => ("nessuno", "none"),
        Msg::DiagOff => ("disattivato", "off"),
        Msg::DiagAgo => ("fa", "ago"),
        Msg::DiagLast => ("ultimo", "last"),
        Msg::MenuKeycast => ("Keycast (mostra le scorciatoie)", "Keycast (show shortcuts)"),
        Msg::MenuLanguage => ("Lingua", "Language"),
        Msg::LanguageAuto => ("Sistema", "System"),
        Msg::KeycastHint => ("Per video e presentazioni: mostra in basso le scorciatoie premute (Ctrl+C, Alt+Tab, F5...). Mai il testo, niente viene salvato.", "For videos and presentations: shows the shortcuts pressed at the bottom (Ctrl+C, Alt+Tab, F5...). Never text, nothing is saved."),
    };
    match lang {
        Lang::It => it,
        Lang::En => en,
    }
}

/// The user's language: Italian for an Italian locale, English otherwise.
pub fn system() -> Lang {
    let italian = is_italian;
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() && v != "C" && v != "POSIX" {
                return if italian(&v) { Lang::It } else { Lang::En };
            }
        }
    }
    if let Some(locale) = os_locale() {
        return if italian(&locale) { Lang::It } else { Lang::En };
    }
    Lang::En
}

/// "it_IT.UTF-8", "it-IT", "\"it\"" are Italian; "en_US", "fr" are not.
fn is_italian(locale: &str) -> bool {
    locale.trim_start_matches(['"', ' ', '(', '\n']).to_ascii_lowercase().starts_with("it")
}

#[cfg(windows)]
fn os_locale() -> Option<String> {
    // primary language id of the UI language: 0x10 = Italian
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    Some(if id & 0x3FF == 0x10 { "it".into() } else { "en".into() })
}

#[cfg(target_os = "macos")]
fn os_locale() -> Option<String> {
    // apps started from the Finder have no LANG: ask for the preferred languages
    let out = std::process::Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .map(str::trim)
        .find(|l| l.starts_with('"') || l.chars().next().is_some_and(char::is_alphabetic))
        .map(str::to_string)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn os_locale() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages() {
        assert_eq!(tr(Lang::It, Msg::MenuQuit), "Esci");
        assert_eq!(tr(Lang::En, Msg::MenuQuit), "Quit");
        assert_eq!(tr(Lang::En, Msg::DiagAgo), "ago");
    }

    #[test]
    fn italian_locales() {
        assert!(is_italian("it_IT.UTF-8"));
        assert!(is_italian("\"it-CH\","));
        assert!(!is_italian("en_US.UTF-8"));
        assert!(!is_italian("fr"));
    }
}
