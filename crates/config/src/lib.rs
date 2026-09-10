use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

pub const SETTINGS_SCHEMA_VERSION: u32 = 5;

static SETTINGS_UPDATE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceMode {
    #[default]
    System,
    Light,
    Dark,
}

impl AppearanceMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageMode {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
}

impl LanguageMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "en" => Some(Self::English),
            "zh-CN" | "zh_CN" | "zh" => Some(Self::SimplifiedChinese),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OcrSettings {
    /// Legacy single-model selection kept for backward-compatible deserialization.
    pub active_model_id: Option<String>,
    pub text_model_id: Option<String>,
    pub document_model_id: Option<String>,
    pub table_model_id: Option<String>,
    pub figure_model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyBindingSettings {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub key: String,
}

impl Default for HotkeyBindingSettings {
    fn default() -> Self {
        Self {
            control: false,
            alt: false,
            shift: false,
            meta: false,
            key: "PrintScreen".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeySettings {
    pub capture: HotkeyBindingSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportSettings {
    pub default_directory: Option<PathBuf>,
    pub last_directory: Option<PathBuf>,
    pub remember_last_directory: bool,
    pub copy_after_capture: bool,
    pub close_after_copy: bool,
    pub close_after_save: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            default_directory: None,
            last_directory: None,
            remember_last_directory: true,
            copy_after_capture: true,
            close_after_copy: true,
            close_after_save: true,
        }
    }
}

impl ExportSettings {
    #[must_use]
    pub fn dialog_directory(&self) -> Option<&Path> {
        if self.remember_last_directory {
            self.last_directory
                .as_deref()
                .or(self.default_directory.as_deref())
        } else {
            self.default_directory.as_deref()
        }
    }

    #[must_use]
    pub fn quick_save_directory(&self) -> PathBuf {
        self.default_directory
            .clone()
            .unwrap_or_else(default_screenshot_directory)
    }
}

fn default_screenshot_directory() -> PathBuf {
    dirs::picture_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Pictures")))
        .unwrap_or_else(std::env::temp_dir)
        .join("Screenshots")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub schema_version: u32,
    pub appearance: AppearanceMode,
    pub language: LanguageMode,
    pub ocr: OcrSettings,
    pub export: ExportSettings,
    pub hotkeys: HotkeySettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            appearance: AppearanceMode::System,
            language: LanguageMode::System,
            ocr: OcrSettings::default(),
            export: ExportSettings::default(),
            hotkeys: HotkeySettings::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsLoad {
    pub settings: AppSettings,
    pub warning: Option<String>,
}

#[derive(Debug)]
pub enum SettingsError {
    Io(io::Error),
    Parse(serde_json::Error),
    UnsupportedSchema(u32),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "settings I/O failed: {error}"),
            Self::Parse(error) => write!(formatter, "settings file is invalid: {error}"),
            Self::UnsupportedSchema(version) => write!(
                formatter,
                "settings schema {version} is newer than supported schema {SETTINGS_SCHEMA_VERSION}"
            ),
        }
    }
}

impl std::error::Error for SettingsError {}

impl From<io::Error> for SettingsError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for SettingsError {
    fn from(error: serde_json::Error) -> Self {
        Self::Parse(error)
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::discover()
    }
}

impl SettingsStore {
    #[must_use]
    pub fn discover() -> Self {
        Self {
            path: default_config_directory().join("settings.json"),
        }
    }

    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        Self { path }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<AppSettings, SettingsError> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(AppSettings::default());
            }
            Err(error) => return Err(SettingsError::Io(error)),
        };

        let mut settings: AppSettings = serde_json::from_str(&contents)?;
        if settings.schema_version > SETTINGS_SCHEMA_VERSION {
            return Err(SettingsError::UnsupportedSchema(settings.schema_version));
        }

        if settings.ocr.text_model_id.is_none() {
            settings.ocr.text_model_id = settings.ocr.active_model_id.clone();
        }
        settings.schema_version = SETTINGS_SCHEMA_VERSION;
        Ok(settings)
    }

    #[must_use]
    pub fn load_or_default(&self) -> SettingsLoad {
        match self.load() {
            Ok(settings) => SettingsLoad {
                settings,
                warning: None,
            },
            Err(error) => SettingsLoad {
                settings: AppSettings::default(),
                warning: Some(error.to_string()),
            },
        }
    }

    pub fn load_for_update(&self) -> Result<AppSettings, SettingsError> {
        match self.load() {
            Ok(settings) => Ok(settings),
            Err(SettingsError::Parse(_)) => Ok(AppSettings::default()),
            Err(error) => Err(error),
        }
    }

    pub fn update<F>(&self, update: F) -> Result<AppSettings, SettingsError>
    where
        F: FnOnce(&mut AppSettings),
    {
        let _guard = SETTINGS_UPDATE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut settings = self.load_for_update()?;
        update(&mut settings);
        self.save(&settings)?;
        Ok(settings)
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), SettingsError> {
        let mut normalized = settings.clone();
        normalized.schema_version = SETTINGS_SCHEMA_VERSION;
        let bytes = serde_json::to_vec_pretty(&normalized)?;

        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;

        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("settings.json");
        let temporary = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);

        replace_file(&temporary, &self.path)?;
        sync_parent_directory(parent);
        Ok(())
    }
}

fn replace_file(source: &Path, target: &Path) -> io::Result<()> {
    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(_) if target.exists() => {
            fs::remove_file(target)?;
            fs::rename(source, target)
        }
        Err(error) => Err(error),
    }
}

fn sync_parent_directory(path: &Path) {
    if let Ok(directory) = File::open(path) {
        let _ = directory.sync_all();
    }
}

fn default_config_directory() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("AzusaLens")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store(name: &str) -> (PathBuf, SettingsStore) {
        let root = std::env::temp_dir().join(format!(
            "azusa-lens-settings-test-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let store = SettingsStore::from_path(root.join("settings.json"));
        (root, store)
    }

    #[test]
    fn missing_settings_use_defaults() {
        let (root, store) = test_store("defaults");
        assert_eq!(store.load().unwrap(), AppSettings::default());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn settings_round_trip() {
        let (root, store) = test_store("round-trip");
        let settings = AppSettings {
            appearance: AppearanceMode::Dark,
            language: LanguageMode::SimplifiedChinese,
            ocr: OcrSettings {
                text_model_id: Some("ppocrv6-small-mnn".to_owned()),
                document_model_id: Some("glm-ocr-ollama".to_owned()),
                table_model_id: Some("deepseek-ocr-ollama".to_owned()),
                ..OcrSettings::default()
            },
            export: ExportSettings {
                default_directory: Some(root.join("exports")),
                last_directory: Some(root.join("last")),
                copy_after_capture: false,
                close_after_save: false,
                ..ExportSettings::default()
            },
            hotkeys: HotkeySettings {
                capture: HotkeyBindingSettings {
                    control: true,
                    shift: true,
                    key: "KeyS".to_owned(),
                    ..HotkeyBindingSettings::default()
                },
            },
            ..AppSettings::default()
        };

        store.save(&settings).unwrap();
        assert_eq!(store.load().unwrap(), settings);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_single_model_becomes_text_model() {
        let (root, store) = test_store("v4-ocr-migration");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            store.path(),
            r#"{"schema_version":4,"appearance":"dark","ocr":{"active_model_id":"ppocrv6-small-mnn"}}"#,
        )
        .unwrap();

        let settings = store.load().unwrap();
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(
            settings.ocr.text_model_id.as_deref(),
            Some("ppocrv6-small-mnn")
        );
        assert_eq!(settings.ocr.document_model_id, None);
        assert_eq!(settings.ocr.table_model_id, None);
        assert_eq!(settings.ocr.figure_model_id, None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn v1_settings_receive_new_defaults_and_upgrade_in_memory() {
        let (root, store) = test_store("v1-migration");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            store.path(),
            r#"{"schema_version":1,"appearance":"dark","ocr":{"active_model_id":"ppocr-v6-small"}}"#,
        )
        .unwrap();

        let settings = store.load().unwrap();
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.appearance, AppearanceMode::Dark);
        assert_eq!(settings.export, ExportSettings::default());
        assert_eq!(settings.hotkeys, HotkeySettings::default());
        assert_eq!(
            settings.ocr.text_model_id.as_deref(),
            Some("ppocr-v6-small")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn v2_settings_receive_hotkey_defaults_and_upgrade_in_memory() {
        let (root, store) = test_store("v2-migration");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            store.path(),
            r#"{"schema_version":2,"appearance":"light","export":{"copy_after_capture":false}}"#,
        )
        .unwrap();

        let settings = store.load().unwrap();
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.appearance, AppearanceMode::Light);
        assert!(!settings.export.copy_after_capture);
        assert_eq!(settings.hotkeys, HotkeySettings::default());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn v3_settings_receive_language_default_and_upgrade_in_memory() {
        let (root, store) = test_store("v3-migration");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            store.path(),
            r#"{"schema_version":3,"appearance":"dark","export":{"copy_after_capture":false}}"#,
        )
        .unwrap();

        let settings = store.load().unwrap();
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.language, LanguageMode::System);
        assert_eq!(settings.appearance, AppearanceMode::Dark);
        assert!(!settings.export.copy_after_capture);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_fields_receive_current_defaults() {
        let (root, store) = test_store("missing-fields");
        fs::create_dir_all(&root).unwrap();
        fs::write(store.path(), r#"{"schema_version":1}"#).unwrap();

        assert_eq!(store.load().unwrap(), AppSettings::default());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_settings_fall_back_without_blocking_startup() {
        let (root, store) = test_store("malformed");
        fs::create_dir_all(&root).unwrap();
        fs::write(store.path(), "{ definitely not json").unwrap();

        let loaded = store.load_or_default();
        assert_eq!(loaded.settings, AppSettings::default());
        assert!(loaded.warning.is_some());
        assert_eq!(store.load_for_update().unwrap(), AppSettings::default());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn future_schema_falls_back_instead_of_being_overwritten() {
        let (root, store) = test_store("future-schema");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            store.path(),
            format!(
                r#"{{"schema_version":{},"appearance":"dark"}}"#,
                SETTINGS_SCHEMA_VERSION + 1
            ),
        )
        .unwrap();

        assert!(matches!(
            store.load(),
            Err(SettingsError::UnsupportedSchema(_))
        ));
        assert!(matches!(
            store.load_for_update(),
            Err(SettingsError::UnsupportedSchema(_))
        ));
        let loaded = store.load_or_default();
        assert_eq!(loaded.settings, AppSettings::default());
        assert!(loaded.warning.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn updates_preserve_unrelated_settings() {
        let (root, store) = test_store("update");
        let initial = AppSettings {
            appearance: AppearanceMode::Light,
            ocr: OcrSettings {
                text_model_id: Some("ppocrv6-small-mnn".to_owned()),
                table_model_id: Some("glm-ocr-ollama".to_owned()),
                ..OcrSettings::default()
            },
            export: ExportSettings {
                default_directory: Some(root.join("exports")),
                copy_after_capture: false,
                ..ExportSettings::default()
            },
            hotkeys: HotkeySettings {
                capture: HotkeyBindingSettings {
                    alt: true,
                    key: "F8".to_owned(),
                    ..HotkeyBindingSettings::default()
                },
            },
            ..AppSettings::default()
        };
        store.save(&initial).unwrap();

        let updated = store
            .update(|settings| settings.appearance = AppearanceMode::Dark)
            .unwrap();
        assert_eq!(updated.appearance, AppearanceMode::Dark);
        assert_eq!(updated.ocr, initial.ocr);
        assert_eq!(updated.export, initial.export);
        assert_eq!(updated.hotkeys, initial.hotkeys);
        assert_eq!(store.load().unwrap(), updated);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dialog_directory_prefers_last_location_when_enabled() {
        let settings = ExportSettings {
            default_directory: Some(PathBuf::from("default")),
            last_directory: Some(PathBuf::from("last")),
            remember_last_directory: true,
            ..ExportSettings::default()
        };
        assert_eq!(settings.dialog_directory(), Some(Path::new("last")));

        let settings = ExportSettings {
            remember_last_directory: false,
            ..settings
        };
        assert_eq!(settings.dialog_directory(), Some(Path::new("default")));
    }
}
