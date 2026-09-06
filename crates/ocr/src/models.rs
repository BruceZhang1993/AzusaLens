use std::{fs, path::PathBuf};

use crate::{
    DEEPSEEK_ENGINE_ID, DEEPSEEK_ENGINE_NAME, DEEPSEEK_LANGUAGE_SUMMARY,
    DEEPSEEK_MODEL_DOWNLOAD_SIZE, DEEPSEEK_MODEL_VERSION, FastModelPaths, FastOcrEngine,
    GLM_ENGINE_ID, GLM_ENGINE_NAME, GLM_LANGUAGE_SUMMARY, GLM_MODEL_DOWNLOAD_SIZE,
    GLM_MODEL_VERSION, OcrDownloadCancellation, OcrEngine, OcrError, OcrModelDownloadProgress,
    OllamaOcrEngine, OllamaOcrModel, PPOCR_MEDIUM_ENGINE_ID, PPOCR_MEDIUM_ENGINE_NAME,
    PPOCR_MEDIUM_LANGUAGE_SUMMARY, PPOCR_MEDIUM_MODEL_DOWNLOAD_SIZE, PPOCR_MEDIUM_MODEL_VERSION,
    PPOCR_SMALL_ENGINE_ID, PPOCR_SMALL_ENGINE_NAME, PPOCR_SMALL_LANGUAGE_SUMMARY,
    PPOCR_SMALL_MODEL_DOWNLOAD_SIZE, PPOCR_SMALL_MODEL_VERSION, PPOCR_TINY_ENGINE_ID,
    PPOCR_TINY_ENGINE_NAME, PPOCR_TINY_LANGUAGE_SUMMARY, PPOCR_TINY_MODEL_DOWNLOAD_SIZE,
    PPOCR_TINY_MODEL_VERSION, PpOcrTier, install_ollama_model_with_progress,
    is_ollama_model_installed, remove_ollama_model,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OcrModelDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub languages: &'static str,
    pub download_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrModelState {
    pub descriptor: OcrModelDescriptor,
    pub installed: bool,
    pub active: bool,
}

const MODEL_CATALOG: [OcrModelDescriptor; 5] = [
    OcrModelDescriptor {
        id: PPOCR_TINY_ENGINE_ID,
        name: PPOCR_TINY_ENGINE_NAME,
        version: PPOCR_TINY_MODEL_VERSION,
        languages: PPOCR_TINY_LANGUAGE_SUMMARY,
        download_size_bytes: PPOCR_TINY_MODEL_DOWNLOAD_SIZE,
    },
    OcrModelDescriptor {
        id: PPOCR_SMALL_ENGINE_ID,
        name: PPOCR_SMALL_ENGINE_NAME,
        version: PPOCR_SMALL_MODEL_VERSION,
        languages: PPOCR_SMALL_LANGUAGE_SUMMARY,
        download_size_bytes: PPOCR_SMALL_MODEL_DOWNLOAD_SIZE,
    },
    OcrModelDescriptor {
        id: PPOCR_MEDIUM_ENGINE_ID,
        name: PPOCR_MEDIUM_ENGINE_NAME,
        version: PPOCR_MEDIUM_MODEL_VERSION,
        languages: PPOCR_MEDIUM_LANGUAGE_SUMMARY,
        download_size_bytes: PPOCR_MEDIUM_MODEL_DOWNLOAD_SIZE,
    },
    OcrModelDescriptor {
        id: GLM_ENGINE_ID,
        name: GLM_ENGINE_NAME,
        version: GLM_MODEL_VERSION,
        languages: GLM_LANGUAGE_SUMMARY,
        download_size_bytes: GLM_MODEL_DOWNLOAD_SIZE,
    },
    OcrModelDescriptor {
        id: DEEPSEEK_ENGINE_ID,
        name: DEEPSEEK_ENGINE_NAME,
        version: DEEPSEEK_MODEL_VERSION,
        languages: DEEPSEEK_LANGUAGE_SUMMARY,
        download_size_bytes: DEEPSEEK_MODEL_DOWNLOAD_SIZE,
    },
];

#[derive(Debug, Clone)]
pub struct OcrModelManager {
    config_directory: PathBuf,
    ppocr_model_directory_override: Option<PathBuf>,
}

impl Default for OcrModelManager {
    fn default() -> Self {
        Self::discover()
    }
}

impl OcrModelManager {
    #[must_use]
    pub fn discover() -> Self {
        Self {
            config_directory: default_config_directory(),
            ppocr_model_directory_override: None,
        }
    }

    #[cfg(test)]
    fn with_directories(config_directory: PathBuf, model_directory: PathBuf) -> Self {
        Self {
            config_directory,
            ppocr_model_directory_override: Some(model_directory),
        }
    }

    #[must_use]
    pub fn catalog() -> &'static [OcrModelDescriptor] {
        &MODEL_CATALOG
    }

    #[must_use]
    pub fn descriptor(model_id: &str) -> Option<&'static OcrModelDescriptor> {
        MODEL_CATALOG.iter().find(|model| model.id == model_id)
    }

    #[must_use]
    pub fn states(&self) -> Vec<OcrModelState> {
        let active_model_id = self.active_model_id();
        MODEL_CATALOG
            .iter()
            .copied()
            .map(|descriptor| OcrModelState {
                installed: self.is_installed(descriptor.id),
                active: active_model_id.as_deref() == Some(descriptor.id),
                descriptor,
            })
            .collect()
    }

    #[must_use]
    pub fn active_model_id(&self) -> Option<String> {
        let value = fs::read_to_string(self.active_model_path()).ok()?;
        let model_id = value.trim();
        if Self::descriptor(model_id).is_some() && self.is_installed(model_id) {
            Some(model_id.to_owned())
        } else {
            None
        }
    }

    #[must_use]
    pub fn is_installed(&self, model_id: &str) -> bool {
        if let Some(tier) = PpOcrTier::from_engine_id(model_id) {
            return self.ppocr_paths(tier).are_ready();
        }
        match model_id {
            GLM_ENGINE_ID => is_ollama_model_installed(OllamaOcrModel::Glm),
            DEEPSEEK_ENGINE_ID => is_ollama_model_installed(OllamaOcrModel::DeepSeek),
            _ => false,
        }
    }

    pub fn install_model(&self, model_id: &str) -> Result<(), OcrError> {
        let cancellation = OcrDownloadCancellation::new();
        self.install_model_with_progress(model_id, &cancellation, |_| {})
    }

    pub fn install_model_with_progress<F>(
        &self,
        model_id: &str,
        cancellation: &OcrDownloadCancellation,
        on_progress: F,
    ) -> Result<(), OcrError>
    where
        F: FnMut(OcrModelDownloadProgress),
    {
        if let Some(tier) = PpOcrTier::from_engine_id(model_id) {
            return self
                .ppocr_paths(tier)
                .install_with_progress(cancellation, on_progress);
        }
        match model_id {
            GLM_ENGINE_ID => install_ollama_model_with_progress(
                OllamaOcrModel::Glm,
                cancellation,
                on_progress,
            ),
            DEEPSEEK_ENGINE_ID => install_ollama_model_with_progress(
                OllamaOcrModel::DeepSeek,
                cancellation,
                on_progress,
            ),
            _ => Err(OcrError::Model(format!("unknown OCR model: {model_id}"))),
        }
    }

    pub fn remove_model(&self, model_id: &str) -> Result<(), OcrError> {
        if let Some(tier) = PpOcrTier::from_engine_id(model_id) {
            self.ppocr_paths(tier).remove()?;
        } else {
            match model_id {
                GLM_ENGINE_ID => remove_ollama_model(OllamaOcrModel::Glm)?,
                DEEPSEEK_ENGINE_ID => remove_ollama_model(OllamaOcrModel::DeepSeek)?,
                _ => return Err(OcrError::Model(format!("unknown OCR model: {model_id}"))),
            }
        }
        if self.selected_model_id().as_deref() == Some(model_id) {
            self.clear_active_model()?;
        }
        Ok(())
    }

    pub fn set_active_model(&self, model_id: &str) -> Result<(), OcrError> {
        let descriptor = Self::descriptor(model_id)
            .ok_or_else(|| OcrError::Model(format!("unknown OCR model: {model_id}")))?;
        if !self.is_installed(model_id) {
            return Err(OcrError::Model(format!(
                "{} is not installed; download it before enabling it",
                descriptor.name
            )));
        }

        fs::create_dir_all(&self.config_directory).map_err(|error| {
            OcrError::Model(format!(
                "failed to create OCR settings directory {}: {error}",
                self.config_directory.display()
            ))
        })?;
        let target = self.active_model_path();
        let partial = target.with_extension("tmp");
        fs::write(&partial, format!("{model_id}\n")).map_err(|error| {
            OcrError::Model(format!("failed to save active OCR model: {error}"))
        })?;
        if target.exists() {
            fs::remove_file(&target).map_err(|error| {
                OcrError::Model(format!(
                    "failed to replace active OCR model setting: {error}"
                ))
            })?;
        }
        fs::rename(&partial, &target)
            .map_err(|error| OcrError::Model(format!("failed to activate OCR model: {error}")))
    }

    pub fn clear_active_model(&self) -> Result<(), OcrError> {
        let target = self.active_model_path();
        if !target.exists() {
            return Ok(());
        }
        fs::remove_file(target)
            .map_err(|error| OcrError::Model(format!("failed to clear active OCR model: {error}")))
    }

    fn ppocr_paths(&self, tier: PpOcrTier) -> FastModelPaths {
        self.ppocr_model_directory_override.as_ref().map_or_else(
            || FastModelPaths::discover_for(tier),
            |directory| FastModelPaths::from_directory_for(directory.clone(), tier),
        )
    }

    fn selected_model_id(&self) -> Option<String> {
        fs::read_to_string(self.active_model_path())
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }

    fn active_model_path(&self) -> PathBuf {
        self.config_directory.join("active-model")
    }
}

pub fn create_engine(model_id: &str) -> Result<Box<dyn OcrEngine>, OcrError> {
    if let Some(tier) = PpOcrTier::from_engine_id(model_id) {
        return Ok(Box::new(FastOcrEngine::new_for(tier)));
    }
    match model_id {
        GLM_ENGINE_ID => Ok(Box::new(OllamaOcrEngine::new(OllamaOcrModel::Glm))),
        DEEPSEEK_ENGINE_ID => Ok(Box::new(OllamaOcrEngine::new(OllamaOcrModel::DeepSeek))),
        _ => Err(OcrError::Model(format!("unknown OCR model: {model_id}"))),
    }
}

fn default_config_directory() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("AzusaOCR")
        .join("ocr")
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use super::*;

    fn create_installed_ppocr_model(paths: &FastModelPaths) {
        fs::create_dir_all(&paths.directory).unwrap();
        let (det_size, rec_size) = match paths.tier {
            PpOcrTier::Tiny => (901_896, 2_251_616),
            PpOcrTier::Small => (4_965_224, 10_646_760),
            PpOcrTier::Medium => (31_078_716, 38_382_108),
        };
        File::create(&paths.detection)
            .unwrap()
            .set_len(det_size)
            .unwrap();
        File::create(&paths.recognition)
            .unwrap()
            .set_len(rec_size)
            .unwrap();
        let mut charset = File::create(&paths.charset).unwrap();
        charset.write_all(&[b'x'; 1_024]).unwrap();
    }

    #[test]
    fn catalog_exposes_three_ppocr_tiers_and_optional_vlm_models() {
        let tiny = OcrModelManager::descriptor(PPOCR_TINY_ENGINE_ID).unwrap();
        assert!(tiny.name.contains("Tiny"));
        assert!(tiny.languages.contains("no Japanese"));

        let small = OcrModelManager::descriptor(PPOCR_SMALL_ENGINE_ID).unwrap();
        assert!(small.name.contains("Small"));
        assert!(small.languages.contains("Japanese"));

        let medium = OcrModelManager::descriptor(PPOCR_MEDIUM_ENGINE_ID).unwrap();
        assert!(medium.name.contains("Medium"));
        assert!(medium.version.contains("inference"));
        assert!(medium.download_size_bytes > small.download_size_bytes);

        let glm = OcrModelManager::descriptor(GLM_ENGINE_ID).unwrap();
        assert!(glm.name.contains("GLM-OCR"));
        assert!(glm.version.contains("Ollama"));

        let deepseek = OcrModelManager::descriptor(DEEPSEEK_ENGINE_ID).unwrap();
        assert!(deepseek.name.contains("DeepSeek-OCR"));
        assert!(deepseek.version.contains("Ollama"));
        assert_eq!(OcrModelManager::catalog().len(), 5);
    }

    #[test]
    fn ppocr_tiers_are_managed_independently() {
        let root = std::env::temp_dir().join(format!(
            "azusaocr-model-manager-test-{}-tiers",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let manager = OcrModelManager::with_directories(root.join("config"), root.join("models"));
        let tiny = manager.ppocr_paths(PpOcrTier::Tiny);
        let medium = manager.ppocr_paths(PpOcrTier::Medium);
        create_installed_ppocr_model(&tiny);

        assert!(manager.is_installed(PPOCR_TINY_ENGINE_ID));
        assert!(!manager.is_installed(PPOCR_MEDIUM_ENGINE_ID));

        create_installed_ppocr_model(&medium);
        assert!(manager.is_installed(PPOCR_MEDIUM_ENGINE_ID));
        manager.remove_model(PPOCR_TINY_ENGINE_ID).unwrap();
        assert!(!manager.is_installed(PPOCR_TINY_ENGINE_ID));
        assert!(manager.is_installed(PPOCR_MEDIUM_ENGINE_ID));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pre_cancelled_manager_install_does_not_touch_model_storage() {
        let root = std::env::temp_dir().join(format!(
            "azusaocr-model-manager-test-{}-cancelled",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let manager = OcrModelManager::with_directories(root.join("config"), root.join("models"));
        let cancellation = OcrDownloadCancellation::new();
        cancellation.cancel();

        let error = manager
            .install_model_with_progress(PPOCR_TINY_ENGINE_ID, &cancellation, |_| {})
            .unwrap_err();

        assert!(matches!(error, OcrError::Cancelled(_)));
        assert!(!root.join("models").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn model_must_be_installed_before_it_can_be_enabled() {
        let root = std::env::temp_dir().join(format!(
            "azusaocr-model-manager-test-{}-missing",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let manager = OcrModelManager::with_directories(root.join("config"), root.join("models"));
        assert!(manager.set_active_model(PPOCR_MEDIUM_ENGINE_ID).is_err());
        assert_eq!(manager.active_model_id(), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn active_model_selection_is_persisted_and_cleared_on_remove() {
        let root = std::env::temp_dir().join(format!(
            "azusaocr-model-manager-test-{}-active",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let manager = OcrModelManager::with_directories(root.join("config"), root.join("models"));
        let paths = manager.ppocr_paths(PpOcrTier::Small);
        create_installed_ppocr_model(&paths);

        manager.set_active_model(PPOCR_SMALL_ENGINE_ID).unwrap();
        assert_eq!(
            manager.active_model_id().as_deref(),
            Some(PPOCR_SMALL_ENGINE_ID)
        );
        manager.remove_model(PPOCR_SMALL_ENGINE_ID).unwrap();
        assert_eq!(manager.active_model_id(), None);
        assert!(!manager.is_installed(PPOCR_SMALL_ENGINE_ID));
        let _ = fs::remove_dir_all(root);
    }
}
