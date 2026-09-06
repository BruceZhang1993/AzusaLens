use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use image::{DynamicImage, RgbaImage};
use ocr_rs::{OcrEngine as PaddleOcrEngine, OcrEngineConfig};

use crate::{OcrEngine, OcrError, OcrImage, OcrPoint, OcrRect, OcrResult, TextBlock};

pub const FAST_ENGINE_ID: &str = "ppocrv6-small-mnn";
pub const FAST_ENGINE_NAME: &str = "PP-OCRv6 Small · local multilingual";
pub const FAST_MODEL_VERSION: &str = "ppocrv6-small-ocr-rs-v2.4.1";
pub const FAST_LANGUAGE_SUMMARY: &str =
    "Simplified/Traditional Chinese, English, Japanese and 46 Latin-script languages";
pub const FAST_MODEL_DOWNLOAD_SIZE: u64 = 15_611_984;

const MODEL_BASE_URL: &str =
    "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/v2.4.1/models";
const DET_MODEL_NAME: &str = "PP-OCRv6_small_det.mnn";
const REC_MODEL_NAME: &str = "PP-OCRv6_small_rec.mnn";
const CHARSET_NAME: &str = "ppocr_keys_v6_small.txt";
const DET_MODEL_SIZE: u64 = 4_965_224;
const REC_MODEL_SIZE: u64 = 10_646_760;
const MIN_CHARSET_SIZE: u64 = 1_024;

#[derive(Debug, Clone)]
pub struct FastModelPaths {
    pub directory: PathBuf,
    pub detection: PathBuf,
    pub recognition: PathBuf,
    pub charset: PathBuf,
}

impl FastModelPaths {
    #[must_use]
    pub fn discover() -> Self {
        let directory = std::env::var_os("AZUSAOCR_OCR_MODEL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(default_model_directory);
        Self::from_directory(directory)
    }

    #[must_use]
    pub fn from_directory(directory: PathBuf) -> Self {
        Self {
            detection: directory.join(DET_MODEL_NAME),
            recognition: directory.join(REC_MODEL_NAME),
            charset: directory.join(CHARSET_NAME),
            directory,
        }
    }

    #[must_use]
    pub fn are_ready(&self) -> bool {
        file_has_size(&self.detection, Some(DET_MODEL_SIZE))
            && file_has_size(&self.recognition, Some(REC_MODEL_SIZE))
            && file_has_size(&self.charset, None)
    }

    pub fn install(&self) -> Result<(), OcrError> {
        fs::create_dir_all(&self.directory).map_err(|error| {
            OcrError::Model(format!(
                "failed to create OCR model directory {}: {error}",
                self.directory.display()
            ))
        })?;

        ensure_model_file(
            &self.detection,
            DET_MODEL_NAME,
            Some(DET_MODEL_SIZE),
            DET_MODEL_SIZE,
        )?;
        ensure_model_file(
            &self.recognition,
            REC_MODEL_NAME,
            Some(REC_MODEL_SIZE),
            REC_MODEL_SIZE,
        )?;
        ensure_model_file(&self.charset, CHARSET_NAME, None, MIN_CHARSET_SIZE)?;
        Ok(())
    }

    pub fn remove(&self) -> Result<(), OcrError> {
        if !self.directory.exists() {
            return Ok(());
        }
        fs::remove_dir_all(&self.directory).map_err(|error| {
            OcrError::Model(format!(
                "failed to remove OCR model directory {}: {error}",
                self.directory.display()
            ))
        })
    }
}

pub struct FastOcrEngine {
    model_paths: FastModelPaths,
    runtime: Option<PaddleOcrEngine>,
}

impl Default for FastOcrEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FastOcrEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            model_paths: FastModelPaths::discover(),
            runtime: None,
        }
    }

    #[must_use]
    pub fn model_paths(&self) -> &FastModelPaths {
        &self.model_paths
    }

    fn runtime(&mut self) -> Result<&mut PaddleOcrEngine, OcrError> {
        if !self.model_paths.are_ready() {
            return Err(OcrError::Model(
                "PP-OCRv6 Small is not installed; download and enable it in Settings > OCR models"
                    .to_owned(),
            ));
        }

        if self.runtime.is_none() {
            let threads = std::thread::available_parallelism()
                .map(|value| value.get().clamp(1, 4) as i32)
                .unwrap_or(4);
            let config = OcrEngineConfig::fast()
                .with_threads(threads)
                .with_min_result_confidence(0.45);
            let engine = PaddleOcrEngine::new(
                &self.model_paths.detection,
                &self.model_paths.recognition,
                &self.model_paths.charset,
                Some(config),
            )
            .map_err(|error| {
                OcrError::Backend(format!("failed to initialize PP-OCRv6: {error}"))
            })?;
            self.runtime = Some(engine);
        }
        self.runtime
            .as_mut()
            .ok_or_else(|| OcrError::Backend("OCR runtime was not initialized".to_owned()))
    }
}

impl OcrEngine for FastOcrEngine {
    fn id(&self) -> &'static str {
        FAST_ENGINE_ID
    }

    fn display_name(&self) -> &'static str {
        FAST_ENGINE_NAME
    }

    fn is_available(&self) -> bool {
        self.model_paths.are_ready()
    }

    fn recognize(&mut self, input: &OcrImage) -> Result<OcrResult, OcrError> {
        let rgba = RgbaImage::from_raw(input.width(), input.height(), input.rgba().to_vec())
            .ok_or_else(|| OcrError::InvalidImage("invalid RGBA buffer dimensions".to_owned()))?;
        let image = DynamicImage::ImageRgba8(rgba);
        let results = self
            .runtime()?
            .recognize(&image)
            .map_err(|error| OcrError::Backend(format!("PP-OCRv6 inference failed: {error}")))?;

        let blocks = results
            .into_iter()
            .filter_map(|result| {
                let text = result.text.trim().to_owned();
                if text.is_empty() {
                    return None;
                }
                let rect = result.bbox.rect;
                let polygon = result.bbox.points.map(|points| {
                    points.map(|point| OcrPoint {
                        x: point.x,
                        y: point.y,
                    })
                });
                Some(TextBlock {
                    text,
                    bounds: OcrRect {
                        x: rect.left().max(0) as f32,
                        y: rect.top().max(0) as f32,
                        width: rect.width() as f32,
                        height: rect.height() as f32,
                    },
                    polygon,
                    confidence: result.confidence.clamp(0.0, 1.0),
                })
            })
            .collect();

        Ok(OcrResult::from_blocks(blocks))
    }
}

fn default_model_directory() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("AzusaOCR")
        .join("ocr")
        .join(FAST_MODEL_VERSION)
}

fn file_has_size(path: &Path, expected: Option<u64>) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    match expected {
        Some(expected) => metadata.len() == expected,
        None => metadata.len() >= MIN_CHARSET_SIZE,
    }
}

fn ensure_model_file(
    path: &Path,
    file_name: &str,
    expected_size: Option<u64>,
    minimum_size: u64,
) -> Result<(), OcrError> {
    if file_has_size(path, expected_size) {
        return Ok(());
    }

    let url = format!("{MODEL_BASE_URL}/{file_name}");
    let partial = path.with_extension(format!(
        "{}.part",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("download")
    ));
    let _ = fs::remove_file(&partial);

    if let Err(error) = download_to(&url, &partial) {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    let downloaded_size = fs::metadata(&partial)
        .map_err(|error| OcrError::Download(format!("cannot inspect downloaded model: {error}")))?
        .len();
    let size_is_valid = expected_size
        .map(|expected| downloaded_size == expected)
        .unwrap_or(downloaded_size >= minimum_size);
    if !size_is_valid {
        let _ = fs::remove_file(&partial);
        return Err(OcrError::Download(format!(
            "downloaded {file_name} has unexpected size {downloaded_size} bytes"
        )));
    }

    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            OcrError::Model(format!("failed to replace {}: {error}", path.display()))
        })?;
    }
    fs::rename(&partial, path).map_err(|error| {
        OcrError::Model(format!(
            "failed to install OCR model {}: {error}",
            path.display()
        ))
    })?;
    Ok(())
}

fn download_to(url: &str, destination: &Path) -> Result<(), OcrError> {
    let response = ureq::get(url)
        .set("User-Agent", "AzusaOCR/0.1")
        .call()
        .map_err(|error| OcrError::Download(format!("model download failed: {error}")))?;
    let mut reader = response.into_reader();
    let mut file = File::create(destination).map_err(|error| {
        OcrError::Download(format!(
            "cannot create model download {}: {error}",
            destination.display()
        ))
    })?;
    io::copy(&mut reader, &mut file)
        .map_err(|error| OcrError::Download(format!("model download interrupted: {error}")))?;
    file.flush()
        .map_err(|error| OcrError::Download(format!("cannot flush model file: {error}")))?;
    file.sync_all()
        .map_err(|error| OcrError::Download(format!("cannot sync model file: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_manifest_is_pinned() {
        assert!(MODEL_BASE_URL.contains("v2.4.1"));
        assert_eq!(DET_MODEL_SIZE, 4_965_224);
        assert_eq!(REC_MODEL_SIZE, 10_646_760);
        assert_eq!(FAST_MODEL_DOWNLOAD_SIZE, DET_MODEL_SIZE + REC_MODEL_SIZE);
        assert!(FAST_LANGUAGE_SUMMARY.contains("Chinese"));
        assert!(FAST_LANGUAGE_SUMMARY.contains("English"));
    }

    #[test]
    fn invalid_model_sizes_are_rejected() {
        let directory =
            std::env::temp_dir().join(format!("azusaocr-ocr-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let file = directory.join(DET_MODEL_NAME);
        fs::write(&file, b"not a model").unwrap();
        assert!(!file_has_size(&file, Some(DET_MODEL_SIZE)));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn recognition_never_auto_installs_missing_models() {
        let directory = std::env::temp_dir().join(format!(
            "azusaocr-ocr-test-{}-no-auto-download",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let mut engine = FastOcrEngine {
            model_paths: FastModelPaths::from_directory(directory.clone()),
            runtime: None,
        };
        let image = OcrImage::new(1, 1, vec![255; 4]).unwrap();

        let error = engine.recognize(&image).unwrap_err();

        assert!(matches!(error, OcrError::Model(_)));
        assert!(!directory.exists());
    }
}
