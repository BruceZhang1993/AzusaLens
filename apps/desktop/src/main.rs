use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

use arboard::{Clipboard, ImageData};
use azusa_capture::{
    CaptureRect, CapturedFrame, RegionCapture, begin_region_capture, detected_backend,
};
use azusa_hotkey::{PrintScreenHotkey, backend_description as hotkey_backend_description};
use azusa_ocr::{default_engine_name, validation_message as ocr_validation_message};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode};

slint::include_modules!();

thread_local! {
    static CLIPBOARD: RefCell<Option<Clipboard>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Copy)]
enum CaptureOrigin {
    MainWindow,
    Background,
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let overlay = RegionOverlay::new()?;
    let tray = AppTray::new()?;

    let latest_frame = Rc::new(RefCell::new(None::<CapturedFrame>));
    let pending_frame = Rc::new(RefCell::new(None::<CapturedFrame>));
    let capture_origin = Rc::new(Cell::new(CaptureOrigin::MainWindow));
    let capture_active = Rc::new(Cell::new(false));

    ui.set_platform_name(detected_backend().to_string().into());
    ui.set_hotkey_name(hotkey_backend_description().into());
    ui.set_ocr_engine_name(default_engine_name().into());
    ui.set_status_text(
        "Ready · press PrtSc or choose Capture region · closing the window keeps the tray active"
            .into(),
    );
    tray.set_app_icon(make_tray_icon());

    let start_capture: Rc<dyn Fn(CaptureOrigin)> = {
        let ui_weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let pending_frame = Rc::clone(&pending_frame);
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
                    finish_capture(&ui, &latest_frame, frame, origin);
                    capture_active.set(false);
                }
                Ok(RegionCapture::NeedsSelection(frame)) => {
                    overlay.set_screenshot(frame_to_image(&frame));
                    *pending_frame.borrow_mut() = Some(frame);
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
                    Ok(frame) => finish_capture(&ui, &latest_frame, frame, origin),
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
                Ok(()) => ui.set_status_text("Captured region copied to the clipboard".into()),
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
                Ok(()) => ui.set_status_text(format!("Saved PNG · {}", path.display()).into()),
                Err(error) => ui.set_status_text(format!("Save failed · {error}").into()),
            }
        });
    }

    {
        let weak = ui.as_weak();
        ui.on_ocr_requested(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_status_text(ocr_validation_message().into());
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
    frame: CapturedFrame,
    origin: CaptureOrigin,
) {
    ui.set_preview_image(frame_to_image(&frame));
    ui.set_has_capture(true);

    let clipboard_result = copy_to_clipboard(&frame);
    let dimensions = format!("{}×{}", frame.width(), frame.height());
    *latest_frame.borrow_mut() = Some(frame);

    match clipboard_result {
        Ok(()) => {
            ui.set_status_text(format!("Captured {dimensions} region · copied to clipboard").into())
        }
        Err(error) => ui.set_status_text(
            format!("Captured {dimensions} region · clipboard failed: {error}").into(),
        ),
    }

    if matches!(origin, CaptureOrigin::MainWindow) {
        let _ = ui.show();
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
