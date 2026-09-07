from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def save(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = load(path)
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:100]!r}")
    save(path, text.replace(old, new, 1))


def sub_once(path: str, pattern: str, replacement: str, flags: int = 0) -> None:
    text = load(path)
    updated, count = re.subn(pattern, replacement, text, count=1, flags=flags)
    if count != 1:
        raise RuntimeError(f"{path}: regex expected one match, found {count}: {pattern[:100]!r}")
    save(path, updated)


# ---------------------------------------------------------------------------
# Export settings: keep the persisted schema backward-compatible, but make
# default_directory the only directory that affects quick save. Legacy
# last_directory fields can still deserialize existing v2/v3 settings.
# ---------------------------------------------------------------------------
replace_once(
    "crates/config/src/lib.rs",
    '''impl ExportSettings {
    #[must_use]
    pub fn dialog_directory(&self) -> Option<&Path> {
        if self.remember_last_directory {
            self.last_directory
                .as_deref()
                .or(self.default_directory.as_deref())
        } else {
            self.default_directory.as_deref()
        }
    }
}
''',
    '''impl ExportSettings {
    #[must_use]
    pub fn dialog_directory(&self) -> Option<&Path> {
        if self.remember_last_directory {
            self.last_directory
                .as_deref()
                .or(self.default_directory.as_deref())
        } else {
            self.default_directory.as_deref()
        }
    }

    #[must_use]
    pub fn quick_save_directory(&self) -> PathBuf {
        self.default_directory
            .clone()
            .unwrap_or_else(default_screenshot_directory)
    }
}

fn default_screenshot_directory() -> PathBuf {
    dirs::picture_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Pictures")))
        .unwrap_or_else(std::env::temp_dir)
        .join("Screenshots")
}
''',
)

# ---------------------------------------------------------------------------
# Save-path helpers: local wall-clock filenames and collision-safe quick save.
# ---------------------------------------------------------------------------
replace_once(
    "crates/capture/src/dialogs.rs",
    '''use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
''',
    '''use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
''',
)
replace_once(
    "crates/capture/src/dialogs.rs",
    '''#[must_use]
pub fn suggested_png_name() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    suggested_png_name_at(seconds)
}
''',
    '''#[must_use]
pub fn suggested_png_name() -> String {
    local_timestamp()
        .map(|timestamp| format!("{timestamp}.png"))
        .unwrap_or_else(|| {
            let seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            suggested_png_name_at(seconds)
        })
}
''',
)
replace_once(
    "crates/capture/src/dialogs.rs",
    '''pub fn choose_directory(initial_directory: Option<&Path>) -> Result<Option<PathBuf>, String> {
    choose_directory_impl(initial_directory)
}
''',
    '''pub fn choose_directory(initial_directory: Option<&Path>) -> Result<Option<PathBuf>, String> {
    choose_directory_impl(initial_directory)
}

pub fn quick_png_save_path(directory: &Path, suggested_name: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("could not create screenshot directory: {error}"))?;
    let initial = validate_png_extension(directory.join(suggested_name))?;
    if !initial.exists() {
        return Ok(initial);
    }

    let stem = initial
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "screenshot filename is not valid UTF-8".to_owned())?;
    for suffix in 2..=9_999_u32 {
        let candidate = directory.join(format!("{stem}_{suffix}.png"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("could not allocate a unique screenshot filename".to_owned())
}
''',
)
replace_once(
    "crates/capture/src/dialogs.rs",
    '''fn suggested_png_name_at(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_date_from_unix_days(days);
    format!("AzusaLens_{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}Z.png")
}
''',
    '''#[cfg(any(target_os = "linux", target_os = "macos"))]
fn local_timestamp() -> Option<String> {
    use std::process::Command;

    let output = Command::new("date")
        .arg("+%Y-%m-%d_%H-%M-%S")
        .output()
        .ok()?;
    command_timestamp(output)
}

#[cfg(target_os = "windows")]
fn local_timestamp() -> Option<String> {
    use std::process::Command;

    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-Date -Format 'yyyy-MM-dd_HH-mm-ss'",
        ])
        .output()
        .ok()?;
    command_timestamp(output)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn local_timestamp() -> Option<String> {
    None
}

fn command_timestamp(output: std::process::Output) -> Option<String> {
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn suggested_png_name_at(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_date_from_unix_days(days);
    format!("{year:04}-{month:02}-{day:02}_{hour:02}-{minute:02}-{second:02}.png")
}
''',
)
replace_once(
    "crates/capture/src/dialogs.rs",
    '''    #[test]
    fn suggested_name_is_stable_for_unix_epoch() {
        assert_eq!(suggested_png_name_at(0), "AzusaLens_19700101_000000Z.png");
    }

    #[test]
    fn suggested_name_handles_leap_day() {
        // 2024-02-29 12:34:56 UTC.
        assert_eq!(
            suggested_png_name_at(1_709_210_096),
            "AzusaLens_20240229_123456Z.png"
        );
    }
''',
    '''    #[test]
    fn suggested_name_is_stable_for_unix_epoch() {
        assert_eq!(suggested_png_name_at(0), "1970-01-01_00-00-00.png");
    }

    #[test]
    fn suggested_name_handles_leap_day() {
        // 2024-02-29 12:34:56 UTC fallback value.
        assert_eq!(
            suggested_png_name_at(1_709_210_096),
            "2024-02-29_12-34-56.png"
        );
    }
''',
)
replace_once(
    "crates/capture/src/dialogs.rs",
    '''    #[test]
    fn png_extension_must_be_explicit() {
        assert_eq!(
            validate_png_extension(PathBuf::from("capture.PNG")).unwrap(),
            PathBuf::from("capture.PNG")
        );
        assert!(validate_png_extension(PathBuf::from("capture")).is_err());
        assert!(validate_png_extension(PathBuf::from("capture.jpg")).is_err());
    }
''',
    '''    #[test]
    fn png_extension_must_be_explicit() {
        assert_eq!(
            validate_png_extension(PathBuf::from("capture.PNG")).unwrap(),
            PathBuf::from("capture.PNG")
        );
        assert!(validate_png_extension(PathBuf::from("capture")).is_err());
        assert!(validate_png_extension(PathBuf::from("capture.jpg")).is_err());
    }

    #[test]
    fn quick_save_path_creates_directory_and_avoids_collisions() {
        let directory = std::env::temp_dir().join(format!(
            "azusa-lens-quick-save-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let first = quick_png_save_path(&directory, "2026-09-08_12-34-56.png").unwrap();
        assert_eq!(first, directory.join("2026-09-08_12-34-56.png"));
        fs::write(&first, b"png").unwrap();
        let second = quick_png_save_path(&directory, "2026-09-08_12-34-56.png").unwrap();
        assert_eq!(second, directory.join("2026-09-08_12-34-56_2.png"));
        let _ = fs::remove_dir_all(directory);
    }
''',
)

# ---------------------------------------------------------------------------
# Settings copy: quick save is a direct action; Save As is explicitly separate.
# ---------------------------------------------------------------------------
replace_once(
    "apps/desktop/ui/settings-view.slint",
    'Text { text: "默认保存目录"; color: Theme.text; font-size: 13px; font-weight: 650; }',
    'Text { text: "快速保存目录"; color: Theme.text; font-size: 13px; font-weight: 650; }',
)
replace_once(
    "apps/desktop/ui/settings-view.slint",
    'text: root.export-default-directory == "" ? "使用系统默认位置" : root.export-default-directory;',
    'text: root.export-default-directory == "" ? "系统图片目录 / Screenshots" : root.export-default-directory;',
)
replace_once(
    "apps/desktop/ui/settings-view.slint",
    'Text { text: "保存时始终打开系统 Save As；默认文件名使用 UTC 时间戳。"; color: Theme.text-faint; font-size: 9px; }',
    'Text { text: "保存会直接写入此目录；另存为可单独选择位置。默认文件名使用本地日期时间。"; color: Theme.text-faint; font-size: 9px; }',
)
replace_once(
    "apps/desktop/ui/settings-view.slint",
    '''                ToggleSettingRow {
                    title: "记住上次保存位置";
                    detail: "下次 Save As 优先从最近一次成功保存的目录打开。";
                    checked: root.export-remember-last-directory;
                    toggled(value) => {
                        root.export-behavior-change-requested(
                            value,
                            root.export-copy-after-capture,
                            root.export-close-after-copy,
                            root.export-close-after-save
                        );
                    }
                }

''',
    '',
)
replace_once(
    "apps/desktop/ui/settings-view.slint",
    'detail: "成功保存 PNG 后自动关闭截图 overlay；取消 Save As 不会结束会话。";',
    'detail: "快速保存或另存为成功后自动关闭截图 overlay；取消另存为不会结束会话。";',
)

# ---------------------------------------------------------------------------
# App window exposes the target settings page and a distinct Save As callback.
# ---------------------------------------------------------------------------
replace_once(
    "apps/desktop/ui/app-window.slint",
    '    private property <string> settings-page: "general";\n',
    '    in-out property <string> settings-page: "general";\n',
)
replace_once(
    "apps/desktop/ui/app-window.slint",
    '    callback save-requested();\n',
    '    callback save-requested();\n    callback save-as-requested();\n',
)

# ---------------------------------------------------------------------------
# Editor surface: do not auto-open properties when selecting a drawing tool;
# add Save As to secondary actions and forward it through overlay controls.
# ---------------------------------------------------------------------------
replace_once(
    "apps/desktop/ui/editor-surface.slint",
    '    callback save-requested();\n',
    '    callback save-requested();\n    callback save-as-requested();\n',
)
replace_once(
    "apps/desktop/ui/editor-surface.slint",
    '''    if root.has-capture && !root.overlay-mode && root.more-actions-visible: Rectangle {
        x: root.overlay-mode
            ? max(8px, min(root.width - 224px, editor-canvas.x + editor-canvas.width - 224px))
            : editor-canvas.x + editor-canvas.width - 224px;
        y: root.overlay-mode
            ? max(8px, root.bottom-bar-y - 102px)
            : editor-canvas.y + editor-canvas.height - 100px;
        width: 224px; height: 94px;
        border-radius: 12px; background: Theme.surface; border-color: Theme.border; border-width: 1px;
        VerticalLayout {
            padding: 10px; spacing: 6px;
            SurfaceButton { width: 204px; height: 32px; label: "Copy all OCR text"; enabled: root.ocr-line-count > 0; clicked => { root.more-actions-visible = false; root.copy-ocr-requested(); } }
            SurfaceButton { width: 204px; height: 32px; label: "Clear annotations"; enabled: root.can-undo; clicked => { root.more-actions-visible = false; root.clear-annotations-requested(); } }
        }
    }
''',
    '''    if root.has-capture && !root.overlay-mode && root.more-actions-visible: Rectangle {
        x: root.overlay-mode
            ? max(8px, min(root.width - 224px, editor-canvas.x + editor-canvas.width - 224px))
            : editor-canvas.x + editor-canvas.width - 224px;
        y: root.overlay-mode
            ? max(8px, root.bottom-bar-y - 140px)
            : editor-canvas.y + editor-canvas.height - 138px;
        width: 224px; height: 132px;
        border-radius: 12px; background: Theme.surface; border-color: Theme.border; border-width: 1px;
        VerticalLayout {
            padding: 10px; spacing: 6px;
            SurfaceButton { width: 204px; height: 32px; label: "Save As…"; clicked => { root.more-actions-visible = false; root.save-as-requested(); } }
            SurfaceButton { width: 204px; height: 32px; label: "Copy all OCR text"; enabled: root.ocr-line-count > 0; clicked => { root.more-actions-visible = false; root.copy-ocr-requested(); } }
            SurfaceButton { width: 204px; height: 32px; label: "Clear annotations"; enabled: root.can-undo; clicked => { root.more-actions-visible = false; root.clear-annotations-requested(); } }
        }
    }
''',
)
replace_once(
    "apps/desktop/ui/editor-surface.slint",
    '        save => { root.save-requested(); }\n',
    '        save => { root.save-requested(); }\n        save-as => { root.save-as-requested(); }\n',
)

# ---------------------------------------------------------------------------
# Overlay controls: compact according to the selected canvas too; choosing a
# tool no longer forces the property palette open; Save As lives in More.
# ---------------------------------------------------------------------------
replace_once(
    "apps/desktop/ui/overlay-controls.slint",
    '    callback save();\n',
    '    callback save();\n    callback save-as();\n',
)
replace_once(
    "apps/desktop/ui/overlay-controls.slint",
    '    private property <bool> compact: root.width < 550px;\n',
    '    private property <bool> compact: root.width < 550px || root.canvas-width < 550;\n',
)
replace_once(
    "apps/desktop/ui/overlay-controls.slint",
    '''    function choose(tool: string) {
        root.more-visible = false;
        root.properties-visible = tool != "select";
        root.tool-selected(tool);
    }
''',
    '''    function choose(tool: string) {
        root.more-visible = false;
        root.properties-visible = false;
        root.tool-selected(tool);
    }
''',
)
replace_once(
    "apps/desktop/ui/overlay-controls.slint",
    '''            OverlayIconButton { icon: @image-url("icons/trash-2.svg"); label: "删除所选标注 · Delete"; viewport-width: root.width; viewport-height: root.height; enabled: root.has-selection; disabled-reason: "请先选择标注"; clicked => { root.delete-selection(); } }
            OverlayIconButton { icon: @image-url("icons/x.svg"); label: "清空标注"; viewport-width: root.width; viewport-height: root.height; enabled: root.can-undo; disabled-reason: "没有可清空的标注"; clicked => { root.clear(); root.more-visible = false; } }
''',
    '''            OverlayIconButton { icon: @image-url("icons/trash-2.svg"); label: "删除所选标注 · Delete"; viewport-width: root.width; viewport-height: root.height; enabled: root.has-selection; disabled-reason: "请先选择标注"; clicked => { root.delete-selection(); } }
            OverlayIconButton { icon: @image-url("icons/x.svg"); label: "清空标注"; viewport-width: root.width; viewport-height: root.height; enabled: root.can-undo; disabled-reason: "没有可清空的标注"; clicked => { root.clear(); root.more-visible = false; } }
            OverlayIconButton { icon: @image-url("icons/save.svg"); label: "另存为… · Ctrl+Shift+S"; viewport-width: root.width; viewport-height: root.height; clicked => { root.more-visible = false; root.save-as(); } }
''',
)

# ---------------------------------------------------------------------------
# Capture overlay: selection remains editable after mouse-up. The existing
# corner handles now resize it, dragging inside moves it, Enter/check confirms.
# ---------------------------------------------------------------------------
replace_once(
    "apps/desktop/ui/capture-overlay.slint",
    '    callback save-requested();\n',
    '    callback save-requested();\n    callback save-as-requested();\n',
)
replace_once(
    "apps/desktop/ui/capture-overlay.slint",
    '''    private property <bool> has-selection: root.selection-width >= 2 && root.selection-height >= 2;

    callback selection-confirmed(float, float, float, float);
''',
    '''    private property <bool> has-selection: root.selection-width >= 2 && root.selection-height >= 2;
    private property <string> selection-drag-mode: "new";
    private property <float> drag-origin-x: 0;
    private property <float> drag-origin-y: 0;
    private property <float> drag-selection-x: 0;
    private property <float> drag-selection-y: 0;
    private property <float> drag-selection-width: 0;
    private property <float> drag-selection-height: 0;

    private function selection-hit-mode(x: float, y: float) -> string {
        if (!root.has-selection) { return "new"; }
        let threshold = 10;
        let left = root.selection-x;
        let top = root.selection-y;
        let right = left + root.selection-width;
        let bottom = top + root.selection-height;
        let near-left = abs(x - left) <= threshold;
        let near-right = abs(x - right) <= threshold;
        let near-top = abs(y - top) <= threshold;
        let near-bottom = abs(y - bottom) <= threshold;
        if (near-left && near-top) { return "nw"; }
        if (near-right && near-top) { return "ne"; }
        if (near-left && near-bottom) { return "sw"; }
        if (near-right && near-bottom) { return "se"; }
        if (x >= left && x <= right && y >= top && y <= bottom) { return "move"; }
        return "new";
    }

    private function update-adjusted-selection(x: float, y: float) {
        let right = root.drag-selection-x + root.drag-selection-width;
        let bottom = root.drag-selection-y + root.drag-selection-height;
        if (root.selection-drag-mode == "move") {
            let next-x = max(0, min(root.width / 1px - root.drag-selection-width,
                root.drag-selection-x + x - root.drag-origin-x));
            let next-y = max(0, min(root.height / 1px - root.drag-selection-height,
                root.drag-selection-y + y - root.drag-origin-y));
            root.start-x = next-x;
            root.start-y = next-y;
            root.end-x = next-x + root.drag-selection-width;
            root.end-y = next-y + root.drag-selection-height;
        } else if (root.selection-drag-mode == "nw") {
            root.start-x = max(0, min(right - 2, x));
            root.start-y = max(0, min(bottom - 2, y));
            root.end-x = right;
            root.end-y = bottom;
        } else if (root.selection-drag-mode == "ne") {
            root.start-x = root.drag-selection-x;
            root.start-y = max(0, min(bottom - 2, y));
            root.end-x = min(root.width / 1px, max(root.drag-selection-x + 2, x));
            root.end-y = bottom;
        } else if (root.selection-drag-mode == "sw") {
            root.start-x = max(0, min(right - 2, x));
            root.start-y = root.drag-selection-y;
            root.end-x = right;
            root.end-y = min(root.height / 1px, max(root.drag-selection-y + 2, y));
        } else if (root.selection-drag-mode == "se") {
            root.start-x = root.drag-selection-x;
            root.start-y = root.drag-selection-y;
            root.end-x = min(root.width / 1px, max(root.drag-selection-x + 2, x));
            root.end-y = min(root.height / 1px, max(root.drag-selection-y + 2, y));
        }
    }

    callback selection-confirmed(float, float, float, float);
''',
)
sub_once(
    "apps/desktop/ui/capture-overlay.slint",
    r'''    capture-area := TouchArea \{\n        visible: !root\.editor-visible;\n        mouse-cursor: crosshair;\n        pointer-event\(event\) => \{.*?\n        \}\n        moved => \{.*?\n        \}\n    \}\n''',
    '''    capture-area := TouchArea {
        visible: !root.editor-visible;
        mouse-cursor: crosshair;
        pointer-event(event) => {
            if (event.button == PointerEventButton.left && event.kind == PointerEventKind.down) {
                root.selection-drag-mode = root.selection-hit-mode(self.mouse-x / 1px, self.mouse-y / 1px);
                root.drag-origin-x = self.mouse-x / 1px;
                root.drag-origin-y = self.mouse-y / 1px;
                root.drag-selection-x = root.selection-x;
                root.drag-selection-y = root.selection-y;
                root.drag-selection-width = root.selection-width;
                root.drag-selection-height = root.selection-height;
                if (root.selection-drag-mode == "new") {
                    root.start-x = self.mouse-x / 1px;
                    root.start-y = self.mouse-y / 1px;
                    root.end-x = root.start-x;
                    root.end-y = root.start-y;
                } else {
                    root.start-x = root.drag-selection-x;
                    root.start-y = root.drag-selection-y;
                    root.end-x = root.drag-selection-x + root.drag-selection-width;
                    root.end-y = root.drag-selection-y + root.drag-selection-height;
                }
            }
            if (event.button == PointerEventButton.left && event.kind == PointerEventKind.up) {
                if (root.selection-drag-mode == "new") {
                    root.end-x = self.mouse-x / 1px;
                    root.end-y = self.mouse-y / 1px;
                } else {
                    root.update-adjusted-selection(self.mouse-x / 1px, self.mouse-y / 1px);
                }
            }
        }
        moved => {
            if (self.pressed) {
                if (root.selection-drag-mode == "new") {
                    root.end-x = self.mouse-x / 1px;
                    root.end-y = self.mouse-y / 1px;
                } else {
                    root.update-adjusted-selection(self.mouse-x / 1px, self.mouse-y / 1px);
                }
            }
        }
    }
''',
    flags=re.S,
)
sub_once(
    "apps/desktop/ui/capture-overlay.slint",
    r'''    Rectangle \{\n        visible: !root\.editor-visible;\n        x: \(parent\.width - self\.width\) / 2;\n        y: 16px; width: 270px; height: 44px;.*?\n    \}\n\n    editor := EditorSurface''',
    '''    Rectangle {
        visible: !root.editor-visible;
        x: (parent.width - self.width) / 2;
        y: 16px; width: root.has-selection ? 310px : 270px; height: 44px;
        border-radius: 10px; background: Theme.overlay-panel;
        border-color: Theme.overlay-panel-border; border-width: 1px;
        TouchArea { }
        HorizontalLayout {
            padding: 6px; spacing: 8px;
            Text { text: root.has-selection ? "调整截图范围" : "拖动选择截图范围"; color: Theme.overlay-text-muted; font-size: 12px; vertical-alignment: center; }
            if root.has-selection: OverlayIconButton {
                icon: @image-url("icons/check-check.svg"); label: "确认截图 · Enter";
                viewport-width: root.width; viewport-height: root.height;
                primary: true;
                clicked => { root.selection-confirmed(root.selection-x, root.selection-y, root.selection-width, root.selection-height); }
            }
            OverlayIconButton {
                icon: @image-url("icons/maximize.svg"); label: "截取全屏";
                viewport-width: root.width; viewport-height: root.height;
                clicked => {
                    root.start-x = 0; root.start-y = 0;
                    root.end-x = root.width / 1px; root.end-y = root.height / 1px;
                    root.selection-confirmed(0, 0, root.end-x, root.end-y);
                }
            }
            OverlayIconButton {
                icon: @image-url("icons/x.svg"); label: "取消截图 · Esc";
                viewport-width: root.width; viewport-height: root.height;
                clicked => { root.cancelled(); }
            }
        }
    }

    editor := EditorSurface''',
    flags=re.S,
)
replace_once(
    "apps/desktop/ui/capture-overlay.slint",
    '        save-requested => { root.save-requested(); }\n',
    '        save-requested => { root.save-requested(); }\n        save-as-requested => { root.save-as-requested(); }\n',
)
replace_once(
    "apps/desktop/ui/capture-overlay.slint",
    '''        KeyBinding {
            keys: @keys(Control + S);
            enabled: root.editor-visible && !root.text-entry-visible && root.has-capture;
            activated => { root.save-requested(); }
        }
''',
    '''        KeyBinding {
            keys: @keys(Control + Shift + S);
            enabled: root.editor-visible && !root.text-entry-visible && root.has-capture;
            activated => { root.save-as-requested(); }
        }
        KeyBinding {
            keys: @keys(Control + S);
            enabled: root.editor-visible && !root.text-entry-visible && root.has-capture;
            activated => { root.save-requested(); }
        }
''',
)
replace_once(
    "apps/desktop/ui/capture-overlay.slint",
    '''        capture-key-pressed(event) => {
            if (event.text == Key.Escape) {
''',
    '''        capture-key-pressed(event) => {
            if (!root.editor-visible && root.has-selection && event.text == Key.Return) {
                root.selection-confirmed(root.selection-x, root.selection-y, root.selection-width, root.selection-height);
                accept
            } else if (event.text == Key.Escape) {
''',
)

# ---------------------------------------------------------------------------
# Desktop wiring: quick Save, explicit Save As, direct OCR-settings redirect.
# ---------------------------------------------------------------------------
replace_once(
    "apps/desktop/src/main.rs",
    '''    dialogs::{choose_directory, choose_png_save_path, suggested_png_name},
''',
    '''    dialogs::{choose_directory, choose_png_save_path, quick_png_save_path, suggested_png_name},
''',
)
replace_once(
    "apps/desktop/src/main.rs",
    '        forward_overlay_no_args!(on_save_requested, invoke_save_requested);\n',
    '        forward_overlay_no_args!(on_save_requested, invoke_save_requested);\n        forward_overlay_no_args!(on_save_as_requested, invoke_save_as_requested);\n',
)
sub_once(
    "apps/desktop/src/main.rs",
    r'''    \{\n        let weak = ui\.as_weak\(\);\n        let overlay_weak = overlay\.as_weak\(\);\n        let latest_frame = Rc::clone\(&latest_frame\);\n        let settings_store = settings_store\.clone\(\);\n        let app_settings = Rc::clone\(&app_settings\);\n        ui\.on_save_requested\(move \|\| \{.*?\n        \}\);\n    \}\n\n    \{\n        let weak = ui\.as_weak\(\);\n        let latest_frame = Rc::clone\(&latest_frame\);\n        let ocr_command_tx = ocr_command_tx\.clone\(\);''',
    '''    {
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
                    ui.set_status_text("Nothing to save · capture a region first".into());
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
                    ui.set_status_text(format!("Quick save failed · {error}").into());
                    return;
                }
            };

            match frame.save_png(&path) {
                Ok(()) => {
                    let notification_body = format!("Saved edited PNG · {}", path.display());
                    show_system_notification("Image saved", &notification_body);
                    ui.set_status_text(format!("Saved · {}", path.display()).into());
                    if close_after_save
                        && let Some(overlay) = overlay_weak.upgrade()
                        && overlay.get_editor_visible()
                    {
                        schedule_capture_exit(&ui, &overlay);
                    }
                }
                Err(error) => ui.set_status_text(format!("Save failed · {error}").into()),
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
                    ui.set_status_text("Nothing to save · capture a region first".into());
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
                    ui.set_status_text("Save As cancelled".into());
                    return;
                }
                Err(error) => {
                    ui.set_status_text(format!("Could not open Save As · {error}").into());
                    return;
                }
            };

            match frame.save_png(&path) {
                Ok(()) => {
                    let notification_body = format!("Saved edited PNG · {}", path.display());
                    show_system_notification("Image saved", &notification_body);
                    ui.set_status_text(format!("Saved As · {}", path.display()).into());
                    if close_after_save
                        && let Some(overlay) = overlay_weak.upgrade()
                        && overlay.get_editor_visible()
                    {
                        schedule_capture_exit(&ui, &overlay);
                    }
                }
                Err(error) => ui.set_status_text(format!("Save As failed · {error}").into()),
            }
        });
    }

    {
        let weak = ui.as_weak();
        let overlay_weak = overlay.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        let ocr_command_tx = ocr_command_tx.clone();''',
    flags=re.S,
)
replace_once(
    "apps/desktop/src/main.rs",
    '''            let Some(model_id) = ocr_model_manager.active_model_id() else {
                ui.set_status_text(
                    "No OCR model is enabled · download and enable one in Settings > OCR models"
                        .into(),
                );
                return;
            };
''',
    '''            let Some(model_id) = ocr_model_manager.active_model_id() else {
                if let Some(overlay) = overlay_weak.upgrade() {
                    overlay.set_editor_visible(false);
                    overlay.set_has_capture(false);
                    let _ = overlay.hide();
                    set_overlay_windowed(&overlay);
                }
                *latest_frame.borrow_mut() = None;
                *editor.borrow_mut() = EditorSession::default();
                ui.set_has_capture(false);
                ui.set_can_undo(false);
                ui.set_can_redo(false);
                ui.set_has_selection(false);
                ui.set_text_entry_visible(false);
                clear_ocr_results(&ui);
                ui.set_settings_page("ocr".into());
                ui.set_status_text("Choose an OCR model to download and enable".into());
                let _ = ui.show();
                return;
            };
''',
)

# ---------------------------------------------------------------------------
# Native overlay QA: selection is now confirmed explicitly.
# ---------------------------------------------------------------------------
replace_once(
    "apps/desktop/src/overlay_tests.rs",
    '''                pointer(&overlay, 180.0, 180.0, true);
                pointer(&overlay, 1000.0, 650.0, false);
                assert!(overlay.get_editor_visible());
''',
    '''                pointer(&overlay, 180.0, 180.0, true);
                pointer(&overlay, 1000.0, 650.0, false);
                assert!(!overlay.get_editor_visible());
                key(
                    &overlay,
                    &slint::SharedString::from(slint::platform::Key::Return),
                    false,
                );
                assert!(overlay.get_editor_visible());
''',
)
replace_once(
    "apps/desktop/src/overlay_tests.rs",
    '''            11 => {
                pointer(&overlay, 380.0, 500.0, true);
                pointer(&overlay, 480.0, 660.0, false);
            }
''',
    '''            11 => {
                pointer(&overlay, 380.0, 500.0, true);
                pointer(&overlay, 480.0, 660.0, false);
                key(
                    &overlay,
                    &slint::SharedString::from(slint::platform::Key::Return),
                    false,
                );
            }
''',
)

print("capture workflow polish applied")
