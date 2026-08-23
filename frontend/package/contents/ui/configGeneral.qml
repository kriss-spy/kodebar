pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami

KCM.SimpleKCM {
    id: root

    property alias cfg_enabledAntigravity: antigravity.checked
    property alias cfg_enabledChatGPT: chatGpt.checked
    property alias cfg_enabledOpenCodeGo: openCodeGo.checked
    property alias cfg_enabledOpenCodeZen: openCodeZen.checked
    property alias cfg_refreshIntervalSeconds: refreshInterval.value
    property string cfg_compactProvider

    function syncCompactProvider() {
        const index = compactProvider.indexOfValue(root.cfg_compactProvider);
        compactProvider.currentIndex = index >= 0 ? index : 0;
    }

    onCfg_compactProviderChanged: root.syncCompactProvider()

    Component.onCompleted: root.syncCompactProvider()

    Kirigami.FormLayout {
        CheckBox {
            id: antigravity
            text: qsTr("Antigravity")
        }

        CheckBox {
            id: chatGpt
            text: qsTr("ChatGPT")
        }

        CheckBox {
            id: openCodeGo
            text: qsTr("OpenCode Go")
        }

        CheckBox {
            id: openCodeZen
            text: qsTr("OpenCode Zen")
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
                { text: qsTr("OpenCode Go"), value: "opencode_go" },
                { text: qsTr("OpenCode Zen"), value: "opencode_zen" },
            ]
            onActivated: root.cfg_compactProvider = currentValue
        }
    }
}
