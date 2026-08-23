#!/usr/bin/env bash

set -euo pipefail

repository_root="$(cd "$(dirname "$0")/../.." && pwd)"
package_root="$repository_root/frontend/package"
output_directory="${1:-$repository_root/dist}"

for command_name in appstreamcli jq kpackagetool6 unzip zip; do
    if ! command -v "$command_name" >/dev/null; then
        echo "kodebar: required packaging command not found: $command_name" >&2
        exit 1
    fi
done

version="$(jq -er '.KPlugin.Version' "$package_root/metadata.json")"
if [[ ! $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "kodebar: KPlugin.Version must use MAJOR.MINOR.PATCH" >&2
    exit 1
fi
mkdir -p "$output_directory"
output_directory="$(cd "$output_directory" && pwd)"
artifact="$output_directory/kodebar-$version.plasmoid"

validation_root="$(mktemp -d /tmp/kodebar-appstream.XXXXXX)"
trap 'rm -rf -- "$validation_root"' EXIT
kpackagetool6 --type Plasma/Applet \
    --appstream-metainfo "$package_root" >"$validation_root/metainfo.xml"
appstreamcli validate --no-net --strict "$validation_root/metainfo.xml" >/dev/null

rm -f -- "$artifact"
(
    cd "$package_root"
    zip -X -q -r "$artifact" .
)
unzip -tqq "$artifact"

printf '%s\n' "$artifact"
