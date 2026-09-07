from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "apps" / "desktop" / "ui"
DESKTOP = ROOT / "apps" / "desktop"

EN_TO_ZH = {
    "Unknown": "未知",
    "Not configured": "未配置",
    "Unknown backend": "未知后端",
    "Ready": "就绪",
    "Azusa Lens · Settings": "Azusa Lens · 设置",
    "Adjust capture area": "调整截图范围",
    "Drag to select capture area": "拖动选择截图范围",
    "Confirm capture · Enter": "确认截图 · Enter",
    "Capture full screen": "截取全屏",
    "Cancel capture · Esc": "取消截图 · Esc",
    "Copy": "复制",
    "Create text annotation": "创建文字标注",
    "Copy all text": "复制全部文字",
    "Text selected; drag across lines or press Ctrl+A to select all": "已选择文字，可拖选多行或按 Ctrl+A 全选",
    "Capture something to begin": "开始一次截图",
    "Press PrtSc or start a new capture. Tools appear only when you need them.": "按 PrtSc 或开始新截图。工具仅在需要时显示。",
    "New capture": "新建截图",
    "Add text": "添加文字",
    "More tools": "更多工具",
    "Ellipse": "椭圆",
    "Line": "直线",
    "Number": "序号",
    "Blur": "模糊",
    "Object properties": "对象属性",
    "Tool properties": "工具属性",
    "Stroke width": "线宽",
    "Delete": "删除",
    "Undo": "撤销",
    "Redo": "重做",
    "Copy image": "复制图片",
    "Save": "保存",
    "Save As…": "另存为…",
    "Copy all OCR text": "复制全部 OCR 文字",
    "Clear annotations": "清空标注",
    "Annotation text": "标注文字",
    "Confirm text · Enter": "确认文字 · Enter",
    "Enter annotation text": "请输入标注文字",
    "Cancel input · Esc": "取消输入 · Esc",
    "Region capture": "区域截图",
    "Global shortcut": "全局快捷键",
    "Configured": "已配置",
    "Press a new shortcut…": "请按下新的快捷键…",
    "Active": "当前生效",
    "Registering": "正在注册",
    "Not registered": "当前未注册",
    "Waiting for the system shortcut service…": "正在等待系统快捷键服务响应…",
    "Retry the current binding or record a new shortcut.": "可重试当前配置，或录制新的快捷键。",
    "Recording…": "按键中…",
    "Change shortcut": "修改快捷键",
    "Restore default": "恢复默认",
    "Retry": "重试",
    "Managed by Wayland": "Wayland 系统管理",
    "Shortcut rules": "快捷键规则",
    "Azusa Lens submits the recorded shortcut as the preferred binding to the XDG GlobalShortcuts Portal. The desktop may ask for confirmation or assign a different key; “Active” always shows the binding returned by the system.": "Azusa Lens 会把录制的组合键作为首选值提交给 XDG GlobalShortcuts Portal；桌面环境可以要求确认或分配不同按键，上方“当前生效”始终显示系统返回的实际绑定。",
    "Letters, digits, and navigation keys require at least Ctrl, Alt, Shift, or Super. PrtSc and F1–F24 can be used as global shortcuts without modifiers.": "普通字母、数字和导航键需要至少包含 Ctrl、Alt、Shift 或 Super；PrtSc 与 F1–F24 可以单独作为全局快捷键。",
    "Press Esc to cancel recording.": "按 Esc 取消录制。",
    "Select annotation": "选择标注",
    "Rectangle": "矩形",
    "Arrow": "箭头",
    "Pen": "画笔",
    "Text": "文字",
    "Mosaic": "马赛克",
    "Color and stroke": "颜色与粗细",
    "Back to annotations": "返回标注",
    "Select all text · Ctrl+A": "全选文字 · Ctrl+A",
    "No text available": "没有可选择的文字",
    "Copy selected text · Ctrl+C": "复制所选文字 · Ctrl+C",
    "Select text first": "请先选择文字",
    "Undo · Ctrl+Z": "撤销 · Ctrl+Z",
    "Nothing to undo": "没有可撤销的标注",
    "Redo · Ctrl+Shift+Z": "重做 · Ctrl+Shift+Z",
    "Nothing to redo": "没有可重做的标注",
    "Recognize text": "识别文字",
    "Recognizing text…": "正在识别文字…",
    "OCR model is busy": "OCR 模型正在处理中",
    "Save image · Ctrl+S": "保存图片 · Ctrl+S",
    "End capture · Esc": "结束截图 · Esc",
    "Delete selected annotation · Delete": "删除所选标注 · Delete",
    "Select an annotation first": "请先选择标注",
    "No annotations to clear": "没有可清空的标注",
    "Save As… · Ctrl+Shift+S": "另存为… · Ctrl+Shift+S",
    "Settings & Management": "设置与管理",
    "⚙  General & Appearance": "⚙  通用与外观",
    "⌨  Shortcuts": "⌨  快捷键",
    "⌗  Capture & Annotation": "⌗  截图与标注",
    "▱  Save & Export": "▱  保存与导出",
    "OCR  OCR Models": "OCR  OCR 模型",
    "ⓘ  About": "ⓘ  关于",
    "Quick capture": "快速截图",
    "You can also start a capture from the tray menu.": "也可从托盘菜单开始截图",
    "General & Appearance": "通用与外观",
    "Shortcuts": "快捷键",
    "Capture & Annotation": "截图与标注",
    "Save & Export": "保存与导出",
    "OCR Models": "OCR 模型",
    "About Azusa Lens": "关于 Azusa Lens",
    "Control application appearance and desktop workflow": "控制应用外观与桌面工作方式",
    "Record and manage the global shortcut for quick capture": "录制并管理快速截图的全局快捷键",
    "Adjust capture and annotation behavior": "调整截图与标注行为",
    "Manage save locations, clipboard, and completion actions": "管理保存位置、剪贴板和完成动作",
    "Download and enable models to recognize text in captures": "下载并启用模型后，在截图中识别文字",
    "Lightweight, local-first capture and recognition tool": "轻量、本地优先的截图与识别工具",
    "Appearance": "外观",
    "Supports light, dark, and automatic system appearance.": "支持亮色、暗色，以及跟随系统自动切换。",
    "Follow system": "跟随系统",
    "Use the current system appearance and switch with it.": "使用当前系统外观，并随系统切换。",
    "Light": "亮色",
    "Bright background with high-contrast controls.": "明亮背景与高对比度操作界面。",
    "Dark": "暗色",
    "Reduce glare for night use and focused work.": "降低眩光，适合夜间和专注工作。",
    "Language": "语言",
    "Choose the language used by Azusa Lens.": "选择 Azusa Lens 使用的界面语言。",
    "Use the operating system language.": "使用操作系统语言。",
    "Simplified Chinese": "简体中文",
    "Use Simplified Chinese.": "使用简体中文。",
    "English": "English",
    "Use English.": "使用英文。",
    "Desktop workflow": "桌面工作流",
    "Lives in the tray": "托盘常驻",
    "The tray icon is Azusa Lens's persistent entry point for opening settings or starting a region capture.": "托盘图标是 Azusa Lens 的常驻入口，可打开主界面或直接开始区域截图。",
    "Current platform: ": "当前平台：",
    "Local-first OCR": "本地优先 OCR",
    "The first recognition never downloads a model automatically; download and enable one manually on the OCR Models page.": "首次识别不会自动下载模型；模型必须在 OCR 模型页手动下载并启用。",
    "Current model: ": "当前模型：",
    "Capture interface": "截图操作界面",
    "The capture stays visually central; common annotation tools float beside the canvas and properties appear contextually.": "截图内容始终保持在视觉中心；常用标注工具悬浮在画布侧边，属性按上下文显示。",
    "Completion actions stay in the lower-right: copy, save, OCR, and more.": "完成动作集中在右下角：复制、保存、OCR 与更多操作。",
    "Capture session retention": "截图会话保留",
    "A capture session is currently available and remains available after capture actions.": "当前已有截图会话；截图操作完成后会继续保留。",
    "No capture session is currently available. Start a region capture to work in the overlay.": "当前没有截图会话。开始区域截图后即可在 overlay 中操作。",
    "Start region capture": "开始区域截图",
    "Quick save directory": "快速保存目录",
    "System Pictures / Screenshots": "系统图片目录 / Screenshots",
    "Save writes directly to this folder; Save As can choose another location. Default filenames use local date and time.": "保存会直接写入此目录；另存为可单独选择位置。默认文件名使用本地日期时间。",
    "Choose folder": "选择目录",
    "Reset": "重置",
    "Copy after capture": "截图后自动复制",
    "Immediately copy the unannotated image after selecting a region.": "完成区域框选后立即把未标注的截图写入剪贴板。",
    "End capture session after copy": "复制后结束截图会话",
    "Automatically close the capture overlay and return to the tray after copying the final edited image.": "复制最终编辑结果后自动关闭截图 overlay，并回到托盘。",
    "End capture session after save": "保存后结束截图会话",
    "Automatically close after Quick Save or Save As succeeds; cancelling Save As keeps the session open.": "快速保存或另存为成功后自动关闭截图 overlay；取消另存为不会结束会话。",
    "Only PNG export is currently supported; the OCR text layer remains interactive and is not written into the image.": "当前仅导出 PNG；OCR 文本层仍为交互层，不会写入图片。",
    "No model enabled": "未启用模型",
    "No model is enabled": "当前没有已启用的模型",
    "Local · ": "本地 · ",
    "Enabled": "已启用",
    "Downloaded · disabled": "已下载 · 未启用",
    "Not downloaded": "尚未下载",
    "Downloading · ": "正在下载 · ",
    "Processing": "正在处理",
    "Download failed · ": "下载失败 · ",
    "Cancel download": "取消下载",
    "Processing…": "处理中…",
    "Enable": "启用",
    "Download": "下载",
    "Lightweight, local-first, extensible desktop capture and recognition tool": "轻量、本地优先、可扩展的桌面截图与识别工具",
    "Platform: ": "平台：",
    "OCR: ": "OCR：",
    "Capture and annotations are retained": "截图和标注已保留",
    "Azusa Lens is running in the tray": "Azusa Lens 正在托盘中运行",
    "Hide to tray": "隐藏到托盘",
    "Azusa Lens · Capture, OCR & Annotation": "Azusa Lens · 截图、OCR 与标注",
    "New region capture (PrtSc)": "新建区域截图 (PrtSc)",
    "Settings": "设置",
    "Quit Azusa Lens": "退出 Azusa Lens",
}

ZH_TO_EN = {zh: en for en, zh in EN_TO_ZH.items() if zh != en}


def q(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def replace_literal(text: str, old: str, replacement: str) -> str:
    return text.replace(q(old), replacement)


def translate_slint_sources() -> None:
    paths = [
        UI / "app-window.slint",
        UI / "capture-overlay.slint",
        UI / "editor-surface.slint",
        UI / "hotkey-settings.slint",
        UI / "overlay-controls.slint",
        UI / "settings-view.slint",
        UI / "tray.slint",
    ]
    for path in paths:
        text = path.read_text(encoding="utf-8")
        for zh, en in sorted(ZH_TO_EN.items(), key=lambda pair: len(pair[0]), reverse=True):
            text = replace_literal(text, zh, f"@tr({q(en)})")
        for en in sorted(EN_TO_ZH, key=len, reverse=True):
            literal = q(en)
            text = re.sub(rf"(?<!@tr\(){re.escape(literal)}", f"@tr({literal})", text)
        path.write_text(text, encoding="utf-8")

    leftovers: list[str] = []
    for path in paths:
        for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for literal in re.findall(r'"(?:\\.|[^"\\])*"', line):
                value = literal[1:-1]
                if re.search(r"[\u3400-\u9fff]", value):
                    leftovers.append(f"{path.relative_to(ROOT)}:{line_no}: {value}")
    if leftovers:
        raise SystemExit("untranslated CJK literals remain:\n" + "\n".join(leftovers))


def patch_design_system() -> None:
    path = UI / "design-system.slint"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        "    out property <color> success: #22c55e;\n",
        "    out property <color> success: #22c55e;\n    out property <color> warning: #f59e0b;\n",
    )
    if "export component FeedbackToast" not in text:
        text += r'''

export component FeedbackToast inherits Rectangle {
    in-out property <string> text;
    in property <string> level: "info";
    in property <duration> duration: 3000ms;

    height: 40px;
    visible: root.text != "";
    border-radius: 12px;
    background: Theme.overlay-panel;
    border-color: root.level == "error" ? Theme.danger
        : root.level == "warning" ? Theme.warning
        : root.level == "success" ? Theme.success
        : Theme.overlay-panel-border;
    border-width: 1px;

    Rectangle {
        x: 10px;
        y: 10px;
        width: 4px;
        height: 20px;
        border-radius: 2px;
        background: root.level == "error" ? Theme.danger
            : root.level == "warning" ? Theme.warning
            : root.level == "success" ? Theme.success
            : Theme.accent;
    }
    Text {
        x: 24px;
        width: parent.width - 36px;
        text: root.text;
        color: Theme.text;
        font-size: 11px;
        overflow: elide;
        vertical-alignment: center;
    }
    Timer {
        interval: root.duration;
        running: root.text != "";
        triggered => { root.text = ""; }
    }
}
'''
    path.write_text(text, encoding="utf-8")


def patch_app_window() -> None:
    path = UI / "app-window.slint"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        'import { Theme } from "design-system.slint";',
        'import { Theme, FeedbackToast } from "design-system.slint";',
    )
    text = text.replace(
        '    in-out property <string> status-text: @tr("Ready");\n',
        '    in-out property <string> status-text: @tr("Ready");\n'
        '    in-out property <string> status-level: "info";\n'
        '    in-out property <duration> status-duration: 3000ms;\n'
        '    in-out property <string> language-mode: "system";\n',
    )
    text = text.replace(
        '    callback export-behavior-change-requested(bool, bool, bool, bool);\n',
        '    callback export-behavior-change-requested(bool, bool, bool, bool);\n'
        '    callback language-change-requested(string);\n',
    )
    text = text.replace(
        '        export-close-after-save: root.export-close-after-save;\n',
        '        export-close-after-save: root.export-close-after-save;\n'
        '        language-mode: root.language-mode;\n',
    )
    text = text.replace(
        '        export-behavior-change-requested(remember-last, copy-after-capture, close-after-copy, close-after-save) => {\n',
        '        language-change-requested(mode) => { root.language-change-requested(mode); }\n'
        '        export-behavior-change-requested(remember-last, copy-after-capture, close-after-copy, close-after-save) => {\n',
    )
    marker = '        model-delete-requested(id) => { root.ocr-model-delete-requested(id); }\n    }\n}'
    replacement = '''        model-delete-requested(id) => { root.ocr-model-delete-requested(id); }
    }

    FeedbackToast {
        x: max(12px, (root.width - self.width) / 2);
        y: root.height - 60px;
        width: min(520px, root.width - 24px);
        text <=> root.status-text;
        level: root.status-level;
        duration: root.status-duration;
    }
}'''
    if marker not in text:
        raise SystemExit("app-window toast insertion marker not found")
    text = text.replace(marker, replacement)
    path.write_text(text, encoding="utf-8")


def patch_settings_view() -> None:
    path = UI / "settings-view.slint"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        '    in property <bool> export-close-after-save: true;\n',
        '    in property <bool> export-close-after-save: true;\n    in property <string> language-mode: "system";\n',
    )
    text = text.replace(
        '    callback export-behavior-change-requested(bool, bool, bool, bool);\n',
        '    callback export-behavior-change-requested(bool, bool, bool, bool);\n    callback language-change-requested(string);\n',
    )
    marker = '''                Rectangle { height: 1px; background: Theme.border; }
                Text { text: @tr("Desktop workflow"); color: Theme.text; font-size: 15px; font-weight: 650; }
'''
    language_section = '''                Text { text: @tr("Language"); color: Theme.text; font-size: 15px; font-weight: 650; }
                Text { text: @tr("Choose the language used by Azusa Lens."); color: Theme.text-muted; font-size: 11px; }
                HorizontalLayout {
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
                }

                Rectangle { height: 1px; background: Theme.border; }
                Text { text: @tr("Desktop workflow"); color: Theme.text; font-size: 15px; font-weight: 650; }
'''
    if marker not in text:
        raise SystemExit("settings language insertion marker not found")
    text = text.replace(marker, language_section)
    path.write_text(text, encoding="utf-8")


def patch_editor_surface() -> None:
    path = UI / "editor-surface.slint"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        'import { Theme, OverlayIconButton, SurfaceButton, ToolButton, ColorChip, StrokeChip, SelectionHandle } from "design-system.slint";',
        'import { Theme, FeedbackToast, OverlayIconButton, SurfaceButton, ToolButton, ColorChip, StrokeChip, SelectionHandle } from "design-system.slint";',
    )
    text = text.replace(
        '    in-out property <string> status-text;\n',
        '    in-out property <string> status-text;\n    in property <string> status-level: "info";\n    in property <duration> status-duration: 3000ms;\n',
    )
    text = re.sub(
        r'\n    status-toast-timer := Timer \{\n        interval: 2200ms;\n        running: root\.status-text != "";\n        triggered => \{ root\.status-text = ""; \}\n    \}\n',
        '\n',
        text,
    )
    old_toast = '''    if root.status-text != "": Rectangle {
        x: max(12px, (root.width - self.width) / 2);
        y: root.overlay-mode ? max(12px, root.height - 78px) : root.height - 58px;
        width: min(520px, root.width - 24px); height: 36px;
        border-radius: 18px; background: Theme.overlay-panel; border-color: Theme.overlay-panel-border; border-width: 1px;
        Text {
            x: 14px; width: parent.width - 28px;
            text: root.status-text; color: Theme.text; font-size: 10px;
            overflow: elide; horizontal-alignment: center; vertical-alignment: center;
        }
    }
'''
    new_toast = '''    FeedbackToast {
        x: max(12px, (root.width - self.width) / 2);
        y: root.overlay-mode ? max(12px, root.height - 82px) : root.height - 62px;
        width: min(520px, root.width - 24px);
        text <=> root.status-text;
        level: root.status-level;
        duration: root.status-duration;
    }
'''
    if old_toast not in text:
        raise SystemExit("editor toast marker not found")
    text = text.replace(old_toast, new_toast)
    path.write_text(text, encoding="utf-8")


def patch_capture_overlay() -> None:
    path = UI / "capture-overlay.slint"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        '    in-out property <string> status-text: "";\n',
        '    in-out property <string> status-text: "";\n    in-out property <string> status-level: "info";\n    in-out property <duration> status-duration: 3000ms;\n',
    )
    text = text.replace(
        '        status-text <=> root.status-text;\n',
        '        status-text <=> root.status-text;\n        status-level: root.status-level;\n        status-duration: root.status-duration;\n',
    )
    path.write_text(text, encoding="utf-8")


def patch_build_and_cargo() -> None:
    (DESKTOP / "build.rs").write_text(
        '''fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("translations")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config)
        .expect("failed to compile Slint UI");
}
''',
        encoding="utf-8",
    )
    cargo = DESKTOP / "Cargo.toml"
    text = cargo.read_text(encoding="utf-8")
    if 'sys-locale = "=0.3.2"' not in text:
        text = text.replace('notify-rust.workspace = true\n', 'notify-rust.workspace = true\nsys-locale = "=0.3.2"\n')
    cargo.write_text(text, encoding="utf-8")


def patch_config() -> None:
    path = ROOT / "crates" / "config" / "src" / "lib.rs"
    text = path.read_text(encoding="utf-8")
    text = text.replace('pub const SETTINGS_SCHEMA_VERSION: u32 = 3;', 'pub const SETTINGS_SCHEMA_VERSION: u32 = 4;')
    appearance_end = '''impl AppearanceMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}
'''
    language = appearance_end + '''
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageMode {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
}

impl LanguageMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "en" => Some(Self::English),
            "zh-CN" | "zh_CN" | "zh" => Some(Self::SimplifiedChinese),
            _ => None,
        }
    }
}
'''
    if 'pub enum LanguageMode' not in text:
        if appearance_end not in text:
            raise SystemExit("appearance block not found")
        text = text.replace(appearance_end, language)
    text = text.replace(
        '    pub appearance: AppearanceMode,\n',
        '    pub appearance: AppearanceMode,\n    pub language: LanguageMode,\n',
    )
    text = text.replace(
        '            appearance: AppearanceMode::System,\n',
        '            appearance: AppearanceMode::System,\n            language: LanguageMode::System,\n',
    )
    text = text.replace(
        '            appearance: AppearanceMode::Dark,\n            ocr: OcrSettings {',
        '            appearance: AppearanceMode::Dark,\n            language: LanguageMode::SimplifiedChinese,\n            ocr: OcrSettings {',
        1,
    )
    migration_marker = '''    #[test]
    fn missing_fields_receive_current_defaults() {
'''
    migration_test = '''    #[test]
    fn v3_settings_receive_language_default_and_upgrade_in_memory() {
        let (root, store) = test_store("v3-migration");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            store.path(),
            r#"{"schema_version":3,"appearance":"dark","export":{"copy_after_capture":false}}"#,
        )
        .unwrap();

        let settings = store.load().unwrap();
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.language, LanguageMode::System);
        assert_eq!(settings.appearance, AppearanceMode::Dark);
        assert!(!settings.export.copy_after_capture);
        let _ = fs::remove_dir_all(root);
    }

'''
    if 'v3_settings_receive_language_default' not in text:
        text = text.replace(migration_marker, migration_test + migration_marker)
    path.write_text(text, encoding="utf-8")


def patch_rust_runtime() -> None:
    main = ROOT / "apps" / "desktop" / "src" / "main.rs"
    text = main.read_text(encoding="utf-8")
    text = text.replace('mod editor;\nmod hotkeys;\n', 'mod editor;\nmod feedback;\nmod hotkeys;\nmod i18n;\n')
    text = text.replace(
        'use azusa_config::{AppSettings, AppearanceMode, SettingsStore};',
        'use azusa_config::{AppSettings, AppearanceMode, LanguageMode, SettingsStore};',
    )
    text = text.replace('use notify_rust::{Notification, Timeout};\n', '')
    text = text.replace(
        '    let loaded_settings = settings_store.load_or_default();\n',
        '    let loaded_settings = settings_store.load_or_default();\n'
        '    if let Err(error) = i18n::apply_language(loaded_settings.settings.language) {\n'
        '        eprintln!("Could not apply configured language: {error}");\n'
        '    }\n',
    )
    # Route all UI status writes through the central feedback channel.
    text = text.replace('ui.set_status_text(', 'feedback::set_status_text(&ui, ')
    # Overlay timer propagation is already localized and must not create a second feedback event.
    text = text.replace(
        'feedback::set_status_text(&ui, overlay.get_status_text());',
        'ui.set_status_text(overlay.get_status_text());',
    )
    text = re.sub(r'\s*show_system_notification\("Image copied", "Edited image copied to clipboard"\);', '', text)
    text = re.sub(r'\s*show_system_notification\("Image saved", &format!\("Saved · \{\}", path\.display\(\)\)\);', '', text)
    text = re.sub(
        r'\nfn show_system_notification\(summary: &str, body: &str\) \{.*?\n\}\n\nfn schedule_capture_exit',
        '\nfn schedule_capture_exit',
        text,
        flags=re.S,
    )
    text = text.replace(
        '    overlay.set_status_text(ui.get_status_text());\n',
        '    overlay.set_status_text(ui.get_status_text());\n'
        '    overlay.set_status_level(ui.get_status_level());\n'
        '    overlay.set_status_duration(ui.get_status_duration());\n',
    )
    text = text.replace(
        'fn sync_export_settings_ui(ui: &AppWindow, settings: &AppSettings) {\n    let export = &settings.export;\n',
        'fn sync_export_settings_ui(ui: &AppWindow, settings: &AppSettings) {\n'
        '    ui.set_language_mode(settings.language.as_str().into());\n'
        '    let export = &settings.export;\n',
    )

    appearance_callback_end = '''        });
    }

    {
        let weak = ui.as_weak();
        let settings_store = settings_store.clone();
        let app_settings = Rc::clone(&app_settings);
        ui.on_export_directory_requested(move || {
'''
    language_callback = '''        });
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
                Err(error) => feedback::set_status_text(
                    &ui,
                    format!("Could not save language · {error}").into(),
                ),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let settings_store = settings_store.clone();
        let app_settings = Rc::clone(&app_settings);
        ui.on_export_directory_requested(move || {
'''
    if appearance_callback_end not in text:
        raise SystemExit("language callback insertion marker not found")
    text = text.replace(appearance_callback_end, language_callback, 1)
    main.write_text(text, encoding="utf-8")

    hotkeys = ROOT / "apps" / "desktop" / "src" / "hotkeys.rs"
    text = hotkeys.read_text(encoding="utf-8")
    text = text.replace('use crate::AppWindow;', 'use crate::{AppWindow, feedback};')
    text = text.replace('ui.set_status_text(', 'feedback::set_status_text(&ui, ')
    hotkeys.write_text(text, encoding="utf-8")


def patch_dynamic_i18n() -> None:
    path = ROOT / "apps" / "desktop" / "src" / "i18n.rs"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        '    if let Some(value) = localized_suffix(raw, "Unsupported appearance mode · ", "不支持的外观模式 · ") {\n        return value;\n    }\n',
        '    if let Some(value) = localized_suffix(raw, "Unsupported appearance mode · ", "不支持的外观模式 · ") {\n'
        '        return value;\n'
        '    }\n'
        '    if let Some(value) = localized_suffix(raw, "Unsupported language mode · ", "不支持的语言模式 · ") {\n'
        '        return value;\n'
        '    }\n'
        '    if let Some(value) = localized_suffix(raw, "Could not apply language · ", "无法应用语言设置 · ") {\n'
        '        return value;\n'
        '    }\n'
        '    if let Some(value) = localized_suffix(raw, "Could not save language · ", "无法保存语言设置 · ") {\n'
        '        return value;\n'
        '    }\n',
    )
    text = text.replace(
        '        "Export directory unchanged" => "快速保存目录未更改",\n',
        '        "Language saved" => "语言设置已保存",\n'
        '        "Export directory unchanged" => "快速保存目录未更改",\n',
    )
    path.write_text(text, encoding="utf-8")


def write_translations() -> None:
    out = DESKTOP / "translations" / "zh-CN" / "LC_MESSAGES" / "azusa-lens-desktop.po"
    out.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        'msgid ""',
        'msgstr ""',
        '"Content-Type: text/plain; charset=UTF-8\\n"',
        '"Language: zh-CN\\n"',
        '',
    ]
    for english, chinese in sorted(EN_TO_ZH.items()):
        lines.append(f"msgid {q(english)}")
        lines.append(f"msgstr {q(chinese)}")
        lines.append("")
    out.write_text("\n".join(lines), encoding="utf-8")


def main() -> None:
    translate_slint_sources()
    patch_design_system()
    patch_app_window()
    patch_settings_view()
    patch_editor_surface()
    patch_capture_overlay()
    patch_build_and_cargo()
    patch_config()
    patch_rust_runtime()
    patch_dynamic_i18n()
    write_translations()
    print("applied i18n and feedback integration")


if __name__ == "__main__":
    main()
