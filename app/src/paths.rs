//! Where configuration, cache, statistics and logs live.
//!
//! Portable mode: if `keyboard-hud.toml` exists next to the executable, or
//! that folder is writable, everything stays next to the executable (works
//! from a USB stick). Otherwise the user configuration folder is used.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const CONFIG_FILE: &str = "keyboard-hud.toml";

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".keyboard-hud-write-test");
    let ok = fs::write(&probe, b"").is_ok();
    let _ = fs::remove_file(&probe);
    ok
}

fn user_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "keyboard-hud")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Base folder for all the app files.
pub fn base_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| match exe_dir() {
        Some(dir) if dir.join(CONFIG_FILE).exists() || is_writable(&dir) => dir,
        _ => user_dir(),
    })
}

pub fn cache_dir() -> PathBuf {
    base_dir().join("cache")
}

/// Cached Vial definition (decompressed JSON), keyed by keyboard UID and
/// compressed size: a firmware with a different `vial.json` has a different size.
pub fn definition_cache(uid: u64, size: u32) -> PathBuf {
    cache_dir().join(format!("vial-{uid:016x}-{size}.json"))
}
