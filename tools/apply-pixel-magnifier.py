from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    if old not in text:
        raise SystemExit(f"expected snippet not found in {path}")
    path.write_text(text.replace(old, new, 1))


overlay = Path("apps/desktop/ui/capture-overlay.slint")

replace_once(
    overlay,
    '''    private property <float> drag-selection-width: 0;\n    private property <float> drag-selection-height: 0;\n\n    function selection-hit-mode(x: float, y: float) -> string {\n''',
    '''    private property <float> drag-selection-width: 0;\n    private property <float> drag-selection-height: 0;\n    private property <float> pointer-x: 0;\n    private property <float> pointer-y: 0;\n    private property <int> magnifier-source-size: 17;\n    private property <int> pointer-source-x: max(0, min(\n        max(0, root.screenshot.width - 1),\n        round(root.pointer-x * root.screenshot.width / max(1, root.width / 1px))\n    ));\n    private property <int> pointer-source-y: max(0, min(\n        max(0, root.screenshot.height - 1),\n        round(root.pointer-y * root.screenshot.height / max(1, root.height / 1px))\n    ));\n    private property <int> magnifier-source-x: max(0, min(\n        max(0, root.screenshot.width - root.magnifier-source-size),\n        root.pointer-source-x - 8\n    ));\n    private property <int> magnifier-source-y: max(0, min(\n        max(0, root.screenshot.height - root.magnifier-source-size),\n        root.pointer-source-y - 8\n    ));\n    private property <float> magnifier-cross-x: (root.pointer-source-x - root.magnifier-source-x + 0.5) * 8;\n    private property <float> magnifier-cross-y: (root.pointer-source-y - root.magnifier-source-y + 0.5) * 8;\n    private property <bool> magnifier-visible: !root.editor-visible\n        && capture-area.pressed\n        && root.selection-drag-mode != "move"\n        && root.screenshot.width >= root.magnifier-source-size\n        && root.screenshot.height >= root.magnifier-source-size;\n\n    function selection-hit-mode(x: float, y: float) -> string {\n''',
)

replace_once(
    overlay,
    '''        pointer-event(event) => {\n            if (event.button == PointerEventButton.left && event.kind == PointerEventKind.down) {\n''',
    '''        pointer-event(event) => {\n            root.pointer-x = self.mouse-x / 1px;\n            root.pointer-y = self.mouse-y / 1px;\n            if (event.button == PointerEventButton.left && event.kind == PointerEventKind.down) {\n''',
)

replace_once(
    overlay,
    '''        moved => {\n            if (self.pressed) {\n''',
    '''        moved => {\n            root.pointer-x = self.mouse-x / 1px;\n            root.pointer-y = self.mouse-y / 1px;\n            if (self.pressed) {\n''',
)

replace_once(
    overlay,
    '''    Rectangle {\n        visible: !root.editor-visible;\n        x: (parent.width - self.width) / 2;\n''',
    '''    if root.magnifier-visible: Rectangle {\n        x: root.pointer-x * 1px + 24px + self.width <= root.width\n            ? root.pointer-x * 1px + 24px\n            : max(8px, root.pointer-x * 1px - self.width - 24px);\n        y: root.pointer-y * 1px + 24px + self.height <= root.height\n            ? root.pointer-y * 1px + 24px\n            : max(8px, root.pointer-y * 1px - self.height - 24px);\n        width: 152px;\n        height: 176px;\n        border-radius: 12px;\n        background: Theme.overlay-panel;\n        border-color: Theme.overlay-panel-border;\n        border-width: 1px;\n\n        Rectangle {\n            x: 8px; y: 8px; width: 136px; height: 136px;\n            clip: true;\n            background: Theme.canvas;\n\n            Image {\n                source: root.screenshot;\n                width: parent.width;\n                height: parent.height;\n                source-clip-x: root.magnifier-source-x;\n                source-clip-y: root.magnifier-source-y;\n                source-clip-width: root.magnifier-source-size;\n                source-clip-height: root.magnifier-source-size;\n                image-fit: fill;\n                image-rendering: pixelated;\n                accessible-role: none;\n            }\n\n            Rectangle {\n                x: root.magnifier-cross-x * 1px - 0.5px;\n                y: 0px;\n                width: 1px;\n                height: parent.height;\n                background: Theme.accent;\n            }\n            Rectangle {\n                x: 0px;\n                y: root.magnifier-cross-y * 1px - 0.5px;\n                width: parent.width;\n                height: 1px;\n                background: Theme.accent;\n            }\n            Rectangle {\n                x: root.magnifier-cross-x * 1px - 4px;\n                y: root.magnifier-cross-y * 1px - 4px;\n                width: 8px;\n                height: 8px;\n                background: transparent;\n                border-color: Theme.accent;\n                border-width: 1px;\n            }\n        }\n\n        Text {\n            x: 8px; y: 148px; width: 136px; height: 20px;\n            text: "X " + root.pointer-source-x + "  Y " + root.pointer-source-y\n                + (root.has-selection ? "  ·  " + round(root.selection-width) + " × " + round(root.selection-height) : "");\n            color: Theme.overlay-text;\n            font-size: 10px;\n            horizontal-alignment: center;\n            vertical-alignment: center;\n        }\n    }\n\n    Rectangle {\n        visible: !root.editor-visible;\n        x: (parent.width - self.width) / 2;\n''',
)

tests = Path("apps/desktop/src/overlay_tests.rs")
replace_once(
    tests,
    '''fn text_model() -> ModelRc<OcrOverlayItem> {\n''',
    '''#[test]\nfn capture_overlay_magnifier_uses_physical_pixels_without_obscuring_move() {\n    let source = include_str!("../ui/capture-overlay.slint");\n\n    for expected in [\n        "source-clip-x: root.magnifier-source-x",\n        "source-clip-y: root.magnifier-source-y",\n        "image-rendering: pixelated",\n        "root.screenshot.width / max(1, root.width / 1px)",\n        "root.screenshot.height / max(1, root.height / 1px)",\n        "root.selection-drag-mode != \\\"move\\\"",\n        "root.pointer-source-x - root.magnifier-source-x + 0.5",\n        "root.pointer-source-y - root.magnifier-source-y + 0.5",\n    ] {\n        assert!(source.contains(expected), "missing magnifier behavior: {expected}");\n    }\n    assert!(source.contains("private property <int> magnifier-source-size: 17"));\n    assert!(source.contains("width: 136px; height: 136px"));\n}\n\nfn text_model() -> ModelRc<OcrOverlayItem> {\n''',
)
