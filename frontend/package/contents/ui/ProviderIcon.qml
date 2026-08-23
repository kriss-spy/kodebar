import "ProviderCards.js" as ProviderCards
import QtQuick
import org.kde.kirigami as Kirigami

Kirigami.Icon {
    id: root

    required property string providerId
    readonly property string iconSource: ProviderCards.iconSource(root.providerId)
    readonly property var resolvedSource: root.iconSource.includes("/")
        ? Qt.resolvedUrl(root.iconSource)
        : root.iconSource

    source: root.resolvedSource
    isMask: true
    Accessible.ignored: true
}
