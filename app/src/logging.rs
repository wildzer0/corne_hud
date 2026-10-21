//! Minimal logger to stderr. File output with rotation comes with the
//! configuration (Milestone 4).

use log::{Level, LevelFilter, Metadata, Record};

struct StderrLogger {
    level: LevelFilter,
}

impl log::Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level && metadata.target().starts_with("keyboard_hud")
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let tag = match record.level() {
                Level::Error => "ERRORE",
                Level::Warn => "AVVISO",
                Level::Info => "info",
                Level::Debug | Level::Trace => "debug",
            };
            eprintln!("[{tag}] {}", record.args());
        }
    }

    fn flush(&self) {}
}

pub fn init(verbose: bool) {
    let level = if verbose { LevelFilter::Debug } else { LevelFilter::Info };
    if log::set_boxed_logger(Box::new(StderrLogger { level })).is_ok() {
        log::set_max_level(level);
    }
}
