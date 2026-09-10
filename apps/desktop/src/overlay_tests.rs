use super::*;
use slint::platform::{PointerEventButton, WindowEvent};
use std::{cell::Cell, path::PathBuf};

#[test]
fn settings_ui_declares_responsive_layout_contract() {
    let settings = include_str!("../ui/settings-view.slint");
    let ocr_settings = include_str!("../ui/ocr-settings.slint");
    let app = include_str!("../ui/app-window.slint");

    for expected in [
        "private property <bool> compact-layout: root.width < 980px",
        "private property <bool> narrow-layout: root.width < 876px",
        "private property <length> sidebar-width",
        "private property <length> content-padding",
        "ChoiceSettingGroup",
        "stacked: root.stack-choice-cards",
        "general-scroll := ScrollView",
        "export-scroll := ScrollView",
        "about-scroll := ScrollView",
        "OcrSettings",
        "compact-layout: root.compact-layout",
        "wrap: word-wrap",
    ] {
        assert!(
            settings.contains(expected),
            "missing responsive settings behavior: {expected}"
        );
    }
    for expected in [
        "in property <bool> compact-layout: false",
        "height: root.compact-layout ? 230px : 208px",
        "wrap: word-wrap",
    ] {
        assert!(
            ocr_settings.contains(expected),
            "missing responsive OCR settings behavior: {expected}"
        );
    }
    assert!(!ocr_settings.contains("TaskRouteRow"));
    assert!(!ocr_settings.contains("model-name"));
    assert!(!settings.contains("Capture & Annotation"));
    assert!(!settings.contains("capture-scroll"));
    assert!(!settings.contains("Desktop workflow"));
    assert!(!settings.contains("Quick capture"));
    assert!(!settings.contains("Hide to tray"));
    assert!(app.contains("min-width: 820px"));
    assert!(app.contains("min-height: 560px"));
}

#[test]
fn application_starts_from_tray_without_auto_opening_settings() {
    let main = include_str!("main.rs");
    let startup = main
        .split("fn create_runtime(")
        .next()
        .expect("create_runtime should be declared after startup");

    assert!(main.contains("tray.show()?;"));
    assert!(main.contains("let runtime = Rc::new(RefCell::new(None::<UiRuntime>));"));
    assert!(!startup.contains("ensure_portal_desktop_entry"));
    assert!(main.contains("let ui = AppWindow::new()?;"));
    assert!(main.contains("let overlay = RegionOverlay::new()?;"));
    assert!(!startup.contains("AppWindow::new()?;"));
    assert!(!startup.contains("RegionOverlay::new()?;"));
    assert!(!main.contains("resume_editor_overlay"));
}

#[test]
fn capture_overlay_declares_eight_way_resize_interactions() {
    let source = include_str!("../ui/capture-overlay.slint");

    for mode in ["n", "e", "s", "w"] {
        assert!(
            source.contains(&format!("selection-drag-mode == \"{mode}\"")),
            "missing edge resize branch for {mode}"
        );
    }
    for cursor in ["ns-resize", "ew-resize", "nwse-resize", "nesw-resize"] {
        assert!(source.contains(cursor), "missing resize cursor {cursor}");
    }
    assert!(
        source.contains("let horizontal-threshold = min(threshold, root.selection-width / 3);"),
        "thin selections must preserve a horizontal move zone"
    );
    assert!(
        source.contains("let vertical-threshold = min(threshold, root.selection-height / 3);"),
        "thin selections must preserve a vertical move zone"
    );
    assert_eq!(
        source.matches("background: Theme.selection-handle").count(),
        8,
        "capture selection should render four corner and four edge handles"
    );
}

#[test]
fn editor_overlay_declares_precise_keyboard_nudge_contract() {
    let overlay = include_str!("../ui/capture-overlay.slint");
    let main = include_str!("main.rs");

    for key in [
        "Key.LeftArrow",
        "Key.RightArrow",
        "Key.UpArrow",
        "Key.DownArrow",
    ] {
        assert!(overlay.contains(key), "missing keyboard nudge key: {key}");
    }
    assert!(overlay.contains("let step = event.modifiers.shift ? 10 : 1;"));
    assert!(overlay.contains("root.nudge-selection-requested(-step, 0);"));
    assert!(overlay.contains("root.nudge-selection-requested(0, step);"));
    assert!(
        main.matches("sync_editor_overlay(&ui, &overlay);").count() >= 8,
        "overlay-originating editor actions must synchronize hidden AppWindow state back to the visible overlay"
    );
}

#[test]
fn capture_overlay_magnifier_uses_physical_pixels_without_obscuring_move() {
    let source = include_str!("../ui/capture-overlay.slint");

    for expected in [
        "source-clip-x: root.magnifier-source-x",
        "source-clip-y: root.magnifier-source-y",
        "image-rendering: pixelated",
        "private property <float> source-scale-x: root.screenshot.width / max(1, root.width / 1px)",
        "private property <float> source-scale-y: root.screenshot.height / max(1, root.height / 1px)",
        "selection-source-left: floor(max(0, root.selection-x) * root.source-scale-x)",
        "selection-source-top: floor(max(0, root.selection-y) * root.source-scale-y)",
        "selection-source-right: ceil(max(0, root.selection-x + root.selection-width) * root.source-scale-x)",
        "selection-source-bottom: ceil(max(0, root.selection-y + root.selection-height) * root.source-scale-y)",
        "root.selection-source-width + \" × \" + root.selection-source-height",
        "root.selection-drag-mode != \"move\"",
        "root.pointer-source-x - root.magnifier-source-x + 0.5",
        "root.pointer-source-y - root.magnifier-source-y + 0.5",
    ] {
        assert!(
            source.contains(expected),
            "missing magnifier behavior: {expected}"
        );
    }
    assert!(source.contains("private property <int> magnifier-source-size: 17"));
    assert!(source.contains("width: 136px; height: 136px"));
}

fn text_model() -> ModelRc<OcrOverlayItem> {
    ModelRc::new(VecModel::from(
        ["轻量截图工具", "Select, annotate, copy", "OCR 文字识别"]
            .into_iter()
            .enumerate()
            .map(|(index, text)| {
                let y = 30.0 + index as f32 * 50.0;
                OcrOverlayItem {
                    text: text.into(),
                    x: 20.0,
                    y,
                    width: 260.0,
                    height: 30.0,
                    p0_x: 20.0,
                    p0_y: y,
                    p1_x: 280.0,
                    p1_y: y,
                    p2_x: 280.0,
                    p2_y: y + 30.0,
                    p3_x: 20.0,
                    p3_y: y + 30.0,
                    ..Default::default()
                }
            })
            .collect::<Vec<_>>(),
    ))
}

#[test]
fn ocr_selection_preserves_reading_order_in_both_directions() {
    let model = text_model();
    assert_eq!(ocr_selection_text(&model, 1, 1), "Select, annotate, copy");
    assert_eq!(
        ocr_selection_text(&model, 0, 2),
        ocr_selection_text(&model, 2, 0)
    );
    assert_eq!(
        ocr_selection_text(&model, 0, 2),
        "轻量截图工具\nSelect, annotate, copy\nOCR 文字识别"
    );
    for (anchor, focus) in [(-1, -1), (0, -1), (0, 3), (3, 0)] {
        assert!(ocr_selection_text(&model, anchor, focus).is_empty());
    }
    assert_eq!(ocr_selection_range(0, 0, 0), None);
}

#[test]
fn ocr_hit_testing_selects_blocks_and_rejects_blank_space() {
    let model = text_model();
    assert_eq!(ocr_hit_test(&model, 40.0, 40.0), Some(0));
    assert_eq!(ocr_hit_test(&model, 40.0, 140.0), Some(2));
    assert_eq!(ocr_hit_test(&model, 40.0, 70.0), None);
    assert_eq!(ocr_hit_test(&model, 300.0, 40.0), None);
}

fn pointer(overlay: &RegionOverlay, x: f32, y: f32, pressed: bool) {
    let position = slint::LogicalPosition::new(x, y);
    overlay
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    let event = if pressed {
        WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        }
    } else {
        WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        }
    };
    overlay.window().dispatch_event(event);
}

fn click(overlay: &RegionOverlay, x: f32, y: f32) {
    pointer(overlay, x, y, true);
    pointer(overlay, x, y, false);
}

fn key(overlay: &RegionOverlay, text: &str, control: bool) {
    if control {
        overlay.window().dispatch_event(WindowEvent::KeyPressed {
            text: slint::platform::Key::Control.into(),
        });
    }
    overlay
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.into() });
    overlay
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text: text.into() });
    if control {
        overlay.window().dispatch_event(WindowEvent::KeyReleased {
            text: slint::platform::Key::Control.into(),
        });
    }
}

fn snapshot(_overlay: &RegionOverlay, name: &str) {
    if let Ok(directory) = std::env::var("AZUSA_OVERLAY_QA_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        let path = PathBuf::from(directory).join(format!("{name}.png"));
        // Read the displayed X11 window, avoiding back-buffer snapshots on GLX.
        let status = std::process::Command::new("import")
            .args(["-window", "Azusa Lens Region Capture"])
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success());
    }
}

// Runs the actual Slint window and input handling. Use an isolated X11 display:
// ImageMagick is needed only when AZUSA_OVERLAY_QA_DIR is set.
// DISPLAY=:97 SLINT_BACKEND=winit cargo test -p azusa-lens-desktop native_overlay -- --ignored
#[test]
#[ignore = "requires a graphical display; optionally writes snapshots to AZUSA_OVERLAY_QA_DIR"]
fn native_overlay_keyboard_selection_and_popovers() {
    let overlay = RegionOverlay::new().unwrap();
    overlay.global::<Theme>().set_mode("light".into());
    let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(1280, 900);
    pixels.make_mut_slice().fill(Rgba8Pixel {
        r: 238,
        g: 242,
        b: 247,
        a: 255,
    });
    for index in 0..3 {
        azusa_annotation::render_annotation_in_place(
            pixels.make_mut_bytes(),
            1280,
            900,
            &azusa_annotation::Annotation::Text {
                origin: azusa_annotation::Point::new(200.0, 210.0 + index as f32 * 50.0),
                value: text_model().row_data(index).unwrap().text.to_string(),
                style: azusa_annotation::AnnotationStyle {
                    color: azusa_annotation::Color::rgb(30, 41, 59),
                    font_size: 22.0,
                    ..Default::default()
                },
            },
        )
        .unwrap();
    }
    overlay.set_screenshot(Image::from_rgba8(pixels.clone()));
    let frame = CapturedFrame::new(1280, 900, pixels.as_bytes().to_vec()).unwrap();
    overlay.set_active_tool("select".into());
    let cancelled = Rc::new(Cell::new(0));
    let count = cancelled.clone();
    overlay.on_cancelled(move || count.set(count.get() + 1));
    let image_copies = Rc::new(Cell::new(0));
    let count = image_copies.clone();
    overlay.on_copy_requested(move || count.set(count.get() + 1));
    let text_copies = Rc::new(Cell::new(0));
    let count = text_copies.clone();
    overlay.on_copy_ocr_selection_requested(move |_, _| count.set(count.get() + 1));
    let weak = overlay.as_weak();
    overlay.on_selection_confirmed(move |x, y, width, height| {
        let overlay = weak.upgrade().unwrap();
        let crop = frame
            .crop(CaptureRect::new(
                x as u32,
                y as u32,
                width as u32,
                height as u32,
            ))
            .unwrap();
        overlay.set_preview_image(frame_to_image(&crop));
        overlay.set_capture_width(width);
        overlay.set_capture_height(height);
        overlay.set_has_capture(true);
        overlay.set_editor_visible(true);
    });
    let model = text_model();
    overlay.set_ocr_items(model.clone());
    overlay
        .on_ocr_hit_test(move |x, y| ocr_hit_test(&model, x, y).map_or(-1, |index| index as i32));
    overlay.show().unwrap();
    overlay
        .window()
        .set_size(slint::LogicalSize::new(1280.0, 900.0));
    let step = Rc::new(Cell::new(0));
    let timer = Timer::default();
    let weak = overlay.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(500), move || {
        let overlay = weak.upgrade().unwrap();
        match step.get() {
            0 => {
                snapshot(&overlay, "01-capture");
                pointer(&overlay, 180.0, 180.0, true);
                pointer(&overlay, 1000.0, 650.0, false);
                assert!(!overlay.get_editor_visible());
                key(
                    &overlay,
                    &slint::SharedString::from(slint::platform::Key::Return),
                    false,
                );
                assert!(overlay.get_editor_visible());
            }
            1 => {
                snapshot(&overlay, "02-toolbar-light");
                // The 1280px fixture places the rectangle button at x=398 and the color/stroke
                // properties button at x=686. The previous single coordinate belonged to a
                // different tool after the compact toolbar changed.
                click(&overlay, 398.0, 680.0);
                assert_eq!(overlay.get_active_tool(), "rectangle");
                click(&overlay, 686.0, 680.0);
            }
            2 => {
                snapshot(&overlay, "03-properties");
                key(
                    &overlay,
                    &slint::SharedString::from(slint::platform::Key::Escape),
                    false,
                );
                assert_eq!(
                    cancelled.get(),
                    0,
                    "Esc must close the property popover first"
                );
                click(&overlay, 650.0, 680.0);
            }
            3 => {
                snapshot(&overlay, "04-more-tools");
                key(
                    &overlay,
                    &slint::SharedString::from(slint::platform::Key::Escape),
                    false,
                );
                assert_eq!(cancelled.get(), 0);
                overlay.set_ocr_line_count(3);
                overlay.set_ocr_overlay_visible(true);
                overlay.set_ocr_selection_anchor(-1);
                overlay.set_ocr_selection_focus(-1);
            }
            4 => {
                key(&overlay, "c", true);
                assert_eq!(
                    image_copies.get(),
                    0,
                    "OCR with no selection must not copy the image"
                );
                assert_eq!(text_copies.get(), 0);
                click(&overlay, 220.0, 220.0);
                assert_eq!(overlay.get_ocr_selection_anchor(), 0);
                pointer(&overlay, 220.0, 320.0, true);
                pointer(&overlay, 220.0, 220.0, false);
                assert_eq!(overlay.get_ocr_selection_anchor(), 2);
                assert_eq!(overlay.get_ocr_selection_focus(), 0);
                key(&overlay, "c", true);
                assert_eq!(text_copies.get(), 1);
                click(&overlay, 500.0, 450.0);
                assert_eq!(overlay.get_ocr_selection_anchor(), -1);
                key(&overlay, "a", true);
                assert_eq!(overlay.get_ocr_selection_anchor(), 0);
                assert_eq!(overlay.get_ocr_selection_focus(), 2);
            }
            5 => {
                snapshot(&overlay, "05-ocr-selection");
                overlay.global::<Theme>().set_mode("dark".into());
            }
            6 => {
                snapshot(&overlay, "06-ocr-dark");
                overlay.set_ocr_overlay_visible(false);
                overlay.set_active_tool("select".into());
            }
            7 => {
                snapshot(&overlay, "07-toolbar-dark");
                key(&overlay, "c", true);
                assert_eq!(image_copies.get(), 1);
                key(
                    &overlay,
                    &slint::SharedString::from(slint::platform::Key::Escape),
                    false,
                );
                assert_eq!(cancelled.get(), 1);
                overlay.set_editor_visible(false);
                pointer(&overlay, 980.0, 780.0, true);
                pointer(&overlay, 1260.0, 880.0, false);
            }
            8 => {
                snapshot(&overlay, "08-bottom-right");
                overlay.set_editor_visible(false);
                pointer(&overlay, 8.0, 8.0, true);
                pointer(&overlay, 128.0, 88.0, false);
            }
            9 => {
                snapshot(&overlay, "09-small-top-left");
                overlay.set_editor_visible(false);
                pointer(&overlay, 0.0, 0.0, true);
                pointer(&overlay, 1280.0, 900.0, false);
            }
            10 => {
                snapshot(&overlay, "10-fullscreen");
                overlay.set_editor_visible(false);
                overlay
                    .window()
                    .set_size(slint::LogicalSize::new(500.0, 700.0));
            }
            11 => {
                pointer(&overlay, 380.0, 500.0, true);
                pointer(&overlay, 480.0, 660.0, false);
                key(
                    &overlay,
                    &slint::SharedString::from(slint::platform::Key::Return),
                    false,
                );
            }
            12 => {
                snapshot(&overlay, "11-narrow");
                overlay.window().dispatch_event(WindowEvent::PointerMoved {
                    position: slint::LogicalPosition::new(260.0, 470.0),
                });
            }
            13 => {
                snapshot(&overlay, "12-tooltip");
                click(&overlay, 260.0, 470.0);
            }
            14 => {
                snapshot(&overlay, "13-narrow-tools");
                key(
                    &overlay,
                    &slint::SharedString::from(slint::platform::Key::Escape),
                    false,
                );
                overlay.set_text_entry_visible(true);
                overlay.set_pending_text("小选区文字标注".into());
            }
            15 => {
                snapshot(&overlay, "14-narrow-text-entry");
                key(&overlay, "c", true);
                assert_eq!(
                    image_copies.get(),
                    1,
                    "Typing must not trigger image shortcuts"
                );
                key(
                    &overlay,
                    &slint::SharedString::from(slint::platform::Key::Escape),
                    false,
                );
                assert!(!overlay.get_text_entry_visible());
                assert!(overlay.get_pending_text().is_empty());
                assert_eq!(
                    cancelled.get(),
                    1,
                    "Esc must cancel text entry without ending capture"
                );
                overlay.hide().unwrap();
                slint::quit_event_loop().unwrap();
            }
            _ => unreachable!(),
        }
        step.set(step.get() + 1);
    });
    slint::run_event_loop().unwrap();
}
