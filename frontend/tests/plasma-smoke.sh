#!/usr/bin/env bash

set -euo pipefail

test_root="$(mktemp -d /tmp/kodebar-plasma-smoke.XXXXXX)"
trap 'rm -rf -- "$test_root"' EXIT

install -Dm600 frontend/tests/fixtures/snapshot.json \
    "$test_root/cache/kodebar/last.json"
kpackagetool6 --type Plasma/Applet \
    --packageroot "$test_root/data/plasma/plasmoids" \
    --install frontend/package >/dev/null

set +e
timeout 8s dbus-run-session -- env \
    XDG_CACHE_HOME="$test_root/cache" \
    XDG_DATA_HOME="$test_root/data" \
    QT_QPA_PLATFORM=offscreen \
    plasmawindowed io.github.kriss_spy.kodebar >"$test_root/plasmawindowed.log" 2>&1
status=$?
set -e

if [[ $status -ne 124 ]]; then
    cat "$test_root/plasmawindowed.log"
    exit "$status"
fi

if grep -Eqi 'failed to load|is not a type|qml.*error|referenceerror|typeerror' \
    "$test_root/plasmawindowed.log"; then
    cat "$test_root/plasmawindowed.log"
    exit 1
fi
