//! Checks before the HUD writes a key to the keyboard: what is refused
//! outright and what deserves a warning (a layer nobody can reach any more,
//! the last key for the bootloader or for HUD_TOG). Pure functions.

use crate::model::Keymap;
use crate::vial::dynamic::Dynamic;
use crate::vial::keycode::{decode, Keycode, LayerOp, TRI_LAYER_ADJUST, TRI_LAYER_LOWER, TRI_LAYER_UPPER};
use crate::vial::keycode::{KC_NO, KC_TRNS};

pub const QK_BOOT: u16 = 0x7C00;

/// The key that would be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
    pub code: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Vial wants the keyboard unlocked for it: that stays in Vial.
    Bootloader,
    /// Same keycode as now.
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warning {
    /// No key left that reaches this layer.
    LayerUnreachable(u8),
    /// The last `QK_BOOT`: flashing would need the reset button.
    LastBootloader,
    /// The last `HUD_TOG`.
    LastHudToggle,
    /// Empty or transparent key on the default layer.
    EmptyOnBase,
}

/// Layers a keycode turns on (tri-layer: lower or upper alone).
fn layers_of(code: u16, dynamic: &Dynamic, out: &mut Vec<u8>) {
    match decode(code) {
        Keycode::Layer { layer, .. } => out.push(layer),
        Keycode::LayerTap { layer, .. } | Keycode::LayerMod { layer, .. } => out.push(layer),
        Keycode::TapDance(n) => {
            if let Some(td) = dynamic.tap_dances.get(n as usize) {
                for c in [td.on_tap, td.on_hold, td.on_double_tap, td.on_tap_hold] {
                    // a tap dance inside a tap dance is not followed
                    if !matches!(decode(c), Keycode::TapDance(_)) {
                        layers_of(c, dynamic, out);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Layers reachable from the default layer with the keys at `present`
/// (combo outputs count from any layer).
pub fn reachable(keymap: &Keymap, present: &[(u8, u8)], dynamic: &Dynamic, default_layer: u8) -> u32 {
    let mut mask = 1u32 << default_layer;
    let mut from_anywhere = Vec::new();
    for c in &dynamic.combos {
        layers_of(c.output, dynamic, &mut from_anywhere);
    }
    loop {
        let before = mask;
        let mut found = from_anywhere.clone();
        let (mut lower, mut upper) = (false, false);
        for layer in (0..keymap.layers()).filter(|l| mask & (1 << l) != 0) {
            for &(r, c) in present {
                let code = keymap.get(layer, r, c);
                match decode(code) {
                    Keycode::Layer { op: LayerOp::TriLayerLower, .. } => lower = true,
                    Keycode::Layer { op: LayerOp::TriLayerUpper, .. } => upper = true,
                    _ => layers_of(code, dynamic, &mut found),
                }
            }
        }
        if lower {
            found.push(TRI_LAYER_LOWER);
        }
        if upper {
            found.push(TRI_LAYER_UPPER);
        }
        if lower && upper {
            found.push(TRI_LAYER_ADJUST);
        }
        for l in found.into_iter().filter(|&l| l < 32) {
            mask |= 1 << l;
        }
        if mask == before {
            return mask;
        }
    }
}

/// Reachable keys with this keycode.
fn count_reachable(keymap: &Keymap, present: &[(u8, u8)], mask: u32, code: u16) -> usize {
    (0..keymap.layers())
        .filter(|l| mask & (1 << l) != 0)
        .map(|l| present.iter().filter(|&&(r, c)| keymap.get(l, r, c) == code).count())
        .sum()
}

/// What to say before writing `change`. `hud_tog` is the firmware's
/// HUD_TOG keycode, if it has one.
pub fn check(
    keymap: &Keymap,
    present: &[(u8, u8)],
    dynamic: &Dynamic,
    default_layer: u8,
    hud_tog: Option<u16>,
    change: Change,
) -> Result<Vec<Warning>, Refusal> {
    let old = keymap.get(change.layer, change.row, change.col);
    if change.code == QK_BOOT {
        return Err(Refusal::Bootloader);
    }
    if change.code == old {
        return Err(Refusal::Unchanged);
    }
    let mut after = keymap.clone();
    after.set(change.layer, change.row, change.col, change.code);
    let before_mask = reachable(keymap, present, dynamic, default_layer);
    let after_mask = reachable(&after, present, dynamic, default_layer);
    let mut warnings: Vec<Warning> =
        (0..32u8).filter(|l| before_mask & !after_mask & (1 << l) != 0).map(Warning::LayerUnreachable).collect();
    // replaced, or left on a layer nobody reaches any more
    let lost = |code: u16| {
        count_reachable(keymap, present, before_mask, code) > 0
            && count_reachable(&after, present, after_mask, code) == 0
    };
    if lost(QK_BOOT) {
        warnings.push(Warning::LastBootloader);
    }
    if hud_tog.is_some_and(lost) {
        warnings.push(Warning::LastHudToggle);
    }
    if change.layer == default_layer && (change.code == KC_NO || change.code == KC_TRNS) {
        warnings.push(Warning::EmptyOnBase);
    }
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vial::dynamic::{Combo, TapDance};

    const MO1: u16 = 0x5221;
    const TG4: u16 = 0x5264;
    const TL_LOWR: u16 = 0x7C77;
    const TL_UPPR: u16 = 0x7C78;
    const HUD_TOG: u16 = 0x7E12;

    /// Five layers, two keys per layer: (0,0) and (0,1).
    fn keymap(keys: &[(u8, u8, u16)]) -> Keymap {
        let mut km = Keymap::new(5, 1, 3);
        for l in 0..5 {
            km.set(l, 0, 0, 0x04);
            km.set(l, 0, 1, 0x05);
        }
        for &(l, c, code) in keys {
            km.set(l, 0, c, code);
        }
        km
    }

    const PRESENT: [(u8, u8); 2] = [(0, 0), (0, 1)];

    fn change(layer: u8, col: u8, code: u16) -> Change {
        Change { layer, row: 0, col, code }
    }

    #[test]
    fn layers_reached_by_keys_tap_dances_combos_and_tri_layer() {
        let none = Dynamic::default();
        // MO(1) on the base layer, TG(4) on layer 1: 0, 1, 4
        let km = keymap(&[(0, 0, MO1), (1, 1, TG4)]);
        assert_eq!(reachable(&km, &PRESENT, &none, 0), 0b10011);
        // a key on a layer nobody reaches does not count
        let km = keymap(&[(2, 0, TG4)]);
        assert_eq!(reachable(&km, &PRESENT, &none, 0), 0b1);
        // tri-layer: both keys reach the third layer
        let km = keymap(&[(0, 0, TL_LOWR), (0, 1, TL_UPPR)]);
        assert_eq!(reachable(&km, &PRESENT, &none, 0), 0b1111);
        // a tap dance holding MO(1)
        let td = Dynamic {
            tap_dances: vec![TapDance { on_tap: 0x04, on_hold: MO1, ..Default::default() }],
            ..Default::default()
        };
        let km = keymap(&[(0, 0, 0x5700)]);
        assert_eq!(reachable(&km, &PRESENT, &td, 0), 0b11);
        // a combo giving TG(4)
        let combo = Dynamic { combos: vec![Combo { keys: [0x04, 0x05, 0, 0], output: TG4 }], ..Default::default() };
        assert_eq!(reachable(&keymap(&[]), &PRESENT, &combo, 0), 0b10001);
        // keys outside `present` are ignored
        let mut km = keymap(&[]);
        km.set(0, 0, 2, MO1);
        assert_eq!(reachable(&km, &PRESENT, &none, 0), 0b1);
    }

    #[test]
    fn warnings() {
        let none = Dynamic::default();
        let km = keymap(&[(0, 0, MO1), (1, 1, QK_BOOT), (1, 0, HUD_TOG)]);
        let check = |c| check(&km, &PRESENT, &none, 0, Some(HUD_TOG), c);
        // replacing the only MO(1): layer 1 is lost, and the keys on it
        assert_eq!(
            check(change(0, 0, 0x06)),
            Ok(vec![Warning::LayerUnreachable(1), Warning::LastBootloader, Warning::LastHudToggle])
        );
        assert_eq!(check(change(1, 1, 0x06)), Ok(vec![Warning::LastBootloader]));
        assert_eq!(check(change(1, 0, 0x06)), Ok(vec![Warning::LastHudToggle]));
        assert_eq!(check(change(0, 1, KC_TRNS)), Ok(vec![Warning::EmptyOnBase]));
        assert_eq!(check(change(2, 1, KC_NO)), Ok(vec![]));
        // without HUD_TOG in the firmware there is nothing to lose
        assert_eq!(super::check(&km, &PRESENT, &none, 0, None, change(1, 0, 0x06)), Ok(vec![]));
        // a second MO(1) elsewhere: nothing lost
        let km2 = keymap(&[(0, 0, MO1), (0, 1, MO1)]);
        assert_eq!(super::check(&km2, &PRESENT, &none, 0, None, change(0, 0, 0x06)), Ok(vec![]));
    }

    #[test]
    fn refusals() {
        let km = keymap(&[]);
        let none = Dynamic::default();
        assert_eq!(check(&km, &PRESENT, &none, 0, None, change(1, 0, QK_BOOT)), Err(Refusal::Bootloader));
        assert_eq!(check(&km, &PRESENT, &none, 0, None, change(1, 0, 0x04)), Err(Refusal::Unchanged));
    }
}
