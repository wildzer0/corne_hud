//! Background thread that owns the keyboard: discovery, reconnection,
//! definition and keymap loading, state polling. Results reach the UI as
//! [`Event`]s over a channel.

use super::device::{Device, DeviceError};
use super::mock::MockKeyboard;
use super::protocol::{flags, HudInfo, HudState, ProtocolError};
use super::transport::{self, HidTransport, TransportError};
use crate::model::Keymap;
use crate::paths;
use crate::vial::definition::Definition;
use hidapi::HidApi;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const REQUEST_TIMEOUT: Duration = Duration::from_millis(50);
const RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_ERRORS: u32 = 5;
const POLL_HZ: f64 = 60.0;

#[derive(Debug, Clone)]
pub enum Source {
    Live { vid: Option<u16>, pid: Option<u16> },
    Mock,
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
    Error(String),
}

#[derive(Debug, Clone)]
pub enum Event {
    Status(Status),
    Layout { definition: Definition, layout_options: u32, keymap: Keymap, info: Option<HudInfo> },
    Keymap(Keymap),
    State(HudState),
}

pub struct Worker {
    pub events: Receiver<Event>,
    stop: Arc<AtomicBool>,
}

impl Worker {
    /// Starts the worker. `wake` is called after each event (e.g. to request a repaint).
    pub fn start(source: Source, wake: impl Fn() + Send + 'static) -> Self {
        let (tx, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let ctx = Ctx { tx, wake: Box::new(wake), stop: stop.clone() };
        thread::Builder::new().name("hid".into()).spawn(move || run(source, ctx)).expect("spawn HID thread");
        Worker { events, stop }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

struct Ctx {
    tx: Sender<Event>,
    wake: Box<dyn Fn() + Send>,
    stop: Arc<AtomicBool>,
}

impl Ctx {
    /// Returns false when the UI is gone.
    fn send(&self, e: Event) -> bool {
        let ok = self.tx.send(e).is_ok();
        (self.wake)();
        ok
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn sleep(&self, d: Duration) {
        let end = Instant::now() + d;
        while !self.stopped() && Instant::now() < end {
            thread::sleep(Duration::from_millis(20).min(end.saturating_duration_since(Instant::now())));
        }
    }
}

struct Connection {
    device: Device,
    product: String,
    info: Option<HudInfo>,
}

fn run(source: Source, ctx: Ctx) {
    let mut api = None;
    let mut last_status = None;
    while !ctx.stopped() {
        let status = match connect(&source, &mut api) {
            Ok(conn) => {
                let status = session(conn, &ctx);
                if ctx.stopped() {
                    return;
                }
                status
            }
            Err(status) => status,
        };
        if last_status.as_ref() != Some(&status) {
            log::info!("HID: {status:?}");
            if !ctx.send(Event::Status(status.clone())) {
                return;
            }
            last_status = Some(status);
        }
        ctx.sleep(RETRY_DELAY);
    }
}

fn connect(source: &Source, api: &mut Option<HidApi>) -> Result<Connection, Status> {
    let (vid, pid) = match source {
        Source::Mock => {
            let mut device = Device::new(Box::new(MockKeyboard::new()), REQUEST_TIMEOUT);
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
        let mut device = Device::new(Box::new(t), REQUEST_TIMEOUT);
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

/// Runs one connected session until the keyboard goes away. Returns the
/// status to report afterwards.
fn session(mut conn: Connection, ctx: &Ctx) -> Status {
    let setup = (|| -> Result<(Definition, u32, Keymap), DeviceError> {
        let def = load_definition(&mut conn.device)?;
        let options = conn.device.layout_options().unwrap_or(0);
        let keymap = read_keymap(&mut conn, &def)?;
        Ok((def, options, keymap))
    })();
    let (definition, layout_options, keymap) = match setup {
        Ok(v) => v,
        Err(e) => return Status::Error(e.to_string()),
    };
    let status = match conn.info {
        Some(_) => Status::Connected { product: conn.product.clone() },
        None => Status::NoHudFirmware { product: conn.product.clone() },
    };
    if !ctx.send(Event::Status(status))
        || !ctx.send(Event::Layout { definition: definition.clone(), layout_options, keymap, info: conn.info })
    {
        return Status::Searching;
    }

    let mut errors = 0;
    let mut last: Option<HudState> = None;
    while !ctx.stopped() {
        let started = Instant::now();
        if conn.info.is_none() {
            // Firmware without HUD: just check that the keyboard is still there.
            if conn.device.layer_count().is_err() {
                return Status::Searching;
            }
            ctx.sleep(Duration::from_secs(2));
            continue;
        }
        match conn.device.state() {
            Ok(state) => {
                errors = 0;
                let keymap_changed = last.is_some_and(|l| {
                    l.keymap_version != state.keymap_version || l.flag(flags::COLEMAK) != state.flag(flags::COLEMAK)
                });
                last = Some(state);
                if !ctx.send(Event::State(state)) {
                    return Status::Searching;
                }
                if keymap_changed {
                    match read_keymap(&mut conn, &definition) {
                        Ok(km) => {
                            if !ctx.send(Event::Keymap(km)) {
                                return Status::Searching;
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
                    return Status::Searching;
                }
            }
        }
        let period = Duration::from_secs_f64(1.0 / POLL_HZ);
        ctx.sleep(period.saturating_sub(started.elapsed()));
    }
    Status::Searching
}
