use image::{DynamicImage, RgbaImage};
use ocr_rs::{
    OcrEngine as PaddleOcrEngine, OcrEngineConfig, OcrResult_ as PaddleOcrResult,
    RecognizeOptions, RotatedTextMode,
};

use crate::{
    OcrEngine, OcrError, OcrImage, OcrPoint, OcrRect, OcrResult, TextBlock,
    fast::{FastModelPaths, PpOcrTier},
};

const MIN_RESULT_CONFIDENCE: f32 = 0.45;
const UPSIDE_DOWN_FALLBACK_MAX_CONFIDENCE: f32 = 0.80;
const UPSIDE_DOWN_FALLBACK_MIN_CONFIDENCE_GAIN: f32 = 0.08;
const UPSIDE_DOWN_FALLBACK_MIN_ASPECT_RATIO: f32 = 1.5;
const UPSIDE_DOWN_CROP_PADDING: u32 = 2;

/// PP-OCR engine policy used by Azusa Lens.
///
/// The upstream robust mode recovers 90°/270° text with rotated detection passes,
/// de-duplicates overlapping detections, and maps polygons back into original image
/// coordinates. Azusa Lens adds a selective 180° recognition-only fallback for
/// low-confidence horizontal lines without re-running full-image detection.
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
                .with_min_result_confidence(MIN_RESULT_CONFIDENCE);
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
        let options = ppocr_recognize_options();
        let runtime = self.runtime()?;
        let mut results = runtime
            .recognize_with_options(&image, &options)
            .map_err(|error| OcrError::Backend(format!("PP-OCRv6 inference failed: {error}")))?;

        recover_upside_down_text(runtime, &image, &mut results);

        let blocks = results
            .into_iter()
            .filter_map(paddle_result_to_text_block)
            .collect();
        Ok(OcrResult::from_blocks(blocks))
    }
}

fn ppocr_recognize_options() -> RecognizeOptions {
    RecognizeOptions::new().with_rotated_text_mode(RotatedTextMode::Robust)
}

fn recover_upside_down_text(
    runtime: &PaddleOcrEngine,
    image: &DynamicImage,
    results: &mut [PaddleOcrResult],
) {
    for result in results {
        let rect = result.bbox.rect;
        if !should_try_upside_down_fallback(result.confidence, rect.width(), rect.height()) {
            continue;
        }

        let Some((x, y, width, height)) = padded_crop_bounds(
            image.width(),
            image.height(),
            rect.left(),
            rect.top(),
            rect.width(),
            rect.height(),
            UPSIDE_DOWN_CROP_PADDING,
        ) else {
            continue;
        };

        let rotated_crop = image.crop_imm(x, y, width, height).rotate180();
        let Ok(candidate) = runtime.recognize_text(&rotated_crop) else {
            continue;
        };
        let candidate_text = candidate.text.trim();
        if candidate_text.is_empty()
            || !should_replace_upside_down_candidate(result.confidence, candidate.confidence)
        {
            continue;
        }

        result.text = candidate_text.to_owned();
        result.confidence = candidate.confidence.clamp(0.0, 1.0);
    }
}

fn should_try_upside_down_fallback(confidence: f32, width: u32, height: u32) -> bool {
    confidence.is_finite()
        && confidence < UPSIDE_DOWN_FALLBACK_MAX_CONFIDENCE
        && height > 0
        && width as f32 / height as f32 >= UPSIDE_DOWN_FALLBACK_MIN_ASPECT_RATIO
}

fn should_replace_upside_down_candidate(current_confidence: f32, candidate_confidence: f32) -> bool {
    candidate_confidence.is_finite()
        && candidate_confidence
            >= current_confidence + UPSIDE_DOWN_FALLBACK_MIN_CONFIDENCE_GAIN
}

#[allow(clippy::too_many_arguments)]
fn padded_crop_bounds(
    image_width: u32,
    image_height: u32,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    padding: u32,
) -> Option<(u32, u32, u32, u32)> {
    if image_width == 0 || image_height == 0 || width == 0 || height == 0 {
        return None;
    }

    let image_width = i64::from(image_width);
    let image_height = i64::from(image_height);
    let padding = i64::from(padding);
    let raw_left = i64::from(left);
    let raw_top = i64::from(top);
    let raw_right = raw_left.saturating_add(i64::from(width));
    let raw_bottom = raw_top.saturating_add(i64::from(height));

    let x0 = raw_left.saturating_sub(padding).clamp(0, image_width);
    let y0 = raw_top.saturating_sub(padding).clamp(0, image_height);
    let x1 = raw_right.saturating_add(padding).clamp(0, image_width);
    let y1 = raw_bottom
        .saturating_add(padding)
        .clamp(0, image_height);

    if x1 <= x0 || y1 <= y0 {
        return None;
    }

    Some((
        x0 as u32,
        y0 as u32,
        (x1 - x0) as u32,
        (y1 - y0) as u32,
    ))
}

fn paddle_result_to_text_block(result: PaddleOcrResult) -> Option<TextBlock> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ppocr_uses_upstream_robust_rotated_text_mode() {
        let options = ppocr_recognize_options();
        assert_eq!(options.rotated_text_mode(), RotatedTextMode::Robust);
    }

    #[test]
    fn upside_down_fallback_is_selective() {
        assert!(should_try_upside_down_fallback(0.62, 180, 40));
        assert!(!should_try_upside_down_fallback(0.92, 180, 40));
        assert!(!should_try_upside_down_fallback(0.62, 40, 180));
        assert!(!should_try_upside_down_fallback(0.62, 0, 0));
    }

    #[test]
    fn upside_down_candidate_needs_meaningful_confidence_gain() {
        assert!(should_replace_upside_down_candidate(0.60, 0.70));
        assert!(!should_replace_upside_down_candidate(0.60, 0.67));
        assert!(!should_replace_upside_down_candidate(0.90, f32::NAN));
    }

    #[test]
    fn padded_crop_bounds_preserve_padding_and_clip_to_image() {
        assert_eq!(
            padded_crop_bounds(100, 80, 10, 20, 30, 10, 2),
            Some((8, 18, 34, 14))
        );
        assert_eq!(
            padded_crop_bounds(100, 80, -3, 75, 10, 10, 2),
            Some((0, 73, 9, 7))
        );
        assert_eq!(padded_crop_bounds(100, 80, 120, 10, 20, 10, 2), None);
    }
}
