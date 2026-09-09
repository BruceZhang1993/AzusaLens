use std::fs;

use image::{DynamicImage, RgbaImage};
use oar_ocr::{
    download::{cache_dir, fetch, find},
    prelude::{OARStructure, OARStructureBuilder},
};

use crate::{
    OcrDownloadCancellation, OcrEngine, OcrError, OcrImage, OcrModelDownloadProgress, OcrResult,
    OcrTaskKind,
};

pub const PP_STRUCTURE_V3_ENGINE_ID: &str = "pp-structurev3-onnx";
pub const PP_STRUCTURE_V3_ENGINE_NAME: &str = "PP-StructureV3 · Rust/ONNX";
pub const PP_STRUCTURE_V3_MODEL_VERSION: &str = "pp-structurev3-oar-ocr-0.9.2";
pub const PP_STRUCTURE_V3_LANGUAGE_SUMMARY: &str =
    "Structured document parsing with PP-OCRv6 Small text recognition, tables and formulas";
pub const PP_STRUCTURE_V3_MODEL_DOWNLOAD_SIZE: u64 = 462_679_813;

/// Balanced PP-StructureV3 bundle for local desktop inference.
///
/// The bundle intentionally stays Python-free. OAR-OCR executes every component through
/// ONNX Runtime and its download registry verifies each artifact with SHA-256.
const PP_STRUCTURE_V3_FILES: &[&str] = &[
    "pp-doclayoutv3.onnx",
    "pp-lcnet_x1_0_doc_ori.onnx",
    "uvdoc.onnx",
    "pp-lcnet_x1_0_textline_ori.onnx",
    "pp-ocrv6_small_det.onnx",
    "pp-ocrv6_small_rec.onnx",
    "ppocrv6_dict.txt",
    "pp-lcnet_x1_0_table_cls.onnx",
    "slanet.onnx",
    "slanet_plus.onnx",
    "table_structure_dict_ch.txt",
    "pp-formulanet_plus-s.onnx",
    "pp-formulanet-tokenizer.json",
];

#[must_use]
pub fn is_pp_structure_v3_installed() -> bool {
    let directory = cache_dir();
    PP_STRUCTURE_V3_FILES.iter().all(|name| {
        let Some(entry) = find(name) else {
            return false;
        };
        fs::metadata(directory.join(name))
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() == entry.size)
    })
}

pub fn install_pp_structure_v3_with_progress<F>(
    cancellation: &OcrDownloadCancellation,
    mut on_progress: F,
) -> Result<(), OcrError>
where
    F: FnMut(OcrModelDownloadProgress),
{
    cancellation.ensure_active()?;
    let total = PP_STRUCTURE_V3_FILES
        .iter()
        .map(|name| {
            find(name)
                .map(|entry| entry.size)
                .ok_or_else(|| OcrError::Model(format!("PP-StructureV3 component is not registered: {name}")))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<u64>();
    let mut completed = 0_u64;

    for name in PP_STRUCTURE_V3_FILES {
        cancellation.ensure_active()?;
        let entry = find(name).ok_or_else(|| {
            OcrError::Model(format!("PP-StructureV3 component is not registered: {name}"))
        })?;
        on_progress(OcrModelDownloadProgress::determinate(
            completed as f32 / total.max(1) as f32,
            format!("Downloading PP-StructureV3 · {name}"),
        ));
        fetch(name).map_err(|error| {
            OcrError::Download(format!("PP-StructureV3 component {name} failed: {error}"))
        })?;
        completed = completed.saturating_add(entry.size);
        cancellation.ensure_active()?;
        on_progress(OcrModelDownloadProgress::determinate(
            completed as f32 / total.max(1) as f32,
            format!("Downloaded PP-StructureV3 · {name}"),
        ));
    }

    Ok(())
}

pub fn remove_pp_structure_v3() -> Result<(), OcrError> {
    let directory = cache_dir();
    for name in PP_STRUCTURE_V3_FILES {
        let path = directory.join(name);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(OcrError::Model(format!(
                    "failed to remove PP-StructureV3 component {}: {error}",
                    path.display()
                )));
            }
        }
        let sidecar = directory.join(format!(".{name}.sha256"));
        match fs::remove_file(sidecar) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(OcrError::Model(format!(
                    "failed to remove PP-StructureV3 verification metadata for {name}: {error}"
                )));
            }
        }
    }
    Ok(())
}

pub struct PpStructureV3Engine {
    runtime: Option<OARStructure>,
}

impl PpStructureV3Engine {
    #[must_use]
    pub const fn new() -> Self {
        Self { runtime: None }
    }

    fn runtime(&mut self) -> Result<&mut OARStructure, OcrError> {
        if !is_pp_structure_v3_installed() {
            return Err(OcrError::Model(
                "PP-StructureV3 is not installed; download it in Settings > OCR models"
                    .to_owned(),
            ));
        }
        if self.runtime.is_none() {
            let directory = cache_dir();
            let runtime = OARStructureBuilder::new(directory.join("pp-doclayoutv3.onnx"))
                .layout_model_name("PP-DocLayoutV3")
                .with_document_orientation(directory.join("pp-lcnet_x1_0_doc_ori.onnx"))
                .with_document_rectification(directory.join("uvdoc.onnx"))
                .with_text_line_orientation(directory.join("pp-lcnet_x1_0_textline_ori.onnx"))
                .with_ocr(
                    directory.join("pp-ocrv6_small_det.onnx"),
                    directory.join("pp-ocrv6_small_rec.onnx"),
                    directory.join("ppocrv6_dict.txt"),
                )
                .text_detection_model_name("PP-OCRv6_small_det")
                .text_recognition_model_name("PP-OCRv6_small_rec")
                .with_table_classification(directory.join("pp-lcnet_x1_0_table_cls.onnx"))
                .with_wired_table_structure(directory.join("slanet.onnx"))
                .wired_table_structure_model_name("SLANet")
                .with_wireless_table_structure(directory.join("slanet_plus.onnx"))
                .wireless_table_structure_model_name("SLANet_plus")
                .table_structure_dict_path(directory.join("table_structure_dict_ch.txt"))
                .use_e2e_wired_table_rec(true)
                .use_e2e_wireless_table_rec(true)
                .with_formula_recognition(
                    directory.join("pp-formulanet_plus-s.onnx"),
                    directory.join("pp-formulanet-tokenizer.json"),
                    "pp_formulanet",
                )
                .image_batch_size(1)
                .region_batch_size(4)
                .build()
                .map_err(|error| {
                    OcrError::Backend(format!("failed to initialize PP-StructureV3: {error}"))
                })?;
            self.runtime = Some(runtime);
        }
        self.runtime
            .as_mut()
            .ok_or_else(|| OcrError::Backend("PP-StructureV3 runtime was not initialized".to_owned()))
    }

    fn recognize_structure(&mut self, input: &OcrImage) -> Result<OcrResult, OcrError> {
        let rgba = RgbaImage::from_raw(input.width(), input.height(), input.rgba().to_vec())
            .ok_or_else(|| OcrError::InvalidImage("invalid RGBA buffer dimensions".to_owned()))?;
        let rgb = DynamicImage::ImageRgba8(rgba).to_rgb8();
        let result = self
            .runtime()?
            .predict_image(rgb)
            .map_err(|error| OcrError::Backend(format!("PP-StructureV3 inference failed: {error}")))?;
        let markdown = result.to_markdown();
        Ok(OcrResult {
            plain_text: markdown,
            blocks: Vec::new(),
        })
    }
}

impl Default for PpStructureV3Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl OcrEngine for PpStructureV3Engine {
    fn id(&self) -> &'static str {
        PP_STRUCTURE_V3_ENGINE_ID
    }

    fn display_name(&self) -> &'static str {
        PP_STRUCTURE_V3_ENGINE_NAME
    }

    fn is_available(&self) -> bool {
        is_pp_structure_v3_installed()
    }

    fn recognize(&mut self, input: &OcrImage) -> Result<OcrResult, OcrError> {
        self.recognize_structure(input)
    }

    fn recognize_task(
        &mut self,
        task: OcrTaskKind,
        input: &OcrImage,
    ) -> Result<OcrResult, OcrError> {
        match task {
            OcrTaskKind::Document | OcrTaskKind::Table => self.recognize_structure(input),
            _ => Err(OcrError::Model(format!(
                "{} does not support {}",
                self.display_name(),
                task.display_name()
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_bundle_size_matches_descriptor() {
        let total = PP_STRUCTURE_V3_FILES
            .iter()
            .map(|name| find(name).expect("registered PP-StructureV3 component").size)
            .sum::<u64>();
        assert_eq!(total, PP_STRUCTURE_V3_MODEL_DOWNLOAD_SIZE);
    }
}
