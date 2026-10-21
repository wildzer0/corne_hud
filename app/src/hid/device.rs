//! High-level keyboard access over any [`Transport`]: HUD requests with
//! sequence numbers and timeouts, plus the VIA/Vial reads the app needs.

use super::protocol::{self as p, HudCommand, HudInfo, HudState, ProtocolError, Report};
use super::transport::{Transport, TransportError};
use crate::model::Keymap;
use crate::vial::definition::DefinitionError;
use std::fmt;
use std::time::{Duration, Instant};

/// Upper bound for the compressed definition (the Corne's is ~2 KiB).
const MAX_DEFINITION_SIZE: u32 = 256 * 1024;

#[derive(Debug)]
pub enum DeviceError {
    Transport(TransportError),
    Protocol(ProtocolError),
    Timeout,
    Definition(DefinitionError),
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceError::Transport(e) => e.fmt(f),
            DeviceError::Protocol(e) => e.fmt(f),
            DeviceError::Timeout => write!(f, "la tastiera non risponde"),
            DeviceError::Definition(e) => write!(f, "definizione Vial: {e}"),
        }
    }
}

impl std::error::Error for DeviceError {}

impl From<TransportError> for DeviceError {
    fn from(e: TransportError) -> Self {
        DeviceError::Transport(e)
    }
}

impl From<ProtocolError> for DeviceError {
    fn from(e: ProtocolError) -> Self {
        DeviceError::Protocol(e)
    }
}

pub struct Device {
    transport: Box<dyn Transport>,
    seq: u8,
    timeout: Duration,
}

impl Device {
    pub fn new(transport: Box<dyn Transport>, timeout: Duration) -> Self {
        Device { transport, seq: 0, timeout }
    }

    fn next_seq(&mut self) -> u8 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    /// Sends a HUD request and waits for the matching response, skipping
    /// stale reports.
    fn hud(&mut self, cmd: HudCommand, args: &[u8]) -> Result<(Report, u8), DeviceError> {
        let seq = self.next_seq();
        self.transport.flush()?;
        self.transport.send(&p::hud_request(cmd, seq, args))?;
        let deadline = Instant::now() + self.timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.transport.recv(left)? {
                Some(r) if p::hud_matches(&r, cmd, seq) => return Ok((r, seq)),
                Some(_) => continue,
                None if Instant::now() >= deadline => return Err(DeviceError::Timeout),
                None => continue,
            }
        }
    }

    /// Sends a VIA/Vial request and returns the first response. VIA echoes the
    /// command id in byte 0; Vial overwrites the whole buffer, so `expect` is
    /// `None` for Vial commands.
    fn via(&mut self, req: &Report, expect: Option<u8>) -> Result<Report, DeviceError> {
        self.transport.flush()?;
        self.transport.send(req)?;
        let deadline = Instant::now() + self.timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.transport.recv(left)? {
                Some(r) if expect.is_none_or(|id| r[0] == id) => return Ok(r),
                Some(_) => continue,
                None if Instant::now() >= deadline => return Err(DeviceError::Timeout),
                None => continue,
            }
        }
    }

    pub fn info(&mut self) -> Result<HudInfo, DeviceError> {
        let (r, seq) = self.hud(HudCommand::Info, &[])?;
        Ok(p::decode_info(&r, seq)?)
    }

    pub fn state(&mut self) -> Result<HudState, DeviceError> {
        let (r, seq) = self.hud(HudCommand::State, &[])?;
        Ok(p::decode_state(&r, seq)?)
    }

    /// Effective keymap (Colemak remap already applied by the firmware).
    pub fn keymap(&mut self, info: &HudInfo) -> Result<Keymap, DeviceError> {
        let mut km = Keymap::new(info.layers, info.rows, info.cols);
        let total = info.rows as u16 * info.cols as u16;
        for layer in 0..info.layers {
            let mut offset = 0u16;
            while offset < total {
                let count = (info.keymap_chunk as u16).min(total - offset) as u8;
                let o = offset.to_le_bytes();
                let (r, seq) = self.hud(HudCommand::Keymap, &[layer, o[0], o[1], count])?;
                let codes = p::decode_keymap(&r, seq, layer, offset, count)?;
                km.set_run(layer, offset, &codes);
                offset += count as u16;
            }
        }
        Ok(km)
    }

    /// Raw keymap via VIA `dynamic_keymap_get_buffer` (no Colemak remap).
    /// Used when the firmware does not speak the HUD protocol.
    pub fn via_keymap(&mut self, layers: u8, rows: u8, cols: u8) -> Result<Keymap, DeviceError> {
        let mut km = Keymap::new(layers, rows, cols);
        let size = layers as usize * rows as usize * cols as usize * 2;
        let mut bytes = Vec::with_capacity(size);
        while bytes.len() < size {
            let offset = bytes.len() as u16;
            let n = (size - bytes.len()).min(28) as u8;
            let o = offset.to_be_bytes();
            let r = self.via(&p::via_request(&[0x12, o[0], o[1], n]), Some(0x12))?;
            if r[1] != o[0] || r[2] != o[1] || r[3] != n {
                return Err(ProtocolError::Mismatch.into());
            }
            bytes.extend_from_slice(&r[4..4 + n as usize]);
        }
        let per_layer = rows as usize * cols as usize;
        for (i, pair) in bytes.chunks_exact(2).enumerate() {
            let (layer, pos) = (i / per_layer, i % per_layer);
            km.set(
                layer as u8,
                (pos / cols as usize) as u8,
                (pos % cols as usize) as u8,
                u16::from_be_bytes([pair[0], pair[1]]),
            );
        }
        Ok(km)
    }

    pub fn layer_count(&mut self) -> Result<u8, DeviceError> {
        let r = self.via(&p::layer_count_request(), Some(p::VIA_GET_LAYER_COUNT))?;
        let n = p::decode_layer_count(&r)?;
        if n == 0 || n > 32 {
            return Err(ProtocolError::Invalid("numero di layer").into());
        }
        Ok(n)
    }

    pub fn layout_options(&mut self) -> Result<u32, DeviceError> {
        let r = self.via(&p::layout_options_request(), Some(p::VIA_GET_KEYBOARD_VALUE))?;
        Ok(p::decode_layout_options(&r)?)
    }

    /// `(vial_protocol, keyboard uid)`.
    pub fn vial_id(&mut self) -> Result<(u32, u64), DeviceError> {
        let r = self.via(&p::vial_id_request(), None)?;
        Ok(p::decode_vial_id(&r))
    }

    pub fn definition_size(&mut self) -> Result<u32, DeviceError> {
        let r = self.via(&p::vial_size_request(), None)?;
        let size = p::decode_vial_size(&r);
        if size == 0 || size > MAX_DEFINITION_SIZE {
            return Err(ProtocolError::Invalid("dimensione della definizione").into());
        }
        Ok(size)
    }

    /// Downloads the compressed definition (`size` from [`Self::definition_size`]).
    pub fn definition_xz(&mut self, size: u32) -> Result<Vec<u8>, DeviceError> {
        let pages = size.div_ceil(p::REPORT_SIZE as u32);
        let mut data = Vec::with_capacity(pages as usize * p::REPORT_SIZE);
        for page in 0..pages {
            let page = u16::try_from(page).map_err(|_| ProtocolError::Invalid("pagina"))?;
            let r = self.via(&p::vial_def_request(page), None)?;
            data.extend_from_slice(&r);
        }
        data.truncate(size as usize);
        Ok(data)
    }
}
