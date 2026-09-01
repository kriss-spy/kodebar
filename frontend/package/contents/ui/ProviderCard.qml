import "ProviderCards.js" as ProviderCards
import "UsagePresentation.js" as UsagePresentation
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

Kirigami.AbstractCard {
    id: root

    signal loginRequested(string providerId)
    signal rowsExpansionRequested

    required property var card
    required property double currentTimeMs
    readonly property int rowCount: root.card.rows.length
    property bool rowsExpanded: false
    readonly property var visibleRows: root.rowsExpanded ? root.card.rows : root.card.rows.slice(0, 3)
    readonly property real mostUsed: {
        let value = -1
        for (let index = 0; index < root.card.rows.length; ++index)
            value = Math.max(value, Number(root.card.rows[index].usagePercent))
        return value
    }
    readonly property real remaining: UsagePresentation.remaining(root.mostUsed)
    readonly property string remainingText: UsagePresentation.percentage(root.remaining)

    objectName: "providerCard-" + root.card.providerId
    padding: Kirigami.Units.largeSpacing
    Accessible.role: Accessible.Grouping
    Accessible.name: root.card.title

    contentItem: ColumnLayout {
        id: content

        spacing: Kirigami.Units.smallSpacing

        RowLayout {
            Layout.fillWidth: true

            ProviderIcon {
                Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                providerId: root.card.providerId
                opacity: root.card.stale ? 0.6 : 1
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 3
                    text: root.card.title
                    elide: Text.ElideRight
                }

                PlasmaComponents.Label {
                    Layout.fillWidth: true
                    visible: root.card.detailText.length > 0
                    text: root.card.detailText
                    color: Kirigami.Theme.disabledTextColor
                    elide: Text.ElideRight
                }
            }

            Kirigami.Heading {
                visible: root.rowCount > 0
                Layout.alignment: Qt.AlignRight
                level: 3
                text: qsTr("%1% left").arg(root.remainingText)
                color: root.mostUsed >= 90 ? Kirigami.Theme.negativeTextColor
                    : root.mostUsed >= 70 ? Kirigami.Theme.neutralTextColor
                    : Kirigami.Theme.positiveTextColor
            }
        }

        RowLayout {
            Layout.fillWidth: true
            visible: root.card.stale

            Kirigami.Icon {
                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                Layout.preferredHeight: Kirigami.Units.iconSizes.small
                source: "data-warning"
            }

            PlasmaComponents.Label {
                objectName: "staleBadge"
                Layout.fillWidth: true
                text: qsTr("Last updated %1").arg(ProviderCards.ageText(root.card.lastUpdated, root.currentTimeMs))
                color: Kirigami.Theme.neutralTextColor
                Accessible.name: qsTr("Stale data")
            }
        }

        Repeater {
            model: root.visibleRows

            delegate: QuotaRow {
                required property var modelData

                Layout.fillWidth: true
                quota: modelData
            }
        }

        PlasmaComponents.ToolButton {
            Layout.alignment: Qt.AlignHCenter
            visible: root.rowCount > 3
            text: root.rowsExpanded
                ? qsTr("Show fewer")
                : qsTr("Show %1 more").arg(root.rowCount - root.visibleRows.length)
            icon.name: root.rowsExpanded ? "go-up" : "go-down"
            onClicked: root.rowsExpansionRequested()
        }

        PlasmaComponents.Button {
            Layout.alignment: Qt.AlignRight
            visible: root.card.actionLabel.length > 0
            text: root.card.actionLabel
            icon.name: "im-user"
            onClicked: root.loginRequested(root.card.providerId)
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            visible: root.card.secondaryText.length > 0
            text: root.card.secondaryText
            color: Kirigami.Theme.disabledTextColor
            wrapMode: Text.Wrap
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
