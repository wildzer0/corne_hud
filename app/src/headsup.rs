//! Heads-up mode: the HUD stays hidden and fades in when it is useful
//! (a layer above the default one or a modifier held for a while, a pending
//! one-shot), then fades out shortly after the keyboard is back to normal.
//! A layer key released before its delay (a combination known by heart)
//! does not bring the HUD up.

use crate::config::{HeadsUp as Settings, ShownIn};
use crate::hid::protocol::HudState;
use crate::model::LayerHold;
use std::time::{Duration, Instant};

const FADE_IN: Duration = Duration::from_millis(120);
const FADE_OUT: Duration = Duration::from_millis(300);

#[derive(Debug)]
pub struct HeadsUp {
    layers_since: Option<Instant>,
    mods_since: Option<Instant>,
    /// A modifier has been held past the delay.
    mods_long: bool,
    last_wanted: Option<Instant>,
    /// 0 = hidden, 1 = fully shown.
    level: f32,
    last_update: Option<Instant>,
}

impl Default for HeadsUp {
    fn default() -> Self {
        // start visible: the first states decide quickly
        HeadsUp {
            layers_since: None,
            mods_since: None,
            mods_long: false,
            last_wanted: None,
            level: 1.0,
            last_update: None,
        }
    }
}

/// The state as heads-up sees it: what is left to the mini indicator
/// (locked layers, pending one-shots) does not bring up the HUD.
pub fn without_indicator_states(state: &HudState, layers: &[(u8, LayerHold)], settings: &Settings) -> HudState {
    let mut s = *state;
    let locked = settings.locked_layer == ShownIn::Indicator;
    let oneshot = settings.oneshot == ShownIn::Indicator;
    for &(l, how) in layers {
        if (how == LayerHold::Locked && locked) || (how == LayerHold::OneShot && oneshot) {
            s.layer_state &= !(1u32 << l);
        }
    }
    if oneshot {
        s.oneshot_layer = None;
        s.oneshot_mods = 0;
        s.oneshot_locked_mods = 0;
    }
    s
}

impl HeadsUp {
    /// Does this state call for the HUD right now?
    pub fn wants(&mut self, state: &HudState, settings: &Settings, now: Instant) -> bool {
        let one_shot = state.oneshot_layer.is_some() || state.oneshot_mods != 0 || state.oneshot_locked_mods != 0;
        let held_long = |since: &mut Option<Instant>, held: bool, delay_ms: u64| {
            if held {
                let t = *since.get_or_insert(now);
                now.duration_since(t) >= Duration::from_millis(delay_ms)
            } else {
                *since = None;
                false
            }
        };
        let layer_up = state.layer_state & !(state.default_layer_state as u32) != 0;
        let above_default = held_long(&mut self.layers_since, layer_up, settings.layer_delay_ms);
        let mods = held_long(&mut self.mods_since, state.mods != 0, settings.modifier_delay_ms);
        self.mods_long = mods;
        above_default || one_shot || (mods && settings.modifiers == ShownIn::Hud)
    }

    /// A layer or a modifier is held, but not for long enough yet: the
    /// state may not change when the delay runs out, so check again soon.
    pub fn waiting(&self) -> bool {
        self.layers_since.is_some() || (self.mods_since.is_some() && !self.mods_long)
    }

    /// A modifier is held past the delay (for the mini indicator).
    pub fn mods_held_long(&self) -> bool {
        self.mods_long
    }

    /// Advances the fade. `force` keeps the HUD fully shown (no live state,
    /// search or menu open, HUD pinned). Returns the visibility level 0..1.
    pub fn update(&mut self, wanted: bool, force: bool, settings: &Settings, now: Instant) -> f32 {
        // the first state keeps the HUD up for a moment: the app is running, and this is where it shows up
        if wanted || force || self.last_update.is_none() {
            self.last_wanted = Some(now);
        }
        let hold = Duration::from_millis(settings.hide_delay_ms);
        let target = if self.last_wanted.is_some_and(|t| now.duration_since(t) <= hold) { 1.0 } else { 0.0 };
        let dt = self.last_update.map_or(Duration::ZERO, |t| now.duration_since(t));
        self.last_update = Some(now);
        let speed = if target > self.level { FADE_IN } else { FADE_OUT };
        let step = dt.as_secs_f32() / speed.as_secs_f32();
        self.level =
            if target > self.level { (self.level + step).min(target) } else { (self.level - step).max(target) };
        // wake up at once when needed, without waiting for the fade-in
        if force {
            self.level = 1.0;
        }
        self.level
    }

    pub fn level(&self) -> f32 {
        self.level
    }

    /// Still fading in or out.
    pub fn animating(&self) -> bool {
        self.level > 0.0 && self.level < 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings::default()
    }

    #[test]
    fn conditions() {
        let s = settings();
        let t0 = Instant::now();
        let mut h = HeadsUp::default();
        let base = HudState { default_layer_state: 1, ..Default::default() };
        assert!(!h.wants(&base, &s, t0));
        // a layer counts only after its delay: a quick Fn1 + key does not bring the HUD up
        let sym = HudState { layer_state: 0b10, ..base };
        assert!(!h.wants(&sym, &s, t0));
        assert!(h.waiting());
        assert!(h.wants(&sym, &s, t0 + Duration::from_millis(s.layer_delay_ms)));
        assert!(!h.wants(&base, &s, t0 + Duration::from_millis(400)));
        assert!(!h.wants(&sym, &s, t0 + Duration::from_millis(450)));
        // without delay: at once
        let now = Settings { layer_delay_ms: 0, ..settings() };
        assert!(HeadsUp::default().wants(&sym, &now, t0));
        assert!(h.wants(&HudState { oneshot_layer: Some(5), ..base }, &s, t0));
        assert!(h.wants(&HudState { oneshot_mods: 0x02, ..base }, &s, t0));
        // a modifier counts only after the delay
        let ctrl = HudState { mods: 0x01, ..base };
        assert!(!h.wants(&ctrl, &s, t0));
        assert!(!h.wants(&ctrl, &s, t0 + Duration::from_millis(300)));
        assert!(h.wants(&ctrl, &s, t0 + Duration::from_millis(450)));
        // released: the timer restarts
        assert!(!h.wants(&base, &s, t0 + Duration::from_millis(500)));
        assert!(!h.wants(&ctrl, &s, t0 + Duration::from_millis(600)));
    }

    #[test]
    fn indicator_states_left_out() {
        let base = HudState { default_layer_state: 1, ..Default::default() };
        let osl = HudState { layer_state: 1 << 5, oneshot_layer: Some(5), oneshot_mods: 0x02, ..base };
        let layers = [(5, LayerHold::OneShot)];
        // default: the one-shot brings up the HUD
        assert_eq!(without_indicator_states(&osl, &layers, &settings()), osl);
        let s = Settings { oneshot: ShownIn::Indicator, layer_delay_ms: 0, ..settings() };
        let mut h = HeadsUp::default();
        assert!(!h.wants(&without_indicator_states(&osl, &layers, &s), &s, Instant::now()));
        // the one-shot layer key still held: that is a held layer, the HUD comes up
        let held = [(5, LayerHold::Held)];
        assert!(h.wants(&without_indicator_states(&osl, &held, &s), &s, Instant::now()));
        // locked layers follow their own setting
        let tg = HudState { layer_state: 1 << 4, ..base };
        let locked = [(4, LayerHold::Locked)];
        assert!(h.wants(&without_indicator_states(&tg, &locked, &s), &s, Instant::now()));
        let s = Settings { locked_layer: ShownIn::Indicator, ..s };
        assert!(!h.wants(&without_indicator_states(&tg, &locked, &s), &s, Instant::now()));
    }

    #[test]
    fn held_modifiers_left_to_the_indicator() {
        let s = Settings { modifiers: ShownIn::Indicator, ..settings() };
        let t0 = Instant::now();
        let mut h = HeadsUp::default();
        let ctrl = HudState { default_layer_state: 1, mods: 0x01, ..Default::default() };
        assert!(!h.wants(&ctrl, &s, t0));
        assert!(!h.mods_held_long());
        assert!(!h.wants(&ctrl, &s, t0 + Duration::from_millis(450)));
        assert!(h.mods_held_long());
    }

    #[test]
    fn fades_in_and_out() {
        let s = settings();
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let mut h = HeadsUp::default();
        h.update(false, false, &s, t0);
        // the first state keeps the HUD up for hide_delay
        assert_eq!(h.update(false, false, &s, ms(500)), 1.0);
        // nothing wanted for longer than hide_delay: fades out
        h.update(false, false, &s, ms(900));
        assert_eq!(h.level(), 0.0);
        // wanted: fades in quickly
        h.update(true, false, &s, ms(1000));
        assert!(h.level() > 0.5 && h.level() < 1.0, "{}", h.level());
        h.update(true, false, &s, ms(1200));
        assert_eq!(h.level(), 1.0);
        // back to base: stays for hide_delay, then fades
        h.update(false, false, &s, ms(1700));
        assert_eq!(h.level(), 1.0);
        h.update(false, false, &s, ms(2100));
        assert!(h.level() < 1.0);
        h.update(false, false, &s, ms(2600));
        assert_eq!(h.level(), 0.0);
        // forced: shown at once
        assert_eq!(h.update(false, true, &s, ms(2700)), 1.0);
    }
}
