from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"missing replacement target: {label}")
    return text.replace(old, new, 1)


settings_path = Path("apps/desktop/ui/settings-view.slint")
settings = settings_path.read_text()

settings = replace_once(
    settings,
    'import { ListView } from "std-widgets.slint";',
    'import { ListView, ScrollView } from "std-widgets.slint";',
    "ScrollView import",
)

choice_component = r'''

component ChoiceSettingGroup inherits Rectangle {
    in property <bool> stacked: false;
    in property <string> selected-key;
    in property <string> first-key;
    in property <string> first-label;
    in property <string> first-detail;
    in property <string> second-key;
    in property <string> second-label;
    in property <string> second-detail;
    in property <string> third-key;
    in property <string> third-label;
    in property <string> third-detail;
    callback chosen(string);

    height: root.stacked ? 280px : 94px;
    background: transparent;

    if !root.stacked: HorizontalLayout {
        spacing: 12px;
        ThemeChoice {
            label: root.first-label;
            detail: root.first-detail;
            selected: root.selected-key == root.first-key;
            chosen => { root.chosen(root.first-key); }
        }
        ThemeChoice {
            label: root.second-label;
            detail: root.second-detail;
            selected: root.selected-key == root.second-key;
            chosen => { root.chosen(root.second-key); }
        }
        ThemeChoice {
            label: root.third-label;
            detail: root.third-detail;
            selected: root.selected-key == root.third-key;
            chosen => { root.chosen(root.third-key); }
        }
    }

    if root.stacked: VerticalLayout {
        spacing: 8px;
        ThemeChoice {
            label: root.first-label;
            detail: root.first-detail;
            selected: root.selected-key == root.first-key;
            chosen => { root.chosen(root.first-key); }
        }
        ThemeChoice {
            label: root.second-label;
            detail: root.second-detail;
            selected: root.selected-key == root.second-key;
            chosen => { root.chosen(root.second-key); }
        }
        ThemeChoice {
            label: root.third-label;
            detail: root.third-detail;
            selected: root.selected-key == root.third-key;
            chosen => { root.chosen(root.third-key); }
        }
    }
}
'''
settings = replace_once(settings, "\n\nexport component SettingsView", choice_component + "\nexport component SettingsView", "choice group component")

settings = replace_once(
    settings,
    '    in property <string> language-mode: "system";\n\n    callback close-requested();',
    '''    in property <string> language-mode: "system";

    private property <bool> compact-layout: root.width < 980px;
    private property <bool> narrow-layout: root.width < 840px;
    private property <bool> stack-choice-cards: root.width < 900px;
    private property <length> sidebar-width: root.narrow-layout ? 76px : root.compact-layout ? 204px : 244px;
    private property <length> content-padding: root.narrow-layout ? 18px : root.compact-layout ? 24px : 34px;
    private property <length> content-top: root.compact-layout ? 84px : 92px;

    callback close-requested();''',
    "responsive properties",
)

settings = replace_once(settings, "        width: 244px;", "        width: root.sidebar-width;", "sidebar width")
settings = replace_once(settings, "            padding-left: 14px;\n            padding-right: 14px;\n            padding-top: 22px;", "            padding-left: root.narrow-layout ? 8px : 14px;\n            padding-right: root.narrow-layout ? 8px : 14px;\n            padding-top: root.narrow-layout ? 14px : 22px;", "sidebar padding")
settings = replace_once(settings, "                spacing: 10px;\n                Rectangle {\n                    width: 38px;", "                spacing: root.narrow-layout ? 0px : 10px;\n                Rectangle {\n                    width: 38px;", "brand spacing")
settings = replace_once(settings, "                VerticalLayout {\n                    spacing: 2px;\n                    Text { text: \"Azusa Lens\";", "                VerticalLayout {\n                    visible: !root.narrow-layout;\n                    spacing: 2px;\n                    Text { text: \"Azusa Lens\";", "brand visibility")

navs = {
    '@tr("⚙  General & Appearance")': 'root.narrow-layout ? "⚙" : @tr("⚙  General & Appearance")',
    '@tr("⌨  Shortcuts")': 'root.narrow-layout ? "⌨" : @tr("⌨  Shortcuts")',
    '@tr("⌗  Capture & Annotation")': 'root.narrow-layout ? "⌗" : @tr("⌗  Capture & Annotation")',
    '@tr("▱  Save & Export")': 'root.narrow-layout ? "▱" : @tr("▱  Save & Export")',
    '@tr("OCR  OCR Models")': 'root.narrow-layout ? "OCR" : @tr("OCR  OCR Models")',
    '@tr("ⓘ  About")': 'root.narrow-layout ? "ⓘ" : @tr("ⓘ  About")',
}
for old, new in navs.items():
    settings = replace_once(settings, f"label: {old};", f"label: {new};", old)

settings = replace_once(
    settings,
    '''            Rectangle {
                height: 74px;
                border-radius: 12px;
                background: Theme.surface-raised;''',
    '''            Rectangle {
                visible: !root.narrow-layout;
                height: 74px;
                border-radius: 12px;
                background: Theme.surface-raised;''',
    "quick capture visibility",
)

settings = replace_once(settings, "        x: 244px;", "        x: root.sidebar-width;", "main x")
settings = replace_once(settings, "        width: parent.width - 244px;", "        width: parent.width - root.sidebar-width;", "main width")
settings = replace_once(settings, "                x: 34px;\n                y: 18px;\n                width: parent.width - 68px;", "                x: root.content-padding;\n                y: root.compact-layout ? 14px : 18px;\n                width: parent.width - 2 * root.content-padding;", "header geometry")
settings = replace_once(settings, "                    font-size: 24px;", "                    font-size: root.compact-layout ? 21px : 24px;", "header title size")
settings = replace_once(settings, "                    color: Theme.text-muted;\n                    font-size: 11px;\n                }", "                    color: Theme.text-muted;\n                    font-size: 11px;\n                    overflow: elide;\n                }", "header subtitle elide")

appearance_group = '''                HorizontalLayout {
                    height: 94px;
                    spacing: 12px;
                    ThemeChoice {
                        label: @tr("Follow system");
                        detail: @tr("Use the current system appearance and switch with it.");
                        selected: Theme.mode == "system";
                        chosen => { Theme.mode = "system"; }
                    }
                    ThemeChoice {
                        label: @tr("Light");
                        detail: @tr("Bright background with high-contrast controls.");
                        selected: Theme.mode == "light";
                        chosen => { Theme.mode = "light"; }
                    }
                    ThemeChoice {
                        label: @tr("Dark");
                        detail: @tr("Reduce glare for night use and focused work.");
                        selected: Theme.mode == "dark";
                        chosen => { Theme.mode = "dark"; }
                    }
                }'''
appearance_replacement = '''                ChoiceSettingGroup {
                    stacked: root.stack-choice-cards;
                    selected-key: Theme.mode;
                    first-key: "system";
                    first-label: @tr("Follow system");
                    first-detail: @tr("Use the current system appearance and switch with it.");
                    second-key: "light";
                    second-label: @tr("Light");
                    second-detail: @tr("Bright background with high-contrast controls.");
                    third-key: "dark";
                    third-label: @tr("Dark");
                    third-detail: @tr("Reduce glare for night use and focused work.");
                    chosen(mode) => { Theme.mode = mode; }
                }'''
settings = replace_once(settings, appearance_group, appearance_replacement, "appearance choices")

language_group = '''                HorizontalLayout {
                    height: 94px;
                    spacing: 12px;
                    ThemeChoice {
                        label: @tr("Follow system");
                        detail: @tr("Use the operating system language.");
                        selected: root.language-mode == "system";
                        chosen => { root.language-change-requested("system"); }
                    }
                    ThemeChoice {
                        label: @tr("Simplified Chinese");
                        detail: @tr("Use Simplified Chinese.");
                        selected: root.language-mode == "zh-CN";
                        chosen => { root.language-change-requested("zh-CN"); }
                    }
                    ThemeChoice {
                        label: @tr("English");
                        detail: @tr("Use English.");
                        selected: root.language-mode == "en";
                        chosen => { root.language-change-requested("en"); }
                    }
                }'''
language_replacement = '''                ChoiceSettingGroup {
                    stacked: root.stack-choice-cards;
                    selected-key: root.language-mode;
                    first-key: "system";
                    first-label: @tr("Follow system");
                    first-detail: @tr("Use the operating system language.");
                    second-key: "zh-CN";
                    second-label: @tr("Simplified Chinese");
                    second-detail: @tr("Use Simplified Chinese.");
                    third-key: "en";
                    third-label: @tr("English");
                    third-detail: @tr("Use English.");
                    chosen(mode) => { root.language-change-requested(mode); }
                }'''
settings = replace_once(settings, language_group, language_replacement, "language choices")

settings = replace_once(
    settings,
    '''        if root.page == "general": Rectangle {
            x: 34px;
            y: 92px;
            width: parent.width - 68px;
            height: parent.height - 116px;
            background: transparent;

            VerticalLayout {
                spacing: 16px;''',
    '''        if root.page == "general": general-scroll := ScrollView {
            x: root.content-padding;
            y: root.content-top;
            width: parent.width - 2 * root.content-padding;
            height: parent.height - root.content-top - 16px;
            horizontal-scrollbar-policy: always-off;
            vertical-scrollbar-policy: as-needed;

            VerticalLayout {
                width: general-scroll.visible-width;
                spacing: 16px;''',
    "general scroll",
)
settings = replace_once(
    settings,
    '''        if root.page == "hotkeys": HotkeySettings {
            x: 34px;
            y: 92px;
            width: parent.width - 68px;
            height: parent.height - 116px;''',
    '''        if root.page == "hotkeys": HotkeySettings {
            x: root.content-padding;
            y: root.content-top;
            width: parent.width - 2 * root.content-padding;
            height: parent.height - root.content-top - 16px;''',
    "hotkey geometry",
)
settings = replace_once(
    settings,
    '''        if root.page == "capture": Rectangle {
            x: 34px; y: 92px; width: parent.width - 68px; height: parent.height - 116px;
            background: transparent;
            VerticalLayout {
                spacing: 14px;''',
    '''        if root.page == "capture": capture-scroll := ScrollView {
            x: root.content-padding; y: root.content-top;
            width: parent.width - 2 * root.content-padding;
            height: parent.height - root.content-top - 16px;
            horizontal-scrollbar-policy: always-off;
            vertical-scrollbar-policy: as-needed;
            VerticalLayout {
                width: capture-scroll.visible-width;
                spacing: 14px;''',
    "capture scroll",
)
settings = replace_once(
    settings,
    '''        if root.page == "export": Rectangle {
            x: 34px; y: 92px; width: parent.width - 68px; height: parent.height - 116px;
            background: transparent;
            VerticalLayout {
                spacing: 12px;''',
    '''        if root.page == "export": export-scroll := ScrollView {
            x: root.content-padding; y: root.content-top;
            width: parent.width - 2 * root.content-padding;
            height: parent.height - root.content-top - 16px;
            horizontal-scrollbar-policy: always-off;
            vertical-scrollbar-policy: as-needed;
            VerticalLayout {
                width: export-scroll.visible-width;
                spacing: 12px;''',
    "export scroll",
)
settings = replace_once(
    settings,
    '''        if root.page == "ocr": Rectangle {
            x: 34px;
            y: 90px;
            width: parent.width - 68px;
            height: parent.height - 108px;''',
    '''        if root.page == "ocr": Rectangle {
            x: root.content-padding;
            y: root.content-top;
            width: parent.width - 2 * root.content-padding;
            height: parent.height - root.content-top - 16px;''',
    "ocr geometry",
)
settings = replace_once(
    settings,
    '''        if root.page == "about": Rectangle {
            x: 34px; y: 92px; width: parent.width - 68px; height: parent.height - 116px;
            background: transparent;
            VerticalLayout {
                spacing: 14px;''',
    '''        if root.page == "about": about-scroll := ScrollView {
            x: root.content-padding; y: root.content-top;
            width: parent.width - 2 * root.content-padding;
            height: parent.height - root.content-top - 16px;
            horizontal-scrollbar-policy: always-off;
            vertical-scrollbar-policy: as-needed;
            VerticalLayout {
                width: about-scroll.visible-width;
                spacing: 14px;''',
    "about scroll",
)

settings = replace_once(settings, "                    height: 108px;\n                    border-radius: 12px;", "                    height: root.compact-layout ? 124px : 108px;\n                    border-radius: 12px;", "export card height")
settings = replace_once(settings, "                        VerticalLayout {\n                            spacing: 5px;\n                            Text { text: @tr(\"Quick save directory\")", "                        VerticalLayout {\n                            horizontal-stretch: 1;\n                            spacing: 5px;\n                            Text { text: @tr(\"Quick save directory\")", "export info stretch")
settings = replace_once(settings, '                            Text { text: @tr("Save writes directly to this folder; Save As can choose another location. Default filenames use local date and time."); color: Theme.text-faint; font-size: 9px; }', '                            Text { text: @tr("Save writes directly to this folder; Save As can choose another location. Default filenames use local date and time."); color: Theme.text-faint; font-size: 9px; wrap: word-wrap; overflow: clip; }', "export helper wrap")
settings = replace_once(settings, "                            width: 96px;", "                            width: root.compact-layout ? 88px : 96px;", "choose folder width")
settings = replace_once(settings, "                            width: 72px;\n                            height: 34px;\n                            label: @tr(\"Reset\");", "                            width: root.compact-layout ? 64px : 72px;\n                            height: 34px;\n                            label: @tr(\"Reset\");", "reset width")

settings = replace_once(settings, "                        height: 170px;", "                        height: root.compact-layout ? 190px : 170px;", "ocr card height")
settings = replace_once(settings, "                            padding: 16px;\n                            spacing: 16px;", "                            padding: root.compact-layout ? 12px : 16px;\n                            spacing: root.compact-layout ? 12px : 16px;", "ocr card spacing")
settings = replace_once(settings, "                                width: 60px;\n                                height: 60px;", "                                width: root.compact-layout ? 52px : 60px;\n                                height: root.compact-layout ? 52px : 60px;", "ocr icon size")
settings = replace_once(settings, "                                width: 230px;", "                                width: root.compact-layout ? 180px : 230px;", "ocr metadata width")
settings = replace_once(settings, "                                Text { text: model.name; color: Theme.text; font-size: 15px; font-weight: 700; }", "                                Text { text: model.name; color: Theme.text; font-size: 15px; font-weight: 700; overflow: elide; }", "ocr model name elide")
settings = replace_once(settings, "                                    font-size: 9px;\n                                    overflow: elide;", "                                    font-size: 9px;\n                                    wrap: word-wrap;\n                                    overflow: clip;", "ocr progress wrap")
settings = replace_once(settings, "                                        width: 106px;", "                                        width: root.compact-layout ? 92px : 106px;", "ocr action width")
settings = replace_once(settings, "                                        width: 76px;", "                                        width: root.compact-layout ? 68px : 76px;", "ocr delete width")

settings = replace_once(settings, "            padding-left: 18px;\n            padding-right: 18px;", "            padding-left: root.narrow-layout ? 12px : 18px;\n            padding-right: root.narrow-layout ? 12px : 18px;", "footer padding")
settings = replace_once(settings, "                vertical-alignment: center;\n            }\n            Rectangle { horizontal-stretch: 1;", "                vertical-alignment: center;\n                overflow: elide;\n            }\n            Rectangle { horizontal-stretch: 1;", "footer elide")

settings_path.write_text(settings)

app_path = Path("apps/desktop/ui/app-window.slint")
app = app_path.read_text()
app = replace_once(
    app,
    '''    width: 1180px;
    height: 820px;''',
    '''    width: 1080px;
    height: 760px;
    min-width: 820px;
    min-height: 560px;''',
    "window min size",
)
app_path.write_text(app)

tests_path = Path("apps/desktop/src/overlay_tests.rs")
tests = tests_path.read_text()
marker = '''#[test]
fn capture_overlay_declares_eight_way_resize_interactions() {'''
responsive_test = r'''#[test]
fn settings_ui_declares_responsive_layout_contract() {
    let settings = include_str!("../ui/settings-view.slint");
    let app = include_str!("../ui/app-window.slint");

    for expected in [
        "private property <bool> compact-layout: root.width < 980px",
        "private property <bool> narrow-layout: root.width < 840px",
        "private property <length> sidebar-width",
        "private property <length> content-padding",
        "ChoiceSettingGroup",
        "stacked: root.stack-choice-cards",
        "general-scroll := ScrollView",
        "capture-scroll := ScrollView",
        "export-scroll := ScrollView",
        "about-scroll := ScrollView",
        "height: root.compact-layout ? 190px : 170px",
        "wrap: word-wrap",
    ] {
        assert!(settings.contains(expected), "missing responsive settings behavior: {expected}");
    }
    assert!(app.contains("min-width: 820px"));
    assert!(app.contains("min-height: 560px"));
}

'''
tests = replace_once(tests, marker, responsive_test + marker, "responsive settings test")
tests_path.write_text(tests)
