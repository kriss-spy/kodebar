pragma ComponentBehavior: Bound

import "ProviderCards.js" as ProviderCards
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

Item {
    id: root

    signal refreshRequested
    signal configureRequested
    signal loginRequested(string providerId)

    property var snapshot: null
    property string readerError: ""
    property var enabledProviders: ({})
    property double currentTimeMs: Date.now()
    property bool countdownUpdatesEnabled: true
    property var expandedProviders: ({})
    readonly property var cards: ProviderCards.cards(root.snapshot, root.currentTimeMs, root.enabledProviders)

    function toggleProviderRows(providerId) {
        const next = Object.assign({}, root.expandedProviders)
        next[providerId] = next[providerId] !== true
        root.expandedProviders = next
    }

    Layout.minimumWidth: Kirigami.Units.gridUnit * 14
    Layout.preferredWidth: Kirigami.Units.gridUnit * 16
    implicitWidth: Layout.preferredWidth
    implicitHeight: Math.min(mainLayout.implicitHeight, Kirigami.Units.gridUnit * 34)

    ColumnLayout {
        id: mainLayout

        anchors.fill: parent
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.smallSpacing
            Layout.topMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.smallSpacing

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 2
                    text: qsTr("Kodebar")
                }
            }

            PlasmaComponents.ToolButton {
                id: refreshButton

                icon.name: "view-refresh"
                text: qsTr("Refresh")
                display: PlasmaComponents.AbstractButton.IconOnly
                Accessible.name: text
                onClicked: root.refreshRequested()

                PlasmaComponents.ToolTip {
                    text: refreshButton.text
                }
            }

            PlasmaComponents.ToolButton {
                id: configureButton

                icon.name: "configure"
                text: qsTr("Configure Kodebar")
                display: PlasmaComponents.AbstractButton.IconOnly
                Accessible.name: text
                onClicked: root.configureRequested()

                PlasmaComponents.ToolTip {
                    text: configureButton.text
                }
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.smallSpacing
            Layout.rightMargin: Kirigami.Units.smallSpacing
            Layout.topMargin: visible ? Kirigami.Units.smallSpacing : 0
            visible: root.readerError.length > 0
            type: Kirigami.MessageType.Warning
            text: root.snapshot
                ? qsTr("Could not refresh usage. Showing the last reading.")
                : qsTr("Kodebar has not received usage data yet.")
        }

        PlasmaComponents.ScrollView {
            id: scrollView

            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: Math.min(cardColumn.implicitHeight + Kirigami.Units.smallSpacing * 2,
                Kirigami.Units.gridUnit * 28)
            Layout.minimumHeight: Math.min(cardColumn.implicitHeight, Kirigami.Units.gridUnit * 12)

            // The card column never needs horizontal scrolling — it always
            // sizes to the available width and elides its contents.
            PlasmaComponents.ScrollBar.horizontal.policy: PlasmaComponents.ScrollBar.AlwaysOff

            ColumnLayout {
                id: cardColumn

                width: scrollView.availableWidth
                spacing: Kirigami.Units.smallSpacing

                Repeater {
                    model: root.cards

                    delegate: ProviderCard {
                        required property var modelData
                        required property int index

                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.smallSpacing
                        Layout.rightMargin: Kirigami.Units.smallSpacing
                        Layout.topMargin: index === 0 ? Kirigami.Units.smallSpacing : 0
                        Layout.bottomMargin: index === root.cards.length - 1 ? Kirigami.Units.smallSpacing : 0
                        card: modelData
                        currentTimeMs: root.currentTimeMs
                        rowsExpanded: root.expandedProviders[modelData.providerId] === true
                        onLoginRequested: providerId => root.loginRequested(providerId)
                        onRowsExpansionRequested: root.toggleProviderRows(modelData.providerId)
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    Layout.topMargin: Kirigami.Units.gridUnit * 2
                    Layout.bottomMargin: Kirigami.Units.gridUnit * 2
                    visible: root.cards.length === 0

                    Kirigami.Icon {
                        Layout.alignment: Qt.AlignHCenter
                        Layout.preferredWidth: Kirigami.Units.iconSizes.large
                        Layout.preferredHeight: Kirigami.Units.iconSizes.large
                        source: "network-disconnect"
                    }

                    Kirigami.Heading {
                        Layout.fillWidth: true
                        level: 3
                        text: qsTr("No plans enabled")
                        horizontalAlignment: Text.AlignHCenter
                    }

                    PlasmaComponents.Label {
                        Layout.fillWidth: true
                        text: qsTr("Enable a provider in Kodebar settings to begin.")
                        color: Kirigami.Theme.disabledTextColor
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                    }
                }
            }
        }
    }

    Timer {
        interval: 30000
        running: root.visible && root.countdownUpdatesEnabled
        repeat: true
        onTriggered: root.currentTimeMs = Date.now()
    }
}
