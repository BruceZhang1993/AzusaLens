use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use azusa_capture::dialogs::suggested_png_name;
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
    match try_run_headless(request) {
        Ok(path) => Ok(path),
        Err(error) => {
            eprintln!("Azusa Lens file OCR failed: {error}");
            std::process::exit(1);
        }
    }
}

fn try_run_headless(request: &FileOcrRequest) -> Result<PathBuf, String> {
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
    let parent = source_path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", source_path.display()))?;
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{} has no valid file name", source_path.display()))?;
    let base = format!("{stem}.azusa-{}", task.as_str());
    let output_path = write_unique_output(
        parent,
        &base,
        task.output_extension(),
        result.plain_text.as_bytes(),
    )?;
    open_with_default_app(&output_path)?;
    Ok(output_path)
}

pub(crate) fn finish_capture_result(
    task: OcrTaskKind,
    result: OcrResult,
) -> Result<PathBuf, String> {
    let directory = documents_directory()?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create Documents directory: {error}"))?;
    let timestamp = suggested_png_name()
        .strip_suffix(".png")
        .unwrap_or("Azusa-Lens-OCR")
        .to_owned();
    write_unique_output(
        &directory,
        &timestamp,
        task.output_extension(),
        result.plain_text.as_bytes(),
    )
}

fn write_unique_output(
    directory: &Path,
    base: &str,
    extension: &str,
    contents: &[u8],
) -> Result<PathBuf, String> {
    for suffix in 1..=9_999_u32 {
        let file_name = if suffix == 1 {
            format!("{base}.{extension}")
        } else {
            format!("{base}_{suffix}.{extension}")
        };
        let candidate = directory.join(file_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(contents) {
                    let _ = fs::remove_file(&candidate);
                    return Err(format!(
                        "could not write OCR result {}: {error}",
                        candidate.display()
                    ));
                }
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "could not create OCR result {}: {error}",
                    candidate.display()
                ));
            }
        }
    }
    Err("could not allocate a unique OCR output filename".to_owned())
}

fn documents_directory() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); [Console]::Write([Environment]::GetFolderPath('MyDocuments'))",
            ])
            .output()
            && output.status.success()
            && let Ok(path) = String::from_utf8(output.stdout)
            && !path.trim().is_empty()
        {
            return Ok(PathBuf::from(path.trim()));
        }
        if let Some(profile) = std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
            return Ok(PathBuf::from(profile).join("Documents"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        let home = std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| "HOME is not set; cannot resolve Documents directory".to_owned())?;
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        if let Ok(contents) = fs::read_to_string(config_home.join("user-dirs.dirs")) {
            for line in contents.lines() {
                let Some(value) = line.strip_prefix("XDG_DOCUMENTS_DIR=") else {
                    continue;
                };
                let value = value.trim().trim_matches('"');
                if !value.is_empty() {
                    let expanded = value.replace("$HOME", &home.to_string_lossy());
                    return Ok(PathBuf::from(expanded));
                }
            }
        }
        return Ok(home.join("Documents"));
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "HOME is not set; cannot resolve Documents directory".to_owned())?;
        return Ok(PathBuf::from(home).join("Documents"));
    }

    #[allow(unreachable_code)]
    Err("could not resolve the system Documents directory".to_owned())
}

pub(crate) fn open_with_default_app(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Invoke-Item -LiteralPath $env:AZUSA_OPEN_PATH",
        ]);
        command.env("AZUSA_OPEN_PATH", path);
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

pub(crate) fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        command.arg("/select,").arg(path);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg("-R").arg(path);
        command
    };
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path.parent().unwrap_or(path));
        command
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return Ok(());

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not reveal OCR result {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_names_include_task_and_extension() {
        let root = std::env::temp_dir().join(format!("azusa-file-ocr-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let text = write_unique_output(&root, "scan.azusa-text", "txt", b"text").unwrap();
        assert_eq!(text, root.join("scan.azusa-text.txt"));
        let document =
            write_unique_output(&root, "scan.azusa-document", "md", b"document").unwrap();
        assert_eq!(document, root.join("scan.azusa-document.md"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn output_names_are_reserved_atomically_and_avoid_existing_files() {
        let root =
            std::env::temp_dir().join(format!("azusa-file-ocr-collision-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("table.azusa-table.md"), b"old").unwrap();

        let output = write_unique_output(&root, "table.azusa-table", "md", b"new").unwrap();
        assert_eq!(output, root.join("table.azusa-table_2.md"));
        assert_eq!(fs::read(root.join("table.azusa-table.md")).unwrap(), b"old");
        assert_eq!(fs::read(&output).unwrap(), b"new");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn capture_output_names_are_timestamp_based() {
        let root = std::env::temp_dir().join(format!("azusa-capture-ocr-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let timestamp = suggested_png_name()
            .strip_suffix(".png")
            .unwrap()
            .to_owned();
        let path = write_unique_output(&root, &timestamp, "md", b"document").unwrap();
        assert_eq!(
            path.extension().and_then(|value| value.to_str()),
            Some("md")
        );
        assert!(
            path.file_stem()
                .and_then(|value| value.to_str())
                .is_some_and(|stem| stem.len() >= 19)
        );
        let _ = fs::remove_dir_all(&root);
    }
}
