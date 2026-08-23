import "ProviderSelection.js" as ProviderSelection
import QtCore
import QtQuick
import org.kde.plasma.plasma5support as Plasma5Support

Item {
    id: root

    property string snapshotPath: root.localFilePath(StandardPaths.writableLocation(StandardPaths.GenericCacheLocation)) + "/kodebar/last.json"
    property var snapshot: null
    property var selectionOptions: ({})
    property var selection: ProviderSelection.selectProvider(null)
    property bool hasSnapshot: false
    property string errorMessage: ""
    property bool pollingEnabled: true
    property int pollingInterval: 30000

    function localFilePath(value) {
        const location = value.toString();
        return location.startsWith("file://")
            ? decodeURIComponent(location.substring(7))
            : location;
    }

    onSelectionOptionsChanged: {
        if (root.snapshot)
            root.selection = ProviderSelection.selectProvider(root.snapshot, root.selectionOptions);
    }

    function readSnapshot(output) {
        try {
            const snapshot = JSON.parse(output);
            if (!snapshot || typeof snapshot !== "object" || !snapshot._meta)
                throw new Error("invalid Snapshot root");

            root.snapshot = snapshot;
            root.selection = ProviderSelection.selectProvider(snapshot, root.selectionOptions);
            root.hasSnapshot = true;
            root.errorMessage = "";
        } catch (error) {
            root.errorMessage = error.toString();
            if (!root.hasSnapshot)
                root.selection = ProviderSelection.selectProvider(null);

        }
    }

    function refresh() {
        // Plasma 6 pure QML has no general local-file content reader. Keep this
        // compatibility adapter fixed, local, and limited to the Snapshot path.
        const command = "/usr/bin/cat -- " + root.shellQuote(root.snapshotPath);
        snapshotSource.disconnectSource(command);
        snapshotSource.connectSource(command);
    }

    function shellQuote(value) {
        return "'" + value.replace(/'/g, "'\\''") + "'";
    }

    Plasma5Support.DataSource {
        id: snapshotSource

        engine: "executable"
        onNewData: function(sourceName, data) {
            snapshotSource.disconnectSource(sourceName);
            root.readSnapshot(data["exit code"] === 0 ? data.stdout : "");
        }
    }

    Timer {
        interval: root.pollingInterval
        running: root.pollingEnabled
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }

}
