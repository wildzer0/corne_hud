//! Wire format of the HUD protocol (see `docs/PROTOCOL.md`) and of the
//! VIA/Vial commands used by the app. Pure functions, no I/O.

use crate::i18n::pick;
use crate::tr_format;
use crate::vial::dynamic::ENTRY_SIZE;

pub const REPORT_SIZE: usize = 32;
pub type Report = [u8; REPORT_SIZE];

pub const HUD_CMD: u8 = 0xF0;
pub const HUD_PROTOCOL_VERSION: u8 = 1;
pub const STATUS_OK: u8 = 0;
pub const STATUS_PENDING: u8 = 0xFF;
pub const ID_UNHANDLED: u8 = 0xFF;
pub const MAX_KEYMAP_CHUNK: u8 = 12;

pub const VIA_GET_KEYBOARD_VALUE: u8 = 0x02;
pub const VIA_LAYOUT_OPTIONS: u8 = 0x02;
pub const VIA_GET_LAYER_COUNT: u8 = 0x11;
pub const VIAL_PREFIX: u8 = 0xFE;
pub const VIAL_GET_KEYBOARD_ID: u8 = 0x00;
pub const VIAL_GET_SIZE: u8 = 0x01;
pub const VIAL_GET_DEF: u8 = 0x02;
pub const VIAL_DYNAMIC_ENTRY_OP: u8 = 0x0D;
pub const VIAL_GET_ENCODER: u8 = 0x03;
pub const VIAL_QMK_SETTINGS_QUERY: u8 = 0x09;
pub const VIAL_QMK_SETTINGS_GET: u8 = 0x0A;
pub const VIA_GET_PROTOCOL_VERSION: u8 = 0x01;
pub const VIA_MACRO_COUNT: u8 = 0x0C;
pub const VIA_MACRO_BUFFER_SIZE: u8 = 0x0D;
pub const VIA_MACRO_BUFFER: u8 = 0x0E;

/// Sub-commands of `vial_dynamic_entry_op` (reads only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicKind {
    TapDance = 0x01,
    Combo = 0x03,
    KeyOverride = 0x05,
    AltRepeat = 0x07,
}
const DYNAMIC_GET_COUNTS: u8 = 0x00;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HudCommand {
    Info = 0x01,
    State = 0x02,
    Keymap = 0x03,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// The firmware answered `id_unhandled`: no HUD support.
    Unsupported,
    /// The request came back unchanged (Vial unlock in progress): retry later.
    Busy,
    /// Firmware status code other than OK.
    Status(u8),
    /// Response does not match the request (stale or foreign report).
    Mismatch,
    /// Field out of the documented range.
    Invalid(&'static str),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::Unsupported => write!(
                f,
                "{}",
                pick("il firmware non supporta il protocollo HUD", "the firmware does not support the HUD protocol")
            ),
            ProtocolError::Busy => {
                write!(
                    f,
                    "{}",
                    pick("tastiera occupata (sblocco Vial in corso)", "keyboard busy (Vial unlock in progress)")
                )
            }
            ProtocolError::Status(s) => {
                write!(f, "{}", tr_format!("errore del firmware (stato {s})", "firmware error (status {s})"))
            }
            ProtocolError::Mismatch => write!(f, "{}", pick("risposta inattesa", "unexpected answer")),
            ProtocolError::Invalid(what) => {
                write!(f, "{}", tr_format!("risposta non valida: {what}", "invalid answer: {what}"))
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

fn u16_le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

// ------------------------------------------------------------------ HUD

pub fn hud_request(cmd: HudCommand, seq: u8, args: &[u8]) -> Report {
    let mut r = [0u8; REPORT_SIZE];
    r[0] = HUD_CMD;
    r[1] = cmd as u8;
    r[2] = STATUS_PENDING;
    r[3] = seq;
    let n = args.len().min(REPORT_SIZE - 4);
    r[4..4 + n].copy_from_slice(&args[..n]);
    r
}

/// Is `resp` the answer to the HUD request `(cmd, seq)`? Used to skip
/// stale reports still queued from a previous request.
pub fn hud_matches(resp: &Report, cmd: HudCommand, seq: u8) -> bool {
    (resp[0] == HUD_CMD || resp[0] == ID_UNHANDLED) && resp[1] == cmd as u8 && resp[3] == seq
}

/// Validates the common header of a HUD response.
pub fn check_hud(resp: &Report, cmd: HudCommand, seq: u8) -> Result<(), ProtocolError> {
    if !hud_matches(resp, cmd, seq) {
        return Err(ProtocolError::Mismatch);
    }
    if resp[0] == ID_UNHANDLED {
        return Err(ProtocolError::Unsupported);
    }
    match resp[2] {
        STATUS_OK => Ok(()),
        STATUS_PENDING => Err(ProtocolError::Busy),
        s => Err(ProtocolError::Status(s)),
    }
}

#[allow(dead_code)] // complete protocol table: the live indicators (M4) use the rest
pub mod caps {
    pub const COLEMAK: u16 = 1 << 0;
    pub const CAPS_WORD: u16 = 1 << 1;
    pub const OS_DETECTION: u16 = 1 << 2;
    pub const HUD_TOG: u16 = 1 << 3;
    pub const LAYER_LOCK: u16 = 1 << 4;
    pub const SPLIT: u16 = 1 << 5;
}

#[allow(dead_code)] // complete protocol table: the live indicators (M4) use the rest
pub mod flags {
    pub const CAPS_LOCK: u8 = 1 << 0;
    pub const CAPS_WORD: u8 = 1 << 1;
    pub const COLEMAK: u8 = 1 << 2;
    pub const DEBUG_LOCK: u8 = 1 << 3;
    pub const NUM_LOCK: u8 = 1 << 4;
    pub const SLAVE_OK: u8 = 1 << 5;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HudInfo {
    pub protocol: u8,
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub capabilities: u16,
    pub hud_tog_keycode: u16,
    pub keymap_chunk: u8,
    pub keymap_version: u8,
}

pub fn decode_info(resp: &Report, seq: u8) -> Result<HudInfo, ProtocolError> {
    check_hud(resp, HudCommand::Info, seq)?;
    let info = HudInfo {
        protocol: resp[4],
        rows: resp[5],
        cols: resp[6],
        layers: resp[7],
        capabilities: u16_le(resp, 8),
        hud_tog_keycode: u16_le(resp, 10),
        keymap_chunk: resp[12],
        keymap_version: resp[13],
    };
    if info.protocol != HUD_PROTOCOL_VERSION {
        return Err(ProtocolError::Invalid("versione del protocollo HUD"));
    }
    if !(1..=8).contains(&info.rows) || !(1..=8).contains(&info.cols) {
        // GET_STATE carries 8 rows of at most 8 columns
        return Err(ProtocolError::Invalid("dimensioni della matrice"));
    }
    if !(1..=32).contains(&info.layers) {
        return Err(ProtocolError::Invalid("numero di layer"));
    }
    if !(1..=MAX_KEYMAP_CHUNK).contains(&info.keymap_chunk) {
        return Err(ProtocolError::Invalid("dimensione dei blocchi del keymap"));
    }
    Ok(info)
}

/// OS mode / active OS as reported by the firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OsKind {
    #[default]
    Auto,
    Windows,
    MacOs,
    Linux,
}

impl OsKind {
    fn from_mode(v: u8) -> Self {
        match v {
            1 => OsKind::Windows,
            2 => OsKind::MacOs,
            3 => OsKind::Linux,
            _ => OsKind::Auto,
        }
    }
}

/// Host OS detected by QMK (`os_variant_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetectedOs {
    #[default]
    Unsure,
    Linux,
    Windows,
    MacOs,
    Ios,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HudState {
    pub layer_state: u32,
    pub default_layer_state: u8,
    pub locked_layers: u8,
    /// One byte per matrix row, bit `c` = column `c`.
    pub matrix: [u8; 8],
    pub mods: u8,
    pub weak_mods: u8,
    pub oneshot_mods: u8,
    pub oneshot_locked_mods: u8,
    pub oneshot_layer: Option<u8>,
    pub flags: u8,
    pub detected_os: DetectedOs,
    pub os_mode: OsKind,
    pub active_os: OsKind,
    pub hud_toggle_count: u8,
    pub keymap_version: u8,
}

impl HudState {
    pub fn is_pressed(&self, row: u8, col: u8) -> bool {
        row < 8 && col < 8 && self.matrix[row as usize] & (1 << col) != 0
    }

    pub fn flag(&self, f: u8) -> bool {
        self.flags & f != 0
    }

    /// Active layers including the default layer.
    pub fn active_mask(&self) -> u32 {
        self.layer_state | self.default_layer_state as u32
    }
}

pub fn decode_state(resp: &Report, seq: u8) -> Result<HudState, ProtocolError> {
    check_hud(resp, HudCommand::State, seq)?;
    let mut matrix = [0u8; 8];
    matrix.copy_from_slice(&resp[10..18]);
    let oneshot_layer = match resp[22] {
        0xFF => None,
        l if l < 32 => Some(l),
        _ => return Err(ProtocolError::Invalid("layer one-shot")),
    };
    let detected_os = match resp[24] {
        1 => DetectedOs::Linux,
        2 => DetectedOs::Windows,
        3 => DetectedOs::MacOs,
        4 => DetectedOs::Ios,
        _ => DetectedOs::Unsure,
    };
    Ok(HudState {
        layer_state: u32_le(resp, 4),
        default_layer_state: resp[8],
        locked_layers: resp[9],
        matrix,
        mods: resp[18],
        weak_mods: resp[19],
        oneshot_mods: resp[20],
        oneshot_locked_mods: resp[21],
        oneshot_layer,
        flags: resp[23],
        detected_os,
        os_mode: OsKind::from_mode(resp[25]),
        active_os: OsKind::from_mode(resp[26]),
        hud_toggle_count: resp[27],
        keymap_version: resp[28],
    })
}

/// Decodes a `GET_KEYMAP` response, checking that it answers `(layer, offset, count)`.
pub fn decode_keymap(resp: &Report, seq: u8, layer: u8, offset: u16, count: u8) -> Result<Vec<u16>, ProtocolError> {
    check_hud(resp, HudCommand::Keymap, seq)?;
    if resp[4] != layer || u16_le(resp, 5) != offset || resp[7] != count {
        return Err(ProtocolError::Mismatch);
    }
    if count == 0 || count > MAX_KEYMAP_CHUNK {
        return Err(ProtocolError::Invalid("numero di keycode"));
    }
    Ok((0..count as usize).map(|i| u16_le(resp, 8 + 2 * i)).collect())
}

// ------------------------------------------------------------------ VIA / Vial

pub fn via_request(bytes: &[u8]) -> Report {
    let mut r = [0u8; REPORT_SIZE];
    let n = bytes.len().min(REPORT_SIZE);
    r[..n].copy_from_slice(&bytes[..n]);
    r
}

pub fn layout_options_request() -> Report {
    via_request(&[VIA_GET_KEYBOARD_VALUE, VIA_LAYOUT_OPTIONS])
}

pub fn decode_layout_options(resp: &Report) -> Result<u32, ProtocolError> {
    if resp[0] != VIA_GET_KEYBOARD_VALUE || resp[1] != VIA_LAYOUT_OPTIONS {
        return Err(ProtocolError::Mismatch);
    }
    Ok(u32::from_be_bytes([resp[2], resp[3], resp[4], resp[5]]))
}

pub fn layer_count_request() -> Report {
    via_request(&[VIA_GET_LAYER_COUNT])
}

pub fn decode_layer_count(resp: &Report) -> Result<u8, ProtocolError> {
    if resp[0] != VIA_GET_LAYER_COUNT {
        return Err(ProtocolError::Mismatch);
    }
    Ok(resp[1])
}

pub fn vial_id_request() -> Report {
    via_request(&[VIAL_PREFIX, VIAL_GET_KEYBOARD_ID])
}

/// `(vial_protocol, uid)`. Vial overwrites the whole buffer, so there is no header to check.
pub fn decode_vial_id(resp: &Report) -> (u32, u64) {
    let mut uid = [0u8; 8];
    uid.copy_from_slice(&resp[4..12]);
    (u32_le(resp, 0), u64::from_le_bytes(uid))
}

pub fn vial_size_request() -> Report {
    via_request(&[VIAL_PREFIX, VIAL_GET_SIZE])
}

pub fn decode_vial_size(resp: &Report) -> u32 {
    u32_le(resp, 0)
}

pub fn vial_def_request(page: u16) -> Report {
    let p = page.to_le_bytes();
    via_request(&[VIAL_PREFIX, VIAL_GET_DEF, p[0], p[1]])
}

pub fn dynamic_counts_request() -> Report {
    via_request(&[VIAL_PREFIX, VIAL_DYNAMIC_ENTRY_OP, DYNAMIC_GET_COUNTS])
}

/// Entries of each kind: `[tap dances, combos, key overrides, alt repeat
/// keys]`. A firmware without dynamic entries leaves the request unchanged.
pub fn decode_dynamic_counts(req: &Report, resp: &Report) -> Result<[u8; 4], ProtocolError> {
    if resp == req {
        return Err(ProtocolError::Unsupported);
    }
    Ok([resp[0], resp[1], resp[2], resp[3]])
}

pub fn dynamic_entry_request(kind: DynamicKind, index: u8) -> Report {
    via_request(&[VIAL_PREFIX, VIAL_DYNAMIC_ENTRY_OP, kind as u8, index])
}

/// One entry: status in byte 0 (0 = OK), then the 10-byte struct.
pub fn decode_dynamic_entry(req: &Report, resp: &Report) -> Result<[u8; ENTRY_SIZE], ProtocolError> {
    if resp == req {
        return Err(ProtocolError::Unsupported);
    }
    if resp[0] != 0 {
        return Err(ProtocolError::Status(resp[0]));
    }
    let mut e = [0u8; ENTRY_SIZE];
    e.copy_from_slice(&resp[1..1 + ENTRY_SIZE]);
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(cmd: HudCommand, seq: u8, payload: &[(usize, u8)]) -> Report {
        let mut r = hud_request(cmd, seq, &[]);
        r[2] = STATUS_OK;
        for &(i, v) in payload {
            r[i] = v;
        }
        r
    }

    fn keymap_request(seq: u8, layer: u8, offset: u16, count: u8) -> Report {
        let o = offset.to_le_bytes();
        hud_request(HudCommand::Keymap, seq, &[layer, o[0], o[1], count])
    }

    #[test]
    fn request_layout() {
        let r = keymap_request(7, 5, 0x0134, 12);
        assert_eq!(&r[..8], &[0xF0, 0x03, 0xFF, 7, 5, 0x34, 0x01, 12]);
        assert!(r[8..].iter().all(|&b| b == 0));
    }

    #[test]
    fn info_round_trip() {
        let r = response(
            HudCommand::Info,
            3,
            &[(4, 1), (5, 8), (6, 7), (7, 6), (8, 0x3F), (9, 0), (10, 0x12), (11, 0x7E), (12, 12), (13, 9)],
        );
        let info = decode_info(&r, 3).unwrap();
        assert_eq!(
            info,
            HudInfo {
                protocol: 1,
                rows: 8,
                cols: 7,
                layers: 6,
                capabilities: 0x3F,
                hud_tog_keycode: 0x7E12,
                keymap_chunk: 12,
                keymap_version: 9
            }
        );
    }

    #[test]
    fn state_round_trip() {
        let r = response(
            HudCommand::State,
            9,
            &[
                (4, 0b0000_0110),
                (8, 1),
                (10, 0b001_0000),
                (17, 0b010_0000),
                (18, 0x01),
                (19, 0x02),
                (20, 0x04),
                (21, 0x08),
                (22, 5),
                (23, flags::CAPS_WORD | flags::DEBUG_LOCK),
                (24, 3),
                (25, 2),
                (26, 2),
                (27, 200),
                (28, 4),
            ],
        );
        let s = decode_state(&r, 9).unwrap();
        assert_eq!(s.layer_state, 0b110);
        assert_eq!(s.active_mask(), 0b111);
        assert!(s.is_pressed(0, 4) && s.is_pressed(7, 5) && !s.is_pressed(0, 0));
        assert!(!s.is_pressed(9, 0));
        assert_eq!((s.mods, s.weak_mods, s.oneshot_mods, s.oneshot_locked_mods), (1, 2, 4, 8));
        assert_eq!(s.oneshot_layer, Some(5));
        assert!(s.flag(flags::CAPS_WORD) && s.flag(flags::DEBUG_LOCK) && !s.flag(flags::COLEMAK));
        assert_eq!((s.detected_os, s.os_mode, s.active_os), (DetectedOs::MacOs, OsKind::MacOs, OsKind::MacOs));
        assert_eq!((s.hud_toggle_count, s.keymap_version), (200, 4));
    }

    #[test]
    fn keymap_round_trip() {
        let mut r = keymap_request(1, 0, 12, 3);
        r[2] = STATUS_OK;
        r[8..14].copy_from_slice(&[0x2B, 0x21, 0x01, 0x00, 0x12, 0x7E]);
        assert_eq!(decode_keymap(&r, 1, 0, 12, 3).unwrap(), vec![0x212B, 0x0001, 0x7E12]);
        assert_eq!(decode_keymap(&r, 1, 0, 0, 3), Err(ProtocolError::Mismatch));
        assert_eq!(decode_keymap(&r, 1, 1, 12, 3), Err(ProtocolError::Mismatch));
    }

    #[test]
    fn header_errors() {
        let mut r = hud_request(HudCommand::State, 4, &[]);
        // echoed unchanged: Vial unlock in progress
        assert_eq!(decode_state(&r, 4), Err(ProtocolError::Busy));
        // stale sequence number
        assert_eq!(decode_state(&r, 5), Err(ProtocolError::Mismatch));
        // wrong command
        assert_eq!(check_hud(&r, HudCommand::Info, 4), Err(ProtocolError::Mismatch));
        // firmware without HUD support
        r[0] = ID_UNHANDLED;
        assert_eq!(decode_state(&r, 4), Err(ProtocolError::Unsupported));
        // error status
        r[0] = HUD_CMD;
        r[2] = 2;
        assert_eq!(decode_state(&r, 4), Err(ProtocolError::Status(2)));
    }

    #[test]
    fn malformed_payloads_are_rejected() {
        let ok = [(4, 1), (5, 8), (6, 7), (7, 6), (12, 12)];
        assert!(decode_info(&response(HudCommand::Info, 0, &ok), 0).is_ok());
        for bad in [(4, 2), (5, 0), (6, 9), (7, 0), (7, 40), (12, 0), (12, 13)] {
            let mut fields = ok.to_vec();
            fields.push(bad);
            assert!(decode_info(&response(HudCommand::Info, 0, &fields), 0).is_err(), "{bad:?}");
        }
        assert!(decode_state(&response(HudCommand::State, 0, &[(22, 40)]), 0).is_err());
        // random garbage never panics
        let mut seed = 0x1234_5678u32;
        for _ in 0..10_000 {
            let mut r = [0u8; REPORT_SIZE];
            for b in r.iter_mut() {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                *b = (seed >> 16) as u8;
            }
            let _ = decode_info(&r, r[3]);
            let _ = decode_state(&r, r[3]);
            let _ = decode_keymap(&r, r[3], r[4], u16_le(&r, 5), r[7]);
            let _ = decode_layout_options(&r);
            let _ = decode_vial_id(&r);
        }
    }

    #[test]
    fn via_and_vial_requests() {
        assert_eq!(&layout_options_request()[..2], &[0x02, 0x02]);
        let mut r = layout_options_request();
        r[2..6].copy_from_slice(&[0, 0, 0, 0x40]);
        assert_eq!(decode_layout_options(&r), Ok(0x40));
        assert_eq!(&vial_def_request(0x0102)[..4], &[0xFE, 0x02, 0x02, 0x01]);
        let mut id = [0u8; REPORT_SIZE];
        id[0] = 6;
        id[4..12].copy_from_slice(&[0x89, 0x36, 0x2A, 0xC7, 0xFA, 0xD8, 0x89, 0x45]);
        assert_eq!(decode_vial_id(&id), (6, 5010774632021243529));
    }

    #[test]
    fn dynamic_entries() {
        let req = dynamic_counts_request();
        assert_eq!(&req[..3], &[0xFE, 0x0D, 0x00]);
        assert_eq!(decode_dynamic_counts(&req, &req), Err(ProtocolError::Unsupported));
        let mut r = [0u8; REPORT_SIZE];
        r[..3].copy_from_slice(&[8, 16, 4]);
        assert_eq!(decode_dynamic_counts(&req, &r), Ok([8, 16, 4, 0]));

        let req = dynamic_entry_request(DynamicKind::Combo, 3);
        assert_eq!(&req[..4], &[0xFE, 0x0D, 0x03, 0x03]);
        let mut r = [0u8; REPORT_SIZE];
        r[1..11].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(decode_dynamic_entry(&req, &r), Ok([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]));
        r[0] = 1;
        assert_eq!(decode_dynamic_entry(&req, &r), Err(ProtocolError::Status(1)));
    }
}
