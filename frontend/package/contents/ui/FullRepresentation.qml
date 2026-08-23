pragma ComponentBehavior: Bound

import "ProviderCards.js" as ProviderCards
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

Item {
    id: root

    property var snapshot: null
    property var enabledProviders: ({})
    property double currentTimeMs: Date.now()
    property bool countdownUpdatesEnabled: true
    readonly property var cards: ProviderCards.cards(root.snapshot, root.currentTimeMs, root.enabledProviders)

    implicitWidth: Kirigami.Units.gridUnit * 24
    implicitHeight: Math.min(cardColumn.implicitHeight, Kirigami.Units.gridUnit * 32)

    PlasmaComponents.ScrollView {
        id: scrollView

        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            id: cardColumn

            width: scrollView.availableWidth
            spacing: Kirigami.Units.smallSpacing

            Repeater {
                model: root.cards

                delegate: ProviderCard {
                    required property var modelData

                    Layout.fillWidth: true
                    card: modelData
                    currentTimeMs: root.currentTimeMs
                }
            }

            PlasmaComponents.Label {
                Layout.fillWidth: true
                visible: root.cards.length === 0
                text: qsTr("No provider data")
                horizontalAlignment: Text.AlignHCenter
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
