//! Platform capture abstraction.
//!
//! This crate intentionally contains only backend detection and contracts in the
//! validation application. Native implementations are added behind this API.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackendKind {
    WindowsGraphicsCapture,
    ScreenCaptureKit,
    WaylandPortal,
    X11,
    Unsupported,
}

impl fmt::Display for CaptureBackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::WindowsGraphicsCapture => "Windows Graphics Capture",
            Self::ScreenCaptureKit => "ScreenCaptureKit",
            Self::WaylandPortal => "Wayland / XDG Portal",
            Self::X11 => "X11",
            Self::Unsupported => "Unsupported platform",
        };
        f.write_str(name)
    }
}

pub trait CaptureBackend: Send + Sync {
    fn kind(&self) -> CaptureBackendKind;
    fn is_available(&self) -> bool;
}

#[cfg(target_os = "windows")]
#[must_use]
pub const fn detected_backend() -> CaptureBackendKind {
    CaptureBackendKind::WindowsGraphicsCapture
}

#[cfg(target_os = "macos")]
#[must_use]
pub const fn detected_backend() -> CaptureBackendKind {
    CaptureBackendKind::ScreenCaptureKit
}

#[cfg(target_os = "linux")]
#[must_use]
pub fn detected_backend() -> CaptureBackendKind {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        CaptureBackendKind::WaylandPortal
    } else if std::env::var_os("DISPLAY").is_some() {
        CaptureBackendKind::X11
    } else {
        CaptureBackendKind::Unsupported
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
#[must_use]
pub const fn detected_backend() -> CaptureBackendKind {
    CaptureBackendKind::Unsupported
}

#[must_use]
pub fn validation_message() -> String {
    format!(
        "Capture contract is wired. Detected backend: {}",
        detected_backend()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_name_is_not_empty() {
        assert!(!detected_backend().to_string().is_empty());
    }
}
