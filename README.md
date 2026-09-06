# AzusaOCR

AzusaOCR is an early Rust desktop application for native cross-platform screenshots, local OCR, annotation, and future extensions.

## Current milestone: Interactive local OCR

The desktop validation app now has a complete capture-to-edit path with local multilingual OCR:

- Rust 2024 workspace
- Slint desktop UI
- Region capture on Windows, macOS, Linux X11, and native Wayland portal sessions
- Global PrtSc shortcut and system tray lifecycle
- Non-destructive annotation document with undo/redo
- Rectangle, ellipse, arrow, line, freehand pen, text, sequence number, mosaic, and blur tools
- Select existing annotations with geometry-aware hit testing that follows rendered shapes and arrowheads
- Move annotations and resize them with eight handles
- Edit the selected object's color and size, or delete it
- Fit-to-view editing with 1×–8× zoom and bounded viewport panning
- Mouse-wheel zoom, middle/right-button drag panning, and keyboard zoom shortcuts
- Stable screen-space selection and resize hit targets at every zoom level
- Keyboard shortcuts for undo/redo, copy, save, delete, cancel, zoom, and fit-to-view
- Live drag preview while drawing or transforming annotations
- Flatten edited pixels for clipboard copy and PNG export without selection chrome
- Fast local OCR using PP-OCRv6 Small through the Rust `ocr-rs`/MNN runtime
- Simplified/Traditional Chinese, English, Japanese, and the additional Latin-script languages supported by PP-OCRv6 Small
- Transparent OCR text layer that stays aligned while zooming and panning
- OCR quadrilateral geometry is preserved for rotated/skewed text, with rectangular fallback when unavailable
- Hover recognized text and drag across OCR lines/blocks to create a reading-order selection
- Right-click selected OCR text to copy it or create normal editable text annotations
- Ctrl/Cmd+C copies OCR text while a text-layer selection is active; **Copy all text** copies the full OCR result
- OCR-to-text conversion uses image-space geometry, estimates font size from each OCR line, and participates in editor undo/redo as one transaction
- **Settings > OCR models** for explicit model download, enable/switch, and deletion
- No automatic model download when OCR is started; missing-model OCR opens model management instead
- Persistent active-model selection and `AZUSAOCR_OCR_MODEL_DIR` storage override
- OCR engine/model catalog kept independent so GLM-OCR and additional fast models can be added later without replacing the editor integration
- VS Code + CodeLLDB debug configuration
- GitHub Actions checks on Windows, macOS, and Linux

The capture, annotation, and OCR layers are intentionally separated. `azusa-capture` owns platform screenshot acquisition, `azusa-annotation` owns annotation geometry/history/software rendering, and `azusa-ocr` owns OCR image/result contracts, the model catalog/manager, and inference backends. Selection chrome, the interactive OCR text layer, and viewport transforms stay in the Slint presentation layer, while annotation geometry, OCR quadrilaterals/bounds, export pixels, and future GLM-OCR results remain in stable image coordinates.

## Requirements

- Rust 1.98.1

Linux requires the native development packages used by Slint and the current capture adapter. Debian/Ubuntu example:

```bash
sudo apt-get install pkg-config libclang-dev libxcb1-dev libxrandr-dev libdbus-1-dev \
  libpipewire-0.3-dev libwayland-dev libegl-dev libgbm-dev libx11-xcb-dev xinput \
  libxcursor-dev libxkbcommon-x11-dev libxkbcommon-dev libx11-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libfontconfig-dev
```

## Run

```bash
cargo run -p azusaocr-desktop
```

Press **PrtSc** or choose **New capture**, select a region, then annotate it directly in the editor. Switch to **Select** to move, resize, restyle, or delete an existing annotation. Use the mouse wheel or **Ctrl/Cmd + Plus/Minus** to zoom, middle/right-button drag to pan while zoomed, and **Ctrl/Cmd + 0** or **Fit** to return to fit-to-view.

Before the first OCR run, open **Settings > OCR models**, choose **Download** for a model, and then choose **Enable**. Downloading does not automatically activate a model. The active choice persists between launches and installed models can be switched or deleted from the same page.

Choose **OCR** to recognize all text in the current edited screenshot locally using the enabled model. If no model is enabled, AzusaOCR does not download anything automatically; it opens OCR model management and asks you to install and enable one. Once installed, PP-OCRv6 Small can run without a network connection. The OCR text layer is presentation-only and does not appear in **Copy edited** or **Save PNG** output. Hover recognized text, drag across OCR lines/blocks to select text in reading order, then right-click for **Copy** or **Create text annotation**. **Ctrl/Cmd+C** copies the active OCR selection, while **Copy all text** copies the full recognized result.

Set `AZUSAOCR_OCR_MODEL_DIR` to use a custom model directory. See [`docs/ocr-models.md`](docs/ocr-models.md) for model management, provenance, storage, and privacy details.

**Copy edited** writes the flattened screenshot/annotations to the clipboard and **Save PNG** writes it to:

```text
<system temp>/AzusaOCR/latest-capture.png
```

Text annotations use an installed system TTF/OTF font. If the automatic font search cannot find a suitable font, set `AZUSAOCR_FONT` to a local font file. On macOS the first capture may require Screen Recording permission. Wayland capture and shortcut permission flows depend on the compositor and desktop portal implementation.

## Debug in VS Code

Install the recommended extensions and run **Debug AzusaOCR** from the Run and Debug panel.

## Workspace

```text
apps/desktop          Slint desktop application and editor interaction layer
crates/core           Shared domain types
crates/capture        Cross-platform capture contract and adapters
crates/hotkey         Native / portal global shortcut abstraction
crates/ocr            OCR engine contract, model manager, and fast local backend
crates/annotation     Annotation document, history, and software renderer
```

## Next milestones

1. Integrate GLM-OCR through an isolated inference process and register it in the existing OCR model manager.
2. Add script-specific OCR model packs for Korean, Arabic, Cyrillic, Thai, and other non-PP-OCRv6-small scripts.
3. Harden dedicated Windows, macOS, Wayland, and X11 capture adapters.
4. Add a public annotation-tool extension registry for optional plugins.
