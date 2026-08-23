import "../../package/contents/ui"
import QtQuick
import QtTest

TestCase {
    id: testCase

    name: "SnapshotSignalWatcher"

    SnapshotSignalWatcher {
        id: watcher
    }

    SignalSpy {
        id: refreshSpy

        target: watcher
        signalName: "refreshRequested"
    }

    function init() {
        refreshSpy.clear();
    }

    function test_snapshotUpdatedRequestsRefresh() {
        watcher.dbusSnapshotUpdated();

        compare(refreshSpy.count, 1);
    }

    function test_matchesKodebarSignalContract() {
        compare(watcher.service, "ai.kodebar");
        compare(watcher.path, "/ai/kodebar");
        compare(watcher.iface, "ai.kodebar");
    }

}
