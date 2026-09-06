# Local OCR models

AzusaOCR's fast OCR backend uses the Rust `ocr-rs` runtime with PaddleOCR models converted to MNN.

## Default fast model

- Runtime crate: `ocr-rs` 2.4.1
- Model family: PP-OCRv6 small
- Detection model: `PP-OCRv6_small_det.mnn`
- Recognition model: `PP-OCRv6_small_rec.mnn`
- Character set: `ppocr_keys_v6_small.txt`
- Primary coverage: Simplified Chinese, Traditional Chinese, English, Japanese, plus the additional Latin-script languages supported by PP-OCRv6 small

The runtime and model URLs are pinned to the upstream `rust-paddle-ocr` `v2.4.1` tag so an AzusaOCR build does not silently switch model revisions.

## Local storage

Model binaries are not committed to the AzusaOCR repository. On first OCR use, the desktop app downloads the pinned model files to the platform cache directory and then performs inference locally on subsequent runs.

Set `AZUSAOCR_OCR_MODEL_DIR` to use a custom model directory. AzusaOCR will use/download the same pinned filenames in that directory.

Approximate default model download size is 16 MiB.

## Provenance and licenses

`ocr-rs` is distributed under Apache-2.0. The PP-OCR model family originates from PaddleOCR/Baidu and the MNN-converted files used by this backend are published by the `rust-paddle-ocr` project. Distribution and attribution requirements should continue to follow the corresponding upstream projects when AzusaOCR starts bundling models in packaged releases.

GLM-OCR is intentionally not part of this fast backend. It will be integrated later behind the same AzusaOCR OCR engine contract, preferably through an isolated inference process.
