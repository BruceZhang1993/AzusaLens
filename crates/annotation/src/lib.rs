//! Non-destructive annotation document and software renderer.
//!
//! The annotation crate owns geometry, styling, history, and flattening. The
//! desktop UI only translates pointer input into annotations, which keeps the
//! editor extensible without coupling tool behavior to Slint components.

use std::{
    fmt,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use fontdue::{Font, FontSettings};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn distance_to(self, other: Self) -> f32 {
        (other.x - self.x).hypot(other.y - self.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[must_use]
    pub const fn new(origin: Point, width: f32, height: f32) -> Self {
        Self {
            origin,
            width,
            height,
        }
    }

    #[must_use]
    pub fn from_points(a: Point, b: Point) -> Self {
        let left = a.x.min(b.x);
        let top = a.y.min(b.y);
        Self {
            origin: Point::new(left, top),
            width: (a.x - b.x).abs(),
            height: (a.y - b.y).abs(),
        }
    }

    #[must_use]
    pub fn is_visible(self) -> bool {
        self.width >= 1.0 && self.height >= 1.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const RED: Self = Self::rgb(239, 68, 68);
    pub const ORANGE: Self = Self::rgb(249, 115, 22);
    pub const YELLOW: Self = Self::rgb(234, 179, 8);
    pub const GREEN: Self = Self::rgb(34, 197, 94);
    pub const BLUE: Self = Self::rgb(59, 130, 246);
    pub const PURPLE: Self = Self::rgb(139, 92, 246);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const BLACK: Self = Self::rgb(17, 24, 39);

    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnnotationStyle {
    pub color: Color,
    pub stroke_width: f32,
    pub font_size: f32,
}

impl Default for AnnotationStyle {
    fn default() -> Self {
        Self {
            color: Color::RED,
            stroke_width: 4.0,
            font_size: 28.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Rectangle,
    Ellipse,
    Arrow,
    Line,
    Pen,
    Text,
    Mosaic,
    Blur,
}

impl ToolKind {
    pub const ALL: [Self; 8] = [
        Self::Rectangle,
        Self::Ellipse,
        Self::Arrow,
        Self::Line,
        Self::Pen,
        Self::Text,
        Self::Mosaic,
        Self::Blur,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Rectangle => "rectangle",
            Self::Ellipse => "ellipse",
            Self::Arrow => "arrow",
            Self::Line => "line",
            Self::Pen => "pen",
            Self::Text => "text",
            Self::Mosaic => "mosaic",
            Self::Blur => "blur",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.id() == id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Annotation {
    Rectangle {
        rect: Rect,
        style: AnnotationStyle,
    },
    Ellipse {
        rect: Rect,
        style: AnnotationStyle,
    },
    Arrow {
        from: Point,
        to: Point,
        style: AnnotationStyle,
    },
    Line {
        from: Point,
        to: Point,
        style: AnnotationStyle,
    },
    Pen {
        points: Vec<Point>,
        style: AnnotationStyle,
    },
    Text {
        origin: Point,
        value: String,
        style: AnnotationStyle,
    },
    Mosaic {
        rect: Rect,
        block_size: u32,
    },
    Blur {
        rect: Rect,
        radius: u32,
    },
}

impl Annotation {
    #[must_use]
    pub fn is_meaningful(&self) -> bool {
        match self {
            Self::Rectangle { rect, .. }
            | Self::Ellipse { rect, .. }
            | Self::Mosaic { rect, .. }
            | Self::Blur { rect, .. } => rect.is_visible(),
            Self::Arrow { from, to, .. } | Self::Line { from, to, .. } => {
                from.distance_to(*to) >= 1.0
            }
            Self::Pen { points, .. } => points.len() >= 2,
            Self::Text { value, .. } => !value.trim().is_empty(),
        }
    }
}

const MAX_HISTORY_ENTRIES: usize = 100;

#[derive(Debug, Clone, PartialEq)]
enum EditCommand {
    Insert {
        index: usize,
        annotation: Annotation,
    },
    Remove {
        index: usize,
    },
    Replace {
        index: usize,
        annotation: Annotation,
    },
    Batch(Vec<Self>),
}

impl EditCommand {
    fn apply(self, items: &mut Vec<Annotation>) -> Self {
        match self {
            Self::Insert { index, annotation } => {
                debug_assert!(index <= items.len());
                items.insert(index, annotation);
                Self::Remove { index }
            }
            Self::Remove { index } => {
                debug_assert!(index < items.len());
                let annotation = items.remove(index);
                Self::Insert { index, annotation }
            }
            Self::Replace { index, annotation } => {
                debug_assert!(index < items.len());
                let previous = std::mem::replace(&mut items[index], annotation);
                Self::Replace {
                    index,
                    annotation: previous,
                }
            }
            Self::Batch(commands) => {
                let inverse = commands
                    .into_iter()
                    .map(|command| command.apply(items))
                    .collect::<Vec<_>>();
                Self::Batch(inverse.into_iter().rev().collect())
            }
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct AnnotationDocument {
    items: Vec<Annotation>,
    undo: Vec<EditCommand>,
    redo: Vec<EditCommand>,
}

impl AnnotationDocument {
    pub fn push(&mut self, annotation: Annotation) {
        let index = self.items.len();
        self.execute(EditCommand::Insert { index, annotation });
    }

    pub fn extend(&mut self, annotations: impl IntoIterator<Item = Annotation>) {
        let start = self.items.len();
        let commands = annotations
            .into_iter()
            .enumerate()
            .map(|(offset, annotation)| EditCommand::Insert {
                index: start + offset,
                annotation,
            })
            .collect::<Vec<_>>();
        if !commands.is_empty() {
            self.execute(EditCommand::Batch(commands));
        }
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.execute(EditCommand::Remove { index });
        true
    }

    pub fn replace(&mut self, index: usize, annotation: Annotation) -> bool {
        let Some(current) = self.items.get(index) else {
            return false;
        };
        if *current == annotation {
            return false;
        }
        self.execute(EditCommand::Replace { index, annotation });
        true
    }

    pub fn undo(&mut self) -> bool {
        let Some(command) = self.undo.pop() else {
            return false;
        };
        let inverse = command.apply(&mut self.items);
        self.redo.push(inverse);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(command) = self.redo.pop() else {
            return false;
        };
        let inverse = command.apply(&mut self.items);
        self.push_undo(inverse);
        true
    }

    pub fn clear(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        let commands = (0..self.items.len())
            .rev()
            .map(|index| EditCommand::Remove { index })
            .collect();
        self.execute(EditCommand::Batch(commands));
        true
    }

    pub fn reset(&mut self) {
        self.items.clear();
        self.undo.clear();
        self.redo.clear();
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[must_use]
    pub fn items(&self) -> &[Annotation] {
        &self.items
    }

    fn execute(&mut self, command: EditCommand) {
        let inverse = command.apply(&mut self.items);
        self.push_undo(inverse);
        self.redo.clear();
    }

    fn push_undo(&mut self, command: EditCommand) {
        if self.undo.len() >= MAX_HISTORY_ENTRIES {
            self.undo.remove(0);
        }
        self.undo.push(command);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    InvalidBuffer {
        width: u32,
        height: u32,
        actual_len: usize,
        expected_len: usize,
    },
    FontUnavailable,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBuffer {
                width,
                height,
                actual_len,
                expected_len,
            } => write!(
                f,
                "invalid RGBA buffer {width}x{height}: got {actual_len} bytes, expected {expected_len}"
            ),
            Self::FontUnavailable => f.write_str(
                "no usable system font was found; set AZUSA_LENS_FONT to a TTF/OTF font file",
            ),
        }
    }
}

impl std::error::Error for RenderError {}

pub fn render_document(
    base_rgba: &[u8],
    width: u32,
    height: u32,
    document: &AnnotationDocument,
) -> Result<Vec<u8>, RenderError> {
    validate_buffer(base_rgba, width, height)?;
    let mut pixels = base_rgba.to_vec();
    for annotation in document.items() {
        render_annotation_in_place(&mut pixels, width, height, annotation)?;
    }
    Ok(pixels)
}

pub fn render_annotation_in_place(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    annotation: &Annotation,
) -> Result<(), RenderError> {
    validate_buffer(pixels, width, height)?;

    match annotation {
        Annotation::Rectangle { rect, style } => {
            draw_rectangle(pixels, width, height, *rect, *style)
        }
        Annotation::Ellipse { rect, style } => draw_ellipse(pixels, width, height, *rect, *style),
        Annotation::Arrow { from, to, style } => {
            draw_arrow(pixels, width, height, *from, *to, *style)
        }
        Annotation::Line { from, to, style } => {
            draw_line(pixels, width, height, *from, *to, *style)
        }
        Annotation::Pen { points, style } => {
            for segment in points.windows(2) {
                draw_line(pixels, width, height, segment[0], segment[1], *style);
            }
        }
        Annotation::Text {
            origin,
            value,
            style,
        } => draw_text(pixels, width, height, *origin, value, *style)?,
        Annotation::Mosaic { rect, block_size } => {
            apply_mosaic(pixels, width, height, *rect, (*block_size).max(2))
        }
        Annotation::Blur { rect, radius } => {
            apply_blur(pixels, width, height, *rect, (*radius).max(1))
        }
    }

    Ok(())
}

fn validate_buffer(pixels: &[u8], width: u32, height: u32) -> Result<(), RenderError> {
    let expected_len = width as usize * height as usize * 4;
    if width == 0 || height == 0 || pixels.len() != expected_len {
        return Err(RenderError::InvalidBuffer {
            width,
            height,
            actual_len: pixels.len(),
            expected_len,
        });
    }
    Ok(())
}

fn draw_rectangle(pixels: &mut [u8], width: u32, height: u32, rect: Rect, style: AnnotationStyle) {
    if !rect.is_visible() {
        return;
    }
    let left = rect.origin.x;
    let top = rect.origin.y;
    let right = left + rect.width;
    let bottom = top + rect.height;
    draw_line(
        pixels,
        width,
        height,
        Point::new(left, top),
        Point::new(right, top),
        style,
    );
    draw_line(
        pixels,
        width,
        height,
        Point::new(right, top),
        Point::new(right, bottom),
        style,
    );
    draw_line(
        pixels,
        width,
        height,
        Point::new(right, bottom),
        Point::new(left, bottom),
        style,
    );
    draw_line(
        pixels,
        width,
        height,
        Point::new(left, bottom),
        Point::new(left, top),
        style,
    );
}

fn draw_ellipse(pixels: &mut [u8], width: u32, height: u32, rect: Rect, style: AnnotationStyle) {
    if !rect.is_visible() {
        return;
    }

    let center = Point::new(
        rect.origin.x + rect.width / 2.0,
        rect.origin.y + rect.height / 2.0,
    );
    let radius_x = rect.width / 2.0;
    let radius_y = rect.height / 2.0;
    let steps = (((rect.width + rect.height) * 0.75).round() as usize).clamp(32, 512);
    let mut previous = Point::new(center.x + radius_x, center.y);

    for step in 1..=steps {
        let angle = std::f32::consts::TAU * step as f32 / steps as f32;
        let point = Point::new(
            center.x + radius_x * angle.cos(),
            center.y + radius_y * angle.sin(),
        );
        draw_line(pixels, width, height, previous, point, style);
        previous = point;
    }
}

fn draw_arrow(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    from: Point,
    to: Point,
    style: AnnotationStyle,
) {
    draw_line(pixels, width, height, from, to, style);
    let angle = (to.y - from.y).atan2(to.x - from.x);
    let head = (style.stroke_width * 5.0).clamp(12.0, 32.0);
    let spread = 0.58;
    let left = Point::new(
        to.x - head * (angle - spread).cos(),
        to.y - head * (angle - spread).sin(),
    );
    let right = Point::new(
        to.x - head * (angle + spread).cos(),
        to.y - head * (angle + spread).sin(),
    );
    draw_line(pixels, width, height, to, left, style);
    draw_line(pixels, width, height, to, right, style);
}

fn draw_line(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    from: Point,
    to: Point,
    style: AnnotationStyle,
) {
    let distance = from.distance_to(to);
    let steps = distance.ceil().max(1.0) as usize;
    let radius = (style.stroke_width.max(1.0) / 2.0).max(0.5);

    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        draw_disc(
            pixels,
            width,
            height,
            from.x + (to.x - from.x) * t,
            from.y + (to.y - from.y) * t,
            radius,
            style.color,
        );
    }
}

fn draw_disc(
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
                blend_pixel(pixels, width, x as u32, y as u32, color, 255);
            }
        }
    }
}

fn draw_text(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    origin: Point,
    value: &str,
    style: AnnotationStyle,
) -> Result<(), RenderError> {
    let font = system_font().ok_or(RenderError::FontUnavailable)?;
    let size = style.font_size.clamp(10.0, 160.0);
    let line_height = size * 1.25;
    let mut pen_x = origin.x;
    let mut baseline = origin.y + size;

    for character in value.chars() {
        if character == '\n' {
            pen_x = origin.x;
            baseline += line_height;
            continue;
        }

        let (metrics, bitmap) = font.rasterize(character, size);
        let glyph_x = pen_x + metrics.xmin as f32;
        let glyph_y = baseline - metrics.height as f32 - metrics.ymin as f32;

        for glyph_row in 0..metrics.height {
            for glyph_col in 0..metrics.width {
                let alpha = bitmap[glyph_row * metrics.width + glyph_col];
                if alpha == 0 {
                    continue;
                }
                let x = glyph_x.round() as i32 + glyph_col as i32;
                let y = glyph_y.round() as i32 + glyph_row as i32;
                if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                    blend_pixel(pixels, width, x as u32, y as u32, style.color, alpha);
                }
            }
        }

        pen_x += metrics.advance_width;
    }

    Ok(())
}

fn blend_pixel(pixels: &mut [u8], width: u32, x: u32, y: u32, color: Color, coverage: u8) {
    let index = ((y * width + x) * 4) as usize;
    let source_alpha = u16::from(color.a) * u16::from(coverage) / 255;
    let inverse = 255_u16.saturating_sub(source_alpha);
    pixels[index] =
        ((u16::from(color.r) * source_alpha + u16::from(pixels[index]) * inverse) / 255) as u8;
    pixels[index + 1] =
        ((u16::from(color.g) * source_alpha + u16::from(pixels[index + 1]) * inverse) / 255) as u8;
    pixels[index + 2] =
        ((u16::from(color.b) * source_alpha + u16::from(pixels[index + 2]) * inverse) / 255) as u8;
    pixels[index + 3] = 255;
}

fn apply_mosaic(pixels: &mut [u8], width: u32, height: u32, rect: Rect, block_size: u32) {
    let Some((left, top, right, bottom)) = clamped_rect(rect, width, height) else {
        return;
    };

    let block = block_size as usize;
    for block_y in (top as usize..bottom as usize).step_by(block) {
        for block_x in (left as usize..right as usize).step_by(block) {
            let end_y = (block_y + block).min(bottom as usize);
            let end_x = (block_x + block).min(right as usize);
            let mut sum = [0_u64; 4];
            let mut count = 0_u64;

            for y in block_y..end_y {
                for x in block_x..end_x {
                    let index = (y * width as usize + x) * 4;
                    for channel in 0..4 {
                        sum[channel] += u64::from(pixels[index + channel]);
                    }
                    count += 1;
                }
            }

            if count == 0 {
                continue;
            }
            let average = [
                (sum[0] / count) as u8,
                (sum[1] / count) as u8,
                (sum[2] / count) as u8,
                (sum[3] / count) as u8,
            ];
            for y in block_y..end_y {
                for x in block_x..end_x {
                    let index = (y * width as usize + x) * 4;
                    pixels[index..index + 4].copy_from_slice(&average);
                }
            }
        }
    }
}

fn apply_blur(pixels: &mut [u8], width: u32, height: u32, rect: Rect, radius: u32) {
    let Some((left, top, right, bottom)) = clamped_rect(rect, width, height) else {
        return;
    };
    let region_width = (right - left) as usize;
    let region_height = (bottom - top) as usize;
    if region_width == 0 || region_height == 0 {
        return;
    }

    let mut horizontal = pixels.to_vec();
    let radius = radius as usize;

    for y in top as usize..bottom as usize {
        let mut prefix = vec![[0_u64; 4]; region_width + 1];
        for local_x in 0..region_width {
            let index = (y * width as usize + left as usize + local_x) * 4;
            for channel in 0..4 {
                prefix[local_x + 1][channel] =
                    prefix[local_x][channel] + u64::from(pixels[index + channel]);
            }
        }
        for local_x in 0..region_width {
            let start = local_x.saturating_sub(radius);
            let end = (local_x + radius + 1).min(region_width);
            let count = (end - start) as u64;
            let index = (y * width as usize + left as usize + local_x) * 4;
            for channel in 0..4 {
                horizontal[index + channel] =
                    ((prefix[end][channel] - prefix[start][channel]) / count) as u8;
            }
        }
    }

    for x in left as usize..right as usize {
        let mut prefix = vec![[0_u64; 4]; region_height + 1];
        for local_y in 0..region_height {
            let index = ((top as usize + local_y) * width as usize + x) * 4;
            for channel in 0..4 {
                prefix[local_y + 1][channel] =
                    prefix[local_y][channel] + u64::from(horizontal[index + channel]);
            }
        }
        for local_y in 0..region_height {
            let start = local_y.saturating_sub(radius);
            let end = (local_y + radius + 1).min(region_height);
            let count = (end - start) as u64;
            let index = ((top as usize + local_y) * width as usize + x) * 4;
            for channel in 0..4 {
                pixels[index + channel] =
                    ((prefix[end][channel] - prefix[start][channel]) / count) as u8;
            }
        }
    }
}

fn clamped_rect(rect: Rect, width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    if !rect.is_visible() {
        return None;
    }
    let left = rect.origin.x.floor().max(0.0).min(width as f32) as u32;
    let top = rect.origin.y.floor().max(0.0).min(height as f32) as u32;
    let right = (rect.origin.x + rect.width)
        .ceil()
        .max(0.0)
        .min(width as f32) as u32;
    let bottom = (rect.origin.y + rect.height)
        .ceil()
        .max(0.0)
        .min(height as f32) as u32;
    (right > left && bottom > top).then_some((left, top, right, bottom))
}

static SYSTEM_FONT: OnceLock<Option<Font>> = OnceLock::new();

fn system_font() -> Option<&'static Font> {
    SYSTEM_FONT.get_or_init(load_system_font).as_ref()
}

fn load_system_font() -> Option<Font> {
    for path in preferred_font_paths() {
        if let Some(font) = load_font(&path) {
            return Some(font);
        }
    }

    for directory in fallback_font_directories() {
        if let Some(path) = find_first_font(&directory, 4)
            && let Some(font) = load_font(&path)
        {
            return Some(font);
        }
    }
    None
}

fn preferred_font_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("AZUSA_LENS_FONT") {
        paths.push(PathBuf::from(path));
    }

    #[cfg(target_os = "windows")]
    {
        let fonts = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("Fonts");
        paths.extend([
            fonts.join("msyh.ttc"),
            fonts.join("segoeui.ttf"),
            fonts.join("arial.ttf"),
        ]);
    }

    #[cfg(target_os = "macos")]
    paths.extend([
        PathBuf::from("/System/Library/Fonts/PingFang.ttc"),
        PathBuf::from("/System/Library/Fonts/SFNS.ttf"),
        PathBuf::from("/Library/Fonts/Arial Unicode.ttf"),
    ]);

    #[cfg(target_os = "linux")]
    paths.extend([
        PathBuf::from("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
    ]);

    paths
}

fn fallback_font_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();

    #[cfg(target_os = "windows")]
    if let Some(windir) = std::env::var_os("WINDIR") {
        directories.push(PathBuf::from(windir).join("Fonts"));
    }

    #[cfg(target_os = "macos")]
    directories.extend([
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
    ]);

    #[cfg(target_os = "linux")]
    directories.extend([
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ]);

    if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".local/share/fonts"));
    }
    directories
}

fn load_font(path: &Path) -> Option<Font> {
    let bytes = std::fs::read(path).ok()?;
    Font::from_bytes(bytes, FontSettings::default()).ok()
}

fn find_first_font(directory: &Path, depth: u8) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(directory).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_first_font(&path, depth - 1) {
                return Some(found);
            }
            continue;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if matches!(
            extension.to_ascii_lowercase().as_str(),
            "ttf" | "otf" | "ttc"
        ) {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white_canvas(width: u32, height: u32) -> Vec<u8> {
        vec![255; width as usize * height as usize * 4]
    }

    #[test]
    fn rect_normalizes_drag_direction() {
        let rect = Rect::from_points(Point::new(8.0, 10.0), Point::new(2.0, 4.0));
        assert_eq!(rect.origin, Point::new(2.0, 4.0));
        assert_eq!(rect.width, 6.0);
        assert_eq!(rect.height, 6.0);
    }

    #[test]
    fn history_supports_undo_and_redo() {
        let mut document = AnnotationDocument::default();
        document.push(Annotation::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(8.0, 8.0),
            style: AnnotationStyle::default(),
        });
        assert!(document.can_undo());
        assert!(!document.can_redo());

        document.undo();
        assert!(!document.can_undo());
        assert!(document.can_redo());

        document.redo();
        assert!(document.can_undo());
        assert!(!document.can_redo());
    }

    #[test]
    fn history_tracks_replace_remove_batch_and_clear() {
        let mut document = AnnotationDocument::default();
        let first = Annotation::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(8.0, 8.0),
            style: AnnotationStyle::default(),
        };
        let second = Annotation::Rectangle {
            rect: Rect::new(Point::new(2.0, 3.0), 10.0, 12.0),
            style: AnnotationStyle::default(),
        };

        document.push(first.clone());
        assert!(document.replace(0, second.clone()));
        assert_eq!(document.items(), std::slice::from_ref(&second));
        assert!(document.undo());
        assert_eq!(document.items(), std::slice::from_ref(&first));
        assert!(document.redo());
        assert_eq!(document.items(), std::slice::from_ref(&second));

        assert!(document.remove(0));
        assert!(document.items().is_empty());
        assert!(document.undo());
        assert_eq!(document.items(), std::slice::from_ref(&second));

        document.extend([first.clone(), second.clone()]);
        assert_eq!(document.items().len(), 3);
        assert!(document.undo());
        assert_eq!(document.items(), std::slice::from_ref(&second));
        assert!(document.redo());
        assert_eq!(document.items().len(), 3);

        assert!(document.clear());
        assert!(document.items().is_empty());
        assert!(document.undo());
        assert_eq!(document.items().len(), 3);
    }

    #[test]
    fn reset_discards_items_and_history() {
        let mut document = AnnotationDocument::default();
        document.push(Annotation::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(8.0, 8.0),
            style: AnnotationStyle::default(),
        });
        document.reset();
        assert!(document.items().is_empty());
        assert!(!document.can_undo());
        assert!(!document.can_redo());
    }

    #[test]
    fn line_render_changes_pixels() {
        let mut pixels = white_canvas(32, 32);
        render_annotation_in_place(
            &mut pixels,
            32,
            32,
            &Annotation::Line {
                from: Point::new(2.0, 2.0),
                to: Point::new(28.0, 28.0),
                style: AnnotationStyle::default(),
            },
        )
        .expect("line should render");
        assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[0] < 255));
    }

    #[test]
    fn mosaic_only_changes_selected_region() {
        let mut pixels = Vec::new();
        for y in 0_u8..8 {
            for x in 0_u8..8 {
                pixels.extend_from_slice(&[x * 20, y * 20, 100, 255]);
            }
        }
        let outside = pixels[0..4].to_vec();
        render_annotation_in_place(
            &mut pixels,
            8,
            8,
            &Annotation::Mosaic {
                rect: Rect::new(Point::new(2.0, 2.0), 4.0, 4.0),
                block_size: 4,
            },
        )
        .expect("mosaic should render");
        assert_eq!(&pixels[0..4], outside.as_slice());
        let first = ((2 * 8 + 2) * 4) as usize;
        let second = ((2 * 8 + 3) * 4) as usize;
        assert_eq!(&pixels[first..first + 4], &pixels[second..second + 4]);
    }
}
