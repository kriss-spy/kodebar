import "UsagePresentation.js" as UsagePresentation
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

ColumnLayout {
    id: root

    required property var quota
    readonly property string usageState: UsagePresentation.state(Number(root.quota.usagePercent), false)

    spacing: 3

    RowLayout {
        Layout.fillWidth: true

        PlasmaComponents.Label {
            Layout.fillWidth: true
            text: root.quota.label
            elide: Text.ElideRight
        }

        PlasmaComponents.Label {
            text: qsTr("%1%").arg(Math.round(root.quota.usagePercent))
            color: root.usageState === "critical" ? Kirigami.Theme.negativeTextColor
                : root.usageState === "warning" ? Kirigami.Theme.neutralTextColor
                : Kirigami.Theme.positiveTextColor
        }
    }

    PlasmaComponents.ProgressBar {
        Layout.fillWidth: true
        from: 0
        to: 100
        value: Math.max(0, Math.min(100, root.quota.usagePercent))
        Accessible.name: qsTr("%1 usage").arg(root.quota.label)
        Accessible.description: qsTr("%1 percent used").arg(Math.round(root.quota.usagePercent))
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
