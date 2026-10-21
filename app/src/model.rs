//! Application model: effective keymap and transparent-key resolution.

use crate::vial::keycode::KC_TRNS;

/// Keycodes for every layer and matrix position (`layer × rows × cols`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    layers: u8,
    rows: u8,
    cols: u8,
    codes: Vec<u16>,
}

impl Keymap {
    pub fn new(layers: u8, rows: u8, cols: u8) -> Self {
        Keymap { layers, rows, cols, codes: vec![0; layers as usize * rows as usize * cols as usize] }
    }

    pub fn layers(&self) -> u8 {
        self.layers
    }

    pub fn rows(&self) -> u8 {
        self.rows
    }

    pub fn cols(&self) -> u8 {
        self.cols
    }

    fn index(&self, layer: u8, row: u8, col: u8) -> Option<usize> {
        (layer < self.layers && row < self.rows && col < self.cols)
            .then(|| (layer as usize * self.rows as usize + row as usize) * self.cols as usize + col as usize)
    }

    /// Keycode at a position; `KC_NO` when out of range.
    pub fn get(&self, layer: u8, row: u8, col: u8) -> u16 {
        self.index(layer, row, col).map(|i| self.codes[i]).unwrap_or(0)
    }

    pub fn set(&mut self, layer: u8, row: u8, col: u8, code: u16) {
        if let Some(i) = self.index(layer, row, col) {
            self.codes[i] = code;
        }
    }

    /// Writes keycodes of `layer` starting at `offset` (`row × cols + col`).
    pub fn set_run(&mut self, layer: u8, offset: u16, codes: &[u16]) {
        for (i, &code) in codes.iter().enumerate() {
            let pos = offset as usize + i;
            let (row, col) = (pos / self.cols as usize, pos % self.cols as usize);
            if row < self.rows as usize {
                self.set(layer, row as u8, col as u8, code);
            }
        }
    }

    /// Resolves `KC_TRNS` like QMK does: starting from `layer`, walk down
    /// the layers that are active (`active_mask`) until a non-transparent
    /// keycode is found. Returns the keycode and the layer it comes from.
    pub fn resolve(&self, layer: u8, active_mask: u32, row: u8, col: u8) -> (u16, u8) {
        let top = layer.min(self.layers.saturating_sub(1));
        for l in (0..=top).rev() {
            if l != top && active_mask & (1 << l) == 0 {
                continue;
            }
            let code = self.get(l, row, col);
            if code != KC_TRNS {
                return (code, l);
            }
        }
        (KC_TRNS, 0)
    }
}

/// Highest active layer, as `get_highest_layer(layer_state | default_layer_state)`.
pub fn highest_layer(mask: u32) -> u8 {
    if mask == 0 {
        0
    } else {
        31 - mask.leading_zeros() as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_set_bounds() {
        let mut k = Keymap::new(2, 2, 3);
        k.set(1, 1, 2, 42);
        assert_eq!(k.get(1, 1, 2), 42);
        k.set(2, 0, 0, 1); // ignored
        k.set(0, 5, 0, 1); // ignored
        assert_eq!(k.get(9, 9, 9), 0);
        k.set_run(0, 2, &[7, 8, 9, 10, 11, 12]); // spills over rows, last one dropped
        assert_eq!((k.get(0, 0, 2), k.get(0, 1, 0), k.get(0, 1, 2)), (7, 8, 10));
    }

    #[test]
    fn transparent_resolution_follows_active_layers() {
        // layers: 0 base, 1 (TRNS at 0,0), 2 (TRNS at 0,0)
        let mut k = Keymap::new(3, 1, 1);
        k.set(0, 0, 0, 0x04);
        k.set(1, 0, 0, 0x05);
        k.set(2, 0, 0, KC_TRNS);
        // layer 1 inactive: layer 2 falls through to base
        assert_eq!(k.resolve(2, 0b101, 0, 0), (0x04, 0));
        // layer 1 active too
        assert_eq!(k.resolve(2, 0b111, 0, 0), (0x05, 1));
        // all transparent
        k.set(0, 0, 0, KC_TRNS);
        k.set(1, 0, 0, KC_TRNS);
        assert_eq!(k.resolve(2, 0b111, 0, 0), (KC_TRNS, 0));
        // out of range layer is clamped
        assert_eq!(k.resolve(9, 0, 0, 0).0, KC_TRNS);
    }

    #[test]
    fn highest() {
        assert_eq!(highest_layer(0), 0);
        assert_eq!(highest_layer(0b1), 0);
        assert_eq!(highest_layer(0b1_0110), 4);
        assert_eq!(highest_layer(1 << 31), 31);
    }
}
