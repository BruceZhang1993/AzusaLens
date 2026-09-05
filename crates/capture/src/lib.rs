//! Cross-platform capture contract and the first working MVP adapter.
//!
//! `XcapCaptureBackend` is intentionally isolated behind this crate's public
//! API. It gives the validation application a real capture path now while
//! preserving the boundary required to replace each platform with dedicated
//! Windows Graphics Capture, ScreenCaptureKit, Wayland portal/PipeWire, and X11
//! implementations later.

use std::{error::Error, fmt, path::Path};

use xcap::Monitor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackendKind {
    WindowsGraphicsCapture,
    MacOsNative,
    WaylandNative,
    X11,
    Unsupported,
}

impl fmt::Display for CaptureBackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::WindowsGraphicsCapture => "Windows / WGC (MVP adapter)",
            Self::MacOsNative => "macOS native capture (MVP adapter)",
            Self::WaylandNative => "Wayland native capture (MVP adapter)",
            Self::X11 => "X11 capture (MVP adapter)",
            Self::Unsupported => "Unsupported platform",
        };
        f.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl CapturedFrame {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, CaptureError> {
        let expected_len = width as usize * height as usize * 4;
        if width == 0 || height == 0 || rgba.len() != expected_len {
            return Err(CaptureError::InvalidFrame {
                width,
                height,
                actual_len: rgba.len(),
                expected_len,
            });
        }

        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn save_png(&self, path: &Path) -> Result<(), CaptureError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(CaptureError::Io)?;
        }

        let image = image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
            .ok_or(CaptureError::InvalidFrame {
                width: self.width,
                height: self.height,
                actual_len: self.rgba.len(),
                expected_len: self.width as usize * self.height as usize * 4,
            })?;

        image
            .save_with_format(path, image::ImageFormat::Png)
            .map_err(CaptureError::Image)
    }
}

#[derive(Debug)]
pub enum CaptureError {
    Backend(String),
    NoMonitor,
    InvalidFrame {
        width: u32,
        height: u32,
        actual_len: usize,
        expected_len: usize,
    },
    Io(std::io::Error),
    Image(image::ImageError),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(message) => write!(f, "capture backend failed: {message}"),
            Self::NoMonitor => f.write_str("no monitor is available for capture"),
            Self::InvalidFrame {
                width,
                height,
                actual_len,
                expected_len,
            } => write!(
                f,
                "invalid RGBA frame {width}x{height}: got {actual_len} bytes, expected {expected_len}"
            ),
            Self::Io(error) => write!(f, "failed to write capture: {error}"),
            Self::Image(error) => write!(f, "failed to encode capture: {error}"),
        }
    }
}

impl Error for CaptureError {}

pub trait CaptureBackend: Send + Sync {
    fn kind(&self) -> CaptureBackendKind;
    fn is_available(&self) -> bool;
    fn capture_primary_monitor(&self) -> Result<CapturedFrame, CaptureError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct XcapCaptureBackend;

impl CaptureBackend for XcapCaptureBackend {
    fn kind(&self) -> CaptureBackendKind {
        detected_backend()
    }

    fn is_available(&self) -> bool {
        !matches!(self.kind(), CaptureBackendKind::Unsupported)
    }

    fn capture_primary_monitor(&self) -> Result<CapturedFrame, CaptureError> {
        if !self.is_available() {
            return Err(CaptureError::Backend(
                "the current operating system has no configured capture backend".into(),
            ));
        }

        let monitors = Monitor::all().map_err(|error| CaptureError::Backend(error.to_string()))?;
        if monitors.is_empty() {
            return Err(CaptureError::NoMonitor);
        }

        let primary_index = monitors
            .iter()
            .position(|monitor| monitor.is_primary().unwrap_or(false))
            .unwrap_or(0);
        let monitor = monitors
            .into_iter()
            .nth(primary_index)
            .ok_or(CaptureError::NoMonitor)?;

        let image = monitor
            .capture_image()
            .map_err(|error| CaptureError::Backend(error.to_string()))?;

        CapturedFrame::new(image.width(), image.height(), image.into_raw())
    }
}

#[cfg(target_os = "windows")]
#[must_use]
pub const fn detected_backend() -> CaptureBackendKind {
    CaptureBackendKind::WindowsGraphicsCapture
}

#[cfg(target_os = "macos")]
#[must_use]
pub const fn detected_backend() -> CaptureBackendKind {
    CaptureBackendKind::MacOsNative
}

#[cfg(target_os = "linux")]
#[must_use]
pub fn detected_backend() -> CaptureBackendKind {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        CaptureBackendKind::WaylandNative
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

pub fn capture_primary_monitor() -> Result<CapturedFrame, CaptureError> {
    XcapCaptureBackend.capture_primary_monitor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_name_is_not_empty() {
        assert!(!detected_backend().to_string().is_empty());
    }

    #[test]
    fn frame_rejects_wrong_rgba_length() {
        let error = CapturedFrame::new(2, 2, vec![0; 15]).expect_err("frame must be rejected");
        assert!(matches!(error, CaptureError::InvalidFrame { .. }));
    }

    #[test]
    fn frame_accepts_rgba_length() {
        let frame = CapturedFrame::new(2, 2, vec![0; 16]).expect("frame must be valid");
        assert_eq!(frame.width(), 2);
        assert_eq!(frame.height(), 2);
        assert_eq!(frame.rgba().len(), 16);
    }
}
