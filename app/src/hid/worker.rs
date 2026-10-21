//! Background thread that owns the keyboard: discovery, reconnection,
//! definition and keymap loading, state polling, and pausing while Vial is
//! running. Results reach the UI as [`Event`]s over a channel.

use super::device::{Device, DeviceError};
use super::mock::MockKeyboard;
use super::protocol::{flags, HudInfo, HudState, ProtocolError};
use super::transport::{self, HidTransport, TransportError};
use crate::model::Keymap;
use crate::paths;
use crate::vial::definition::Definition;
use hidapi::HidApi;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const RETRY_DELAY: Duration = Duration::from_secs(1);
const PAUSE_CHECK: Duration = Duration::from_millis(250);
const VIAL_CHECK_INTERVAL: Duration = Duration::from_secs(2);
const MAX_ERRORS: u32 = 5;

#[derive(Debug, Clone)]
pub enum Source {
    Live {
        vid: Option<u16>,
        pid: Option<u16>,
    },
    /// Simulated keyboard, starting `n` ms into its scripted loop.
    Mock(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseReason {
    /// Vial (or VIA) is running and uses the same HID interface.
    Vial,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Searching,
    /// Linux: the hidraw node is not readable (missing udev rule).
    PermissionDenied(String),
    Connected {
        product: String,
    },
    /// Keyboard found, but its firmware does not speak the HUD protocol:
    /// keymap read via VIA, no live state.
    NoHudFirmware {
        product: String,
    },
    Paused(PauseReason),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum Event {
    Status(Status),
    Layout { definition: Definition, layout_options: u32, keymap: Keymap, info: Option<HudInfo> },
    Keymap(Keymap),
    State(HudState),
}

/// Settings shared with the HID thread.
struct Control {
    stop: AtomicBool,
    paused: AtomicBool,
    poll_hz: AtomicU32,
    timeout_ms: AtomicU64,
}

pub struct Worker {
    pub events: Receiver<Event>,
    control: Arc<Control>,
}

impl Worker {
    /// Starts the worker. `wake` is called after each event (e.g. to request a repaint).
    pub fn start(source: Source, poll_hz: u32, timeout_ms: u64, wake: impl Fn() + Send + 'static) -> Self {
        let (tx, events) = mpsc::channel();
        let control = Arc::new(Control {
            stop: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            poll_hz: AtomicU32::new(poll_hz),
            timeout_ms: AtomicU64::new(timeout_ms),
        });
        let ctx = Ctx { tx, wake: Box::new(wake), control: control.clone() };
        thread::Builder::new().name("hid".into()).spawn(move || run(source, ctx)).expect("spawn HID thread");
        Worker { events, control }
    }

    pub fn set_poll_hz(&self, hz: u32) {
        self.control.poll_hz.store(hz.clamp(1, 250), Ordering::Relaxed);
    }

    pub fn set_timeout_ms(&self, ms: u64) {
        self.control.timeout_ms.store(ms.clamp(5, 1000), Ordering::Relaxed);
    }

    /// Manual pause: closes the device until resumed.
    pub fn set_paused(&self, paused: bool) {
        self.control.paused.store(paused, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.control.paused.load(Ordering::Relaxed)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.control.stop.store(true, Ordering::Relaxed);
    }
}

struct Ctx {
    tx: Sender<Event>,
    wake: Box<dyn Fn() + Send>,
    control: Arc<Control>,
}

impl Ctx {
    /// Returns false when the UI is gone.
    fn send(&self, e: Event) -> bool {
        let ok = self.tx.send(e).is_ok();
        (self.wake)();
        ok
    }

    fn stopped(&self) -> bool {
        self.control.stop.load(Ordering::Relaxed)
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(self.control.timeout_ms.load(Ordering::Relaxed))
    }

    fn sleep(&self, d: Duration) {
        let end = Instant::now() + d;
        while !self.stopped() && Instant::now() < end {
            thread::sleep(Duration::from_millis(10).min(end.saturating_duration_since(Instant::now())));
        }
    }
}

/// Process names that use the keyboard's Raw HID interface.
pub fn is_vial_process(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let n = n.strip_suffix(".exe").unwrap_or(&n);
    // AppImages run as e.g. "Vial-v0.7.5-x86_64.AppImage" (truncated by Linux to 15 chars)
    n == "vial" || n.starts_with("vial-") || n.starts_with("vial.") || n == "via"
}

/// Detects Vial/VIA among the running processes (checked every 2 s).
struct VialWatch {
    system: sysinfo::System,
    last: Option<Instant>,
    running: bool,
}

impl VialWatch {
    fn new() -> Self {
        VialWatch { system: sysinfo::System::new(), last: None, running: false }
    }

    fn running(&mut self) -> bool {
        if self.last.is_some_and(|t| t.elapsed() < VIAL_CHECK_INTERVAL) {
            return self.running;
        }
        self.last = Some(Instant::now());
        self.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::nothing(),
        );
        self.running = self.system.processes().values().any(|p| is_vial_process(&p.name().to_string_lossy()));
        self.running
    }
}

struct Connection {
    device: Device,
    product: String,
    info: Option<HudInfo>,
}

enum SessionEnd {
    Lost(Status),
    Paused,
}

fn pause_reason(ctx: &Ctx, vial: &mut Option<VialWatch>) -> Option<PauseReason> {
    if ctx.control.paused.load(Ordering::Relaxed) {
        Some(PauseReason::Manual)
    } else if vial.as_mut().is_some_and(VialWatch::running) {
        Some(PauseReason::Vial)
    } else {
        None
    }
}

/// Sends status changes only (repeated failures are not reported again).
struct Reporter {
    last: Option<Status>,
}

impl Reporter {
    fn report(&mut self, ctx: &Ctx, status: Status) -> bool {
        if self.last.as_ref() == Some(&status) {
            return true;
        }
        log::info!("HID: {status:?}");
        self.last = Some(status.clone());
        ctx.send(Event::Status(status))
    }
}

fn run(source: Source, ctx: Ctx) {
    let mut api = None;
    // the simulated keyboard does not touch the HID interface: no need to yield to Vial
    let mut vial = matches!(source, Source::Live { .. }).then(VialWatch::new);
    let mut reporter = Reporter { last: None };

    while !ctx.stopped() {
        if let Some(reason) = pause_reason(&ctx, &mut vial) {
            if !reporter.report(&ctx, Status::Paused(reason)) {
                return;
            }
            ctx.sleep(PAUSE_CHECK);
            continue;
        }
        let status = match connect(&source, &mut api, ctx.timeout()) {
            Ok(conn) => match session(conn, &ctx, &mut vial, &mut reporter) {
                SessionEnd::Paused => continue,
                SessionEnd::Lost(status) => status,
            },
            Err(status) => status,
        };
        if ctx.stopped() || !reporter.report(&ctx, status) {
            return;
        }
        ctx.sleep(RETRY_DELAY);
    }
}

fn connect(source: &Source, api: &mut Option<HidApi>, timeout: Duration) -> Result<Connection, Status> {
    let (vid, pid) = match source {
        Source::Mock(offset) => {
            let mut device = Device::new(Box::new(MockKeyboard::starting_at(*offset)), timeout);
            let info = device.info().map_err(|e| Status::Error(e.to_string()))?;
            return Ok(Connection { device, product: "Tastiera simulata".into(), info: Some(info) });
        }
        Source::Live { vid, pid } => (*vid, *pid),
    };
    if api.is_none() {
        *api = Some(HidApi::new().map_err(|e| Status::Error(e.to_string()))?);
    }
    let api = api.as_mut().expect("initialized above");
    let candidates = transport::enumerate(api, vid, pid).map_err(|e| Status::Error(e.to_string()))?;
    let mut permission = None;
    let mut fallback = None;
    for c in &candidates {
        let t = match HidTransport::open(api, c) {
            Ok(t) => t,
            Err(TransportError::PermissionDenied(path)) => {
                permission.get_or_insert(path);
                continue;
            }
            Err(e) => {
                log::debug!("{} ({:04x}:{:04x}): {e}", c.product, c.vendor_id, c.product_id);
                continue;
            }
        };
        let mut device = Device::new(Box::new(t), timeout);
        match device.info() {
            Ok(info) => return Ok(Connection { device, product: c.product.clone(), info: Some(info) }),
            Err(DeviceError::Protocol(ProtocolError::Unsupported)) => {
                if fallback.is_none() {
                    fallback = Some(Connection { device, product: c.product.clone(), info: None });
                }
            }
            Err(e) => log::debug!("{}: {e}", c.product),
        }
    }
    if let Some(conn) = fallback {
        return Ok(conn);
    }
    Err(match permission {
        Some(path) => Status::PermissionDenied(path),
        None => Status::Searching,
    })
}

/// Loads the definition, using the on-disk cache when possible.
fn load_definition(device: &mut Device) -> Result<Definition, DeviceError> {
    let (_, uid) = device.vial_id()?;
    let size = device.definition_size()?;
    let cache = paths::definition_cache(uid, size);
    if let Ok(text) = std::fs::read_to_string(&cache) {
        match Definition::from_json_str(&text) {
            Ok(d) => return Ok(d),
            Err(e) => log::warn!("cache {} non valida: {e}", cache.display()),
        }
    }
    let xz = device.definition_xz(size)?;
    let text = Definition::xz_to_json(&xz).map_err(DeviceError::Definition)?;
    let def = Definition::from_json_str(&text).map_err(DeviceError::Definition)?;
    if std::fs::create_dir_all(paths::cache_dir()).and_then(|_| std::fs::write(&cache, &text)).is_err() {
        log::warn!("impossibile scrivere la cache {}", cache.display());
    }
    Ok(def)
}

fn read_keymap(conn: &mut Connection, def: &Definition) -> Result<Keymap, DeviceError> {
    match &conn.info {
        Some(info) => conn.device.keymap(info),
        None => {
            let layers = conn.device.layer_count()?;
            conn.device.via_keymap(layers, def.rows, def.cols)
        }
    }
}

/// Runs one connected session until the keyboard goes away or a pause is
/// requested (the device is then closed by dropping `conn`).
fn session(mut conn: Connection, ctx: &Ctx, vial: &mut Option<VialWatch>, reporter: &mut Reporter) -> SessionEnd {
    let setup = (|| -> Result<(Definition, u32, Keymap), DeviceError> {
        let def = load_definition(&mut conn.device)?;
        let options = conn.device.layout_options().unwrap_or(0);
        let keymap = read_keymap(&mut conn, &def)?;
        Ok((def, options, keymap))
    })();
    let (definition, layout_options, keymap) = match setup {
        Ok(v) => v,
        Err(e) => return SessionEnd::Lost(Status::Error(e.to_string())),
    };
    let status = match conn.info {
        Some(_) => Status::Connected { product: conn.product.clone() },
        None => Status::NoHudFirmware { product: conn.product.clone() },
    };
    // force the status event: the UI drops the layout on any non-connected status
    reporter.last = None;
    if !reporter.report(ctx, status)
        || !ctx.send(Event::Layout { definition: definition.clone(), layout_options, keymap, info: conn.info })
    {
        return SessionEnd::Lost(Status::Searching);
    }

    let mut errors = 0;
    let mut last: Option<HudState> = None;
    while !ctx.stopped() {
        let started = Instant::now();
        if pause_reason(ctx, vial).is_some() {
            log::info!("HID: dispositivo chiuso (pausa)");
            return SessionEnd::Paused;
        }
        if conn.info.is_none() {
            // Firmware without HUD: just check now and then that the keyboard is still there.
            if conn.device.layer_count().is_err() {
                return SessionEnd::Lost(Status::Searching);
            }
            ctx.sleep(Duration::from_secs(1));
            continue;
        }
        match conn.device.state() {
            Ok(state) => {
                errors = 0;
                let keymap_changed = last.is_some_and(|l| {
                    l.keymap_version != state.keymap_version || l.flag(flags::COLEMAK) != state.flag(flags::COLEMAK)
                });
                // only changes reach the UI: an idle keyboard costs no repaints
                let changed = last != Some(state);
                last = Some(state);
                if changed && !ctx.send(Event::State(state)) {
                    return SessionEnd::Lost(Status::Searching);
                }
                if keymap_changed {
                    log::info!("keymap cambiato (versione {}): rilettura", state.keymap_version);
                    match read_keymap(&mut conn, &definition) {
                        Ok(km) => {
                            if !ctx.send(Event::Keymap(km)) {
                                return SessionEnd::Lost(Status::Searching);
                            }
                        }
                        Err(e) => log::warn!("rilettura del keymap fallita: {e}"),
                    }
                }
            }
            Err(DeviceError::Protocol(ProtocolError::Busy)) => {}
            Err(e) => {
                errors += 1;
                log::debug!("GET_STATE: {e}");
                if errors >= MAX_ERRORS {
                    log::warn!("la tastiera non risponde: riconnessione");
                    return SessionEnd::Lost(Status::Searching);
                }
            }
        }
        let hz = ctx.control.poll_hz.load(Ordering::Relaxed).max(1);
        ctx.sleep(Duration::from_secs_f64(1.0 / hz as f64).saturating_sub(started.elapsed()));
    }
    SessionEnd::Lost(Status::Searching)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vial_process_names() {
        for yes in ["Vial", "Vial.exe", "vial", "Vial-v0.7.5-x8", "VIA.exe", "via"] {
            assert!(is_vial_process(yes), "{yes}");
        }
        for no in ["keyboard-hud", "viable", "trivial", "vialtools", "Via Ferrata"] {
            assert!(!is_vial_process(no), "{no}");
        }
    }

    #[test]
    fn process_list_is_readable() {
        let mut w = VialWatch::new();
        let _ = w.running();
        assert!(!w.system.processes().is_empty(), "sysinfo returned no processes");
    }

    #[test]
    fn mock_session_delivers_layout_states_and_pauses() {
        let w = Worker::start(Source::Mock(0), 100, 50, || {});
        let mut got_layout = false;
        let mut states = 0;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !(got_layout && states >= 3) {
            match w.events.recv_timeout(Duration::from_millis(100)) {
                Ok(Event::Layout { keymap, .. }) => {
                    assert_eq!(keymap.layers(), 6);
                    got_layout = true;
                }
                Ok(Event::State(_)) => states += 1,
                _ => {}
            }
        }
        assert!(got_layout && states >= 3, "layout {got_layout}, states {states}");

        // manual pause closes the session and reports it; resuming reloads the layout
        w.set_paused(true);
        assert!(w.is_paused());
        let wait_for = |pred: &dyn Fn(&Event) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if let Ok(e) = w.events.recv_timeout(Duration::from_millis(100)) {
                    if pred(&e) {
                        return true;
                    }
                }
            }
            false
        };
        assert!(wait_for(&|e| matches!(e, Event::Status(Status::Paused(PauseReason::Manual)))));
        w.set_paused(false);
        assert!(wait_for(&|e| matches!(e, Event::Layout { .. })));
    }
}
