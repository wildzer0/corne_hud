//! Keyboard HUD: portable on-screen display of a Vial/QMK keyboard state.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cli;
mod hid;
mod i18n;
mod kle;
mod logging;
mod model;
mod paths;
mod ui;
mod vial;

use cli::Options;
use hid::mock::{MOCK_VIAL_JSON, MOCK_VIL};
use hid::worker::{Source, Worker};
use std::path::Path;
use std::process::ExitCode;
use ui::{HudApp, Layout, Mode, Screenshot};
use vial::definition::Definition;
use vial::vil::Vil;

fn load_offline(vial_json: &Path, vil: &Path) -> Result<Layout, String> {
    let def_text = std::fs::read_to_string(vial_json).map_err(|e| format!("{}: {e}", vial_json.display()))?;
    let vil_text = std::fs::read_to_string(vil).map_err(|e| format!("{}: {e}", vil.display()))?;
    layout_from(&def_text, &vil_text)
}

fn layout_from(def_text: &str, vil_text: &str) -> Result<Layout, String> {
    let def = Definition::from_json_str(def_text).map_err(|e| format!("vial.json: {e}"))?;
    let vil = Vil::parse(vil_text, def.rows, def.cols).map_err(|e| e.to_string())?;
    if let Some(uid) = vil.uid {
        log::info!("offline: {} (uid {uid:016x})", def.name);
    }
    if !vil.unknown.is_empty() {
        log::warn!("keycode sconosciuti nel .vil: {:?}", vil.unknown);
    }
    Ok(Layout::new(def, vil.layout_options, vil.keymap))
}

/// `vial.json` and the first `*.vil` in the app folder, if present.
fn find_offline_files() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let dir = paths::base_dir();
    let json = dir.join("vial.json");
    let vil = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vil")))
        .min()?;
    json.exists().then_some((json, vil))
}

fn main() -> ExitCode {
    let opts = match cli::parse(std::env::args().skip(1)) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}\n\n{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    if opts.help {
        println!("{}", cli::USAGE);
        return ExitCode::SUCCESS;
    }
    if opts.version {
        println!("keyboard-hud {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if opts.print_udev_rule {
        print!("{}", hid::transport::udev_instructions());
        return ExitCode::SUCCESS;
    }
    logging::init(opts.verbose);
    run(opts)
}

fn run(opts: Options) -> ExitCode {
    if opts.x11 {
        // Force XWayland: winit picks Wayland only when this variable is set.
        // Safe here: no other thread exists yet.
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    // Offline layout: explicit paths, files next to the app, or (with
    // --offline / --mock) the bundled sample.
    let offline = match (&opts.vial_json, &opts.vil) {
        (Some(j), Some(v)) => match load_offline(j, v) {
            Ok(l) => Some(l),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        },
        (None, None) => find_offline_files()
            .and_then(|(j, v)| load_offline(&j, &v).map_err(|e| log::warn!("{e}")).ok())
            .or_else(|| (opts.offline || opts.mock).then(|| layout_from(MOCK_VIAL_JSON, MOCK_VIL).ok()).flatten()),
        _ => {
            eprintln!("--vial-json e --vil vanno indicati insieme");
            return ExitCode::from(2);
        }
    };

    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Keyboard HUD")
            .with_app_id("keyboard-hud")
            .with_inner_size([900.0, 400.0])
            .with_min_inner_size([300.0, 140.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top(),
        ..Default::default()
    };

    let result = eframe::run_native(
        "Keyboard HUD",
        native,
        Box::new(move |cc| {
            ui::install_fonts(&cc.egui_ctx);
            let mode = if opts.offline {
                Mode::Offline
            } else {
                let ctx = cc.egui_ctx.clone();
                let source = if opts.mock { Source::Mock } else { Source::Live { vid: opts.vid, pid: opts.pid } };
                Mode::Live { worker: Worker::start(source, move || ctx.request_repaint()), mock: opts.mock }
            };
            let shot = opts.screenshot.map(|p| Screenshot::new(p, std::time::Duration::from_secs(2)));
            Ok(Box::new(HudApp::new(mode, offline, opts.layer, shot)))
        }),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
