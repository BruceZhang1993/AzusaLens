# AzusaOCR

AzusaOCR is an early validation scaffold for a native cross-platform screenshot, local OCR, and annotation tool.

## Current validation scope

- Rust 2024 workspace
- Slint desktop UI
- Windows / macOS / Linux project layout
- Runtime capture-backend detection contract
  - Windows Graphics Capture target
  - macOS ScreenCaptureKit target
  - Wayland XDG Portal target
  - X11 target
- Pluggable OCR contract prepared for GLM-OCR and a fast OCR backend
- Non-destructive annotation scene model
- VS Code + CodeLLDB debug configuration
- GitHub Actions checks on Windows, macOS, and Linux

The current Capture and OCR buttons are deliberately stubbed. They validate UI callbacks and crate wiring without requiring capture permissions or model downloads.

## Requirements

- Rust 1.98.1
- On Linux, the native packages required by Slint's X11/Wayland desktop backend

Debian/Ubuntu example:

```bash
sudo apt install libx11-xcb-dev xinput libxcursor-dev libxkbcommon-x11-dev libxkbcommon-dev libwayland-dev libx11-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libfontconfig-dev
```

## Run

```bash
cargo run -p azusaocr-desktop
```

## Debug in VS Code

Install the recommended extensions and run **Debug AzusaOCR** from the Run and Debug panel.

## Workspace

```text
apps/desktop          Slint desktop validation application
crates/core           Shared domain types
crates/capture        Cross-platform capture contract and backend detection
crates/ocr            OCR engine contract
crates/annotation     Non-destructive annotation scene model
```

## Next milestones

1. Implement Windows Graphics Capture and ScreenCaptureKit adapters.
2. Implement Wayland XDG Desktop Portal and X11 capture adapters.
3. Add region-selection overlay and clipboard export.
4. Add fast local OCR backend.
5. Integrate GLM-OCR through an isolated inference process.
6. Add annotation rendering and undo/redo command stack.
