mod editor;

use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::mpsc::{self, TryRecvError},
    thread,
    time::Duration,
};

use arboard::{Clipboard, ImageData};
use azusa_capture::{
    CaptureRect, CapturedFrame, RegionCapture, begin_region_capture, detected_backend,
};
use azusa_hotkey::{PrintScreenHotkey, backend_description as hotkey_backend_description};
use azusa_ocr::{FastOcrEngine, OcrEngine, OcrImage, OcrResult, default_engine_name};
use editor::{BeginResult, EditorSession};
use slint::{
    ComponentHandle, Image, ModelRc, PhysicalPosition, Rgba8Pixel, SharedPixelBuffer, Timer,
    TimerMode, VecModel,
};

slint::include_modules!();

thread_local! {
    static CLIPBOARD: RefCell<Option<Clipboard>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Copy)]
enum CaptureOrigin {
    MainWindow,
    Background,
}

struct OcrJob {
    epoch: i32,
    image: OcrImage,
}

struct OcrWorkerMessage {
    epoch: i32,
    result: Result<OcrResult, String>,
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let overlay = RegionOverlay::new()?;
    let tray = AppTray::new()?;

    let latest_frame = Rc::new(RefCell::new(None::<CapturedFrame>));
    let pending_frame = Rc::new(RefCell::new(None::<CapturedFrame>));
    let editor = Rc::new(RefCell::new(EditorSession::default()));
    let capture_origin = Rc::new(Cell::new(CaptureOrigin::MainWindow));
    let capture_active = Rc::new(Cell::new(false));

    let (ocr_job_tx, ocr_job_rx) = mpsc::channel::<OcrJob>();
    let (ocr_result_tx, ocr_result_rx) = mpsc::channel::<OcrWorkerMessage>();
    thread::Builder::new()
        .name("azusa-fast-ocr".to_owned())
        .spawn(move || {
            let mut engine = FastOcrEngine::new();
            while let Ok(job) = ocr_job_rx.recv() {
                let result = engine
                    .recognize(&job.image)
                    .map_err(|error| error.to_string());
                if ocr_result_tx
                    .send(OcrWorkerMessage {
                        epoch: job.epoch,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .expect("failed to start local OCR worker");

    ui.set_platform_name(detected_backend().to_string().into());
    ui.set_hotkey_name(hotkey_backend_description().into());
    ui.set_ocr_engine_name(default_engine_name().into());
    ui.set_status_text(
        "Ready · capture a region, then annotate it with the editor tools below".into(),
    );
    tray.set_app_icon(make_tray_icon());

    let start_capture: Rc<dyn Fn(CaptureOrigin)> = {
        let ui_weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let pending_frame = Rc::clone(&pending_frame);
        let editor = Rc::clone(&editor);
        let capture_origin = Rc::clone(&capture_origin);
        let capture_active = Rc::clone(&capture_active);

        Rc::new(move |origin| {
            if capture_active.replace(true) {
                return;
            }

            let Some(ui) = ui_weak.upgrade() else {
                capture_active.set(false);
                return;
            };
            let Some(overlay) = overlay_weak.upgrade() else {
                capture_active.set(false);
                return;
            };

            capture_origin.set(origin);
            ui.set_status_text("Starting region capture…".into());
            let _ = ui.hide();

            match begin_region_capture() {
                Ok(RegionCapture::Selected(frame)) => {
                    finish_capture(&ui, &latest_frame, &editor, frame);
                    capture_active.set(false);
                }
                Ok(RegionCapture::NeedsSelection(selection)) => {
                    let (anchor_x, anchor_y) = selection.anchor();
                    let frame = selection.into_frame();
                    overlay.set_screenshot(frame_to_image(&frame));
                    *pending_frame.borrow_mut() = Some(frame);

                    overlay.window().set_fullscreen(false);
                    overlay
                        .window()
                        .set_position(PhysicalPosition::new(anchor_x, anchor_y));
                    overlay.window().set_fullscreen(true);

                    if let Err(error) = overlay.show() {
                        *pending_frame.borrow_mut() = None;
                        capture_active.set(false);
                        ui.set_status_text(format!("Selection overlay failed · {error}").into());
                        if matches!(origin, CaptureOrigin::MainWindow) {
                            let _ = ui.show();
                        }
                    }
                }
                Err(error) => {
                    capture_active.set(false);
                    ui.set_status_text(format!("Region capture failed · {error}").into());
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
        let pending_frame = Rc::clone(&pending_frame);
        let editor = Rc::clone(&editor);
        let capture_origin = Rc::clone(&capture_origin);
        let capture_active = Rc::clone(&capture_active);

        overlay.on_selection_confirmed(
            move |selection_x, selection_y, selection_width, selection_height| {
                let Some(ui) = ui_weak.upgrade() else {
                    return;
                };
                let Some(overlay) = overlay_weak.upgrade() else {
                    return;
                };

                let _ = overlay.hide();
                overlay.window().set_fullscreen(false);
                let Some(frame) = pending_frame.borrow_mut().take() else {
                    capture_active.set(false);
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
                let origin = capture_origin.get();

                match frame.crop(rect) {
                    Ok(frame) => finish_capture(&ui, &latest_frame, &editor, frame),
                    Err(error) => {
                        ui.set_status_text(format!("Region crop failed · {error}").into());
                        if matches!(origin, CaptureOrigin::MainWindow) {
                            let _ = ui.show();
                        }
                    }
                }
                capture_active.set(false);
            },
        );
    }

    {
        let ui_weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let pending_frame = Rc::clone(&pending_frame);
        let capture_origin = Rc::clone(&capture_origin);
        let capture_active = Rc::clone(&capture_active);

        overlay.on_cancelled(move || {
            if let Some(overlay) = overlay_weak.upgrade() {
                let _ = overlay.hide();
                overlay.window().set_fullscreen(false);
            }
            *pending_frame.borrow_mut() = None;
            capture_active.set(false);

            if let Some(ui) = ui_weak.upgrade() {
                ui.set_status_text("Region capture cancelled".into());
                if matches!(capture_origin.get(), CaptureOrigin::MainWindow) {
                    let _ = ui.show();
                }
            }
        });
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
                    ui.set_status_text(
                        "Select tool · click an object, then drag it or use a resize handle".into(),
                    );
                } else {
                    ui.set_status_text(format!("Annotation tool · {tool}").into());
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
                    ui.set_status_text("Updated selected object color".into());
                }
                Ok(None) => {}
                Err(error) => ui.set_status_text(format!("Color update failed · {error}").into()),
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
                    ui.set_status_text("Updated selected object size".into());
                }
                Ok(None) => {}
                Err(error) => ui.set_status_text(format!("Size update failed · {error}").into()),
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
            match editor.borrow_mut().begin_canvas(x, y, width, height) {
                BeginResult::TextInput => {
                    ui.set_pending_text("".into());
                    ui.set_text_entry_visible(true);
                    ui.set_status_text("Text anchor placed · type text and choose Add text".into());
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
                Err(error) => {
                    ui.set_status_text(format!("Annotation preview failed · {error}").into())
                }
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
            let was_select = editor.borrow().is_select_tool();
            let result = editor.borrow_mut().end_canvas(x, y, width, height);
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    if was_select {
                        ui.set_status_text(
                            "Selection updated · drag to move, resize with handles, or edit properties"
                                .into(),
                        );
                    } else {
                        ui.set_status_text(
                            "Annotation added · continue editing or export the image".into(),
                        );
                    }
                }
                Ok(None) => sync_selection(&ui, &editor.borrow()),
                Err(error) => ui.set_status_text(format!("Annotation failed · {error}").into()),
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
            let result = editor.borrow_mut().commit_text(value.as_str());
            match result {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                    ui.set_text_entry_visible(false);
                    ui.set_pending_text("".into());
                    ui.set_status_text("Text annotation added".into());
                }
                Ok(None) => ui.set_text_entry_visible(false),
                Err(error) => {
                    ui.set_status_text(format!("Text annotation failed · {error}").into())
                }
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
                    ui.set_status_text("Undid last editor change".into());
                }
                Ok(None) => {}
                Err(error) => ui.set_status_text(format!("Undo failed · {error}").into()),
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
                    ui.set_status_text("Redid editor change".into());
                }
                Ok(None) => {}
                Err(error) => ui.set_status_text(format!("Redo failed · {error}").into()),
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
                    ui.set_status_text("Deleted selected annotation".into());
                }
                Ok(None) => {}
                Err(error) => ui.set_status_text(format!("Delete failed · {error}").into()),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_cancel_editor_action_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = editor.borrow_mut().cancel_action();
            ui.set_text_entry_visible(false);
            ui.set_pending_text("".into());
            match result {
                Ok(Some(frame)) => set_editor_frame(&ui, &latest_frame, frame),
                Ok(None) => {}
                Err(error) => ui.set_status_text(format!("Cancel failed · {error}").into()),
            }
            sync_history(&ui, &editor.borrow());
            sync_selection(&ui, &editor.borrow());
            ui.set_status_text("Editor action cancelled".into());
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
                    ui.set_status_text("All annotations cleared".into());
                }
                Ok(None) => {}
                Err(error) => ui.set_status_text(format!("Clear failed · {error}").into()),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        ui.on_copy_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let frame = latest_frame.borrow();
            let Some(frame) = frame.as_ref() else {
                ui.set_status_text("Nothing to copy · capture a region first".into());
                return;
            };

            match copy_to_clipboard(frame) {
                Ok(()) => ui.set_status_text("Edited image copied to the clipboard".into()),
                Err(error) => ui.set_status_text(format!("Clipboard failed · {error}").into()),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        ui.on_save_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let frame = latest_frame.borrow();
            let Some(frame) = frame.as_ref() else {
                ui.set_status_text("Nothing to save · capture a region first".into());
                return;
            };

            let path = default_capture_path();
            match frame.save_png(&path) {
                Ok(()) => {
                    ui.set_status_text(format!("Saved edited PNG · {}", path.display()).into())
                }
                Err(error) => ui.set_status_text(format!("Save failed · {error}").into()),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let ocr_job_tx = ocr_job_tx.clone();
        ui.on_ocr_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.get_ocr_running() {
                return;
            }
            let frame = latest_frame.borrow();
            let Some(frame) = frame.as_ref() else {
                ui.set_status_text("Nothing to OCR · capture a region first".into());
                return;
            };
            let image = match OcrImage::new(frame.width(), frame.height(), frame.rgba().to_vec()) {
                Ok(image) => image,
                Err(error) => {
                    ui.set_status_text(format!("OCR input failed · {error}").into());
                    return;
                }
            };
            if let Err(error) = ocr_job_tx.send(OcrJob {
                epoch: ui.get_ocr_epoch(),
                image,
            }) {
                ui.set_status_text(format!("OCR worker unavailable · {error}").into());
                return;
            }
            ui.set_ocr_running(true);
            ui.set_status_text(
                "Fast local OCR running · first use may download about 16 MiB of PP-OCRv6 models"
                    .into(),
            );
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
                ui.set_status_text("No OCR text to copy".into());
                return;
            }
            match copy_text_to_clipboard(text.as_str()) {
                Ok(()) => ui.set_status_text("OCR text copied to the clipboard".into()),
                Err(error) => ui.set_status_text(format!("OCR clipboard failed · {error}").into()),
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

    ui.show()?;
    tray.show()?;

    let hotkey = match PrintScreenHotkey::register() {
        Ok(hotkey) => {
            ui.set_hotkey_name(format!("{} · active", hotkey_backend_description()).into());
            Some(Rc::new(hotkey))
        }
        Err(error) => {
            ui.set_hotkey_name(format!("PrtSc unavailable · {error}").into());
            ui.set_status_text(
                format!("PrtSc registration failed · {error} · tray capture remains available")
                    .into(),
            );
            None
        }
    };

    let hotkey_timer = Timer::default();
    if let Some(hotkey) = hotkey {
        let start_capture = Rc::clone(&start_capture);
        hotkey_timer.start(TimerMode::Repeated, Duration::from_millis(40), move || {
            if hotkey.take_pressed() {
                start_capture(CaptureOrigin::Background);
            }
        });
    }

    let ocr_result_timer = Timer::default();
    {
        let weak = ui.as_weak();
        ocr_result_timer.start(TimerMode::Repeated, Duration::from_millis(40), move || loop {
            let message = match ocr_result_rx.try_recv() {
                Ok(message) => message,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            };
            let Some(ui) = weak.upgrade() else { break; };
            if message.epoch != ui.get_ocr_epoch() { continue; }
            ui.set_ocr_running(false);
            match message.result {
                Ok(result) => {
                    let line_count = result.blocks.len();
                    let items = result.blocks.into_iter().map(|block| OcrOverlayItem {
                        x: block.bounds.x,
                        y: block.bounds.y,
                        width: block.bounds.width,
                        height: block.bounds.height,
                        confidence: block.confidence,
                        text: block.text.into(),
                    }).collect::<Vec<_>>();
                    ui.set_ocr_items(ModelRc::new(VecModel::from(items)));
                    ui.set_ocr_text(result.plain_text.into());
                    ui.set_ocr_line_count(line_count as i32);
                    ui.set_ocr_overlay_visible(line_count > 0);
                    if line_count == 0 {
                        ui.set_status_text("Fast local OCR completed · no text found".into());
                    } else {
                        ui.set_status_text(format!("Fast local OCR completed · {line_count} text blocks · OCR boxes are not exported").into());
                    }
                }
                Err(error) => {
                    ui.set_ocr_overlay_visible(false);
                    ui.set_status_text(format!("Fast local OCR failed · {error}").into());
                }
            }
        });
    }

    slint::run_event_loop()
}

fn frame_to_image(frame: &CapturedFrame) -> Image {
    let pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        frame.rgba(),
        frame.width(),
        frame.height(),
    );
    Image::from_rgba8(pixel_buffer)
}

fn finish_capture(
    ui: &AppWindow,
    latest_frame: &Rc<RefCell<Option<CapturedFrame>>>,
    editor: &Rc<RefCell<EditorSession>>,
    frame: CapturedFrame,
) {
    editor.borrow_mut().reset(frame.clone());
    ui.set_zoom_factor(1.0);
    ui.set_pan_x(0.0);
    ui.set_pan_y(0.0);
    ui.set_ocr_epoch(ui.get_ocr_epoch().wrapping_add(1));
    ui.set_ocr_running(false);
    ui.set_ocr_overlay_visible(false);
    ui.set_ocr_line_count(0);
    ui.set_ocr_text("".into());
    set_editor_frame(ui, latest_frame, frame.clone());
    sync_history(ui, &editor.borrow());
    sync_selection(ui, &editor.borrow());
    ui.set_text_entry_visible(false);

    let clipboard_result = copy_to_clipboard(&frame);
    let dimensions = format!("{}×{}", frame.width(), frame.height());
    match clipboard_result {
        Ok(()) => ui.set_status_text(
            format!("Captured {dimensions} · copied to clipboard · ready to annotate").into(),
        ),
        Err(error) => ui.set_status_text(
            format!("Captured {dimensions} · clipboard failed: {error} · ready to annotate").into(),
        ),
    }
    let _ = ui.show();
}

fn set_editor_frame(
    ui: &AppWindow,
    latest_frame: &Rc<RefCell<Option<CapturedFrame>>>,
    frame: CapturedFrame,
) {
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

fn make_tray_icon() -> Image {
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(32, 32);
    let pixels = buffer.make_mut_slice();
    pixels.fill(Rgba8Pixel {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    });

    for y in 3_usize..29 {
        for x in 3_usize..29 {
            let dx = x.abs_diff(16);
            let dy = y.abs_diff(16);
            if dx + dy < 21 {
                pixels[y * 32 + x] = Rgba8Pixel {
                    r: 126,
                    g: 92,
                    b: 236,
                    a: 255,
                };
            }
        }
    }

    for y in 9_usize..24 {
        let half_width = (y - 9) / 2;
        let left = 16_usize.saturating_sub(half_width);
        let right = (16 + half_width).min(31);
        pixels[y * 32 + left] = Rgba8Pixel {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        pixels[y * 32 + right] = Rgba8Pixel {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
    }
    for x in 11_usize..22 {
        pixels[19 * 32 + x] = Rgba8Pixel {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
    }

    Image::from_rgba8(buffer)
}

fn default_capture_path() -> PathBuf {
    std::env::temp_dir()
        .join("AzusaOCR")
        .join("latest-capture.png")
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
