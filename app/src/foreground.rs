//! Which application is in the foreground, for per-application labels:
//! - Windows: process of the foreground window (`Code.exe`);
//! - Linux/X11: `_NET_ACTIVE_WINDOW` → `_NET_WM_PID` → `/proc/<pid>/comm` (`code`);
//! - macOS: `NSWorkspace.frontmostApplication` (localized name, `Code`).
//!
//! Wayland has no standard way to know it: the result is `None`, and only
//! generic labels are shown. The HUD itself never counts as the foreground
//! application: clicking it keeps the labels of the previous one.
//!
//! Also whether that window is full screen (a game, a video, a slide show),
//! to hide the HUD: on Windows it covers its whole monitor, on X11 it has
//! `_NET_WM_STATE_FULLSCREEN`. Not known on Wayland and macOS.

use std::time::{Duration, Instant};

const CHECK_EVERY: Duration = Duration::from_millis(500);

pub struct Foreground {
    imp: imp::Imp,
    last_check: Option<Instant>,
    app: Option<String>,
    fullscreen: bool,
}

/// The foreground window.
pub struct Window {
    pub pid: u32,
    /// Executable or process name.
    pub app: String,
    pub fullscreen: bool,
}

impl Foreground {
    // `Imp` holds state on Linux only: `default()` works everywhere
    #[allow(clippy::default_constructed_unit_structs)]
    pub fn new() -> Self {
        Foreground { imp: imp::Imp::default(), last_check: None, app: None, fullscreen: false }
    }

    /// The foreground application, checked at most every 500 ms.
    pub fn poll(&mut self) -> Option<&str> {
        self.check();
        self.app.as_deref()
    }

    /// The foreground window is full screen, checked at most every 500 ms.
    pub fn fullscreen(&mut self) -> bool {
        self.check();
        self.fullscreen
    }

    fn check(&mut self) {
        if self.last_check.is_none_or(|t| t.elapsed() >= CHECK_EVERY) {
            self.last_check = Some(Instant::now());
            match self.imp.current() {
                Some(w) if w.pid == std::process::id() => {}
                Some(w) => {
                    self.app = Some(w.app);
                    self.fullscreen = w.fullscreen;
                }
                None => {
                    self.app = None;
                    self.fullscreen = false;
                }
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::Window;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, RECT};
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONULL};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsZoomed,
    };

    /// Covers its whole monitor, exactly: a maximized window goes a little
    /// past it, the desktop (`Progman`, `WorkerW`) does not count.
    ///
    /// # Safety
    /// `hwnd` must be a window handle (it may have been destroyed meanwhile).
    unsafe fn fullscreen(hwnd: HWND) -> bool {
        let mut class = [0u16; 32];
        let n = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
        let class = String::from_utf16_lossy(&class[..n]);
        if class == "Progman" || class == "WorkerW" || IsZoomed(hwnd) != 0 {
            return false;
        }
        let mut r: RECT = std::mem::zeroed();
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..std::mem::zeroed() };
        if GetWindowRect(hwnd, &mut r) == 0 || monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
            return false;
        }
        let m = info.rcMonitor;
        (r.left, r.top, r.right, r.bottom) == (m.left, m.top, m.right, m.bottom)
    }

    #[derive(Default)]
    pub struct Imp;

    impl Imp {
        /// The executable file name is the application.
        pub fn current(&mut self) -> Option<Window> {
            // SAFETY: plain Win32 calls; the buffer size is passed in characters
            // and the process handle is always closed
            unsafe {
                let hwnd = GetForegroundWindow();
                if hwnd.is_null() {
                    return None;
                }
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid == 0 {
                    return None;
                }
                let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if process.is_null() {
                    return None;
                }
                let mut buf = [0u16; 1024];
                let mut len = buf.len() as u32;
                let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0;
                CloseHandle(process);
                if !ok {
                    return None;
                }
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                let app = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
                Some(Window { pid, app, fullscreen: fullscreen(hwnd) })
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window as XWindow};
    use x11rb::rust_connection::RustConnection;

    struct X {
        conn: RustConnection,
        root: XWindow,
        active: u32,
        pid: u32,
        state: u32,
        fullscreen: u32,
    }

    /// The X connection is opened on first use; without X (pure Wayland) it
    /// is not tried again.
    #[derive(Default)]
    pub struct Imp {
        x: Option<X>,
        failed: bool,
    }

    fn connect() -> Option<X> {
        std::env::var_os("DISPLAY")?;
        let (conn, screen) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let atom = |name: &[u8]| conn.intern_atom(false, name).ok()?.reply().ok().map(|r| r.atom);
        let active = atom(b"_NET_ACTIVE_WINDOW")?;
        let pid = atom(b"_NET_WM_PID")?;
        let state = atom(b"_NET_WM_STATE")?;
        let fullscreen = atom(b"_NET_WM_STATE_FULLSCREEN")?;
        Some(X { conn, root, active, pid, state, fullscreen })
    }

    impl Imp {
        /// The process name is the application.
        pub fn current(&mut self) -> Option<super::Window> {
            if self.x.is_none() && !self.failed {
                self.x = connect();
                self.failed = self.x.is_none();
            }
            let x = self.x.as_ref()?;
            let prop = |win: XWindow, atom: u32, kind: AtomEnum| -> Option<u32> {
                let reply = x.conn.get_property(false, win, atom, kind, 0, 1).ok()?.reply().ok()?;
                let value = reply.value32()?.next();
                value
            };
            let win = prop(x.root, x.active, AtomEnum::WINDOW).filter(|&w| w != 0)?;
            let pid = prop(win, x.pid, AtomEnum::CARDINAL)?;
            let name = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
            let fullscreen = x
                .conn
                .get_property(false, win, x.state, AtomEnum::ATOM, 0, 32)
                .ok()
                .and_then(|c| c.reply().ok())
                .and_then(|r| r.value32().map(|mut v| v.any(|a| a == x.fullscreen)))
                .unwrap_or(false);
            Some(super::Window { pid, app: name.trim().to_string(), fullscreen })
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::Window;
    use objc2_app_kit::NSWorkspace;

    #[derive(Default)]
    pub struct Imp;

    impl Imp {
        /// The localized name is the application; full screen is not known.
        pub fn current(&mut self) -> Option<Window> {
            let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
            let pid = u32::try_from(app.processIdentifier()).ok()?;
            Some(Window { pid, app: app.localizedName()?.to_string(), fullscreen: false })
        }
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod imp {
    use super::Window;

    #[derive(Default)]
    pub struct Imp;

    impl Imp {
        pub fn current(&mut self) -> Option<Window> {
            None
        }
    }
}
