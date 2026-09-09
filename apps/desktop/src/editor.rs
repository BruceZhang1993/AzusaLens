use std::time::{Duration, Instant};

use azusa_annotation::{
    Annotation, AnnotationDocument, AnnotationStyle, Color, Point, Rect, ToolKind,
    render_annotation_in_place,
};
use azusa_capture::CapturedFrame;

const MAX_PREVIEW_DIMENSION: u32 = 1600;
const SELECT_TOOL_ID: &str = "select";
const SEQUENCE_TOOL_ID: &str = "number";
const SEQUENCE_SENTINEL_STROKE: f32 = -1.0;
const MIN_SELECTION_EXTENT: f32 = 8.0;
const HANDLE_TOLERANCE_PX: f32 = 6.0;
const HIT_TOLERANCE_PX: f32 = 7.0;
const TEXT_EDIT_DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginResult {
    Drawing,
    TextInput,
    TextEdit,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTool {
    Select,
    Annotation(ToolKind),
    Sequence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeHandle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

#[derive(Debug, Clone)]
enum SelectionDrag {
    Move {
        index: usize,
        start: Point,
        original: Annotation,
    },
    Resize {
        index: usize,
        handle: ResizeHandle,
        original: Annotation,
    },
}

struct PreviewSurface {
    width: u32,
    height: u32,
    scale_x: f32,
    scale_y: f32,
    base_rgba: Vec<u8>,
    committed_rgba: Vec<u8>,
}

impl PreviewSurface {
    fn new(frame: &CapturedFrame) -> Self {
        let max_dimension = frame.width().max(frame.height());
        let scale = (MAX_PREVIEW_DIMENSION as f32 / max_dimension as f32).min(1.0);
        let width = ((frame.width() as f32 * scale).round() as u32).max(1);
        let height = ((frame.height() as f32 * scale).round() as u32).max(1);
        let base_rgba =
            resize_rgba_nearest(frame.rgba(), frame.width(), frame.height(), width, height);

        Self {
            width,
            height,
            scale_x: width as f32 / frame.width() as f32,
            scale_y: height as f32 / frame.height() as f32,
            committed_rgba: base_rgba.clone(),
            base_rgba,
        }
    }

    fn apply(&mut self, annotation: &Annotation) -> Result<(), String> {
        let scaled = scale_annotation(annotation, self.scale_x, self.scale_y);
        render_editor_annotation_in_place(
            &mut self.committed_rgba,
            self.width,
            self.height,
            &scaled,
        )
    }

    fn rebuild(&mut self, document: &AnnotationDocument) -> Result<(), String> {
        self.committed_rgba.clone_from(&self.base_rgba);
        for annotation in document.items() {
            self.apply(annotation)?;
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.committed_rgba.clone_from(&self.base_rgba);
    }

    fn frame_with_draft(&self, draft: Option<&Annotation>) -> Result<CapturedFrame, String> {
        let mut pixels = self.committed_rgba.clone();
        if let Some(draft) = draft {
            let scaled = scale_annotation(draft, self.scale_x, self.scale_y);
            render_editor_annotation_in_place(&mut pixels, self.width, self.height, &scaled)?;
        }
        CapturedFrame::new(self.width, self.height, pixels).map_err(|error| error.to_string())
    }

    fn frame_with_override(
        &self,
        document: &AnnotationDocument,
        index: usize,
        replacement: &Annotation,
    ) -> Result<CapturedFrame, String> {
        let mut pixels = self.base_rgba.clone();
        for (item_index, annotation) in document.items().iter().enumerate() {
            let annotation = if item_index == index {
                replacement
            } else {
                annotation
            };
            let scaled = scale_annotation(annotation, self.scale_x, self.scale_y);
            render_editor_annotation_in_place(&mut pixels, self.width, self.height, &scaled)?;
        }
        CapturedFrame::new(self.width, self.height, pixels).map_err(|error| error.to_string())
    }
}

pub struct EditorSession {
    base: Option<CapturedFrame>,
    document: AnnotationDocument,
    committed_rgba: Option<Vec<u8>>,
    preview: Option<PreviewSurface>,
    draft: Option<Annotation>,
    drag_start: Option<Point>,
    pen_points: Vec<Point>,
    pending_text_origin: Option<Point>,
    pending_text_edit_index: Option<usize>,
    tool: ActiveTool,
    style: AnnotationStyle,
    sequence_next: u32,
    last_preview_at: Option<Instant>,
    selected_index: Option<usize>,
    selection_drag: Option<SelectionDrag>,
    selection_preview: Option<Annotation>,
    last_select_click: Option<(usize, Instant)>,
}

impl Default for EditorSession {
    fn default() -> Self {
        Self {
            base: None,
            document: AnnotationDocument::default(),
            committed_rgba: None,
            preview: None,
            draft: None,
            drag_start: None,
            pen_points: Vec::new(),
            pending_text_origin: None,
            pending_text_edit_index: None,
            tool: ActiveTool::Annotation(ToolKind::Rectangle),
            style: AnnotationStyle::default(),
            sequence_next: 1,
            last_preview_at: None,
            selected_index: None,
            selection_drag: None,
            selection_preview: None,
            last_select_click: None,
        }
    }
}

impl EditorSession {
    pub fn reset(&mut self, frame: CapturedFrame) {
        self.preview = Some(PreviewSurface::new(&frame));
        self.committed_rgba = Some(frame.rgba().to_vec());
        self.base = Some(frame);
        self.document.reset();
        self.sequence_next = 1;
        self.selected_index = None;
        self.cancel_draft();
    }

    pub fn set_tool(&mut self, id: &str) -> bool {
        let tool = if id == SELECT_TOOL_ID {
            ActiveTool::Select
        } else if id == SEQUENCE_TOOL_ID {
            ActiveTool::Sequence
        } else {
            let Some(tool) = ToolKind::from_id(id) else {
                return false;
            };
            ActiveTool::Annotation(tool)
        };
        self.tool = tool;
        self.cancel_draft();
        if self.tool != ActiveTool::Select {
            self.selected_index = None;
        }
        true
    }

    #[must_use]
    pub fn is_select_tool(&self) -> bool {
        self.tool == ActiveTool::Select
    }

    pub fn set_color(&mut self, index: i32) -> Result<Option<CapturedFrame>, String> {
        let color = color_from_index(index);
        self.style.color = color;

        let Some(selected_index) = self.selected_index else {
            return Ok(None);
        };
        let Some(current) = self.document.items().get(selected_index).cloned() else {
            self.selected_index = None;
            return Ok(None);
        };
        let updated = annotation_with_color(&current, color);
        if updated == current {
            return Ok(None);
        }
        self.replace_annotation(selected_index, updated)?;
        self.current_frame().map(Some)
    }

    pub fn set_stroke_width(&mut self, width: f32) -> Result<Option<CapturedFrame>, String> {
        self.style.stroke_width = width.clamp(1.0, 24.0);
        self.style.font_size = (20.0 + self.style.stroke_width * 2.0).clamp(22.0, 56.0);

        let Some(selected_index) = self.selected_index else {
            return Ok(None);
        };
        let Some(current) = self.document.items().get(selected_index).cloned() else {
            self.selected_index = None;
            return Ok(None);
        };
        let updated = annotation_with_size(&current, self.style.stroke_width, self.style.font_size);
        if updated == current {
            return Ok(None);
        }
        self.replace_annotation(selected_index, updated)?;
        self.current_frame().map(Some)
    }

    #[must_use]
    pub fn selected_color_index(&self) -> Option<i32> {
        let annotation = self.selected_annotation()?;
        annotation_style(annotation).map(|style| color_index(style.color))
    }

    #[must_use]
    pub fn selected_stroke_width(&self) -> Option<f32> {
        match self.selected_annotation()? {
            Annotation::Rectangle { style, .. }
            | Annotation::Ellipse { style, .. }
            | Annotation::Arrow { style, .. }
            | Annotation::Line { style, .. }
            | Annotation::Pen { style, .. } => Some(style.stroke_width),
            Annotation::Text { style, .. } if style.stroke_width != SEQUENCE_SENTINEL_STROKE => {
                Some(((style.font_size - 20.0) / 2.0).clamp(1.0, 24.0))
            }
            Annotation::Mosaic { block_size, .. } => {
                Some((*block_size as f32 / 3.5).clamp(1.0, 24.0))
            }
            Annotation::Blur { radius, .. } => Some((*radius as f32 / 2.0).clamp(1.0, 24.0)),
            Annotation::Text { .. } => None,
        }
    }

    #[must_use]
    pub fn selection_bounds(&self) -> Option<SelectionBounds> {
        let rect = annotation_bounds(self.selected_annotation()?);
        Some(SelectionBounds {
            x: rect.origin.x,
            y: rect.origin.y,
            width: rect.width,
            height: rect.height,
        })
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.document.can_undo()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.document.can_redo()
    }

    pub fn begin_canvas(
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
        if self.tool == ActiveTool::Annotation(ToolKind::Text) {
            self.pending_text_origin = Some(point);
            return BeginResult::TextInput;
        }

        self.drag_start = Some(point);
        if self.tool == ActiveTool::Annotation(ToolKind::Pen) {
            self.pen_points.push(point);
            self.draft = Some(Annotation::Pen {
                points: self.pen_points.clone(),
                style: self.style,
            });
        } else {
            self.draft = self.annotation_from_drag(point, point);
        }
        BeginResult::Drawing
    }

    pub fn move_canvas(
        &mut self,
        x: f32,
        y: f32,
        canvas_width: f32,
        canvas_height: f32,
    ) -> Result<Option<CapturedFrame>, String> {
        let Some(point) = self.canvas_to_image(x, y, canvas_width, canvas_height) else {
            return Ok(None);
        };

        if self.tool == ActiveTool::Select {
            return self.move_selection(point);
        }

        let Some(start) = self.drag_start else {
            return Ok(None);
        };
        if self.tool == ActiveTool::Annotation(ToolKind::Pen) {
            if self
                .pen_points
                .last()
                .is_none_or(|last| last.distance_to(point) >= 0.75)
            {
                self.pen_points.push(point);
            }
            self.draft = Some(Annotation::Pen {
                points: self.pen_points.clone(),
                style: self.style,
            });
        } else {
            self.draft = self.annotation_from_drag(start, point);
        }

        let interval = self.preview_interval();
        if self
            .last_preview_at
            .as_ref()
            .is_some_and(|last| last.elapsed() < interval)
        {
            return Ok(None);
        }

        let frame = self.preview_frame()?;
        self.last_preview_at = Some(Instant::now());
        Ok(Some(frame))
    }

    pub fn end_canvas(
        &mut self,
        x: f32,
        y: f32,
        canvas_width: f32,
        canvas_height: f32,
    ) -> Result<Option<CapturedFrame>, String> {
        if self.tool == ActiveTool::Select {
            let point = self.canvas_to_image(x, y, canvas_width, canvas_height);
            return self.end_selection(point);
        }

        let Some(start) = self.drag_start else {
            return Ok(None);
        };
        if let Some(point) = self.canvas_to_image(x, y, canvas_width, canvas_height) {
            if self.tool == ActiveTool::Annotation(ToolKind::Pen) {
                if self
                    .pen_points
                    .last()
                    .is_none_or(|last| last.distance_to(point) >= 0.5)
                {
                    self.pen_points.push(point);
                }
                self.draft = Some(Annotation::Pen {
                    points: self.pen_points.clone(),
                    style: self.style,
                });
            } else {
                self.draft = self.annotation_from_drag(start, point);
            }
        }

        self.drag_start = None;
        self.pen_points.clear();
        self.last_preview_at = None;
        let Some(annotation) = self.draft.take() else {
            return self.current_frame().map(Some);
        };
        if !annotation.is_meaningful() {
            return self.current_frame().map(Some);
        }

        self.apply_annotation(annotation)?;
        self.current_frame().map(Some)
    }

    pub fn commit_text(&mut self, value: &str) -> Result<Option<CapturedFrame>, String> {
        self.last_select_click = None;
        if let Some(index) = self.pending_text_edit_index.take() {
            self.pending_text_origin = None;
            let Some(current) = self.document.items().get(index).cloned() else {
                self.selected_index = None;
                return Ok(None);
            };
            let Annotation::Text {
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

    pub fn add_text_annotations_from_ocr(
        &mut self,
        blocks: &[(f32, f32, f32, String)],
    ) -> Result<Option<CapturedFrame>, String> {
        let annotations = blocks
            .iter()
            .filter_map(|(x, y, box_height, value)| {
                let value = value.trim();
                if value.is_empty() {
                    return None;
                }
                let mut style = self.style;
                style.font_size = (*box_height * 0.8).clamp(10.0, 160.0);
                style.stroke_width = style.stroke_width.max(1.0);
                Some(Annotation::Text {
                    origin: Point::new(x.max(0.0), y.max(0.0)),
                    value: value.to_owned(),
                    style,
                })
            })
            .collect::<Vec<_>>();
        if annotations.is_empty() {
            return Ok(None);
        }

        let base = self
            .base
            .as_ref()
            .ok_or_else(|| "editor has no captured image".to_owned())?;
        let mut next_pixels = self
            .committed_rgba
            .as_ref()
            .ok_or_else(|| "editor has no committed pixel buffer".to_owned())?
            .clone();
        for annotation in &annotations {
            render_editor_annotation_in_place(
                &mut next_pixels,
                base.width(),
                base.height(),
                annotation,
            )?;
        }

        self.committed_rgba = Some(next_pixels);
        if let Some(preview) = self.preview.as_mut() {
            for annotation in &annotations {
                preview.apply(annotation)?;
            }
        }
        self.document.extend(annotations);
        self.recompute_sequence_next();
        self.current_frame().map(Some)
    }

    pub fn delete_selected(&mut self) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        let Some(index) = self.selected_index.take() else {
            return Ok(None);
        };
        if !self.document.remove(index) {
            return Ok(None);
        }
        self.rebuild_committed()?;
        self.recompute_sequence_next();
        self.current_frame().map(Some)
    }

    pub fn nudge_selected(&mut self, dx: f32, dy: f32) -> Result<Option<CapturedFrame>, String> {
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
        self.cancel_draft();
        if !self.document.undo() {
            return Ok(None);
        }
        self.selected_index = None;
        self.rebuild_committed()?;
        self.recompute_sequence_next();
        self.current_frame().map(Some)
    }

    pub fn redo(&mut self) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        if !self.document.redo() {
            return Ok(None);
        }
        self.selected_index = None;
        self.rebuild_committed()?;
        self.recompute_sequence_next();
        self.current_frame().map(Some)
    }

    pub fn clear(&mut self) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        if self.document.items().is_empty() {
            self.selected_index = None;
            return Ok(None);
        }
        self.document.clear();
        self.selected_index = None;
        self.sequence_next = 1;
        let Some(base) = self.base.as_ref() else {
            return Ok(None);
        };
        self.committed_rgba = Some(base.rgba().to_vec());
        if let Some(preview) = self.preview.as_mut() {
            preview.reset();
        }
        self.current_frame().map(Some)
    }

    fn begin_selection(
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

    fn move_selection(&mut self, point: Point) -> Result<Option<CapturedFrame>, String> {
        let Some(updated) = self.selection_update(point) else {
            return Ok(None);
        };
        let index = selection_drag_index(
            self.selection_drag
                .as_ref()
                .expect("selection update requires an active drag"),
        );
        self.selection_preview = Some(updated.clone());

        if self
            .last_preview_at
            .as_ref()
            .is_some_and(|last| last.elapsed() < Duration::from_millis(33))
        {
            return Ok(None);
        }
        let frame = self.selection_preview_frame(index, &updated)?;
        self.last_preview_at = Some(Instant::now());
        Ok(Some(frame))
    }

    fn end_selection(&mut self, point: Option<Point>) -> Result<Option<CapturedFrame>, String> {
        if let Some(point) = point
            && let Some(updated) = self.selection_update(point)
        {
            self.selection_preview = Some(updated);
        }

        self.last_preview_at = None;
        let Some(drag) = self.selection_drag.take() else {
            self.selection_preview = None;
            return self.current_frame().map(Some);
        };
        let index = selection_drag_index(&drag);
        let original = selection_drag_original(&drag).clone();
        let replacement = self
            .selection_preview
            .take()
            .unwrap_or_else(|| original.clone());
        if replacement != original {
            self.last_select_click = None;
            self.replace_annotation(index, replacement)?;
        }
        self.current_frame().map(Some)
    }

    fn selection_update(&self, point: Point) -> Option<Annotation> {
        match self.selection_drag.as_ref()? {
            SelectionDrag::Move {
                start, original, ..
            } => Some(translate_annotation(
                original,
                point.x - start.x,
                point.y - start.y,
            )),
            SelectionDrag::Resize {
                handle, original, ..
            } => {
                let old_bounds = annotation_bounds(original);
                let new_bounds = resized_bounds(old_bounds, *handle, point);
                Some(resize_annotation(original, old_bounds, new_bounds, *handle))
            }
        }
    }

    fn hit_test(&self, point: Point, tolerance: f32) -> Option<usize> {
        self.document
            .items()
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, annotation)| {
                annotation_hit_test(annotation, point, tolerance).then_some(index)
            })
    }

    fn selected_annotation(&self) -> Option<&Annotation> {
        let index = self.selected_index?;
        self.selection_preview
            .as_ref()
            .or_else(|| self.document.items().get(index))
    }

    fn image_tolerance(&self, canvas_width: f32, canvas_height: f32, screen_pixels: f32) -> f32 {
        let Some(frame) = self.base.as_ref() else {
            return screen_pixels;
        };
        if canvas_width <= 0.0 || canvas_height <= 0.0 {
            return screen_pixels;
        }
        let scale = (canvas_width / frame.width() as f32)
            .min(canvas_height / frame.height() as f32)
            .max(f32::EPSILON);
        screen_pixels / scale
    }

    fn replace_annotation(&mut self, index: usize, replacement: Annotation) -> Result<(), String> {
        let Some(current) = self.document.items().get(index) else {
            self.selected_index = None;
            return Ok(());
        };
        if *current == replacement {
            return Ok(());
        }
        if !self.document.replace(index, replacement) {
            self.selected_index = None;
            return Ok(());
        }
        self.rebuild_committed()?;
        self.recompute_sequence_next();
        Ok(())
    }

    fn apply_annotation(&mut self, annotation: Annotation) -> Result<(), String> {
        let base = self
            .base
            .as_ref()
            .ok_or_else(|| "editor has no captured image".to_owned())?;
        let pixels = self
            .committed_rgba
            .as_mut()
            .ok_or_else(|| "editor has no committed pixel buffer".to_owned())?;
        render_editor_annotation_in_place(pixels, base.width(), base.height(), &annotation)?;
        if let Some(preview) = self.preview.as_mut() {
            preview.apply(&annotation)?;
        }
        self.document.push(annotation);
        self.recompute_sequence_next();
        Ok(())
    }

    fn rebuild_committed(&mut self) -> Result<(), String> {
        let base = self
            .base
            .as_ref()
            .ok_or_else(|| "editor has no captured image".to_owned())?;
        let mut pixels = base.rgba().to_vec();
        for annotation in self.document.items() {
            render_editor_annotation_in_place(
                &mut pixels,
                base.width(),
                base.height(),
                annotation,
            )?;
        }
        self.committed_rgba = Some(pixels);
        if let Some(preview) = self.preview.as_mut() {
            preview.rebuild(&self.document)?;
        }
        Ok(())
    }

    fn recompute_sequence_next(&mut self) {
        self.sequence_next = self
            .document
            .items()
            .iter()
            .filter_map(sequence_number)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
    }

    fn preview_frame(&self) -> Result<CapturedFrame, String> {
        self.preview
            .as_ref()
            .ok_or_else(|| "editor has no preview surface".to_owned())?
            .frame_with_draft(self.draft.as_ref())
    }

    fn selection_preview_frame(
        &self,
        index: usize,
        replacement: &Annotation,
    ) -> Result<CapturedFrame, String> {
        self.preview
            .as_ref()
            .ok_or_else(|| "editor has no preview surface".to_owned())?
            .frame_with_override(&self.document, index, replacement)
    }

    fn current_frame(&self) -> Result<CapturedFrame, String> {
        let base = self
            .base
            .as_ref()
            .ok_or_else(|| "editor has no captured image".to_owned())?;
        let pixels = self
            .committed_rgba
            .as_ref()
            .ok_or_else(|| "editor has no committed pixel buffer".to_owned())?
            .clone();
        CapturedFrame::new(base.width(), base.height(), pixels).map_err(|error| error.to_string())
    }

    fn annotation_from_drag(&self, start: Point, end: Point) -> Option<Annotation> {
        let rect = Rect::from_points(start, end);
        match self.tool {
            ActiveTool::Annotation(ToolKind::Rectangle) => Some(Annotation::Rectangle {
                rect,
                style: self.style,
            }),
            ActiveTool::Annotation(ToolKind::Ellipse) => Some(Annotation::Ellipse {
                rect,
                style: self.style,
            }),
            ActiveTool::Annotation(ToolKind::Arrow) => Some(Annotation::Arrow {
                from: start,
                to: end,
                style: self.style,
            }),
            ActiveTool::Annotation(ToolKind::Line) => Some(Annotation::Line {
                from: start,
                to: end,
                style: self.style,
            }),
            ActiveTool::Annotation(ToolKind::Mosaic) => Some(Annotation::Mosaic {
                rect,
                block_size: 14,
            }),
            ActiveTool::Annotation(ToolKind::Blur) => Some(Annotation::Blur { rect, radius: 8 }),
            ActiveTool::Sequence => Some(self.sequence_annotation(end)),
            ActiveTool::Select | ActiveTool::Annotation(ToolKind::Pen | ToolKind::Text) => None,
        }
    }

    fn sequence_annotation(&self, center: Point) -> Annotation {
        Annotation::Text {
            // Sequence markers temporarily reuse Text as a document payload so they stay a single
            // history item. The negative sentinel routes them to the dedicated badge renderer.
            origin: center,
            value: self.sequence_next.to_string(),
            style: AnnotationStyle {
                color: self.style.color,
                stroke_width: SEQUENCE_SENTINEL_STROKE,
                font_size: self.style.font_size.clamp(28.0, 64.0),
            },
        }
    }

    fn canvas_to_image(
        &self,
        x: f32,
        y: f32,
        canvas_width: f32,
        canvas_height: f32,
    ) -> Option<Point> {
        let frame = self.base.as_ref()?;
        if canvas_width <= 0.0 || canvas_height <= 0.0 {
            return None;
        }

        let scale = (canvas_width / frame.width() as f32)
            .min(canvas_height / frame.height() as f32)
            .max(f32::EPSILON);
        let display_width = frame.width() as f32 * scale;
        let display_height = frame.height() as f32 * scale;
        let offset_x = (canvas_width - display_width) / 2.0;
        let offset_y = (canvas_height - display_height) / 2.0;

        if x < offset_x
            || y < offset_y
            || x > offset_x + display_width
            || y > offset_y + display_height
        {
            return None;
        }

        Some(Point::new(
            ((x - offset_x) / scale).clamp(0.0, frame.width() as f32),
            ((y - offset_y) / scale).clamp(0.0, frame.height() as f32),
        ))
    }

    fn preview_interval(&self) -> Duration {
        match self.tool {
            ActiveTool::Annotation(ToolKind::Blur) => Duration::from_millis(80),
            ActiveTool::Annotation(ToolKind::Mosaic) => Duration::from_millis(50),
            _ => Duration::from_millis(33),
        }
    }

    fn cancel_draft(&mut self) {
        self.draft = None;
        self.drag_start = None;
        self.pen_points.clear();
        self.pending_text_origin = None;
        self.pending_text_edit_index = None;
        self.last_select_click = None;
        self.selection_drag = None;
        self.selection_preview = None;
        self.last_preview_at = None;
    }
}

fn is_editable_text_annotation(annotation: &Annotation) -> bool {
    matches!(
        annotation,
        Annotation::Text { style, .. } if style.stroke_width != SEQUENCE_SENTINEL_STROKE
    )
}

fn selection_drag_index(drag: &SelectionDrag) -> usize {
    match drag {
        SelectionDrag::Move { index, .. } | SelectionDrag::Resize { index, .. } => *index,
    }
}

fn selection_drag_original(drag: &SelectionDrag) -> &Annotation {
    match drag {
        SelectionDrag::Move { original, .. } | SelectionDrag::Resize { original, .. } => original,
    }
}

fn color_from_index(index: i32) -> Color {
    match index {
        0 => Color::RED,
        1 => Color::ORANGE,
        2 => Color::YELLOW,
        3 => Color::GREEN,
        4 => Color::BLUE,
        5 => Color::PURPLE,
        6 => Color::WHITE,
        7 => Color::BLACK,
        _ => Color::RED,
    }
}

fn color_index(color: Color) -> i32 {
    if color == Color::ORANGE {
        1
    } else if color == Color::YELLOW {
        2
    } else if color == Color::GREEN {
        3
    } else if color == Color::BLUE {
        4
    } else if color == Color::PURPLE {
        5
    } else if color == Color::WHITE {
        6
    } else if color == Color::BLACK {
        7
    } else {
        0
    }
}

fn annotation_style(annotation: &Annotation) -> Option<AnnotationStyle> {
    match annotation {
        Annotation::Rectangle { style, .. }
        | Annotation::Ellipse { style, .. }
        | Annotation::Arrow { style, .. }
        | Annotation::Line { style, .. }
        | Annotation::Pen { style, .. }
        | Annotation::Text { style, .. } => Some(*style),
        Annotation::Mosaic { .. } | Annotation::Blur { .. } => None,
    }
}

fn annotation_with_color(annotation: &Annotation, color: Color) -> Annotation {
    let mut updated = annotation.clone();
    match &mut updated {
        Annotation::Rectangle { style, .. }
        | Annotation::Ellipse { style, .. }
        | Annotation::Arrow { style, .. }
        | Annotation::Line { style, .. }
        | Annotation::Pen { style, .. }
        | Annotation::Text { style, .. } => style.color = color,
        Annotation::Mosaic { .. } | Annotation::Blur { .. } => {}
    }
    updated
}

fn annotation_with_size(annotation: &Annotation, stroke_width: f32, font_size: f32) -> Annotation {
    let mut updated = annotation.clone();
    match &mut updated {
        Annotation::Rectangle { style, .. }
        | Annotation::Ellipse { style, .. }
        | Annotation::Arrow { style, .. }
        | Annotation::Line { style, .. }
        | Annotation::Pen { style, .. } => style.stroke_width = stroke_width,
        Annotation::Text { style, .. } => {
            if style.stroke_width != SEQUENCE_SENTINEL_STROKE {
                style.stroke_width = stroke_width;
            }
            style.font_size = font_size;
        }
        Annotation::Mosaic { block_size, .. } => {
            *block_size = (stroke_width * 3.5).round().clamp(4.0, 64.0) as u32;
        }
        Annotation::Blur { radius, .. } => {
            *radius = (stroke_width * 2.0).round().clamp(2.0, 32.0) as u32;
        }
    }
    updated
}

fn annotation_bounds(annotation: &Annotation) -> Rect {
    let rect = match annotation {
        Annotation::Rectangle { rect, .. }
        | Annotation::Ellipse { rect, .. }
        | Annotation::Mosaic { rect, .. }
        | Annotation::Blur { rect, .. } => *rect,
        Annotation::Arrow { from, to, .. } | Annotation::Line { from, to, .. } => {
            Rect::from_points(*from, *to)
        }
        Annotation::Pen { points, .. } => {
            points_bounds(points).unwrap_or(Rect::new(Point::new(0.0, 0.0), 1.0, 1.0))
        }
        Annotation::Text {
            origin,
            value,
            style,
        } if style.stroke_width == SEQUENCE_SENTINEL_STROKE => {
            let radius = sequence_radius(style.font_size);
            Rect::new(
                Point::new(origin.x - radius, origin.y - radius),
                radius * 2.0,
                radius * 2.0,
            )
        }
        Annotation::Text {
            origin,
            value,
            style,
        } => {
            let lines: Vec<&str> = value.split('\n').collect();
            let max_chars = lines
                .iter()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(1)
                .max(1) as f32;
            let line_count = lines.len().max(1) as f32;
            Rect::new(
                *origin,
                (max_chars * style.font_size * 0.62).max(style.font_size * 0.5),
                (line_count * style.font_size * 1.25).max(style.font_size),
            )
        }
    };
    ensure_selection_extent(rect)
}

fn points_bounds(points: &[Point]) -> Option<Rect> {
    let first = *points.first()?;
    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;
    for point in &points[1..] {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    Some(Rect::new(
        Point::new(min_x, min_y),
        max_x - min_x,
        max_y - min_y,
    ))
}

fn ensure_selection_extent(rect: Rect) -> Rect {
    let mut origin = rect.origin;
    let mut width = rect.width;
    let mut height = rect.height;
    if width < MIN_SELECTION_EXTENT {
        origin.x -= (MIN_SELECTION_EXTENT - width) / 2.0;
        width = MIN_SELECTION_EXTENT;
    }
    if height < MIN_SELECTION_EXTENT {
        origin.y -= (MIN_SELECTION_EXTENT - height) / 2.0;
        height = MIN_SELECTION_EXTENT;
    }
    Rect::new(origin, width, height)
}

fn annotation_hit_test(annotation: &Annotation, point: Point, tolerance: f32) -> bool {
    match annotation {
        Annotation::Rectangle { rect, .. }
        | Annotation::Mosaic { rect, .. }
        | Annotation::Blur { rect, .. } => point_in_rect(point, *rect, tolerance),
        Annotation::Ellipse { rect, .. } => point_in_ellipse(point, *rect, tolerance),
        Annotation::Arrow { from, to, style } => {
            let threshold = tolerance + style.stroke_width / 2.0;
            let (head_left, head_right) = arrow_head_points(*from, *to, *style);
            distance_to_segment(point, *from, *to) <= threshold
                || distance_to_segment(point, *to, head_left) <= threshold
                || distance_to_segment(point, *to, head_right) <= threshold
        }
        Annotation::Line { from, to, style } => {
            distance_to_segment(point, *from, *to) <= tolerance + style.stroke_width / 2.0
        }
        Annotation::Pen { points, style } => points.windows(2).any(|segment| {
            distance_to_segment(point, segment[0], segment[1])
                <= tolerance + style.stroke_width / 2.0
        }),
        Annotation::Text { origin, style, .. }
            if style.stroke_width == SEQUENCE_SENTINEL_STROKE =>
        {
            point.distance_to(*origin) <= sequence_radius(style.font_size) + tolerance
        }
        Annotation::Text { .. } => point_in_rect(point, annotation_bounds(annotation), tolerance),
    }
}

fn point_in_rect(point: Point, rect: Rect, tolerance: f32) -> bool {
    point.x >= rect.origin.x - tolerance
        && point.y >= rect.origin.y - tolerance
        && point.x <= rect.origin.x + rect.width + tolerance
        && point.y <= rect.origin.y + rect.height + tolerance
}

fn point_in_ellipse(point: Point, rect: Rect, tolerance: f32) -> bool {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return false;
    }
    let center = Point::new(
        rect.origin.x + rect.width / 2.0,
        rect.origin.y + rect.height / 2.0,
    );
    let radius_x = rect.width / 2.0 + tolerance.max(0.0);
    let radius_y = rect.height / 2.0 + tolerance.max(0.0);
    let dx = (point.x - center.x) / radius_x.max(f32::EPSILON);
    let dy = (point.y - center.y) / radius_y.max(f32::EPSILON);
    dx * dx + dy * dy <= 1.0
}

fn arrow_head_points(from: Point, to: Point, style: AnnotationStyle) -> (Point, Point) {
    let angle = (to.y - from.y).atan2(to.x - from.x);
    let head = (style.stroke_width * 5.0).clamp(12.0, 32.0);
    let spread = 0.58;
    (
        Point::new(
            to.x - head * (angle - spread).cos(),
            to.y - head * (angle - spread).sin(),
        ),
        Point::new(
            to.x - head * (angle + spread).cos(),
            to.y - head * (angle + spread).sin(),
        ),
    )
}

fn distance_to_segment(point: Point, from: Point, to: Point) -> f32 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let length_sq = dx * dx + dy * dy;
    if length_sq <= f32::EPSILON {
        return point.distance_to(from);
    }
    let t = (((point.x - from.x) * dx + (point.y - from.y) * dy) / length_sq).clamp(0.0, 1.0);
    point.distance_to(Point::new(from.x + dx * t, from.y + dy * t))
}

fn resize_handle_at(bounds: Rect, point: Point, tolerance: f32) -> Option<ResizeHandle> {
    let left = bounds.origin.x;
    let top = bounds.origin.y;
    let right = left + bounds.width;
    let bottom = top + bounds.height;
    let center_x = left + bounds.width / 2.0;
    let center_y = top + bounds.height / 2.0;
    let handles = [
        (ResizeHandle::NorthWest, Point::new(left, top)),
        (ResizeHandle::NorthEast, Point::new(right, top)),
        (ResizeHandle::SouthEast, Point::new(right, bottom)),
        (ResizeHandle::SouthWest, Point::new(left, bottom)),
        (ResizeHandle::North, Point::new(center_x, top)),
        (ResizeHandle::East, Point::new(right, center_y)),
        (ResizeHandle::South, Point::new(center_x, bottom)),
        (ResizeHandle::West, Point::new(left, center_y)),
    ];
    handles
        .into_iter()
        .find_map(|(handle, position)| (point.distance_to(position) <= tolerance).then_some(handle))
}

fn resized_bounds(bounds: Rect, handle: ResizeHandle, point: Point) -> Rect {
    let mut left = bounds.origin.x;
    let mut top = bounds.origin.y;
    let mut right = left + bounds.width;
    let mut bottom = top + bounds.height;

    match handle {
        ResizeHandle::NorthWest => {
            left = point.x;
            top = point.y;
        }
        ResizeHandle::North => top = point.y,
        ResizeHandle::NorthEast => {
            right = point.x;
            top = point.y;
        }
        ResizeHandle::East => right = point.x,
        ResizeHandle::SouthEast => {
            right = point.x;
            bottom = point.y;
        }
        ResizeHandle::South => bottom = point.y,
        ResizeHandle::SouthWest => {
            left = point.x;
            bottom = point.y;
        }
        ResizeHandle::West => left = point.x,
    }

    ensure_selection_extent(Rect::from_points(
        Point::new(left, top),
        Point::new(right, bottom),
    ))
}

fn translate_annotation(annotation: &Annotation, dx: f32, dy: f32) -> Annotation {
    let translate_point = |point: Point| Point::new(point.x + dx, point.y + dy);
    let translate_rect =
        |rect: Rect| Rect::new(translate_point(rect.origin), rect.width, rect.height);
    match annotation {
        Annotation::Rectangle { rect, style } => Annotation::Rectangle {
            rect: translate_rect(*rect),
            style: *style,
        },
        Annotation::Ellipse { rect, style } => Annotation::Ellipse {
            rect: translate_rect(*rect),
            style: *style,
        },
        Annotation::Arrow { from, to, style } => Annotation::Arrow {
            from: translate_point(*from),
            to: translate_point(*to),
            style: *style,
        },
        Annotation::Line { from, to, style } => Annotation::Line {
            from: translate_point(*from),
            to: translate_point(*to),
            style: *style,
        },
        Annotation::Pen { points, style } => Annotation::Pen {
            points: points.iter().copied().map(translate_point).collect(),
            style: *style,
        },
        Annotation::Text {
            origin,
            value,
            style,
        } => Annotation::Text {
            origin: translate_point(*origin),
            value: value.clone(),
            style: *style,
        },
        Annotation::Mosaic { rect, block_size } => Annotation::Mosaic {
            rect: translate_rect(*rect),
            block_size: *block_size,
        },
        Annotation::Blur { rect, radius } => Annotation::Blur {
            rect: translate_rect(*rect),
            radius: *radius,
        },
    }
}

fn resize_annotation(
    annotation: &Annotation,
    old_bounds: Rect,
    new_bounds: Rect,
    handle: ResizeHandle,
) -> Annotation {
    let scale_x = new_bounds.width / old_bounds.width.max(f32::EPSILON);
    let scale_y = new_bounds.height / old_bounds.height.max(f32::EPSILON);
    let scale_point = |point: Point| {
        Point::new(
            new_bounds.origin.x + (point.x - old_bounds.origin.x) * scale_x,
            new_bounds.origin.y + (point.y - old_bounds.origin.y) * scale_y,
        )
    };
    let size_scale = match handle {
        ResizeHandle::North | ResizeHandle::South => scale_y,
        ResizeHandle::East | ResizeHandle::West => scale_x,
        ResizeHandle::NorthWest
        | ResizeHandle::NorthEast
        | ResizeHandle::SouthEast
        | ResizeHandle::SouthWest => {
            if (scale_x - 1.0).abs() >= (scale_y - 1.0).abs() {
                scale_x
            } else {
                scale_y
            }
        }
    }
    .max(0.05);

    match annotation {
        Annotation::Rectangle { style, .. } => Annotation::Rectangle {
            rect: new_bounds,
            style: *style,
        },
        Annotation::Ellipse { style, .. } => Annotation::Ellipse {
            rect: new_bounds,
            style: *style,
        },
        Annotation::Arrow { from, to, style } => Annotation::Arrow {
            from: scale_point(*from),
            to: scale_point(*to),
            style: *style,
        },
        Annotation::Line { from, to, style } => Annotation::Line {
            from: scale_point(*from),
            to: scale_point(*to),
            style: *style,
        },
        Annotation::Pen { points, style } => Annotation::Pen {
            points: points.iter().copied().map(scale_point).collect(),
            style: *style,
        },
        Annotation::Text {
            origin,
            value,
            style,
        } if style.stroke_width == SEQUENCE_SENTINEL_STROKE => {
            let mut style = *style;
            style.font_size = (style.font_size * size_scale).clamp(12.0, 160.0);
            Annotation::Text {
                origin: Point::new(
                    new_bounds.origin.x + new_bounds.width / 2.0,
                    new_bounds.origin.y + new_bounds.height / 2.0,
                ),
                value: value.clone(),
                style,
            }
        }
        Annotation::Text {
            origin: _,
            value,
            style,
        } => {
            let mut style = *style;
            style.font_size = (style.font_size * size_scale).clamp(10.0, 160.0);
            Annotation::Text {
                origin: new_bounds.origin,
                value: value.clone(),
                style,
            }
        }
        Annotation::Mosaic { block_size, .. } => Annotation::Mosaic {
            rect: new_bounds,
            block_size: ((*block_size as f32 * size_scale).round() as u32).max(2),
        },
        Annotation::Blur { radius, .. } => Annotation::Blur {
            rect: new_bounds,
            radius: ((*radius as f32 * size_scale).round() as u32).max(1),
        },
    }
}

fn sequence_number(annotation: &Annotation) -> Option<u32> {
    match annotation {
        Annotation::Text { value, style, .. } if style.stroke_width == SEQUENCE_SENTINEL_STROKE => {
            value.parse().ok()
        }
        _ => None,
    }
}

#[cfg(test)]
fn is_sequence_annotation(annotation: &Annotation) -> bool {
    sequence_number(annotation).is_some()
}

fn render_editor_annotation_in_place(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    annotation: &Annotation,
) -> Result<(), String> {
    if let Annotation::Text {
        origin,
        value,
        style,
    } = annotation
        && style.stroke_width == SEQUENCE_SENTINEL_STROKE
    {
        let number = value
            .parse::<u32>()
            .map_err(|_| format!("invalid sequence marker value: {value}"))?;
        return render_sequence_marker(pixels, width, height, *origin, number, *style);
    }

    render_annotation_in_place(pixels, width, height, annotation).map_err(|error| error.to_string())
}

fn render_sequence_marker(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: Point,
    number: u32,
    style: AnnotationStyle,
) -> Result<(), String> {
    let radius = sequence_radius(style.font_size);
    let left = (center.x - radius).floor().max(0.0).min(width as f32) as u32;
    let top = (center.y - radius).floor().max(0.0).min(height as f32) as u32;
    let right = (center.x + radius).ceil().max(0.0).min(width as f32) as u32;
    let bottom = (center.y + radius).ceil().max(0.0).min(height as f32) as u32;
    if right <= left || bottom <= top {
        return Ok(());
    }

    let snapshot_width = right - left;
    let snapshot_height = bottom - top;
    let snapshot = copy_region(pixels, width, left, top, right, bottom);

    fill_disc(
        pixels,
        width,
        height,
        center.x,
        center.y,
        radius,
        style.color,
    );

    let digits = number.to_string();
    let font_size = sequence_digit_font_size(radius, digits.len());
    let mask_size = ((radius * 4.0).ceil() as u32).max(64);
    let mut mask = vec![0_u8; mask_size as usize * mask_size as usize * 4];
    let mask_text = Annotation::Text {
        origin: Point::new(mask_size as f32 * 0.25, mask_size as f32 * 0.25),
        value: digits,
        style: AnnotationStyle {
            color: Color::WHITE,
            stroke_width: 1.0,
            font_size,
        },
    };
    render_annotation_in_place(&mut mask, mask_size, mask_size, &mask_text)
        .map_err(|error| error.to_string())?;

    let Some((glyph_left, glyph_top, glyph_right, glyph_bottom)) = alpha_bounds(&mask, mask_size)
    else {
        return Ok(());
    };
    let glyph_width = glyph_right - glyph_left;
    let glyph_height = glyph_bottom - glyph_top;
    let target_left = (center.x - glyph_width as f32 / 2.0).round() as i32;
    let target_top = (center.y - glyph_height as f32 / 2.0).round() as i32;

    for mask_y in glyph_top..glyph_bottom {
        for mask_x in glyph_left..glyph_right {
            let mask_index = ((mask_y * mask_size + mask_x) * 4) as usize;
            let coverage = mask[mask_index];
            if coverage == 0 {
                continue;
            }

            let target_x = target_left + (mask_x - glyph_left) as i32;
            let target_y = target_top + (mask_y - glyph_top) as i32;
            if target_x < left as i32
                || target_y < top as i32
                || target_x >= right as i32
                || target_y >= bottom as i32
            {
                continue;
            }

            restore_snapshot_pixel(
                pixels,
                width,
                target_x as u32,
                target_y as u32,
                &snapshot,
                snapshot_width,
                snapshot_height,
                left,
                top,
                coverage,
            );
        }
    }

    Ok(())
}

fn sequence_radius(font_size: f32) -> f32 {
    (font_size * 0.72).clamp(16.0, 36.0)
}

fn sequence_digit_font_size(radius: f32, digits: usize) -> f32 {
    let factor = match digits {
        0 | 1 => 1.18,
        2 => 0.95,
        _ => 0.76,
    };
    (radius * factor).clamp(12.0, 64.0)
}

fn copy_region(pixels: &[u8], width: u32, left: u32, top: u32, right: u32, bottom: u32) -> Vec<u8> {
    let region_width = (right - left) as usize;
    let region_height = (bottom - top) as usize;
    let mut snapshot = vec![0_u8; region_width * region_height * 4];

    for row in 0..region_height {
        let source_start = ((top as usize + row) * width as usize + left as usize) * 4;
        let source_end = source_start + region_width * 4;
        let target_start = row * region_width * 4;
        snapshot[target_start..target_start + region_width * 4]
            .copy_from_slice(&pixels[source_start..source_end]);
    }
    snapshot
}

fn fill_disc(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center_x: f32,
    center_y: f32,
    radius: f32,
    color: Color,
) {
    let min_x = (center_x - radius).floor().max(0.0) as i32;
    let min_y = (center_y - radius).floor().max(0.0) as i32;
    let max_x = (center_x + radius)
        .ceil()
        .min(width.saturating_sub(1) as f32) as i32;
    let max_y = (center_y + radius)
        .ceil()
        .min(height.saturating_sub(1) as f32) as i32;
    let radius_sq = radius * radius;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 + 0.5 - center_x;
            let dy = y as f32 + 0.5 - center_y;
            if dx * dx + dy * dy <= radius_sq {
                blend_rgba_pixel(pixels, width, x as u32, y as u32, color);
            }
        }
    }
}

fn blend_rgba_pixel(pixels: &mut [u8], width: u32, x: u32, y: u32, color: Color) {
    let index = ((y * width + x) * 4) as usize;
    let alpha = u16::from(color.a);
    let inverse = 255_u16.saturating_sub(alpha);
    pixels[index] = ((u16::from(color.r) * alpha + u16::from(pixels[index]) * inverse) / 255) as u8;
    pixels[index + 1] =
        ((u16::from(color.g) * alpha + u16::from(pixels[index + 1]) * inverse) / 255) as u8;
    pixels[index + 2] =
        ((u16::from(color.b) * alpha + u16::from(pixels[index + 2]) * inverse) / 255) as u8;
    pixels[index + 3] = 255;
}

fn alpha_bounds(mask: &[u8], width: u32) -> Option<(u32, u32, u32, u32)> {
    let height = mask.len() as u32 / 4 / width;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;

    for y in 0..height {
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            if mask[index] == 0 {
                continue;
            }
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + 1);
            max_y = max_y.max(y + 1);
        }
    }

    found.then_some((min_x, min_y, max_x, max_y))
}

#[allow(clippy::too_many_arguments)]
fn restore_snapshot_pixel(
    pixels: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    snapshot: &[u8],
    snapshot_width: u32,
    snapshot_height: u32,
    snapshot_left: u32,
    snapshot_top: u32,
    coverage: u8,
) {
    let local_x = x.saturating_sub(snapshot_left);
    let local_y = y.saturating_sub(snapshot_top);
    if local_x >= snapshot_width || local_y >= snapshot_height {
        return;
    }

    let target_index = ((y * width + x) * 4) as usize;
    let source_index = ((local_y * snapshot_width + local_x) * 4) as usize;
    let alpha = u16::from(coverage);
    let inverse = 255_u16.saturating_sub(alpha);
    for channel in 0..4 {
        pixels[target_index + channel] = ((u16::from(pixels[target_index + channel]) * inverse
            + u16::from(snapshot[source_index + channel]) * alpha)
            / 255) as u8;
    }
}

fn resize_rgba_nearest(
    pixels: &[u8],
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> Vec<u8> {
    if source_width == target_width && source_height == target_height {
        return pixels.to_vec();
    }

    let mut resized = vec![0_u8; target_width as usize * target_height as usize * 4];
    for target_y in 0..target_height {
        let source_y = ((u64::from(target_y) * u64::from(source_height)) / u64::from(target_height))
            .min(u64::from(source_height - 1)) as u32;
        for target_x in 0..target_width {
            let source_x = ((u64::from(target_x) * u64::from(source_width))
                / u64::from(target_width))
            .min(u64::from(source_width - 1)) as u32;
            let source_index = ((source_y * source_width + source_x) * 4) as usize;
            let target_index = ((target_y * target_width + target_x) * 4) as usize;
            resized[target_index..target_index + 4]
                .copy_from_slice(&pixels[source_index..source_index + 4]);
        }
    }
    resized
}

fn scale_annotation(annotation: &Annotation, scale_x: f32, scale_y: f32) -> Annotation {
    let scale_point = |point: Point| Point::new(point.x * scale_x, point.y * scale_y);
    let scale_rect = |rect: Rect| {
        Rect::new(
            scale_point(rect.origin),
            rect.width * scale_x,
            rect.height * scale_y,
        )
    };
    let style_scale = scale_x.min(scale_y);
    let scale_style = |style: AnnotationStyle| AnnotationStyle {
        color: style.color,
        stroke_width: if style.stroke_width == SEQUENCE_SENTINEL_STROKE {
            SEQUENCE_SENTINEL_STROKE
        } else {
            (style.stroke_width * style_scale).max(1.0)
        },
        font_size: style.font_size * style_scale,
    };

    match annotation {
        Annotation::Rectangle { rect, style } => Annotation::Rectangle {
            rect: scale_rect(*rect),
            style: scale_style(*style),
        },
        Annotation::Ellipse { rect, style } => Annotation::Ellipse {
            rect: scale_rect(*rect),
            style: scale_style(*style),
        },
        Annotation::Arrow { from, to, style } => Annotation::Arrow {
            from: scale_point(*from),
            to: scale_point(*to),
            style: scale_style(*style),
        },
        Annotation::Line { from, to, style } => Annotation::Line {
            from: scale_point(*from),
            to: scale_point(*to),
            style: scale_style(*style),
        },
        Annotation::Pen { points, style } => Annotation::Pen {
            points: points.iter().copied().map(scale_point).collect(),
            style: scale_style(*style),
        },
        Annotation::Text {
            origin,
            value,
            style,
        } => Annotation::Text {
            origin: scale_point(*origin),
            value: value.clone(),
            style: scale_style(*style),
        },
        Annotation::Mosaic { rect, block_size } => Annotation::Mosaic {
            rect: scale_rect(*rect),
            block_size: ((*block_size as f32 * style_scale).round() as u32).max(2),
        },
        Annotation::Blur { rect, radius } => Annotation::Blur {
            rect: scale_rect(*rect),
            radius: ((*radius as f32 * style_scale).round() as u32).max(1),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(width: u32, height: u32) -> CapturedFrame {
        CapturedFrame::new(
            width,
            height,
            vec![255; width as usize * height as usize * 4],
        )
        .expect("test frame should be valid")
    }

    #[test]
    fn cancelling_text_does_not_render_or_add_history() {
        let mut editor = EditorSession::default();
        editor.reset(frame(100, 100));
        editor.set_tool("text");
        editor.begin_canvas(20.0, 20.0, 100.0, 100.0);
        assert!(editor.commit_text("").unwrap().is_none());
        assert!(editor.pending_text_origin.is_none());
        assert!(!editor.can_undo());
        assert!(editor.document.items().is_empty());
    }

    fn place_number(editor: &mut EditorSession, x: f32, y: f32) {
        assert!(editor.set_tool(SEQUENCE_TOOL_ID));
        assert_eq!(
            editor.begin_canvas(x, y, 200.0, 100.0),
            BeginResult::Drawing
        );
        editor
            .end_canvas(x, y, 200.0, 100.0)
            .expect("number marker should render");
    }

    fn place_rectangle(editor: &mut EditorSession, start: Point, end: Point) {
        assert!(editor.set_tool("rectangle"));
        assert_eq!(
            editor.begin_canvas(start.x, start.y, 100.0, 100.0),
            BeginResult::Drawing
        );
        editor
            .end_canvas(end.x, end.y, 100.0, 100.0)
            .expect("rectangle should render");
    }

    #[test]
    fn preview_surface_bounds_large_images() {
        let preview = PreviewSurface::new(&frame(3840, 2160));
        assert_eq!(preview.width, 1600);
        assert_eq!(preview.height, 900);
    }

    #[test]
    fn canvas_mapping_keeps_exclusive_bottom_right_endpoint() {
        let mut editor = EditorSession::default();
        editor.reset(frame(100, 50));
        let point = editor
            .canvas_to_image(200.0, 100.0, 200.0, 100.0)
            .expect("bottom-right edge belongs to the image");
        assert_eq!(point, Point::new(100.0, 50.0));
    }

    #[test]
    fn sequence_tool_increments_and_tracks_history() {
        let mut editor = EditorSession::default();
        editor.reset(frame(100, 50));

        place_number(&mut editor, 40.0, 40.0);
        place_number(&mut editor, 80.0, 40.0);
        assert_eq!(editor.sequence_next, 3);
        assert_eq!(editor.document.items().len(), 2);
        assert_eq!(sequence_number(&editor.document.items()[0]), Some(1));
        assert_eq!(sequence_number(&editor.document.items()[1]), Some(2));

        editor.undo().expect("undo should succeed");
        assert_eq!(editor.sequence_next, 2);
        editor.redo().expect("redo should succeed");
        assert_eq!(editor.sequence_next, 3);
        editor.clear().expect("clear should succeed");
        assert_eq!(editor.sequence_next, 1);
    }

    #[test]
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
        let mut editor = EditorSession::default();
        editor.reset(frame(100, 100));
        place_rectangle(&mut editor, Point::new(10.0, 10.0), Point::new(30.0, 30.0));

        assert!(editor.set_tool(SELECT_TOOL_ID));
        assert_eq!(
            editor.begin_canvas(20.0, 20.0, 100.0, 100.0),
            BeginResult::Drawing
        );
        editor
            .move_canvas(40.0, 45.0, 100.0, 100.0)
            .expect("selection preview should render");
        editor
            .end_canvas(40.0, 45.0, 100.0, 100.0)
            .expect("selection move should commit");

        let bounds = annotation_bounds(&editor.document.items()[0]);
        assert_eq!(bounds.origin, Point::new(30.0, 35.0));
        assert_eq!(bounds.width, 20.0);
        assert_eq!(bounds.height, 20.0);

        editor.undo().expect("move undo should succeed");
        let bounds = annotation_bounds(&editor.document.items()[0]);
        assert_eq!(bounds.origin, Point::new(10.0, 10.0));
        assert_eq!(editor.document.items().len(), 1);
    }

    #[test]
    fn selection_resize_uses_corner_handles() {
        let mut editor = EditorSession::default();
        editor.reset(frame(100, 100));
        place_rectangle(&mut editor, Point::new(10.0, 10.0), Point::new(30.0, 30.0));
        assert!(editor.set_tool(SELECT_TOOL_ID));
        editor.begin_canvas(20.0, 20.0, 100.0, 100.0);
        editor.end_canvas(20.0, 20.0, 100.0, 100.0).unwrap();

        editor.begin_canvas(30.0, 30.0, 100.0, 100.0);
        editor
            .move_canvas(50.0, 60.0, 100.0, 100.0)
            .expect("resize preview should render");
        editor
            .end_canvas(50.0, 60.0, 100.0, 100.0)
            .expect("resize should commit");

        let bounds = annotation_bounds(&editor.document.items()[0]);
        assert_eq!(bounds.origin, Point::new(10.0, 10.0));
        assert_eq!(bounds.width, 40.0);
        assert_eq!(bounds.height, 50.0);
    }

    #[test]
    fn selected_object_delete_and_style_change_are_undoable() {
        let mut editor = EditorSession::default();
        editor.reset(frame(100, 100));
        place_rectangle(&mut editor, Point::new(10.0, 10.0), Point::new(30.0, 30.0));
        assert!(editor.set_tool(SELECT_TOOL_ID));
        editor.begin_canvas(20.0, 20.0, 100.0, 100.0);
        editor.end_canvas(20.0, 20.0, 100.0, 100.0).unwrap();

        editor
            .set_color(4)
            .expect("selected color change should render");
        match &editor.document.items()[0] {
            Annotation::Rectangle { style, .. } => assert_eq!(style.color, Color::BLUE),
            _ => panic!("expected rectangle"),
        }
        editor.undo().expect("style undo should succeed");
        match &editor.document.items()[0] {
            Annotation::Rectangle { style, .. } => assert_eq!(style.color, Color::RED),
            _ => panic!("expected rectangle"),
        }

        editor.set_tool(SELECT_TOOL_ID);
        editor.begin_canvas(20.0, 20.0, 100.0, 100.0);
        editor.end_canvas(20.0, 20.0, 100.0, 100.0).unwrap();
        editor.delete_selected().expect("delete should render");
        assert!(editor.document.items().is_empty());
        editor.undo().expect("delete undo should succeed");
        assert_eq!(editor.document.items().len(), 1);
    }

    #[test]
    fn ellipse_hit_test_rejects_transparent_bounding_box_corner() {
        let ellipse = Annotation::Ellipse {
            rect: Rect::new(Point::new(0.0, 0.0), 100.0, 60.0),
            style: AnnotationStyle::default(),
        };
        assert!(!annotation_hit_test(&ellipse, Point::new(2.0, 2.0), 1.0));
        assert!(annotation_hit_test(&ellipse, Point::new(50.0, 30.0), 1.0));
    }

    #[test]
    fn arrow_hit_test_includes_rendered_arrowhead_segments() {
        let style = AnnotationStyle {
            stroke_width: 4.0,
            ..AnnotationStyle::default()
        };
        let from = Point::new(10.0, 50.0);
        let to = Point::new(80.0, 50.0);
        let (_, head_right) = arrow_head_points(from, to, style);
        let head_midpoint = Point::new((to.x + head_right.x) / 2.0, (to.y + head_right.y) / 2.0);
        let arrow = Annotation::Arrow { from, to, style };
        assert!(distance_to_segment(head_midpoint, from, to) > 3.0);
        assert!(annotation_hit_test(&arrow, head_midpoint, 1.0));
    }

    #[test]
    fn text_side_handle_resize_scales_regular_and_sequence_text() {
        let regular = Annotation::Text {
            origin: Point::new(10.0, 10.0),
            value: "Resize me".to_owned(),
            style: AnnotationStyle {
                font_size: 30.0,
                ..AnnotationStyle::default()
            },
        };
        let regular_bounds = annotation_bounds(&regular);
        let wider_regular = Rect::new(
            regular_bounds.origin,
            regular_bounds.width * 1.5,
            regular_bounds.height,
        );
        let resized_regular =
            resize_annotation(&regular, regular_bounds, wider_regular, ResizeHandle::East);
        match resized_regular {
            Annotation::Text { style, .. } => assert!(style.font_size > 30.0),
            _ => panic!("expected text annotation"),
        }

        let sequence = Annotation::Text {
            origin: Point::new(50.0, 50.0),
            value: "1".to_owned(),
            style: AnnotationStyle {
                stroke_width: SEQUENCE_SENTINEL_STROKE,
                font_size: 32.0,
                ..AnnotationStyle::default()
            },
        };
        let sequence_bounds = annotation_bounds(&sequence);
        let wider_sequence = Rect::new(
            sequence_bounds.origin,
            sequence_bounds.width * 1.5,
            sequence_bounds.height,
        );
        let resized_sequence = resize_annotation(
            &sequence,
            sequence_bounds,
            wider_sequence,
            ResizeHandle::East,
        );
        match resized_sequence {
            Annotation::Text { style, .. } => assert!(style.font_size > 32.0),
            _ => panic!("expected sequence text annotation"),
        }
    }

    #[test]
    fn ocr_text_selection_conversion_is_one_undoable_transaction() {
        let mut editor = EditorSession::default();
        editor.reset(frame(240, 120));
        let blocks = vec![
            (12.0, 18.0, 30.0, "  Hello OCR  ".to_owned()),
            (12.0, 58.0, 20.0, "第二行".to_owned()),
        ];
        editor
            .add_text_annotations_from_ocr(&blocks)
            .expect("OCR text conversion should render");
        assert_eq!(editor.document.items().len(), 2);
        match &editor.document.items()[0] {
            Annotation::Text {
                origin,
                value,
                style,
            } => {
                assert_eq!(*origin, Point::new(12.0, 18.0));
                assert_eq!(value, "Hello OCR");
                assert!((style.font_size - 24.0).abs() < 0.01);
                assert!(style.stroke_width > 0.0);
            }
            _ => panic!("expected text annotation"),
        }
        assert!(editor.can_undo());
        editor
            .undo()
            .expect("OCR selection conversion should undo once");
        assert!(editor.document.items().is_empty());
        editor
            .redo()
            .expect("OCR selection conversion should redo once");
        assert_eq!(editor.document.items().len(), 2);
    }

    #[test]
    fn sequence_marker_renders_filled_disc_with_punched_out_digit() {
        let mut editor = EditorSession::default();
        editor.reset(frame(100, 50));
        place_number(&mut editor, 100.0, 50.0);
        let rendered = editor.current_frame().expect("rendered frame should exist");

        let center = Point::new(50.0, 25.0);
        let radius = sequence_radius(AnnotationStyle::default().font_size);
        let mut fill_pixels = 0;
        let mut restored_pixels = 0;
        for y in 0..rendered.height() {
            for x in 0..rendered.width() {
                let dx = x as f32 + 0.5 - center.x;
                let dy = y as f32 + 0.5 - center.y;
                if dx * dx + dy * dy > (radius * 0.8).powi(2) {
                    continue;
                }
                let index = ((y * rendered.width() + x) * 4) as usize;
                let pixel = &rendered.rgba()[index..index + 4];
                if pixel == [Color::RED.r, Color::RED.g, Color::RED.b, 255] {
                    fill_pixels += 1;
                }
                if pixel == [255, 255, 255, 255] {
                    restored_pixels += 1;
                }
            }
        }
        assert!(fill_pixels > 0, "marker must contain its fill color");
        assert!(
            restored_pixels > 0,
            "digit must reveal the pixels that existed before the marker was drawn"
        );
    }

    #[test]
    fn preview_scaling_preserves_sequence_marker_identity() {
        let annotation = Annotation::Text {
            origin: Point::new(100.0, 50.0),
            value: "12".to_owned(),
            style: AnnotationStyle {
                color: Color::BLUE,
                stroke_width: SEQUENCE_SENTINEL_STROKE,
                font_size: 32.0,
            },
        };
        let scaled = scale_annotation(&annotation, 0.5, 0.5);
        assert!(is_sequence_annotation(&scaled));
        assert_eq!(sequence_number(&scaled), Some(12));
    }
}
