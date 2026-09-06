//! OCR engine contract, model management, and local fast OCR implementation.
//!
//! OCR engines consume owned RGBA image data and return text blocks in stable image-space
//! coordinates. The desktop UI can therefore render OCR overlays independently from viewport
//! zoom/pan, and future engines such as GLM-OCR can implement the same contract.

mod fast;
mod models;

use std::{error::Error, fmt};

pub use fast::{
    FAST_ENGINE_ID, FAST_ENGINE_NAME, FAST_LANGUAGE_SUMMARY, FAST_MODEL_DOWNLOAD_SIZE,
    FAST_MODEL_VERSION, FastModelPaths, FastOcrEngine,
};
pub use models::{
    OcrModelDescriptor, OcrModelManager, OcrModelState, create_engine,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OcrPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OcrRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextBlock {
    pub text: String,
    pub bounds: OcrRect,
    pub polygon: Option<[OcrPoint; 4]>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrResult {
    pub plain_text: String,
    pub blocks: Vec<TextBlock>,
}

impl OcrResult {
    #[must_use]
    pub fn from_blocks(blocks: Vec<TextBlock>) -> Self {
        let plain_text = blocks
            .iter()
            .map(|block| block.text.as_str())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        Self { plain_text, blocks }
    }
}

#[derive(Debug, Clone)]
pub struct OcrImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl OcrImage {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, OcrError> {
        if width == 0 || height == 0 {
            return Err(OcrError::InvalidImage(
                "OCR image dimensions must be non-zero".to_owned(),
            ));
        }
        let expected = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| OcrError::InvalidImage("OCR image dimensions overflow".to_owned()))?
            as usize;
        if rgba.len() != expected {
            return Err(OcrError::InvalidImage(format!(
                "RGBA buffer length {} does not match {width}×{height}×4 ({expected})",
                rgba.len()
            )));
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrError {
    InvalidImage(String),
    Model(String),
    Download(String),
    Backend(String),
}

impl fmt::Display for OcrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidImage(message) => write!(f, "invalid OCR image: {message}"),
            Self::Model(message) => write!(f, "OCR model error: {message}"),
            Self::Download(message) => write!(f, "OCR model download error: {message}"),
            Self::Backend(message) => write!(f, "OCR backend error: {message}"),
        }
    }
}

impl Error for OcrError {}

pub trait OcrEngine {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn recognize(&mut self, input: &OcrImage) -> Result<OcrResult, OcrError>;
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

    fn recognize(&mut self, _input: &OcrImage) -> Result<OcrResult, OcrError> {
        Err(OcrError::Backend(
            "GLM-OCR is not configured yet".to_owned(),
        ))
    }
}

#[must_use]
pub fn validation_message() -> &'static str {
    "OCR models are managed explicitly in Settings; GLM-OCR remains a future optional backend."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_rgba_buffer_length() {
        assert!(OcrImage::new(2, 2, vec![0; 16]).is_ok());
        assert!(OcrImage::new(2, 2, vec![0; 15]).is_err());
        assert!(OcrImage::new(0, 2, Vec::new()).is_err());
    }

    #[test]
    fn plain_text_follows_block_order() {
        let blocks = vec![
            TextBlock {
                text: "你好 Azusa".to_owned(),
                bounds: OcrRect {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                },
                polygon: None,
                confidence: 0.95,
            },
            TextBlock {
                text: "Local OCR".to_owned(),
                bounds: OcrRect {
                    x: 1.0,
                    y: 8.0,
                    width: 5.0,
                    height: 4.0,
                },
                polygon: None,
                confidence: 0.91,
            },
        ];
        let result = OcrResult::from_blocks(blocks);
        assert_eq!(result.plain_text, "你好 Azusa\nLocal OCR");
    }
}
