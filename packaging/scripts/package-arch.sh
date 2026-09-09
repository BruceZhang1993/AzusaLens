#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
    echo "usage: package-arch.sh BINARY VERSION COMMIT OUTPUT_DIR" >&2
    exit 2
fi

binary=$1
version=$2
commit=$3
output_dir=$4
script_dir=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)

[[ -f "$binary" ]] || { echo "binary does not exist: $binary" >&2; exit 1; }
command -v makepkg >/dev/null 2>&1 || { echo "makepkg is required" >&2; exit 1; }
mkdir -p "$output_dir"

build_root=$(mktemp -d "${TMPDIR:-/tmp}/azusa-lens-arch.XXXXXX")
cleanup() { rm -rf "$build_root"; }
trap cleanup EXIT
install -Dm755 "$binary" "$build_root/azusa-lens"
install -Dm644 "$repo_root/packaging/linux/com.azusalens.AzusaLens.desktop" \
    "$build_root/com.azusalens.AzusaLens.desktop"
install -Dm644 "$repo_root/packaging/assets/com.azusalens.AzusaLens.svg" \
    "$build_root/com.azusalens.AzusaLens.svg"
sed "s/@VERSION@/$version/" "$repo_root/packaging/linux/PKGBUILD" > "$build_root/PKGBUILD"

if [[ ${EUID:-$(id -u)} -eq 0 ]]; then
    builder_user=azusa-builder
    useradd --create-home --user-group "$builder_user"
    chown -R "$builder_user:$builder_user" "$build_root"
    runuser -u "$builder_user" -- bash -c \
        "cd '$build_root' && makepkg --noconfirm --clean --force"
else
    (
        cd "$build_root"
        makepkg --noconfirm --clean --force
    )
fi

artifact=$(find "$build_root" -maxdepth 1 -type f -name "azusa-lens-${version}-*.pkg.tar.zst" -print -quit)
[[ -n "$artifact" ]] || { echo "makepkg did not produce a pacman package" >&2; exit 1; }
artifact_name="azusa-lens-${version}-x86_64.pkg.tar.zst"
mv "$artifact" "$output_dir/$artifact_name"
"$script_dir/write-manifest.sh" "$output_dir/$artifact_name" "$version" "$commit" \
    "x86_64-unknown-linux-gnu" "arch-pacman"
