import "UsagePresentation.js" as UsagePresentation
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

ColumnLayout {
    id: root

    required property var quota
    readonly property string usageState: UsagePresentation.state(Number(root.quota.usagePercent), false)
    readonly property real remaining: UsagePresentation.remaining(root.quota.usagePercent)
    readonly property string remainingText: UsagePresentation.percentage(root.remaining)

    spacing: 3

    RowLayout {
        Layout.fillWidth: true

        PlasmaComponents.Label {
            Layout.fillWidth: true
            text: root.quota.label
            elide: Text.ElideRight
        }

        PlasmaComponents.Label {
            text: qsTr("%1% left").arg(root.remainingText)
            color: root.usageState === "critical" ? Kirigami.Theme.negativeTextColor
                : root.usageState === "warning" ? Kirigami.Theme.neutralTextColor
                : Kirigami.Theme.positiveTextColor
        }
    }

    PlasmaComponents.ProgressBar {
        Layout.fillWidth: true
        from: 0
        to: 100
        value: root.remaining
        Accessible.name: qsTr("%1 remaining").arg(root.quota.label)
        Accessible.description: qsTr("%1 percent remaining").arg(root.remainingText)
    }

    RowLayout {
        Layout.fillWidth: true

        PlasmaComponents.Label {
            Layout.fillWidth: true
            text: root.quota.resetText
            opacity: 0.7
            elide: Text.ElideRight
        }

        PlasmaComponents.Label {
            visible: text.length > 0
            text: root.quota.statusText
            opacity: 0.7
        }
    }
}
