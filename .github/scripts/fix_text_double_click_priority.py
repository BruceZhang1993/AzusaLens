from pathlib import Path

path = Path("apps/desktop/src/editor.rs")
source = path.read_text()
old = '''    fn begin_selection(
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
            && self
                .last_select_click
                .as_ref()
                .is_some_and(|(last_index, last)| {
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
'''
new = '''    fn begin_selection(
        &mut self,
        point: Point,
        canvas_width: f32,
        canvas_height: f32,
    ) -> BeginResult {
        let hit_tolerance = self.image_tolerance(canvas_width, canvas_height, HIT_TOLERANCE_PX);
        let hit_index = self.hit_test(point, hit_tolerance);

        if let Some(index) = hit_index
            && self.selected_index == Some(index)
            && let Some(annotation) = self.document.items().get(index)
            && is_editable_text_annotation(annotation)
            && self
                .last_select_click
                .as_ref()
                .is_some_and(|(last_index, last)| {
                    *last_index == index && last.elapsed() <= TEXT_EDIT_DOUBLE_CLICK_INTERVAL
                })
        {
            self.last_select_click = None;
            self.pending_text_edit_index = Some(index);
            self.selection_preview = None;
            self.selection_drag = None;
            return BeginResult::TextEdit;
        }

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

        self.selected_index = hit_index;
        let Some(index) = self.selected_index else {
            self.last_select_click = None;
            return BeginResult::Drawing;
        };
        let Some(annotation) = self.document.items().get(index).cloned() else {
            self.selected_index = None;
            self.last_select_click = None;
            return BeginResult::Ignored;
        };

        self.last_select_click = Some((index, Instant::now()));
        self.selection_preview = Some(annotation.clone());
        self.selection_drag = Some(SelectionDrag::Move {
            index,
            start: point,
            original: annotation,
        });
        BeginResult::Drawing
    }
'''
if old not in source:
    raise SystemExit("begin_selection block did not match expected source")
path.write_text(source.replace(old, new, 1))
