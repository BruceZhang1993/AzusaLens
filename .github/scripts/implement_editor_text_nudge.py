from pathlib import Path
import re


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


def sub_once(text: str, pattern: str, replacement: str, label: str, flags: int = 0) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=flags)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return updated


# --- EditorSession ---------------------------------------------------------
editor_path = Path("apps/desktop/src/editor.rs")
editor = editor_path.read_text()

editor = replace_once(
    editor,
    'const HIT_TOLERANCE_PX: f32 = 7.0;\n',
    'const HIT_TOLERANCE_PX: f32 = 7.0;\nconst TEXT_EDIT_DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(400);\n',
    "double click interval",
)
editor = replace_once(
    editor,
    '''pub enum BeginResult {
    Drawing,
    TextInput,
    Ignored,
}
''',
    '''pub enum BeginResult {
    Drawing,
    TextInput,
    TextEdit,
    Ignored,
}
''',
    "begin result",
)
editor = replace_once(
    editor,
    '''    pen_points: Vec<Point>,
    pending_text_origin: Option<Point>,
    tool: ActiveTool,
''',
    '''    pen_points: Vec<Point>,
    pending_text_origin: Option<Point>,
    pending_text_edit_index: Option<usize>,
    tool: ActiveTool,
''',
    "pending text edit field",
)
editor = replace_once(
    editor,
    '''    selection_drag: Option<SelectionDrag>,
    selection_preview: Option<Annotation>,
}
''',
    '''    selection_drag: Option<SelectionDrag>,
    selection_preview: Option<Annotation>,
    last_select_click: Option<(usize, Instant)>,
}
''',
    "last select click field",
)
editor = replace_once(
    editor,
    '''            pen_points: Vec::new(),
            pending_text_origin: None,
            tool: ActiveTool::Annotation(ToolKind::Rectangle),
''',
    '''            pen_points: Vec::new(),
            pending_text_origin: None,
            pending_text_edit_index: None,
            tool: ActiveTool::Annotation(ToolKind::Rectangle),
''',
    "pending text edit default",
)
editor = replace_once(
    editor,
    '''            selected_index: None,
            selection_drag: None,
            selection_preview: None,
        }
''',
    '''            selected_index: None,
            selection_drag: None,
            selection_preview: None,
            last_select_click: None,
        }
''',
    "last click default",
)

# Preserve the previous click only for a select-tool pointer-down; every other
# interaction goes through cancel_draft() and breaks a double-click sequence.
old_begin_prefix = '''    pub fn begin_canvas(
        &mut self,
        x: f32,
        y: f32,
        canvas_width: f32,
        canvas_height: f32,
    ) -> BeginResult {
        let Some(point) = self.canvas_to_image(x, y, canvas_width, canvas_height) else {
            return BeginResult::Ignored;
        };

        self.cancel_draft();
        if self.tool == ActiveTool::Select {
            return self.begin_selection(point, canvas_width, canvas_height);
        }
'''
new_begin_prefix = '''    pub fn begin_canvas(
        &mut self,
        x: f32,
        y: f32,
        canvas_width: f32,
        canvas_height: f32,
    ) -> BeginResult {
        let Some(point) = self.canvas_to_image(x, y, canvas_width, canvas_height) else {
            self.last_select_click = None;
            return BeginResult::Ignored;
        };

        let previous_select_click = self.last_select_click.take();
        self.cancel_draft();
        if self.tool == ActiveTool::Select {
            self.last_select_click = previous_select_click;
            return self.begin_selection(point, canvas_width, canvas_height);
        }
'''
editor = replace_once(editor, old_begin_prefix, new_begin_prefix, "begin canvas click preservation")

old_commit_text = '''    pub fn commit_text(&mut self, value: &str) -> Result<Option<CapturedFrame>, String> {
        let Some(origin) = self.pending_text_origin.take() else {
            return Ok(None);
        };
        let annotation = Annotation::Text {
            origin,
            value: value.to_owned(),
            style: self.style,
        };
        if !annotation.is_meaningful() {
            return Ok(None);
        }
        self.apply_annotation(annotation)?;
        self.current_frame().map(Some)
    }
'''
new_commit_text = '''    pub fn commit_text(&mut self, value: &str) -> Result<Option<CapturedFrame>, String> {
        self.last_select_click = None;
        if let Some(index) = self.pending_text_edit_index.take() {
            self.pending_text_origin = None;
            let Some(current) = self.document.items().get(index).cloned() else {
                self.selected_index = None;
                return Ok(None);
            };
            let Annotation::Text {
                origin,
                style,
                ..
            } = current
            else {
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
            self.replace_annotation(index, replacement)?;
            self.selected_index = Some(index);
            return self.current_frame().map(Some);
        }

        let Some(origin) = self.pending_text_origin.take() else {
            return Ok(None);
        };
        let annotation = Annotation::Text {
            origin,
            value: value.to_owned(),
            style: self.style,
        };
        if !annotation.is_meaningful() {
            return Ok(None);
        }
        self.apply_annotation(annotation)?;
        self.current_frame().map(Some)
    }

    #[must_use]
    pub fn pending_text_value(&self) -> Option<&str> {
        let index = self.pending_text_edit_index?;
        match self.document.items().get(index)? {
            Annotation::Text { value, style, .. }
                if style.stroke_width != SEQUENCE_SENTINEL_STROKE =>
            {
                Some(value.as_str())
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn is_editing_text(&self) -> bool {
        self.pending_text_edit_index.is_some()
    }
'''
editor = replace_once(editor, old_commit_text, new_commit_text, "commit text editing")

# Insert nudge after delete_selected so it shares the same selected-index model.
old_delete_tail = '''        self.recompute_sequence_next();
        self.current_frame().map(Some)
    }

    pub fn undo(&mut self) -> Result<Option<CapturedFrame>, String> {
'''
new_delete_tail = '''        self.recompute_sequence_next();
        self.current_frame().map(Some)
    }

    pub fn nudge_selected(
        &mut self,
        dx: f32,
        dy: f32,
    ) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        if dx == 0.0 && dy == 0.0 {
            return Ok(None);
        }
        let Some(index) = self.selected_index else {
            return Ok(None);
        };
        let Some(current) = self.document.items().get(index).cloned() else {
            self.selected_index = None;
            return Ok(None);
        };
        let replacement = translate_annotation(&current, dx, dy);
        self.replace_annotation(index, replacement)?;
        self.current_frame().map(Some)
    }

    pub fn undo(&mut self) -> Result<Option<CapturedFrame>, String> {
'''
editor = replace_once(editor, old_delete_tail, new_delete_tail, "nudge selected")

old_begin_selection = r'''    fn begin_selection\(
        &mut self,
        point: Point,
        canvas_width: f32,
        canvas_height: f32,
    \) -> BeginResult \{.*?\n    \}\n\n    fn move_selection'''
new_begin_selection = '''    fn begin_selection(
        &mut self,
        point: Point,
        canvas_width: f32,
        canvas_height: f32,
    ) -> BeginResult {
        let handle_tolerance =
            self.image_tolerance(canvas_width, canvas_height, HANDLE_TOLERANCE_PX);
        if let Some(index) = self.selected_index
            && let Some(annotation) = self.document.items().get(index).cloned()
        {
            let bounds = annotation_bounds(&annotation);
            if let Some(handle) = resize_handle_at(bounds, point, handle_tolerance) {
                self.last_select_click = None;
                self.selection_preview = Some(annotation.clone());
                self.selection_drag = Some(SelectionDrag::Resize {
                    index,
                    handle,
                    original: annotation,
                });
                return BeginResult::Drawing;
            }
        }

        let hit_tolerance = self.image_tolerance(canvas_width, canvas_height, HIT_TOLERANCE_PX);
        let previous_selection = self.selected_index;
        self.selected_index = self.hit_test(point, hit_tolerance);
        let Some(index) = self.selected_index else {
            self.last_select_click = None;
            return BeginResult::Drawing;
        };
        let Some(annotation) = self.document.items().get(index).cloned() else {
            self.selected_index = None;
            self.last_select_click = None;
            return BeginResult::Ignored;
        };

        let is_double_click = previous_selection == Some(index)
            && is_editable_text_annotation(&annotation)
            && self.last_select_click.as_ref().is_some_and(|(last_index, last)| {
                *last_index == index && last.elapsed() <= TEXT_EDIT_DOUBLE_CLICK_INTERVAL
            });
        if is_double_click {
            self.last_select_click = None;
            self.pending_text_edit_index = Some(index);
            self.selection_preview = None;
            self.selection_drag = None;
            return BeginResult::TextEdit;
        }

        self.last_select_click = Some((index, Instant::now()));
        self.selection_preview = Some(annotation.clone());
        self.selection_drag = Some(SelectionDrag::Move {
            index,
            start: point,
            original: annotation,
        });
        BeginResult::Drawing
    }

    fn move_selection'''
editor = sub_once(editor, old_begin_selection, new_begin_selection, "begin selection double click", re.S)

# A real drag is not part of a click sequence.
editor = replace_once(
    editor,
    '''        if replacement != original {
            self.replace_annotation(index, replacement)?;
        }
        self.current_frame().map(Some)
''',
    '''        if replacement != original {
            self.last_select_click = None;
            self.replace_annotation(index, replacement)?;
        }
        self.current_frame().map(Some)
''',
    "selection drag breaks double click",
)

# cancel_draft is the common boundary for keyboard/document/tool interactions.
editor = replace_once(
    editor,
    '''        self.pen_points.clear();
        self.pending_text_origin = None;
        self.selection_drag = None;
''',
    '''        self.pen_points.clear();
        self.pending_text_origin = None;
        self.pending_text_edit_index = None;
        self.last_select_click = None;
        self.selection_drag = None;
''',
    "cancel pending text edit",
)

# Helper next to the existing sequence helper keeps the sentinel rule centralized.
editor = replace_once(
    editor,
    '''fn selection_drag_index(drag: &SelectionDrag) -> usize {
''',
    '''fn is_editable_text_annotation(annotation: &Annotation) -> bool {
    matches!(
        annotation,
        Annotation::Text { style, .. } if style.stroke_width != SEQUENCE_SENTINEL_STROKE
    )
}

fn selection_drag_index(drag: &SelectionDrag) -> usize {
''',
    "editable text helper",
)

# Unit coverage: double-click edit, sequence exclusion, and precise nudge.
test_anchor = '''    #[test]
    fn selection_move_is_one_undoable_transaction() {
'''
test_insert = '''    #[test]
    fn regular_text_double_click_edits_in_place_and_is_undoable() {
        let mut editor = EditorSession::default();
        editor.reset(frame(200, 100));
        assert!(editor.set_tool("text"));
        assert_eq!(
            editor.begin_canvas(20.0, 20.0, 200.0, 100.0),
            BeginResult::TextInput
        );
        editor
            .commit_text("before")
            .expect("text creation should render");

        assert!(editor.set_tool(SELECT_TOOL_ID));
        assert_eq!(
            editor.begin_canvas(20.0, 20.0, 200.0, 100.0),
            BeginResult::Drawing
        );
        editor
            .end_canvas(20.0, 20.0, 200.0, 100.0)
            .expect("first selection click should finish");
        assert_eq!(
            editor.begin_canvas(20.0, 20.0, 200.0, 100.0),
            BeginResult::TextEdit
        );
        assert_eq!(editor.pending_text_value(), Some("before"));
        assert!(editor.is_editing_text());

        editor
            .commit_text("after")
            .expect("text edit should render");
        match &editor.document.items()[0] {
            Annotation::Text { value, .. } => assert_eq!(value, "after"),
            _ => panic!("expected text annotation"),
        }
        assert!(!editor.is_editing_text());

        editor.undo().expect("text edit should undo");
        match &editor.document.items()[0] {
            Annotation::Text { value, .. } => assert_eq!(value, "before"),
            _ => panic!("expected text annotation"),
        }
    }

    #[test]
    fn sequence_markers_do_not_enter_text_edit_mode() {
        let mut editor = EditorSession::default();
        editor.reset(frame(100, 50));
        place_number(&mut editor, 100.0, 50.0);
        assert!(editor.set_tool(SELECT_TOOL_ID));
        assert_eq!(
            editor.begin_canvas(50.0, 25.0, 100.0, 50.0),
            BeginResult::Drawing
        );
        editor
            .end_canvas(50.0, 25.0, 100.0, 50.0)
            .expect("first sequence click should finish");
        assert_eq!(
            editor.begin_canvas(50.0, 25.0, 100.0, 50.0),
            BeginResult::Drawing
        );
        assert!(!editor.is_editing_text());
    }

    #[test]
    fn selected_annotation_nudges_in_image_pixels_and_undoes() {
        let mut editor = EditorSession::default();
        editor.reset(frame(100, 100));
        place_rectangle(&mut editor, Point::new(10.0, 10.0), Point::new(30.0, 30.0));
        assert!(editor.set_tool(SELECT_TOOL_ID));
        editor.begin_canvas(20.0, 20.0, 100.0, 100.0);
        editor.end_canvas(20.0, 20.0, 100.0, 100.0).unwrap();

        editor
            .nudge_selected(1.0, -1.0)
            .expect("one-pixel nudge should render");
        let bounds = annotation_bounds(&editor.document.items()[0]);
        assert_eq!(bounds.origin, Point::new(11.0, 9.0));
        assert_eq!(editor.selection_bounds().unwrap().x, 11.0);

        editor.undo().expect("nudge should undo");
        let bounds = annotation_bounds(&editor.document.items()[0]);
        assert_eq!(bounds.origin, Point::new(10.0, 10.0));

        assert!(editor.set_tool(SELECT_TOOL_ID));
        editor.begin_canvas(20.0, 20.0, 100.0, 100.0);
        editor.end_canvas(20.0, 20.0, 100.0, 100.0).unwrap();
        editor
            .nudge_selected(10.0, 0.0)
            .expect("ten-pixel nudge should render");
        let bounds = annotation_bounds(&editor.document.items()[0]);
        assert_eq!(bounds.origin, Point::new(20.0, 10.0));
    }

    #[test]
    fn selection_move_is_one_undoable_transaction() {
'''
editor = replace_once(editor, test_anchor, test_insert, "editor interaction tests")
editor_path.write_text(editor)


# --- Slint callback surface ------------------------------------------------
app_path = Path("apps/desktop/ui/app-window.slint")
app = app_path.read_text()
app = replace_once(
    app,
    '''    callback text-submit(string);
    callback undo-requested();
''',
    '''    callback text-submit(string);
    callback nudge-selection-requested(float, float);
    callback undo-requested();
''',
    "app nudge callback",
)
app_path.write_text(app)

overlay_path = Path("apps/desktop/ui/capture-overlay.slint")
overlay = overlay_path.read_text()
overlay = replace_once(
    overlay,
    '''    callback text-submit(string);
    callback undo-requested();
''',
    '''    callback text-submit(string);
    callback nudge-selection-requested(float, float);
    callback undo-requested();
''',
    "overlay nudge callback",
)

old_key_tail = '''        capture-key-pressed(event) => {
            if (!root.editor-visible && root.has-selection && event.text == Key.Return) {
                root.commit-selection();
                accept
            } else if (event.text == Key.Escape) {
'''
new_key_tail = '''        capture-key-pressed(event) => {
            if (!root.editor-visible && root.has-selection && event.text == Key.Return) {
                root.commit-selection();
                accept
            } else if (root.editor-visible
                && !root.text-entry-visible
                && root.editor-has-selection
                && !event.modifiers.control
                && !event.modifiers.alt
                && (event.text == Key.LeftArrow
                    || event.text == Key.RightArrow
                    || event.text == Key.UpArrow
                    || event.text == Key.DownArrow)) {
                let step = event.modifiers.shift ? 10 : 1;
                if (event.text == Key.LeftArrow) {
                    root.nudge-selection-requested(-step, 0);
                } else if (event.text == Key.RightArrow) {
                    root.nudge-selection-requested(step, 0);
                } else if (event.text == Key.UpArrow) {
                    root.nudge-selection-requested(0, -step);
                } else {
                    root.nudge-selection-requested(0, step);
                }
                accept
            } else if (event.text == Key.Escape) {
'''
overlay = replace_once(overlay, old_key_tail, new_key_tail, "overlay keyboard nudge")
overlay_path.write_text(overlay)


# --- Rust UI bridge --------------------------------------------------------
main_path = Path("apps/desktop/src/main.rs")
main = main_path.read_text()

# Every overlay-originating action that invokes a hidden AppWindow callback must
# copy the resulting editor state back before returning to the visible overlay.
old_macro = '''        macro_rules! forward_overlay_no_args {
            ($on:ident, $invoke:ident) => {{
                let weak = ui.as_weak();
                overlay.$on(move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.$invoke();
                    }
                });
            }};
        }
'''
new_macro = '''        macro_rules! forward_overlay_no_args {
            ($on:ident, $invoke:ident) => {{
                let ui_weak = ui.as_weak();
                let overlay_weak = overlay.as_weak();
                overlay.$on(move || {
                    if let (Some(ui), Some(overlay)) =
                        (ui_weak.upgrade(), overlay_weak.upgrade())
                    {
                        ui.$invoke();
                        sync_editor_overlay(&ui, &overlay);
                    }
                });
            }};
        }
'''
main = replace_once(main, old_macro, new_macro, "overlay no-arg state bridge")

# Replace the simple one/two-argument forwarders with state-synchronizing forms.
def sync_forward(method, invoke, args, pre=""):
    nonlocal_placeholder = None

forward_specs = [
    (
        '''        {
            let weak = ui.as_weak();
            overlay.on_tool_selected(move |tool| {
                if let Some(ui) = weak.upgrade() {
                    ui.set_active_tool(tool.clone());
                    ui.invoke_tool_selected(tool);
                }
            });
        }
''',
        '''        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_tool_selected(move |tool| {
                if let (Some(ui), Some(overlay)) =
                    (ui_weak.upgrade(), overlay_weak.upgrade())
                {
                    ui.set_active_tool(tool.clone());
                    ui.invoke_tool_selected(tool);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }
''',
        "tool forward sync",
    ),
    (
        '''        {
            let weak = ui.as_weak();
            overlay.on_color_selected(move |index| {
                if let Some(ui) = weak.upgrade() {
                    ui.set_active_color(index);
                    ui.invoke_color_selected(index);
                }
            });
        }
''',
        '''        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_color_selected(move |index| {
                if let (Some(ui), Some(overlay)) =
                    (ui_weak.upgrade(), overlay_weak.upgrade())
                {
                    ui.set_active_color(index);
                    ui.invoke_color_selected(index);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }
''',
        "color forward sync",
    ),
    (
        '''        {
            let weak = ui.as_weak();
            overlay.on_stroke_selected(move |width| {
                if let Some(ui) = weak.upgrade() {
                    ui.set_active_stroke(width);
                    ui.invoke_stroke_selected(width);
                }
            });
        }
''',
        '''        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_stroke_selected(move |width| {
                if let (Some(ui), Some(overlay)) =
                    (ui_weak.upgrade(), overlay_weak.upgrade())
                {
                    ui.set_active_stroke(width);
                    ui.invoke_stroke_selected(width);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }
''',
        "stroke forward sync",
    ),
]
for old, new, label in forward_specs:
    main = replace_once(main, old, new, label)

for callback, invoke in [
    ("editor_pointer_down", "editor_pointer_down"),
    ("editor_pointer_moved", "editor_pointer_moved"),
    ("editor_pointer_up", "editor_pointer_up"),
]:
    old = f'''        {{
            let weak = ui.as_weak();
            overlay.on_{callback}(move |x, y, width, height| {{
                if let Some(ui) = weak.upgrade() {{
                    ui.invoke_{invoke}(x, y, width, height);
                }}
            }});
        }}
'''
    new = f'''        {{
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_{callback}(move |x, y, width, height| {{
                if let (Some(ui), Some(overlay)) =
                    (ui_weak.upgrade(), overlay_weak.upgrade())
                {{
                    ui.invoke_{invoke}(x, y, width, height);
                    sync_editor_overlay(&ui, &overlay);
                }}
            }});
        }}
'''
    main = replace_once(main, old, new, f"{callback} forward sync")

main = replace_once(
    main,
    '''        {
            let weak = ui.as_weak();
            overlay.on_text_submit(move |value| {
                if let Some(ui) = weak.upgrade() {
                    ui.set_pending_text(value.clone());
                    ui.invoke_text_submit(value);
                }
            });
        }
''',
    '''        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_text_submit(move |value| {
                if let (Some(ui), Some(overlay)) =
                    (ui_weak.upgrade(), overlay_weak.upgrade())
                {
                    ui.set_pending_text(value.clone());
                    ui.invoke_text_submit(value);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_nudge_selection_requested(move |dx, dy| {
                if let (Some(ui), Some(overlay)) =
                    (ui_weak.upgrade(), overlay_weak.upgrade())
                {
                    ui.invoke_nudge_selection_requested(dx, dy);
                    sync_editor_overlay(&ui, &overlay);
                }
            });
        }
''',
    "text and nudge forwarding",
)

for callback, invoke in [
    ("copy_ocr_selection_requested", "copy_ocr_selection_requested"),
    ("ocr_selection_to_text_requested", "ocr_selection_to_text_requested"),
]:
    old = f'''        {{
            let weak = ui.as_weak();
            overlay.on_{callback}(move |anchor, focus| {{
                if let Some(ui) = weak.upgrade() {{
                    ui.invoke_{invoke}(anchor, focus);
                }}
            }});
        }}
'''
    new = f'''        {{
            let ui_weak = ui.as_weak();
            let overlay_weak = overlay.as_weak();
            overlay.on_{callback}(move |anchor, focus| {{
                if let (Some(ui), Some(overlay)) =
                    (ui_weak.upgrade(), overlay_weak.upgrade())
                {{
                    ui.invoke_{invoke}(anchor, focus);
                    sync_editor_overlay(&ui, &overlay);
                }}
            }});
        }}
'''
    main = replace_once(main, old, new, f"{callback} sync")

# Main editor callback: distinguish new text from re-edit and pre-fill existing value.
old_pointer_down_handler = '''            match editor.borrow_mut().begin_canvas(x, y, width, height) {
                BeginResult::TextInput => {
                    ui.set_pending_text("".into());
                    ui.set_text_entry_visible(true);
                    feedback::set_status_text(
                        &ui,
                        "Text anchor placed · type text and choose Add text".into(),
                    );
                }
                BeginResult::Drawing => ui.set_text_entry_visible(false),
                BeginResult::Ignored => {}
            }
            sync_selection(&ui, &editor.borrow());
'''
new_pointer_down_handler = '''            let begin_result = editor.borrow_mut().begin_canvas(x, y, width, height);
            match begin_result {
                BeginResult::TextInput => {
                    ui.set_pending_text("".into());
                    ui.set_text_entry_visible(true);
                    feedback::set_status_text(
                        &ui,
                        "Text anchor placed · type text and press Enter".into(),
                    );
                }
                BeginResult::TextEdit => {
                    let value = editor
                        .borrow()
                        .pending_text_value()
                        .unwrap_or_default()
                        .to_owned();
                    ui.set_pending_text(value.into());
                    ui.set_text_entry_visible(true);
                    feedback::set_status_text(
                        &ui,
                        "Editing text annotation · Enter to apply · Esc to cancel".into(),
                    );
                }
                BeginResult::Drawing => ui.set_text_entry_visible(false),
                BeginResult::Ignored => {}
            }
            sync_selection(&ui, &editor.borrow());
'''
main = replace_once(main, old_pointer_down_handler, new_pointer_down_handler, "text edit pointer down")

# Text entry owns pointer-up after it becomes visible; don't let select end_canvas
# overwrite the edit status or perform an unnecessary full-frame copy.
main = replace_once(
    main,
    '''            let was_select = editor.borrow().is_select_tool();
            let result = editor.borrow_mut().end_canvas(x, y, width, height);
''',
    '''            if ui.get_text_entry_visible() {
                sync_selection(&ui, &editor.borrow());
                return;
            }
            let was_select = editor.borrow().is_select_tool();
            let result = editor.borrow_mut().end_canvas(x, y, width, height);
''',
    "skip pointer up while text entry visible",
)

old_text_submit_start = '''            let result = editor.borrow_mut().commit_text(value.as_str());
            match result {
'''
new_text_submit_start = '''            let was_editing = editor.borrow().is_editing_text();
            let result = editor.borrow_mut().commit_text(value.as_str());
            match result {
'''
main = replace_once(main, old_text_submit_start, new_text_submit_start, "text submit edit state")
main = replace_once(
    main,
    '''                    feedback::set_status_text(&ui, "Text annotation added".into());
''',
    '''                    feedback::set_status_text(
                        &ui,
                        if was_editing {
                            "Text annotation updated".into()
                        } else {
                            "Text annotation added".into()
                        },
                    );
''',
    "text edit feedback",
)

# Keyboard nudge handler sits alongside delete/undo and intentionally does not
# emit a toast for every key repeat.
nudge_anchor = '''    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_delete_selection_requested(move || {
'''
nudge_block = '''    {
        let weak = ui.as_weak();
        let latest_frame = Rc::clone(&latest_frame);
        let editor = Rc::clone(&editor);
        ui.on_nudge_selection_requested(move |dx, dy| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            match editor.borrow_mut().nudge_selected(dx, dy) {
                Ok(Some(frame)) => {
                    set_editor_frame(&ui, &latest_frame, frame);
                    sync_history(&ui, &editor.borrow());
                    sync_selection(&ui, &editor.borrow());
                }
                Ok(None) => {}
                Err(error) => feedback::set_status_text(
                    &ui,
                    format!("Move selected annotation failed · {error}").into(),
                ),
            }
        });
    }

'''
main = replace_once(main, nudge_anchor, nudge_block + nudge_anchor, "nudge callback handler")
main_path.write_text(main)


# --- Source-contract tests for the UI bridge and key mapping ---------------
overlay_tests_path = Path("apps/desktop/src/overlay_tests.rs")
tests = overlay_tests_path.read_text()
test_anchor = '''#[test]
fn capture_overlay_magnifier_uses_physical_pixels_without_obscuring_move() {
'''
test_block = '''#[test]
fn editor_overlay_declares_precise_keyboard_nudge_contract() {
    let overlay = include_str!("../ui/capture-overlay.slint");
    let main = include_str!("main.rs");

    for key in ["Key.LeftArrow", "Key.RightArrow", "Key.UpArrow", "Key.DownArrow"] {
        assert!(overlay.contains(key), "missing keyboard nudge key: {key}");
    }
    assert!(overlay.contains("let step = event.modifiers.shift ? 10 : 1;"));
    assert!(overlay.contains("root.nudge-selection-requested(-step, 0);"));
    assert!(overlay.contains("root.nudge-selection-requested(0, step);"));
    assert!(
        main.matches("sync_editor_overlay(&ui, &overlay);").count() >= 8,
        "overlay-originating editor actions must synchronize hidden AppWindow state back to the visible overlay"
    );
}

#[test]
fn capture_overlay_magnifier_uses_physical_pixels_without_obscuring_move() {
'''
tests = replace_once(tests, test_anchor, test_block, "overlay nudge contract test")
overlay_tests_path.write_text(tests)
