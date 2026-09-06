use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use image::{DynamicImage, RgbaImage};
use ocr_rs::{OcrEngine as PaddleOcrEngine, OcrEngineConfig};

use crate::{
    OcrDownloadCancellation, OcrEngine, OcrError, OcrImage, OcrModelDownloadProgress, OcrPoint,
    OcrRect, OcrResult, TextBlock,
};

pub const PPOCR_TINY_ENGINE_ID: &str = "ppocrv6-tiny-mnn";
pub const PPOCR_TINY_ENGINE_NAME: &str = "PP-OCRv6 Tiny · fastest local";
pub const PPOCR_TINY_MODEL_VERSION: &str = "ppocrv6-tiny-ocr-rs-v2.4.1";
pub const PPOCR_TINY_LANGUAGE_SUMMARY: &str =
    "Simplified/Traditional Chinese, English and 46 Latin-script languages; no Japanese";
pub const PPOCR_TINY_MODEL_DOWNLOAD_SIZE: u64 = 3_153_512;

pub const PPOCR_SMALL_ENGINE_ID: &str = "ppocrv6-small-mnn";
pub const PPOCR_SMALL_ENGINE_NAME: &str = "PP-OCRv6 Small · balanced local";
pub const PPOCR_SMALL_MODEL_VERSION: &str = "ppocrv6-small-ocr-rs-v2.4.1";
pub const PPOCR_SMALL_LANGUAGE_SUMMARY: &str =
    "Simplified/Traditional Chinese, English, Japanese and 46 Latin-script languages";
pub const PPOCR_SMALL_MODEL_DOWNLOAD_SIZE: u64 = 15_611_984;

pub const PPOCR_MEDIUM_ENGINE_ID: &str = "ppocrv6-medium-mnn";
pub const PPOCR_MEDIUM_ENGINE_NAME: &str = "PP-OCRv6 Medium · accuracy inference";
pub const PPOCR_MEDIUM_MODEL_VERSION: &str = "ppocrv6-medium-inference-ocr-rs-v2.4.1";
pub const PPOCR_MEDIUM_LANGUAGE_SUMMARY: &str =
    "Simplified/Traditional Chinese, English, Japanese and 46 Latin-script languages";
pub const PPOCR_MEDIUM_MODEL_DOWNLOAD_SIZE: u64 = 69_460_824;

// Backward-compatible aliases for callers that previously treated Small as the only fast model.
pub const FAST_ENGINE_ID: &str = PPOCR_SMALL_ENGINE_ID;
pub const FAST_ENGINE_NAME: &str = PPOCR_SMALL_ENGINE_NAME;
pub const FAST_MODEL_VERSION: &str = PPOCR_SMALL_MODEL_VERSION;
pub const FAST_LANGUAGE_SUMMARY: &str = PPOCR_SMALL_LANGUAGE_SUMMARY;
pub const FAST_MODEL_DOWNLOAD_SIZE: u64 = PPOCR_SMALL_MODEL_DOWNLOAD_SIZE;

const MODEL_BASE_URL: &str =
    "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/v2.4.1/models";
const MIN_CHARSET_SIZE: u64 = 1_024;
const DOWNLOAD_BUFFER_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpOcrTier {
    Tiny,
    Small,
    Medium,
}

impl PpOcrTier {
    #[must_use]
    pub fn from_engine_id(model_id: &str) -> Option<Self> {
        match model_id {
            PPOCR_TINY_ENGINE_ID => Some(Self::Tiny),
            PPOCR_SMALL_ENGINE_ID => Some(Self::Small),
            PPOCR_MEDIUM_ENGINE_ID => Some(Self::Medium),
            _ => None,
        }
    }

    #[must_use]
    pub const fn engine_id(self) -> &'static str {
        match self {
            Self::Tiny => PPOCR_TINY_ENGINE_ID,
            Self::Small => PPOCR_SMALL_ENGINE_ID,
            Self::Medium => PPOCR_MEDIUM_ENGINE_ID,
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Tiny => PPOCR_TINY_ENGINE_NAME,
            Self::Small => PPOCR_SMALL_ENGINE_NAME,
            Self::Medium => PPOCR_MEDIUM_ENGINE_NAME,
        }
    }

    #[must_use]
    pub const fn model_version(self) -> &'static str {
        match self {
            Self::Tiny => PPOCR_TINY_MODEL_VERSION,
            Self::Small => PPOCR_SMALL_MODEL_VERSION,
            Self::Medium => PPOCR_MEDIUM_MODEL_VERSION,
        }
    }

    #[must_use]
    pub const fn language_summary(self) -> &'static str {
        match self {
            Self::Tiny => PPOCR_TINY_LANGUAGE_SUMMARY,
            Self::Small => PPOCR_SMALL_LANGUAGE_SUMMARY,
            Self::Medium => PPOCR_MEDIUM_LANGUAGE_SUMMARY,
        }
    }

    #[must_use]
    pub const fn download_size(self) -> u64 {
        match self {
            Self::Tiny => PPOCR_TINY_MODEL_DOWNLOAD_SIZE,
            Self::Small => PPOCR_SMALL_MODEL_DOWNLOAD_SIZE,
            Self::Medium => PPOCR_MEDIUM_MODEL_DOWNLOAD_SIZE,
        }
    }

    const fn detection_model_name(self) -> &'static str {
        match self {
            Self::Tiny => "PP-OCRv6_tiny_det.mnn",
            Self::Small => "PP-OCRv6_small_det.mnn",
            Self::Medium => "PP-OCRv6_medium_det.mnn",
        }
    }

    const fn recognition_model_name(self) -> &'static str {
        match self {
            Self::Tiny => "PP-OCRv6_tiny_rec.mnn",
            Self::Small => "PP-OCRv6_small_rec.mnn",
            Self::Medium => "PP-OCRv6_medium_rec.mnn",
        }
    }

    const fn charset_name(self) -> &'static str {
        match self {
            Self::Tiny => "ppocr_keys_v6_tiny.txt",
            Self::Small => "ppocr_keys_v6_small.txt",
            Self::Medium => "ppocr_keys_v6_medium.txt",
        }
    }

    const fn detection_model_size(self) -> u64 {
        match self {
            Self::Tiny => 901_896,
            Self::Small => 4_965_224,
            Self::Medium => 31_078_716,
        }
    }

    const fn recognition_model_size(self) -> u64 {
        match self {
            Self::Tiny => 2_251_616,
            Self::Small => 10_646_760,
            Self::Medium => 38_382_108,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FastModelPaths {
    pub tier: PpOcrTier,
    pub directory: PathBuf,
    pub detection: PathBuf,
    pub recognition: PathBuf,
    pub charset: PathBuf,
}

impl FastModelPaths {
    #[must_use]
    pub fn discover() -> Self {
        Self::discover_for(PpOcrTier::Small)
    }

    #[must_use]
    pub fn discover_for(tier: PpOcrTier) -> Self {
        let directory = std::env::var_os("AZUSA_LENS_OCR_MODEL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| default_model_directory(tier));
        Self::from_directory_for(directory, tier)
    }

    #[must_use]
    pub fn from_directory(directory: PathBuf) -> Self {
        Self::from_directory_for(directory, PpOcrTier::Small)
    }

    #[must_use]
    pub fn from_directory_for(directory: PathBuf, tier: PpOcrTier) -> Self {
        Self {
            detection: directory.join(tier.detection_model_name()),
            recognition: directory.join(tier.recognition_model_name()),
            charset: directory.join(tier.charset_name()),
            tier,
            directory,
        }
    }

    #[must_use]
    pub fn are_ready(&self) -> bool {
        file_has_size(&self.detection, Some(self.tier.detection_model_size()))
            && file_has_size(&self.recognition, Some(self.tier.recognition_model_size()))
            && file_has_size(&self.charset, None)
    }

    pub fn install(&self) -> Result<(), OcrError> {
        let cancellation = OcrDownloadCancellation::new();
        self.install_with_progress(&cancellation, |_| {})
    }

    pub fn install_with_progress<F>(
        &self,
        cancellation: &OcrDownloadCancellation,
        mut on_progress: F,
    ) -> Result<(), OcrError>
    where
        F: FnMut(OcrModelDownloadProgress),
    {
        cancellation.ensure_active()?;
        fs::create_dir_all(&self.directory).map_err(|error| {
            OcrError::Model(format!(
                "failed to create OCR model directory {}: {error}",
                self.directory.display()
            ))
        })?;

        let total_size = self.tier.download_size();
        let detection_size = self.tier.detection_model_size();
        let recognition_size = self.tier.recognition_model_size();
        let mut completed_size = 0;

        ensure_model_file(
            &self.detection,
            self.tier.detection_model_name(),
            Some(detection_size),
            detection_size,
            completed_size,
            detection_size,
            total_size,
            cancellation,
            &mut on_progress,
        )?;
        completed_size += detection_size;

        ensure_model_file(
            &self.recognition,
            self.tier.recognition_model_name(),
            Some(recognition_size),
            recognition_size,
            completed_size,
            recognition_size,
            total_size,
            cancellation,
            &mut on_progress,
        )?;
        completed_size += recognition_size;

        ensure_model_file(
            &self.charset,
            self.tier.charset_name(),
            None,
            MIN_CHARSET_SIZE,
            completed_size,
            0,
            total_size,
            cancellation,
            &mut on_progress,
        )?;
        cancellation.ensure_active()?;
        on_progress(OcrModelDownloadProgress::determinate(
            1.0,
            format!("{} downloaded", self.tier.display_name()),
        ));
        Ok(())
    }

    pub fn remove(&self) -> Result<(), OcrError> {
        for path in [&self.detection, &self.recognition, &self.charset] {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(OcrError::Model(format!(
                        "failed to remove OCR model file {}: {error}",
                        path.display()
                    )));
                }
            }
        }

        // A custom model directory can be shared with other models or user files. `remove_dir`
        // only succeeds when it is empty, so unrelated contents are never removed recursively.
        let _ = fs::remove_dir(&self.directory);
        Ok(())
    }
}

pub struct FastOcrEngine {
    tier: PpOcrTier,
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
        Self::new_for(PpOcrTier::Small)
    }

    #[must_use]
    pub fn new_for(tier: PpOcrTier) -> Self {
        Self {
            tier,
            model_paths: FastModelPaths::discover_for(tier),
            runtime: None,
        }
    }

    #[must_use]
    pub fn tier(&self) -> PpOcrTier {
        self.tier
    }

    #[must_use]
    pub fn model_paths(&self) -> &FastModelPaths {
        &self.model_paths
    }

    fn runtime(&mut self) -> Result<&mut PaddleOcrEngine, OcrError> {
        if !self.model_paths.are_ready() {
            return Err(OcrError::Model(format!(
                "{} is not installed; download and enable it in Settings > OCR models",
                self.tier.display_name()
            )));
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
                OcrError::Backend(format!(
                    "failed to initialize {}: {error}",
                    self.tier.display_name()
                ))
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
        self.tier.engine_id()
    }

    fn display_name(&self) -> &'static str {
        self.tier.display_name()
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

fn default_model_directory(tier: PpOcrTier) -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("AzusaLens")
        .join("ocr")
        .join(tier.model_version())
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

#[allow(clippy::too_many_arguments)]
fn ensure_model_file<F>(
    path: &Path,
    file_name: &str,
    expected_size: Option<u64>,
    minimum_size: u64,
    progress_base: u64,
    progress_span: u64,
    progress_total: u64,
    cancellation: &OcrDownloadCancellation,
    on_progress: &mut F,
) -> Result<(), OcrError>
where
    F: FnMut(OcrModelDownloadProgress),
{
    cancellation.ensure_active()?;
    if file_has_size(path, expected_size) {
        emit_file_progress(
            progress_base + progress_span,
            progress_total,
            file_name,
            on_progress,
        );
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

    if let Err(error) = download_to(
        &url,
        &partial,
        file_name,
        progress_base,
        progress_span,
        progress_total,
        cancellation,
        on_progress,
    ) {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    cancellation.ensure_active()?;
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
    emit_file_progress(
        progress_base + progress_span,
        progress_total,
        file_name,
        on_progress,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn download_to<F>(
    url: &str,
    destination: &Path,
    file_name: &str,
    progress_base: u64,
    progress_span: u64,
    progress_total: u64,
    cancellation: &OcrDownloadCancellation,
    on_progress: &mut F,
) -> Result<(), OcrError>
where
    F: FnMut(OcrModelDownloadProgress),
{
    cancellation.ensure_active()?;
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(1))
        .build();
    let response = agent
        .get(url)
        .set("User-Agent", "AzusaLens/0.1")
        .call()
        .map_err(|error| OcrError::Download(format!("model download failed: {error}")))?;
    let mut reader = response.into_reader();
    let mut file = File::create(destination).map_err(|error| {
        OcrError::Download(format!(
            "cannot create model download {}: {error}",
            destination.display()
        ))
    })?;
    let mut buffer = [0_u8; DOWNLOAD_BUFFER_SIZE];
    let mut downloaded = 0_u64;

    loop {
        cancellation.ensure_active()?;
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                file.write_all(&buffer[..read]).map_err(|error| {
                    OcrError::Download(format!("cannot write model download: {error}"))
                })?;
                downloaded = downloaded.saturating_add(read as u64);
                let accounted = if progress_span == 0 {
                    progress_base
                } else {
                    progress_base + downloaded.min(progress_span)
                };
                emit_file_progress(accounted, progress_total, file_name, on_progress);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) => {}
            Err(error) => {
                return Err(OcrError::Download(format!(
                    "model download interrupted: {error}"
                )));
            }
        }
    }

    cancellation.ensure_active()?;
    file.flush()
        .map_err(|error| OcrError::Download(format!("cannot flush model file: {error}")))?;
    file.sync_all()
        .map_err(|error| OcrError::Download(format!("cannot sync model file: {error}")))?;
    Ok(())
}

fn emit_file_progress<F>(completed: u64, total: u64, file_name: &str, on_progress: &mut F)
where
    F: FnMut(OcrModelDownloadProgress),
{
    let fraction = if total == 0 {
        0.0
    } else {
        completed as f32 / total as f32
    };
    on_progress(OcrModelDownloadProgress::determinate(
        fraction,
        format!("Downloading {file_name}"),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_manifest_is_pinned_for_all_v6_tiers() {
        assert!(MODEL_BASE_URL.contains("v2.4.1"));
        assert_eq!(PpOcrTier::Tiny.detection_model_size(), 901_896);
        assert_eq!(PpOcrTier::Tiny.recognition_model_size(), 2_251_616);
        assert_eq!(
            PPOCR_TINY_MODEL_DOWNLOAD_SIZE,
            PpOcrTier::Tiny.detection_model_size() + PpOcrTier::Tiny.recognition_model_size()
        );
        assert_eq!(PpOcrTier::Small.detection_model_size(), 4_965_224);
        assert_eq!(PpOcrTier::Small.recognition_model_size(), 10_646_760);
        assert_eq!(
            PPOCR_SMALL_MODEL_DOWNLOAD_SIZE,
            PpOcrTier::Small.detection_model_size() + PpOcrTier::Small.recognition_model_size()
        );
        assert_eq!(PpOcrTier::Medium.detection_model_size(), 31_078_716);
        assert_eq!(PpOcrTier::Medium.recognition_model_size(), 38_382_108);
        assert_eq!(
            PPOCR_MEDIUM_MODEL_DOWNLOAD_SIZE,
            PpOcrTier::Medium.detection_model_size() + PpOcrTier::Medium.recognition_model_size()
        );
        assert!(PPOCR_TINY_LANGUAGE_SUMMARY.contains("no Japanese"));
        assert!(PPOCR_MEDIUM_MODEL_VERSION.contains("inference"));
    }

    #[test]
    fn tier_specific_paths_do_not_collide() {
        let directory = PathBuf::from("models");
        let tiny = FastModelPaths::from_directory_for(directory.clone(), PpOcrTier::Tiny);
        let small = FastModelPaths::from_directory_for(directory.clone(), PpOcrTier::Small);
        let medium = FastModelPaths::from_directory_for(directory, PpOcrTier::Medium);
        assert_ne!(tiny.detection, small.detection);
        assert_ne!(small.recognition, medium.recognition);
        assert_ne!(tiny.charset, medium.charset);
    }

    #[test]
    fn invalid_model_sizes_are_rejected() {
        let directory =
            std::env::temp_dir().join(format!("azusa-lens-ocr-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let paths = FastModelPaths::from_directory_for(directory.clone(), PpOcrTier::Tiny);
        fs::write(&paths.detection, b"not a model").unwrap();
        assert!(!file_has_size(
            &paths.detection,
            Some(PpOcrTier::Tiny.detection_model_size())
        ));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn pre_cancelled_install_never_starts_a_download() {
        let directory = std::env::temp_dir().join(format!(
            "azusa-lens-ocr-test-{}-cancelled-download",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let paths = FastModelPaths::from_directory_for(directory.clone(), PpOcrTier::Tiny);
        let cancellation = OcrDownloadCancellation::new();
        cancellation.cancel();

        let error = paths
            .install_with_progress(&cancellation, |_| {})
            .unwrap_err();

        assert!(matches!(error, OcrError::Cancelled(_)));
        assert!(!directory.exists());
    }

    #[test]
    fn already_installed_model_reports_completion_without_network() {
        let directory = std::env::temp_dir().join(format!(
            "azusa-lens-ocr-test-{}-installed-progress",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let paths = FastModelPaths::from_directory_for(directory.clone(), PpOcrTier::Tiny);
        File::create(&paths.detection)
            .unwrap()
            .set_len(PpOcrTier::Tiny.detection_model_size())
            .unwrap();
        File::create(&paths.recognition)
            .unwrap()
            .set_len(PpOcrTier::Tiny.recognition_model_size())
            .unwrap();
        fs::write(&paths.charset, vec![b'x'; MIN_CHARSET_SIZE as usize]).unwrap();
        let cancellation = OcrDownloadCancellation::new();
        let mut progress = Vec::new();

        paths
            .install_with_progress(&cancellation, |update| progress.push(update))
            .unwrap();

        assert_eq!(
            progress.last().and_then(|update| update.fraction),
            Some(1.0)
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn recognition_never_auto_installs_missing_models() {
        let directory = std::env::temp_dir().join(format!(
            "azusa-lens-ocr-test-{}-no-auto-download",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let tier = PpOcrTier::Medium;
        let mut engine = FastOcrEngine {
            tier,
            model_paths: FastModelPaths::from_directory_for(directory.clone(), tier),
            runtime: None,
        };
        let image = OcrImage::new(1, 1, vec![255; 4]).unwrap();

        let error = engine.recognize(&image).unwrap_err();

        assert!(matches!(error, OcrError::Model(_)));
        assert!(!directory.exists());
    }

    #[test]
    fn removal_preserves_unrelated_files_in_custom_directory() {
        let directory = std::env::temp_dir().join(format!(
            "azusa-lens-ocr-test-{}-shared-model-dir",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let paths = FastModelPaths::from_directory_for(directory.clone(), PpOcrTier::Medium);
        fs::write(&paths.detection, b"managed detection").unwrap();
        fs::write(&paths.recognition, b"managed recognition").unwrap();
        fs::write(&paths.charset, b"managed charset").unwrap();
        let unrelated = directory.join("keep-me.txt");
        fs::write(&unrelated, b"user data").unwrap();

        paths.remove().unwrap();

        assert!(!paths.detection.exists());
        assert!(!paths.recognition.exists());
        assert!(!paths.charset.exists());
        assert!(unrelated.exists());
        assert!(directory.exists());
        let _ = fs::remove_dir_all(directory);
    }
}
