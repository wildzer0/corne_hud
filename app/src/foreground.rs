//! Which application is in the foreground, for per-application labels:
//! - Windows: process of the foreground window (`Code.exe`);
//! - Linux/X11: `_NET_ACTIVE_WINDOW` → `_NET_WM_PID` → `/proc/<pid>/comm` (`code`);
//! - macOS: `NSWorkspace.frontmostApplication` (localized name, `Code`).
//!
//! Wayland has no standard way to know it: the result is `None`, and only
//! generic labels are shown. The HUD itself never counts as the foreground
//! application: clicking it keeps the labels of the previous one.

use std::time::{Duration, Instant};

const CHECK_EVERY: Duration = Duration::from_millis(500);

pub struct Foreground {
    imp: imp::Imp,
    last_check: Option<Instant>,
    app: Option<String>,
}

impl Foreground {
    // `Imp` holds state on Linux only: `default()` works everywhere
    #[allow(clippy::default_constructed_unit_structs)]
    pub fn new() -> Self {
        Foreground { imp: imp::Imp::default(), last_check: None, app: None }
    }

    /// The foreground application, checked at most every 500 ms.
    pub fn poll(&mut self) -> Option<&str> {
        if self.last_check.is_none_or(|t| t.elapsed() >= CHECK_EVERY) {
            self.last_check = Some(Instant::now());
            match self.imp.current() {
                Some((pid, _)) if pid == std::process::id() => {}
                Some((_, name)) => self.app = Some(name),
                None => self.app = None,
            }
        }
        self.app.as_deref()
    }
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    #[derive(Default)]
    pub struct Imp;

    impl Imp {
        /// `(pid, executable file name)`.
        pub fn current(&mut self) -> Option<(u32, String)> {
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
                let name = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
                Some((pid, name))
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};
    use x11rb::rust_connection::RustConnection;

    struct X {
        conn: RustConnection,
        root: Window,
        active: u32,
        pid: u32,
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
        Some(X { conn, root, active, pid })
    }

    impl Imp {
        /// `(pid, process name)`.
        pub fn current(&mut self) -> Option<(u32, String)> {
            if self.x.is_none() && !self.failed {
                self.x = connect();
                self.failed = self.x.is_none();
            }
            let x = self.x.as_ref()?;
            let prop = |win: Window, atom: u32, kind: AtomEnum| -> Option<u32> {
                let reply = x.conn.get_property(false, win, atom, kind, 0, 1).ok()?.reply().ok()?;
                let value = reply.value32()?.next();
                value
            };
            let win = prop(x.root, x.active, AtomEnum::WINDOW).filter(|&w| w != 0)?;
            let pid = prop(win, x.pid, AtomEnum::CARDINAL)?;
            let name = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
            Some((pid, name.trim().to_string()))
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_app_kit::NSWorkspace;

    #[derive(Default)]
    pub struct Imp;

    impl Imp {
        /// `(pid, localized application name)`.
        pub fn current(&mut self) -> Option<(u32, String)> {
            let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
            let pid = u32::try_from(app.processIdentifier()).ok()?;
            Some((pid, app.localizedName()?.to_string()))
        }
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod imp {
    #[derive(Default)]
    pub struct Imp;

    impl Imp {
        pub fn current(&mut self) -> Option<(u32, String)> {
            None
        }
    }
}
