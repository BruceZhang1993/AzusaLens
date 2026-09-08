use std::{cell::RefCell, rc::Rc, sync::mpsc, thread, time::Duration};

use azusa_config::{AppSettings, HotkeyBindingSettings, SettingsStore};
use azusa_hotkey::{
    GlobalHotkey, HotkeyBinding, backend_description as hotkey_backend_description, portal_managed,
};
use slint::{ComponentHandle, SharedString, Timer, TimerMode, platform::Key};

use crate::{AppWindow, feedback};

struct PendingRegistration {
    receiver: mpsc::Receiver<Result<GlobalHotkey, String>>,
    requested: HotkeyBinding,
    previous: Option<HotkeyBinding>,
    persist: bool,
    rollback: bool,
}

pub(crate) struct HotkeyController {
    active: Rc<RefCell<Option<Rc<GlobalHotkey>>>>,
    _registration_timer: Timer,
}

impl HotkeyController {
    pub(crate) fn new(
        ui: &AppWindow,
        settings_store: SettingsStore,
        app_settings: Rc<RefCell<AppSettings>>,
    ) -> Self {
        let configured = binding_from_settings(&app_settings.borrow()).unwrap_or_else(|error| {
            feedback::set_status_text(
                &ui,
                format!("Saved shortcut is invalid · {error} · using PrtSc until it is changed")
                    .into(),
            );
            HotkeyBinding::capture_default()
        });

        ui.set_hotkey_backend_name(hotkey_backend_description().into());
        ui.set_hotkey_portal_managed(portal_managed());
        ui.set_hotkey_configured_name(configured.display_label().into());
        ui.set_hotkey_name(configured.display_label().into());
        ui.set_hotkey_active(false);
        ui.set_hotkey_registration_pending(false);

        let active = Rc::new(RefCell::new(None::<Rc<GlobalHotkey>>));
        let pending = Rc::new(RefCell::new(None::<PendingRegistration>));

        {
            let weak = ui.as_weak();
            let active = Rc::clone(&active);
            let pending = Rc::clone(&pending);
            let app_settings = Rc::clone(&app_settings);
            ui.on_hotkey_register_requested(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                if pending.borrow().is_some() {
                    return;
                }
                let binding = binding_from_settings(&app_settings.borrow())
                    .unwrap_or_else(|_| HotkeyBinding::capture_default());
                active.borrow_mut().take();
                start_registration(&ui, &pending, binding, None, false, false);
            });
        }

        {
            let weak = ui.as_weak();
            let active = Rc::clone(&active);
            let pending = Rc::clone(&pending);
            let app_settings = Rc::clone(&app_settings);
            ui.on_hotkey_binding_requested(move |control, alt, shift, meta, key_text| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                if pending.borrow().is_some() {
                    return;
                }

                let Some(code_name) = slint_key_to_code_name(key_text.as_str()) else {
                    feedback::set_status_text(
                        &ui,
                        "Unsupported shortcut key · choose another key".into(),
                    );
                    return;
                };
                let (control, meta) = platform_modifier_mapping(control, meta);
                let requested = match HotkeyBinding::from_parts(
                    control,
                    alt,
                    shift,
                    meta,
                    code_name.as_str(),
                ) {
                    Ok(binding) => binding,
                    Err(error) => {
                        feedback::set_status_text(
                            &ui,
                            format!("Shortcut rejected · {error}").into(),
                        );
                        return;
                    }
                };

                let (previous, matches_saved) =
                    previous_binding(&app_settings.borrow(), &requested);
                if matches_saved && active.borrow().is_some() {
                    feedback::set_status_text(&ui, "Shortcut is already active".into());
                    return;
                }

                active.borrow_mut().take();
                start_registration(&ui, &pending, requested, Some(previous), true, false);
            });
        }

        {
            let weak = ui.as_weak();
            let active = Rc::clone(&active);
            let pending = Rc::clone(&pending);
            let app_settings = Rc::clone(&app_settings);
            ui.on_hotkey_reset_requested(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                if pending.borrow().is_some() {
                    return;
                }
                let requested = HotkeyBinding::capture_default();
                let (previous, matches_saved) =
                    previous_binding(&app_settings.borrow(), &requested);
                if matches_saved && active.borrow().is_some() {
                    feedback::set_status_text(&ui, "PrtSc is already the active shortcut".into());
                    return;
                }

                active.borrow_mut().take();
                start_registration(&ui, &pending, requested, Some(previous), true, false);
            });
        }

        let registration_timer = Timer::default();
        {
            let weak = ui.as_weak();
            let active = Rc::clone(&active);
            let pending = Rc::clone(&pending);
            let settings_store = settings_store.clone();
            let app_settings = Rc::clone(&app_settings);
            registration_timer.start(
                TimerMode::Repeated,
                Duration::from_millis(40),
                move || {
                    let completed = {
                        let mut pending_state = pending.borrow_mut();
                        let Some(registration) = pending_state.as_ref() else {
                            return;
                        };
                        match registration.receiver.try_recv() {
                            Ok(result) => {
                                let registration = pending_state
                                    .take()
                                    .expect("pending registration existed above");
                                Some((registration, result))
                            }
                            Err(mpsc::TryRecvError::Disconnected) => {
                                let registration = pending_state
                                    .take()
                                    .expect("pending registration existed above");
                                Some((
                                    registration,
                                    Err("registration worker disconnected".to_owned()),
                                ))
                            }
                            Err(mpsc::TryRecvError::Empty) => None,
                        }
                    };

                    let Some((registration, result)) = completed else {
                        return;
                    };
                    let Some(ui) = weak.upgrade() else {
                        return;
                    };

                    match result {
                        Ok(hotkey) => {
                            if registration.persist {
                                match settings_store.update(|settings| {
                                    write_binding(settings, hotkey.binding());
                                }) {
                                    Ok(updated) => {
                                        *app_settings.borrow_mut() = updated;
                                    }
                                    Err(error) => {
                                        drop(hotkey);
                                        if let Some(previous) = registration.previous {
                                            feedback::set_status_text(&ui,
                                                format!(
                                                    "Could not save shortcut · {error} · restoring {}",
                                                    previous.display_label()
                                                )
                                                .into(),
                                            );
                                            start_registration(
                                                &ui,
                                                &pending,
                                                previous,
                                                None,
                                                false,
                                                true,
                                            );
                                        } else {
                                            ui.set_hotkey_registration_pending(false);
                                            ui.set_hotkey_active(false);
                                            feedback::set_status_text(&ui,
                                                format!("Could not save shortcut · {error}").into(),
                                            );
                                        }
                                        return;
                                    }
                                }
                            }

                            let configured = hotkey.binding().display_label();
                            let effective = hotkey.effective_label();
                            ui.set_hotkey_configured_name(configured.clone().into());
                            ui.set_hotkey_name(effective.clone().into());
                            ui.set_hotkey_registration_pending(false);
                            ui.set_hotkey_active(true);
                            *active.borrow_mut() = Some(Rc::new(hotkey));

                            if registration.rollback {
                                feedback::set_status_text(&ui,
                                    format!("Previous shortcut restored · {effective}").into(),
                                );
                            } else if portal_managed() && effective != configured {
                                feedback::set_status_text(&ui,
                                    format!(
                                        "Shortcut active · requested {configured} · system assigned {effective}"
                                    )
                                    .into(),
                                );
                            } else {
                                feedback::set_status_text(&ui,
                                    format!("Screenshot shortcut active · {effective}").into(),
                                );
                            }
                        }
                        Err(error) => {
                            if let Some(previous) = registration.previous {
                                feedback::set_status_text(&ui,
                                    format!(
                                        "Shortcut {} failed · {error} · restoring {}",
                                        registration.requested.display_label(),
                                        previous.display_label()
                                    )
                                    .into(),
                                );
                                start_registration(
                                    &ui,
                                    &pending,
                                    previous,
                                    None,
                                    false,
                                    true,
                                );
                            } else {
                                ui.set_hotkey_registration_pending(false);
                                ui.set_hotkey_active(false);
                                ui.set_hotkey_name(
                                    format!(
                                        "{} unavailable",
                                        registration.requested.display_label()
                                    )
                                    .into(),
                                );
                                feedback::set_status_text(&ui,
                                    format!(
                                        "Screenshot shortcut registration failed · {error} · click retry to try again"
                                    )
                                    .into(),
                                );
                            }
                        }
                    }
                },
            );
        }

        start_registration(ui, &pending, configured, None, false, false);

        Self {
            active,
            _registration_timer: registration_timer,
        }
    }

    #[must_use]
    pub(crate) fn take_pressed(&self) -> bool {
        self.active
            .borrow()
            .as_ref()
            .is_some_and(|hotkey| hotkey.take_pressed())
    }
}

fn start_registration(
    ui: &AppWindow,
    pending: &Rc<RefCell<Option<PendingRegistration>>>,
    requested: HotkeyBinding,
    previous: Option<HotkeyBinding>,
    persist: bool,
    rollback: bool,
) {
    if pending.borrow().is_some() {
        return;
    }

    let label = requested.display_label();
    ui.set_hotkey_registration_pending(true);
    ui.set_hotkey_active(false);
    ui.set_hotkey_name(format!("Registering {label}…").into());
    if !rollback {
        feedback::set_status_text(
            &ui,
            format!("Registering screenshot shortcut · {label}").into(),
        );
    }
    *pending.borrow_mut() = Some(PendingRegistration {
        receiver: begin_registration(requested.clone()),
        requested,
        previous,
        persist,
        rollback,
    });
}

fn begin_registration(binding: HotkeyBinding) -> mpsc::Receiver<Result<GlobalHotkey, String>> {
    let (sender, receiver) = mpsc::channel();

    #[cfg(target_os = "linux")]
    {
        let worker_sender = sender.clone();
        if let Err(error) = thread::Builder::new()
            .name("azusa-hotkey-registration".to_owned())
            .spawn(move || {
                let result = GlobalHotkey::register(binding).map_err(|error| error.to_string());
                let _ = worker_sender.send(result);
            })
        {
            let _ = sender.send(Err(format!("failed to start registration worker: {error}")));
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = sender.send(GlobalHotkey::register(binding).map_err(|error| error.to_string()));
    }

    receiver
}

fn binding_from_settings(settings: &AppSettings) -> Result<HotkeyBinding, String> {
    let binding = &settings.hotkeys.capture;
    HotkeyBinding::from_parts(
        binding.control,
        binding.alt,
        binding.shift,
        binding.meta,
        binding.key.as_str(),
    )
    .map_err(|error| error.to_string())
}

fn previous_binding(settings: &AppSettings, requested: &HotkeyBinding) -> (HotkeyBinding, bool) {
    match binding_from_settings(settings) {
        Ok(binding) => {
            let matches_saved = &binding == requested;
            (binding, matches_saved)
        }
        Err(_) => (HotkeyBinding::capture_default(), false),
    }
}

fn write_binding(settings: &mut AppSettings, binding: &HotkeyBinding) {
    settings.hotkeys.capture = HotkeyBindingSettings {
        control: binding.control(),
        alt: binding.alt(),
        shift: binding.shift(),
        meta: binding.meta(),
        key: binding.code_name(),
    };
}

fn platform_modifier_mapping(control: bool, meta: bool) -> (bool, bool) {
    #[cfg(target_os = "macos")]
    {
        // Slint intentionally maps Command to `control` and physical Control to
        // `meta` on macOS for app shortcuts. Global-hotkey needs the physical
        // modifiers, so swap them back while recording user bindings.
        (meta, control)
    }
    #[cfg(not(target_os = "macos"))]
    {
        (control, meta)
    }
}

fn slint_key_to_code_name(text: &str) -> Option<String> {
    let named = [
        (Key::Return, "Enter"),
        (Key::Escape, "Escape"),
        (Key::Tab, "Tab"),
        (Key::Backspace, "Backspace"),
        (Key::Delete, "Delete"),
        (Key::Space, "Space"),
        (Key::UpArrow, "ArrowUp"),
        (Key::DownArrow, "ArrowDown"),
        (Key::LeftArrow, "ArrowLeft"),
        (Key::RightArrow, "ArrowRight"),
        (Key::Insert, "Insert"),
        (Key::Home, "Home"),
        (Key::End, "End"),
        (Key::PageUp, "PageUp"),
        (Key::PageDown, "PageDown"),
        (Key::SysReq, "PrintScreen"),
        (Key::F1, "F1"),
        (Key::F2, "F2"),
        (Key::F3, "F3"),
        (Key::F4, "F4"),
        (Key::F5, "F5"),
        (Key::F6, "F6"),
        (Key::F7, "F7"),
        (Key::F8, "F8"),
        (Key::F9, "F9"),
        (Key::F10, "F10"),
        (Key::F11, "F11"),
        (Key::F12, "F12"),
        (Key::F13, "F13"),
        (Key::F14, "F14"),
        (Key::F15, "F15"),
        (Key::F16, "F16"),
        (Key::F17, "F17"),
        (Key::F18, "F18"),
        (Key::F19, "F19"),
        (Key::F20, "F20"),
        (Key::F21, "F21"),
        (Key::F22, "F22"),
        (Key::F23, "F23"),
        (Key::F24, "F24"),
    ];
    for (key, code) in named {
        let value = SharedString::from(key);
        if value.as_str() == text {
            return Some(code.to_owned());
        }
    }

    let mut characters = text.chars();
    let character = characters.next()?;
    if characters.next().is_some() {
        return None;
    }

    if character.is_ascii_alphabetic() {
        return Some(format!("Key{}", character.to_ascii_uppercase()));
    }
    if character.is_ascii_digit() {
        return Some(format!("Digit{character}"));
    }

    Some(
        match character {
            '`' | '~' => "Backquote",
            '-' | '_' => "Minus",
            '=' | '+' => "Equal",
            '[' | '{' => "BracketLeft",
            ']' | '}' => "BracketRight",
            '\\' | '|' => "Backslash",
            ';' | ':' => "Semicolon",
            '\'' | '"' => "Quote",
            ',' | '<' => "Comma",
            '.' | '>' => "Period",
            '/' | '?' => "Slash",
            '!' => "Digit1",
            '@' => "Digit2",
            '#' => "Digit3",
            '$' => "Digit4",
            '%' => "Digit5",
            '^' => "Digit6",
            '&' => "Digit7",
            '*' => "Digit8",
            '(' => "Digit9",
            ')' => "Digit0",
            _ => return None,
        }
        .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_print_screen_and_function_keys() {
        assert_eq!(
            slint_key_to_code_name(SharedString::from(Key::SysReq).as_str()).as_deref(),
            Some("PrintScreen")
        );
        assert_eq!(
            slint_key_to_code_name(SharedString::from(Key::F8).as_str()).as_deref(),
            Some("F8")
        );
    }

    #[test]
    fn maps_printable_keys_to_physical_codes() {
        assert_eq!(slint_key_to_code_name("s").as_deref(), Some("KeyS"));
        assert_eq!(slint_key_to_code_name("S").as_deref(), Some("KeyS"));
        assert_eq!(slint_key_to_code_name("1").as_deref(), Some("Digit1"));
        assert_eq!(slint_key_to_code_name("!").as_deref(), Some("Digit1"));
        assert_eq!(slint_key_to_code_name("+").as_deref(), Some("Equal"));
    }

    #[test]
    fn invalid_saved_binding_does_not_match_default_fallback() {
        let mut settings = AppSettings::default();
        settings.hotkeys.capture.key = "DefinitelyInvalid".to_owned();
        let requested = HotkeyBinding::capture_default();

        let (previous, matches_saved) = previous_binding(&settings, &requested);
        assert_eq!(previous, requested);
        assert!(!matches_saved);
    }
}
