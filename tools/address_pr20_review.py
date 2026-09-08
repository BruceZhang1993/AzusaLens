from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new)


main = Path("apps/desktop/src/main.rs")
text = main.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''            if let Err(error) = i18n::apply_language(language) {
                feedback::set_status_text(
                    &ui,
                    format!("Could not apply language · {error}").into(),
                );
                return;
            }
            match settings_store.update(|settings| settings.language = language) {''',
    '''            let previous_language = app_settings.borrow().language;
            if let Err(error) = i18n::apply_language(language) {
                feedback::set_status_text(
                    &ui,
                    format!("Could not apply language · {error}").into(),
                );
                return;
            }
            match settings_store.update(|settings| settings.language = language) {''',
    "language apply",
)
text = replace_once(
    text,
    '''                Err(error) => feedback::set_status_text(
                    &ui,
                    format!("Could not save language · {error}").into(),
                ),
            }
        });''',
    '''                Err(error) => {
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
        });''',
    "language rollback",
)
text = replace_once(
    text,
    '                    feedback::set_status_text(&ui, "".into());',
    '                    feedback::set_status_text(&ui, "Edited image copied to clipboard".into());',
    "copy success",
)
text = replace_once(
    text,
    '''    ui.set_ocr_models(ModelRc::new(VecModel::from(items)));
    let engine_name = active_model_id
        .as_deref()''',
    '''    ui.set_ocr_models(ModelRc::new(VecModel::from(items)));
    ui.set_ocr_engine_enabled(active_model_id.is_some());
    let engine_name = active_model_id
        .as_deref()''',
    "OCR state",
)
main.write_text(text, encoding="utf-8")

app = Path("apps/desktop/ui/app-window.slint")
text = app.read_text(encoding="utf-8")
text = replace_once(
    text,
    '    in property <string> ocr-engine-name: @tr("Not configured");\n',
    '    in property <string> ocr-engine-name: @tr("Not configured");\n    in property <bool> ocr-engine-enabled: false;\n',
    "app OCR property",
)
text = replace_once(
    text,
    '        ocr-engine-name: root.ocr-engine-name;\n',
    '        ocr-engine-name: root.ocr-engine-name;\n        ocr-engine-enabled: root.ocr-engine-enabled;\n',
    "app OCR binding",
)
app.write_text(text, encoding="utf-8")

settings = Path("apps/desktop/ui/settings-view.slint")
text = settings.read_text(encoding="utf-8")
text = replace_once(
    text,
    '    in property <string> ocr-engine-name;\n',
    '    in property <string> ocr-engine-name;\n    in property <bool> ocr-engine-enabled: false;\n',
    "settings OCR property",
)
text = replace_once(
    text,
    '                if root.ocr-engine-name == @tr("No model enabled"): Rectangle {',
    '                if !root.ocr-engine-enabled: Rectangle {',
    "settings OCR condition",
)
settings.write_text(text, encoding="utf-8")

i18n = Path("apps/desktop/src/i18n.rs")
text = i18n.read_text(encoding="utf-8")
anchor = '''    if let Some(value) = localized_suffix(raw, "Saved edited PNG · ", "已保存编辑后的 PNG · ")
    {
        return value;
    }
'''
text = replace_once(
    text,
    anchor,
    anchor + '''    if let Some(value) = localized_suffix(raw, "Saved · ", "已保存 · ") {
        return value;
    }
''',
    "quick-save localization",
)
anchor = '''    #[test]
    fn chinese_status_localizes_dynamic_capture_result() {
'''
text = replace_once(
    text,
    anchor,
    '''    #[test]
    fn chinese_status_localizes_quick_save_result() {
        CURRENT_LANGUAGE.with(|language| language.set(EffectiveLanguage::SimplifiedChinese));
        assert_eq!(
            localize_status("Saved · /tmp/capture.png"),
            "已保存 · /tmp/capture.png"
        );
        CURRENT_LANGUAGE.with(|language| language.set(EffectiveLanguage::English));
    }

''' + anchor,
    "i18n test",
)
i18n.write_text(text, encoding="utf-8")
