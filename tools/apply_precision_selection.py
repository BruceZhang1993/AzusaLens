from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


capture = Path("apps/desktop/ui/capture-overlay.slint")
text = capture.read_text(encoding="utf-8")

old = '''        let near-left = abs(x - left) <= threshold;
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

    function update-adjusted-selection(x: float, y: float) {'''
new = '''        let near-left = abs(x - left) <= threshold;
        let near-right = abs(x - right) <= threshold;
        let near-top = abs(y - top) <= threshold;
        let near-bottom = abs(y - bottom) <= threshold;
        let within-horizontal = x >= left - threshold && x <= right + threshold;
        let within-vertical = y >= top - threshold && y <= bottom + threshold;
        if (near-left && near-top) { return "nw"; }
        if (near-right && near-top) { return "ne"; }
        if (near-left && near-bottom) { return "sw"; }
        if (near-right && near-bottom) { return "se"; }
        if (near-top && within-horizontal) { return "n"; }
        if (near-right && within-vertical) { return "e"; }
        if (near-bottom && within-horizontal) { return "s"; }
        if (near-left && within-vertical) { return "w"; }
        if (x >= left && x <= right && y >= top && y <= bottom) { return "move"; }
        return "new";
    }

    function selection-cursor(mode: string) -> MouseCursor {
        if (mode == "move") { return move; }
        if (mode == "n" || mode == "s") { return ns-resize; }
        if (mode == "e" || mode == "w") { return ew-resize; }
        if (mode == "nw" || mode == "se") { return nwse-resize; }
        if (mode == "ne" || mode == "sw") { return nesw-resize; }
        return crosshair;
    }

    function update-adjusted-selection(x: float, y: float) {'''
text = replace_once(text, old, new, "edge hit testing")

old = '''        } else if (root.selection-drag-mode == "se") {
            root.start-x = root.drag-selection-x;
            root.start-y = root.drag-selection-y;
            root.end-x = min(root.width / 1px, max(root.drag-selection-x + 2, x));
            root.end-y = min(root.height / 1px, max(root.drag-selection-y + 2, y));
        }
    }
'''
new = '''        } else if (root.selection-drag-mode == "se") {
            root.start-x = root.drag-selection-x;
            root.start-y = root.drag-selection-y;
            root.end-x = min(root.width / 1px, max(root.drag-selection-x + 2, x));
            root.end-y = min(root.height / 1px, max(root.drag-selection-y + 2, y));
        } else if (root.selection-drag-mode == "n") {
            root.start-x = root.drag-selection-x;
            root.start-y = max(0, min(bottom - 2, y));
            root.end-x = right;
            root.end-y = bottom;
        } else if (root.selection-drag-mode == "e") {
            root.start-x = root.drag-selection-x;
            root.start-y = root.drag-selection-y;
            root.end-x = min(root.width / 1px, max(root.drag-selection-x + 2, x));
            root.end-y = bottom;
        } else if (root.selection-drag-mode == "s") {
            root.start-x = root.drag-selection-x;
            root.start-y = root.drag-selection-y;
            root.end-x = right;
            root.end-y = min(root.height / 1px, max(root.drag-selection-y + 2, y));
        } else if (root.selection-drag-mode == "w") {
            root.start-x = max(0, min(right - 2, x));
            root.start-y = root.drag-selection-y;
            root.end-x = right;
            root.end-y = bottom;
        }
    }
'''
text = replace_once(text, old, new, "edge resize updates")

old = '''        Rectangle { x: -5px; y: -5px; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
        Rectangle { x: parent.width - 5px; y: -5px; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
        Rectangle { x: -5px; y: parent.height - 5px; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
        Rectangle { x: parent.width - 5px; y: parent.height - 5px; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
'''
new = '''        Rectangle { x: -5px; y: -5px; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
        Rectangle { x: (parent.width - 10px) / 2; y: -5px; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
        Rectangle { x: parent.width - 5px; y: -5px; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
        Rectangle { x: parent.width - 5px; y: (parent.height - 10px) / 2; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
        Rectangle { x: parent.width - 5px; y: parent.height - 5px; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
        Rectangle { x: (parent.width - 10px) / 2; y: parent.height - 5px; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
        Rectangle { x: -5px; y: parent.height - 5px; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
        Rectangle { x: -5px; y: (parent.height - 10px) / 2; width: 10px; height: 10px; background: Theme.selection-handle; border-color: Theme.accent; border-width: 2px; }
'''
text = replace_once(text, old, new, "eight handles")

old = '''    capture-area := TouchArea {
        visible: !root.editor-visible;
        mouse-cursor: crosshair;
'''
new = '''    capture-area := TouchArea {
        visible: !root.editor-visible;
        mouse-cursor: root.selection-cursor(
            self.pressed
                ? root.selection-drag-mode
                : root.selection-hit-mode(self.mouse-x / 1px, self.mouse-y / 1px)
        );
'''
text = replace_once(text, old, new, "dynamic resize cursor")

old = '''    if root.has-selection: Rectangle {
        x: max(8px, root.selection-x * 1px + 8px);
        y: root.editor-visible && (root.selection-y + root.selection-height) * 1px + 52px > root.height
            ? min(root.height - 32px, root.selection-y * 1px + 8px)
            : max(8px, root.selection-y * 1px - 30px);
        width: 92px; height: 24px; border-radius: 6px; background: Theme.overlay-panel;
'''
new = '''    if root.has-selection: Rectangle {
        x: max(8px, min(root.width - 108px, root.selection-x * 1px + 8px));
        y: root.selection-y >= 32
            ? root.selection-y * 1px - 30px
            : (root.selection-y + root.selection-height) * 1px + 30px <= root.height
                ? (root.selection-y + root.selection-height) * 1px + 6px
                : max(8px, min(root.height - 32px, root.selection-y * 1px + 8px));
        width: 100px; height: 24px; border-radius: 6px; background: Theme.overlay-panel;
'''
text = replace_once(text, old, new, "dimension label positioning")

capture.write_text(text, encoding="utf-8")


tests = Path("apps/desktop/src/overlay_tests.rs")
text = tests.read_text(encoding="utf-8")
anchor = '''fn text_model() -> ModelRc<OcrOverlayItem> {
'''
insert = '''#[test]
fn capture_overlay_declares_eight_way_resize_interactions() {
    let source = include_str!("../ui/capture-overlay.slint");

    for mode in ["n", "e", "s", "w"] {
        assert!(
            source.contains(&format!("selection-drag-mode == \\\"{mode}\\\"")),
            "missing edge resize branch for {mode}"
        );
    }
    for cursor in ["ns-resize", "ew-resize", "nwse-resize", "nesw-resize"] {
        assert!(source.contains(cursor), "missing resize cursor {cursor}");
    }
    assert_eq!(
        source.matches("background: Theme.selection-handle").count(),
        8,
        "capture selection should render four corner and four edge handles"
    );
}

'''
if text.count(anchor) != 1:
    raise SystemExit("overlay test anchor not found exactly once")
text = text.replace(anchor, insert + anchor, 1)
tests.write_text(text, encoding="utf-8")
