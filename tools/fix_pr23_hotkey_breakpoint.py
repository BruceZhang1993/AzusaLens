from pathlib import Path

settings_path = Path("apps/desktop/ui/settings-view.slint")
settings = settings_path.read_text()
old = "private property <bool> narrow-layout: root.width < 840px;"
new = "private property <bool> narrow-layout: root.width < 876px;"
if settings.count(old) != 1:
    raise SystemExit(f"expected exactly one narrow breakpoint, found {settings.count(old)}")
settings_path.write_text(settings.replace(old, new, 1))

tests_path = Path("apps/desktop/src/overlay_tests.rs")
tests = tests_path.read_text()
old_test = '"private property <bool> narrow-layout: root.width < 840px",'
new_test = '"private property <bool> narrow-layout: root.width < 876px",'
if tests.count(old_test) != 1:
    raise SystemExit(f"expected exactly one breakpoint assertion, found {tests.count(old_test)}")
tests_path.write_text(tests.replace(old_test, new_test, 1))
