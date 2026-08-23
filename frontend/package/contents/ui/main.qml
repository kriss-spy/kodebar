pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.plasmoid

PlasmoidItem {
    id: root

    readonly property var enabledProviders: ({
        "antigravity": Plasmoid.configuration.enabledAntigravity,
        "chatgpt": Plasmoid.configuration.enabledChatGPT,
        "opencode_go": Plasmoid.configuration.enabledOpenCodeGo,
        "opencode_zen": Plasmoid.configuration.enabledOpenCodeZen
    })
    readonly property var selectionOptions: ({
        "enabledProviders": root.enabledProviders,
        "compactProvider": Plasmoid.configuration.compactProvider
    })
    readonly property var selectedProvider: snapshotReader.selection
    readonly property string compactText: selectedProvider.providerId ? qsTr("%1 %2").arg(root.providerName(selectedProvider.providerId)).arg(selectedProvider.value) : qsTr("No data")

    function providerName(providerId) {
        switch (providerId) {
        case "antigravity":
            return qsTr("Antigravity");
        case "chatgpt":
            return qsTr("ChatGPT");
        case "opencode_go":
            return qsTr("Go");
        case "opencode_zen":
            return qsTr("Zen");
        default:
            return providerId;
        }
    }

    preferredRepresentation: compactRepresentation
    toolTipMainText: qsTr("Kodebar")
    toolTipSubText: root.compactText

    SnapshotReader {
        id: snapshotReader

        selectionOptions: root.selectionOptions
        pollingInterval: Math.max(10, Math.min(3600, Number(Plasmoid.configuration.refreshIntervalSeconds) || 60)) * 1000
    }

    SnapshotSignalWatcher {
        onRefreshRequested: snapshotReader.refresh()
    }

    compactRepresentation: PlasmaComponents.Label {
        id: compactLabel

        Layout.preferredWidth: implicitWidth
        Layout.preferredHeight: implicitHeight
        text: root.compactText
        opacity: root.selectedProvider.stale ? 0.6 : 1
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }

    fullRepresentation: FullRepresentation {
        snapshot: snapshotReader.snapshot
        enabledProviders: root.enabledProviders
    }

}
