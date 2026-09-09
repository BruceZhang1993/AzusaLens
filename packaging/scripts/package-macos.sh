#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
    echo "usage: package-macos.sh BINARY VERSION COMMIT TARGET OUTPUT_DIR" >&2
    exit 2
fi

binary=$1
version=$2
commit=$3
target=$4
output_dir=$5
script_dir=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)

[[ -f "$binary" ]] || { echo "binary does not exist: $binary" >&2; exit 1; }
command -v magick >/dev/null 2>&1 || { echo "ImageMagick (magick) is required" >&2; exit 1; }
command -v sips >/dev/null 2>&1 || { echo "macOS sips is required" >&2; exit 1; }
command -v iconutil >/dev/null 2>&1 || { echo "macOS iconutil is required" >&2; exit 1; }
command -v hdiutil >/dev/null 2>&1 || { echo "macOS hdiutil is required" >&2; exit 1; }
mkdir -p "$output_dir"

stage_root=$(mktemp -d "${TMPDIR:-/tmp}/azusa-lens-macos.XXXXXX")
trap 'rm -rf "$stage_root"' EXIT
app_dir="$stage_root/Azusa Lens.app"
contents_dir="$app_dir/Contents"
mkdir -p "$contents_dir/MacOS" "$contents_dir/Resources"
install -Dm755 "$binary" "$contents_dir/MacOS/azusa-lens"
sed "s/@VERSION@/$version/g" "$repo_root/packaging/macos/Info.plist.in" \
    > "$contents_dir/Info.plist"

icon_png="$stage_root/azusa-lens.png"
magick "$repo_root/packaging/assets/com.azusalens.AzusaLens.svg" \
    -background none -resize 1024x1024 "$icon_png"
iconset="$stage_root/AzusaLens.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    double_size=$((size * 2))
    sips -z "$size" "$size" "$icon_png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
    sips -z "$double_size" "$double_size" "$icon_png" \
        --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$contents_dir/Resources/AzusaLens.icns"

artifact="$output_dir/azusa-lens-${version}-${target}.dmg"
hdiutil create -volname "Azusa Lens" -srcfolder "$app_dir" -ov -format UDZO "$artifact" >/dev/null
"$script_dir/write-manifest.sh" "$artifact" "$version" "$commit" "$target" "dmg"
