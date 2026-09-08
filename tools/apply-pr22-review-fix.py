from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"missing anchor: {label}")
    return text.replace(old, new, 1)


overlay_path = Path("apps/desktop/ui/capture-overlay.slint")
overlay = overlay_path.read_text()
anchor = '''    private property <int> magnifier-source-size: 17;\n    private property <int> pointer-source-x: max(0, min(\n'''
replacement = '''    private property <int> magnifier-source-size: 17;\n    private property <float> source-scale-x: root.screenshot.width / max(1, root.width / 1px);\n    private property <float> source-scale-y: root.screenshot.height / max(1, root.height / 1px);\n    private property <int> selection-source-left: floor(max(0, root.selection-x) * root.source-scale-x);\n    private property <int> selection-source-top: floor(max(0, root.selection-y) * root.source-scale-y);\n    private property <int> selection-source-right: ceil(max(0, root.selection-x + root.selection-width) * root.source-scale-x);\n    private property <int> selection-source-bottom: ceil(max(0, root.selection-y + root.selection-height) * root.source-scale-y);\n    private property <int> selection-source-width: max(0, root.selection-source-right - root.selection-source-left);\n    private property <int> selection-source-height: max(0, root.selection-source-bottom - root.selection-source-top);\n    private property <int> pointer-source-x: max(0, min(\n'''
overlay = replace_once(overlay, anchor, replacement, "source scale properties")
overlay = overlay.replace(
    'round(root.pointer-x * root.screenshot.width / max(1, root.width / 1px))',
    'round(root.pointer-x * root.source-scale-x)',
    1,
).replace(
    'round(root.pointer-y * root.screenshot.height / max(1, root.height / 1px))',
    'round(root.pointer-y * root.source-scale-y)',
    1,
)
overlay = replace_once(
    overlay,
    '"  ·  " + round(root.selection-width) + " × " + round(root.selection-height)',
    '"  ·  " + root.selection-source-width + " × " + root.selection-source-height',
    "magnifier dimension label",
)
overlay_path.write_text(overlay)

tests_path = Path("apps/desktop/src/overlay_tests.rs")
tests = tests_path.read_text()
old = '''        "root.screenshot.width / max(1, root.width / 1px)",\n        "root.screenshot.height / max(1, root.height / 1px)",\n        "root.selection-drag-mode != \\"move\\"",\n'''
new = '''        "private property <float> source-scale-x: root.screenshot.width / max(1, root.width / 1px)",\n        "private property <float> source-scale-y: root.screenshot.height / max(1, root.height / 1px)",\n        "selection-source-left: floor(max(0, root.selection-x) * root.source-scale-x)",\n        "selection-source-top: floor(max(0, root.selection-y) * root.source-scale-y)",\n        "selection-source-right: ceil(max(0, root.selection-x + root.selection-width) * root.source-scale-x)",\n        "selection-source-bottom: ceil(max(0, root.selection-y + root.selection-height) * root.source-scale-y)",\n        "root.selection-source-width + \\" × \\" + root.selection-source-height",\n        "root.selection-drag-mode != \\"move\\"",\n'''
tests = replace_once(tests, old, new, "magnifier regression assertions")
tests_path.write_text(tests)
