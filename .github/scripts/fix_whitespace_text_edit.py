from pathlib import Path

path = Path("apps/desktop/src/editor.rs")
source = path.read_text()
old = "if style.stroke_width == SEQUENCE_SENTINEL_STROKE || value.is_empty() {"
new = "if style.stroke_width == SEQUENCE_SENTINEL_STROKE || value.trim().is_empty() {"
if old not in source:
    raise SystemExit("text edit validation condition not found")
source = source.replace(old, new, 1)

anchor = '''    #[test]\n    fn sequence_markers_do_not_enter_text_edit_mode() {\n'''
test = '''    #[test]\n    fn whitespace_only_text_edit_is_rejected_without_history_entry() {\n        let mut editor = EditorSession::default();\n        editor.reset(frame(200, 100));\n        assert!(editor.set_tool("text"));\n        assert_eq!(\n            editor.begin_canvas(20.0, 20.0, 200.0, 100.0),\n            BeginResult::TextInput\n        );\n        editor\n            .commit_text("visible")\n            .expect("text creation should render");\n\n        assert!(editor.set_tool(SELECT_TOOL_ID));\n        editor.begin_canvas(20.0, 20.0, 200.0, 100.0);\n        editor\n            .end_canvas(20.0, 20.0, 200.0, 100.0)\n            .expect("first selection click should finish");\n        assert_eq!(\n            editor.begin_canvas(20.0, 20.0, 200.0, 100.0),\n            BeginResult::TextEdit\n        );\n        assert!(\n            editor\n                .commit_text("   \\t  ")\n                .expect("whitespace edit should be rejected")\n                .is_none()\n        );\n        match &editor.document.items()[0] {\n            Annotation::Text { value, .. } => assert_eq!(value, "visible"),\n            _ => panic!("expected text annotation"),\n        }\n\n        editor.undo().expect("only the original text creation should be undoable");\n        assert!(editor.document.items().is_empty());\n    }\n\n'''
if anchor not in source:
    raise SystemExit("test insertion anchor not found")
source = source.replace(anchor, test + anchor, 1)
path.write_text(source)
