import "ProviderCards.js" as ProviderCards
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

Rectangle {
    id: root

    required property var card
    required property double currentTimeMs
    readonly property int rowCount: root.card.rows.length

    objectName: "providerCard-" + root.card.providerId
    implicitHeight: content.implicitHeight + Kirigami.Units.largeSpacing * 2
    radius: Kirigami.Units.smallSpacing
    color: root.card.stale ? Kirigami.Theme.alternateBackgroundColor : Kirigami.Theme.backgroundColor
    border.color: root.card.stale ? Kirigami.Theme.disabledTextColor : Kirigami.Theme.textColor
    Accessible.role: Accessible.Grouping
    Accessible.name: root.card.title

    ColumnLayout {
        id: content

        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.smallSpacing

        RowLayout {
            Layout.fillWidth: true

            ProviderIcon {
                Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                providerId: root.card.providerId
                opacity: root.card.stale ? 0.6 : 1
            }

            Kirigami.Heading {
                Layout.fillWidth: true
                level: 3
                text: root.card.title
                elide: Text.ElideRight
            }

            PlasmaComponents.Label {
                objectName: "staleBadge"
                visible: root.card.stale
                text: qsTr("Stale")
                color: Kirigami.Theme.disabledTextColor
                Accessible.name: qsTr("Stale data")
            }
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            visible: root.card.detailText.length > 0
            text: root.card.detailText
            wrapMode: Text.Wrap
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            visible: root.card.secondaryText.length > 0
            text: root.card.secondaryText
            opacity: 0.8
            wrapMode: Text.Wrap
        }

        Repeater {
            model: root.card.rows

            delegate: QuotaRow {
                required property var modelData

                Layout.fillWidth: true
                quota: modelData
            }
        }

        PlasmaComponents.Label {
            id: lastUpdatedLabel

            objectName: "lastUpdatedLabel"
            Layout.fillWidth: true
            text: qsTr("Last updated %1").arg(ProviderCards.ageText(root.card.lastUpdated, root.currentTimeMs))
            opacity: 0.7
            elide: Text.ElideRight
        }

        PlasmaComponents.Label {
            id: errorLabel

            objectName: "errorLabel"
            Layout.fillWidth: true
            visible: text.length > 0
            text: root.card.error
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
        }
    }
}
