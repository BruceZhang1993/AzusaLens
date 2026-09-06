//! Cross-platform global shortcut support.
//!
//! Windows, macOS, and X11 use native platform hotkey APIs through the
//! Wayclip fork of `global-hotkey`. Wayland uses the XDG GlobalShortcuts
//! portal instead of falling back to XWayland.

use std::{error::Error, fmt};

use wayclip_global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey},
};

pub const APP_ID: &str = "com.azusaocr.AzusaOCR";

#[derive(Debug)]
pub struct HotkeyError(String);

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for HotkeyError {}

pub struct PrintScreenHotkey {
    _manager: GlobalHotKeyManager,
    hotkey: HotKey,
}

impl PrintScreenHotkey {
    pub fn register() -> Result<Self, HotkeyError> {
        configure_wayland_app_id();

        let manager = GlobalHotKeyManager::new()
            .map_err(|error| HotkeyError(format!("failed to initialize global hotkeys: {error}")))?;
        let hotkey = HotKey::new(None, Code::PrintScreen);
        manager
            .register(hotkey)
            .map_err(|error| HotkeyError(format!("failed to register PrtSc: {error}")))?;

        Ok(Self {
            _manager: manager,
            hotkey,
        })
    }

    #[must_use]
    pub fn take_pressed(&self) -> bool {
        let mut pressed = false;
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() && event.state == HotKeyState::Pressed {
                pressed = true;
            }
        }
        pressed
    }
}

#[cfg(target_os = "linux")]
fn configure_wayland_app_id() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("GLOBAL_HOTKEY_APP_ID").is_none()
    {
        // SAFETY: AzusaOCR calls this during single-threaded process startup,
        // before the hotkey crate creates its Wayland portal runtime.
        unsafe {
            std::env::set_var("GLOBAL_HOTKEY_APP_ID", APP_ID);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_wayland_app_id() {}

#[must_use]
pub fn backend_description() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Windows native global hotkey"
    }
    #[cfg(target_os = "macos")]
    {
        "macOS native global hotkey"
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            "Wayland XDG GlobalShortcuts portal"
        } else {
            "X11 native global hotkey"
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "Unsupported global hotkey backend"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_description_is_not_empty() {
        assert!(!backend_description().is_empty());
    }
}
