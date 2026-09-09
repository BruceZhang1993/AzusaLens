mod controllers;
mod editor;
mod feedback;
mod hotkeys;
mod i18n;

#[cfg(test)]
mod overlay_tests;

use std::{borrow::Cow, cell::RefCell, rc::Rc, time::Duration};

use arboard::{Clipboard, ImageData};
use azusa_capture::{
    CaptureRect, CapturedFrame, detected_backend,
    dialogs::{choose_directory, choose_png_save_path, quick_png_save_path, suggested_png_name},
};
use azusa_config::{AppSettings, AppearanceMode, LanguageMode, SettingsStore};
use azusa_ocr::{OcrImage, OcrModelManager};
use controllers::{
    capture::{CaptureController, CaptureEvent, CaptureOrigin},
    ocr::{OcrActionResult, OcrController, OcrEvent, OcrModelAction},
};
use editor::{BeginResult, EditorSession};
use hotkeys::HotkeyController;
use slint::{
    ComponentHandle, Image, Model, ModelRc, PhysicalPosition, Rgba8Pixel, SharedPixelBuffer, Timer,
    TimerMode, VecModel,
    winit_030::{WinitWindowAccessor, winit},
};

slint::include_modules!();

thread_local! {
    static CLIPBOARD: RefCell<Option<Clipboard>> = const { RefCell::new(None) };
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let overlay = RegionOverlay::new()?;
    let tray = AppTray::new()?;

    let settings_store = SettingsStore::discover();
    let loaded_settings = settings_store.load_or_default();
    if let Err(error) = i18n::apply_language(loaded_settings.settings.language) {
        eprintln!("Could not apply configured language: {error}");
    }
    ui.global::<Theme>()
        .set_mode(loaded_settings.settings.appearance.as_str().into());
    let app_settings = Rc::new(RefCell::new(loaded_settings.settings));
    sync_export_settings_ui(&ui, &app_settings.borrow());
    let settings_warning = loaded_settings.warning;

    let latest_frame = Rc::new(RefCell::new(None::<CapturedFrame>));
    let editor = Rc::new(RefCell::new(EditorSession::default()));
    let capture_controller = CaptureController::default();

    let ocr_model_manager = OcrModelManager::discover();
    let ocr_event_manager = ocr_model_manager.clone();
    let ocr_event_ui = ui.as_weak();
    let ocr_controller = OcrController::new(ocr_model_manager.clone(), move |event| {
        if let Some(ui) = ocr_event_ui.upgrade() {
            handle_ocr_event(&ui, &ocr_event_manager, event);
        }
    });

    ui.set_platform_name(detected_backend().to_string().into());
    sync_ocr_model_ui(&ui, &ocr_model_manager);
    feedback::set_status_text(
        &ui,
        "Ready · capture a region, then use the overlay tools".into(),
    );
    if let Some(warning) = settings_warning {
        feedback::set_status_text(
            &ui,
            format!("Settings loaded with safe defaults · {warning}").into(),
        );
    }
    let app_icon = make_app_icon();
    ui.set_app_icon(app_icon.clone());
    overlay.set_app_icon(app_icon.clone());
    tray.set_app_icon(app_icon);

    {
        let weak = ui.as_weak();
        let settings_store = settings_store.clone();
        let app_settings = Rc::clone(&app_settings);
        ui.global::<Theme>().on_mode_change_requested(move |mode| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let Some(appearance) = AppearanceMode::from_value(mode.as_str()) else {
                feedback::set_status_text(
                    &ui,
                    format!("Unsupported appearance mode · {mode}").into(),
                );
                return;
            };
            match settings_store.update(|settings| settings.appearance = appearance) {
                Ok(updated) => {
                    *app_settings.borrow_mut() = updated;
                    sync_export_settings_ui(&ui, &app_settings.borrow());
                    feedback::set_status_text(
                        &ui,
                        format!("Appearance saved · {}", appearance.as_str()).into(),
                    )
                }
                Err(error) => feedback::set_status_text(
                    &ui,
                    format!("Could not save appearance · {error}").into(),
                ),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let settings_store = settings_store.clone();
        let app_settings = Rc::clone(&app_settings);
        ui.on_language_change_requested(move |mode| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let Some(language) = LanguageMode::from_value(mode.as_str()) else {
                feedback::set_status_text(
                    &ui,
                    format!("Unsupported language mode · {mode}").into(),
                );
                return;
            };
            let previous_language = app_settings.borrow().language;
            if let Err(error) = i18n::apply_language(language) {
                feedback::set_status_text(
                    &ui,
                    format!("Could not apply language · {error}").into(),
                );
                return;
            }
            match settings_store.update(|settings| settings.language = language) {
                Ok(updated) => {
                    *app_settings.borrow_mut() = updated;
                    sync_export_settings_ui(&ui, &app_settings.borrow());
                    feedback::set_status_text(&ui, "Language saved".into());
                }
                Err(error) => {
                    if let Err(rollback_error) = i18n::apply_language(previous_language) {
                        feedback::set_status_text(
                            &ui,
                            format!(
                                "Could not save language · {error}; rollback failed · {rollback_error}"
                            )
                            .into(),
                        );
                    } else {
                        sync_export_settings_ui(&ui, &app_settings.borrow());
                        feedback::set_status_text(
                            &ui,
                            format!("Could not save language · {error}").into(),
                        );
                    }
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let settings_store = settings_store.clone();
        let app_settings = Rc::clone(&app_settings);
        ui.on_export_directory_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let initial_directory = app_settings
                .borrow()
                .export
                .dialog_directory()
                .map(|path| path.to_path_buf());
            match choose_directory(initial_directory.as_deref()) {
                Ok(Some(directory)) => match settings_store.update(|settings| {
                    settings.export.default_directory = Some(directory.clone());
                    settings.export.last_directory = None;
                }) {
                    Ok(updated) => {
                        *app_settings.borrow_mut() = updated;
                        sync_export_settings_ui(&ui, &app_settings.borrow());
                        feedback::set_status_text(
                            &ui,
                            format!("Default export directory saved · {}", directory.display())
                                .into(),
                        );
                    }
                    Err(error) => feedback::set_status_text(
                        &ui,
                        format!("Could not save export directory · {error}").into(),
                    ),
                },
                Ok(None) => feedback::set_status_text(&ui, "Export directory unchanged".into()),
                Err(error) => ui
                    .set_status_text(format!("Could not choose export directory · {error}").into()),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let settings_store = settings_store.clone();
        let app_settings = Rc::clone(&app_settings);
        ui.on_export_directory_reset_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            match settings_store.update(|settings| {
                settings.export.default_directory = None;
                settings.export.last_directory = None;
            }) {
                Ok(updated) => {
                    *app_settings.borrow_mut() = updated;
                    sync_export_settings_ui(&ui, &app_settings.borrow());
                    feedback::set_status_text(&ui, "Default export directory reset".into());
                }
                Err(error) => feedback::set_status_text(
                    &ui,
                    format!("Could not reset export directory · {error}").into(),
                ),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let settings_store = settings_store.clone();
        let app_settings = Rc::clone(&app_settings);
        ui.on_export_behavior_change_requested(
            move |remember_last, copy_after_capture, close_after_copy, close_after_save| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                match settings_store.update(|settings| {
                    settings.export.remember_last_directory = remember_last;
                    settings.export.copy_after_capture = copy_after_capture;
                    settings.export.close_after_copy = close_after_copy;
                    settings.export.close_after_save = close_after_save;
                }) {
                    Ok(updated) => {
                        *app_settings.borrow_mut() = updated;
                        sync_export_settings_ui(&ui, &app_settings.borrow());
                        feedback::set_status_text(&ui, "Export preferences saved".into());
                    }
                    Err(error) => feedback::set_status_text(
                        &ui,
                        format!("Could not save export preferences · {error}").into(),
                    ),
                }
            },
        );
    }

    let start_capture: Rc<dyn Fn(CaptureOrigin)> = {
        let ui_weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let capture_controller = capture_controller.clone();

        Rc::new(move |origin| {
            if capture_controller.is_active() {
                return;
            }

            let Some(ui) = ui_weak.upgrade() else {
                return;
            };

            feedback::set_status_text(&ui, "Starting region capture…".into());
            let _ = ui.hide();
            if let Some(overlay) = overlay_weak.upgrade() {
                overlay.set_editor_visible(false);
                overlay.set_capture_sequence(overlay.get_capture_sequence().wrapping_add(1));
                let _ = overlay.hide();
                set_overlay_windowed(&overlay);
            }

            let event_ui_weak = ui.as_weak();
            let event_overlay_weak = overlay_weak.clone();
            let event_controller = capture_controller.clone();
            match capture_controller.start(origin, move |event| {
                let Some(ui) = event_ui_weak.upgrade() else {
                    event_controller.reset();
                    return;
                };
                let Some(overlay) = event_overlay_weak.upgrade() else {
                    event_controller.reset();
                    if matches!(origin, CaptureOrigin::MainWindow) {
                        let _ = ui.show();
                    }
                    return;
                };

                match event {
                    CaptureEvent::SelectionReady {
                        origin,
                        anchor_x,
                        anchor_y,
                        frame,
                    } => {
                        overlay.set_screenshot(frame_to_image(&frame));
                        event_controller.store_selection(origin, frame);
                        set_overlay_windowed(&overlay);
                        overlay
                            .window()
                            .set_position(PhysicalPosition::new(anchor_x, anchor_y));
                        let monitor_selected =
                            set_overlay_fullscreen_on_monitor(&overlay, anchor_x, anchor_y);

                        if let Err(error) = overlay.show() {
                            event_controller.reset();
                            feedback::set_status_text(
                                &ui,
                                format!("Selection overlay failed · {error}").into(),
                            );
                            if matches!(origin, CaptureOrigin::MainWindow) {
                                let _ = ui.show();
                            }
                        } else {
                            if !monitor_selected
                                && !set_overlay_fullscreen_on_monitor(&overlay, anchor_x, anchor_y)
                            {
                                overlay.window().set_fullscreen(true);
                            }
                            focus_overlay(&overlay);
                        }
                    }
                    CaptureEvent::Failed { origin, error } => {
                        event_controller.reset();
                        feedback::set_status_text(
                            &ui,
                            format!("Region capture failed · {error}").into(),
                        );
                        if matches!(origin, CaptureOrigin::MainWindow) {
                            let _ = ui.show();
                        }
                    }
                }
            }) {
                Ok(true) => {}
                Ok(false) => {
                    if matches!(origin, CaptureOrigin::MainWindow) {
                        let _ = ui.show();
                    }
                }
                Err(error) => {
                    feedback::set_status_text(
                        &ui,
                        format!("Region capture worker failed · {error}").into(),
                    );
                    if matches!(origin, CaptureOrigin::MainWindow) {
                        let _ = ui.show();
                    }
                }
            }
        })
    };

    {
        let start_capture = Rc::clone(&start_capture);
        ui.on_capture_requested(move || start_capture(CaptureOrigin::MainWindow));
    }

    {
        let ui_weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        let capture_controller = capture_controller.clone();
        let app_settings = Rc::clone(&app_settings);

        overlay.on_selection_confirmed(
            move |selection_x, selection_y, selection_width, selection_height| {
                let Some(ui) = ui_weak.upgrade() else {
                    return;
                };
                let Some(overlay) = overlay_weak.upgrade() else {
                    return;
                };
                let Some((origin, frame)) = capture_controller.take_selection() else {
                    return;
                };

                let rect = selection_to_capture_rect(
                    &overlay,
                    &frame,
                    selection_x,
                    selection_y,
                    selection_width,
                    selection_height,
                );
                match frame.crop(rect) {
                    Ok(frame) => {
                        finish_capture(&ui, &latest_frame, &editor, &app_settings.borrow(), frame);
                        editor.borrow_mut().set_tool("select");
                        ui.set_active_tool("select".into());
                        sync_editor_overlay(&ui, &overlay);
                        overlay.set_editor_visible(true);
                    }
                    Err(error) => {
                        capture_controller.reset();
                        let _ = overlay.hide();
                        set_overlay_windowed(&overlay);
                        feedback::set_status_text(
                            &ui,
                            format!("Region crop failed · {error}").into(),
                        );
                        if matches!(origin, CaptureOrigin::MainWindow) {
                            let _ = ui.show();
                        }
                    }
                }
            },
        );
    }

    {
        let ui_weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let capture_controller = capture_controller.clone();
        overlay.on_cancelled(move || {
            if let Some(overlay) = overlay_weak.upgrade() {
                overlay.set_editor_visible(false);
                let _ = overlay.hide();
                set_overlay_windowed(&overlay);
            }
            let origin = capture_controller
                .cancel_selection()
                .unwrap_or(CaptureOrigin::Background);
            if let Some(ui) = ui_weak.upgrade() {
                feedback::set_status_text(&ui, "Region capture cancelled".into());
                if matches!(origin, CaptureOrigin::MainWindow) {
                    let _ = ui.show();
                }
            }
        });
    }

    {
        macro_rules! forward_overlay_no_args {
            ($on:ident, $invoke:ident) => {{
                let ui_weak = ui.as_weak();
                let overlay_weak = overlay.as_weak();
                overlay.$on(move || {
                    if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                        ui.$invoke();
                        sync_editor_overlay(&ui, &overlay);
                    }
                });
            }};
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_capture_requested(move || {
                if let Some(overlay) = overlay_weak.upgrade() {
                    overlay.set_editor_visible(false);
                    let _ = overlay.hide();
                    set_overlay_windowed(&overlay);
                }
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_capture_requested();
                }
            });
        }

        forward_overlay_no_args!(on_copy_requested, invoke_copy_requested);
        forward_overlay_no_args!(on_save_requested, invoke_save_requested);
        forward_overlay_no_args!(on_save_as_requested, invoke_save_as_requested);
        forward_overlay_no_args!(on_ocr_requested, invoke_ocr_requested);
        forward_overlay_no_args!(on_copy_ocr_requested, invoke_copy_ocr_requested);
        forward_overlay_no_args!(on_undo_requested, invoke_undo_requested);
        forward_overlay_no_args!(on_redo_requested, invoke_redo_requested);
        forward_overlay_no_args!(
            on_delete_selection_requested,
            invoke_delete_selection_requested
        );
        forward_overlay_no_args!(
            on_clear_annotations_requested,
            invoke_clear_annotations_requested
        );

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_tool_selected(move |tool| {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.set_active_tool(tool.clone());
                    ui.invoke_tool_selected(tool);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_color_selected(move |index| {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.set_active_color(index);
                    ui.invoke_color_selected(index);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_stroke_selected(move |width| {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.set_active_stroke(width);
                    ui.invoke_stroke_selected(width);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_editor_pointer_down(move |x, y, width, height| {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.invoke_editor_pointer_down(x, y, width, height);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_editor_pointer_moved(move |x, y, width, height| {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.invoke_editor_pointer_moved(x, y, width, height);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_editor_pointer_up(move |x, y, width, height| {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.invoke_editor_pointer_up(x, y, width, height);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_text_submit(move |value| {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.set_pending_text(value.clone());
                    ui.invoke_text_submit(value);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_nudge_selection_requested(move |dx, dy| {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.invoke_nudge_selection_requested(dx, dy);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let weak = ui.as_weak();
            overlay.on_ocr_hit_test(move |x, y| {
                weak.upgrade()
                    .map(|ui| ui.invoke_ocr_hit_test(x, y))
                    .unwrap_or(-1)
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_copy_ocr_selection_requested(move |anchor, focus| {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.invoke_copy_ocr_selection_requested(anchor, focus);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_ocr_selection_to_text_requested(move |anchor, focus| {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.invoke_ocr_selection_to_text_requested(anchor, focus);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_editor_state_changed(move || {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.set_pending_text(overlay.get_pending_text());
                    ui.set_active_tool(overlay.get_active_tool());
                    ui.set_active_color(overlay.get_active_color());
                    ui.set_active_stroke(overlay.get_active_stroke());
                    ui.set_zoom_factor(overlay.get_zoom_factor());
                    ui.set_pan_x(overlay.get_pan_x());
                    ui.set_pan_y(overlay.get_pan_y());
                    ui.set_ocr_overlay_visible(overlay.get_ocr_overlay_visible());
                    ui.set_ocr_selection_anchor(overlay.get_ocr_selection_anchor());
                    ui.set_ocr_selection_focus(overlay.get_ocr_selection_focus());
                    ui.set_ocr_hover_index(overlay.get_ocr_hover_index());
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_status_text_changed(move || {
                if let (Some(ui), Some(overlay)) = (ui_weak.upgrade(), overlay_weak.upgrade()) {
                    ui.set_status_text(overlay.get_status_text());
                }
            });
        }
    }

    {
        let weak = ui.as_weak();
        let editor = Rc::clone(&editor);
        ui.on_tool_selected(move |tool| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if editor.borrow_mut().set_tool(tool.as_str()) {
                ui.set_text_entry_visible(false);
                sync_selection(&ui, &editor.borrow());
                if tool.as_str() == "select" {
                    feedback::set_status_text(
                        &ui,
                        "Select tool · click an object, then drag it or use a resize handle".into(),
                    );
                } else {
                    feedback::set_status_text(&ui, format!("Annotation tool · {tool}").into());
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_color_selected(move |index| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = editor.borrow_mut().set_color(index);
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    feedback::set_status_text(&ui, "Updated selected object color".into());
                }
                Ok(None) => {}
                Err(error) => {
                    feedback::set_status_text(&ui, format!("Color update failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_stroke_selected(move |width| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = editor.borrow_mut().set_stroke_width(width);
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    feedback::set_status_text(&ui, "Updated selected object size".into());
                }
                Ok(None) => {}
                Err(error) => {
                    feedback::set_status_text(&ui, format!("Size update failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let editor = Rc::clone(&editor);
        ui.on_editor_pointer_down(move |x, y, width, height| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let begin_result = editor.borrow_mut().begin_canvas(x, y, width, height);
            match begin_result {
                BeginResult::TextInput => {
                    ui.set_pending_text("".into());
                    ui.set_text_entry_visible(true);
                    feedback::set_status_text(
                        &ui,
                        "Text anchor placed · type text and press Enter".into(),
                    );
                }
                BeginResult::TextEdit => {
                    let value = editor
                        .borrow()
                        .pending_text_value()
                        .unwrap_or_default()
                        .to_owned();
                    ui.set_pending_text(value.into());
                    ui.set_text_entry_visible(true);
                    feedback::set_status_text(
                        &ui,
                        "Editing text annotation · Enter to apply · Esc to cancel".into(),
                    );
                }
                BeginResult::Drawing => ui.set_text_entry_visible(false),
                BeginResult::Ignored => {}
            }
            sync_selection(&ui, &editor.borrow());
        });
    }

    {
        let weak = ui.as_weak();
        let editor = Rc::clone(&editor);
        ui.on_editor_pointer_moved(move |x, y, width, height| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = editor.borrow_mut().move_canvas(x, y, width, height);
            match result {
                Ok(Some(frame)) => ui.set_preview_image(frame_to_image(&frame)),
                Ok(None) => {}
                Err(error) => feedback::set_status_text(
                    &ui,
                    format!("Annotation preview failed · {error}").into(),
                ),
            }
            sync_selection(&ui, &editor.borrow());
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_editor_pointer_up(move |x, y, width, height| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.get_text_entry_visible() {
                sync_selection(&ui, &editor.borrow());
                return;
            }
            let was_select = editor.borrow().is_select_tool();
            let result = editor.borrow_mut().end_canvas(x, y, width, height);
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    if was_select {
                        feedback::set_status_text(&ui,
                            "Selection updated · drag to move, resize with handles, or edit properties"
                                .into(),
                        );
                    } else {
                        feedback::set_status_text(&ui,
                            "Annotation added · continue editing or export the image".into(),
                        );
                    }
                }
                Ok(None) => sync_selection(&ui, &editor.borrow()),
                Err(error) => feedback::set_status_text(&ui, format!("Annotation failed · {error}").into()),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_text_submit(move |value| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let was_editing = editor.borrow().is_editing_text();
            let result = editor.borrow_mut().commit_text(value.as_str());
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    ui.set_text_entry_visible(false);
                    ui.set_pending_text("".into());
                    feedback::set_status_text(
                        &ui,
                        if was_editing {
                            "Text annotation updated".into()
                        } else {
                            "Text annotation added".into()
                        },
                    );
                }
                Ok(None) => {
                    ui.set_text_entry_visible(false);
                    ui.set_pending_text("".into());
                }
                Err(error) => feedback::set_status_text(
                    &ui,
                    format!("Text annotation failed · {error}").into(),
                ),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_undo_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = editor.borrow_mut().undo();
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    feedback::set_status_text(&ui, "Undid last editor change".into());
                }
                Ok(None) => {}
                Err(error) => {
                    feedback::set_status_text(&ui, format!("Undo failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_redo_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = editor.borrow_mut().redo();
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    feedback::set_status_text(&ui, "Redid editor change".into());
                }
                Ok(None) => {}
                Err(error) => {
                    feedback::set_status_text(&ui, format!("Redo failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_nudge_selection_requested(move |dx, dy| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = editor.borrow_mut().nudge_selected(dx, dy);
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                }
                Ok(None) => {}
                Err(error) => feedback::set_status_text(
                    &ui,
                    format!("Move selected annotation failed · {error}").into(),
                ),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_delete_selection_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = editor.borrow_mut().delete_selected();
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    feedback::set_status_text(&ui, "Deleted selected annotation".into());
                }
                Ok(None) => {}
                Err(error) => {
                    feedback::set_status_text(&ui, format!("Delete failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_clear_annotations_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = editor.borrow_mut().clear();
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    feedback::set_status_text(&ui, "All annotations cleared".into());
                }
                Ok(None) => {}
                Err(error) => {
                    feedback::set_status_text(&ui, format!("Clear failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let app_settings = Rc::clone(&app_settings);
        ui.on_copy_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let frame = latest_frame.borrow();
            let Some(frame) = frame.as_ref() else {
                feedback::set_status_text(&ui, "Nothing to copy · capture a region first".into());
                return;
            };

            match copy_to_clipboard(frame) {
                Ok(()) => {
                    feedback::set_status_text(&ui, "Edited image copied to clipboard".into());
                    if app_settings.borrow().export.close_after_copy
                        && let Some(overlay) = overlay_weak.upgrade()
                        && overlay.get_editor_visible()
                    {
                        schedule_capture_exit(&ui, &overlay);
                    }
                }
                Err(error) => {
                    feedback::set_status_text(&ui, format!("Clipboard failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let app_settings = Rc::clone(&app_settings);
        ui.on_save_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let frame = {
                let frame = latest_frame.borrow();
                let Some(frame) = frame.as_ref() else {
                    feedback::set_status_text(
                        &ui,
                        "Nothing to save · capture a region first".into(),
                    );
                    return;
                };
                frame.clone()
            };
            let (directory, close_after_save) = {
                let settings = app_settings.borrow();
                (
                    settings.export.quick_save_directory(),
                    settings.export.close_after_save,
                )
            };
            let suggested_name = suggested_png_name();
            let path = match quick_png_save_path(&directory, &suggested_name) {
                Ok(path) => path,
                Err(error) => {
                    feedback::set_status_text(&ui, format!("Quick save failed · {error}").into());
                    return;
                }
            };

            match frame.save_png(&path) {
                Ok(()) => {
                    feedback::set_status_text(&ui, format!("Saved · {}", path.display()).into());
                    if close_after_save
                        && let Some(overlay) = overlay_weak.upgrade()
                        && overlay.get_editor_visible()
                    {
                        schedule_capture_exit(&ui, &overlay);
                    }
                }
                Err(error) => {
                    feedback::set_status_text(&ui, format!("Save failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let app_settings = Rc::clone(&app_settings);
        ui.on_save_as_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let frame = {
                let frame = latest_frame.borrow();
                let Some(frame) = frame.as_ref() else {
                    feedback::set_status_text(
                        &ui,
                        "Nothing to save · capture a region first".into(),
                    );
                    return;
                };
                frame.clone()
            };
            let (initial_directory, close_after_save) = {
                let settings = app_settings.borrow();
                (
                    settings.export.quick_save_directory(),
                    settings.export.close_after_save,
                )
            };
            let _ = std::fs::create_dir_all(&initial_directory);
            let suggested_name = suggested_png_name();
            let path = match choose_png_save_path(Some(&initial_directory), &suggested_name) {
                Ok(Some(path)) => path,
                Ok(None) => {
                    feedback::set_status_text(&ui, "Save As cancelled".into());
                    return;
                }
                Err(error) => {
                    feedback::set_status_text(
                        &ui,
                        format!("Could not open Save As · {error}").into(),
                    );
                    return;
                }
            };

            match frame.save_png(&path) {
                Ok(()) => {
                    feedback::set_status_text(&ui, format!("Saved As · {}", path.display()).into());
                    if close_after_save
                        && let Some(overlay) = overlay_weak.upgrade()
                        && overlay.get_editor_visible()
                    {
                        schedule_capture_exit(&ui, &overlay);
                    }
                }
                Err(error) => {
                    feedback::set_status_text(&ui, format!("Save As failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let ocr_controller = ocr_controller.clone();
        let ocr_model_manager = ocr_model_manager.clone();
        ui.on_ocr_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.get_ocr_running() || !ui.get_ocr_model_busy_id().is_empty() {
                return;
            }
            let Some(model_id) = ocr_model_manager.active_model_id() else {
                if let Some(overlay) = overlay_weak.upgrade() {
                    overlay.set_editor_visible(false);
                    let _ = overlay.hide();
                    set_overlay_windowed(&overlay);
                }
                // OCR setup must not destroy the current image or annotation history. The
                // editor can be reopened automatically after a model is enabled.
                clear_ocr_results(&ui);
                ui.set_settings_page("ocr".into());
                feedback::set_status_text(
                    &ui,
                    if ui.get_has_capture() {
                        "Choose an OCR model to download and enable · current capture retained"
                            .into()
                    } else {
                        "Choose an OCR model to download and enable".into()
                    },
                );
                let _ = ui.show();
                return;
            };
            let frame = latest_frame.borrow();
            let Some(frame) = frame.as_ref() else {
                feedback::set_status_text(&ui, "Nothing to OCR · capture a region first".into());
                return;
            };
            let image = match OcrImage::new(frame.width(), frame.height(), frame.rgba().to_vec()) {
                Ok(image) => image,
                Err(error) => {
                    feedback::set_status_text(&ui, format!("OCR input failed · {error}").into());
                    return;
                }
            };
            let model_name = OcrModelManager::descriptor(&model_id)
                .map(|model| model.name)
                .unwrap_or("Local OCR");
            if let Err(error) = ocr_controller.recognize(ui.get_ocr_epoch(), model_id, image) {
                feedback::set_status_text(&ui, format!("OCR worker unavailable · {error}").into());
                return;
            }
            clear_ocr_results(&ui);
            ui.set_ocr_running(true);
            feedback::set_status_text(&ui, format!("{model_name} running locally…").into());
        });
    }

    {
        let weak = ui.as_weak();
        let ocr_controller = ocr_controller.clone();
        ui.on_ocr_model_download_requested(move |model_id| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.get_ocr_running() || !ui.get_ocr_model_busy_id().is_empty() {
                return;
            }
            let model_id = model_id.to_string();
            let model_name = OcrModelManager::descriptor(&model_id)
                .map(|model| model.name)
                .unwrap_or("OCR model");
            if let Err(error) = ocr_controller.install_model(model_id.clone()) {
                feedback::set_status_text(
                    &ui,
                    format!("OCR model worker unavailable · {error}").into(),
                );
                return;
            }
            ui.set_ocr_model_busy_id(model_id.clone().into());
            ui.set_ocr_model_progress(0.0);
            ui.set_ocr_model_progress_label(format!("Starting {model_name} download…").into());
            ui.set_ocr_model_cancellable(true);
            ui.set_ocr_model_error_id("".into());
            ui.set_ocr_model_error_text("".into());
            feedback::set_status_text(&ui, format!("Downloading {model_name}…").into());
        });
    }

    {
        let weak = ui.as_weak();
        let ocr_controller = ocr_controller.clone();
        ui.on_ocr_model_cancel_requested(move |model_id| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if !ui.get_ocr_model_cancellable()
                || ui.get_ocr_model_busy_id().as_str() != model_id.as_str()
            {
                return;
            }
            if ocr_controller.cancel_model_download() {
                ui.set_ocr_model_cancellable(false);
                ui.set_ocr_model_progress_label("Cancelling download…".into());
                feedback::set_status_text(&ui, "Cancelling OCR model download…".into());
            }
        });
    }

    {
        let weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let ocr_model_manager = ocr_model_manager.clone();
        ui.on_ocr_model_enable_requested(move |model_id| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.get_ocr_running() || !ui.get_ocr_model_busy_id().is_empty() {
                return;
            }
            let model_id = model_id.to_string();
            let model_name = OcrModelManager::descriptor(&model_id)
                .map(|model| model.name)
                .unwrap_or("OCR model");
            match ocr_model_manager.set_active_model(&model_id) {
                Ok(()) => {
                    sync_ocr_model_ui(&ui, &ocr_model_manager);
                    feedback::set_status_text(
                        &ui,
                        format!("Enabled OCR model · {model_name}").into(),
                    );
                    if ui.get_has_capture()
                        && let Some(overlay) = overlay_weak.upgrade()
                    {
                        resume_editor_overlay(&ui, &overlay);
                    }
                }
                Err(error) => {
                    sync_ocr_model_ui(&ui, &ocr_model_manager);
                    feedback::set_status_text(
                        &ui,
                        format!("Could not enable OCR model · {error}").into(),
                    );
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let ocr_controller = ocr_controller.clone();
        ui.on_ocr_model_delete_requested(move |model_id| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.get_ocr_running() || !ui.get_ocr_model_busy_id().is_empty() {
                return;
            }
            let model_id = model_id.to_string();
            let model_name = OcrModelManager::descriptor(&model_id)
                .map(|model| model.name)
                .unwrap_or("OCR model");
            if let Err(error) = ocr_controller.remove_model(model_id.clone()) {
                feedback::set_status_text(
                    &ui,
                    format!("OCR model worker unavailable · {error}").into(),
                );
                return;
            }
            ui.set_ocr_model_busy_id(model_id.into());
            ui.set_ocr_model_progress(-1.0);
            ui.set_ocr_model_progress_label(format!("Removing {model_name}…").into());
            ui.set_ocr_model_cancellable(false);
            feedback::set_status_text(&ui, format!("Removing {model_name}…").into());
        });
    }

    {
        let weak = ui.as_weak();
        ui.on_copy_ocr_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let text = ui.get_ocr_text();
            if text.is_empty() {
                feedback::set_status_text(&ui, "No OCR text to copy".into());
                return;
            }
            match copy_text_to_clipboard(text.as_str()) {
                Ok(()) => feedback::set_status_text(&ui, "OCR text copied to the clipboard".into()),
                Err(error) => {
                    feedback::set_status_text(&ui, format!("OCR clipboard failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        ui.on_ocr_hit_test(move |x, y| {
            let Some(ui) = weak.upgrade() else {
                return -1;
            };
            ocr_hit_test(&ui.get_ocr_items(), x, y)
                .map(|index| index as i32)
                .unwrap_or(-1)
        });
    }

    {
        let weak = ui.as_weak();
        ui.on_copy_ocr_selection_requested(move |anchor, focus| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let text = ocr_selection_text(&ui.get_ocr_items(), anchor, focus);
            if text.is_empty() {
                feedback::set_status_text(&ui, "No OCR text is selected".into());
                return;
            }
            match copy_text_to_clipboard(&text) {
                Ok(()) => feedback::set_status_text(
                    &ui,
                    "Selected OCR text copied to the clipboard".into(),
                ),
                Err(error) => {
                    feedback::set_status_text(&ui, format!("OCR clipboard failed · {error}").into())
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_ocr_selection_to_text_requested(move |anchor, focus| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let blocks = ocr_selection_blocks(&ui.get_ocr_items(), anchor, focus);
            if blocks.is_empty() {
                feedback::set_status_text(&ui, "No OCR text is selected".into());
                return;
            }
            let result = editor.borrow_mut().add_text_annotations_from_ocr(&blocks);
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    feedback::set_status_text(
                        &ui,
                        "Selected OCR text converted to editable annotations · undo is available"
                            .into(),
                    );
                }
                Ok(None) => feedback::set_status_text(&ui, "Selected OCR text is empty".into()),
                Err(error) => feedback::set_status_text(
                    &ui,
                    format!("OCR-to-text conversion failed · {error}").into(),
                ),
            }
        });
    }

    {
        let start_capture = Rc::clone(&start_capture);
        tray.on_quick_capture(move || start_capture(CaptureOrigin::Background));
    }

    {
        let weak = ui.as_weak();
        tray.on_show_main(move || {
            if let Some(ui) = weak.upgrade() {
                let _ = ui.show();
            }
        });
    }

    tray.on_quit(|| {
        let _ = slint::quit_event_loop();
    });

    let hotkey_controller = Rc::new(HotkeyController::new(
        &ui,
        settings_store.clone(),
        Rc::clone(&app_settings),
    ));

    ui.show()?;
    tray.show()?;

    let hotkey_timer = Timer::default();
    {
        let hotkey_controller = Rc::clone(&hotkey_controller);
        let start_capture = Rc::clone(&start_capture);
        hotkey_timer.start(TimerMode::Repeated, Duration::from_millis(40), move || {
            if hotkey_controller.take_pressed() {
                start_capture(CaptureOrigin::Background);
            }
        });
    }

    slint::run_event_loop()
}

fn handle_ocr_event(ui: &AppWindow, manager: &OcrModelManager, event: OcrEvent) {
    match event {
        OcrEvent::Recognition { epoch, result } => {
            if epoch != ui.get_ocr_epoch() {
                return;
            }
            ui.set_ocr_running(false);
            match result {
                Ok(result) => {
                    let line_count = result.blocks.len();
                    let items = result
                        .blocks
                        .into_iter()
                        .map(|block| {
                            let bounds = block.bounds;
                            let quad = block
                                .polygon
                                .map(|polygon| {
                                    [
                                        (polygon[0].x, polygon[0].y),
                                        (polygon[1].x, polygon[1].y),
                                        (polygon[2].x, polygon[2].y),
                                        (polygon[3].x, polygon[3].y),
                                    ]
                                })
                                .unwrap_or([
                                    (bounds.x, bounds.y),
                                    (bounds.x + bounds.width, bounds.y),
                                    (bounds.x + bounds.width, bounds.y + bounds.height),
                                    (bounds.x, bounds.y + bounds.height),
                                ]);
                            OcrOverlayItem {
                                x: bounds.x,
                                y: bounds.y,
                                width: bounds.width,
                                height: bounds.height,
                                confidence: block.confidence,
                                text: block.text.into(),
                                p0_x: quad[0].0,
                                p0_y: quad[0].1,
                                p1_x: quad[1].0,
                                p1_y: quad[1].1,
                                p2_x: quad[2].0,
                                p2_y: quad[2].1,
                                p3_x: quad[3].0,
                                p3_y: quad[3].1,
                            }
                        })
                        .collect::<Vec<_>>();
                    ui.set_ocr_items(ModelRc::new(VecModel::from(items)));
                    ui.set_ocr_text(result.plain_text.into());
                    ui.set_ocr_line_count(line_count as i32);
                    ui.set_ocr_overlay_visible(line_count > 0);
                    if line_count == 0 {
                        feedback::set_status_text(ui, "Local OCR completed · no text found".into());
                    } else {
                        feedback::set_status_text(
                            ui,
                            format!(
                                "Local OCR completed · {line_count} text blocks · OCR text layer is not exported"
                            )
                            .into(),
                        );
                    }
                }
                Err(error) => {
                    clear_ocr_results(ui);
                    feedback::set_status_text(ui, format!("Local OCR failed · {error}").into());
                }
            }
        }
        OcrEvent::ModelProgress {
            model_id,
            fraction,
            detail,
        } => {
            if ui.get_ocr_model_busy_id().as_str() != model_id {
                return;
            }
            ui.set_ocr_model_progress(fraction);
            let label = if fraction >= 0.0 {
                format!(
                    "{detail} · {:.0}%",
                    (fraction as f64 * 100.0).clamp(0.0, 100.0)
                )
            } else {
                detail
            };
            ui.set_ocr_model_progress_label(label.into());
        }
        OcrEvent::ModelAction {
            model_id,
            action,
            result,
        } => {
            ui.set_ocr_model_busy_id("".into());
            ui.set_ocr_model_progress(-1.0);
            ui.set_ocr_model_progress_label("".into());
            ui.set_ocr_model_cancellable(false);
            sync_ocr_model_ui(ui, manager);
            let model_name = OcrModelManager::descriptor(&model_id)
                .map(|model| model.name)
                .unwrap_or("OCR model");
            match (action, result) {
                (OcrModelAction::Install, OcrActionResult::Succeeded) => {
                    ui.set_ocr_model_error_id("".into());
                    ui.set_ocr_model_error_text("".into());
                    feedback::set_status_text(
                        ui,
                        format!("Downloaded {model_name} · select Enable to use it for OCR").into(),
                    );
                }
                (OcrModelAction::Remove, OcrActionResult::Succeeded) => {
                    feedback::set_status_text(
                        ui,
                        format!("Removed OCR model · {model_name}").into(),
                    );
                }
                (OcrModelAction::Install, OcrActionResult::Cancelled(_)) => {
                    feedback::set_status_text(
                        ui,
                        format!("Download cancelled · {model_name} remains disabled").into(),
                    );
                }
                (OcrModelAction::Install, OcrActionResult::Failed(error)) => {
                    ui.set_ocr_model_error_id(model_id.into());
                    ui.set_ocr_model_error_text(error.clone().into());
                    feedback::set_status_text(
                        ui,
                        format!("OCR model download failed · {error}").into(),
                    );
                }
                (OcrModelAction::Remove, OcrActionResult::Failed(error)) => {
                    feedback::set_status_text(
                        ui,
                        format!("OCR model removal failed · {error}").into(),
                    );
                }
                (OcrModelAction::Remove, OcrActionResult::Cancelled(message)) => {
                    feedback::set_status_text(
                        ui,
                        format!("OCR model removal cancelled · {message}").into(),
                    );
                }
            }
        }
    }
}

fn sync_ocr_model_ui(ui: &AppWindow, manager: &OcrModelManager) {
    let active_model_id = manager.active_model_id();
    let items = manager
        .states()
        .into_iter()
        .map(|state| OcrModelItem {
            id: state.descriptor.id.into(),
            name: state.descriptor.name.into(),
            version: state.descriptor.version.into(),
            languages: state.descriptor.languages.into(),
            size_label: format_model_size(state.descriptor.download_size_bytes).into(),
            installed: state.installed,
            active: state.active,
        })
        .collect::<Vec<_>>();
    ui.set_ocr_models(ModelRc::new(VecModel::from(items)));
    ui.set_ocr_engine_enabled(active_model_id.is_some());
    let engine_name = active_model_id
        .as_deref()
        .and_then(OcrModelManager::descriptor)
        .map(|model| model.name)
        .unwrap_or("No model enabled");
    ui.set_ocr_engine_name(engine_name.into());
}

fn sync_export_settings_ui(ui: &AppWindow, settings: &AppSettings) {
    ui.set_language_mode(settings.language.as_str().into());
    let export = &settings.export;
    ui.set_export_default_directory(
        export
            .default_directory
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_default()
            .into(),
    );
    ui.set_export_remember_last_directory(export.remember_last_directory);
    ui.set_export_copy_after_capture(export.copy_after_capture);
    ui.set_export_close_after_copy(export.close_after_copy);
    ui.set_export_close_after_save(export.close_after_save);
}

fn format_model_size(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * MIB;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes / GIB)
    } else {
        format!("{:.1} MiB", bytes / MIB)
    }
}

fn clear_ocr_results(ui: &AppWindow) {
    ui.set_ocr_selection_anchor(-1);
    ui.set_ocr_selection_focus(-1);
    ui.set_ocr_hover_index(-1);
    ui.set_ocr_overlay_visible(false);
    ui.set_ocr_line_count(0);
    ui.set_ocr_text("".into());
    ui.set_ocr_items(ModelRc::new(VecModel::from(Vec::<OcrOverlayItem>::new())));
}

fn ocr_selection_range(anchor: i32, focus: i32, row_count: usize) -> Option<(usize, usize)> {
    if anchor < 0 || focus < 0 || row_count == 0 {
        return None;
    }
    let start = anchor.min(focus) as usize;
    let end = anchor.max(focus) as usize;
    (end < row_count).then_some((start, end))
}

fn ocr_selection_text(model: &ModelRc<OcrOverlayItem>, anchor: i32, focus: i32) -> String {
    let Some((start, end)) = ocr_selection_range(anchor, focus, model.row_count()) else {
        return String::new();
    };
    (start..=end)
        .filter_map(|index| model.row_data(index))
        .map(|item| item.text.to_string())
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn ocr_selection_blocks(
    model: &ModelRc<OcrOverlayItem>,
    anchor: i32,
    focus: i32,
) -> Vec<(f32, f32, f32, String)> {
    let Some((start, end)) = ocr_selection_range(anchor, focus, model.row_count()) else {
        return Vec::new();
    };
    (start..=end)
        .filter_map(|index| model.row_data(index))
        .filter_map(|item| {
            let text = item.text.trim();
            (!text.is_empty())
                .then(|| (item.x, item.y, ocr_text_line_height(&item), text.to_owned()))
        })
        .collect()
}

fn ocr_text_line_height(item: &OcrOverlayItem) -> f32 {
    let points = [
        (item.p0_x, item.p0_y),
        (item.p1_x, item.p1_y),
        (item.p2_x, item.p2_y),
        (item.p3_x, item.p3_y),
    ];
    let mut edge_lengths = [0.0_f32; 4];
    for index in 0..4 {
        let (ax, ay) = points[index];
        let (bx, by) = points[(index + 1) % 4];
        edge_lengths[index] = (bx - ax).hypot(by - ay);
    }
    edge_lengths.sort_by(f32::total_cmp);
    let polygon_thickness = (edge_lengths[0] + edge_lengths[1]) * 0.5;
    if polygon_thickness.is_finite() && polygon_thickness >= 1.0 {
        polygon_thickness
    } else {
        item.height.max(1.0)
    }
}

fn ocr_hit_test(model: &ModelRc<OcrOverlayItem>, x: f32, y: f32) -> Option<usize> {
    (0..model.row_count()).rev().find(|&index| {
        model
            .row_data(index)
            .is_some_and(|item| point_in_ocr_quad(&item, x, y))
    })
}

fn point_in_ocr_quad(item: &OcrOverlayItem, x: f32, y: f32) -> bool {
    let points = [
        (item.p0_x, item.p0_y),
        (item.p1_x, item.p1_y),
        (item.p2_x, item.p2_y),
        (item.p3_x, item.p3_y),
    ];
    let mut positive = false;
    let mut negative = false;
    for index in 0..4 {
        let (ax, ay) = points[index];
        let (bx, by) = points[(index + 1) % 4];
        let cross = (bx - ax) * (y - ay) - (by - ay) * (x - ax);
        if cross > 0.001 {
            positive = true;
        } else if cross < -0.001 {
            negative = true;
        }
        if positive && negative {
            return false;
        }
    }
    true
}

fn invalidate_ocr_results(ui: &AppWindow) {
    ui.set_ocr_epoch(ui.get_ocr_epoch().wrapping_add(1));
    ui.set_ocr_running(false);
    clear_ocr_results(ui);
}

fn frame_to_image(frame: &CapturedFrame) -> Image {
    let pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        frame.rgba(),
        frame.width(),
        frame.height(),
    );
    Image::from_rgba8(pixel_buffer)
}

fn sync_editor_overlay(ui: &AppWindow, overlay: &RegionOverlay) {
    overlay.set_status_text(ui.get_status_text());
    overlay.set_status_level(ui.get_status_level());
    overlay.set_status_duration(ui.get_status_duration());
    overlay.set_preview_image(ui.get_preview_image());
    overlay.set_has_capture(ui.get_has_capture());
    overlay.set_can_undo(ui.get_can_undo());
    overlay.set_can_redo(ui.get_can_redo());
    overlay.set_text_entry_visible(ui.get_text_entry_visible());
    overlay.set_pending_text(ui.get_pending_text());
    overlay.set_active_tool(ui.get_active_tool());
    overlay.set_active_color(ui.get_active_color());
    overlay.set_active_stroke(ui.get_active_stroke());
    overlay.set_capture_width(ui.get_capture_width());
    overlay.set_capture_height(ui.get_capture_height());
    overlay.set_zoom_factor(ui.get_zoom_factor());
    overlay.set_pan_x(ui.get_pan_x());
    overlay.set_pan_y(ui.get_pan_y());
    overlay.set_editor_has_selection(ui.get_has_selection());
    overlay.set_object_selection_x(ui.get_object_selection_x());
    overlay.set_object_selection_y(ui.get_object_selection_y());
    overlay.set_object_selection_width(ui.get_object_selection_width());
    overlay.set_object_selection_height(ui.get_object_selection_height());
    overlay.set_ocr_items(ui.get_ocr_items());
    overlay.set_ocr_line_count(ui.get_ocr_line_count());
    overlay.set_ocr_overlay_visible(ui.get_ocr_overlay_visible());
    overlay.set_ocr_running(ui.get_ocr_running());
    overlay.set_ocr_selection_anchor(ui.get_ocr_selection_anchor());
    overlay.set_ocr_selection_focus(ui.get_ocr_selection_focus());
    overlay.set_ocr_hover_index(ui.get_ocr_hover_index());
    overlay.set_ocr_model_busy_id(ui.get_ocr_model_busy_id());
}

fn resume_editor_overlay(ui: &AppWindow, overlay: &RegionOverlay) {
    sync_editor_overlay(ui, overlay);
    set_overlay_windowed(overlay);
    overlay.set_editor_visible(true);
    let _ = ui.hide();
    if let Err(error) = overlay.show() {
        overlay.set_editor_visible(false);
        feedback::set_status_text(
            ui,
            format!("Could not reopen capture editor · {error}").into(),
        );
        let _ = ui.show();
        return;
    }
    focus_overlay(overlay);
}

fn set_overlay_windowed(overlay: &RegionOverlay) {
    let _ = overlay
        .window()
        .with_winit_window(|window| window.set_fullscreen(None));
    overlay.window().set_fullscreen(false);
}

fn set_overlay_fullscreen_on_monitor(
    overlay: &RegionOverlay,
    cursor_x: i32,
    cursor_y: i32,
) -> bool {
    let selected = overlay
        .window()
        .with_winit_window(|window| {
            let monitor = window.available_monitors().find(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                let x = i64::from(cursor_x);
                let y = i64::from(cursor_y);
                let left = i64::from(position.x);
                let top = i64::from(position.y);
                let right = left + i64::from(size.width);
                let bottom = top + i64::from(size.height);
                x >= left && x < right && y >= top && y < bottom
            });
            let Some(monitor) = monitor else {
                return false;
            };

            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(monitor))));
            true
        })
        .unwrap_or(false);

    if selected {
        overlay.window().set_fullscreen(true);
    }
    selected
}

fn focus_overlay(overlay: &RegionOverlay) {
    let _ = overlay
        .window()
        .with_winit_window(|window| window.focus_window());
}

fn schedule_capture_exit(ui: &AppWindow, overlay: &RegionOverlay) {
    let ui_weak = ui.as_weak();
    let overlay_weak = overlay.as_weak();
    let capture_sequence = overlay.get_capture_sequence();
    Timer::single_shot(Duration::from_millis(700), move || {
        if let Some(overlay) = overlay_weak.upgrade()
            && overlay.get_editor_visible()
            && overlay.get_capture_sequence() == capture_sequence
        {
            overlay.set_editor_visible(false);
            let _ = overlay.hide();
            set_overlay_windowed(&overlay);
            if let Some(ui) = ui_weak.upgrade() {
                let _ = ui.hide();
            }
        }
    });
}

fn finish_capture(
    ui: &AppWindow,
    latest_frame: &Rc<RefCell<Option<CapturedFrame>>>,
    editor: &Rc<RefCell<EditorSession>>,
    settings: &AppSettings,
    frame: CapturedFrame,
) {
    editor.borrow_mut().reset(frame.clone());
    ui.set_zoom_factor(1.0);
    ui.set_pan_x(0.0);
    ui.set_pan_y(0.0);
    set_editor_frame(ui, latest_frame, frame.clone());
    sync_history(ui, &editor.borrow());
    sync_selection(ui, &editor.borrow());
    ui.set_text_entry_visible(false);

    let dimensions = format!("{}×{}", frame.width(), frame.height());
    if !settings.export.copy_after_capture {
        feedback::set_status_text(
            ui,
            format!("Captured {dimensions} · ready to annotate").into(),
        );
        return;
    }

    match copy_to_clipboard(&frame) {
        Ok(()) => feedback::set_status_text(
            ui,
            format!("Captured {dimensions} · copied to clipboard · ready to annotate").into(),
        ),
        Err(error) => feedback::set_status_text(
            ui,
            format!("Captured {dimensions} · clipboard failed: {error} · ready to annotate").into(),
        ),
    }
}

fn set_editor_frame(
    ui: &AppWindow,
    latest_frame: &Rc<RefCell<Option<CapturedFrame>>>,
    frame: CapturedFrame,
) {
    invalidate_ocr_results(ui);
    ui.set_capture_width(frame.width() as f32);
    ui.set_capture_height(frame.height() as f32);
    ui.set_preview_image(frame_to_image(&frame));
    ui.set_has_capture(true);
    *latest_frame.borrow_mut() = Some(frame);
}

fn sync_history(ui: &AppWindow, editor: &EditorSession) {
    ui.set_can_undo(editor.can_undo());
    ui.set_can_redo(editor.can_redo());
}

fn sync_selection(ui: &AppWindow, editor: &EditorSession) {
    let Some(bounds) = editor.selection_bounds() else {
        ui.set_has_selection(false);
        return;
    };

    ui.set_has_selection(true);
    ui.set_object_selection_x(bounds.x);
    ui.set_object_selection_y(bounds.y);
    ui.set_object_selection_width(bounds.width);
    ui.set_object_selection_height(bounds.height);
    if let Some(color) = editor.selected_color_index() {
        ui.set_active_color(color);
    }
    if let Some(stroke) = editor.selected_stroke_width() {
        ui.set_active_stroke(stroke);
    }
}

fn selection_to_capture_rect(
    overlay: &RegionOverlay,
    frame: &CapturedFrame,
    selection_x: f32,
    selection_y: f32,
    selection_width: f32,
    selection_height: f32,
) -> CaptureRect {
    let physical_size = overlay.window().size();
    let scale_factor = overlay.window().scale_factor().max(f32::EPSILON);
    let logical_width = (physical_size.width as f32 / scale_factor).max(1.0);
    let logical_height = (physical_size.height as f32 / scale_factor).max(1.0);
    let scale_x = frame.width() as f32 / logical_width;
    let scale_y = frame.height() as f32 / logical_height;

    let left = (selection_x.max(0.0) * scale_x).floor() as u32;
    let top = (selection_y.max(0.0) * scale_y).floor() as u32;
    let right = ((selection_x + selection_width).max(0.0) * scale_x).ceil() as u32;
    let bottom = ((selection_y + selection_height).max(0.0) * scale_y).ceil() as u32;

    CaptureRect::new(
        left,
        top,
        right.saturating_sub(left),
        bottom.saturating_sub(top),
    )
}

fn make_app_icon() -> Image {
    // Keep the runtime raster aligned with packaging/assets/com.azusalens.AzusaLens.svg.
    const SIZE: usize = 64;
    const BLUE: Rgba8Pixel = Rgba8Pixel {
        r: 37,
        g: 99,
        b: 235,
        a: 255,
    };
    const WHITE: Rgba8Pixel = Rgba8Pixel {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(SIZE as u32, SIZE as u32);
    let pixels = buffer.make_mut_slice();
    pixels.fill(Rgba8Pixel {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    });

    for y in 0..SIZE {
        for x in 0..SIZE {
            let coverage = rounded_rect_coverage(x as f32 + 0.5, y as f32 + 0.5, SIZE as f32, 16.0);
            blend_icon_pixel(&mut pixels[y * SIZE + x], BLUE, coverage);
        }
    }

    let az_stroke = [
        (14.0, 46.0),
        (26.0, 16.0),
        (38.0, 46.0),
        (43.0, 24.0),
        (55.0, 24.0),
        (39.0, 46.0),
        (55.0, 46.0),
    ];
    draw_icon_stroke(pixels, SIZE, &az_stroke, 5.0, WHITE);
    draw_icon_stroke(pixels, SIZE, &[(19.0, 35.0), (33.0, 35.0)], 5.0, WHITE);

    Image::from_rgba8(buffer)
}

fn rounded_rect_coverage(x: f32, y: f32, size: f32, radius: f32) -> f32 {
    let corner_x = x.clamp(radius, size - radius);
    let corner_y = y.clamp(radius, size - radius);
    (radius + 0.75 - (x - corner_x).hypot(y - corner_y)).clamp(0.0, 1.0)
}

fn blend_icon_pixel(pixel: &mut Rgba8Pixel, color: Rgba8Pixel, coverage: f32) {
    let alpha = (color.a as f32 / 255.0 * coverage.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let inverse = 1.0 - alpha;
    pixel.r = (color.r as f32 * alpha + pixel.r as f32 * inverse).round() as u8;
    pixel.g = (color.g as f32 * alpha + pixel.g as f32 * inverse).round() as u8;
    pixel.b = (color.b as f32 * alpha + pixel.b as f32 * inverse).round() as u8;
    pixel.a = (255.0 * (alpha + pixel.a as f32 / 255.0 * inverse)).round() as u8;
}

fn draw_icon_stroke(
    pixels: &mut [Rgba8Pixel],
    size: usize,
    points: &[(f32, f32)],
    width: f32,
    color: Rgba8Pixel,
) {
    for y in 0..size {
        for x in 0..size {
            let point = (x as f32 + 0.5, y as f32 + 0.5);
            let distance = points
                .windows(2)
                .map(|segment| point_to_segment_distance(point, segment[0], segment[1]))
                .fold(f32::MAX, f32::min);
            let coverage = (width / 2.0 + 0.8 - distance).clamp(0.0, 1.0);
            blend_icon_pixel(&mut pixels[y * size + x], color, coverage);
        }
    }
}

fn point_to_segment_distance(point: (f32, f32), start: (f32, f32), end: (f32, f32)) -> f32 {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let length_squared = dx * dx + dy * dy;
    let projection = if length_squared > 0.0 {
        ((point.0 - start.0) * dx + (point.1 - start.1) * dy) / length_squared
    } else {
        0.0
    };
    let t = projection.clamp(0.0, 1.0);
    (point.0 - (start.0 + t * dx)).hypot(point.1 - (start.1 + t * dy))
}

fn copy_to_clipboard(frame: &CapturedFrame) -> Result<(), String> {
    CLIPBOARD.with(|clipboard| {
        let mut clipboard = clipboard.borrow_mut();
        if clipboard.is_none() {
            *clipboard = Some(Clipboard::new().map_err(|error| error.to_string())?);
        }

        let image = ImageData {
            width: frame.width() as usize,
            height: frame.height() as usize,
            bytes: Cow::Borrowed(frame.rgba()),
        };

        clipboard
            .as_mut()
            .expect("clipboard was initialized above")
            .set_image(image)
            .map_err(|error| error.to_string())
    })
}

fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    CLIPBOARD.with(|clipboard| {
        let mut clipboard = clipboard.borrow_mut();
        if clipboard.is_none() {
            *clipboard = Some(Clipboard::new().map_err(|error| error.to_string())?);
        }
        clipboard
            .as_mut()
            .expect("clipboard was initialized above")
            .set_text(text.to_owned())
            .map_err(|error| error.to_string())
    })
}
