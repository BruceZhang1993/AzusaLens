//! OCR engine contract, model management, and local OCR implementations.
//!
//! OCR engines consume owned RGBA image data and return text blocks in stable image-space
//! coordinates. The desktop UI can therefore render OCR overlays independently from viewport
//! zoom/pan while each backend remains isolated behind the same contract.

mod fast;
mod models;
mod ollama;
mod progress;
mod rotated;

use std::{error::Error, fmt, path::Path};

#[doc(hidden)]
pub use fast::FastOcrEngine as SinglePassFastOcrEngine;
pub use fast::{
    FAST_ENGINE_ID, FAST_ENGINE_NAME, FAST_LANGUAGE_SUMMARY, FAST_MODEL_DOWNLOAD_SIZE,
    FAST_MODEL_VERSION, FastModelPaths, PPOCR_MEDIUM_ENGINE_ID, PPOCR_MEDIUM_ENGINE_NAME,
    PPOCR_MEDIUM_LANGUAGE_SUMMARY, PPOCR_MEDIUM_MODEL_DOWNLOAD_SIZE, PPOCR_MEDIUM_MODEL_VERSION,
    PPOCR_SMALL_ENGINE_ID, PPOCR_SMALL_ENGINE_NAME, PPOCR_SMALL_LANGUAGE_SUMMARY,
    PPOCR_SMALL_MODEL_DOWNLOAD_SIZE, PPOCR_SMALL_MODEL_VERSION, PPOCR_TINY_ENGINE_ID,
    PPOCR_TINY_ENGINE_NAME, PPOCR_TINY_LANGUAGE_SUMMARY, PPOCR_TINY_MODEL_DOWNLOAD_SIZE,
    PPOCR_TINY_MODEL_VERSION, PpOcrTier,
};
pub use models::{OcrModelDescriptor, OcrModelManager, OcrModelState, create_engine};
pub use ollama::{
    DEEPSEEK_ENGINE_ID, DEEPSEEK_ENGINE_NAME, DEEPSEEK_LANGUAGE_SUMMARY,
    DEEPSEEK_MODEL_DOWNLOAD_SIZE, DEEPSEEK_MODEL_VERSION, GLM_ENGINE_ID, GLM_ENGINE_NAME,
    GLM_LANGUAGE_SUMMARY, GLM_MODEL_DOWNLOAD_SIZE, GLM_MODEL_VERSION, OllamaOcrEngine,
    OllamaOcrModel, install_ollama_model, install_ollama_model_with_progress,
    is_ollama_model_installed, remove_ollama_model,
};
pub use progress::{OcrDownloadCancellation, OcrModelDownloadProgress};
pub use rotated::FastOcrEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OcrTaskKind {
    Text,
    Document,
    Table,
    Figure,
}

impl OcrTaskKind {
    pub const ALL: [Self; 4] = [Self::Text, Self::Document, Self::Table, Self::Figure];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Document => "document",
            Self::Table => "table",
            Self::Figure => "figure",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "text" => Some(Self::Text),
            "document" => Some(Self::Document),
            "table" => Some(Self::Table),
            "figure" => Some(Self::Figure),
            _ => None,
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Text => "Text OCR",
            Self::Document => "Document to Markdown",
            Self::Table => "Table recognition",
            Self::Figure => "Figure recognition",
        }
    }

    #[must_use]
    pub const fn output_extension(self) -> &'static str {
        match self {
            Self::Text => "txt",
            Self::Document | Self::Table | Self::Figure => "md",
        }
    }
}

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

    pub fn open_path(path: &Path) -> Result<Self, OcrError> {
        let image = image::open(path).map_err(|error| {
            OcrError::InvalidImage(format!(
                "could not decode OCR input {}: {error}",
                path.display()
            ))
        })?;
        let rgba = image.to_rgba8();
        Self::new(rgba.width(), rgba.height(), rgba.into_raw())
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
    Cancelled(String),
    Backend(String),
}

impl fmt::Display for OcrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidImage(message) => write!(f, "invalid OCR image: {message}"),
            Self::Model(message) => write!(f, "OCR model error: {message}"),
            Self::Download(message) => write!(f, "OCR model download error: {message}"),
            Self::Cancelled(message) => write!(f, "OCR operation cancelled: {message}"),
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

    fn recognize_task(
        &mut self,
        task: OcrTaskKind,
        input: &OcrImage,
    ) -> Result<OcrResult, OcrError> {
        if task == OcrTaskKind::Text {
            self.recognize(input)
        } else {
            Err(OcrError::Model(format!(
                "{} does not support {}",
                self.display_name(),
                task.display_name()
            )))
        }
    }
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
    fn task_ids_round_trip() {
        for task in OcrTaskKind::ALL {
            assert_eq!(OcrTaskKind::from_value(task.as_str()), Some(task));
        }
        assert_eq!(OcrTaskKind::from_value("unknown"), None);
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
