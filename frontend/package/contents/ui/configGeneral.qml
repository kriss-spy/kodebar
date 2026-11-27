pragma ComponentBehavior: Bound

import "ProviderCards.js" as ProviderCards
import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami

KCM.SimpleKCM {
    id: root

    property alias cfg_enabledAntigravity: antigravity.checked
    property alias cfg_enabledFishAudio: fishAudio.checked
    property alias cfg_enabledXPremium: xPremium.checked
    property alias cfg_enabledChatGPT: chatGpt.checked
    property alias cfg_enabledOpenCodeGo: openCodeGo.checked
    property alias cfg_refreshIntervalSeconds: refreshInterval.value
    property string cfg_providerOrder
    property int selectedOrderIndex: 0
    readonly property var orderIds: ProviderCards.providerIds(root.cfg_providerOrder).filter(function(id) {
        return id !== "opencode_zen";
    })
    property string cfg_compactProvider

    function moveProvider(delta) {
        const ids = root.orderIds.slice();
        const target = root.selectedOrderIndex + delta;
        if (target < 0 || target >= ids.length)
            return;
        const moved = ids.splice(root.selectedOrderIndex, 1)[0];
        ids.splice(target, 0, moved);
        root.cfg_providerOrder = ids.join(",");
        root.selectedOrderIndex = target;
    }


    function syncCompactProvider() {
        const index = compactProvider.indexOfValue(root.cfg_compactProvider);
        compactProvider.currentIndex = index >= 0 ? index : 0;
    }

    onCfg_compactProviderChanged: root.syncCompactProvider()

    Component.onCompleted: root.syncCompactProvider()

    Kirigami.FormLayout {
        Kirigami.Heading {
            Kirigami.FormData.isSection: true
            text: qsTr("Subscription plans")
            level: 3
        }

        CheckBox {
            id: antigravity
            text: qsTr("Antigravity")
        }

        CheckBox {
            id: fishAudio
            text: qsTr("Fish Audio")
        }

        CheckBox {
            id: xPremium
            text: qsTr("X Premium")
        }

        CheckBox {
            id: chatGpt
            text: qsTr("ChatGPT")
        }

        CheckBox {
            id: openCodeGo
            text: qsTr("OpenCode Go")
        }

        ColumnLayout {
            Kirigami.FormData.label: qsTr("Provider order:")
            spacing: Kirigami.Units.smallSpacing

            Repeater {
                model: root.orderIds

                ItemDelegate {
                    required property int index
                    required property string modelData
                    Layout.fillWidth: true
                    text: qsTr("%1. %2").arg(index + 1).arg(ProviderCards.providerDefinition(modelData).title)
                    highlighted: root.selectedOrderIndex === index
                    onClicked: root.selectedOrderIndex = index
                }
            }

            RowLayout {
                Button {
                    text: qsTr("Move up")
                    icon.name: "go-up"
                    enabled: root.selectedOrderIndex > 0
                    onClicked: root.moveProvider(-1)
                }
                Button {
                    text: qsTr("Move down")
                    icon.name: "go-down"
                    enabled: root.selectedOrderIndex < root.orderIds.length - 1
                    onClicked: root.moveProvider(1)
                }
            }
        }

        SpinBox {
            id: refreshInterval

            Kirigami.FormData.label: qsTr("Snapshot check interval:")
            from: 10
            to: 3600
            stepSize: 10
            editable: true
            textFromValue: function(value) {
                return qsTr("%1 seconds").arg(value);
            }
            valueFromText: function(text) {
                return parseInt(text, 10);
            }
        }

        ComboBox {
            id: compactProvider

            Kirigami.FormData.label: qsTr("Compact Provider:")
            textRole: "text"
            valueRole: "value"
            model: [
                { text: qsTr("Highest usage"), value: "highest" },
                { text: qsTr("Antigravity"), value: "antigravity" },
                { text: qsTr("ChatGPT"), value: "chatgpt" },
                { text: qsTr("X Premium"), value: "x_premium" },
                { text: qsTr("Fish Audio"), value: "fish_audio" },
                { text: qsTr("OpenCode Go"), value: "opencode_go" },
            ]
            onActivated: root.cfg_compactProvider = currentValue
        }

    }
}
