from __future__ import annotations

import ast
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "tools" / "i18n_inventory.txt"

entries: list[str] = []

for path in sorted((ROOT / "apps" / "desktop" / "ui").glob("*.slint")):
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        for match in re.finditer(r'"(?:\\.|[^"\\])*"', line):
            raw = match.group(0)
            try:
                value = ast.literal_eval(raw)
            except Exception:
                continue
            if not value:
                continue
            # Exclude paths, stable internal ids, colors, and glyph-only tokens.
            if value.endswith((".slint", ".svg", ".png")) or value.startswith(("#", "icons/")):
                continue
            if re.fullmatch(r"[a-z0-9_-]+", value) and value in {
                "general", "hotkeys", "capture", "export", "ocr", "about",
                "system", "light", "dark", "select", "rectangle", "arrow", "pen",
                "text", "mosaic", "line", "ellipse",
            }:
                continue
            entries.append(f"SLINT\t{path.relative_to(ROOT)}:{lineno}\t{value}")

for path in [ROOT / "apps" / "desktop" / "src" / "main.rs", ROOT / "apps" / "desktop" / "src" / "hotkeys.rs"]:
    lines = path.read_text(encoding="utf-8").splitlines()
    for lineno, line in enumerate(lines, 1):
        if not any(token in line for token in ("set_status_text", "show_system_notification", "format!(", "Err(\"", "Ok(\"")):
            continue
        for match in re.finditer(r'"(?:\\.|[^"\\])*"', line):
            raw = match.group(0)
            try:
                value = ast.literal_eval(raw)
            except Exception:
                continue
            if value and not value.startswith(("azusa-", "http", "--")):
                entries.append(f"RUST\t{path.relative_to(ROOT)}:{lineno}\t{value}")

OUT.write_text("\n".join(entries) + "\n", encoding="utf-8")
print(f"wrote {len(entries)} entries to {OUT}")
