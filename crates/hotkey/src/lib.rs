//! Cross-platform global shortcut support.
//!
//! Windows, macOS, and X11 use native platform hotkey APIs through the
//! Wayclip fork of `global-hotkey`. Wayland uses the XDG GlobalShortcuts
//! portal instead of falling back to XWayland.

use std::{error::Error, fmt, str::FromStr};

use wayclip_global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};

#[cfg(target_os = "linux")]
mod wayland;

pub const APP_ID: &str = "com.azusalens.AzusaLens";

#[derive(Debug)]
pub struct HotkeyError(String);

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for HotkeyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyBinding {
    control: bool,
    alt: bool,
    shift: bool,
    meta: bool,
    code: Code,
}

impl Default for HotkeyBinding {
    fn default() -> Self {
        Self::capture_default()
    }
}

impl HotkeyBinding {
    #[must_use]
    pub const fn capture_default() -> Self {
        Self {
            control: false,
            alt: false,
            shift: false,
            meta: false,
            code: Code::PrintScreen,
        }
    }

    pub fn from_parts(
        control: bool,
        alt: bool,
        shift: bool,
        meta: bool,
        code_name: &str,
    ) -> Result<Self, HotkeyError> {
        let code = Code::from_str(code_name)
            .map_err(|_| HotkeyError(format!("unsupported shortcut key: {code_name}")))?;
        let binding = Self {
            control,
            alt,
            shift,
            meta,
            code,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<(), HotkeyError> {
        if is_modifier_code(self.code) {
            return Err(HotkeyError(
                "a modifier key cannot be used as the shortcut trigger".to_owned(),
            ));
        }

        if !self.has_modifier() && !allows_bare_trigger(self.code) {
            return Err(HotkeyError(
                "ordinary keys require Ctrl, Alt, Shift, or Super to avoid intercepting normal typing"
                    .to_owned(),
            ));
        }

        Ok(())
    }

    #[must_use]
    pub const fn control(&self) -> bool {
        self.control
    }

    #[must_use]
    pub const fn alt(&self) -> bool {
        self.alt
    }

    #[must_use]
    pub const fn shift(&self) -> bool {
        self.shift
    }

    #[must_use]
    pub const fn meta(&self) -> bool {
        self.meta
    }

    #[must_use]
    pub fn code_name(&self) -> String {
        self.code.to_string()
    }

    #[must_use]
    pub fn display_label(&self) -> String {
        let mut parts = Vec::with_capacity(5);

        #[cfg(target_os = "macos")]
        {
            if self.control {
                parts.push("⌃".to_owned());
            }
            if self.alt {
                parts.push("⌥".to_owned());
            }
            if self.shift {
                parts.push("⇧".to_owned());
            }
            if self.meta {
                parts.push("⌘".to_owned());
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            if self.control {
                parts.push("Ctrl".to_owned());
            }
            if self.alt {
                parts.push("Alt".to_owned());
            }
            if self.shift {
                parts.push("Shift".to_owned());
            }
            if self.meta {
                parts.push(if cfg!(target_os = "windows") {
                    "Win".to_owned()
                } else {
                    "Super".to_owned()
                });
            }
        }

        parts.push(display_code(self.code));
        parts.join(if cfg!(target_os = "macos") { "" } else { "+" })
    }

    #[must_use]
    pub const fn has_modifier(&self) -> bool {
        self.control || self.alt || self.shift || self.meta
    }

    fn modifiers(&self) -> Option<Modifiers> {
        let mut modifiers = Modifiers::empty();
        if self.control {
            modifiers |= Modifiers::CONTROL;
        }
        if self.alt {
            modifiers |= Modifiers::ALT;
        }
        if self.shift {
            modifiers |= Modifiers::SHIFT;
        }
        if self.meta {
            modifiers |= Modifiers::SUPER;
        }
        (!modifiers.is_empty()).then_some(modifiers)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn xdg_trigger(&self) -> Option<String> {
        let key = xdg_key_name(self.code)?;
        let mut parts = Vec::with_capacity(5);
        if self.control {
            parts.push("CTRL");
        }
        if self.alt {
            parts.push("ALT");
        }
        if self.shift {
            parts.push("SHIFT");
        }
        if self.meta {
            parts.push("LOGO");
        }
        parts.push(key);
        Some(parts.join("+"))
    }
}

pub struct GlobalHotkey {
    backend: HotkeyBackend,
    hotkey: HotKey,
    binding: HotkeyBinding,
}

enum HotkeyBackend {
    Native(GlobalHotKeyManager),
    #[cfg(target_os = "linux")]
    Wayland(wayland::WaylandHotkey),
}

impl GlobalHotkey {
    pub fn register(binding: HotkeyBinding) -> Result<Self, HotkeyError> {
        binding.validate()?;
        configure_wayland_app_id();

        let hotkey = HotKey::new(binding.modifiers(), binding.code);

        #[cfg(target_os = "linux")]
        if use_wayland_portal() {
            let backend = wayland::WaylandHotkey::register(hotkey, &binding).map_err(|error| {
                HotkeyError(format!("failed to register {}: {error}", binding.display_label()))
            })?;
            return Ok(Self {
                backend: HotkeyBackend::Wayland(backend),
                hotkey,
                binding,
            });
        }

        let manager = GlobalHotKeyManager::new().map_err(|error| {
            HotkeyError(format!("failed to initialize global hotkeys: {error}"))
        })?;
        manager.register(hotkey).map_err(|error| {
            HotkeyError(format!(
                "failed to register {}: {error}",
                binding.display_label()
            ))
        })?;

        Ok(Self {
            backend: HotkeyBackend::Native(manager),
            hotkey,
            binding,
        })
    }

    #[must_use]
    pub fn binding(&self) -> &HotkeyBinding {
        &self.binding
    }

    #[must_use]
    pub fn effective_label(&self) -> String {
        match &self.backend {
            #[cfg(target_os = "linux")]
            HotkeyBackend::Wayland(backend) if !backend.trigger_description().is_empty() => {
                backend.trigger_description().to_owned()
            }
            _ => self.binding.display_label(),
        }
    }

    #[must_use]
    pub fn take_pressed(&self) -> bool {
        match &self.backend {
            #[cfg(target_os = "linux")]
            HotkeyBackend::Wayland(backend) => backend.take_pressed(),
            HotkeyBackend::Native(manager) => {
                let _ = manager;
                let mut pressed = false;
                while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                    if event.id == self.hotkey.id() && event.state == HotKeyState::Pressed {
                        pressed = true;
                    }
                }
                pressed
            }
        }
    }
}

impl Drop for GlobalHotkey {
    fn drop(&mut self) {
        if let HotkeyBackend::Native(manager) = &self.backend {
            let _ = manager.unregister(self.hotkey);
        }
    }
}

fn is_modifier_code(code: Code) -> bool {
    matches!(
        code,
        Code::ControlLeft
            | Code::ControlRight
            | Code::ShiftLeft
            | Code::ShiftRight
            | Code::AltLeft
            | Code::AltRight
            | Code::MetaLeft
            | Code::MetaRight
    )
}

fn allows_bare_trigger(code: Code) -> bool {
    code == Code::PrintScreen || matches!(
        code,
        Code::F1
            | Code::F2
            | Code::F3
            | Code::F4
            | Code::F5
            | Code::F6
            | Code::F7
            | Code::F8
            | Code::F9
            | Code::F10
            | Code::F11
            | Code::F12
            | Code::F13
            | Code::F14
            | Code::F15
            | Code::F16
            | Code::F17
            | Code::F18
            | Code::F19
            | Code::F20
            | Code::F21
            | Code::F22
            | Code::F23
            | Code::F24
    )
}

fn display_code(code: Code) -> String {
    let raw = code.to_string();
    match raw.as_str() {
        "PrintScreen" => "PrtSc".to_owned(),
        "Enter" => "Enter".to_owned(),
        "Escape" => "Esc".to_owned(),
        "Space" => "Space".to_owned(),
        "ArrowUp" => "↑".to_owned(),
        "ArrowDown" => "↓".to_owned(),
        "ArrowLeft" => "←".to_owned(),
        "ArrowRight" => "→".to_owned(),
        _ => raw
            .strip_prefix("Key")
            .or_else(|| raw.strip_prefix("Digit"))
            .unwrap_or(raw.as_str())
            .to_owned(),
    }
}

#[cfg(target_os = "linux")]
fn xdg_key_name(code: Code) -> Option<&'static str> {
    Some(match code {
        Code::PrintScreen => "Print",
        Code::KeyA => "a",
        Code::KeyB => "b",
        Code::KeyC => "c",
        Code::KeyD => "d",
        Code::KeyE => "e",
        Code::KeyF => "f",
        Code::KeyG => "g",
        Code::KeyH => "h",
        Code::KeyI => "i",
        Code::KeyJ => "j",
        Code::KeyK => "k",
        Code::KeyL => "l",
        Code::KeyM => "m",
        Code::KeyN => "n",
        Code::KeyO => "o",
        Code::KeyP => "p",
        Code::KeyQ => "q",
        Code::KeyR => "r",
        Code::KeyS => "s",
        Code::KeyT => "t",
        Code::KeyU => "u",
        Code::KeyV => "v",
        Code::KeyW => "w",
        Code::KeyX => "x",
        Code::KeyY => "y",
        Code::KeyZ => "z",
        Code::Digit0 => "0",
        Code::Digit1 => "1",
        Code::Digit2 => "2",
        Code::Digit3 => "3",
        Code::Digit4 => "4",
        Code::Digit5 => "5",
        Code::Digit6 => "6",
        Code::Digit7 => "7",
        Code::Digit8 => "8",
        Code::Digit9 => "9",
        Code::F1 => "F1",
        Code::F2 => "F2",
        Code::F3 => "F3",
        Code::F4 => "F4",
        Code::F5 => "F5",
        Code::F6 => "F6",
        Code::F7 => "F7",
        Code::F8 => "F8",
        Code::F9 => "F9",
        Code::F10 => "F10",
        Code::F11 => "F11",
        Code::F12 => "F12",
        Code::F13 => "F13",
        Code::F14 => "F14",
        Code::F15 => "F15",
        Code::F16 => "F16",
        Code::F17 => "F17",
        Code::F18 => "F18",
        Code::F19 => "F19",
        Code::F20 => "F20",
        Code::F21 => "F21",
        Code::F22 => "F22",
        Code::F23 => "F23",
        Code::F24 => "F24",
        Code::Enter => "Return",
        Code::Escape => "Escape",
        Code::Tab => "Tab",
        Code::Space => "space",
        Code::Backspace => "BackSpace",
        Code::Delete => "Delete",
        Code::Insert => "Insert",
        Code::Home => "Home",
        Code::End => "End",
        Code::PageUp => "Prior",
        Code::PageDown => "Next",
        Code::ArrowUp => "Up",
        Code::ArrowDown => "Down",
        Code::ArrowLeft => "Left",
        Code::ArrowRight => "Right",
        _ => return None,
    })
}

#[cfg(target_os = "linux")]
fn use_wayland_portal() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var("GDK_BACKEND")
            .map(|backend| backend != "x11")
            .unwrap_or(true)
}

#[cfg(target_os = "linux")]
fn configure_wayland_app_id() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("GLOBAL_HOTKEY_APP_ID").is_none()
    {
        // SAFETY: Azusa Lens calls this during single-threaded process startup,
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
        if use_wayland_portal() {
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

#[must_use]
pub fn portal_managed() -> bool {
    #[cfg(target_os = "linux")]
    {
        use_wayland_portal()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_description_is_not_empty() {
        assert!(!backend_description().is_empty());
    }

    #[test]
    fn default_binding_is_print_screen() {
        let binding = HotkeyBinding::capture_default();
        assert_eq!(binding.code_name(), "PrintScreen");
        assert!(!binding.has_modifier());
        assert!(binding.validate().is_ok());
    }

    #[test]
    fn configurable_binding_round_trips_parts() {
        let binding = HotkeyBinding::from_parts(true, true, false, false, "KeyS").unwrap();
        assert!(binding.control());
        assert!(binding.alt());
        assert!(!binding.shift());
        assert_eq!(binding.code_name(), "KeyS");
        assert!(binding.display_label().contains('S'));
    }

    #[test]
    fn ordinary_bare_keys_are_rejected() {
        let error = HotkeyBinding::from_parts(false, false, false, false, "KeyA").unwrap_err();
        assert!(error.to_string().contains("require"));
    }

    #[test]
    fn bare_function_keys_are_allowed() {
        assert!(HotkeyBinding::from_parts(false, false, false, false, "F8").is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn xdg_trigger_uses_portal_shortcut_syntax() {
        let binding = HotkeyBinding::from_parts(true, true, false, false, "KeyS").unwrap();
        assert_eq!(binding.xdg_trigger().as_deref(), Some("CTRL+ALT+s"));
        assert_eq!(HotkeyBinding::capture_default().xdg_trigger().as_deref(), Some("Print"));
    }
}
