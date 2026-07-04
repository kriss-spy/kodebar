use clap::{Parser, Subcommand};
use serde::Serialize;
use std::process::ExitCode;

/// Kodebar — Linux-native AI provider usage tracker.
#[derive(Parser, Debug)]
#[command(name = "kodebar", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show the current provider usage snapshot.
    Status {
        /// Emit the raw snapshot as JSON to stdout.
        #[arg(long)]
        json: bool,
    },
}

/// Snapshot metadata.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotMeta {
    version: u32,
    last_updated: Option<String>,
}

/// The merged JSON result of all provider probes. The single boundary
/// between the backend and the frontend.
#[derive(Serialize)]
struct Snapshot {
    _meta: SnapshotMeta,
    providers: serde_json::Value,
}

fn empty_snapshot() -> Snapshot {
    Snapshot {
        _meta: SnapshotMeta {
            version: 1,
            last_updated: None,
        },
        providers: serde_json::json!({}),
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Some(Command::Status { json }) => {
            let snapshot = empty_snapshot();
            if json {
                let encoded = serde_json::to_string(&snapshot)
                    .map_err(|e| format!("failed to encode snapshot: {e}"))?;
                println!("{encoded}");
            } else {
                println!("No providers configured.");
            }
        }
        None => {
            println!("No providers configured.");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kodebar: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_json_is_valid_snapshot_with_version_one() {
        let snapshot = empty_snapshot();
        let encoded = serde_json::to_string(&snapshot).unwrap();
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value["_meta"]["version"], 1);
        assert!(value["providers"].is_object());
        assert!(value["providers"].as_object().unwrap().is_empty());
    }

    #[test]
    fn status_json_matches_expected_shape() {
        let snapshot = empty_snapshot();
        let encoded = serde_json::to_string(&snapshot).unwrap();
        let expected = r#"{"_meta":{"version":1,"lastUpdated":null},"providers":{}}"#;
        // Order-insensitive comparison via round-trip parse.
        let got: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        let want: serde_json::Value = serde_json::from_str(expected).unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn snapshot_last_updated_is_null_for_empty() {
        let snapshot = empty_snapshot();
        assert!(snapshot._meta.last_updated.is_none());
    }
}