pragma ComponentBehavior: Bound

import "ProviderCards.js" as ProviderCards
import "UsagePresentation.js" as UsagePresentation
import "WheelNavigation.js" as WheelNavigation
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasma5support as Plasma5Support
import org.kde.plasma.plasmoid

PlasmoidItem {
    id: root

    readonly property var enabledProviders: ({
        "chatgpt": Plasmoid.configuration.enabledChatGPT,
        "opencode_go": Plasmoid.configuration.enabledOpenCodeGo,
        "opencode_zen": false
    })
    readonly property var selectionOptions: ({
        "enabledProviders": root.enabledProviders,
        "compactProvider": Plasmoid.configuration.compactProvider
    })
    readonly property var selectedProvider: snapshotReader.selection
    readonly property string compactText: selectedProvider.providerId ? qsTr("%1 %2").arg(ProviderCards.providerCompactTitle(selectedProvider.providerId)).arg(selectedProvider.value) : qsTr("No quota data")
    readonly property string compactState: UsagePresentation.state(root.selectedProvider.usage, root.selectedProvider.stale)
    readonly property color compactColor: root.compactState === "critical" ? Kirigami.Theme.negativeTextColor
        : root.compactState === "warning" ? Kirigami.Theme.neutralTextColor
        : root.compactState === "ok" ? Kirigami.Theme.positiveTextColor
        : root.compactState === "stale" ? Kirigami.Theme.disabledTextColor
        : Kirigami.Theme.textColor
    readonly property string pollCommand: "systemctl --user start kodebar.service"

    function startGuidedLogin(providerId) {
        const commands = {
            "chatgpt": "konsole -e codex login",
            "opencode_go": "konsole -e kodebar login opencode",
        }
        const command = commands[providerId]
        if (!command)
            return
        loginRunner.disconnectSource(command)
        loginRunner.connectSource(command)
    }

    function refreshUsage() {
        refreshRunner.disconnectSource(root.pollCommand)
        refreshRunner.connectSource(root.pollCommand)
    }

    toolTipMainText: qsTr("Kodebar")
    toolTipSubText: root.compactText
    preferredRepresentation: compactRepresentation

    SnapshotReader {
        id: snapshotReader

        selectionOptions: root.selectionOptions
        pollingInterval: Math.max(10, Math.min(3600, Number(Plasmoid.configuration.refreshIntervalSeconds) || 60)) * 1000
    }

    SnapshotSignalWatcher {
        onRefreshRequested: snapshotReader.refresh()
    }

    Plasma5Support.DataSource {
        id: loginRunner

        engine: "executable"
        onNewData: function(sourceName) {
            loginRunner.disconnectSource(sourceName)
            if (sourceName.startsWith("konsole -e ")) {
                root.refreshUsage()
            }
        }
    }

    Plasma5Support.DataSource {
        id: refreshRunner

        engine: "executable"
        onNewData: function(sourceName) {
            refreshRunner.disconnectSource(sourceName)
            snapshotReader.refresh()
        }
    }

    compactRepresentation: Item {
        id: compactItem

        readonly property bool horizontal: Plasmoid.formFactor !== PlasmaCore.Types.Vertical
        readonly property real pixelWheelStepSize: Kirigami.Units.gridUnit * 2
        readonly property real remainingFraction: root.selectedProvider.usage === null
            ? 0
            : Math.max(0, Math.min(100, 100 - root.selectedProvider.usage)) / 100
        property real angleWheelRemainder: 0
        property real pixelWheelRemainder: 0

        function handleWheel(wheel) {
            const usesAngleDelta = wheel.angleDelta.y !== 0
            const delta = usesAngleDelta ? wheel.angleDelta.y : wheel.pixelDelta.y
            if (delta === 0 || !snapshotReader.canCycleCompactProvider())
                return false

            const result = WheelNavigation.consumeDelta(
                delta,
                usesAngleDelta ? compactItem.angleWheelRemainder : compactItem.pixelWheelRemainder,
                usesAngleDelta ? 120 : compactItem.pixelWheelStepSize)
            if (usesAngleDelta)
                compactItem.angleWheelRemainder = result.remainder
            else
                compactItem.pixelWheelRemainder = result.remainder
            if (result.steps !== 0)
                snapshotReader.cycleCompactProviderSteps(-result.steps)
            return true
        }

        implicitWidth: Plasmoid.formFactor === PlasmaCore.Types.Vertical
            ? Math.min(compactRow.implicitWidth, Kirigami.Units.iconSizes.smallMedium)
            : compactRow.implicitWidth
        implicitHeight: compactRow.implicitHeight
        clip: true
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: qsTr("Open Kodebar, %1").arg(root.compactText)

        Keys.onSpacePressed: root.expanded = !root.expanded
        Keys.onReturnPressed: root.expanded = !root.expanded

        TapHandler {
            onTapped: root.expanded = !root.expanded
        }

        MouseArea {
            anchors.fill: parent
            enabled: !root.expanded
            acceptedButtons: Qt.NoButton
            propagateComposedEvents: true
            onWheel: wheel => wheel.accepted = compactItem.handleWheel(wheel)
        }

        RowLayout {
            id: compactRow

            anchors.centerIn: parent
            width: Math.min(implicitWidth, compactItem.width)
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

                Layout.preferredWidth: Math.min(implicitWidth, Kirigami.Units.gridUnit * 10)
                visible: compactItem.horizontal
                text: root.compactText
                color: root.compactColor
                elide: Text.ElideRight
                wrapMode: Text.NoWrap
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
        }

        Rectangle {
            id: compactUnderlineTrack

            anchors.horizontalCenter: compactItem.horizontal ? parent.horizontalCenter : undefined
            anchors.bottom: compactItem.horizontal ? parent.bottom : undefined
            anchors.verticalCenter: compactItem.horizontal ? undefined : parent.verticalCenter
            anchors.right: compactItem.horizontal ? undefined : parent.right
            width: compactItem.horizontal ? compactRow.width : 2
            height: compactItem.horizontal ? 2 : parent.height
            radius: 1
            visible: root.selectedProvider.usage !== null
            color: root.compactColor
            opacity: 0.25
        }

        Rectangle {
            id: compactUnderlineFill

            anchors.bottom: compactItem.horizontal ? parent.bottom : undefined
            anchors.left: compactItem.horizontal ? compactUnderlineTrack.left : undefined
            anchors.right: compactItem.horizontal ? undefined : parent.right
            anchors.verticalCenter: compactItem.horizontal ? undefined : compactUnderlineTrack.verticalCenter
            width: compactItem.horizontal
                ? compactUnderlineTrack.width * compactItem.remainingFraction
                : 2
            height: compactItem.horizontal
                ? 2
                : compactUnderlineTrack.height * compactItem.remainingFraction
            radius: 1
            visible: compactUnderlineTrack.visible
            color: root.compactColor
            Accessible.role: Accessible.ProgressBar
            Accessible.name: root.compactText
            Accessible.description: qsTr("%1% remaining").arg(Math.round(compactItem.remainingFraction * 100))
        }
    }

    fullRepresentation: FullRepresentation {
        snapshot: snapshotReader.snapshot
        readerError: snapshotReader.errorMessage
        enabledProviders: root.enabledProviders
        onRefreshRequested: root.refreshUsage()
        onConfigureRequested: Plasmoid.internalAction("configure").trigger()
        onLoginRequested: providerId => root.startGuidedLogin(providerId)
    }

}
