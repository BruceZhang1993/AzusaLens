<div align="center">

# Azusa Lens

A lightweight cross-platform screenshot, annotation, and local OCR tool.

[简体中文](README.zh-CN.md) · English

[![CI](https://github.com/BruceZhang1993/AzusaLens/actions/workflows/ci.yml/badge.svg)](https://github.com/BruceZhang1993/AzusaLens/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/BruceZhang1993/AzusaLens)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-1.98.1-orange?logo=rust)
![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue)

<a href="https://slint.dev/"><img src="https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png" alt="Made with Slint" height="24"></a>

</div>

> Azusa Lens is under active development. There are no stable release packages yet; build from source for now.

## Features

- Region capture on Windows, macOS, Linux X11, and native Wayland portal sessions
- Global `PrtSc` shortcut and system tray workflow
- Rectangle, ellipse, arrow, line, pen, text, number, mosaic, and blur annotations
- Select, move, resize, restyle, undo, and redo annotations
- Local OCR with selectable text overlays, including rotated/skewed text geometry
- PP-OCRv6 local models, with optional GLM-OCR and DeepSeek-OCR through local Ollama
- Explicit OCR model download, progress, cancel, enable, retry, and delete controls
- Light, dark, and system appearance modes
- Clipboard copy and PNG export

## Usage

1. Start Azusa Lens. The app stays available from the system tray.
2. Press **PrtSc** or choose **Capture region** from the tray.
3. Select an area and edit it with the floating annotation tools.
4. For OCR, open **OCR Models**, download a model, then enable it. Models are never downloaded automatically.
5. Copy the edited image or save it as PNG.

The management window is available from the tray and contains appearance, shortcuts, capture settings, export settings, OCR models, and application information.

### OCR

PP-OCRv6 models run locally in-process. GLM-OCR and DeepSeek-OCR are optional and use a locally running Ollama instance. Azusa Lens does not silently fall back to a cloud OCR service.

Set a custom PP-OCR model directory with:

```bash
AZUSA_LENS_OCR_MODEL_DIR=/path/to/models
```

If automatic font discovery cannot find a suitable font for text annotations, set:

```bash
AZUSA_LENS_FONT=/path/to/font.ttf
```

See [OCR model documentation](docs/ocr-models.md) for model storage, runtimes, provenance, and privacy details.

## Build & Run

### Requirements

- Rust **1.98.1**
- Git
- Ollama only if using GLM-OCR or DeepSeek-OCR

The repository includes `rust-toolchain.toml`, so `rustup` selects the expected Rust toolchain automatically.

Clone and run:

```bash
git clone https://github.com/BruceZhang1993/AzusaLens.git
cd AzusaLens
cargo run -p azusa-lens-desktop
```

### Linux dependencies

Debian / Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config libclang-dev libxcb1-dev libxrandr-dev libdbus-1-dev \
  libpipewire-0.3-dev libwayland-dev libegl-dev libgbm-dev libx11-xcb-dev \
  libxcursor-dev libxkbcommon-x11-dev libxkbcommon-dev libx11-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libfontconfig-dev
```

On macOS, the first capture may require **Screen Recording** permission. On Wayland, capture and shortcut behavior depends on the compositor and desktop portal implementation.

## Development

Common checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check -p azusa-lens-desktop
```

VS Code users can install the recommended extensions and start **Debug Azusa Lens** from the Run and Debug panel.

### Workspace

```text
apps/desktop       Desktop application and Slint UI
crates/core        Shared domain types
crates/capture     Cross-platform screen capture
crates/hotkey      Global shortcut abstraction
crates/annotation  Annotation document and renderer
crates/ocr         OCR engines and model management
```

CI runs formatting, Clippy, tests, license compliance, and desktop checks across Linux, Windows, and macOS.

## Contributing

Issues and pull requests are welcome. Before submitting a PR, please run formatting, Clippy, and tests locally when possible.

## License

Azusa Lens is licensed under the [Apache License 2.0](LICENSE).

Slint is used under the Slint Royalty-Free Desktop, Mobile, and Web Applications License 2.0. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for third-party licensing and attribution details.
