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

## PP-OCRv6 tiers

All three PP-OCRv6 tiers run in-process through Rust `ocr-rs` 2.4.1 and MNN. They are independently downloadable and selectable in **Settings > OCR models**.

| Tier | Detection | Recognition | Coverage | Approx. model download | Positioning |
| --- | --- | --- | --- | ---: | --- |
| Tiny | `PP-OCRv6_tiny_det.mnn` | `PP-OCRv6_tiny_rec.mnn` | Simplified/Traditional Chinese, English and 46 Latin-script languages; Japanese excluded | 3.2 MiB | Fastest / lowest resource use |
| Small | `PP-OCRv6_small_det.mnn` | `PP-OCRv6_small_rec.mnn` | Official 50-language PP-OCRv6 set including Chinese, English and Japanese | 15.6 MiB | Balanced default tier |
| Medium | `PP-OCRv6_medium_det.mnn` | `PP-OCRv6_medium_rec.mnn` | Official 50-language PP-OCRv6 set including Chinese, English and Japanese | 69.5 MiB | Accuracy-first inference tier |

The corresponding character dictionaries are `ppocr_keys_v6_tiny.txt`, `ppocr_keys_v6_small.txt`, and `ppocr_keys_v6_medium.txt`.

Medium uses the PP-OCRv6 Medium **inference** model converted to the MNN runtime format; AzusaOCR does not download or use training checkpoints. PaddleOCR describes the Medium recognition model as the accuracy-first PP-OCRv6 tier, while Tiny is the lightweight tier and Small balances speed and accuracy.

The runtime and model URLs are pinned to the upstream `rust-paddle-ocr` `v2.4.1` tag so an AzusaOCR build does not silently switch model revisions. The exact MNN file sizes are validated before a model is considered installed.

The existing Small model ID and default cache directory are preserved for backward compatibility, so installations created by earlier AzusaOCR builds remain discoverable. Tiny and Medium use their own default versioned cache directories. When `AZUSAOCR_OCR_MODEL_DIR` is set, all three tiers may share that custom directory because their filenames are tier-specific; deleting one tier removes only its three managed files.

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

Model binaries are not committed to the AzusaOCR repository. The PP-OCR backend never uploads screenshot pixels for recognition and only uses network access when the user explicitly downloads its model files. Ollama-backed recognition is handed to the configured Ollama runtime, so a non-loopback `OLLAMA_HOST` can transmit screenshot pixels to that remote endpoint.

PP-OCR files are stored in AzusaOCR's model directories. Set `AZUSAOCR_OCR_MODEL_DIR` to override that location. Installation and removal from the settings page use that directory when the override is present. Removal deletes only the files owned by the selected PP-OCR tier; unrelated files and other installed tiers are preserved.

GLM-OCR and DeepSeek-OCR model files are managed in Ollama's own model store rather than `AZUSAOCR_OCR_MODEL_DIR`.

The active model choice is stored separately in AzusaOCR's application configuration directory. Removing the active model also clears that selection.

## Provenance and licenses

`ocr-rs` is distributed under Apache-2.0. The PP-OCR model family originates from PaddleOCR/Baidu and the MNN-converted inference files used by this backend are published by the `rust-paddle-ocr` project.

GLM-OCR model weights are released under the MIT License; the upstream complete GLM-OCR pipeline additionally uses PP-DocLayout-V3 under Apache-2.0. DeepSeek-OCR is released under the MIT License. Ollama model packaging remains external to AzusaOCR. Distribution and attribution requirements should continue to follow the corresponding upstream projects if AzusaOCR starts bundling any of these models in packaged releases.
