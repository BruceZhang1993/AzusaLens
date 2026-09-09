#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 6 ]]; then
    echo "usage: package-appimage.sh BINARY VERSION COMMIT OUTPUT_DIR LINUXDEPLOY APPIMAGETOOL" >&2
    exit 2
fi

binary=$1
version=$2
commit=$3
output_dir=$4
linuxdeploy=$5
appimagetool=$6
script_dir=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)

[[ -f "$binary" ]] || { echo "binary does not exist: $binary" >&2; exit 1; }
[[ -x "$linuxdeploy" ]] || { echo "linuxdeploy is not executable: $linuxdeploy" >&2; exit 1; }
[[ -x "$appimagetool" ]] || { echo "appimagetool is not executable: $appimagetool" >&2; exit 1; }
mkdir -p "$output_dir"

stage_root=$(mktemp -d "${TMPDIR:-/tmp}/azusa-lens-appimage.XXXXXX")
trap 'rm -rf "$stage_root"' EXIT
app_dir="$stage_root/AzusaLens.AppDir"
mkdir -p \
    "$app_dir/usr/bin" \
    "$app_dir/usr/share/applications" \
    "$app_dir/usr/share/icons/hicolor/scalable/apps"

install -Dm755 "$binary" "$app_dir/usr/bin/azusa-lens"
install -Dm644 "$repo_root/packaging/linux/com.azusalens.AzusaLens.desktop" \
    "$app_dir/com.azusalens.AzusaLens.desktop"
install -Dm644 "$repo_root/packaging/linux/com.azusalens.AzusaLens.desktop" \
    "$app_dir/usr/share/applications/com.azusalens.AzusaLens.desktop"
install -Dm644 "$repo_root/packaging/assets/com.azusalens.AzusaLens.svg" \
    "$app_dir/com.azusalens.AzusaLens.svg"
install -Dm644 "$repo_root/packaging/assets/com.azusalens.AzusaLens.svg" \
    "$app_dir/usr/share/icons/hicolor/scalable/apps/com.azusalens.AzusaLens.svg"
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)' \
    'exec "$HERE/usr/bin/azusa-lens" "$@"' > "$app_dir/AppRun"
chmod 755 "$app_dir/AppRun"

artifact="$output_dir/azusa-lens-${version}-x86_64.AppImage"
APPIMAGE_EXTRACT_AND_RUN=1 "$linuxdeploy" \
    --appdir "$app_dir" \
    --executable "$app_dir/usr/bin/azusa-lens" \
    --desktop-file "$app_dir/com.azusalens.AzusaLens.desktop"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$appimagetool" --no-appstream "$app_dir" "$artifact"
"$script_dir/write-manifest.sh" "$artifact" "$version" "$commit" \
    "x86_64-unknown-linux-gnu" "appimage"
