from pathlib import Path

ui_path = Path("apps/desktop/ui/capture-overlay.slint")
source = ui_path.read_text()
old = '''        let near-left = abs(x - left) <= threshold;
        let near-right = abs(x - right) <= threshold;
        let near-top = abs(y - top) <= threshold;
        let near-bottom = abs(y - bottom) <= threshold;
        let within-horizontal = x >= left - threshold && x <= right + threshold;
        let within-vertical = y >= top - threshold && y <= bottom + threshold;
'''
new = '''        let horizontal-threshold = min(threshold, root.selection-width / 3);
        let vertical-threshold = min(threshold, root.selection-height / 3);
        let near-left = abs(x - left) <= horizontal-threshold;
        let near-right = abs(x - right) <= horizontal-threshold;
        let near-top = abs(y - top) <= vertical-threshold;
        let near-bottom = abs(y - bottom) <= vertical-threshold;
        let within-horizontal = x >= left - horizontal-threshold && x <= right + horizontal-threshold;
        let within-vertical = y >= top - vertical-threshold && y <= bottom + vertical-threshold;
'''
if old not in source:
    raise SystemExit("selection hit-test block not found")
ui_path.write_text(source.replace(old, new, 1))

test_path = Path("apps/desktop/src/overlay_tests.rs")
tests = test_path.read_text()
needle = '''    for cursor in ["ns-resize", "ew-resize", "nwse-resize", "nesw-resize"] {
        assert!(source.contains(cursor), "missing resize cursor {cursor}");
    }
'''
replacement = needle + '''    assert!(
        source.contains("let horizontal-threshold = min(threshold, root.selection-width / 3);"),
        "thin selections must preserve a horizontal move zone"
    );
    assert!(
        source.contains("let vertical-threshold = min(threshold, root.selection-height / 3);"),
        "thin selections must preserve a vertical move zone"
    );
'''
if needle not in tests:
    raise SystemExit("resize regression test insertion point not found")
test_path.write_text(tests.replace(needle, replacement, 1))
