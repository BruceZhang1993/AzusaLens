//! Non-destructive annotation scene model.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Annotation {
    Rectangle(Rect),
    Arrow { from: Point, to: Point },
    Pen(Vec<Point>),
    Text { origin: Point, value: String },
    Mosaic(Rect),
    Blur(Rect),
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct AnnotationDocument {
    items: Vec<Annotation>,
}

impl AnnotationDocument {
    pub fn push(&mut self, annotation: Annotation) {
        self.items.push(annotation);
    }

    pub fn undo(&mut self) -> Option<Annotation> {
        self.items.pop()
    }

    #[must_use]
    pub fn items(&self) -> &[Annotation] {
        &self.items
    }
}
