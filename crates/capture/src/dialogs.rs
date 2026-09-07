use chrono::Local;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[must_use]
pub fn suggested_png_name() -> String {
    format!("{}.png", Local::now().format("%Y-%m-%d_%H-%M-%S"))
}

pub fn choose_png_save_path(
    initial_directory: Option<&Path>,
    suggested_name: &str,
) -> Result<Option<PathBuf>, String> {
    choose_png_save_path_impl(initial_directory, suggested_name)?
        .map(validate_png_extension)
        .transpose()
}

pub fn choose_directory(initial_directory: Option<&Path>) -> Result<Option<PathBuf>, String> {
    choose_directory_impl(initial_directory)
}

pub fn quick_png_save_path(directory: &Path, suggested_name: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("could not create screenshot directory: {error}"))?;
    let initial = validate_png_extension(directory.join(suggested_name))?;
    if !initial.exists() {
        return Ok(initial);
    }

    let stem = initial
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "screenshot filename is not valid UTF-8".to_owned())?;
    for suffix in 2..=9_999_u32 {
        let candidate = directory.join(format!("{stem}_{suffix}.png"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("could not allocate a unique screenshot filename".to_owned())
}

#[cfg(target_os = "linux")]
fn choose_png_save_path_impl(
    initial_directory: Option<&Path>,
    suggested_name: &str,
) -> Result<Option<PathBuf>, String> {
    use ashpd::{
        Error as PortalError,
        desktop::{
            ResponseError,
            file_chooser::{FileFilter, SelectedFiles},
        },
    };

    let runtime = portal_runtime()?;
    runtime.block_on(async {
        let mut chooser = SelectedFiles::save_file()
            .title("Save screenshot")
            .accept_label("Save")
            .modal(true)
            .current_name(suggested_name)
            .filter(FileFilter::new("PNG image").glob("*.png"));
        if let Some(directory) = initial_directory.filter(|path| path.is_dir()) {
            chooser = chooser
                .current_folder(directory)
                .map_err(|error| format!("invalid initial export directory: {error}"))?;
        }

        let request = chooser
            .send()
            .await
            .map_err(|error| format!("could not open the save dialog: {error}"))?;
        let response = match request.response() {
            Ok(response) => response,
            Err(PortalError::Response(ResponseError::Cancelled)) => return Ok(None),
            Err(error) => return Err(format!("save dialog failed: {error}")),
        };
        portal_selected_path(&response).map(Some)
    })
}

#[cfg(target_os = "linux")]
fn choose_directory_impl(initial_directory: Option<&Path>) -> Result<Option<PathBuf>, String> {
    use ashpd::{
        Error as PortalError,
        desktop::{ResponseError, file_chooser::SelectedFiles},
    };

    let runtime = portal_runtime()?;
    runtime.block_on(async {
        let mut chooser = SelectedFiles::open_file()
            .title("Choose default export folder")
            .accept_label("Select")
            .modal(true)
            .multiple(false)
            .directory(true);
        if let Some(directory) = initial_directory.filter(|path| path.is_dir()) {
            chooser = chooser
                .current_folder(directory)
                .map_err(|error| format!("invalid initial export directory: {error}"))?;
        }

        let request = chooser
            .send()
            .await
            .map_err(|error| format!("could not open the folder dialog: {error}"))?;
        let response = match request.response() {
            Ok(response) => response,
            Err(PortalError::Response(ResponseError::Cancelled)) => return Ok(None),
            Err(error) => return Err(format!("folder dialog failed: {error}")),
        };
        portal_selected_path(&response).map(Some)
    })
}

#[cfg(target_os = "linux")]
fn portal_runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to start portal runtime: {error}"))
}

#[cfg(target_os = "linux")]
fn portal_selected_path(
    response: &ashpd::desktop::file_chooser::SelectedFiles,
) -> Result<PathBuf, String> {
    let uri = response
        .uris()
        .first()
        .ok_or_else(|| "file chooser returned no selected path".to_owned())?;
    let uri = url::Url::parse(uri.as_str())
        .map_err(|error| format!("file chooser returned an invalid URI: {error}"))?;
    uri.to_file_path()
        .map_err(|()| "file chooser returned a non-file URI".to_owned())
}

#[cfg(target_os = "windows")]
fn choose_png_save_path_impl(
    initial_directory: Option<&Path>,
    suggested_name: &str,
) -> Result<Option<PathBuf>, String> {
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$dialog = New-Object System.Windows.Forms.SaveFileDialog
$dialog.Title = 'Save screenshot'
$dialog.Filter = 'PNG image (*.png)|*.png'
$dialog.DefaultExt = 'png'
$dialog.AddExtension = $true
$dialog.OverwritePrompt = $true
$dialog.FileName = $env:AZUSA_DIALOG_NAME
if ($env:AZUSA_DIALOG_DIR -and (Test-Path -LiteralPath $env:AZUSA_DIALOG_DIR -PathType Container)) {
    $dialog.InitialDirectory = $env:AZUSA_DIALOG_DIR
}
if ((Show-AzusaDialog $dialog) -eq [System.Windows.Forms.DialogResult]::OK) {
    [Console]::Write($dialog.FileName)
}
"#;

    run_powershell_dialog(SCRIPT, initial_directory, Some(suggested_name))
}

#[cfg(target_os = "windows")]
fn choose_directory_impl(initial_directory: Option<&Path>) -> Result<Option<PathBuf>, String> {
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = 'Choose default export folder'
$dialog.ShowNewFolderButton = $true
if ($env:AZUSA_DIALOG_DIR -and (Test-Path -LiteralPath $env:AZUSA_DIALOG_DIR -PathType Container)) {
    $dialog.SelectedPath = $env:AZUSA_DIALOG_DIR
}
if ((Show-AzusaDialog $dialog) -eq [System.Windows.Forms.DialogResult]::OK) {
    [Console]::Write($dialog.SelectedPath)
}
"#;

    run_powershell_dialog(SCRIPT, initial_directory, None)
}

#[cfg(target_os = "windows")]
fn run_powershell_dialog(
    script: &str,
    initial_directory: Option<&Path>,
    suggested_name: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    use std::process::Command;

    const DIALOG_OWNER_HELPER: &str = r#"
Add-Type -AssemblyName System.Windows.Forms
function Show-AzusaDialog($dialog) {
    $owner = New-Object System.Windows.Forms.Form
    $owner.ShowInTaskbar = $false
    $owner.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None
    $owner.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
    $owner.Location = New-Object System.Drawing.Point(-32000, -32000)
    $owner.Size = New-Object System.Drawing.Size(1, 1)
    $owner.Opacity = 0
    $owner.TopMost = $true
    $owner.Show()
    try {
        return $dialog.ShowDialog($owner)
    } finally {
        $owner.Close()
        $owner.Dispose()
    }
}
"#;

    let full_script = format!("{DIALOG_OWNER_HELPER}\n{script}");
    let mut command = Command::new("powershell.exe");
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-STA",
        "-Command",
        &full_script,
    ]);
    command.env(
        "AZUSA_DIALOG_DIR",
        initial_directory.unwrap_or_else(|| Path::new("")),
    );
    command.env("AZUSA_DIALOG_NAME", suggested_name.unwrap_or(""));
    let output = command
        .output()
        .map_err(|error| format!("failed to start Windows file dialog: {error}"))?;
    command_output_path(output, "Windows file dialog")
}

#[cfg(target_os = "macos")]
fn choose_png_save_path_impl(
    initial_directory: Option<&Path>,
    suggested_name: &str,
) -> Result<Option<PathBuf>, String> {
    const SCRIPT: &str = r#"
set targetName to system attribute "AZUSA_DIALOG_NAME"
set targetDirectory to system attribute "AZUSA_DIALOG_DIR"
try
    if targetDirectory is "" then
        set chosenFile to choose file name with prompt "Save screenshot" default name targetName
    else
        set chosenFile to choose file name with prompt "Save screenshot" default location (POSIX file targetDirectory) default name targetName
    end if
    return POSIX path of chosenFile
on error number -128
    return ""
end try
"#;

    run_osascript_dialog(SCRIPT, initial_directory, Some(suggested_name))
}

#[cfg(target_os = "macos")]
fn choose_directory_impl(initial_directory: Option<&Path>) -> Result<Option<PathBuf>, String> {
    const SCRIPT: &str = r#"
set targetDirectory to system attribute "AZUSA_DIALOG_DIR"
try
    if targetDirectory is "" then
        set chosenFolder to choose folder with prompt "Choose default export folder"
    else
        set chosenFolder to choose folder with prompt "Choose default export folder" default location (POSIX file targetDirectory)
    end if
    return POSIX path of chosenFolder
on error number -128
    return ""
end try
"#;

    run_osascript_dialog(SCRIPT, initial_directory, None)
}

#[cfg(target_os = "macos")]
fn run_osascript_dialog(
    script: &str,
    initial_directory: Option<&Path>,
    suggested_name: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    use std::process::Command;

    let mut command = Command::new("osascript");
    command.args(["-e", script]);
    command.env(
        "AZUSA_DIALOG_DIR",
        initial_directory.unwrap_or_else(|| Path::new("")),
    );
    command.env("AZUSA_DIALOG_NAME", suggested_name.unwrap_or(""));
    let output = command
        .output()
        .map_err(|error| format!("failed to start macOS file dialog: {error}"))?;
    command_output_path(output, "macOS file dialog")
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn command_output_path(
    output: std::process::Output,
    dialog_name: &str,
) -> Result<Option<PathBuf>, String> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if stderr.is_empty() {
            format!("{dialog_name} exited with status {}", output.status)
        } else {
            format!("{dialog_name} failed: {stderr}")
        });
    }

    let path = String::from_utf8(output.stdout)
        .map_err(|error| format!("{dialog_name} returned non-UTF-8 output: {error}"))?;
    let path = path.trim();
    Ok((!path.is_empty()).then(|| PathBuf::from(path)))
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn choose_png_save_path_impl(
    _initial_directory: Option<&Path>,
    _suggested_name: &str,
) -> Result<Option<PathBuf>, String> {
    Err("native save dialogs are not supported on this platform".to_owned())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn choose_directory_impl(_initial_directory: Option<&Path>) -> Result<Option<PathBuf>, String> {
    Err("native folder dialogs are not supported on this platform".to_owned())
}

fn validate_png_extension(path: PathBuf) -> Result<PathBuf, String> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        Ok(path)
    } else {
        Err("save filename must end in .png".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_name_uses_filename_safe_local_datetime_shape() {
        let name = suggested_png_name();
        assert_eq!(name.len(), 23);
        assert!(name.ends_with(".png"));
        for index in [4, 7] {
            assert_eq!(name.as_bytes()[index], b'-');
        }
        assert_eq!(name.as_bytes()[10], b'_');
        for index in [13, 16] {
            assert_eq!(name.as_bytes()[index], b'-');
        }
        assert!(
            name[..19]
                .chars()
                .enumerate()
                .all(|(index, character)| [4, 7, 10, 13, 16].contains(&index)
                    || character.is_ascii_digit())
        );
    }

    #[test]
    fn png_extension_must_be_explicit() {
        assert_eq!(
            validate_png_extension(PathBuf::from("capture.PNG")).unwrap(),
            PathBuf::from("capture.PNG")
        );
        assert!(validate_png_extension(PathBuf::from("capture")).is_err());
        assert!(validate_png_extension(PathBuf::from("capture.jpg")).is_err());
    }

    #[test]
    fn quick_save_path_creates_directory_and_avoids_collisions() {
        let directory =
            std::env::temp_dir().join(format!("azusa-lens-quick-save-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let first = quick_png_save_path(&directory, "2026-09-08_12-34-56.png").unwrap();
        assert_eq!(first, directory.join("2026-09-08_12-34-56.png"));
        fs::write(&first, b"png").unwrap();
        let second = quick_png_save_path(&directory, "2026-09-08_12-34-56.png").unwrap();
        assert_eq!(second, directory.join("2026-09-08_12-34-56_2.png"));
        let _ = fs::remove_dir_all(directory);
    }
}
