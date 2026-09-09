#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
    echo "usage: package-smoke.sh TYPE ARTIFACT [VERSION]" >&2
    exit 2
fi

package_type=$1
artifact=$2
expected_version=${3:-}
script_dir=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)

[[ -f "$artifact" ]] || { echo "artifact does not exist: $artifact" >&2; exit 1; }
case "$package_type" in
    appimage)
        command -v desktop-file-validate >/dev/null 2>&1 || exit 1
        smoke_root=$(mktemp -d "${TMPDIR:-/tmp}/azusa-lens-appimage-smoke.XXXXXX")
        trap 'rm -rf "$smoke_root"' EXIT
        (
            cd "$smoke_root"
            "$artifact" --appimage-extract >/dev/null
        )
        test -x "$smoke_root/squashfs-root/usr/bin/azusa-lens"
        test -f "$smoke_root/squashfs-root/com.azusalens.AzusaLens.desktop"
        test -f "$smoke_root/squashfs-root/com.azusalens.AzusaLens.svg"
        desktop-file-validate "$smoke_root/squashfs-root/com.azusalens.AzusaLens.desktop"
        grep -Fqx 'Icon=com.azusalens.AzusaLens' \
            "$smoke_root/squashfs-root/com.azusalens.AzusaLens.desktop"
        ;;
    arch-pacman)
        command -v tar >/dev/null 2>&1 || exit 1
        package_listing=$(tar --zstd -tf "$artifact")
        grep -Eq '(^|/)usr/bin/azusa-lens$' <<< "$package_listing"
        grep -Eq '(^|/)usr/share/applications/com.azusalens.AzusaLens.desktop$' <<< "$package_listing"
        grep -Eq '(^|/)usr/share/icons/hicolor/scalable/apps/com.azusalens.AzusaLens.svg$' <<< "$package_listing"
        ;;
    *)
        echo "unsupported package type for this host: $package_type" >&2
        exit 2
        ;;
esac

test -f "$repo_root/packaging/assets/com.azusalens.AzusaLens.svg"
manifest_path="$(dirname "$artifact")/manifest.json"
test -f "$manifest_path"
grep -Fqx '  "application_id": "com.azusalens.AzusaLens",' "$manifest_path"
grep -Fqx '  "architecture": "x86_64",' "$manifest_path"
if [[ -n "$expected_version" ]]; then
    grep -Fqx "  \"version\": \"$expected_version\"," "$manifest_path"
fi
checksum_path="$artifact.sha256"
test -f "$checksum_path"
expected_checksum=$(awk '{print $1}' "$checksum_path")
if command -v sha256sum >/dev/null 2>&1; then
    actual_checksum=$(sha256sum "$artifact" | awk '{print $1}')
else
    actual_checksum=$(shasum -a 256 "$artifact" | awk '{print $1}')
fi
test "$actual_checksum" = "$expected_checksum"
manifest_checksum=$(sed -n 's/  "sha256": "\([0-9a-f]\{64\}\)",/\1/p' "$manifest_path")
test "$actual_checksum" = "$manifest_checksum"
