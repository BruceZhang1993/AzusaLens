use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use image::{ColorType, ImageFormat};

use crate::{
    OcrDownloadCancellation, OcrEngine, OcrError, OcrImage, OcrModelDownloadProgress, OcrRect,
    OcrResult, TextBlock,
};

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
const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
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

    #[must_use]
    pub const fn download_size(self) -> u64 {
        match self {
            Self::Glm => GLM_MODEL_DOWNLOAD_SIZE,
            Self::DeepSeek => DEEPSEEK_MODEL_DOWNLOAD_SIZE,
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
    let cancellation = OcrDownloadCancellation::new();
    install_ollama_model_with_progress(model, &cancellation, |_| {})
}

pub fn install_ollama_model_with_progress<F>(
    model: OllamaOcrModel,
    cancellation: &OcrDownloadCancellation,
    mut on_progress: F,
) -> Result<(), OcrError>
where
    F: FnMut(OcrModelDownloadProgress),
{
    cancellation.ensure_active()?;
    on_progress(OcrModelDownloadProgress::indeterminate(format!(
        "Connecting to Ollama for {}",
        model.display_name()
    )));

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(1))
        .build();
    let url = format!("{}/api/pull", ollama_base_url());
    let body = format!("{{\"model\":\"{}\",\"stream\":true}}", model.ollama_model());
    let response = agent
        .post(&url)
        .set("User-Agent", "AzusaLens/0.1")
        .set("Content-Type", "application/json")
        .send_string(&body)
        .map_err(|error| {
            OcrError::Download(format!(
                "could not start {} download through the local Ollama API: {error}. Start Ollama and try again",
                model.display_name()
            ))
        })?;

    let mut reader = BufReader::new(response.into_reader());
    let mut line = String::new();
    let mut layer_completed = HashMap::<String, u64>::new();
    let expected_total = model.download_size().max(1);
    let mut reported_success = false;

    loop {
        cancellation.ensure_active()?;
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Some(error) = ollama_pull_error(&line, model) {
                    return Err(error);
                }

                let status = json_string_field(&line, "status")
                    .unwrap_or_else(|| "Downloading with Ollama".to_owned());
                if status.eq_ignore_ascii_case("success") {
                    reported_success = true;
                    on_progress(OcrModelDownloadProgress::determinate(
                        1.0,
                        format!("{} downloaded", model.display_name()),
                    ));
                    break;
                }

                let completed = json_u64_field(&line, "completed");
                let total = json_u64_field(&line, "total");
                if let Some(completed) = completed {
                    let layer_key =
                        json_string_field(&line, "digest").unwrap_or_else(|| status.clone());
                    layer_completed.insert(layer_key, completed);
                    let downloaded = layer_completed
                        .values()
                        .copied()
                        .fold(0_u64, u64::saturating_add);
                    // Catalog sizes are approximate for Ollama models, so reserve the final 2% for
                    // verification/manifest writing and report 100% only after Ollama says success.
                    let fraction = (downloaded as f64 / expected_total as f64).min(0.98) as f32;
                    let detail = total.filter(|total| *total > 0).map_or_else(
                        || status.clone(),
                        |total| {
                            format!(
                                "{status} · {:.0}% of current layer",
                                (completed as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
                            )
                        },
                    );
                    on_progress(OcrModelDownloadProgress::determinate(fraction, detail));
                } else {
                    on_progress(OcrModelDownloadProgress::indeterminate(status));
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) => {}
            Err(error) => {
                return Err(OcrError::Download(format!(
                    "Ollama model download stream failed: {error}"
                )));
            }
        }
    }

    cancellation.ensure_active()?;
    if reported_success {
        Ok(())
    } else {
        Err(OcrError::Download(
            "Ollama download ended before reporting success".to_owned(),
        ))
    }
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
    ensure_command_success(output, "remove", model)
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
) -> Result<(), OcrError> {
    if output.status.success() {
        Ok(())
    } else {
        Err(OcrError::Backend(format_command_failure(
            action, model, &output,
        )))
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

fn ollama_base_url() -> String {
    normalize_ollama_host(std::env::var("OLLAMA_HOST").ok().as_deref())
}

fn normalize_ollama_host(host: Option<&str>) -> String {
    let host = host.map(str::trim).filter(|value| !value.is_empty());
    match host {
        None => DEFAULT_OLLAMA_BASE_URL.to_owned(),
        Some(host) if host.starts_with("http://") || host.starts_with("https://") => {
            host.trim_end_matches('/').to_owned()
        }
        Some(host) => format!("http://{}", host.trim_end_matches('/')),
    }
}

fn ollama_pull_error(line: &str, model: OllamaOcrModel) -> Option<OcrError> {
    json_string_field(line, "error").map(|error| {
        OcrError::Download(format!(
            "{} download failed through Ollama: {error}",
            model.display_name()
        ))
    })
}

fn json_string_field(line: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let after_key = line.get(line.find(&marker)? + marker.len()..)?;
    let after_colon = after_key.get(after_key.find(':')? + 1..)?.trim_start();
    let value = after_colon.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_owned())
}

fn json_u64_field(line: &str, key: &str) -> Option<u64> {
    let marker = format!("\"{key}\"");
    let after_key = line.get(line.find(&marker)? + marker.len()..)?;
    let after_colon = after_key.get(after_key.find(':')? + 1..)?.trim_start();
    let digits = after_colon
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
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
            "azusa-lens-ollama-{}-{timestamp}-{sequence}.png",
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
            !(character.is_ascii_digit() || matches!(character, '.' | '-' | '+' | 'e' | 'E'))
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
    for coordinates in numbers.as_chunks::<4>().0 {
        let [x1, y1, x2, y2] = coordinates;
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
    fn ollama_host_is_normalized_for_local_api_calls() {
        assert_eq!(normalize_ollama_host(None), DEFAULT_OLLAMA_BASE_URL);
        assert_eq!(
            normalize_ollama_host(Some("localhost:11434/")),
            "http://localhost:11434"
        );
        assert_eq!(
            normalize_ollama_host(Some("https://ollama.example.test/")),
            "https://ollama.example.test"
        );
    }

    #[test]
    fn parses_ollama_pull_progress_fields_without_json_dependency() {
        let line = r#"{\"status\":\"pulling layer\",\"digest\":\"sha256:abc\",\"total\":200,\"completed\":50}"#;
        let line = line.replace("\\\"", "\"");
        assert_eq!(
            json_string_field(&line, "status").as_deref(),
            Some("pulling layer")
        );
        assert_eq!(
            json_string_field(&line, "digest").as_deref(),
            Some("sha256:abc")
        );
        assert_eq!(json_u64_field(&line, "total"), Some(200));
        assert_eq!(json_u64_field(&line, "completed"), Some(50));
    }

    #[test]
    fn streamed_ollama_pull_error_preserves_backend_detail() {
        let line = r#"{\"error\":\"model manifest not found\"}"#;
        let line = line.replace("\\\"", "\"");
        let error = ollama_pull_error(&line, OllamaOcrModel::Glm).unwrap();
        assert!(matches!(error, OcrError::Download(_)));
        assert!(error.to_string().contains("model manifest not found"));
    }

    #[test]
    fn pre_cancelled_ollama_pull_never_opens_a_connection() {
        let cancellation = OcrDownloadCancellation::new();
        cancellation.cancel();
        let error = install_ollama_model_with_progress(OllamaOcrModel::Glm, &cancellation, |_| {})
            .unwrap_err();
        assert!(matches!(error, OcrError::Cancelled(_)));
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
        let raw = "<|ref|>title<|/ref|><|det|>[[10, 20, 900, 100]]<|/det|>Azusa Lens\n";
        let blocks = parse_deepseek_grounding(raw, 999, 999);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "Azusa Lens");
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
