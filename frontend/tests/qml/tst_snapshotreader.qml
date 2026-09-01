import "../../package/contents/ui"
import QtCore
import QtQuick
import QtTest

TestCase {
    id: testCase

    function init() {
        reader.snapshotPath = reader.localFilePath(StandardPaths.writableLocation(StandardPaths.GenericCacheLocation)) + "/kodebar/last.json";
        reader.snapshot = null;
        reader.selectionOptions = ({});
        reader.selection = ({
            "providerId": "",
            "value": "",
            "usage": null,
            "stale": false
        });
        reader.hasSnapshot = false;
        reader.errorMessage = "";
    }

    function test_readsHighestActionableProvider() {
        reader.readSnapshot(JSON.stringify({
            "_meta": {
                "version": 1
            },
            "chatgpt": {
                "type": "quota-based",
                "limits": {
                    "codex": { "primary": { "usagePercent": 42 } }
                },
                "stale": false
            },
            "opencode_go": {
                "type": "quota-based",
                "windows": {
                    "rolling": {
                        "usagePercent": 71,
                        "status": "ok"
                    }
                },
                "stale": false
            }
        }));
        compare(reader.selection.providerId, "opencode_go");
        compare(reader.selection.value, "29% left");
        compare(reader.snapshot.opencode_go.windows.rolling.usagePercent, 71);
        verify(reader.hasSnapshot);
    }

    function test_defaultSnapshotPathIsLocalFilesystemPath() {
        verify(!reader.snapshotPath.startsWith("file:"));
        verify(reader.snapshotPath.endsWith("/kodebar/last.json"));
    }

    function test_settingsFilterAndPinCompactProvider() {
        reader.selectionOptions = ({
            "enabledProviders": {
                "chatgpt": false,
                "opencode_go": true
            },
            "compactProvider": "opencode_go"
        });

        reader.readSnapshot(JSON.stringify({
            "_meta": { "version": 1 },
            "chatgpt": {
                "type": "quota-based",
                "usagePercentage": 95,
                "stale": false
            },
            "opencode_go": {
                "type": "quota-based",
                "windows": {
                    "rolling": { "usagePercent": 35, "status": "ok" }
                },
                "stale": false
            }
        }));

        compare(reader.selection.providerId, "opencode_go");
        compare(reader.selection.value, "65% left");
    }

    function test_settingsChangesReselectLastValidSnapshotImmediately() {
        reader.readSnapshot(JSON.stringify({
            "_meta": { "version": 1 },
            "chatgpt": {
                "type": "quota-based",
                "usagePercentage": 95,
                "stale": false
            },
            "opencode_go": {
                "type": "quota-based",
                "windows": {
                    "rolling": { "usagePercent": 35, "status": "ok" }
                },
                "stale": false
            }
        }));
        compare(reader.selection.providerId, "chatgpt");

        reader.selectionOptions = ({ "compactProvider": "opencode_go" });

        compare(reader.selection.providerId, "opencode_go");
    }

    function test_invalidSnapshotPreservesLastValidSelection() {
        reader.readSnapshot(JSON.stringify({
            "_meta": {
                "version": 1
            },
            "chatgpt": {
                "type": "quota-based",
                "usagePercentage": 42,
                "stale": false
            }
        }));
        const lastValidSnapshot = reader.snapshot;
        reader.readSnapshot("not JSON");
        compare(reader.selection.providerId, "chatgpt");
        compare(reader.snapshot, lastValidSnapshot);
        verify(reader.errorMessage.length > 0);
    }

    function test_invalidInitialSnapshotHasSafeEmptySelection() {
        reader.readSnapshot("");
        compare(reader.selection.providerId, "");
        verify(!reader.hasSnapshot);
    }

    function test_refreshReadsSnapshotFile() {
        reader.snapshotPath = Qt.resolvedUrl("../fixtures/snapshot.json").toString().replace(/^file:\/\//, "");

        reader.refresh();

        tryCompare(reader, "hasSnapshot", true, 2000);
        compare(reader.selection.providerId, "chatgpt");
        compare(reader.selection.value, "33% left");
    }

    name: "SnapshotReader"

    SnapshotReader {
        id: reader

        pollingEnabled: false
    }

}
