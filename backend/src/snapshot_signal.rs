//! Best-effort Frontend refresh signaling after a Snapshot is persisted.

/// Well-known name held while the update signal is emitted.
pub const BUS_NAME: &str = "ai.kodebar";
/// Object path carrying Kodebar's refresh signals.
pub const OBJECT_PATH: &str = "/ai/kodebar";
/// Interface carrying Kodebar's refresh signals.
pub const INTERFACE: &str = "ai.kodebar";
/// Emitted with no arguments after a Snapshot is atomically persisted.
pub const SNAPSHOT_UPDATED_SIGNAL: &str = "SnapshotUpdated";

/// Injectable boundary between Snapshot persistence and desktop IPC.
pub trait SnapshotNotifier {
    fn snapshot_updated(&self) -> Result<(), String>;
}

/// Emits Snapshot updates on the current user's D-Bus session bus.
pub struct SessionBusSnapshotNotifier;

impl SnapshotNotifier for SessionBusSnapshotNotifier {
    fn snapshot_updated(&self) -> Result<(), String> {
        let connection = zbus::blocking::Connection::session()
            .map_err(|error| format!("failed to connect to the session bus: {error}"))?;
        connection
            .request_name(BUS_NAME)
            .map_err(|error| format!("failed to own {BUS_NAME}: {error}"))?;
        connection
            .emit_signal(
                None::<&str>,
                OBJECT_PATH,
                INTERFACE,
                SNAPSHOT_UPDATED_SIGNAL,
                &(),
            )
            .map_err(|error| format!("failed to emit {SNAPSHOT_UPDATED_SIGNAL}: {error}"))
    }
}
