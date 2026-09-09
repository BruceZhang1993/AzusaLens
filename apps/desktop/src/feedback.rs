use std::{path::Path, thread};

use azusa_ocr::OcrTaskKind;
use notify_rust::{Notification, Timeout};
use slint::SharedString;

use crate::{AppWindow, file_ocr, i18n};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeedbackLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl FeedbackLevel {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    const fn duration_ms(self) -> i64 {
        match self {
            Self::Info => 3_000,
            Self::Success => 2_200,
            Self::Warning => 4_000,
            Self::Error => 5_500,
        }
    }
}

pub(crate) fn set_status_text(ui: &AppWindow, text: SharedString) {
    let raw = text.to_string();
    if raw.is_empty() {
        ui.set_status_text(SharedString::new());
        return;
    }

    let level = classify(&raw);
    let localized = i18n::localize_status(&raw);

    // Toggle through an empty value so a new feedback event restarts the bound Slint timer
    // even when another toast is already visible.
    ui.set_status_text(SharedString::new());
    ui.set_status_level(level.as_str().into());
    ui.set_status_duration(level.duration_ms());
    ui.set_status_text(localized.clone().into());

    if should_use_system_notification(&raw, level) {
        show_system_notification(level, &localized);
    }
}

pub(crate) fn show_ocr_result_notification(task: OcrTaskKind, path: &Path) {
    let path = path.to_path_buf();
    let is_chinese = i18n::current_language() == i18n::EffectiveLanguage::SimplifiedChinese;
    let task_name = match (is_chinese, task) {
        (true, OcrTaskKind::Text) => "文字识别",
        (true, OcrTaskKind::Document) => "文档解析",
        (true, OcrTaskKind::Table) => "表格识别",
        (true, OcrTaskKind::Figure) => "图形解析",
        (false, _) => task.display_name(),
    }
    .to_owned();
    let summary = if is_chinese {
        format!("Azusa Lens · {task_name}完成")
    } else {
        format!("Azusa Lens · {task_name} completed")
    };
    let body = if is_chinese {
        format!("结果已保存到 Documents\n{}", path.display())
    } else {
        format!("Saved to Documents\n{}", path.display())
    };
    let open_label = if is_chinese { "打开文件" } else { "Open file" };
    let folder_label = if is_chinese {
        "打开所在文件夹"
    } else {
        "Open folder"
    };

    let _ = thread::Builder::new()
        .name("azusa-ocr-notification".to_owned())
        .spawn(move || {
            let handle = match Notification::new()
                .appname("Azusa Lens")
                .summary(&summary)
                .body(&body)
                .action("open", open_label)
                .action("folder", folder_label)
                .timeout(Timeout::Never)
                .show()
            {
                Ok(handle) => handle,
                Err(error) => {
                    eprintln!("OCR result notification failed: {error}");
                    return;
                }
            };

            handle.wait_for_action(|action| match action {
                "open" => {
                    if let Err(error) = file_ocr::open_with_default_app(&path) {
                        eprintln!("Could not open OCR result from notification: {error}");
                    }
                }
                "folder" => {
                    if let Err(error) = file_ocr::reveal_in_file_manager(&path) {
                        eprintln!("Could not reveal OCR result from notification: {error}");
                    }
                }
                _ => {}
            });
        });
}

fn classify(raw: &str) -> FeedbackLevel {
    let lower = raw.to_ascii_lowercase();
    if lower.contains(" failed")
        || lower.starts_with("failed")
        || lower.starts_with("could not")
        || lower.contains(" invalid")
        || lower.contains(" rejected")
        || lower.contains(" unavailable")
        || lower.starts_with("unsupported")
    {
        return FeedbackLevel::Error;
    }

    if lower.contains("cancelled")
        || lower.contains("cancelling")
        || lower.contains(" unchanged")
        || lower.starts_with("no ")
        || lower.starts_with("nothing ")
        || lower.contains("remains disabled")
        || lower.starts_with("choose an ocr model")
    {
        return FeedbackLevel::Warning;
    }

    if lower.contains(" saved")
        || lower.starts_with("saved")
        || lower.contains(" copied")
        || lower.starts_with("image copied")
        || lower.contains(" completed")
        || lower.starts_with("downloaded")
        || lower.starts_with("removed")
        || lower.starts_with("enabled")
        || lower.contains(" active")
        || lower.starts_with("updated")
        || lower.starts_with("deleted")
        || lower.starts_with("all annotations cleared")
        || lower.starts_with("undid")
        || lower.starts_with("redid")
        || lower.starts_with("previous shortcut restored")
    {
        return FeedbackLevel::Success;
    }

    FeedbackLevel::Info
}

fn should_use_system_notification(raw: &str, level: FeedbackLevel) -> bool {
    if matches!(level, FeedbackLevel::Error)
        && (raw.starts_with("Region capture worker failed")
            || raw.starts_with("Region capture failed")
            || raw.starts_with("Selection overlay failed")
            || raw.starts_with("OCR model download failed")
            || raw.starts_with("OCR model removal failed"))
    {
        return true;
    }

    raw.starts_with("Downloaded ") && raw.ends_with(" · select Enable to use it for OCR")
}

fn show_system_notification(level: FeedbackLevel, body: &str) {
    let summary = match (i18n::current_language(), level) {
        (i18n::EffectiveLanguage::SimplifiedChinese, FeedbackLevel::Error) => "Azusa Lens · 错误",
        (i18n::EffectiveLanguage::SimplifiedChinese, FeedbackLevel::Success) => {
            "Azusa Lens · 已完成"
        }
        (_, FeedbackLevel::Error) => "Azusa Lens · Error",
        (_, FeedbackLevel::Success) => "Azusa Lens · Completed",
        _ => "Azusa Lens",
    };

    if let Err(error) = Notification::new()
        .appname("Azusa Lens")
        .summary(summary)
        .body(body)
        .timeout(Timeout::Milliseconds(4_000))
        .show()
    {
        eprintln!("System notification failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_classification_is_stable_for_common_actions() {
        assert_eq!(classify("Image copied"), FeedbackLevel::Success);
        assert_eq!(classify("Save As cancelled"), FeedbackLevel::Warning);
        assert_eq!(
            classify("Region capture failed · denied"),
            FeedbackLevel::Error
        );
        assert_eq!(classify("Starting region capture…"), FeedbackLevel::Info);
    }

    #[test]
    fn system_notifications_are_reserved_for_background_worthy_events() {
        assert!(!should_use_system_notification(
            "Image copied",
            FeedbackLevel::Success
        ));
        assert!(!should_use_system_notification(
            "Saved edited PNG · /tmp/a.png",
            FeedbackLevel::Success
        ));
        assert!(should_use_system_notification(
            "OCR model download failed · network",
            FeedbackLevel::Error
        ));
        assert!(should_use_system_notification(
            "Downloaded Fast OCR · select Enable to use it for OCR",
            FeedbackLevel::Success
        ));
    }
}
