use std::{fs, path::{Path, PathBuf}, process::Command};

use azusa_ocr::{OcrImage, OcrModelManager, OcrResult, OcrTaskKind, create_engine};

#[derive(Debug, Clone)]
pub(crate) struct FileOcrRequest {
    pub(crate) task: OcrTaskKind,
    pub(crate) source_path: PathBuf,
}

#[must_use]
pub(crate) fn request_from_args() -> Option<FileOcrRequest> {
    let mut args = std::env::args_os().skip(1);
    let mut task = None;
    let mut source_path = None;
    while let Some(argument) = args.next() {
        match argument.to_string_lossy().as_ref() {
            "--ocr-type" => {
                task = args
                    .next()
                    .and_then(|value| OcrTaskKind::from_value(&value.to_string_lossy()));
            }
            "--ocr-file" => source_path = args.next().map(PathBuf::from),
            _ => {}
        }
    }
    source_path.map(|source_path| FileOcrRequest {
        // The file-manager action intentionally defaults to document parsing.
        task: task.unwrap_or(OcrTaskKind::Document),
        source_path,
    })
}

pub(crate) fn run_headless(request: &FileOcrRequest) -> Result<PathBuf, String> {
    let manager = OcrModelManager::discover();
    let model_id = manager.active_model_id_for(request.task).ok_or_else(|| {
        format!(
            "No model is configured for {}. Open Settings > OCR Models & Routing first.",
            request.task.display_name()
        )
    })?;
    let mut engine = create_engine(&model_id).map_err(|error| error.to_string())?;
    let input = OcrImage::open_path(&request.source_path).map_err(|error| error.to_string())?;
    let result = engine
        .recognize_task(request.task, &input)
        .map_err(|error| error.to_string())?;
    finish_result(request.task, &request.source_path, result)
}

pub(crate) fn finish_result(
    task: OcrTaskKind,
    source_path: &Path,
    result: OcrResult,
) -> Result<PathBuf, String> {
    let output_path = unique_output_path(task, source_path)?;
    fs::write(&output_path, result.plain_text.as_bytes())
        .map_err(|error| format!("could not write OCR result {}: {error}", output_path.display()))?;
    open_with_default_app(&output_path)?;
    Ok(output_path)
}

fn unique_output_path(task: OcrTaskKind, source_path: &Path) -> Result<PathBuf, String> {
    let parent = source_path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", source_path.display()))?;
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{} has no valid file name", source_path.display()))?;
    let extension = task.output_extension();
    let base = format!("{stem}.azusa-{}", task.as_str());
    let first = parent.join(format!("{base}.{extension}"));
    if !first.exists() {
        return Ok(first);
    }
    for suffix in 2..=9_999_u32 {
        let candidate = parent.join(format!("{base}_{suffix}.{extension}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("could not allocate a unique OCR output filename".to_owned())
}

fn open_with_default_app(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd.exe");
        command.args(["/C", "start", "", &path.to_string_lossy()]);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return Ok(());

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not open OCR result {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_names_include_task_and_extension() {
        let root = std::env::temp_dir().join(format!("azusa-file-ocr-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("scan.png");
        fs::write(&source, b"not decoded in this test").unwrap();
        assert_eq!(
            unique_output_path(OcrTaskKind::Text, &source).unwrap(),
            root.join("scan.azusa-text.txt")
        );
        assert_eq!(
            unique_output_path(OcrTaskKind::Document, &source).unwrap(),
            root.join("scan.azusa-document.md")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn output_names_avoid_existing_files() {
        let root = std::env::temp_dir().join(format!("azusa-file-ocr-collision-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("table.jpg");
        fs::write(&source, b"source").unwrap();
        fs::write(root.join("table.azusa-table.md"), b"old").unwrap();
        assert_eq!(
            unique_output_path(OcrTaskKind::Table, &source).unwrap(),
            root.join("table.azusa-table_2.md")
        );
        let _ = fs::remove_dir_all(&root);
    }
}
