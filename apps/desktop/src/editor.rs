use azusa_annotation::{
    Annotation, AnnotationDocument, AnnotationStyle, Color, Point, Rect, ToolKind,
    render_annotation_in_place, render_document,
};
use azusa_capture::CapturedFrame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginResult {
    Drawing,
    TextInput,
    Ignored,
}

pub struct EditorSession {
    base: Option<CapturedFrame>,
    document: AnnotationDocument,
    committed_rgba: Option<Vec<u8>>,
    draft: Option<Annotation>,
    drag_start: Option<Point>,
    pen_points: Vec<Point>,
    pending_text_origin: Option<Point>,
    tool: ToolKind,
    style: AnnotationStyle,
}

impl Default for EditorSession {
    fn default() -> Self {
        Self {
            base: None,
            document: AnnotationDocument::default(),
            committed_rgba: None,
            draft: None,
            drag_start: None,
            pen_points: Vec::new(),
            pending_text_origin: None,
            tool: ToolKind::Rectangle,
            style: AnnotationStyle::default(),
        }
    }
}

impl EditorSession {
    pub fn reset(&mut self, frame: CapturedFrame) {
        self.committed_rgba = Some(frame.rgba().to_vec());
        self.base = Some(frame);
        self.document.clear();
        self.draft = None;
        self.drag_start = None;
        self.pen_points.clear();
        self.pending_text_origin = None;
    }

    pub fn set_tool(&mut self, id: &str) -> bool {
        let Some(tool) = ToolKind::from_id(id) else {
            return false;
        };
        self.tool = tool;
        self.cancel_draft();
        true
    }

    pub fn set_color(&mut self, index: i32) {
        self.style.color = match index {
            0 => Color::RED,
            1 => Color::ORANGE,
            2 => Color::YELLOW,
            3 => Color::GREEN,
            4 => Color::BLUE,
            5 => Color::PURPLE,
            6 => Color::WHITE,
            7 => Color::BLACK,
            _ => Color::RED,
        };
    }

    pub fn set_stroke_width(&mut self, width: f32) {
        self.style.stroke_width = width.clamp(1.0, 24.0);
        self.style.font_size = (20.0 + self.style.stroke_width * 2.0).clamp(22.0, 56.0);
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
            return BeginResult::Ignored;
        };

        self.cancel_draft();
        if self.tool == ToolKind::Text {
            self.pending_text_origin = Some(point);
            return BeginResult::TextInput;
        }

        self.drag_start = Some(point);
        if self.tool == ToolKind::Pen {
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
        let Some(start) = self.drag_start else {
            return Ok(None);
        };
        let Some(point) = self.canvas_to_image(x, y, canvas_width, canvas_height) else {
            return Ok(None);
        };

        if self.tool == ToolKind::Pen {
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
        self.preview_frame().map(Some)
    }

    pub fn end_canvas(
        &mut self,
        x: f32,
        y: f32,
        canvas_width: f32,
        canvas_height: f32,
    ) -> Result<Option<CapturedFrame>, String> {
        let Some(start) = self.drag_start else {
            return Ok(None);
        };
        if let Some(point) = self.canvas_to_image(x, y, canvas_width, canvas_height) {
            if self.tool == ToolKind::Pen {
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
        let Some(origin) = self.pending_text_origin.take() else {
            return Ok(None);
        };
        let annotation = Annotation::Text {
            origin,
            value: value.to_owned(),
            style: self.style,
        };
        if !annotation.is_meaningful() {
            return self.current_frame().map(Some);
        }
        self.apply_annotation(annotation)?;
        self.current_frame().map(Some)
    }

    pub fn undo(&mut self) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        if self.document.undo().is_none() {
            return Ok(None);
        }
        self.rebuild_committed()?;
        self.current_frame().map(Some)
    }

    pub fn redo(&mut self) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        if self.document.redo().is_none() {
            return Ok(None);
        }
        self.rebuild_committed()?;
        self.current_frame().map(Some)
    }

    pub fn clear(&mut self) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        self.document.clear();
        let Some(base) = self.base.as_ref() else {
            return Ok(None);
        };
        self.committed_rgba = Some(base.rgba().to_vec());
        self.current_frame().map(Some)
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
        render_annotation_in_place(pixels, base.width(), base.height(), &annotation)
            .map_err(|error| error.to_string())?;
        self.document.push(annotation);
        Ok(())
    }

    fn rebuild_committed(&mut self) -> Result<(), String> {
        let base = self
            .base
            .as_ref()
            .ok_or_else(|| "editor has no captured image".to_owned())?;
        self.committed_rgba = Some(
            render_document(base.rgba(), base.width(), base.height(), &self.document)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    }

    fn preview_frame(&self) -> Result<CapturedFrame, String> {
        let base = self
            .base
            .as_ref()
            .ok_or_else(|| "editor has no captured image".to_owned())?;
        let mut pixels = self
            .committed_rgba
            .as_ref()
            .ok_or_else(|| "editor has no committed pixel buffer".to_owned())?
            .clone();
        if let Some(draft) = self.draft.as_ref() {
            render_annotation_in_place(&mut pixels, base.width(), base.height(), draft)
                .map_err(|error| error.to_string())?;
        }
        CapturedFrame::new(base.width(), base.height(), pixels).map_err(|error| error.to_string())
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
            ToolKind::Rectangle => Some(Annotation::Rectangle {
                rect,
                style: self.style,
            }),
            ToolKind::Ellipse => Some(Annotation::Ellipse {
                rect,
                style: self.style,
            }),
            ToolKind::Arrow => Some(Annotation::Arrow {
                from: start,
                to: end,
                style: self.style,
            }),
            ToolKind::Line => Some(Annotation::Line {
                from: start,
                to: end,
                style: self.style,
            }),
            ToolKind::Mosaic => Some(Annotation::Mosaic {
                rect,
                block_size: 14,
            }),
            ToolKind::Blur => Some(Annotation::Blur { rect, radius: 8 }),
            ToolKind::Pen | ToolKind::Text => None,
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
            ((x - offset_x) / scale).clamp(0.0, frame.width().saturating_sub(1) as f32),
            ((y - offset_y) / scale).clamp(0.0, frame.height().saturating_sub(1) as f32),
        ))
    }

    fn cancel_draft(&mut self) {
        self.draft = None;
        self.drag_start = None;
        self.pen_points.clear();
        self.pending_text_origin = None;
    }
}
