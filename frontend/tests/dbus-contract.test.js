const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const repositoryRoot = path.join(__dirname, "../..");
const backend = fs.readFileSync(path.join(repositoryRoot, "backend/src/snapshot_signal.rs"), "utf8");
const watcher = fs.readFileSync(path.join(repositoryRoot, "frontend/package/contents/ui/SnapshotSignalWatcher.qml"), "utf8");
const main = fs.readFileSync(path.join(repositoryRoot, "frontend/package/contents/ui/main.qml"), "utf8");

const contract = {
    service: backend.match(/BUS_NAME: &str = "([^"]+)"/)[1],
    path: backend.match(/OBJECT_PATH: &str = "([^"]+)"/)[1],
    iface: backend.match(/INTERFACE: &str = "([^"]+)"/)[1],
    member: backend.match(/SNAPSHOT_UPDATED_SIGNAL: &str = "([^"]+)"/)[1],
};

assert.match(watcher, new RegExp(`service: "${contract.service}"`));
assert.match(watcher, new RegExp(`path: "${contract.path}"`));
assert.match(watcher, new RegExp(`iface: "${contract.iface}"`));
assert.match(watcher, new RegExp(`function dbus${contract.member}\\(\\)`));
assert.match(main, /onRefreshRequested: snapshotReader\.refresh\(\)/);
