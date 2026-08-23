import "ProviderSelection.js" as ProviderSelection
import QtCore
import QtQuick
import org.kde.plasma.plasma5support as Plasma5Support

Item {
    id: root

    property string snapshotPath: StandardPaths.writableLocation(StandardPaths.GenericCacheLocation) + "/kodebar/last.json"
    property var selection: ProviderSelection.selectProvider(null)
    property bool hasSnapshot: false
    property string errorMessage: ""
    property bool pollingEnabled: true

    function readSnapshot(output) {
        try {
            const snapshot = JSON.parse(output);
            if (!snapshot || typeof snapshot !== "object" || !snapshot._meta)
                throw new Error("invalid Snapshot root");

            root.selection = ProviderSelection.selectProvider(snapshot);
            root.hasSnapshot = true;
            root.errorMessage = "";
        } catch (error) {
            root.errorMessage = error.toString();
            if (!root.hasSnapshot)
                root.selection = ProviderSelection.selectProvider(null);

        }
    }

    function refresh() {
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
        interval: 30000
        running: root.pollingEnabled
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }

}
