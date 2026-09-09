from pathlib import Path

editor_path = Path("apps/desktop/src/editor.rs")
editor = editor_path.read_text()
old = '''            let Annotation::Text { origin, style, .. } = current else {
                return Ok(None);
            };
            if style.stroke_width == SEQUENCE_SENTINEL_STROKE || value.is_empty() {
                return Ok(None);
            }
            let replacement = Annotation::Text {
                origin,
                value: value.to_owned(),
                style,
            };
            if replacement == current {
                return Ok(None);
            }
'''
new = '''            let Annotation::Text {
                origin,
                value: current_value,
                style,
            } = &current
            else {
                return Ok(None);
            };
            if style.stroke_width == SEQUENCE_SENTINEL_STROKE || value.is_empty() {
                return Ok(None);
            }
            if current_value == value {
                return Ok(None);
            }
            let replacement = Annotation::Text {
                origin: *origin,
                value: value.to_owned(),
                style: *style,
            };
'''
if editor.count(old) != 1:
    raise SystemExit("text edit ownership block mismatch")
editor_path.write_text(editor.replace(old, new, 1))

main_path = Path("apps/desktop/src/main.rs")
main = main_path.read_text()
old = '''            match editor.borrow_mut().nudge_selected(dx, dy) {
                Ok(Some(frame)) => {
'''
new = '''            let result = editor.borrow_mut().nudge_selected(dx, dy);
            match result {
                Ok(Some(frame)) => {
'''
if main.count(old) != 1:
    raise SystemExit("nudge RefCell scope block mismatch")
main_path.write_text(main.replace(old, new, 1))
