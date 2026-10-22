//! Start with the system, for the current user only and without admin rights:
//! - Windows: value in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`;
//! - Linux: `~/.config/autostart/keyboard-hud.desktop` (XDG autostart);
//! - macOS: `~/Library/LaunchAgents/io.github.keyboard-hud.plist`.
//!
//! Each system stores one text (command line or file). The entry is "on"
//! when that text is exactly what this executable would write, "stale" when
//! it points somewhere else (e.g. the executable was moved).

use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Off,
    On,
    /// Registered, but for another path or other options.
    Stale,
}

/// Where the entry lives, for messages.
pub fn location() -> String {
    imp::location()
}

pub fn state(exe: &Path, args: &[String]) -> io::Result<State> {
    Ok(match imp::read()? {
        None => State::Off,
        Some(t) if t == imp::expected(exe, args) => State::On,
        Some(_) => State::Stale,
    })
}

/// Turns autostart on (for this executable and `args`) or off.
pub fn set(enabled: bool, exe: &Path, args: &[String]) -> io::Result<()> {
    if enabled {
        imp::write(&imp::expected(exe, args))
    } else {
        imp::remove()
    }
}

/// `"exe" args...`, quoting the executable path (Windows command line).
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_command(exe: &Path, args: &[String]) -> String {
    let mut s = format!("\"{}\"", exe.display());
    for a in args {
        s.push(' ');
        s.push_str(a);
    }
    s
}

/// XDG desktop entry. `Exec` quotes every argument, escaping as the spec asks.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn desktop_entry(exe: &Path, args: &[String]) -> String {
    let quote = |s: &str| {
        let mut q = String::from("\"");
        for ch in s.chars() {
            match ch {
                '"' | '`' | '$' | '\\' => {
                    q.push('\\');
                    q.push(ch);
                }
                // field codes like %f
                '%' => q.push_str("%%"),
                _ => q.push(ch),
            }
        }
        q.push('"');
        // the desktop entry format escapes backslashes once more
        q.replace('\\', "\\\\")
    };
    let mut exec = quote(&exe.display().to_string());
    for a in args {
        exec.push(' ');
        exec.push_str(&quote(a));
    }
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Keyboard HUD\n\
         Comment=HUD della tastiera, avviato con la sessione\n\
         Exec={exec}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

/// launchd agent that starts the app at login.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn launch_agent(exe: &Path, args: &[String]) -> String {
    let xml = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut program = format!("        <string>{}</string>\n", xml(&exe.display().to_string()));
    for a in args {
        program.push_str(&format!("        <string>{}</string>\n", xml(a)));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \x20   <key>Label</key>\n\
         \x20   <string>io.github.keyboard-hud</string>\n\
         \x20   <key>ProgramArguments</key>\n\
         \x20   <array>\n\
         {program}\
         \x20   </array>\n\
         \x20   <key>RunAtLoad</key>\n\
         \x20   <true/>\n\
         \x20   <key>ProcessType</key>\n\
         \x20   <string>Interactive</string>\n\
         </dict>\n\
         </plist>\n"
    )
}

/// Linux and macOS: the entry is a file.
#[cfg(not(windows))]
mod file {
    use std::io;
    use std::path::Path;

    pub fn read(path: &Path) -> io::Result<Option<String>> {
        match std::fs::read_to_string(path) {
            Ok(t) => Ok(Some(t)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn write(path: &Path, text: &str) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, text)
    }

    pub fn remove(path: &Path) -> io::Result<()> {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::file;
    use std::io;
    use std::path::{Path, PathBuf};

    fn path() -> io::Result<PathBuf> {
        let base = directories::BaseDirs::new().ok_or_else(|| io::Error::other("cartella home non trovata"))?;
        Ok(base.config_dir().join("autostart").join("keyboard-hud.desktop"))
    }

    pub fn location() -> String {
        path().map(|p| p.display().to_string()).unwrap_or_default()
    }

    pub fn expected(exe: &Path, args: &[String]) -> String {
        super::desktop_entry(exe, args)
    }

    pub fn read() -> io::Result<Option<String>> {
        file::read(&path()?)
    }

    pub fn write(text: &str) -> io::Result<()> {
        file::write(&path()?, text)
    }

    pub fn remove() -> io::Result<()> {
        file::remove(&path()?)
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::file;
    use std::io;
    use std::path::{Path, PathBuf};

    fn path() -> io::Result<PathBuf> {
        let base = directories::BaseDirs::new().ok_or_else(|| io::Error::other("cartella home non trovata"))?;
        Ok(base.home_dir().join("Library/LaunchAgents/io.github.keyboard-hud.plist"))
    }

    pub fn location() -> String {
        path().map(|p| p.display().to_string()).unwrap_or_default()
    }

    pub fn expected(exe: &Path, args: &[String]) -> String {
        super::launch_agent(exe, args)
    }

    pub fn read() -> io::Result<Option<String>> {
        file::read(&path()?)
    }

    pub fn write(text: &str) -> io::Result<()> {
        file::write(&path()?, text)
    }

    pub fn remove() -> io::Result<()> {
        file::remove(&path()?)
    }
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::path::Path;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
    };

    const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "Keyboard HUD";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn check(code: u32) -> io::Result<()> {
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(code as i32))
        }
    }

    pub fn location() -> String {
        format!(r"HKEY_CURRENT_USER\{KEY}\{VALUE}")
    }

    pub fn expected(exe: &Path, args: &[String]) -> String {
        super::windows_command(exe, args)
    }

    pub fn read() -> io::Result<Option<String>> {
        let (key, value) = (wide(KEY), wide(VALUE));
        let mut buf = vec![0u16; 2048];
        let mut bytes = (buf.len() * 2) as u32;
        // SAFETY: valid NUL-terminated strings; the buffer size is passed in bytes
        let code = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(code)?;
        let len = (bytes as usize / 2).min(buf.len());
        let text = String::from_utf16_lossy(&buf[..len]);
        Ok(Some(text.trim_end_matches('\0').to_string()))
    }

    pub fn write(text: &str) -> io::Result<()> {
        let (key, value, data) = (wide(KEY), wide(VALUE), wide(text));
        // SAFETY: valid NUL-terminated strings; `data` length in bytes includes the NUL
        check(unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        })
    }

    pub fn remove() -> io::Result<()> {
        let (key, value) = (wide(KEY), wide(VALUE));
        // SAFETY: valid NUL-terminated strings
        let code = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn args() -> Vec<String> {
        vec!["--x11".into(), "--vid".into(), "0x4653".into()]
    }

    #[test]
    fn windows_command_quotes_the_path() {
        let exe = PathBuf::from(r"C:\Program Files\kbd_hud\keyboard-hud.exe");
        assert_eq!(windows_command(&exe, &[]), r#""C:\Program Files\kbd_hud\keyboard-hud.exe""#);
        assert_eq!(windows_command(&exe, &args()[1..]), r#""C:\Program Files\kbd_hud\keyboard-hud.exe" --vid 0x4653"#);
    }

    #[test]
    fn desktop_entry_escapes_exec() {
        let e = desktop_entry(Path::new("/home/me/My Apps/keyboard-hud"), &args());
        assert!(e.contains("\nExec=\"/home/me/My Apps/keyboard-hud\" \"--x11\" \"--vid\" \"0x4653\"\n"), "{e}");
        let e = desktop_entry(Path::new("/opt/a$b\"c%d/hud"), &[]);
        assert!(e.contains(r#"Exec="/opt/a\\$b\\"c%%d/hud""#), "{e}");
    }

    #[test]
    fn launch_agent_lists_arguments() {
        let p = launch_agent(Path::new("/Applications/Keyboard HUD.app/Contents/MacOS/keyboard-hud"), &args()[..1]);
        assert!(p.contains("<string>/Applications/Keyboard HUD.app/Contents/MacOS/keyboard-hud</string>"));
        assert!(p.contains("<string>--x11</string>"));
        assert!(p.contains("<key>RunAtLoad</key>"));
        assert!(launch_agent(Path::new("/a&b/<x>"), &[]).contains("<string>/a&amp;b/&lt;x&gt;</string>"));
    }

    #[cfg(not(windows))]
    #[test]
    fn file_entries_round_trip() {
        let dir = std::env::temp_dir().join(format!("keyboard-hud-autostart-{}", std::process::id()));
        let path = dir.join("sub/entry");
        assert_eq!(file::read(&path).unwrap(), None);
        file::write(&path, "x").unwrap();
        assert_eq!(file::read(&path).unwrap().as_deref(), Some("x"));
        file::remove(&path).unwrap();
        file::remove(&path).unwrap(); // already gone: fine
        assert_eq!(file::read(&path).unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
