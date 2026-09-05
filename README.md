# AzusaOCR

AzusaOCR is an early Rust desktop application for native cross-platform screenshots, local OCR, annotation, and future extensions.

## Current milestone: Capture MVP

The validation application now exercises a real screenshot data path instead of a stub:

- Rust 2024 workspace
- Slint desktop UI
- Real primary-display capture with an in-app RGBA preview
- Copy the captured image to the system clipboard
- Save the captured frame as PNG
- Windows capture with XCap's WGC feature enabled
- Native Linux X11 / Wayland capture paths exposed through the capture adapter
- macOS capture through the current native XCap adapter
- Pluggable OCR contract prepared for GLM-OCR and a fast OCR backend
- Non-destructive annotation scene model
- VS Code + CodeLLDB debug configuration
- GitHub Actions checks on Windows, macOS, and Linux

The XCap dependency is an **MVP implementation detail** inside `azusa-capture`, not the application's public capture API. Dedicated per-platform backends can replace it without changing the UI or higher-level capture flow. Wayland remains a first-class target; its behavior will be hardened against GNOME, KDE, wlroots compositors, and portal permission flows in subsequent milestones.

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

Then click **Capture primary display**. A successful capture is rendered directly in the application. **Copy image** writes it to the clipboard and **Save PNG** writes it to:

```text
<system temp>/AzusaOCR/latest-capture.png
```

On macOS the first capture may require Screen Recording permission. Wayland behavior depends on the compositor/session and may involve a desktop permission flow.

## Debug in VS Code

Install the recommended extensions and run **Debug AzusaOCR** from the Run and Debug panel.

## Workspace

```text
apps/desktop          Slint desktop application
crates/core           Shared domain types
crates/capture        Capture contract + current MVP adapter
crates/ocr            OCR engine contract
crates/annotation     Non-destructive annotation scene model
```

## Next milestones

1. Add region-selection overlay and multi-monitor selection.
2. Harden dedicated Windows, macOS, Wayland, and X11 adapters behind `azusa-capture`.
3. Add global hotkeys and tray lifecycle.
4. Add fast local OCR with text bounding boxes.
5. Integrate GLM-OCR through an isolated inference process.
6. Add annotation rendering and undo/redo command stack.
