from pathlib import Path
import re

root = Path('.')

cargo = root / 'crates/capture/Cargo.toml'
text = cargo.read_text(encoding='utf-8')
needle = '[dependencies]\ndevice_query = "=3.0.0"\n'
replacement = '[dependencies]\nchrono = "=0.4.45"\ndevice_query = "=3.0.0"\n'
if text.count(needle) != 1:
    raise RuntimeError('unexpected capture Cargo.toml dependency layout')
cargo.write_text(text.replace(needle, replacement, 1), encoding='utf-8')

path = root / 'crates/capture/src/dialogs.rs'
text = path.read_text(encoding='utf-8')
old_import = '''use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
'''
new_import = '''use chrono::Local;
use std::{
    fs,
    path::{Path, PathBuf},
};
'''
if text.count(old_import) != 1:
    raise RuntimeError('unexpected dialogs import block')
text = text.replace(old_import, new_import, 1)

old_name = '''#[must_use]
pub fn suggested_png_name() -> String {
    local_timestamp()
        .map(|timestamp| format!("{timestamp}.png"))
        .unwrap_or_else(|| {
            let seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            suggested_png_name_at(seconds)
        })
}
'''
new_name = '''#[must_use]
pub fn suggested_png_name() -> String {
    format!("{}.png", Local::now().format("%Y-%m-%d_%H-%M-%S"))
}
'''
if text.count(old_name) != 1:
    raise RuntimeError('unexpected suggested_png_name implementation')
text = text.replace(old_name, new_name, 1)

pattern = re.compile(r'''\n#\[cfg\(any\(target_os = "linux", target_os = "macos"\)\)\]\nfn local_timestamp\(\) -> Option<String> \{.*?\nfn civil_date_from_unix_days\(days: i64\) -> \(i64, u32, u32\) \{.*?\n\}\n\n#\[cfg\(test\)\]''', re.S)
text, count = pattern.subn('\n#[cfg(test)]', text, count=1)
if count != 1:
    raise RuntimeError(f'unexpected timestamp helper block count: {count}')

old_tests = '''    #[test]
    fn suggested_name_is_stable_for_unix_epoch() {
        assert_eq!(suggested_png_name_at(0), "1970-01-01_00-00-00.png");
    }

    #[test]
    fn suggested_name_handles_leap_day() {
        // 2024-02-29 12:34:56 UTC fallback value.
        assert_eq!(
            suggested_png_name_at(1_709_210_096),
            "2024-02-29_12-34-56.png"
        );
    }
'''
new_tests = '''    #[test]
    fn suggested_name_uses_filename_safe_local_datetime_shape() {
        let name = suggested_png_name();
        assert_eq!(name.len(), 23);
        assert!(name.ends_with(".png"));
        for index in [4, 7] {
            assert_eq!(name.as_bytes()[index], b'-');
        }
        assert_eq!(name.as_bytes()[10], b'_');
        for index in [13, 16] {
            assert_eq!(name.as_bytes()[index], b'-');
        }
        assert!(name[..19]
            .chars()
            .enumerate()
            .all(|(index, character)| [4, 7, 10, 13, 16].contains(&index)
                || character.is_ascii_digit()));
    }
'''
if text.count(old_tests) != 1:
    raise RuntimeError('unexpected timestamp tests')
text = text.replace(old_tests, new_tests, 1)
path.write_text(text, encoding='utf-8')

print('optimized screenshot timestamp formatting')
