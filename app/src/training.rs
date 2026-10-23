//! Training mode: the HUD asks for a symbol or shortcut ("{", "Ctrl+Z",
//! "F5") and watches the keyboard for it. Keys from the layers above the
//! default one, the least used and the most missed asked more often.
//! Pure logic: the UI feeds it the key presses.

use crate::model::Keymap;
use crate::vial::keycode::{decode, Keycode, Mods5};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Questions in a round.
pub const ROUND: usize = 20;
/// The answer is shown after this long.
pub const HINT_AFTER: Duration = Duration::from_secs(6);

/// One thing to type, and where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub code: u16,
    pub layer: u8,
    pub row: u8,
    pub col: u8,
}

/// Keycodes worth practicing: what types or does something on its own
/// (characters, F-keys, navigation, shortcuts), not modifiers or layer keys.
fn trainable(code: u16) -> bool {
    match decode(code) {
        // letters, digits, punctuation, Enter/Esc/Tab..., F-keys, navigation, keypad, media
        Keycode::Basic(c) => (0x04..=0x73).contains(&c) || (0xA5..=0xBF).contains(&c),
        Keycode::Mods { kc, .. } => (0x04..=0x73).contains(&kc),
        _ => false,
    }
}

/// Every keycode to practice, once each, at its first position: from the
/// layers above `default_layer` (all layers if those have none).
pub fn pool(keymap: &Keymap, present: &[(u8, u8)], default_layer: u8) -> Vec<Target> {
    let collect = |above: bool| {
        let mut out: Vec<Target> = Vec::new();
        for layer in (0..keymap.layers()).filter(|&l| !above || l > default_layer) {
            for &(row, col) in present {
                let code = keymap.get(layer, row, col);
                if trainable(code) && !out.iter().any(|t| t.code == code) {
                    out.push(Target { code, layer, row, col });
                }
            }
        }
        out
    };
    let above = collect(true);
    if above.is_empty() {
        collect(false)
    } else {
        above
    }
}

/// What a key press meant for the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Correct,
    Wrong,
    /// Layer keys, modifiers, the right key without its modifier yet: not an answer.
    Ignored,
}

/// HID modifier byte (bit 0-3 left Ctrl/Shift/Alt/Gui, 4-7 right) holds the
/// modifiers of `m`, on either side.
fn holds(hid: u8, m: Mods5) -> bool {
    let either = (hid | (hid >> 4)) & 0x0F;
    m.0 & 0x0F & !either == 0
}

/// Judges one press: `code` resolved on the active layers, `mods` the HID
/// modifiers held (one-shot included).
pub fn judge(target: u16, code: u16, mods: u8) -> Outcome {
    if code == target {
        return Outcome::Correct;
    }
    if let Keycode::Mods { mods: m, kc } = decode(target) {
        if code == kc as u16 {
            // Shift+9 for "(": right with the modifiers, not an answer without
            return if holds(mods, m) { Outcome::Correct } else { Outcome::Ignored };
        }
    }
    match decode(code) {
        Keycode::Basic(c) if (0xE0..=0xE7).contains(&c) => Outcome::Ignored,
        Keycode::Layer { .. }
        | Keycode::LayerTap { .. }
        | Keycode::LayerMod { .. }
        | Keycode::OneShotMod(_)
        | Keycode::ModTap { .. }
        | Keycode::TapDance(_)
        | Keycode::Custom(_)
        | Keycode::Transparent
        | Keycode::No => Outcome::Ignored,
        _ => Outcome::Wrong,
    }
}

/// How one question went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer {
    pub target: Target,
    pub time: Duration,
    pub wrong: u32,
    /// The answer was shown before the right key came.
    pub hinted: bool,
}

#[derive(Debug, Clone, Copy)]
struct Current {
    target: Target,
    since: Instant,
    wrong: u32,
}

/// A round of questions.
#[derive(Debug, Clone)]
pub struct Session {
    pool: Vec<Target>,
    /// Presses so far per keycode (from the statistics): rare keys come first.
    usage: HashMap<u16, u64>,
    total: usize,
    current: Option<Current>,
    pub answers: Vec<Answer>,
    misses: HashMap<u16, u32>,
    rng: u64,
}

impl Session {
    pub fn new(pool: Vec<Target>, usage: HashMap<u16, u64>, total: usize, seed: u64, now: Instant) -> Self {
        let mut s =
            Session { pool, usage, total, current: None, answers: Vec::new(), misses: HashMap::new(), rng: seed | 1 };
        s.ask(now);
        s
    }

    fn random(&mut self) -> u64 {
        // xorshift64
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }

    /// Weight of a target: rare and missed keys come up more often.
    fn weight(&self, t: &Target) -> u64 {
        let used = self.usage.get(&t.code).copied().unwrap_or(0);
        let rare = match used {
            0..=4 => 4,
            5..=49 => 2,
            _ => 1,
        };
        rare + 3 * self.misses.get(&t.code).copied().unwrap_or(0) as u64
    }

    fn ask(&mut self, now: Instant) {
        if self.answers.len() >= self.total || self.pool.is_empty() {
            self.current = None;
            return;
        }
        let last = self.answers.last().map(|a| a.target.code);
        let choices: Vec<(Target, u64)> = self
            .pool
            .iter()
            .filter(|t| self.pool.len() == 1 || Some(t.code) != last)
            .map(|t| (*t, self.weight(t)))
            .collect();
        let sum: u64 = choices.iter().map(|c| c.1).sum();
        let mut pick = self.random() % sum.max(1);
        let target = choices
            .iter()
            .find(|(_, w)| {
                if pick < *w {
                    true
                } else {
                    pick -= w;
                    false
                }
            })
            .map_or(choices[0].0, |c| c.0);
        self.current = Some(Current { target, since: now, wrong: 0 });
    }

    pub fn current(&self) -> Option<Target> {
        self.current.map(|c| c.target)
    }

    /// Time on the current question.
    pub fn elapsed(&self, now: Instant) -> Duration {
        self.current.map_or(Duration::ZERO, |c| now.duration_since(c.since))
    }

    pub fn hint_due(&self, now: Instant) -> bool {
        self.current.is_some() && self.elapsed(now) >= HINT_AFTER
    }

    /// Wrong presses on the current question.
    pub fn wrong(&self) -> u32 {
        self.current.map_or(0, |c| c.wrong)
    }

    pub fn finished(&self) -> bool {
        self.current.is_none()
    }

    pub fn total(&self) -> usize {
        if self.pool.is_empty() {
            0
        } else {
            self.total
        }
    }

    /// A key press, resolved on the active layers. Moves to the next
    /// question when it is the right one.
    pub fn press(&mut self, code: u16, mods: u8, now: Instant) -> Outcome {
        let Some(cur) = &mut self.current else { return Outcome::Ignored };
        let outcome = judge(cur.target.code, code, mods);
        match outcome {
            Outcome::Correct => {
                let hinted = now.duration_since(cur.since) >= HINT_AFTER;
                let answer =
                    Answer { target: cur.target, time: now.duration_since(cur.since), wrong: cur.wrong, hinted };
                if answer.wrong > 0 || hinted {
                    *self.misses.entry(answer.target.code).or_default() += 1;
                }
                self.answers.push(answer);
                self.ask(now);
            }
            Outcome::Wrong => cur.wrong += 1,
            Outcome::Ignored => {}
        }
        outcome
    }

    /// Skips the current question (counted as hinted, i.e. not known).
    pub fn skip(&mut self, now: Instant) {
        if let Some(cur) = self.current {
            *self.misses.entry(cur.target.code).or_default() += 1;
            self.answers.push(Answer {
                target: cur.target,
                time: now.duration_since(cur.since),
                wrong: cur.wrong,
                hinted: true,
            });
            self.ask(now);
        }
    }

    pub fn summary(&self) -> Summary {
        let n = self.answers.len();
        let clean = self.answers.iter().filter(|a| a.wrong == 0 && !a.hinted).count();
        let time = self.answers.iter().map(|a| a.time).sum::<Duration>() / n.max(1) as u32;
        let mut hardest: Vec<Answer> = self.answers.iter().filter(|a| a.wrong > 0 || a.hinted).copied().collect();
        hardest.sort_by_key(|a| std::cmp::Reverse((a.hinted, a.wrong, a.time)));
        // each key once, at its worst
        let mut seen = Vec::new();
        hardest.retain(|a| {
            let first = !seen.contains(&a.target.code);
            seen.push(a.target.code);
            first
        });
        hardest.truncate(5);
        Summary { answered: n, clean, average: time, hardest }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub answered: usize,
    /// Right at the first try, without the hint.
    pub clean: usize,
    pub average: Duration,
    pub hardest: Vec<Answer>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const LBRC: u16 = 0x2F;
    const LCBR: u16 = 0x022F; // S(KC_LBRC) = {
    const UNDO: u16 = 0x011D; // C(KC_Z)
    const MO1: u16 = 0x5221;
    const LSFT: u16 = 0xE1;

    /// Two layers, 1 × 4: base A B MO(1) Shift, layer 1 { Ctrl+Z ▽ F5.
    fn keymap() -> Keymap {
        let mut km = Keymap::new(2, 1, 4);
        for (c, code) in [0x04, 0x05, MO1, LSFT].into_iter().enumerate() {
            km.set(0, 0, c as u8, code);
        }
        for (c, code) in [LCBR, UNDO, 0x01, 0x3E].into_iter().enumerate() {
            km.set(1, 0, c as u8, code);
        }
        km
    }

    const PRESENT: [(u8, u8); 4] = [(0, 0), (0, 1), (0, 2), (0, 3)];

    #[test]
    fn pool_takes_the_upper_layers() {
        let p = pool(&keymap(), &PRESENT, 0);
        let codes: Vec<u16> = p.iter().map(|t| t.code).collect();
        assert_eq!(codes, vec![LCBR, UNDO, 0x3E]);
        assert_eq!(p[1], Target { code: UNDO, layer: 1, row: 0, col: 1 });
        // a single layer: its own keys, no modifiers or layer keys
        let mut one = Keymap::new(1, 1, 4);
        for (c, code) in [0x04, MO1, LSFT, 0x04].into_iter().enumerate() {
            one.set(0, 0, c as u8, code);
        }
        assert_eq!(pool(&one, &PRESENT, 0).iter().map(|t| t.code).collect::<Vec<_>>(), vec![0x04]);
    }

    #[test]
    fn judging_presses() {
        assert_eq!(judge(LCBR, LCBR, 0), Outcome::Correct);
        // Shift + [ is a { too, either Shift
        assert_eq!(judge(LCBR, LBRC, 0x02), Outcome::Correct);
        assert_eq!(judge(LCBR, LBRC, 0x20), Outcome::Correct);
        assert_eq!(judge(LCBR, LBRC, 0), Outcome::Ignored);
        assert_eq!(judge(UNDO, 0x1D, 0x01), Outcome::Correct);
        assert_eq!(judge(UNDO, 0x1D, 0x02), Outcome::Ignored);
        // layer keys and modifiers are the way there
        assert_eq!(judge(LCBR, MO1, 0), Outcome::Ignored);
        assert_eq!(judge(LCBR, LSFT, 0), Outcome::Ignored);
        assert_eq!(judge(LCBR, 0x2105, 0), Outcome::Ignored); // mod-tap
        assert_eq!(judge(LCBR, 0x04, 0), Outcome::Wrong);
    }

    #[test]
    fn a_round() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let p = pool(&keymap(), &PRESENT, 0);
        let mut s = Session::new(p, HashMap::new(), 3, 42, t0);
        assert_eq!(s.total(), 3);
        // first: one wrong press, then right
        let first = s.current().unwrap();
        assert_eq!(s.press(0x04, 0, at(500)), Outcome::Wrong);
        assert_eq!(s.wrong(), 1);
        assert_eq!(s.press(first.code, 0, at(1500)), Outcome::Correct);
        // the same question never comes twice in a row
        let second = s.current().unwrap();
        assert_ne!(second.code, first.code);
        assert!(!s.hint_due(at(1500 + 5000)));
        assert!(s.hint_due(at(1500 + 6000)));
        s.skip(at(9000));
        let third = s.current().unwrap();
        assert_eq!(s.press(third.code, 0, at(10_000)), Outcome::Correct);
        assert!(s.finished());
        assert_eq!(s.press(third.code, 0, at(10_100)), Outcome::Ignored);
        let sum = s.summary();
        assert_eq!((sum.answered, sum.clean), (3, 1));
        assert_eq!(sum.hardest.len(), 2);
        assert!(sum.hardest[0].hinted, "skipped first");
        assert_eq!(sum.average, Duration::from_millis(1500 + 7500 + 1000) / 3);
    }

    #[test]
    fn missed_and_rare_keys_weigh_more() {
        let p = pool(&keymap(), &PRESENT, 0);
        let usage = HashMap::from([(LCBR, 100), (UNDO, 100), (0x3E, 0)]);
        let mut s = Session::new(p.clone(), usage, ROUND, 7, Instant::now());
        assert_eq!(s.weight(&p[0]), 1);
        assert_eq!(s.weight(&p[2]), 4);
        s.misses.insert(LCBR, 2);
        assert_eq!(s.weight(&p[0]), 7);
        // an empty pool: nothing to ask
        let empty = Session::new(Vec::new(), HashMap::new(), ROUND, 1, Instant::now());
        assert!(empty.finished());
        assert_eq!(empty.total(), 0);
    }
}
