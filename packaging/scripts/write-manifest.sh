#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
    echo "usage: write-manifest.sh ARTIFACT VERSION COMMIT TARGET PACKAGE" >&2
    exit 2
fi

artifact_path=$1
version=$2
commit=$3
target=$4
package_type=$5
architecture=${target%%-*}
artifact_dir=$(cd "$(dirname "$artifact_path")" && pwd)
artifact_name=$(basename "$artifact_path")
checksum_path="$artifact_path.sha256"

if [[ ! -f "$artifact_path" ]]; then
    echo "artifact does not exist: $artifact_path" >&2
    exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
    (cd "$artifact_dir" && sha256sum "$artifact_name") > "$checksum_path"
    checksum=$(cut -d' ' -f1 "$checksum_path")
elif command -v shasum >/dev/null 2>&1; then
    (cd "$artifact_dir" && shasum -a 256 "$artifact_name") > "$checksum_path"
    checksum=$(cut -d' ' -f1 "$checksum_path")
else
    echo "neither sha256sum nor shasum is available" >&2
    exit 1
fi

model_version=${AZUSA_LENS_OCR_MODEL_VERSION:-ppocrv6-tiny-ocr-rs-v2.4.1}
manifest_path="$artifact_dir/manifest.json"
temporary_manifest="$manifest_path.tmp.$$"
{
    printf '{\n'
    printf '  "application": "Azusa Lens",\n'
    printf '  "application_id": "com.azusalens.AzusaLens",\n'
    printf '  "version": "%s",\n' "$version"
    printf '  "commit": "%s",\n' "$commit"
    printf '  "target": "%s",\n' "$target"
    printf '  "architecture": "%s",\n' "$architecture"
    printf '  "package": "%s",\n' "$package_type"
    printf '  "artifact": "%s",\n' "$artifact_name"
    printf '  "sha256": "%s",\n' "$checksum"
    printf '  "ocr_model_version": "%s",\n' "$model_version"
    printf '  "signed": false\n'
    printf '}\n'
} > "$temporary_manifest"
mv "$temporary_manifest" "$manifest_path"
