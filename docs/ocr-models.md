# Local OCR models

AzusaOCR exposes every OCR backend through one model catalog and the same explicit model-management flow in **Settings > OCR models**.

## Model management

OCR models are managed explicitly from **Settings > OCR models**. AzusaOCR does not download a model when the user starts OCR.

The model manager is responsible for the complete lifecycle:

- show every OCR model known to the current build
- show installed, active, and not-installed states
- download/install a model only after an explicit user action
- enable an installed model as the active OCR engine
- switch between installed models
- delete installed models
- persist the active model selection between launches

Downloading a model does not automatically enable it. This keeps network access and model activation separate and explicit.

If **OCR** is used while no installed model is enabled, AzusaOCR opens the OCR model management page and asks the user to download and enable a model first.

## PP-OCRv6 Small

- Runtime: in-process Rust `ocr-rs` 2.4.1 / MNN
- Model family: PP-OCRv6 Small
- Detection model: `PP-OCRv6_small_det.mnn`
- Recognition model: `PP-OCRv6_small_rec.mnn`
- Character set: `ppocr_keys_v6_small.txt`
- Primary coverage: Simplified Chinese, Traditional Chinese, English, Japanese, plus the additional Latin-script languages supported by PP-OCRv6 Small
- Approximate download size: 16 MiB

The runtime and model URLs are pinned to the upstream `rust-paddle-ocr` `v2.4.1` tag so an AzusaOCR build does not silently switch model revisions.

## GLM-OCR

- Runtime: local Ollama CLI
- Ollama model: `glm-ocr:latest`
- Model-management label: `GLM-OCR · local via Ollama`
- Approximate download size: 2.2 GB
- Primary use: multilingual text/document recognition

GLM-OCR is optional. It is neither downloaded nor enabled by default. Choosing **Download** runs `ollama pull glm-ocr:latest`; choosing **Enable** is a separate action after installation succeeds.

The current Ollama model returns recognized text but does not expose the complete PP-DocLayout-V3 region pipeline used by the upstream GLM-OCR SDK. AzusaOCR therefore preserves the full recognized text and represents it as one image-space fallback block. A later native layout adapter can provide finer-grained regions without changing the editor contract.

## DeepSeek-OCR

- Runtime: local Ollama CLI 0.13.0 or newer
- Ollama model: `deepseek-ocr:latest`
- Model-management label: `DeepSeek-OCR · local via Ollama`
- Approximate download size: 6.7 GB
- Primary use: multilingual document OCR with grounding coordinates

DeepSeek-OCR is optional. It is neither downloaded nor enabled by default. The backend uses the model's grounding output and converts its 0–999 normalized boxes back into AzusaOCR image coordinates, so recognized regions can participate in the existing OCR highlight/selection layer.

## Ollama runtime

GLM-OCR and DeepSeek-OCR require the local `ollama` executable. AzusaOCR uses the official CLI for model listing, pull/removal, and image inference; the CLI follows Ollama's standard `OLLAMA_HOST` configuration when a different endpoint is configured.

If Ollama cannot be started or reached, the two optional models remain **NOT INSTALLED** and their **Download** action reports a clear error. AzusaOCR does not install Ollama automatically and never falls back to a cloud OCR service.

Recognition creates a uniquely named temporary PNG so the Ollama CLI can consume the screenshot image, and removes that temporary file immediately after the command completes or errors. Keep `OLLAMA_HOST` pointed at a loopback/local endpoint if screenshots must never leave the device.

## Local storage and privacy

Model binaries are not committed to the AzusaOCR repository. The PP-OCR backend never uploads screenshot pixels for recognition. Network access is used only when the user explicitly chooses **Download** for a model.

PP-OCR files are stored in AzusaOCR's model directory. Set `AZUSAOCR_OCR_MODEL_DIR` to override that directory. Installation and removal from the settings page use that directory when the override is present. Removal deletes only the files owned by the selected PP-OCR model; unrelated files are preserved.

GLM-OCR and DeepSeek-OCR model files are managed in Ollama's own model store rather than `AZUSAOCR_OCR_MODEL_DIR`.

The active model choice is stored separately in AzusaOCR's application configuration directory. Removing the active model also clears that selection.

## Provenance and licenses

`ocr-rs` is distributed under Apache-2.0. The PP-OCR model family originates from PaddleOCR/Baidu and the MNN-converted files used by this backend are published by the `rust-paddle-ocr` project.

GLM-OCR model weights are released under the MIT License; the upstream complete GLM-OCR pipeline additionally uses PP-DocLayout-V3 under Apache-2.0. DeepSeek-OCR is released under the MIT License. Ollama model packaging remains external to AzusaOCR. Distribution and attribution requirements should continue to follow the corresponding upstream projects if AzusaOCR starts bundling any of these models in packaged releases.
