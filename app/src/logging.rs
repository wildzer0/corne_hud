//! Logger: `keyboard-hud.log` next to the config (rotated at 1 MB, one
//! backup `.log.1`) plus stderr. The level can change at runtime when the
//! config is reloaded.

use log::{Level, LevelFilter, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_LOG_SIZE: u64 = 1024 * 1024;

struct LogFile {
    path: PathBuf,
    file: File,
    size: u64,
}

impl LogFile {
    fn open(path: &Path) -> Option<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path).ok()?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Some(LogFile { path: path.to_path_buf(), file, size })
    }

    fn write(&mut self, line: &str) {
        if self.size + line.len() as u64 > MAX_LOG_SIZE {
            let mut backup = self.path.clone().into_os_string();
            backup.push(".1");
            let _ = std::fs::rename(&self.path, backup);
            if let Some(fresh) = LogFile::open(&self.path) {
                *self = fresh;
            }
        }
        if self.file.write_all(line.as_bytes()).is_ok() {
            self.size += line.len() as u64;
        }
    }
}

struct HudLogger {
    file: Mutex<Option<LogFile>>,
}

static LOGGER: HudLogger = HudLogger { file: Mutex::new(None) };

impl log::Log for HudLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // the app at the configured level; libraries (eframe, winit, ...) only warnings and errors
        let own = metadata.target().starts_with("keyboard_hud");
        metadata.level() <= log::max_level() && (own || metadata.level() <= Level::Warn)
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let tag = match record.level() {
            Level::Error => "ERRORE",
            Level::Warn => "AVVISO",
            Level::Info => "info",
            Level::Debug | Level::Trace => "debug",
        };
        let line = format!("{} [{tag}] {}\n", crate::clock::timestamp(), record.args());
        eprint!("{line}");
        if let Ok(mut f) = self.file.lock() {
            if let Some(f) = f.as_mut() {
                f.write(&line);
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file.lock() {
            if let Some(f) = f.as_mut() {
                let _ = f.file.flush();
            }
        }
    }
}

pub fn parse_level(s: &str) -> LevelFilter {
    match s.trim().to_ascii_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" | "warning" => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

pub fn init(path: &Path, level: LevelFilter) {
    if let Ok(mut f) = LOGGER.file.lock() {
        *f = LogFile::open(path);
    }
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(level);
    }
}

pub fn set_level(level: LevelFilter) {
    log::set_max_level(level);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_at_limit() {
        let dir = std::env::temp_dir().join(format!("hud-log-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.log");
        let mut f = LogFile::open(&path).unwrap();
        let line = "x".repeat(1000) + "\n";
        for _ in 0..1100 {
            f.write(&line);
        }
        assert!(std::fs::metadata(&path).unwrap().len() <= MAX_LOG_SIZE);
        assert!(dir.join("t.log.1").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn levels() {
        assert_eq!(parse_level("DEBUG"), LevelFilter::Debug);
        assert_eq!(parse_level("boh"), LevelFilter::Info);
    }
}
