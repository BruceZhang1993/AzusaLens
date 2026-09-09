# Candidate packages

Packaging is intentionally kept outside the Rust workspace. The candidate workflow builds
unsigned CI Artifacts only; it does not create a GitHub Release or bundle OCR model files.

All packages use `com.azusalens.AzusaLens` as the application identifier and the static icon in
`assets/com.azusalens.AzusaLens.svg`, which follows the geometry and colors of the runtime icon.
PP-OCRv6 Tiny is downloaded only by the OCR smoke test from the pinned `rust-paddle-ocr` v2.4.1
source. It is never copied into an installer or committed to the repository.

The supported candidate artifact names are:

- `azusa-lens-<version>-x86_64-pc-windows-msvc.msi`
- `azusa-lens-<version>-x86_64-apple-darwin.dmg`
- `azusa-lens-<version>-aarch64-apple-darwin.dmg`
- `azusa-lens-<version>-x86_64.pkg.tar.zst`
- `azusa-lens-<version>-x86_64.AppImage`

The AppImage build runs `linuxdeploy` before `appimagetool` so non-system shared libraries are
copied into the AppDir; glibc and compositor services remain host-provided by design.

Each package directory also contains `manifest.json` and a `<package>.sha256` file. The manifest
records the version, source commit, target, architecture, package type, SHA-256, application
identifier, and PP-OCRv6 Tiny model version.

The packages are unsigned in P0. Windows SmartScreen and macOS Gatekeeper warnings are expected
until a later signing/notarization phase.
