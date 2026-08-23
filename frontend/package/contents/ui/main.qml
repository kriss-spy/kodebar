pragma ComponentBehavior: Bound

import "ProviderCards.js" as ProviderCards
import "UsagePresentation.js" as UsagePresentation
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
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
    readonly property string compactText: selectedProvider.providerId ? qsTr("%1 %2").arg(ProviderCards.providerCompactTitle(selectedProvider.providerId)).arg(selectedProvider.value) : qsTr("No data")
    readonly property string compactState: UsagePresentation.state(root.selectedProvider.usage, root.selectedProvider.stale)
    readonly property color compactColor: root.compactState === "critical" ? Kirigami.Theme.negativeTextColor
        : root.compactState === "warning" ? Kirigami.Theme.neutralTextColor
        : root.compactState === "ok" ? Kirigami.Theme.positiveTextColor
        : root.compactState === "stale" ? Kirigami.Theme.disabledTextColor
        : Kirigami.Theme.textColor

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

    compactRepresentation: RowLayout {
        spacing: Kirigami.Units.smallSpacing

        ProviderIcon {
            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
            visible: root.selectedProvider.providerId.length > 0
            providerId: root.selectedProvider.providerId
            opacity: root.selectedProvider.stale ? 0.6 : 1
        }

        PlasmaComponents.Label {
            id: compactLabel

            Layout.preferredWidth: implicitWidth
            Layout.preferredHeight: implicitHeight
            text: root.compactText
            color: root.compactColor
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }

    fullRepresentation: FullRepresentation {
        snapshot: snapshotReader.snapshot
        enabledProviders: root.enabledProviders
    }

}
