# OCR orientation handling

Azusa Lens keeps OCR geometry in the original screenshot coordinate system and avoids whole-image deskew or enhancement heuristics by default.

## PP-OCRv6 orientation policy

PP-OCRv6 Tiny, Small, and Medium use the `ocr-rs` 2.4.1 robust rotated-text pipeline for every recognition call.

The upstream robust pipeline:

- runs the normal detection/recognition pass
- re-orients tall regions detected by the normal pass
- performs additional 90° and 270° detection passes to recover text that the normal pass missed
- recognizes only relevant horizontal candidates from those rotated passes
- maps recovered quadrilateral geometry back into the original image coordinate system
- de-duplicates overlapping spatial results before returning them

Azusa Lens therefore receives one reading-order result set whose polygons remain usable by the existing selectable OCR text layer.

## 180° fallback

The upstream robust mode targets mixed 90°/270° text. Azusa Lens adds a selective fallback for upside-down horizontal text without running a fourth full-image detection pass.

A result is eligible when:

- the primary recognition confidence is below `0.80`
- its detected bounds are horizontal with an aspect ratio of at least `1.5`

Azusa Lens crops only that detected line, adds a small bounded padding, rotates the crop 180°, and runs recognition-only inference. The fallback replaces the original text only when its confidence improves by at least `0.08`.

The fallback changes only the recognized text and confidence. The original bounds and quadrilateral remain unchanged, so overlays, hit testing, copying, and OCR-to-annotation conversion stay aligned with the screenshot.

## Slightly skewed text

Azusa Lens does not currently apply whole-image deskew, binarization, sharpening, contrast enhancement, or super-resolution before OCR. PP-OCR detection already returns quadrilateral regions and the recognition pipeline rectifies detected text regions for recognition. Keeping the source image unchanged also allows multiple text regions with different angles to coexist in one screenshot.

Image preprocessing can be added later only when benchmark data shows a clear accuracy benefit that justifies the extra latency and complexity.

## Performance characteristics

Robust mode costs more than the legacy single-pass path because 90° and 270° detection passes are added. The 180° recovery path is deliberately recognition-only and runs only for low-confidence horizontal lines, avoiding another full-page detection pass.

A future benchmark should compare normal horizontal screenshots, mixed horizontal/vertical screenshots, 180° text, and lightly skewed text across PP-OCRv6 Tiny, Small, and Medium before exposing additional quality/performance controls in Settings.
