use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[must_use]
pub fn suggested_png_name() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    suggested_png_name_at(seconds)
}

pub fn choose_png_save_path(
    initial_directory: Option<&Path>,
    suggested_name: &str,
) -> Result<Option<PathBuf>, String> {
    choose_png_save_path_impl(initial_directory, suggested_name)
        .map(|path| path.map(ensure_png_extension))
}

pub fn choose_directory(initial_directory: Option<&Path>) -> Result<Option<PathBuf>, String> {
    choose_directory_impl(initial_directory)
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
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
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
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
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

    let mut command = Command::new("powershell.exe");
    command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-STA", "-Command", script]);
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

fn ensure_png_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        path
    } else {
        path.with_extension("png")
    }
}

fn suggested_png_name_at(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_date_from_unix_days(days);
    format!("AzusaLens_{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}Z.png")
}

fn civil_date_from_unix_days(days: i64) -> (i64, u32, u32) {
    // Howard Hinnant's civil-from-days algorithm, with day zero at 1970-01-01.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096)
            / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_name_is_stable_for_unix_epoch() {
        assert_eq!(
            suggested_png_name_at(0),
            "AzusaLens_19700101_000000Z.png"
        );
    }

    #[test]
    fn suggested_name_handles_leap_day() {
        // 2024-02-29 12:34:56 UTC.
        assert_eq!(
            suggested_png_name_at(1_709_210_096),
            "AzusaLens_20240229_123456Z.png"
        );
    }

    #[test]
    fn png_extension_is_preserved_or_added() {
        assert_eq!(
            ensure_png_extension(PathBuf::from("capture.PNG")),
            PathBuf::from("capture.PNG")
        );
        assert_eq!(
            ensure_png_extension(PathBuf::from("capture")),
            PathBuf::from("capture.png")
        );
        assert_eq!(
            ensure_png_extension(PathBuf::from("capture.jpg")),
            PathBuf::from("capture.png")
        );
    }
}
