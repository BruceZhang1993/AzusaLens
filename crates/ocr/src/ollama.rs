use std::{io::Cursor, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::{DynamicImage, ImageFormat, RgbaImage};
use serde_json::{Value, json};

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
const GLM_PROMPT: &str = "Text Recognition:";
const DEEPSEEK_PROMPT: &str = "<|grounding|>OCR this image.";

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

    const fn prompt(self) -> &'static str {
        match self {
            Self::Glm => GLM_PROMPT,
            Self::DeepSeek => DEEPSEEK_PROMPT,
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
                "{} is not installed or Ollama is not running; open Settings > OCR models and download it first",
                self.model.display_name()
            )));
        }

        let image = encode_png_base64(input)?;
        let response = request_generation(self.model, &image)?;

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
    let Ok(response) = status_agent().get(&ollama_url("tags")).call() else {
        return false;
    };
    let Ok(payload) = response.into_json::<Value>() else {
        return false;
    };
    payload
        .get("models")
        .and_then(Value::as_array)
        .is_some_and(|models| {
            models.iter().any(|entry| {
                entry
                    .get("name")
                    .and_then(Value::as_str)
                    .or_else(|| entry.get("model").and_then(Value::as_str))
                    .is_some_and(|installed| model_names_match(installed, model.ollama_model()))
            })
        })
}

pub fn install_ollama_model(model: OllamaOcrModel) -> Result<(), OcrError> {
    let body = json!({
        "model": model.ollama_model(),
        "stream": false,
    });
    download_agent()
        .post(&ollama_url("pull"))
        .send_json(body)
        .map_err(|error| ollama_download_error("download", model, error))?;
    Ok(())
}

pub fn remove_ollama_model(model: OllamaOcrModel) -> Result<(), OcrError> {
    if !is_ollama_model_installed(model) {
        return Ok(());
    }
    let body = json!({ "model": model.ollama_model() });
    request_agent()
        .delete(&ollama_url("delete"))
        .send_json(body)
        .map_err(|error| ollama_backend_error("remove", model, error))?;
    Ok(())
}

fn request_generation(model: OllamaOcrModel, image: &str) -> Result<String, OcrError> {
    let body = json!({
        "model": model.ollama_model(),
        "prompt": model.prompt(),
        "images": [image],
        "stream": false,
        "options": {
            "temperature": 0,
        },
    });
    let response = request_agent()
        .post(&ollama_url("generate"))
        .send_json(body)
        .map_err(|error| ollama_backend_error("run", model, error))?;
    let payload = response.into_json::<Value>().map_err(|error| {
        OcrError::Backend(format!(
            "{} returned an invalid Ollama response: {error}",
            model.display_name()
        ))
    })?;
    payload
        .get("response")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            OcrError::Backend(format!(
                "{} returned no OCR text from Ollama",
                model.display_name()
            ))
        })
}

fn encode_png_base64(input: &OcrImage) -> Result<String, OcrError> {
    let rgba = RgbaImage::from_raw(input.width(), input.height(), input.rgba().to_vec())
        .ok_or_else(|| OcrError::InvalidImage("invalid RGBA buffer dimensions".to_owned()))?;
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(rgba)
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|error| OcrError::Backend(format!("failed to encode OCR image: {error}")))?;
    Ok(BASE64.encode(cursor.into_inner()))
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
    let value = serde_json::from_str::<Value>(raw).ok()?;
    let mut boxes = Vec::new();
    collect_boxes(&value, &mut boxes);
    if boxes.is_empty() {
        return None;
    }

    let mut min_x = 999.0_f32;
    let mut min_y = 999.0_f32;
    let mut max_x = 0.0_f32;
    let mut max_y = 0.0_f32;
    for [x1, y1, x2, y2] in boxes {
        min_x = min_x.min(x1);
        min_y = min_y.min(y1);
        max_x = max_x.max(x2);
        max_y = max_y.max(y2);
    }
    if max_x <= min_x || max_y <= min_y {
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

fn collect_boxes(value: &Value, output: &mut Vec<[f32; 4]>) {
    let Some(items) = value.as_array() else {
        return;
    };
    if items.len() == 4 && items.iter().all(Value::is_number) {
        let mut result = [0.0; 4];
        for (index, item) in items.iter().enumerate() {
            let Some(number) = item.as_f64() else {
                return;
            };
            result[index] = number as f32;
        }
        output.push(result);
        return;
    }
    for item in items {
        collect_boxes(item, output);
    }
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

fn ollama_url(endpoint: &str) -> String {
    format!("{}/api/{endpoint}", ollama_base_url())
}

fn ollama_base_url() -> String {
    let value = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_owned());
    let value = value.trim_end_matches('/');
    if value.starts_with("http://") || value.starts_with("https://") {
        value.to_owned()
    } else {
        format!("http://{value}")
    }
}

fn status_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(300))
        .timeout_read(Duration::from_secs(1))
        .timeout_write(Duration::from_secs(1))
        .build()
}

fn request_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(2))
        .timeout_read(Duration::from_secs(300))
        .timeout_write(Duration::from_secs(30))
        .build()
}

fn download_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(2))
        .timeout_read(Duration::from_secs(3600))
        .timeout_write(Duration::from_secs(30))
        .build()
}

fn ollama_download_error(action: &str, model: OllamaOcrModel, error: ureq::Error) -> OcrError {
    OcrError::Download(format!(
        "failed to {action} {} through Ollama at {}: {error}. Install/start Ollama and try again from Settings > OCR models",
        model.display_name(),
        ollama_base_url()
    ))
}

fn ollama_backend_error(action: &str, model: OllamaOcrModel, error: ureq::Error) -> OcrError {
    OcrError::Backend(format!(
        "failed to {action} {} through Ollama at {}: {error}",
        model.display_name(),
        ollama_base_url()
    ))
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
