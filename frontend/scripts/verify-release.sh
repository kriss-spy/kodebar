#!/usr/bin/env bash

set -euo pipefail

repository_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repository_root"

cargo test --all-targets --manifest-path backend/Cargo.toml
cargo fmt --all --manifest-path backend/Cargo.toml -- --check
cargo clippy --all-targets --manifest-path backend/Cargo.toml -- -D warnings

for test_file in frontend/tests/*.test.js; do
    node "$test_file"
done

for qml_test in \
    frontend/tests/qml/tst_providericon.qml \
    frontend/tests/qml/tst_snapshotreader.qml \
    frontend/tests/qml/tst_snapshotsignalwatcher.qml; do
    env QT_QPA_PLATFORM=offscreen \
        /usr/lib/qt6/bin/qmltestrunner -input "$qml_test"
done

for qml_file in \
    frontend/package/contents/ui/*.qml \
    frontend/package/contents/config/*.qml; do
    /usr/lib/qt6/bin/qmllint "$qml_file"
done

jq empty frontend/package/metadata.json
xmllint --noout \
    frontend/package/contents/config/main.xml \
    frontend/package/contents/images/*.svg
bash -n \
    frontend/scripts/package-plasmoid.sh \
    frontend/scripts/verify-release.sh \
    frontend/tests/dbus-e2e.sh \
    frontend/tests/plasma-smoke.sh

frontend/tests/dbus-e2e.sh
frontend/tests/plasma-smoke.sh
git diff --check
