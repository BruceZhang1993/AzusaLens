use std::time::{Duration, Instant};

use azusa_annotation::{
    Annotation, AnnotationDocument, AnnotationStyle, Color, Point, Rect, ToolKind,
    render_annotation_in_place, render_document,
};
use azusa_capture::CapturedFrame;

const MAX_PREVIEW_DIMENSION: u32 = 1600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginResult {
    Drawing,
    TextInput,
    Ignored,
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
        render_annotation_in_place(&mut self.committed_rgba, self.width, self.height, &scaled)
            .map_err(|error| error.to_string())
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
            render_annotation_in_place(&mut pixels, self.width, self.height, &scaled)
                .map_err(|error| error.to_string())?;
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
    tool: ToolKind,
    style: AnnotationStyle,
    last_preview_at: Option<Instant>,
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
            tool: ToolKind::Rectangle,
            style: AnnotationStyle::default(),
            last_preview_at: None,
        }
    }
}

impl EditorSession {
    pub fn reset(&mut self, frame: CapturedFrame) {
        self.preview = Some(PreviewSurface::new(&frame));
        self.committed_rgba = Some(frame.rgba().to_vec());
        self.base = Some(frame);
        self.document.clear();
        self.cancel_draft();
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
        if let Some(preview) = self.preview.as_mut() {
            preview.reset();
        }
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
        if let Some(preview) = self.preview.as_mut() {
            preview.apply(&annotation)?;
        }
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
        if let Some(preview) = self.preview.as_mut() {
            preview.rebuild(&self.document)?;
        }
        Ok(())
    }

    fn preview_frame(&self) -> Result<CapturedFrame, String> {
        self.preview
            .as_ref()
            .ok_or_else(|| "editor has no preview surface".to_owned())?
            .frame_with_draft(self.draft.as_ref())
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
            ((x - offset_x) / scale).clamp(0.0, frame.width() as f32),
            ((y - offset_y) / scale).clamp(0.0, frame.height() as f32),
        ))
    }

    fn preview_interval(&self) -> Duration {
        match self.tool {
            ToolKind::Blur => Duration::from_millis(80),
            ToolKind::Mosaic => Duration::from_millis(50),
            _ => Duration::from_millis(33),
        }
    }

    fn cancel_draft(&mut self) {
        self.draft = None;
        self.drag_start = None;
        self.pen_points.clear();
        self.pending_text_origin = None;
        self.last_preview_at = None;
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
        stroke_width: (style.stroke_width * style_scale).max(1.0),
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
}
