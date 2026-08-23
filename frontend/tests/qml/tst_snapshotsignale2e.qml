import "../../package/contents/ui"
import QtQuick
import QtTest

TestCase {
    id: testCase

    name: "SnapshotSignalE2E"

    SnapshotReader {
        id: snapshotReader

        pollingEnabled: false
    }

    SnapshotSignalWatcher {
        id: watcher

        onRefreshRequested: snapshotReader.refresh()
    }

    SignalSpy {
        id: refreshSpy

        target: watcher
        signalName: "refreshRequested"
    }

    function test_backendSignalRefreshesPersistedSnapshot() {
        verify(!snapshotReader.hasSnapshot);
        tryCompare(refreshSpy, "count", 1, 5000);
        wait(2000);
        verify(snapshotReader.hasSnapshot,
               snapshotReader.snapshotPath + ": " + snapshotReader.errorMessage);
        compare(snapshotReader.snapshot._meta.version, 1);
    }
}
