//! Simulated keyboard (`--mock`): answers HUD, VIA and Vial requests like
//! the real firmware, with a scripted loop of realistic states (typing,
//! Fn1/Fn2/tri-layer, one-shot ACC, mod-tap Ctrl, Caps Word, DEBUG lock).

use super::protocol::{self as p, flags, Report, REPORT_SIZE};
use super::transport::{Transport, TransportError};
use crate::model::Keymap;
use crate::vial::vil::Vil;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub const MOCK_VIAL_JSON: &str = include_str!("../../data/mock/vial.json");
pub const MOCK_VIL: &str = include_str!("../../data/mock/corne.vil");
// "MOCK" + 1: distinct from real keyboards, so the definition cache never mixes them
const MOCK_UID: u64 = 0x4D4F_434B_0000_0001;

/// Simulated firmware state at one instant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MockFrame {
    pub layer_state: u32,
    pub pressed: [u8; 8],
    pub mods: u8,
    pub oneshot_layer: Option<u8>,
    pub flags: u8,
}

impl MockFrame {
    fn press(&mut self, row: u8, col: u8) {
        self.pressed[row as usize] |= 1 << col;
    }
}

const PERIOD_MS: u64 = 16_000;

/// Keys tapped every 160 ms (pressed for 90 ms) during a scripted phase.
fn tap_sequence(frame: &mut MockFrame, t: u64, start: u64, keys: &[(u8, u8)]) {
    let since = t.saturating_sub(start);
    let i = (since / 160) as usize;
    if since % 160 < 90 {
        if let Some(&(r, c)) = keys.get(i % keys.len().max(1)) {
            frame.press(r, c);
        }
    }
}

/// Scripted state at `t` milliseconds into the loop.
pub fn frame_at(t_ms: u64) -> MockFrame {
    let t = t_ms % PERIOD_MS;
    let mut f = MockFrame::default();
    const FN1: (u8, u8) = (3, 4);
    const FN2: (u8, u8) = (7, 4);
    match t {
        // typing on BASE: "hello world"-ish positions
        0..=1999 => tap_sequence(&mut f, t, 0, &[(5, 5), (0, 3), (5, 2), (5, 2), (4, 2), (3, 5), (0, 2), (4, 2)]),
        // hold Fn1: SYM layer, type a few symbols and digits
        2000..=3999 => {
            f.layer_state = 1 << 1;
            f.press(FN1.0, FN1.1);
            tap_sequence(&mut f, t, 2200, &[(1, 1), (1, 2), (5, 4), (5, 3), (5, 2)]);
        }
        // hold Fn2: NAV layer, arrows
        4000..=5999 => {
            f.layer_state = 1 << 2;
            f.press(FN2.0, FN2.1);
            tap_sequence(&mut f, t, 4200, &[(5, 5), (5, 4), (5, 3), (5, 2)]);
        }
        // Fn1 + Fn2: tri-layer SYS
        6000..=7999 => {
            f.layer_state = (1 << 1) | (1 << 2) | (1 << 3);
            f.press(FN1.0, FN1.1);
            f.press(FN2.0, FN2.1);
            if (6800..7000).contains(&t) {
                f.press(0, 2); // F9
            }
        }
        // one-shot ACC: tap OSL(5), wait, then the vowel
        8000..=9499 => {
            if t < 8100 {
                f.press(1, 6);
            }
            if t < 9000 {
                f.layer_state = 1 << 5;
                f.oneshot_layer = Some(5);
            } else if t < 9100 {
                f.press(0, 3); // è
            }
        }
        // mod-tap Ctrl held (LCTL_T(KC_TAB)) + C
        9500..=10999 => {
            f.press(1, 0);
            if t >= 9700 {
                f.mods = 0x01;
            }
            if (10_000..10_150).contains(&t) {
                f.press(2, 3);
            }
        }
        // Caps Word while typing
        11_000..=12_499 => {
            f.flags |= flags::CAPS_WORD;
            tap_sequence(&mut f, t, 11_000, &[(0, 1), (0, 2), (0, 3), (0, 4)]);
        }
        // DEBUG locked with TG(4), step over / step into
        12_500..=14_999 => {
            if t < 12_600 {
                f.press(0, 0);
            }
            f.layer_state = 1 << 4;
            f.flags |= flags::DEBUG_LOCK;
            tap_sequence(&mut f, t, 13_000, &[(1, 3), (1, 3), (1, 4), (1, 2)]);
        }
        _ => {}
    }
    f
}

pub struct MockKeyboard {
    start: Instant,
    keymap: Keymap,
    definition_xz: Vec<u8>,
    pending: VecDeque<Report>,
    /// Fixed time for tests; `None` = real clock.
    frozen_ms: Option<u64>,
}

impl MockKeyboard {
    pub fn new() -> Self {
        let keymap = Vil::parse(MOCK_VIL, 8, 7).map(|v| v.keymap).unwrap_or_else(|_| Keymap::new(6, 8, 7));
        let mut definition_xz = Vec::new();
        // compressing an in-memory buffer cannot fail
        let _ = lzma_rs::xz_compress(&mut MOCK_VIAL_JSON.as_bytes(), &mut definition_xz);
        MockKeyboard { start: Instant::now(), keymap, definition_xz, pending: VecDeque::new(), frozen_ms: None }
    }

    #[cfg(test)]
    pub fn frozen(ms: u64) -> Self {
        MockKeyboard { frozen_ms: Some(ms), ..Self::new() }
    }

    fn now_ms(&self) -> u64 {
        self.frozen_ms.unwrap_or_else(|| self.start.elapsed().as_millis() as u64)
    }

    fn answer(&self, req: &Report) -> Option<Report> {
        let mut r = *req;
        match req[0] {
            p::HUD_CMD => self.answer_hud(&mut r),
            p::VIAL_PREFIX => {
                r = [0u8; REPORT_SIZE];
                match req[1] {
                    p::VIAL_GET_KEYBOARD_ID => {
                        r[0..4].copy_from_slice(&6u32.to_le_bytes());
                        r[4..12].copy_from_slice(&MOCK_UID.to_le_bytes());
                        r[12] = 1;
                    }
                    p::VIAL_GET_SIZE => r[0..4].copy_from_slice(&(self.definition_xz.len() as u32).to_le_bytes()),
                    p::VIAL_GET_DEF => {
                        let start = u16::from_le_bytes([req[2], req[3]]) as usize * REPORT_SIZE;
                        if start >= self.definition_xz.len() {
                            return Some(*req); // out of range: echo, like vial.c
                        }
                        let end = (start + REPORT_SIZE).min(self.definition_xz.len());
                        r[..end - start].copy_from_slice(&self.definition_xz[start..end]);
                    }
                    _ => r = *req,
                }
            }
            p::VIA_GET_KEYBOARD_VALUE if req[1] == p::VIA_LAYOUT_OPTIONS => r[2..6].copy_from_slice(&[0; 4]),
            p::VIA_GET_LAYER_COUNT => r[1] = self.keymap.layers(),
            0x12 => {
                let offset = u16::from_be_bytes([req[1], req[2]]) as usize;
                let size = req[3] as usize;
                if size <= 28 {
                    let per_layer = self.keymap.rows() as usize * self.keymap.cols() as usize;
                    for i in 0..size {
                        let byte = offset + i;
                        let (idx, low) = (byte / 2, byte % 2 == 1);
                        let (layer, pos) = (idx / per_layer, idx % per_layer);
                        let cols = self.keymap.cols() as usize;
                        let kc = self.keymap.get(layer as u8, (pos / cols) as u8, (pos % cols) as u8);
                        r[4 + i] = if low { kc as u8 } else { (kc >> 8) as u8 };
                    }
                }
            }
            _ => r[0] = p::ID_UNHANDLED,
        }
        Some(r)
    }

    fn answer_hud(&self, r: &mut Report) {
        let args = [r[4], r[5], r[6], r[7]];
        r[4..].fill(0);
        r[2] = p::STATUS_OK;
        match r[1] {
            0x01 => {
                r[4] = p::HUD_PROTOCOL_VERSION;
                r[5] = self.keymap.rows();
                r[6] = self.keymap.cols();
                r[7] = self.keymap.layers();
                r[8..10].copy_from_slice(&0x3Fu16.to_le_bytes());
                r[10..12].copy_from_slice(&0x7E12u16.to_le_bytes());
                r[12] = p::MAX_KEYMAP_CHUNK;
            }
            0x02 => {
                let f = frame_at(self.now_ms());
                r[4..8].copy_from_slice(&f.layer_state.to_le_bytes());
                r[8] = 1;
                r[10..18].copy_from_slice(&f.pressed);
                r[18] = f.mods;
                r[22] = f.oneshot_layer.unwrap_or(0xFF);
                r[23] = f.flags | flags::SLAVE_OK;
                r[24] = 2; // Windows
                r[25] = 0;
                r[26] = 1;
            }
            0x03 => {
                let (layer, offset, count) = (args[0], u16::from_le_bytes([args[1], args[2]]), args[3]);
                r[4..8].copy_from_slice(&args);
                let total = self.keymap.rows() as u16 * self.keymap.cols() as u16;
                if layer >= self.keymap.layers()
                    || count == 0
                    || count > p::MAX_KEYMAP_CHUNK
                    || offset + count as u16 > total
                {
                    r[2] = 2;
                    return;
                }
                let cols = self.keymap.cols() as u16;
                for i in 0..count as u16 {
                    let pos = offset + i;
                    let kc = self.keymap.get(layer, (pos / cols) as u8, (pos % cols) as u8);
                    r[8 + 2 * i as usize..10 + 2 * i as usize].copy_from_slice(&kc.to_le_bytes());
                }
            }
            _ => r[2] = 1,
        }
    }
}

impl Default for MockKeyboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for MockKeyboard {
    fn send(&mut self, report: &Report) -> Result<(), TransportError> {
        if let Some(r) = self.answer(report) {
            self.pending.push_back(r);
        }
        Ok(())
    }

    fn recv(&mut self, _timeout: Duration) -> Result<Option<Report>, TransportError> {
        Ok(self.pending.pop_front())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::device::Device;

    fn device_at(ms: u64) -> Device {
        Device::new(Box::new(MockKeyboard::frozen(ms)), Duration::from_millis(50))
    }

    #[test]
    fn full_session_over_the_protocol() {
        let mut d = device_at(0);
        let info = d.info().unwrap();
        assert_eq!((info.rows, info.cols, info.layers, info.hud_tog_keycode), (8, 7, 6, 0x7E12));
        let size = d.definition_size().unwrap();
        let def = crate::vial::definition::Definition::from_xz(&d.definition_xz(size).unwrap()).unwrap();
        assert_eq!(def.visible_keys(d.layout_options().unwrap()).len(), 46);
        assert_eq!(d.vial_id().unwrap(), (6, MOCK_UID));
        let km = d.keymap(&info).unwrap();
        let expected = Vil::parse(MOCK_VIL, 8, 7).unwrap().keymap;
        assert_eq!(km, expected);
        assert_eq!(d.via_keymap(info.layers, info.rows, info.cols).unwrap(), expected);
        assert_eq!(d.layer_count().unwrap(), 6);
    }

    #[test]
    fn scripted_states() {
        assert_eq!(device_at(3000).state().unwrap().layer_state, 1 << 1);
        let sys = device_at(7000).state().unwrap();
        assert_eq!(sys.layer_state, 0b1110);
        assert!(sys.is_pressed(3, 4) && sys.is_pressed(7, 4));
        let osl = device_at(8500).state().unwrap();
        assert_eq!(osl.oneshot_layer, Some(5));
        assert_eq!(device_at(10_000).state().unwrap().mods, 0x01);
        assert!(device_at(14_000).state().unwrap().flag(flags::DEBUG_LOCK));
        assert_eq!(device_at(PERIOD_MS + 3000).state().unwrap().layer_state, 1 << 1);
    }

    #[test]
    fn unknown_commands_are_unhandled() {
        let mut m = MockKeyboard::frozen(0);
        m.send(&p::via_request(&[0x42])).unwrap();
        assert_eq!(m.recv(Duration::ZERO).unwrap().unwrap()[0], p::ID_UNHANDLED);
    }
}
