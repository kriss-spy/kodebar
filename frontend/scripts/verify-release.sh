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

qml_testrunner=/usr/lib/qt6/bin/qmltestrunner
if [[ -x "$qml_testrunner" ]]; then
    for qml_test in \
        frontend/tests/qml/tst_providericon.qml \
        frontend/tests/qml/tst_snapshotreader.qml \
        frontend/tests/qml/tst_snapshotsignalwatcher.qml; do
        env QT_QPA_PLATFORM=offscreen \
            "$qml_testrunner" -input "$qml_test"
    done
else
    echo "kodebar: warning: qmltestrunner not found — skipping QML unit tests" >&2
fi

qml_lint=/usr/lib/qt6/bin/qmllint
if [[ -x "$qml_lint" ]]; then
    for qml_file in \
        frontend/package/contents/ui/*.qml \
        frontend/package/contents/config/*.qml; do
        "$qml_lint" "$qml_file"
    done
else
    echo "kodebar: warning: qmllint not found — skipping QML lint" >&2
fi

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
