//! Usage statistics. Privacy first: only aggregate counters per day, layer
//! and physical key position. No text, no sequences, no timing between keys.

use crate::clock;
use crate::model::Keymap;
use crate::vial::keycode::{qmk_name, KC_NO, KC_TRNS};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const FORMAT_VERSION: u32 = 1;
const SAVE_EVERY: Duration = Duration::from_secs(30);
/// Daily buckets older than this are dropped.
const KEEP_DAYS: i64 = 400;

/// On-disk format: `days["YYYY-MM-DD"]["layer,row,col"] = presses`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct StatsFile {
    version: u32,
    days: BTreeMap<String, BTreeMap<String, u64>>,
}

pub type Position = (u8, u8, u8);

/// Presses per (layer, row, col).
pub type Totals = HashMap<Position, u64>;

fn key_of((l, r, c): Position) -> String {
    format!("{l},{r},{c}")
}

fn parse_key(s: &str) -> Option<Position> {
    let mut it = s.split(',').map(|p| p.trim().parse::<u8>().ok());
    let pos = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(pos)
}

pub struct Stats {
    data: StatsFile,
    path: PathBuf,
    dirty_since: Option<Instant>,
}

impl Stats {
    /// Loads the counters; a corrupt file is renamed to `.bad` and replaced.
    pub fn load(path: PathBuf) -> Self {
        let data = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<StatsFile>(&text) {
                Ok(d) if d.version == FORMAT_VERSION => d,
                Ok(_) | Err(_) => {
                    let mut bad = path.clone().into_os_string();
                    bad.push(".bad");
                    log::warn!("statistiche non leggibili: salvate in {}", PathBuf::from(&bad).display());
                    let _ = std::fs::rename(&path, bad);
                    StatsFile { version: FORMAT_VERSION, ..Default::default() }
                }
            },
            Err(_) => StatsFile { version: FORMAT_VERSION, ..Default::default() },
        };
        Stats { data, path, dirty_since: None }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn record(&mut self, pos: Position, day: i64) {
        let bucket = self.data.days.entry(clock::date_string(day)).or_default();
        *bucket.entry(key_of(pos)).or_insert(0) += 1;
        self.dirty_since.get_or_insert_with(Instant::now);
    }

    pub fn reset(&mut self) {
        self.data.days.clear();
        self.dirty_since = Some(Instant::now());
        self.save();
    }

    /// Saves at most every 30 s while there are new presses.
    pub fn save_if_due(&mut self) {
        if self.dirty_since.is_some_and(|t| t.elapsed() >= SAVE_EVERY) {
            self.save();
        }
    }

    pub fn save(&mut self) {
        if self.dirty_since.is_none() {
            return;
        }
        let oldest = clock::date_string(clock::today() - KEEP_DAYS);
        self.data.days.retain(|day, _| *day >= oldest);
        let result = serde_json::to_string(&self.data).map_err(|e| e.to_string()).and_then(|text| {
            if let Some(dir) = self.path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            // write then rename: a crash never leaves a truncated file
            let tmp = self.path.with_extension("json.tmp");
            std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &self.path)).map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => self.dirty_since = None,
            Err(e) => log::warn!("{}: {e}", self.path.display()),
        }
    }

    /// Presses per position since `from_day` (inclusive), or over all days.
    pub fn totals(&self, from_day: Option<i64>) -> Totals {
        let from = from_day.map(clock::date_string);
        let mut out = HashMap::new();
        for (day, bucket) in &self.data.days {
            if from.as_ref().is_some_and(|f| day < f) {
                continue;
            }
            for (k, n) in bucket {
                if let Some(pos) = parse_key(k) {
                    *out.entry(pos).or_insert(0) += n;
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finger {
    Pinky,
    Ring,
    Middle,
    Index,
    Thumb,
}

pub const FINGERS: [Finger; 5] = [Finger::Pinky, Finger::Ring, Finger::Middle, Finger::Index, Finger::Thumb];

impl Finger {
    pub fn name(self) -> &'static str {
        match self {
            Finger::Pinky => "mignolo",
            Finger::Ring => "anulare",
            Finger::Middle => "medio",
            Finger::Index => "indice",
            Finger::Thumb => "pollice",
        }
    }
}

/// Hand and finger of a matrix position, for the Corne matrix: rows
/// 0..rows/2 left half, the last row of each half are the thumbs; columns
/// from the outside: 0-1 pinky, 2 ring, 3 middle, 4-6 index.
pub fn finger_of(row: u8, col: u8, rows: u8) -> (Hand, Finger) {
    let half = (rows / 2).max(1);
    let hand = if row < half { Hand::Left } else { Hand::Right };
    let finger = if row % half == half - 1 {
        Finger::Thumb
    } else {
        match col {
            0 | 1 => Finger::Pinky,
            2 => Finger::Ring,
            3 => Finger::Middle,
            _ => Finger::Index,
        }
    };
    (hand, finger)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub days: u32,
    /// Presses in the period.
    pub total: u64,
    pub all_time: u64,
    pub per_layer: Vec<u64>,
    /// [left, right]
    pub hands: [u64; 2],
    /// `[hand][finger]`, fingers in [`FINGERS`] order.
    pub fingers: [[u64; 5]; 2],
    /// Keys with a function that were never pressed in the period.
    pub unused: Vec<(Position, u16)>,
}

/// Builds the report for the last `days` days. `positions` are the matrix
/// positions that exist on the keyboard (from the geometry).
pub fn report(stats: &Stats, keymap: &Keymap, positions: &[(u8, u8)], days: u32, today: i64) -> Report {
    let recent = stats.totals(Some(today - days as i64 + 1));
    let all = stats.totals(None);
    let mut r = Report {
        days,
        per_layer: vec![0; keymap.layers() as usize],
        all_time: all.values().sum(),
        ..Default::default()
    };
    for (&(l, row, col), &n) in &recent {
        r.total += n;
        if let Some(v) = r.per_layer.get_mut(l as usize) {
            *v += n;
        }
        let (hand, finger) = finger_of(row, col, keymap.rows());
        let h = if hand == Hand::Left { 0 } else { 1 };
        r.hands[h] += n;
        let f = FINGERS.iter().position(|x| *x == finger).unwrap_or(0);
        r.fingers[h][f] += n;
    }
    for layer in 0..keymap.layers() {
        for &(row, col) in positions {
            let code = keymap.get(layer, row, col);
            if code != KC_NO && code != KC_TRNS && !recent.contains_key(&(layer, row, col)) {
                r.unused.push(((layer, row, col), code));
            }
        }
    }
    r.unused.sort();
    r
}

/// CSV export: one line per layer and key, all-time and recent counts.
pub fn csv(
    stats: &Stats,
    keymap: &Keymap,
    positions: &[(u8, u8)],
    layer_names: &[String],
    days: u32,
    today: i64,
) -> String {
    let recent = stats.totals(Some(today - days as i64 + 1));
    let all = stats.totals(None);
    let mut out =
        format!("layer,nome_layer,riga,colonna,mano,dito,keycode,pressioni_totali,pressioni_ultimi_{days}_giorni\n");
    for layer in 0..keymap.layers() {
        let name = layer_names.get(layer as usize).cloned().unwrap_or_default();
        for &(row, col) in positions {
            let (hand, finger) = finger_of(row, col, keymap.rows());
            let hand = if hand == Hand::Left { "sinistra" } else { "destra" };
            let pos = (layer, row, col);
            out.push_str(&format!(
                "{layer},{name},{row},{col},{hand},{},\"{}\",{},{}\n",
                finger.name(),
                qmk_name(keymap.get(layer, row, col)).replace('"', "\"\""),
                all.get(&pos).copied().unwrap_or(0),
                recent.get(&pos).copied().unwrap_or(0),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hud-stats-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("stats.json")
    }

    #[test]
    fn records_saves_and_reloads() {
        let path = temp("roundtrip");
        let today = clock::today();
        let mut s = Stats::load(path.clone());
        s.record((0, 1, 1), today);
        s.record((0, 1, 1), today);
        s.record((1, 5, 4), today - 10);
        s.save();
        let s = Stats::load(path.clone());
        assert_eq!(s.totals(None).get(&(0, 1, 1)), Some(&2));
        assert_eq!(s.totals(Some(today - 5)).get(&(1, 5, 4)), None);
        assert_eq!(s.totals(Some(today - 10)).get(&(1, 5, 4)), Some(&1));
        // the file holds counters only
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"0,1,1\":2"), "{text}");
    }

    #[test]
    fn corrupt_file_is_set_aside() {
        let path = temp("corrupt");
        std::fs::write(&path, "{ not json").unwrap();
        let s = Stats::load(path.clone());
        assert!(s.totals(None).is_empty());
        assert!(path.with_extension("json.bad").exists());
    }

    #[test]
    fn reset_clears_everything() {
        let path = temp("reset");
        let mut s = Stats::load(path.clone());
        s.record((0, 0, 0), clock::today());
        s.reset();
        assert!(Stats::load(path).totals(None).is_empty());
    }

    #[test]
    fn fingers_on_the_corne() {
        assert_eq!(finger_of(1, 1, 8), (Hand::Left, Finger::Pinky));
        assert_eq!(finger_of(1, 4, 8), (Hand::Left, Finger::Index));
        assert_eq!(finger_of(3, 5, 8), (Hand::Left, Finger::Thumb));
        assert_eq!(finger_of(5, 3, 8), (Hand::Right, Finger::Middle));
        assert_eq!(finger_of(7, 3, 8), (Hand::Right, Finger::Thumb));
        assert_eq!(finger_of(4, 2, 8), (Hand::Right, Finger::Ring));
    }

    #[test]
    fn report_and_csv() {
        let path = temp("report");
        let today = clock::today();
        let mut km = Keymap::new(2, 8, 7);
        km.set(0, 1, 1, 0x04); // A
        km.set(0, 5, 4, 0x0D); // J
        km.set(1, 1, 1, 0x01); // TRNS on layer 1: never "unused"
        km.set(1, 5, 4, 0x1E); // 1
        let positions = [(1, 1), (5, 4), (0, 0)];
        let mut s = Stats::load(path);
        for _ in 0..3 {
            s.record((0, 1, 1), today);
        }
        s.record((1, 5, 4), today - 100); // outside the 30-day window
        let r = report(&s, &km, &positions, 30, today);
        assert_eq!((r.total, r.all_time), (3, 4));
        assert_eq!(r.per_layer, vec![3, 0]);
        assert_eq!(r.hands, [3, 0]);
        assert_eq!(r.fingers[0][0], 3);
        assert_eq!(r.unused, vec![((0, 5, 4), 0x0D), ((1, 5, 4), 0x1E)]);
        let names = vec!["BASE".to_string(), "SYM".to_string()];
        let text = csv(&s, &km, &positions, &names, 30, today);
        assert!(text.starts_with("layer,nome_layer"));
        assert!(text.contains("0,BASE,1,1,sinistra,mignolo,\"KC_A\",3,3"), "{text}");
        assert!(text.contains("1,SYM,5,4,destra,indice,\"KC_1\",1,0"), "{text}");
    }

    #[test]
    fn malformed_keys_are_ignored() {
        assert_eq!(parse_key("1,2,3"), Some((1, 2, 3)));
        for bad in ["", "1,2", "1,2,3,4", "a,b,c", "300,1,1"] {
            assert_eq!(parse_key(bad), None, "{bad}");
        }
    }
}
