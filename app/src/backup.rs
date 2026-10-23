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

/// File with the history of changes, newest first.
pub const CHANGELOG: &str = "CHANGELOG.md";

/// Adds an entry for the backup `file` at the top of the history in `dir`.
pub fn append_changelog(dir: &Path, keyboard: &str, file: &Path, now_secs: u64, lines: &[String]) -> io::Result<()> {
    let path = dir.join(CHANGELOG);
    let title = crate::i18n::pick(
        "# Storico della configurazione della tastiera\n\nOgni voce corrisponde a un backup `.vil` di questa cartella (ora UTC), dal più recente.\n",
        "# Keyboard configuration history\n\nEach entry matches a `.vil` backup in this folder (UTC time), newest first.\n",
    );
    let old = std::fs::read_to_string(&path).unwrap_or_else(|_| title.to_string());
    let (head, rest) = match old.find("\n## ") {
        Some(i) => old.split_at(i + 1),
        None => (old.as_str(), ""),
    };
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut entry = format!("\n## {} · {keyboard} · `{name}`\n\n", clock::timestamp_at(now_secs));
    for l in lines {
        entry.push_str(&format!("- {l}\n"));
    }
    std::fs::write(
        &path,
        format!(
            "{}{entry}{}{rest}",
            head.trim_end_matches('\n').to_string() + "\n",
            if rest.is_empty() { "" } else { "\n" }
        ),
    )
}

/// Opens `dir` in the file manager (created if missing).
pub fn open_folder(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    open_path(dir)
}

/// Opens a folder in the file manager, or a file with the program the system
/// associates to it (Windows asks which one when there is none).
pub fn open_path(path: &Path) -> io::Result<()> {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program).arg(path).spawn().map(|_| ())
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

/// Every backup in `dir`, oldest first.
fn read_all(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Entry> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|path| {
            let stamp = stamp_of(&path)?;
            let content: Value = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
            Some(Entry { path, stamp, content })
        })
        .collect();
    out.sort_by(|a, b| a.stamp.cmp(&b.stamp).then(a.path.cmp(&b.path)));
    out
}

/// Backups of the keyboard `uid`, oldest first.
fn entries(dir: &Path, uid: u64) -> Vec<Entry> {
    let mut all = read_all(dir);
    all.retain(|e| e.content.get("uid").and_then(Value::as_u64) == Some(uid));
    all
}

/// A backup on disk, for the history window.
#[derive(Debug, Clone)]
pub struct BackupFile {
    pub path: PathBuf,
    /// When it was written, e.g. "2026-10-22 08:15:30 UTC".
    pub when: String,
    pub uid: Option<u64>,
    pub content: Value,
}

/// Every backup in `dir`, newest first.
pub fn history(dir: &Path) -> Vec<BackupFile> {
    let mut out: Vec<BackupFile> = read_all(dir)
        .into_iter()
        .map(|e| BackupFile {
            when: stamp_display(&e.stamp),
            uid: e.content.get("uid").and_then(Value::as_u64),
            path: e.path,
            content: e.content,
        })
        .collect();
    out.reverse();
    out
}

/// `2026-10-22T081530Z-2` → `2026-10-22 08:15:30 UTC (2)`.
fn stamp_display(stamp: &str) -> String {
    let (core, n) = stamp.split_once("Z-").map_or((stamp.trim_end_matches('Z'), None), |(c, n)| (c, Some(n)));
    let t = |a: usize, b: usize| core.get(a..b).unwrap_or("");
    let base = format!("{} {}:{}:{} UTC", t(0, 10), t(11, 13), t(13, 15), t(15, 17));
    match n {
        Some(n) => format!("{base} ({n})"),
        None => base,
    }
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
    fn changelog_newest_first() {
        let dir = temp_dir("log");
        std::fs::create_dir_all(&dir).unwrap();
        let f = |n: &str| PathBuf::from(format!("corne-v4_{n}.vil"));
        append_changelog(&dir, "Corne v4", &f("a"), 0, &["Primo backup".into()]).unwrap();
        append_changelog(&dir, "Corne v4", &f("b"), 60, &["uno".into(), "due".into()]).unwrap();
        let text = std::fs::read_to_string(dir.join(CHANGELOG)).unwrap();
        let b = text.find("`corne-v4_b.vil`").unwrap();
        let a = text.find("`corne-v4_a.vil`").unwrap();
        assert!(text.starts_with("# Storico"));
        assert!(b < a, "{text}");
        assert!(
            text.contains("## 1970-01-01 00:01:00 UTC · Corne v4 · `corne-v4_b.vil`\n\n- uno\n- due\n\n## "),
            "{text}"
        );
        let _ = std::fs::remove_dir_all(&dir);
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

    #[test]
    fn history_lists_every_keyboard_newest_first() {
        let dir = std::env::temp_dir().join(format!("keyboard-hud-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let settings = Settings { dir: dir.clone(), keep: 10 };
        save_if_changed(&settings, "Corne", &json!({"uid": 1, "n": 1}), 1_000).unwrap();
        save_if_changed(&settings, "Corne", &json!({"uid": 1, "n": 2}), 2_000).unwrap();
        save_if_changed(&settings, "Other", &json!({"uid": 2, "n": 1}), 2_000).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a backup").unwrap();
        let h = history(&dir);
        assert_eq!(h.len(), 3);
        assert!(
            h[0].when >= h[1].when && h[1].when >= h[2].when,
            "{:?}",
            h.iter().map(|b| &b.when).collect::<Vec<_>>()
        );
        assert_eq!(h.last().map(|b| b.uid), Some(Some(1)));
        assert_eq!(stamp_display("2026-10-22T081530Z"), "2026-10-22 08:15:30 UTC");
        assert_eq!(stamp_display("2026-10-22T081530Z-2"), "2026-10-22 08:15:30 UTC (2)");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
