# Local OCR models

AzusaOCR's fast OCR backend uses the Rust `ocr-rs` runtime with PaddleOCR models converted to MNN.

## Model management

OCR models are managed explicitly from **Settings > OCR models**. AzusaOCR does not download a model when the user starts OCR.

The model manager is responsible for the complete lifecycle:

- show every OCR model known to the current build
- show installed, active, and not-installed states
- download/install a model only after an explicit user action
- enable an installed model as the active OCR engine
- switch between installed models
- delete installed model files
- persist the active model selection between launches

Downloading a model does not automatically enable it. This keeps network access and model activation separate and explicit.

If **OCR** is used while no installed model is enabled, AzusaOCR opens the OCR model management page and asks the user to download and enable a model first.

## PP-OCRv6 Small

- Runtime crate: `ocr-rs` 2.4.1
- Model family: PP-OCRv6 Small
- Detection model: `PP-OCRv6_small_det.mnn`
- Recognition model: `PP-OCRv6_small_rec.mnn`
- Character set: `ppocr_keys_v6_small.txt`
- Primary coverage: Simplified Chinese, Traditional Chinese, English, Japanese, plus the additional Latin-script languages supported by PP-OCRv6 Small
- Approximate download size: 16 MiB

The runtime and model URLs are pinned to the upstream `rust-paddle-ocr` `v2.4.1` tag so an AzusaOCR build does not silently switch model revisions.

## Local storage and privacy

Model binaries are not committed to the AzusaOCR repository. The fast backend never uploads screenshot pixels for recognition. Network access is used only when the user explicitly chooses **Download** for a model in OCR model management.

After installation, OCR inference is local and the model can be reused without a network connection.

Set `AZUSAOCR_OCR_MODEL_DIR` to override the model storage directory. Model installation and removal from the settings page use that directory when the override is present.

The active model choice is stored separately in AzusaOCR's application configuration directory. Removing the active model also clears that selection.

## Extensibility

The OCR model manager uses a central model catalog and engine factory rather than PP-OCR-specific UI logic. Future OCR backends can therefore expose their metadata, installation state, and engine implementation through the same model management page.

GLM-OCR is intentionally not part of the current fast backend. It will be integrated later behind the same OCR engine contract, preferably through an isolated inference process. Additional script-specific fast models can also be registered in the same catalog.

## Provenance and licenses

`ocr-rs` is distributed under Apache-2.0. The PP-OCR model family originates from PaddleOCR/Baidu and the MNN-converted files used by this backend are published by the `rust-paddle-ocr` project. Distribution and attribution requirements should continue to follow the corresponding upstream projects when AzusaOCR starts bundling models in packaged releases.
