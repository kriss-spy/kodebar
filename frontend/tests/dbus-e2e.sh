#!/usr/bin/env bash

set -euo pipefail

repository_root="$(cd "$(dirname "$0")/../.." && pwd)"

if [[ -z ${KODEBAR_DBUS_E2E_SESSION:-} ]]; then
    exec dbus-run-session -- env KODEBAR_DBUS_E2E_SESSION=1 "$0"
fi

test_root="$(mktemp -d /tmp/kodebar-dbus-e2e.XXXXXX)"
trap 'rm -rf -- "$test_root"' EXIT

qml_testrunner=/usr/lib/qt6/bin/qmltestrunner
if [[ ! -x "$qml_testrunner" ]]; then
    echo "kodebar: warning: qmltestrunner not found — skipping D-Bus e2e test" >&2
    exit 0
fi

env QT_QPA_PLATFORM=offscreen \
    XDG_CACHE_HOME="$test_root/cache" \
    "$qml_testrunner" \
    -input "$repository_root/frontend/tests/qml/tst_snapshotsignale2e.qml" &
runner_pid=$!

sleep 0.5
env HOME="$test_root/home" \
    XDG_CACHE_HOME="$test_root/cache" \
    "$repository_root/backend/target/debug/kodebar" status --json >/dev/null

wait "$runner_pid"
