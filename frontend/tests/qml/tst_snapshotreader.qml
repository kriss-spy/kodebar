import "../../package/contents/ui"
import QtCore
import QtQuick
import QtTest

TestCase {
    id: testCase

    function init() {
        reader.snapshotPath = StandardPaths.writableLocation(StandardPaths.GenericCacheLocation) + "/kodebar/last.json";
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
            "antigravity": {
                "type": "quota-based",
                "usagePercentage": 42,
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
        compare(reader.selection.value, "71%");
        verify(reader.hasSnapshot);
    }

    function test_invalidSnapshotPreservesLastValidSelection() {
        reader.readSnapshot(JSON.stringify({
            "_meta": {
                "version": 1
            },
            "antigravity": {
                "type": "quota-based",
                "usagePercentage": 42,
                "stale": false
            }
        }));
        reader.readSnapshot("not JSON");
        compare(reader.selection.providerId, "antigravity");
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
        compare(reader.selection.value, "67%");
    }

    name: "SnapshotReader"

    SnapshotReader {
        id: reader

        pollingEnabled: false
    }

}
