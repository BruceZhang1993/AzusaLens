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


annotation_path = Path("crates/annotation/src/lib.rs")
annotation = annotation_path.read_text()
old_document = '''#[derive(Debug, Default, Clone, PartialEq)]
pub struct AnnotationDocument {
    items: Vec<Annotation>,
    redo: Vec<Annotation>,
}

impl AnnotationDocument {
    pub fn push(&mut self, annotation: Annotation) {
        self.items.push(annotation);
        self.redo.clear();
    }

    pub fn undo(&mut self) -> Option<&Annotation> {
        let item = self.items.pop()?;
        self.redo.push(item);
        self.redo.last()
    }

    pub fn redo(&mut self) -> Option<&Annotation> {
        let item = self.redo.pop()?;
        self.items.push(item);
        self.items.last()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.redo.clear();
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.items.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[must_use]
    pub fn items(&self) -> &[Annotation] {
        &self.items
    }
}
'''
new_document = '''const MAX_HISTORY_ENTRIES: usize = 100;

#[derive(Debug, Clone, PartialEq)]
enum EditCommand {
    Insert { index: usize, annotation: Annotation },
    Remove { index: usize },
    Replace { index: usize, annotation: Annotation },
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
'''
annotation = replace_once(annotation, old_document, new_document, "annotation document")

test_anchor = '''    #[test]
    fn line_render_changes_pixels() {
'''
test_insert = '''    #[test]
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
        assert_eq!(document.items(), &[second.clone()]);
        assert!(document.undo());
        assert_eq!(document.items(), &[first.clone()]);
        assert!(document.redo());
        assert_eq!(document.items(), &[second.clone()]);

        assert!(document.remove(0));
        assert!(document.items().is_empty());
        assert!(document.undo());
        assert_eq!(document.items(), &[second.clone()]);

        document.extend([first.clone(), second.clone()]);
        assert_eq!(document.items().len(), 3);
        assert!(document.undo());
        assert_eq!(document.items(), &[second.clone()]);
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
'''
annotation = replace_once(annotation, test_anchor, test_insert, "annotation tests")
annotation_path.write_text(annotation)

editor_path = Path("apps/desktop/src/editor.rs")
editor = editor_path.read_text()
editor = replace_once(editor, 'const MAX_HISTORY_ENTRIES: usize = 100;\n', '', "editor history limit")
editor = sub_once(editor, r'\n    undo_history: Vec<AnnotationDocument>,\n    redo_history: Vec<AnnotationDocument>,', '', "editor history fields")
editor = sub_once(editor, r'\n            undo_history: Vec::new\(\),\n            redo_history: Vec::new\(\),', '', "editor history defaults")
editor = sub_once(editor, r'        self\.document\.clear\(\);\n        self\.undo_history\.clear\(\);\n        self\.redo_history\.clear\(\);', '        self.document.reset();', "editor reset")
editor = sub_once(editor, r'pub fn can_undo\(&self\) -> bool \{\n        !self\.undo_history\.is_empty\(\)\n    \}', 'pub fn can_undo(&self) -> bool {\n        self.document.can_undo()\n    }', "editor can undo")
editor = sub_once(editor, r'pub fn can_redo\(&self\) -> bool \{\n        !self\.redo_history\.is_empty\(\)\n    \}', 'pub fn can_redo(&self) -> bool {\n        self.document.can_redo()\n    }', "editor can redo")

record_count = editor.count('        self.record_history();\n')
if record_count < 4:
    raise SystemExit(f"editor record_history call count unexpectedly low: {record_count}")
editor = editor.replace('        self.record_history();\n', '')
editor = replace_once(
    editor,
    '''        for annotation in annotations {
            self.document.push(annotation);
        }
''',
    '''        self.document.extend(annotations);
''',
    "OCR batch history",
)

editor = sub_once(
    editor,
    r'    pub fn delete_selected\(&mut self\) -> Result<Option<CapturedFrame>, String> \{.*?\n    \}\n\n    pub fn undo',
    '''    pub fn delete_selected(&mut self) -> Result<Option<CapturedFrame>, String> {
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

    pub fn undo''',
    "editor delete",
    re.S,
)
editor = sub_once(
    editor,
    r'    pub fn undo\(&mut self\) -> Result<Option<CapturedFrame>, String> \{.*?\n    \}\n\n    pub fn redo',
    '''    pub fn undo(&mut self) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        if !self.document.undo() {
            return Ok(None);
        }
        self.selected_index = None;
        self.rebuild_committed()?;
        self.recompute_sequence_next();
        self.current_frame().map(Some)
    }

    pub fn redo''',
    "editor undo",
    re.S,
)
editor = sub_once(
    editor,
    r'    pub fn redo\(&mut self\) -> Result<Option<CapturedFrame>, String> \{.*?\n    \}\n\n    pub fn clear',
    '''    pub fn redo(&mut self) -> Result<Option<CapturedFrame>, String> {
        self.cancel_draft();
        if !self.document.redo() {
            return Ok(None);
        }
        self.selected_index = None;
        self.rebuild_committed()?;
        self.recompute_sequence_next();
        self.current_frame().map(Some)
    }

    pub fn clear''',
    "editor redo",
    re.S,
)
editor = sub_once(
    editor,
    r'    fn replace_annotation\(&mut self, index: usize, replacement: Annotation\) -> Result<\(\), String> \{.*?\n    \}\n\n    fn apply_annotation',
    '''    fn replace_annotation(&mut self, index: usize, replacement: Annotation) -> Result<(), String> {
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

    fn apply_annotation''',
    "editor replace",
    re.S,
)
editor = sub_once(
    editor,
    r'    fn record_history\(&mut self\) \{.*?\n    fn recompute_sequence_next',
    '    fn recompute_sequence_next',
    "editor history helpers",
    re.S,
)
editor = sub_once(
    editor,
    r'fn document_from_items\(items: impl IntoIterator<Item = Annotation>\) -> AnnotationDocument \{.*?\n\}\n\n',
    '',
    "editor document helper",
    re.S,
)

leftovers = [name for name in ("undo_history", "redo_history", "record_history", "snapshot_document", "document_from_items") if name in editor]
if leftovers:
    raise SystemExit(f"editor history leftovers: {leftovers}")
editor_path.write_text(editor)
