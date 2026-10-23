//! Updates from the GitHub releases: a check (once a day, or from the menu)
//! and, when asked, download, checksum and replacement of the portable
//! executable where it is, then a restart.
//!
//! Network access goes through the system `curl` (Windows 10 and later,
//! macOS and nearly every Linux have it): no TLS library in the program.
//! Nothing is sent but the request for the latest release.
//! - Windows: the running `.exe` cannot be overwritten but can be renamed:
//!   it becomes `<name>.old`, deleted at the next start;
//! - Linux: the new file takes the place of the old one (rename);
//! - macOS: the zip is unpacked with `ditto` and the whole `.app` is swapped.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const REPO: &str = "wildzer0/corne_hud";
/// The first automatic check waits a little: the start stays quick.
const FIRST_CHECK: Duration = Duration::from_secs(30);
const CHECK_EVERY: Duration = Duration::from_secs(24 * 3600);

/// The release file for this system, `None` where there is none.
const ASSET: Option<&str> = if cfg!(all(windows, target_arch = "x86_64")) {
    Some("keyboard-hud-windows-x86_64.exe")
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Some("keyboard-hud-linux-x86_64")
} else if cfg!(target_os = "macos") {
    Some("keyboard-hud-macos-universal.zip")
} else {
    None
};
const SUMS: &str = "SHA256SUMS.txt";

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// Without the leading `v`.
    pub version: String,
    /// The release page on GitHub.
    pub page: String,
    asset: Option<String>,
    sums: Option<String>,
}

impl Release {
    /// It can be installed here: the file for this system is there.
    pub fn installable(&self) -> bool {
        self.asset.is_some() && self.sums.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Installing(Release),
    /// Replaced: the program must restart.
    Installed,
    Failed(String),
}

pub struct Updater {
    state: Arc<Mutex<State>>,
    /// The executable as it was at start: once replaced, the system may
    /// report the renamed or deleted file.
    exe: Option<PathBuf>,
    started: Instant,
    last_check: Option<Instant>,
    /// The last check was asked by the user (it says "up to date" too).
    pub manual: bool,
}

impl Updater {
    pub fn new() -> Self {
        let exe = std::env::current_exe().ok();
        if let Some(exe) = exe.clone() {
            // the previous version may still be closing
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(5));
                cleanup(&exe);
            });
        }
        Updater {
            state: Arc::new(Mutex::new(State::Idle)),
            exe,
            started: Instant::now(),
            last_check: None,
            manual: false,
        }
    }

    pub fn state(&self) -> State {
        self.state.lock().map(|s| s.clone()).unwrap_or(State::Idle)
    }

    pub fn busy(&self) -> bool {
        matches!(self.state(), State::Checking | State::Installing(_) | State::Installed)
    }

    /// The automatic check, when it is due.
    pub fn tick(&mut self, ctx: &egui::Context, auto: bool) {
        let due = match self.last_check {
            None => self.started.elapsed() >= FIRST_CHECK,
            Some(t) => t.elapsed() >= CHECK_EVERY,
        };
        if auto && due && !self.busy() {
            self.check(ctx, false);
        }
    }

    pub fn check(&mut self, ctx: &egui::Context, manual: bool) {
        if self.busy() {
            return;
        }
        self.last_check = Some(Instant::now());
        self.manual = manual;
        self.run(ctx, State::Checking, || match latest() {
            Ok(r) if newer(&r.version, env!("CARGO_PKG_VERSION")) => State::Available(r),
            Ok(_) => State::UpToDate,
            Err(e) => State::Failed(e),
        });
    }

    pub fn install(&mut self, ctx: &egui::Context) {
        let State::Available(release) = self.state() else { return };
        let Some(exe) = self.exe.clone() else {
            self.set(State::Failed(
                crate::i18n::pick("percorso dell'eseguibile sconosciuto", "unknown executable path").into(),
            ));
            return;
        };
        self.manual = true;
        self.run(ctx, State::Installing(release.clone()), move || match install(&release, &exe) {
            Ok(()) => State::Installed,
            Err(e) => State::Failed(e),
        });
    }

    /// Starts the new version, with the same arguments.
    pub fn relaunch(&self) -> std::io::Result<()> {
        let exe = self.exe.as_deref().ok_or_else(|| std::io::Error::other("unknown executable path"))?;
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if cfg!(target_os = "macos") {
            if let Some(app) = bundle(exe) {
                return Command::new("open").arg("-n").arg(app).arg("--args").args(args).spawn().map(|_| ());
            }
        }
        Command::new(exe).args(args).spawn().map(|_| ())
    }

    fn set(&self, s: State) {
        if let Ok(mut g) = self.state.lock() {
            *g = s;
        }
    }

    fn run(&self, ctx: &egui::Context, now: State, job: impl FnOnce() -> State + Send + 'static) {
        self.set(now);
        let (state, ctx) = (self.state.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = job();
            if let Ok(mut g) = state.lock() {
                *g = result;
            }
            ctx.request_repaint();
        });
    }
}

/// `curl` with the output in memory.
fn curl(url: &str) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("curl");
    cmd.args(["-fsSL", "--max-time", "300", "-A", concat!("keyboard-hud/", env!("CARGO_PKG_VERSION"))]).arg(url);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if why.is_empty() { format!("curl: {}", out.status) } else { why });
    }
    Ok(out.stdout)
}

fn latest() -> Result<Release, String> {
    let body = curl(&format!("https://api.github.com/repos/{REPO}/releases/latest"))?;
    parse_release(&body)
}

fn parse_release(body: &[u8]) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().ok_or(crate::i18n::pick("risposta senza tag_name", "answer without tag_name"))?;
    let url_of = |name: &str| {
        v["assets"].as_array()?.iter().find(|a| a["name"] == name)?["browser_download_url"].as_str().map(str::to_owned)
    };
    Ok(Release {
        version: tag.trim_start_matches('v').to_string(),
        page: v["html_url"].as_str().unwrap_or_default().to_string(),
        asset: ASSET.and_then(url_of),
        sums: url_of(SUMS),
    })
}

fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut parts = s.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u32>().ok());
    let v = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(v)
}

/// `candidate` is a later version than `current` (anything unusual is not).
fn newer(candidate: &str, current: &str) -> bool {
    matches!((parse_version(candidate), parse_version(current)), (Some(a), Some(b)) if a > b)
}

/// The checksum of `name` in a `sha256sum` listing.
fn expected_sum<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|l| {
        let (hash, file) = l.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then_some(hash)
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn install(release: &Release, exe: &Path) -> Result<(), String> {
    let (Some(asset), Some(sums), Some(name)) = (&release.asset, &release.sums, ASSET) else {
        return Err(crate::i18n::pick(
            "nessun file per questo sistema nella release",
            "no file for this system in the release",
        )
        .into());
    };
    let data = curl(asset)?;
    let sums = String::from_utf8_lossy(&curl(sums)?).into_owned();
    let want = expected_sum(&sums, name)
        .ok_or(crate::i18n::pick("checksum mancante in SHA256SUMS.txt", "checksum missing from SHA256SUMS.txt"))?;
    if !hex(&Sha256::digest(&data)).eq_ignore_ascii_case(want) {
        return Err(crate::i18n::pick("checksum diverso: file scartato", "checksum mismatch: file discarded").into());
    }
    replace(exe, &data)
}

/// `<file>.<suffix>` next to it.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// The `.app` holding the executable (`X.app/Contents/MacOS/x`).
fn bundle(exe: &Path) -> Option<&Path> {
    exe.ancestors().nth(3).filter(|p| p.extension().is_some_and(|e| e == "app"))
}

fn not_writable(dir: &Path, e: std::io::Error) -> String {
    crate::tr_format!("cartella non scrivibile ({}): {e}", "folder not writable ({}): {e}", dir.display())
}

#[cfg(not(target_os = "macos"))]
fn replace(exe: &Path, data: &[u8]) -> Result<(), String> {
    let dir = exe.parent().unwrap_or(Path::new("."));
    let new = sibling(exe, ".new");
    std::fs::write(&new, data).map_err(|e| not_writable(dir, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        // a running .exe can be renamed, not overwritten
        let old = sibling(exe, ".old");
        let _ = std::fs::remove_file(&old);
        if let Err(e) = std::fs::rename(exe, &old) {
            let _ = std::fs::remove_file(&new);
            return Err(not_writable(dir, e));
        }
        if let Err(e) = std::fs::rename(&new, exe) {
            let _ = std::fs::rename(&old, exe);
            return Err(e.to_string());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    std::fs::rename(&new, exe).map_err(|e| {
        let _ = std::fs::remove_file(&new);
        not_writable(dir, e)
    })
}

#[cfg(target_os = "macos")]
fn replace(exe: &Path, data: &[u8]) -> Result<(), String> {
    let app = bundle(exe)
        .ok_or(crate::i18n::pick("non è dentro un .app: aggiornalo a mano", "not inside a .app: update it by hand"))?;
    let dir = app.parent().unwrap_or(Path::new("."));
    let work = dir.join(".keyboard-hud-update");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| not_writable(dir, e))?;
    let zip = work.join("update.zip");
    let result = (|| {
        std::fs::write(&zip, data).map_err(|e| e.to_string())?;
        let ok = Command::new("ditto").arg("-x").arg("-k").arg(&zip).arg(&work).status().is_ok_and(|s| s.success());
        let new = work.join("Keyboard HUD.app");
        if !ok || !new.is_dir() {
            return Err(crate::i18n::pick("archivio non valido", "invalid archive").to_string());
        }
        let old = sibling(app, ".old");
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(app, &old).map_err(|e| not_writable(dir, e))?;
        if let Err(e) = std::fs::rename(&new, app) {
            let _ = std::fs::rename(&old, app);
            return Err(e.to_string());
        }
        let _ = std::fs::remove_dir_all(&old);
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// What a previous update left behind.
fn cleanup(exe: &Path) {
    let _ = std::fs::remove_file(sibling(exe, ".old"));
    let _ = std::fs::remove_file(sibling(exe, ".new"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(newer("1.3.0", "1.2.3"));
        assert!(newer("v2.0.0", "1.9.9"));
        assert!(newer("1.2.10", "1.2.9"));
        assert!(!newer("1.2.3", "1.2.3"));
        assert!(!newer("1.2.2", "1.2.3"));
        assert!(!newer("1.3.0-beta", "1.2.3"));
        assert!(!newer("1.3", "1.2.3"));
    }

    #[test]
    fn release_and_checksums() {
        let body = br#"{"tag_name":"v9.8.7","html_url":"https://github.com/x/y/releases/tag/v9.8.7","assets":[
            {"name":"SHA256SUMS.txt","browser_download_url":"https://e/SHA256SUMS.txt"},
            {"name":"keyboard-hud-windows-x86_64.exe","browser_download_url":"https://e/w.exe"},
            {"name":"keyboard-hud-linux-x86_64","browser_download_url":"https://e/l"},
            {"name":"keyboard-hud-macos-universal.zip","browser_download_url":"https://e/m.zip"}]}"#;
        let r = parse_release(body).unwrap();
        assert_eq!(r.version, "9.8.7");
        assert_eq!(r.sums.as_deref(), Some("https://e/SHA256SUMS.txt"));
        assert_eq!(r.installable(), ASSET.is_some());
        assert!(parse_release(b"{}").is_err());

        let sums = "aa11  keyboard-hud-linux-x86_64\nbb22 *keyboard-hud-windows-x86_64.exe\n";
        assert_eq!(expected_sum(sums, "keyboard-hud-linux-x86_64"), Some("aa11"));
        assert_eq!(expected_sum(sums, "keyboard-hud-windows-x86_64.exe"), Some("bb22"));
        assert_eq!(expected_sum(sums, "keyboard-hud.example.toml"), None);
        assert_eq!(hex(&Sha256::digest(b"abc"))[..8], *"ba7816bf");
    }

    #[test]
    fn paths() {
        assert_eq!(sibling(Path::new("/a/hud.exe"), ".old"), PathBuf::from("/a/hud.exe.old"));
        let exe = Path::new("/Apps/Keyboard HUD.app/Contents/MacOS/keyboard-hud");
        assert_eq!(bundle(exe), Some(Path::new("/Apps/Keyboard HUD.app")));
        assert_eq!(bundle(Path::new("/usr/bin/keyboard-hud")), None);
    }

    /// The whole path on the real latest release: `cargo test -- --ignored`.
    #[cfg(not(any(windows, target_os = "macos")))]
    #[test]
    #[ignore = "network"]
    fn installs_the_latest_release() {
        let release = latest().unwrap();
        assert!(release.installable(), "{release:?}");
        let dir = std::env::temp_dir().join(format!("hud-install-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("keyboard-hud");
        std::fs::write(&exe, b"old").unwrap();
        install(&release, &exe).unwrap();
        let out = Command::new(&exe).arg("--version").output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains(&release.version), "{out:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    #[test]
    fn replaces_the_file_in_place() {
        let dir = std::env::temp_dir().join(format!("hud-update-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("keyboard-hud");
        std::fs::write(&exe, b"old").unwrap();
        replace(&exe, b"new").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert!(!sibling(&exe, ".new").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
