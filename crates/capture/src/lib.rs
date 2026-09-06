//! Cross-platform capture contract and region-capture routing.
//!
//! Windows, macOS, and X11 capture a frozen display frame and let AzusaOCR's
//! own overlay choose the final rectangle. Native Wayland uses the XDG
//! Screenshot portal with the `Area` target because compositors intentionally
//! prevent applications from reading arbitrary desktop pixels behind an
//! overlay.

use std::{error::Error, fmt, path::Path};

use xcap::Monitor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackendKind {
    WindowsGraphicsCapture,
    MacOsNative,
    WaylandPortal,
    X11,
    Unsupported,
}

impl fmt::Display for CaptureBackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::WindowsGraphicsCapture => "Windows / WGC",
            Self::MacOsNative => "macOS native capture",
            Self::WaylandPortal => "Wayland XDG Screenshot portal",
            Self::X11 => "X11 native capture",
            Self::Unsupported => "Unsupported platform",
        };
        f.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl CaptureRect {
    #[must_use]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn clamped_to(self, frame_width: u32, frame_height: u32) -> Option<Self> {
        let x = self.x.min(frame_width);
        let y = self.y.min(frame_height);
        let right = self.x.saturating_add(self.width).min(frame_width);
        let bottom = self.y.saturating_add(self.height).min(frame_height);
        let width = right.saturating_sub(x);
        let height = bottom.saturating_sub(y);

        (width > 0 && height > 0).then_some(Self {
            x,
            y,
            width,
            height,
        })
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

    pub fn crop(&self, rect: CaptureRect) -> Result<Self, CaptureError> {
        let rect = rect
            .clamped_to(self.width, self.height)
            .ok_or(CaptureError::InvalidRegion(rect))?;
        let row_bytes = rect.width as usize * 4;
        let mut rgba = Vec::with_capacity(row_bytes * rect.height as usize);

        for row in rect.y..rect.y + rect.height {
            let start = ((row as usize * self.width as usize) + rect.x as usize) * 4;
            rgba.extend_from_slice(&self.rgba[start..start + row_bytes]);
        }

        Self::new(rect.width, rect.height, rgba)
    }

    pub fn save_png(&self, path: &Path) -> Result<(), CaptureError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(CaptureError::Io)?;
        }

        let image = image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone()).ok_or(
            CaptureError::InvalidFrame {
                width: self.width,
                height: self.height,
                actual_len: self.rgba.len(),
                expected_len: self.width as usize * self.height as usize * 4,
            },
        )?;

        image
            .save_with_format(path, image::ImageFormat::Png)
            .map_err(CaptureError::Image)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionCapture {
    /// The caller must display this frozen frame and ask the user to select a
    /// rectangle. Used on Windows, macOS, and X11.
    NeedsSelection(CapturedFrame),
    /// The platform already performed an interactive region selection. Used by
    /// the native Wayland screenshot portal.
    Selected(CapturedFrame),
}

#[derive(Debug)]
pub enum CaptureError {
    Backend(String),
    Portal(String),
    NoMonitor,
    InvalidFrame {
        width: u32,
        height: u32,
        actual_len: usize,
        expected_len: usize,
    },
    InvalidRegion(CaptureRect),
    Io(std::io::Error),
    Image(image::ImageError),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(message) => write!(f, "capture backend failed: {message}"),
            Self::Portal(message) => write!(f, "Wayland portal capture failed: {message}"),
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
            Self::InvalidRegion(rect) => write!(
                f,
                "invalid capture region {},{} {}x{}",
                rect.x, rect.y, rect.width, rect.height
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

pub fn capture_primary_monitor() -> Result<CapturedFrame, CaptureError> {
    XcapCaptureBackend.capture_primary_monitor()
}

pub fn begin_region_capture() -> Result<RegionCapture, CaptureError> {
    #[cfg(target_os = "linux")]
    if matches!(detected_backend(), CaptureBackendKind::WaylandPortal) {
        return capture_wayland_region().map(RegionCapture::Selected);
    }

    capture_primary_monitor().map(RegionCapture::NeedsSelection)
}

#[cfg(target_os = "linux")]
fn capture_wayland_region() -> Result<CapturedFrame, CaptureError> {
    use ashpd::desktop::screenshot::{AvailableTargets, Screenshot};

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .map_err(|error| {
            CaptureError::Portal(format!("failed to start portal runtime: {error}"))
        })?;

    let response = runtime
        .block_on(async {
            Screenshot::request()
                .interactive(true)
                .modal(false)
                .target(AvailableTargets::Area)
                .send()
                .await?
                .response()
        })
        .map_err(|error| CaptureError::Portal(error.to_string()))?;

    let uri = url::Url::parse(response.uri().as_str())
        .map_err(|error| CaptureError::Portal(format!("invalid screenshot URI: {error}")))?;
    let path = uri
        .to_file_path()
        .map_err(|()| CaptureError::Portal("portal returned a non-file screenshot URI".into()))?;
    let image = image::open(&path).map_err(CaptureError::Image)?.to_rgba8();

    CapturedFrame::new(image.width(), image.height(), image.into_raw())
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

    #[test]
    fn region_is_clamped_to_frame() {
        let rect = CaptureRect::new(2, 1, 4, 4)
            .clamped_to(4, 3)
            .expect("region should overlap frame");
        assert_eq!(rect, CaptureRect::new(2, 1, 2, 2));
    }

    #[test]
    fn crop_preserves_expected_pixels() {
        let mut rgba = Vec::new();
        for pixel in 0_u8..16 {
            rgba.extend_from_slice(&[pixel, pixel, pixel, 255]);
        }
        let frame = CapturedFrame::new(4, 4, rgba).expect("valid frame");
        let cropped = frame
            .crop(CaptureRect::new(1, 1, 2, 2))
            .expect("valid crop");

        assert_eq!(cropped.width(), 2);
        assert_eq!(cropped.height(), 2);
        assert_eq!(cropped.rgba()[0], 5);
        assert_eq!(cropped.rgba()[4], 6);
        assert_eq!(cropped.rgba()[8], 9);
        assert_eq!(cropped.rgba()[12], 10);
    }
}
