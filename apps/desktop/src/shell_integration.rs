#[cfg(target_os = "linux")]
use std::{fs, path::PathBuf};
#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::process::Command;

pub(crate) const CONTEXT_MENU_LABEL: &str = "使用 Azusa Lens 识别";

#[must_use]
pub(crate) const fn is_supported() -> bool {
    cfg!(any(target_os = "windows", target_os = "linux"))
}

#[must_use]
pub(crate) fn status_detail() -> String {
    #[cfg(target_os = "windows")]
    {
        return "Adds “使用 Azusa Lens 识别” to the context menu for supported image files. The action uses the configured Document OCR model.".to_owned();
    }
    #[cfg(target_os = "linux")]
    {
        return "Registers a KDE service-menu action and a freedesktop image handler named “使用 Azusa Lens 识别”. Other file managers may expose it under Open With.".to_owned();
    }
    #[cfg(target_os = "macos")]
    {
        return "Finder context-menu registration is not enabled yet. Azusa Lens does not install a Finder extension or Service automatically.".to_owned();
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        "File-manager context-menu registration is not supported on this platform.".to_owned()
    }
}

#[must_use]
pub(crate) fn is_registered() -> bool {
    #[cfg(target_os = "windows")]
    {
        return windows_context_menu_registered();
    }
    #[cfg(target_os = "linux")]
    {
        return linux_context_menu_registered();
    }
    #[allow(unreachable_code)]
    false
}

pub(crate) fn register() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        return register_windows_context_menu();
    }
    #[cfg(target_os = "linux")]
    {
        return register_linux_context_menu();
    }
    #[allow(unreachable_code)]
    Err(status_detail())
}

pub(crate) fn unregister() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        return unregister_windows_context_menu();
    }
    #[cfg(target_os = "linux")]
    {
        return unregister_linux_context_menu();
    }
    #[allow(unreachable_code)]
    Err(status_detail())
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn integration_executable() -> Result<std::path::PathBuf, String> {
    #[cfg(target_os = "linux")]
    if let Some(appimage) = std::env::var_os("APPIMAGE").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(appimage));
    }

    std::env::current_exe()
        .map_err(|error| format!("could not resolve Azusa Lens executable: {error}"))
}

#[cfg(target_os = "windows")]
const WINDOWS_IMAGE_EXTENSIONS: &[&str] =
    &[".png", ".jpg", ".jpeg", ".webp", ".bmp", ".tif", ".tiff"];

#[cfg(target_os = "windows")]
fn windows_context_menu_key(extension: &str) -> String {
    format!(r"HKCU\Software\Classes\SystemFileAssociations\{extension}\shell\AzusaLensOCR")
}

#[cfg(target_os = "windows")]
fn windows_context_menu_registered() -> bool {
    WINDOWS_IMAGE_EXTENSIONS.iter().all(|extension| {
        Command::new("reg.exe")
            .args(["query", &windows_context_menu_key(extension)])
            .output()
            .is_ok_and(|output| output.status.success())
    })
}

#[cfg(target_os = "windows")]
fn register_windows_context_menu() -> Result<(), String> {
    let executable = integration_executable()?;
    let executable = executable.to_string_lossy();
    let command = format!("\"{executable}\" --ocr-type document --ocr-file \"%1\"");

    for extension in WINDOWS_IMAGE_EXTENSIONS {
        let key = windows_context_menu_key(extension);
        run_reg(["add", key.as_str(), "/ve", "/d", CONTEXT_MENU_LABEL, "/f"])?;
        run_reg([
            "add",
            key.as_str(),
            "/v",
            "Icon",
            "/d",
            executable.as_ref(),
            "/f",
        ])?;
        let command_key = format!(r"{key}\command");
        run_reg([
            "add",
            command_key.as_str(),
            "/ve",
            "/d",
            command.as_str(),
            "/f",
        ])?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn unregister_windows_context_menu() -> Result<(), String> {
    for extension in WINDOWS_IMAGE_EXTENSIONS {
        let key = windows_context_menu_key(extension);
        let output = Command::new("reg.exe")
            .args(["delete", key.as_str(), "/f"])
            .output()
            .map_err(|error| format!("failed to start reg.exe: {error}"))?;
        // `reg delete` returns a failure when a key is already absent; absence is the desired state.
        if !output.status.success() && windows_key_exists(&key) {
            return Err(reg_error("delete context-menu key", output));
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_key_exists(key: &str) -> bool {
    Command::new("reg.exe")
        .args(["query", key])
        .output()
        .is_ok_and(|output| output.status.success())
}

#[cfg(target_os = "windows")]
fn run_reg<const N: usize>(args: [&str; N]) -> Result<(), String> {
    let output = Command::new("reg.exe")
        .args(args)
        .output()
        .map_err(|error| format!("failed to start reg.exe: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(reg_error("update context-menu registration", output))
    }
}

#[cfg(target_os = "windows")]
fn reg_error(action: &str, output: std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let detail = if stderr.is_empty() { stdout } else { stderr };
    if detail.is_empty() {
        format!("failed to {action}: {}", output.status)
    } else {
        format!("failed to {action}: {detail}")
    }
}

#[cfg(target_os = "linux")]
fn linux_data_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "HOME is not set; cannot register file-manager integration".to_owned())?;
    Ok(PathBuf::from(home).join(".local/share"))
}

#[cfg(target_os = "linux")]
fn linux_service_menu_path() -> Result<PathBuf, String> {
    Ok(linux_data_home()?.join("kio/servicemenus/azusa-lens-ocr.desktop"))
}

#[cfg(target_os = "linux")]
fn linux_desktop_handler_path() -> Result<PathBuf, String> {
    Ok(linux_data_home()?.join("applications/azusa-lens-ocr.desktop"))
}

#[cfg(target_os = "linux")]
fn linux_context_menu_registered() -> bool {
    linux_service_menu_path().is_ok_and(|path| path.is_file())
        && linux_desktop_handler_path().is_ok_and(|path| path.is_file())
}

#[cfg(target_os = "linux")]
fn register_linux_context_menu() -> Result<(), String> {
    let executable = integration_executable()?;
    let executable = desktop_exec_quote(&executable.to_string_lossy());
    let mime_types = "image/png;image/jpeg;image/webp;image/bmp;image/tiff;";

    let service_menu = linux_service_menu_path()?;
    ensure_parent(&service_menu)?;
    fs::write(
        &service_menu,
        format!(
            "[Desktop Entry]\nType=Service\nMimeType={mime_types}\nActions=AzusaLensOCR;\nX-KDE-ServiceTypes=KonqPopupMenu/Plugin\n\n[Desktop Action AzusaLensOCR]\nName={CONTEXT_MENU_LABEL}\nIcon=com.azusalens.AzusaLens\nExec={executable} --ocr-type document --ocr-file %f\n"
        ),
    )
    .map_err(|error| format!("could not write {}: {error}", service_menu.display()))?;

    let desktop_handler = linux_desktop_handler_path()?;
    ensure_parent(&desktop_handler)?;
    fs::write(
        &desktop_handler,
        format!(
            "[Desktop Entry]\nType=Application\nName={CONTEXT_MENU_LABEL}\nComment=Recognize an image with Azusa Lens document OCR\nIcon=com.azusalens.AzusaLens\nNoDisplay=true\nTerminal=false\nExec={executable} --ocr-type document --ocr-file %f\nMimeType={mime_types}\nCategories=Graphics;Utility;\n"
        ),
    )
    .map_err(|error| format!("could not write {}: {error}", desktop_handler.display()))?;

    let _ = Command::new("update-desktop-database")
        .arg(linux_data_home()?.join("applications"))
        .output();
    Ok(())
}

#[cfg(target_os = "linux")]
fn unregister_linux_context_menu() -> Result<(), String> {
    for path in [linux_service_menu_path()?, linux_desktop_handler_path()?] {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("could not remove {}: {error}", path.display())),
        }
    }
    let _ = Command::new("update-desktop-database")
        .arg(linux_data_home()?.join("applications"))
        .output();
    Ok(())
}

#[cfg(target_os = "linux")]
fn ensure_parent(path: &std::path::Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))
}

#[cfg(target_os = "linux")]
fn desktop_exec_quote(path: &str) -> String {
    format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_matches_product_copy() {
        assert_eq!(CONTEXT_MENU_LABEL, "使用 Azusa Lens 识别");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_exec_path_is_quoted() {
        assert_eq!(
            desktop_exec_quote("/tmp/Azusa Lens/azusa-lens"),
            "\"/tmp/Azusa Lens/azusa-lens\""
        );
    }
}
