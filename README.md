# AzusaOCR

AzusaOCR is an early Rust desktop application for native cross-platform screenshots, local OCR, annotation, and future extensions.

## Current milestone: Editable screenshot annotations

The desktop validation app now has a complete capture-to-edit path:

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
- Pluggable OCR contract prepared for GLM-OCR and a fast OCR backend
- VS Code + CodeLLDB debug configuration
- GitHub Actions checks on Windows, macOS, and Linux

The capture and annotation layers are intentionally separated. `azusa-capture` owns platform screenshot acquisition, while `azusa-annotation` owns annotation geometry, history, and software rendering. Selection chrome and viewport transforms stay in the Slint presentation layer, while object transforms, history, export pixels, and future OCR bounding boxes remain in stable image coordinates. New tools can therefore extend the editor without coupling their implementation to Slint or to a specific screenshot backend.

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

Press **PrtSc** or choose **New capture**, select a region, then annotate it directly in the editor. Switch to **Select** to move, resize, restyle, or delete an existing annotation. Use the mouse wheel or **Ctrl/Cmd + Plus/Minus** to zoom, middle/right-button drag to pan while zoomed, and **Ctrl/Cmd + 0** or **Fit** to return to fit-to-view. **Copy edited** writes the flattened result to the clipboard and **Save PNG** writes it to:

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
crates/ocr            OCR engine contract
crates/annotation     Annotation document, history, and software renderer
```

## Next milestones

1. Add fast local OCR with text bounding boxes and OCR-to-annotation actions.
2. Integrate GLM-OCR through an isolated inference process.
3. Harden dedicated Windows, macOS, Wayland, and X11 capture adapters.
4. Add a public annotation-tool extension registry for optional plugins.
