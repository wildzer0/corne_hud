//! Command line options (hand-written: a handful of flags, no extra dependency).

use std::path::PathBuf;

pub const USAGE: &str = "\
Uso: keyboard-hud [opzioni]

  --mock                tastiera simulata (sviluppo dell'interfaccia)
  --offline             nessuna tastiera: mostra vial.json + .vil dal disco
  --vial-json PERCORSO  definizione Vial per la modalità offline
  --vil PERCORSO        backup Vial (.vil) per la modalità offline
  --vid 0xVVVV          usa solo dispositivi con questo Vendor ID
  --pid 0xPPPP          usa solo dispositivi con questo Product ID
  --x11                 Linux: forza XWayland (finestra sempre in primo piano)
  --print-udev-rule     stampa la regola udev per Linux ed esce
  --layer N             layer mostrato all'avvio in modalità offline
  --mock-at MS          sviluppo: fa partire lo scenario simulato da MS millisecondi
  --screenshot FILE     sviluppo: salva la finestra in FILE (PPM) ed esce
  --screenshot-after MS sviluppo: attesa prima dello screenshot (default 2000)
  --search TESTO        sviluppo: apre la ricerca con questo testo
  --heatmap layer|all   sviluppo: mostra la heatmap all'avvio
  --stats-window        sviluppo: apre la finestra delle statistiche
  --menu                sviluppo: apre il menu contestuale all'avvio
  --verbose             log dettagliato
  --version             versione
  -h, --help            questo aiuto";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub mock: bool,
    pub offline: bool,
    pub vial_json: Option<PathBuf>,
    pub vil: Option<PathBuf>,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub x11: bool,
    pub layer: u8,
    pub screenshot: Option<PathBuf>,
    pub mock_at: u64,
    pub screenshot_after: Option<u64>,
    pub search: Option<String>,
    pub heatmap: Option<String>,
    pub stats_window: bool,
    pub menu: bool,
    pub print_udev_rule: bool,
    pub verbose: bool,
    pub version: bool,
    pub help: bool,
}

fn parse_id(s: &str) -> Result<u16, String> {
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    u16::from_str_radix(hex, 16).map_err(|_| format!("ID non valido: {s}"))
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut o = Options::default();
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} richiede un valore"));
        match a.as_str() {
            "--mock" => o.mock = true,
            "--offline" => o.offline = true,
            "--vial-json" => o.vial_json = Some(value("--vial-json")?.into()),
            "--vil" => o.vil = Some(value("--vil")?.into()),
            "--vid" => o.vid = Some(parse_id(&value("--vid")?)?),
            "--pid" => o.pid = Some(parse_id(&value("--pid")?)?),
            "--x11" => o.x11 = true,
            "--layer" => o.layer = value("--layer")?.parse().map_err(|_| "--layer: numero non valido".to_string())?,
            "--mock-at" => {
                o.mock_at = value("--mock-at")?.parse().map_err(|_| "--mock-at: numero non valido".to_string())?
            }
            "--screenshot" => o.screenshot = Some(value("--screenshot")?.into()),
            "--screenshot-after" => {
                o.screenshot_after = Some(
                    value("--screenshot-after")?
                        .parse()
                        .map_err(|_| "--screenshot-after: numero non valido".to_string())?,
                )
            }
            "--search" => o.search = Some(value("--search")?),
            "--heatmap" => {
                let v = value("--heatmap")?;
                if v != "layer" && v != "all" {
                    return Err("--heatmap: usa 'layer' o 'all'".into());
                }
                o.heatmap = Some(v);
            }
            "--stats-window" => o.stats_window = true,
            "--menu" => o.menu = true,
            "--print-udev-rule" => o.print_udev_rule = true,
            "--verbose" | "-v" => o.verbose = true,
            "--version" => o.version = true,
            "--help" | "-h" => o.help = true,
            other => return Err(format!("opzione sconosciuta: {other}")),
        }
    }
    if o.mock && o.offline {
        return Err("--mock e --offline sono alternativi".into());
    }
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Options, String> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_flags() {
        let o = p(&["--mock", "--vid", "0x4653", "--pid", "0004", "--verbose"]).unwrap();
        assert!(o.mock && o.verbose);
        assert_eq!((o.vid, o.pid), (Some(0x4653), Some(0x0004)));
        let o = p(&["--offline", "--vial-json", "a.json", "--vil", "b.vil"]).unwrap();
        assert_eq!(o.vil, Some(PathBuf::from("b.vil")));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(p(&["--nope"]).is_err());
        assert!(p(&["--vid"]).is_err());
        assert!(p(&["--vid", "xyz"]).is_err());
        assert!(p(&["--mock", "--offline"]).is_err());
    }
}
