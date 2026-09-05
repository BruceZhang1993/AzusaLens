use std::{
    borrow::Cow,
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
};

use arboard::{Clipboard, ImageData};
use azusa_capture::{CapturedFrame, capture_primary_monitor, detected_backend};
use azusa_ocr::{default_engine_name, validation_message as ocr_validation_message};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

slint::include_modules!();

thread_local! {
    static CLIPBOARD: RefCell<Option<Clipboard>> = RefCell::new(None);
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let latest_frame = Rc::new(RefCell::new(None::<CapturedFrame>));

    ui.set_platform_name(detected_backend().to_string().into());
    ui.set_ocr_engine_name(default_engine_name().into());
    ui.set_status_text("Ready · capture the primary display to validate the real data path".into());

    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        ui.on_capture_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };

            ui.set_status_text("Capturing primary display…".into());

            match capture_primary_monitor() {
                Ok(frame) => {
                    let pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        frame.rgba(),
                        frame.width(),
                        frame.height(),
                    );
                    ui.set_preview_image(Image::from_rgba8(pixel_buffer));
                    ui.set_has_capture(true);
                    ui.set_status_text(
                        format!(
                            "Captured {}×{} RGBA pixels · preview is live",
                            frame.width(),
                            frame.height()
                        )
                        .into(),
                    );
                    *latest_frame.borrow_mut() = Some(frame);
                }
                Err(error) => {
                    ui.set_status_text(format!("Capture failed · {error}").into());
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
                ui.set_status_text("Nothing to copy · capture a display first".into());
                return;
            };

            match copy_to_clipboard(frame) {
                Ok(()) => ui.set_status_text("Captured image copied to the clipboard".into()),
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
                ui.set_status_text("Nothing to save · capture a display first".into());
                return;
            };

            let path = default_capture_path();
            match frame.save_png(&path) {
                Ok(()) => ui.set_status_text(
                    format!("Saved PNG · {}", path.display()).into(),
                ),
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

    ui.run()
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
