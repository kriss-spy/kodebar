import QtQuick
import org.kde.plasma.workspace.dbus as DBus

DBus.SignalWatcher {
    id: root

    signal refreshRequested

    enabled: true
    busType: DBus.BusType.Session
    service: "ai.kodebar"
    path: "/ai/kodebar"
    iface: "ai.kodebar"

    // Plasma dispatches a D-Bus member named SnapshotUpdated to this generated
    // dbus<Member> handler name.
    function dbusSnapshotUpdated() {
        root.refreshRequested();
    }
}
