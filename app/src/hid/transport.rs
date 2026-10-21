//! Raw HID transport (hidapi) behind a small trait, so that the protocol
//! code also runs against the simulated keyboard (`--mock`).

use super::protocol::{Report, REPORT_SIZE};
use hidapi::{HidApi, HidDevice};
use std::ffi::CString;
use std::fmt;
use std::time::Duration;

pub const RAW_USAGE_PAGE: u16 = 0xFF60;
pub const RAW_USAGE: u16 = 0x61;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// Linux: the hidraw node is not accessible (missing udev rule).
    PermissionDenied(String),
    Io(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::PermissionDenied(path) => write!(f, "permesso negato su {path}"),
            TransportError::Io(e) => write!(f, "errore HID: {e}"),
        }
    }
}

impl std::error::Error for TransportError {}

pub trait Transport: Send {
    fn send(&mut self, report: &Report) -> Result<(), TransportError>;
    /// Next input report, or `None` after `timeout`.
    fn recv(&mut self, timeout: Duration) -> Result<Option<Report>, TransportError>;

    /// Discards input reports still queued (e.g. answers to timed-out requests).
    fn flush(&mut self) -> Result<(), TransportError> {
        for _ in 0..64 {
            if self.recv(Duration::ZERO)?.is_none() {
                break;
            }
        }
        Ok(())
    }
}

/// A Raw HID interface found during enumeration.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub path: CString,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product: String,
}

/// Lists the Raw HID interfaces (usage page 0xFF60, usage 0x61), optionally
/// filtered by VID/PID.
pub fn enumerate(api: &mut HidApi, vid: Option<u16>, pid: Option<u16>) -> Result<Vec<Candidate>, TransportError> {
    api.refresh_devices().map_err(|e| TransportError::Io(e.to_string()))?;
    Ok(api
        .device_list()
        .filter(|d| d.usage_page() == RAW_USAGE_PAGE && d.usage() == RAW_USAGE)
        .filter(|d| vid.is_none_or(|v| v == d.vendor_id()) && pid.is_none_or(|p| p == d.product_id()))
        .map(|d| Candidate {
            path: d.path().to_owned(),
            vendor_id: d.vendor_id(),
            product_id: d.product_id(),
            product: d.product_string().unwrap_or("").to_string(),
        })
        .collect())
}

fn is_permission_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("permission denied") || m.contains("os error 13") || m.contains("access is denied")
}

pub struct HidTransport {
    dev: HidDevice,
}

impl HidTransport {
    pub fn open(api: &HidApi, candidate: &Candidate) -> Result<Self, TransportError> {
        api.open_path(&candidate.path).map(|dev| HidTransport { dev }).map_err(|e| {
            let msg = e.to_string();
            if is_permission_error(&msg) {
                TransportError::PermissionDenied(candidate.path.to_string_lossy().into_owned())
            } else {
                TransportError::Io(msg)
            }
        })
    }
}

impl Transport for HidTransport {
    fn send(&mut self, report: &Report) -> Result<(), TransportError> {
        // Report ID 0x00 in front: required by hidapi on every platform for
        // devices without numbered reports (33 bytes in total).
        let mut buf = [0u8; REPORT_SIZE + 1];
        buf[1..].copy_from_slice(report);
        match self.dev.write(&buf) {
            Ok(n) if n >= REPORT_SIZE => Ok(()),
            Ok(n) => Err(TransportError::Io(format!("scrittura incompleta ({n} byte)"))),
            Err(e) => Err(TransportError::Io(e.to_string())),
        }
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<Report>, TransportError> {
        let mut buf = [0u8; REPORT_SIZE + 1];
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        match self.dev.read_timeout(&mut buf, ms) {
            Ok(0) => Ok(None),
            Ok(n) if n >= REPORT_SIZE => {
                let mut r = [0u8; REPORT_SIZE];
                r.copy_from_slice(&buf[..REPORT_SIZE]);
                Ok(Some(r))
            }
            // short reports are not ours: drop them
            Ok(_) => Ok(None),
            Err(e) => Err(TransportError::Io(e.to_string())),
        }
    }
}

/// udev rule for Linux, same as the one recommended by Vial.
pub const UDEV_RULE: &str = r#"KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{serial}=="*vial:f64c2b3c*", MODE="0660", GROUP="users", TAG+="uaccess", TAG+="udev-acl""#;

pub fn udev_instructions() -> String {
    format!(
        "# Salva questa riga in /etc/udev/rules.d/59-vial.rules:\n{UDEV_RULE}\n\n\
         # Poi ricarica le regole e ricollega la tastiera:\n\
         # sudo udevadm control --reload && sudo udevadm trigger\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_errors_are_recognized() {
        assert!(is_permission_error("Permission denied (os error 13)"));
        assert!(is_permission_error("Access is denied."));
        assert!(!is_permission_error("No such device"));
    }
}
