//! Keyboard Layout Editor (KLE) parser, as used by Vial's `layouts.keymap`.
//!
//! Semantics follow vial-gui's `kle_serial.py` (a port of `kle-serial`):
//! positions are in key units, rotation is around `(rx, ry)` in degrees, and
//! `rx`/`ry` reset the cursor to the rotation origin ("cluster").

use serde_json::Value;
use std::fmt;

/// One physical key (or encoder) of the layout.
#[derive(Debug, Clone, PartialEq)]
pub struct KleKey {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Rotation angle in degrees (clockwise, as in KLE).
    pub r: f32,
    pub rx: f32,
    pub ry: f32,
    pub decal: bool,
    /// Labels after alignment reordering (12 slots, as in kle-serial).
    pub labels: [Option<String>; 12],
}

/// What a KLE key means for Vial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRole {
    Matrix { row: u8, col: u8 },
    Encoder { index: u8, direction: u8 },
}

impl KleKey {
    /// Vial meaning of the key: matrix position (label 0 `"r,c"`) or encoder
    /// (label 4 `"e"`, label 0 `"index,direction"`). `None` for decals and
    /// keys without a valid position.
    pub fn role(&self) -> Option<KeyRole> {
        if self.decal {
            return None;
        }
        let (a, b) = parse_pair(self.labels[0].as_deref()?)?;
        if self.labels[4].as_deref() == Some("e") {
            Some(KeyRole::Encoder { index: a, direction: b })
        } else {
            Some(KeyRole::Matrix { row: a, col: b })
        }
    }

    /// Layout option `(group, choice)` from label 8, if any.
    pub fn layout_option(&self) -> Option<(u8, u8)> {
        parse_pair(self.labels[8].as_deref()?)
    }

    /// The four corners in key units, after rotation (clockwise from top-left).
    pub fn corners(&self) -> [(f32, f32); 4] {
        let pts = [
            (self.x, self.y),
            (self.x + self.w, self.y),
            (self.x + self.w, self.y + self.h),
            (self.x, self.y + self.h),
        ];
        pts.map(|p| self.rotate(p))
    }

    /// Center of the key in key units, after rotation.
    pub fn center(&self) -> (f32, f32) {
        self.rotate((self.x + self.w / 2.0, self.y + self.h / 2.0))
    }

    fn rotate(&self, (px, py): (f32, f32)) -> (f32, f32) {
        if self.r == 0.0 {
            return (px, py);
        }
        let (s, c) = self.r.to_radians().sin_cos();
        let (dx, dy) = (px - self.rx, py - self.ry);
        (self.rx + dx * c - dy * s, self.ry + dx * s + dy * c)
    }
}

fn parse_pair(s: &str) -> Option<(u8, u8)> {
    let (a, b) = s.split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

#[derive(Debug, Clone, PartialEq)]
pub enum KleError {
    NotAnArray,
    RotationNotFirst { row: usize },
    BadValue { row: usize, field: &'static str },
}

impl fmt::Display for KleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KleError::NotAnArray => write!(f, "KLE layout is not an array"),
            KleError::RotationNotFirst { row } => {
                write!(f, "KLE row {row}: rotation can only be specified on the first key in a row")
            }
            KleError::BadValue { row, field } => write!(f, "KLE row {row}: invalid value for '{field}'"),
        }
    }
}

impl std::error::Error for KleError {}

const LABEL_MAP: [[i8; 12]; 8] = [
    [0, 6, 2, 8, 9, 11, 3, 5, 1, 4, 7, 10],
    [1, 7, -1, -1, 9, 11, 4, -1, -1, -1, -1, 10],
    [3, -1, 5, -1, 9, 11, -1, -1, 4, -1, -1, 10],
    [4, -1, -1, -1, 9, 11, -1, -1, -1, -1, -1, 10],
    [0, 6, 2, 8, 10, -1, 3, 5, 1, 4, 7, -1],
    [1, 7, -1, -1, 10, -1, 4, -1, -1, -1, -1, -1],
    [3, -1, 5, -1, 10, -1, -1, -1, 4, -1, -1, -1],
    [4, -1, -1, -1, 10, -1, -1, -1, -1, -1, -1, -1],
];

fn reorder_labels(raw: &str, align: usize) -> [Option<String>; 12] {
    let mut out: [Option<String>; 12] = Default::default();
    let map = &LABEL_MAP[align.min(7)];
    for (i, label) in raw.split('\n').enumerate().take(12) {
        if !label.is_empty() && map[i] >= 0 {
            out[map[i] as usize] = Some(label.to_string());
        }
    }
    out
}

#[derive(Clone)]
struct Cursor {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    rx: f32,
    ry: f32,
    decal: bool,
}

/// Parses a KLE keymap array (the value of `layouts.keymap` in `vial.json`).
pub fn parse(layout: &Value) -> Result<Vec<KleKey>, KleError> {
    let rows = layout.as_array().ok_or(KleError::NotAnArray)?;
    let mut cur = Cursor { x: 0.0, y: 0.0, w: 1.0, h: 1.0, r: 0.0, rx: 0.0, ry: 0.0, decal: false };
    let (mut cluster_x, mut cluster_y) = (0.0f32, 0.0f32);
    let mut align = 4usize;
    let mut keys = Vec::new();

    for (r, row) in rows.iter().enumerate() {
        // A leading object (keyboard metadata) is allowed and ignored.
        let Some(items) = row.as_array() else { continue };
        for (k, item) in items.iter().enumerate() {
            match item {
                Value::String(s) => {
                    keys.push(KleKey {
                        x: cur.x,
                        y: cur.y,
                        w: cur.w,
                        h: cur.h,
                        r: cur.r,
                        rx: cur.rx,
                        ry: cur.ry,
                        decal: cur.decal,
                        labels: reorder_labels(s, align),
                    });
                    cur.x += cur.w;
                    cur.w = 1.0;
                    cur.h = 1.0;
                    cur.decal = false;
                }
                Value::Object(p) => {
                    let num = |field: &'static str| -> Result<Option<f32>, KleError> {
                        match p.get(field) {
                            None => Ok(None),
                            Some(v) => v
                                .as_f64()
                                .filter(|f| f.is_finite())
                                .map(|f| Some(f as f32))
                                .ok_or(KleError::BadValue { row: r, field }),
                        }
                    };
                    if k != 0 && (p.contains_key("r") || p.contains_key("rx") || p.contains_key("ry")) {
                        return Err(KleError::RotationNotFirst { row: r });
                    }
                    if let Some(v) = num("r")? {
                        cur.r = v;
                    }
                    if let Some(v) = num("rx")? {
                        cur.rx = v;
                        cluster_x = v;
                        cur.x = cluster_x;
                        cur.y = cluster_y;
                    }
                    if let Some(v) = num("ry")? {
                        cur.ry = v;
                        cluster_y = v;
                        cur.x = cluster_x;
                        cur.y = cluster_y;
                    }
                    if let Some(v) = num("a")? {
                        align = (v.max(0.0) as usize).min(7);
                    }
                    if let Some(v) = num("x")? {
                        cur.x += v;
                    }
                    if let Some(v) = num("y")? {
                        cur.y += v;
                    }
                    if let Some(v) = num("w")? {
                        cur.w = v;
                    }
                    if let Some(v) = num("h")? {
                        cur.h = v;
                    }
                    if let Some(d) = p.get("d") {
                        cur.decal = d.as_bool().unwrap_or(false);
                    }
                }
                _ => return Err(KleError::BadValue { row: r, field: "item" }),
            }
        }
        cur.y += 1.0;
        cur.x = cur.rx;
    }
    Ok(keys)
}

/// Bit width of each layout option group, from `layouts.labels`:
/// a string is a boolean (1 bit), a list `[name, choice...]` is a select.
pub fn option_widths(labels: &Value) -> Vec<u32> {
    labels
        .as_array()
        .map(|groups| {
            groups
                .iter()
                .map(|g| match g.as_array() {
                    Some(list) => {
                        let choices = list.len().saturating_sub(1).max(1);
                        usize::BITS - (choices - 1).leading_zeros()
                    }
                    None => 1,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Decodes the VIA layout options value into one choice per group.
/// VIA packs the groups so that the last group uses the lowest bits.
pub fn decode_options(value: u32, widths: &[u32]) -> Vec<u8> {
    let mut v = value;
    let mut out = vec![0u8; widths.len()];
    for (i, w) in widths.iter().enumerate().rev() {
        let w = (*w).min(8);
        out[i] = (v & ((1u32 << w) - 1)) as u8;
        v = v.checked_shr(w).unwrap_or(0);
    }
    out
}

/// Keeps only the keys visible with the given option choices.
pub fn filter_by_options(keys: &[KleKey], choices: &[u8]) -> Vec<KleKey> {
    keys.iter()
        .filter(|k| match k.layout_option() {
            None => true,
            Some((group, choice)) => choices.get(group as usize).copied().unwrap_or(0) == choice,
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const VIAL_JSON: &str = include_str!("../data/mock/vial.json");

    fn corne_keys() -> Vec<KleKey> {
        let v: Value = serde_json::from_str(VIAL_JSON).unwrap();
        parse(&v["layouts"]["keymap"]).unwrap()
    }

    fn find(keys: &[KleKey], row: u8, col: u8) -> &KleKey {
        keys.iter()
            .find(|k| k.role() == Some(KeyRole::Matrix { row, col }) && k.layout_option().is_none_or(|o| o.1 == 0))
            .unwrap_or_else(|| panic!("key {row},{col} not found"))
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn simple_row_advances_x() {
        let keys = parse(&json!([["a", "b", {"w": 2}, "c"], ["d"]])).unwrap();
        assert_eq!(keys.len(), 4);
        assert_eq!((keys[1].x, keys[1].y), (1.0, 0.0));
        assert_eq!((keys[2].x, keys[2].w), (2.0, 2.0));
        assert_eq!((keys[3].x, keys[3].y, keys[3].w), (0.0, 1.0, 1.0));
    }

    #[test]
    fn rotation_must_be_first() {
        assert_eq!(parse(&json!([["a", {"r": 10}, "b"]])), Err(KleError::RotationNotFirst { row: 0 }));
    }

    #[test]
    fn malformed_values_are_errors_not_panics() {
        assert!(parse(&json!({"a": 1})).is_err());
        assert!(parse(&json!([["a", {"x": "nope"}]])).is_err());
        assert!(parse(&json!([[1]])).is_err());
        assert!(parse(&json!([{"name": "meta"}, ["a"]])).is_ok());
    }

    #[test]
    fn rotation_resets_to_cluster_and_inherits_ry() {
        let keys = parse(&json!([
            [{"r": 30, "rx": 5.0, "ry": 9.0, "y": -1.0, "x": 1.0}, "a"],
            [{"r": -30, "rx": 11.0, "x": 0.5}, "b"]
        ]))
        .unwrap();
        assert_eq!((keys[0].x, keys[0].y, keys[0].rx, keys[0].ry), (6.0, 8.0, 5.0, 9.0));
        // second key: rx changes, ry is inherited from the previous cluster
        assert_eq!((keys[1].x, keys[1].y, keys[1].rx, keys[1].ry, keys[1].r), (11.5, 9.0, 11.0, 9.0, -30.0));
    }

    #[test]
    fn corners_rotate_around_origin() {
        let k = KleKey {
            x: 1.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            r: 90.0,
            rx: 0.0,
            ry: 0.0,
            decal: false,
            labels: Default::default(),
        };
        let c = k.corners();
        assert!(approx(c[0].0, 0.0) && approx(c[0].1, 1.0), "{c:?}");
        assert!(approx(c[2].0, -1.0) && approx(c[2].1, 2.0), "{c:?}");
    }

    #[test]
    fn labels_follow_default_alignment() {
        let keys = parse(&json!([["0,6\n\n\n0,1\n\n\n\n\n\ne"]])).unwrap();
        assert_eq!(keys[0].labels[0].as_deref(), Some("0,6"));
        assert_eq!(keys[0].labels[8].as_deref(), Some("0,1"));
        assert_eq!(keys[0].labels[4].as_deref(), Some("e"));
        assert_eq!(keys[0].role(), Some(KeyRole::Encoder { index: 0, direction: 6 }));
        assert_eq!(keys[0].layout_option(), Some((0, 1)));
    }

    #[test]
    fn corne_has_46_keys_with_default_options() {
        let v: Value = serde_json::from_str(VIAL_JSON).unwrap();
        let all = corne_keys();
        let widths = option_widths(&v["layouts"]["labels"]);
        assert_eq!(widths, vec![2, 2, 2, 2]);
        let visible = filter_by_options(&all, &decode_options(0, &widths));
        let matrix: Vec<_> = visible.iter().filter_map(|k| k.role()).collect();
        assert!(matrix.iter().all(|r| matches!(r, KeyRole::Matrix { .. })));
        assert_eq!(matrix.len(), 46);
        let mut uniq = matrix.clone();
        uniq.sort_by_key(|r| format!("{r:?}"));
        uniq.dedup();
        assert_eq!(uniq.len(), 46, "duplicate matrix positions");
    }

    #[test]
    fn corne_encoder_option_replaces_inner_keys() {
        let v: Value = serde_json::from_str(VIAL_JSON).unwrap();
        let widths = option_widths(&v["layouts"]["labels"]);
        // group 0 (Left EX1) = Encoder: choice 1 in the highest 2 bits
        let choices = decode_options(0b01_00_00_00, &widths);
        assert_eq!(choices, vec![1, 0, 0, 0]);
        let visible = filter_by_options(&corne_keys(), &choices);
        let encoders = visible.iter().filter(|k| matches!(k.role(), Some(KeyRole::Encoder { .. }))).count();
        assert_eq!(encoders, 2);
        // the full-size inner key is replaced by the half-height encoder push button
        let inner: Vec<_> = visible.iter().filter(|k| k.role() == Some(KeyRole::Matrix { row: 0, col: 6 })).collect();
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].h, 0.5);
    }

    #[test]
    fn corne_thumbs_are_rotated() {
        let keys = corne_keys();
        let l3 = find(&keys, 3, 3);
        assert_eq!(l3.r, 0.0);
        let l4 = find(&keys, 3, 4);
        assert_eq!((l4.r, l4.rx, l4.ry), (15.0, 4.5, 9.1));
        let l5 = find(&keys, 3, 5);
        assert_eq!((l5.r, l5.rx, l5.ry, l5.h), (30.0, 5.4, 9.3, 1.5));
        // 7,5 declares r and rx only: ry is inherited (9.3)
        let r5 = find(&keys, 7, 5);
        assert_eq!((r5.r, r5.rx, r5.ry, r5.h), (-30.0, 11.1, 9.3, 1.5));
        assert!(approx(r5.x, 11.5) && approx(r5.y, 4.25), "{r5:?}");
        let r4 = find(&keys, 7, 4);
        assert_eq!((r4.r, r4.rx, r4.ry), (-15.0, 12.0, 9.1));
        // halves mirror each other around the middle of the board
        let (lx, ly) = l5.center();
        let (rx, ry) = r5.center();
        assert!(approx(ly, ry), "{ly} vs {ry}");
        assert!(approx(lx + rx, 2.0 * 8.25), "{lx} + {rx}");
    }

    #[test]
    fn corne_right_half_is_mirrored() {
        let keys = corne_keys();
        assert!(find(&keys, 0, 0).x < find(&keys, 0, 5).x);
        assert!(find(&keys, 4, 0).x > find(&keys, 4, 5).x);
        assert!(find(&keys, 0, 3).y < find(&keys, 0, 0).y, "middle finger column is staggered up");
    }
}
