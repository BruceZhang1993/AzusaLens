//! Shared application-domain types for Azusa Lens.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Ready,
    Capturing,
    Recognizing,
}

impl AppState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Capturing => "Capturing",
            Self::Recognizing => "Recognizing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityStatus {
    pub capture: bool,
    pub ocr: bool,
    pub annotation: bool,
}

impl Default for CapabilityStatus {
    fn default() -> Self {
        Self {
            capture: true,
            ocr: false,
            annotation: true,
        }
    }
}
