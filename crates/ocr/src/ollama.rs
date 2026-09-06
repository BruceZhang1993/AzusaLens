use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use image::{ColorType, ImageFormat};

use crate::{OcrEngine, OcrError, OcrImage, OcrRect, OcrResult, TextBlock};

pub const GLM_ENGINE_ID: &str = "glm-ocr-ollama";
pub const GLM_ENGINE_NAME: &str = "GLM-OCR · local via Ollama";
pub const GLM_MODEL_VERSION: &str = "Ollama · glm-ocr:latest";
pub const GLM_LANGUAGE_SUMMARY: &str = "Chinese, English and multilingual document OCR";
pub const GLM_MODEL_DOWNLOAD_SIZE: u64 = 2_200_000_000;

pub const DEEPSEEK_ENGINE_ID: &str = "deepseek-ocr-ollama";
pub const DEEPSEEK_ENGINE_NAME: &str = "DeepSeek-OCR · local via Ollama";
pub const DEEPSEEK_MODEL_VERSION: &str = "Ollama · deepseek-ocr:latest";
pub const DEEPSEEK_LANGUAGE_SUMMARY: &str = "Multilingual document OCR with grounding boxes";
pub const DEEPSEEK_MODEL_DOWNLOAD_SIZE: u64 = 6_700_000_000;

const GLM_OLLAMA_MODEL: &str = "glm-ocr:latest";
const DEEPSEEK_OLLAMA_MODEL: &str = "deepseek-ocr:latest";
const DEEPSEEK_PROMPT: &str = "<|grounding|>Given the layout of the image.";
static TEMP_IMAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OllamaOcrModel {
    Glm,
    DeepSeek,
}

impl OllamaOcrModel {
    #[must_use]
    pub const fn engine_id(self) -> &'static str {
        match self {
            Self::Glm => GLM_ENGINE_ID,
            Self::DeepSeek => DEEPSEEK_ENGINE_ID,
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Glm => GLM_ENGINE_NAME,
            Self::DeepSeek => DEEPSEEK_ENGINE_NAME,
        }
    }

    #[must_use]
    pub const fn ollama_model(self) -> &'static str {
        match self {
            Self::Glm => GLM_OLLAMA_MODEL,
            Self::DeepSeek => DEEPSEEK_OLLAMA_MODEL,
        }
    }
}

pub struct OllamaOcrEngine {
    model: OllamaOcrModel,
}

impl OllamaOcrEngine {
    #[must_use]
    pub const fn new(model: OllamaOcrModel) -> Self {
        Self { model }
    }
}

impl OcrEngine for OllamaOcrEngine {
    fn id(&self) -> &'static str {
        self.model.engine_id()
    }

    fn display_name(&self) -> &'static str {
        self.model.display_name()
    }

    fn is_available(&self) -> bool {
        is_ollama_model_installed(self.model)
    }

    fn recognize(&mut self, input: &OcrImage) -> Result<OcrResult, OcrError> {
        if !self.is_available() {
            return Err(OcrError::Model(format!(
                "{} is not installed or Ollama is unavailable; open Settings > OCR models and download it first",
                self.model.display_name()
            )));
        }

        let image = TempOcrImage::create(input)?;
        let response = run_ollama_ocr(self.model, image.path())?;

        if self.model == OllamaOcrModel::DeepSeek {
            let blocks = parse_deepseek_grounding(&response, input.width(), input.height());
            if !blocks.is_empty() {
                return Ok(OcrResult::from_blocks(blocks));
            }
        }

        Ok(full_image_result(
            clean_generated_text(&response),
            input.width(),
            input.height(),
        ))
    }
}

#[must_use]
pub fn is_ollama_model_installed(model: OllamaOcrModel) -> bool {
    let Ok(output) = Command::new("ollama").arg("list").output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .any(|installed| model_names_match(installed, model.ollama_model()))
}

pub fn install_ollama_model(model: OllamaOcrModel) -> Result<(), OcrError> {
    let output = Command::new("ollama")
        .args(["pull", model.ollama_model()])
        .output()
        .map_err(|error| {
            OcrError::Download(format!(
                "failed to start Ollama while downloading {}: {error}. Install/start Ollama and try again from Settings > OCR models",
                model.display_name()
            ))
        })?;
    ensure_command_success(output, "download", model, true)
}

pub fn remove_ollama_model(model: OllamaOcrModel) -> Result<(), OcrError> {
    if !is_ollama_model_installed(model) {
        return Ok(());
    }
    let output = Command::new("ollama")
        .args(["rm", model.ollama_model()])
        .output()
        .map_err(|error| {
            OcrError::Backend(format!(
                "failed to start Ollama while removing {}: {error}",
                model.display_name()
            ))
        })?;
    ensure_command_success(output, "remove", model, false)
}

fn run_ollama_ocr(model: OllamaOcrModel, image_path: &Path) -> Result<String, OcrError> {
    let mut command = Command::new("ollama");
    command.arg("run").arg(model.ollama_model());
    match model {
        OllamaOcrModel::Glm => {
            command.args(["Text", "Recognition:"]).arg(image_path);
        }
        OllamaOcrModel::DeepSeek => {
            command.arg(format!(
                "{}\n{DEEPSEEK_PROMPT}",
                image_path.to_string_lossy()
            ));
        }
    }

    let output = command.output().map_err(|error| {
        OcrError::Backend(format!(
            "failed to start {} through Ollama: {error}",
            model.display_name()
        ))
    })?;
    if !output.status.success() {
        return Err(OcrError::Backend(format_command_failure(
            "run", model, &output,
        )));
    }
    String::from_utf8(output.stdout).map_err(|error| {
        OcrError::Backend(format!(
            "{} returned non-UTF-8 OCR output: {error}",
            model.display_name()
        ))
    })
}

fn ensure_command_success(
    output: Output,
    action: &str,
    model: OllamaOcrModel,
    download_error: bool,
) -> Result<(), OcrError> {
    if output.status.success() {
        return Ok(());
    }
    let message = format_command_failure(action, model, &output);
    if download_error {
        Err(OcrError::Download(message))
    } else {
        Err(OcrError::Backend(message))
    }
}

fn format_command_failure(action: &str, model: OllamaOcrModel, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if detail.is_empty() {
        format!(
            "failed to {action} {} through Ollama (exit status {})",
            model.display_name(),
            output.status
        )
    } else {
        format!(
            "failed to {action} {} through Ollama: {detail}",
            model.display_name()
        )
    }
}

struct TempOcrImage {
    path: PathBuf,
}

impl TempOcrImage {
    fn create(input: &OcrImage) -> Result<Self, OcrError> {
        let sequence = TEMP_IMAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "azusaocr-ollama-{}-{timestamp}-{sequence}.png",
            std::process::id()
        ));
        image::save_buffer_with_format(
            &path,
            input.rgba(),
            input.width(),
            input.height(),
            ColorType::Rgba8,
            ImageFormat::Png,
        )
        .map_err(|error| {
            OcrError::Backend(format!(
                "failed to create temporary OCR image {}: {error}",
                path.display()
            ))
        })?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempOcrImage {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn parse_deepseek_grounding(raw: &str, width: u32, height: u32) -> Vec<TextBlock> {
    const REF_START: &str = "<|ref|>";
    const REF_END: &str = "<|/ref|>";
    const DET_START: &str = "<|det|>";
    const DET_END: &str = "<|/det|>";

    let mut blocks = Vec::new();
    let mut cursor = 0;

    while let Some(ref_offset) = raw[cursor..].find(REF_START) {
        let ref_start = cursor + ref_offset + REF_START.len();
        let Some(ref_end_offset) = raw[ref_start..].find(REF_END) else {
            break;
        };
        let ref_end = ref_start + ref_end_offset;
        let label = raw[ref_start..ref_end].trim();

        let after_ref = ref_end + REF_END.len();
        let Some(det_offset) = raw[after_ref..].find(DET_START) else {
            cursor = after_ref;
            continue;
        };
        let det_start = after_ref + det_offset + DET_START.len();
        let Some(det_end_offset) = raw[det_start..].find(DET_END) else {
            break;
        };
        let det_end = det_start + det_end_offset;
        let coordinates = raw[det_start..det_end].trim();
        let after_det = det_end + DET_END.len();
        let next_ref = raw[after_det..]
            .find(REF_START)
            .map_or(raw.len(), |offset| after_det + offset);
        let trailing_text = clean_generated_text(&raw[after_det..next_ref]);
        let text = if is_layout_label(label) && !trailing_text.is_empty() {
            trailing_text
        } else {
            label.to_owned()
        };

        if !text.is_empty()
            && let Some(bounds) = parse_grounding_bounds(coordinates, width, height)
        {
            blocks.push(TextBlock {
                text,
                bounds,
                polygon: None,
                // Generative OCR backends do not expose calibrated confidence scores.
                confidence: 0.0,
            });
        }
        cursor = after_det;
    }

    blocks
}

fn parse_grounding_bounds(raw: &str, width: u32, height: u32) -> Option<OcrRect> {
    let numbers = raw
        .split(|character: char| {
            !(character.is_ascii_digit()
                || matches!(character, '.' | '-' | '+' | 'e' | 'E'))
        })
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<f32>().ok())
        .collect::<Vec<_>>();
    if numbers.len() < 4 {
        return None;
    }

    let mut min_x = 999.0_f32;
    let mut min_y = 999.0_f32;
    let mut max_x = 0.0_f32;
    let mut max_y = 0.0_f32;
    let mut found_box = false;
    for coordinates in numbers.chunks_exact(4) {
        let [x1, y1, x2, y2] = coordinates else {
            continue;
        };
        min_x = min_x.min(*x1);
        min_y = min_y.min(*y1);
        max_x = max_x.max(*x2);
        max_y = max_y.max(*y2);
        found_box = true;
    }
    if !found_box || max_x <= min_x || max_y <= min_y {
        return None;
    }

    let scale_x = width as f32 / 999.0;
    let scale_y = height as f32 / 999.0;
    Some(OcrRect {
        x: min_x.clamp(0.0, 999.0) * scale_x,
        y: min_y.clamp(0.0, 999.0) * scale_y,
        width: (max_x.clamp(0.0, 999.0) - min_x.clamp(0.0, 999.0)) * scale_x,
        height: (max_y.clamp(0.0, 999.0) - min_y.clamp(0.0, 999.0)) * scale_y,
    })
}

fn full_image_result(text: String, width: u32, height: u32) -> OcrResult {
    if text.is_empty() {
        return OcrResult::from_blocks(Vec::new());
    }
    OcrResult::from_blocks(vec![TextBlock {
        text,
        bounds: OcrRect {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
        },
        polygon: None,
        confidence: 0.0,
    }])
}

fn clean_generated_text(text: &str) -> String {
    text.replace("<|grounding|>", "").trim().to_owned()
}

fn is_layout_label(label: &str) -> bool {
    matches!(
        label.trim().to_ascii_lowercase().as_str(),
        "title"
            | "text"
            | "image"
            | "table"
            | "formula"
            | "header"
            | "footer"
            | "page_number"
            | "reference"
            | "figure"
            | "caption"
    )
}

fn model_names_match(installed: &str, expected: &str) -> bool {
    installed == expected
        || installed.strip_suffix(":latest") == Some(expected)
        || expected.strip_suffix(":latest") == Some(installed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_model_names_are_equivalent() {
        assert!(model_names_match("glm-ocr", "glm-ocr:latest"));
        assert!(model_names_match("deepseek-ocr:latest", "deepseek-ocr"));
        assert!(!model_names_match("glm-ocr:q8_0", "glm-ocr:latest"));
    }

    #[test]
    fn parses_deepseek_text_grounding_into_image_coordinates() {
        let raw = "<|ref|>你好 Azusa<|/ref|><|det|>[[100, 200, 500, 300]]<|/det|>";
        let blocks = parse_deepseek_grounding(raw, 999, 999);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "你好 Azusa");
        assert_eq!(blocks[0].bounds.x, 100.0);
        assert_eq!(blocks[0].bounds.y, 200.0);
        assert_eq!(blocks[0].bounds.width, 400.0);
        assert_eq!(blocks[0].bounds.height, 100.0);
    }

    #[test]
    fn parses_multiple_grounding_boxes_into_one_region() {
        let bounds = parse_grounding_bounds("[[10,20,200,100],[8,120,300,200]]", 999, 999).unwrap();
        assert_eq!(bounds.x, 8.0);
        assert_eq!(bounds.y, 20.0);
        assert_eq!(bounds.width, 292.0);
        assert_eq!(bounds.height, 180.0);
    }

    #[test]
    fn parses_layout_label_with_following_content() {
        let raw = "<|ref|>title<|/ref|><|det|>[[10, 20, 900, 100]]<|/det|>AzusaOCR\n";
        let blocks = parse_deepseek_grounding(raw, 999, 999);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "AzusaOCR");
    }

    #[test]
    fn full_image_fallback_preserves_plain_text() {
        let result = full_image_result("line one\nline two".to_owned(), 640, 480);
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.plain_text, "line one\nline two");
        assert_eq!(result.blocks[0].bounds.width, 640.0);
        assert_eq!(result.blocks[0].bounds.height, 480.0);
    }
}
