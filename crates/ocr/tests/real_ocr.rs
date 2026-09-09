use azusa_ocr::{FastModelPaths, FastOcrEngine, OcrEngine, OcrImage, PpOcrTier};

const GLYPHS: &[(char, [&str; 7])] = &[
    (
        'A',
        [
            ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ],
    ),
    (
        'E',
        [
            "#####", "#....", "#....", "####.", "#....", "#....", "#####",
        ],
    ),
    (
        'L',
        [
            "#....", "#....", "#....", "#....", "#....", "#....", "#####",
        ],
    ),
    (
        'N',
        [
            "#...#", "##..#", "##..#", "#.#.#", "#..##", "#..##", "#...#",
        ],
    ),
    (
        'S',
        [
            ".####", "#....", "#....", ".###.", "....#", "....#", "####.",
        ],
    ),
    (
        'U',
        [
            "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
        ],
    ),
    (
        'Z',
        [
            "#####", "....#", "...#.", "..#..", ".#...", "#....", "#####",
        ],
    ),
    (
        '1',
        [
            "..#..", ".##..", "#.#..", "..#..", "..#..", "..#..", ".###.",
        ],
    ),
    (
        '2',
        [
            ".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####",
        ],
    ),
    (
        '3',
        [
            "####.", "....#", "....#", ".###.", "....#", "....#", "####.",
        ],
    ),
];

fn fixed_fixture() -> OcrImage {
    let text = "AZUSA LENS 123";
    let scale = 8_u32;
    let glyph_width = 5 * scale;
    let glyph_gap = scale;
    let margin = 32_u32;
    let width = margin * 2 + text.chars().count() as u32 * (glyph_width + glyph_gap);
    let height = margin * 2 + 7 * scale;
    let mut rgba = vec![255_u8; width as usize * height as usize * 4];

    for (index, character) in text.chars().enumerate() {
        let Some((_, glyph)) = GLYPHS.iter().find(|(key, _)| *key == character) else {
            continue;
        };
        let origin_x = margin + index as u32 * (glyph_width + glyph_gap);
        for (row, pixels) in glyph.iter().enumerate() {
            for (column, pixel) in pixels.bytes().enumerate() {
                if pixel != b'#' {
                    continue;
                }
                for y in 0..scale {
                    for x in 0..scale {
                        let px = origin_x + column as u32 * scale + x;
                        let py = margin + row as u32 * scale + y;
                        let offset = (py as usize * width as usize + px as usize) * 4;
                        rgba[offset..offset + 4].copy_from_slice(&[16, 24, 40, 255]);
                    }
                }
            }
        }
    }

    OcrImage::new(width, height, rgba).expect("fixed OCR fixture must be valid")
}

#[test]
#[ignore = "downloads the pinned PP-OCRv6 Tiny model and runs native inference"]
fn ppocr_tiny_recognizes_fixed_fixture() {
    let paths = FastModelPaths::discover_for(PpOcrTier::Tiny);
    if !paths.are_ready() {
        paths
            .install()
            .expect("pinned PP-OCRv6 Tiny model download must succeed");
    }

    let image = fixed_fixture();
    let mut engine = FastOcrEngine::new_for(PpOcrTier::Tiny);
    let result = engine
        .recognize(&image)
        .expect("PP-OCRv6 Tiny recognition must succeed");
    assert!(
        !result.blocks.is_empty(),
        "OCR must return at least one text block"
    );

    let normalized = result.plain_text.to_ascii_uppercase().replace(' ', "");
    assert!(
        normalized.contains("AZUSA") && normalized.contains("LENS"),
        "OCR result must contain the fixture keywords, got {:?}",
        result.plain_text
    );
    for block in result.blocks {
        assert!(block.bounds.x >= 0.0);
        assert!(block.bounds.y >= 0.0);
        assert!(block.bounds.width >= 0.0);
        assert!(block.bounds.height >= 0.0);
        assert!(block.bounds.x + block.bounds.width <= image.width() as f32 + 1.0);
        assert!(block.bounds.y + block.bounds.height <= image.height() as f32 + 1.0);
    }
}
