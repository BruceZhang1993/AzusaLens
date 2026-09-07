from pathlib import Path

path = Path("apps/desktop/ui/capture-overlay.slint")
text = path.read_text(encoding="utf-8")
needle = "    private function "
count = text.count(needle)
if count != 2:
    raise RuntimeError(f"expected exactly two private functions, found {count}")
path.write_text(text.replace(needle, "    function "), encoding="utf-8")
print("fixed Slint helper function syntax")
