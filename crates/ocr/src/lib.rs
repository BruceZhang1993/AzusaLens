//! OCR engine contract.
//!
//! GLM-OCR and a fast OCR backend will implement this interface later.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrResult {
    pub plain_text: String,
}

pub trait OcrEngine: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn is_available(&self) -> bool;
}

pub struct ValidationOcrEngine;

impl OcrEngine for ValidationOcrEngine {
    fn id(&self) -> &'static str {
        "validation"
    }

    fn display_name(&self) -> &'static str {
        "GLM-OCR (not configured)"
    }

    fn is_available(&self) -> bool {
        false
    }
}

#[must_use]
pub fn default_engine_name() -> &'static str {
    ValidationOcrEngine.display_name()
}

#[must_use]
pub fn validation_message() -> &'static str {
    "OCR contract is wired. Local model integration is the next milestone."
}
