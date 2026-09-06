use std::time::{Duration, Instant};

use azusa_annotation::{
    Annotation, AnnotationDocument, AnnotationStyle, Color, Point, Rect, ToolKind,
    render_annotation_in_place,
};
use azusa_capture::CapturedFrame;

const MAX_PREVIEW_DIMENSION: u32 = 1600;
const SEQUENCE_TOOL_ID: &str = "number";
const SEQUENCE_SENTINEL_STROKE: f32 = -1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginResult {
    Drawing,
    TextInput,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTool {
    Annotation(ToolKind),
    Sequence,
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
    tool: ActiveTool,
    style: AnnotationStyle,
    sequence_next: u32,
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
            tool: ActiveTool::Annotation(ToolKind::Rectangle),
            style: AnnotationStyle::default(),
            sequence_next: 1,
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
        self.sequence_next = 1;
        self.cancel_draft();
    }

    pub fn set_tool(&mut self, id: &str) -> bool {
        let tool = if id == SEQUENCE_TOOL_ID {
            ActiveTool::Sequence
        } else {
            let Some(tool) = ToolKind::from_id(id) else {
                return false;
            };
            ActiveTool::Annotation(tool)
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
        let Some(start) = self.drag_start else {
            return Ok(None);
        };
        let Some(point) = self.canvas_to_image(x, y, canvas_width, canvas_height) else {
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
        self.recompute_sequence_next();
        self.current_frame().map(Some)
    }

    pub fn redo(&mut self) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        if self.document.redo().is_none() {
            return Ok(None);
        }
        self.rebuild_committed()?;
        self.recompute_sequence_next();
        self.current_frame().map(Some)
    }

    pub fn clear(&mut self) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        self.document.clear();
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
            ActiveTool::Annotation(ToolKind::Pen | ToolKind::Text) => None,
        }
    }

    fn sequence_annotation(&self, center: Point) -> Annotation {
        Annotation::Text {
            // Sequence markers temporarily reuse Text as a document payload so they stay a single
            // undo/redo item. The negative sentinel routes them to the dedicated badge renderer.
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
        self.last_preview_at = None;
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
