use azusa_annotation::{Annotation, AnnotationDocument, AnnotationStyle, Point};

fn line(offset: f32) -> Annotation {
    Annotation::Line {
        from: Point::new(offset, offset),
        to: Point::new(offset + 8.0, offset + 8.0),
        style: AnnotationStyle::default(),
    }
}

#[test]
fn divergent_edit_clears_redo_history() {
    let mut document = AnnotationDocument::default();
    document.push(line(0.0));
    document.push(line(10.0));

    assert!(document.undo());
    assert!(document.can_redo());

    document.push(line(20.0));

    assert!(!document.can_redo());
    assert!(!document.redo());
    assert_eq!(document.items().len(), 2);
}

#[test]
fn command_history_keeps_only_the_latest_hundred_edits() {
    let mut document = AnnotationDocument::default();
    for index in 0..101 {
        document.push(line(index as f32));
    }

    for _ in 0..100 {
        assert!(document.undo());
    }

    assert!(!document.can_undo());
    assert_eq!(document.items().len(), 1);
}
