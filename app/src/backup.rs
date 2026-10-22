//! Automatic backups of the keyboard configuration as `.vil` files, written
//! when it differs from the last backup of the same keyboard (by UID).
//!
//! Files are named `<keyboard>_<YYYY-MM-DDTHHMMSSZ>.vil` (UTC); only files
//! with that shape are ever pruned.

use crate::clock;
use serde_json::Value;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub dir: PathBuf,
    /// Backups kept per keyboard; older ones are deleted.
    pub keep: usize,
}

/// Opens `dir` in the file manager (created if missing).
pub fn open_folder(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program).arg(dir).spawn().map(|_| ())
}

/// `Corne v4` → `corne-v4`.
pub fn slug(name: &str) -> String {
    let s: String =
        name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() {
        "keyboard".into()
    } else {
        s
    }
}

/// A backup file: its stamp (sortable) and UID.
struct Entry {
    path: PathBuf,
    stamp: String,
    content: Value,
}

fn stamp_of(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    if path.extension()? != "vil" {
        return None;
    }
    let (_, stamp) = stem.rsplit_once('_')?;
    // YYYY-MM-DDTHHMMSSZ, then "-N" for more backups in the same second
    let (core, n) = match stamp.split_once("Z-") {
        Some((core, n)) => (format!("{core}Z"), n),
        None => (stamp.to_string(), "1"),
    };
    let ok = core.len() == 18
        && core.ends_with('Z')
        && core.as_bytes()[10] == b'T'
        && !n.is_empty()
        && n.bytes().all(|b| b.is_ascii_digit());
    ok.then(|| stamp.to_string())
}

/// Backups of the keyboard `uid`, oldest first.
fn entries(dir: &Path, uid: u64) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Entry> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|path| {
            let stamp = stamp_of(&path)?;
            let content: Value = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
            (content.get("uid").and_then(Value::as_u64) == Some(uid)).then_some(Entry { path, stamp, content })
        })
        .collect();
    out.sort_by(|a, b| a.stamp.cmp(&b.stamp).then(a.path.cmp(&b.path)));
    out
}

/// Last backup of `uid`: path and content.
pub fn latest(dir: &Path, uid: u64) -> Option<(PathBuf, Value)> {
    entries(dir, uid).pop().map(|e| (e.path, e.content))
}

/// Writes `vil` unless the last backup of the same keyboard is identical.
/// Returns the new file and the previous backup's content.
pub fn save_if_changed(
    s: &Settings,
    keyboard: &str,
    vil: &Value,
    now_secs: u64,
) -> io::Result<Option<(PathBuf, Option<Value>)>> {
    let uid = vil.get("uid").and_then(Value::as_u64).unwrap_or(0);
    let previous = latest(&s.dir, uid);
    if previous.as_ref().is_some_and(|(_, c)| c == vil) {
        return Ok(None);
    }
    std::fs::create_dir_all(&s.dir)?;
    let mut path = s.dir.join(format!("{}_{}.vil", slug(keyboard), clock::file_stamp(now_secs)));
    // two backups in the same second: keep both
    let mut n = 1;
    while path.exists() {
        n += 1;
        path = s.dir.join(format!("{}_{}-{n}.vil", slug(keyboard), clock::file_stamp(now_secs)));
    }
    let text = serde_json::to_string_pretty(vil).map_err(io::Error::other)?;
    std::fs::write(&path, text)?;
    prune(s, uid);
    Ok(Some((path, previous.map(|(_, c)| c))))
}

fn prune(s: &Settings, uid: u64) {
    let all = entries(&s.dir, uid);
    let extra = all.len().saturating_sub(s.keep.max(1));
    for e in &all[..extra] {
        if let Err(err) = std::fs::remove_file(&e.path) {
            log::warn!("{}: {err}", e.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("keyboard-hud-backup-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn names() {
        assert_eq!(slug("Corne v4"), "corne-v4");
        assert_eq!(slug("  ** "), "keyboard");
        assert_eq!(clock::file_stamp(1_792_656_930), "2026-10-22T081530Z");
        assert_eq!(stamp_of(Path::new("/x/corne-v4_2026-10-22T081530Z.vil")).as_deref(), Some("2026-10-22T081530Z"));
        assert!(stamp_of(Path::new("/x/corne-v4_2026-10-22T081530Z-2.vil")).is_some());
        assert_eq!(stamp_of(Path::new("/x/corne_2026-10-22T081530Z-x.vil")), None);
        assert_eq!(stamp_of(Path::new("/x/corne.vil")), None);
        assert_eq!(stamp_of(Path::new("/x/corne_2026-10-22T081530Z.json")), None);
    }

    #[test]
    fn saves_only_changes_and_prunes() {
        let dir = temp_dir("save");
        let s = Settings { dir: dir.clone(), keep: 2 };
        let v1 = json!({"uid": 7, "layout": [1]});
        let v2 = json!({"uid": 7, "layout": [2]});
        let other = json!({"uid": 8, "layout": [1]});

        let (p1, prev) = save_if_changed(&s, "Corne v4", &v1, 1000).unwrap().unwrap();
        assert!(prev.is_none());
        assert!(p1.file_name().unwrap().to_str().unwrap().starts_with("corne-v4_1970-01-01T001640Z"));
        // unchanged: nothing written
        assert!(save_if_changed(&s, "Corne v4", &v1, 2000).unwrap().is_none());
        // changed: written, with the previous content
        let (_, prev) = save_if_changed(&s, "Corne v4", &v2, 3000).unwrap().unwrap();
        assert_eq!(prev, Some(v1.clone()));
        // another keyboard has its own history
        assert!(save_if_changed(&s, "Corne v4", &other, 3000).unwrap().is_some());
        // same second: a second file, then pruning keeps the last two of uid 7
        save_if_changed(&s, "Corne v4", &v1, 3000).unwrap().unwrap();
        let kept = entries(&dir, 7);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[1].content, v1);
        assert_eq!(latest(&dir, 8).map(|(_, c)| c), Some(other));
        // files that are not ours are left alone
        std::fs::write(dir.join("mine.vil"), "{}").unwrap();
        save_if_changed(&s, "Corne v4", &v2, 4000).unwrap().unwrap();
        assert!(dir.join("mine.vil").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
