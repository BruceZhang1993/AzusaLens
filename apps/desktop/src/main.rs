use azusa_capture::{detected_backend, validation_message as capture_validation_message};
use azusa_ocr::{default_engine_name, validation_message as ocr_validation_message};

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    ui.set_platform_name(detected_backend().to_string().into());
    ui.set_ocr_engine_name(default_engine_name().into());
    ui.set_status_text("Ready".into());

    let weak = ui.as_weak();
    ui.on_capture_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_status_text(capture_validation_message().into());
        }
    });

    let weak = ui.as_weak();
    ui.on_ocr_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_status_text(ocr_validation_message().into());
        }
    });

    ui.run()
}
