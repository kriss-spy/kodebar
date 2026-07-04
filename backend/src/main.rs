mod probe;

use clap::{Parser, Subcommand};
use probe::antigravity::{self, AntigravityPayload};
use probe::{CodeAssistClient, ProbeError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
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
    /// Write the snapshot to the cache file only, with no stdout output.
    /// Intended for use by a systemd timer.
    Poll,
}

/// Snapshot metadata.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SnapshotMeta {
    version: u32,
    last_updated: Option<String>,
}

/// The provider-specific payload carried by a [`ProviderEntry`]. Each variant
/// serializes its fields inline so the entry is a flat object keyed by
/// provider ID, matching PRD §5.5. `#[serde(untagged)]` is safe here because
/// each variant carries a distinct `type` discriminator (`quota-based` vs
/// `pay-as-you-go`) and structurally disjoint fields.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum ProviderPayload {
    Antigravity(AntigravityPayload),
}

/// A single provider's entry in the Snapshot. Carries the Kodebar-specific
/// `stale` / `last_updated` extensions (PRD §5.5) alongside the
/// provider-specific [`ProviderPayload`] via `#[serde(flatten)]`, so the two
/// common fields sit at the same level as `type`, `usagePercentage`, etc.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ProviderEntry {
    #[serde(flatten)]
    payload: ProviderPayload,
    /// True when serving last-known-good data after the most recent Probe
    /// failed. See backend/CONTEXT.md "Stale".
    stale: bool,
    /// ISO 8601 timestamp of the last successful Probe for this provider.
    /// `None` until a probe has succeeded.
    last_updated: Option<String>,
}

impl ProviderEntry {
    /// Build a fresh (non-stale) entry from a successful Probe.
    fn fresh(payload: ProviderPayload, now: String) -> Self {
        Self {
            payload,
            stale: false,
            last_updated: Some(now),
        }
    }

    /// Build a stale entry, preserving the prior entry's payload and
    /// `last_updated` (last-known-good). For the no-prior-data case pass an
    /// [`ProviderPayload::Antigravity`] built from
    /// [`AntigravityPayload::empty`].
    fn stale_from_prior(prior: ProviderEntry) -> Self {
        Self {
            payload: prior.payload,
            stale: true,
            last_updated: prior.last_updated,
        }
    }
}

/// The merged JSON result of all provider probes, written to
/// `~/.cache/kodebar/last.json`. The single boundary between Backend and
/// Frontend. See PRD §5.5 and backend/CONTEXT.md "Snapshot".
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Snapshot {
    _meta: SnapshotMeta,
    #[serde(default)]
    providers: BTreeMap<String, ProviderEntry>,
}

fn empty_snapshot() -> Snapshot {
    Snapshot {
        _meta: SnapshotMeta {
            version: 1,
            last_updated: None,
        },
        providers: BTreeMap::new(),
    }
}

/// ISO 8601 UTC timestamp in the form the Snapshot schema expects (PRD §5.5
/// example: `2026-07-02T11:17:00Z`).
fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Resolve the cache directory for the snapshot.
///
/// Honours `XDG_CACHE_HOME` per the Linux convention, falling back to
/// `~/.cache/kodebar`. The cache filename is `last.json` (PRD §5.5).
fn default_cache_dir() -> Result<PathBuf, String> {
    let base = std::env::var("XDG_CACHE_HOME").or_else(|_| {
        std::env::var("HOME")
            .map(|h| format!("{h}/.cache"))
            .map_err(|_| "XDG_CACHE_HOME and HOME are both unset".to_string())
    })?;
    Ok(PathBuf::from(base).join("kodebar"))
}

/// Atomically write the snapshot as JSON to `<dir>/last.json`.
///
/// The directory is created (0700) if missing. The payload is written to a
/// tempfile in the same directory and then `rename`d into place so a reader
/// (e.g. the Plasmoid) never observes a partial write.
fn write_snapshot_atomic(dir: &Path, snapshot: &Snapshot) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|e| format!("failed to create cache dir {}: {e}", dir.display()))?;

    // Pin the directory permissions to 0700. `create_dir_all` may inherit the
    // parent's umask, so we set it explicitly for both fresh and pre-existing
    // directories.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("failed to set cache dir perms: {e}"))?;
    }

    let encoded = serde_json::to_string_pretty(snapshot)
        .map_err(|e| format!("failed to encode snapshot: {e}"))?;

    let final_path = dir.join("last.json");
    // Tempfile in the same directory so the rename is atomic on the same
    // filesystem. A pid-suffixed name keeps concurrent polls from colliding.
    let tmp_path = dir.join(format!(".last.json.tmp.{}", std::process::id()));

    fs::write(&tmp_path, encoded)
        .map_err(|e| format!("failed to write tempfile {}: {e}", tmp_path.display()))?;
    fs::rename(&tmp_path, &final_path).map_err(|e| {
        // Best-effort cleanup of the tempfile on rename failure.
        let _ = fs::remove_file(&tmp_path);
        format!("failed to rename snapshot into place: {e}")
    })?;
    Ok(())
}

fn render_human(snapshot: &Snapshot) -> String {
    if snapshot.providers.is_empty() {
        "No providers configured.".to_string()
    } else {
        let count = snapshot.providers.len();
        format!("{count} provider(s) configured.")
    }
}

/// Run the Antigravity Probe against an injectable client and merge the
/// result into `providers`. On failure the provider is flagged `stale`
/// rather than dropped — last-known-good from `prior` is preserved (PRD
/// §7.1, §7.5). `ProbeError::NoCredentials` means the provider isn't
/// configured on this machine, so the entry is simply omitted.
fn probe_antigravity<C: CodeAssistClient>(
    client: &C,
    prior: Option<ProviderEntry>,
    providers: &mut BTreeMap<String, ProviderEntry>,
) {
    match antigravity::run(client, &antigravity::gemini_dir(), true) {
        Ok(payload) => {
            providers.insert(
                "antigravity".to_string(),
                ProviderEntry::fresh(ProviderPayload::Antigravity(payload), now_iso()),
            );
        }
        Err(ProbeError::NoCredentials(msg)) => {
            // Provider not configured — omit silently (no last-known-good to
            // serve, and no stale badge to show).
            eprintln!("kodebar: antigravity skipped: {msg}");
        }
        Err(ProbeError::RateLimited) | Err(_) => {
            // Back off and serve last-known-good, flagged stale (PRD §5.1,
            // §7.1, §7.4). With no prior data, emit an empty stale entry so
            // the UI can still show the provider as "unavailable / stale".
            let entry = match prior {
                Some(p) => ProviderEntry::stale_from_prior(p),
                None => ProviderEntry::fresh(
                    ProviderPayload::Antigravity(AntigravityPayload::empty()),
                    // No successful probe has ever happened.
                    now_iso(),
                )
                .stale_with_no_prior(),
            };
            providers.insert("antigravity".to_string(), entry);
        }
    }
}

impl ProviderEntry {
    /// Mark a fresh-built empty entry as stale with no prior successful
    /// Probe (so `last_updated` is `None`).
    fn stale_with_no_prior(self) -> Self {
        Self {
            payload: self.payload,
            stale: true,
            last_updated: None,
        }
    }
}

/// Build the current Snapshot by probing every configured provider. Probes
/// are independent and isolated — one failing must not block others (PRD
/// §7.3). The prior Snapshot (read from the cache file) supplies
/// last-known-good data for the Stale path.
fn build_snapshot<C: CodeAssistClient>(client: &C) -> Snapshot {
    let prior = load_prior_snapshot().providers;
    let mut providers = BTreeMap::new();

    probe_antigravity(client, prior.get("antigravity").cloned(), &mut providers);

    let last_updated = if providers.is_empty() {
        None
    } else {
        Some(now_iso())
    };
    Snapshot {
        _meta: SnapshotMeta {
            version: 1,
            last_updated,
        },
        providers,
    }
}

/// Read the prior Snapshot from `<cache_dir>/last.json`, best-effort. On any
/// read/parse failure an empty Snapshot is returned — the next poll will
/// simply have no last-known-good to serve.
fn load_prior_snapshot() -> Snapshot {
    let path = match default_cache_dir() {
        Ok(dir) => dir.join("last.json"),
        Err(_) => return empty_snapshot(),
    };
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| empty_snapshot()),
        Err(_) => empty_snapshot(),
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let snapshot = build_snapshot(
        &antigravity::ReqwestClient::new()
            .map_err(|e| format!("failed to init HTTP client: {e:?}"))?,
    );
    match cli.command {
        Some(Command::Status { json }) => {
            if json {
                let encoded = serde_json::to_string(&snapshot)
                    .map_err(|e| format!("failed to encode snapshot: {e}"))?;
                println!("{encoded}");
            } else {
                let dir = default_cache_dir()?;
                write_snapshot_atomic(&dir, &snapshot)?;
                println!("{}", render_human(&snapshot));
            }
        }
        Some(Command::Poll) => {
            let dir = default_cache_dir()?;
            write_snapshot_atomic(&dir, &snapshot)?;
        }
        // Bare `kodebar` defaults to the status invocation: write the cache
        // then print the human-readable summary.
        None => {
            let dir = default_cache_dir()?;
            write_snapshot_atomic(&dir, &snapshot)?;
            println!("{}", render_human(&snapshot));
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
    use std::fs;
    use tempfile::TempDir;

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

    #[test]
    fn provider_entry_has_stale_and_last_updated_fields() {
        let entry = ProviderEntry {
            payload: ProviderPayload::Antigravity(AntigravityPayload::empty()),
            stale: true,
            last_updated: Some("2026-07-02T11:17:00Z".to_string()),
        };
        let v: serde_json::Value = serde_json::to_value(&entry).unwrap();
        // Common Kodebar extensions sit at the top level alongside the
        // provider payload fields (PRD §5.5).
        assert_eq!(v["stale"], true);
        assert_eq!(v["lastUpdated"], "2026-07-02T11:17:00Z");
        // The provider payload is flattened in, not nested.
        assert_eq!(v["type"], "quota-based");
        assert_eq!(v["usagePercentage"], 0);
    }

    #[test]
    fn write_snapshot_round_trips_through_cache_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("kodebar");
        let snapshot = empty_snapshot();

        write_snapshot_atomic(&dir, &snapshot).unwrap();

        let path = dir.join("last.json");
        let contents = fs::read_to_string(&path).unwrap();
        let read_back: Snapshot = serde_json::from_str(&contents).unwrap();
        assert_eq!(snapshot, read_back);
        assert_eq!(read_back._meta.version, 1);
        assert!(read_back._meta.last_updated.is_none());
        assert!(read_back.providers.is_empty());
    }

    #[test]
    fn write_snapshot_creates_cache_dir_with_0700_perms() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("nested").join("kodebar");
        write_snapshot_atomic(&dir, &empty_snapshot()).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&dir).unwrap().permissions().mode();
            assert_eq!(
                mode & 0o777,
                0o700,
                "cache dir should be 0700, got {:o}",
                mode
            );
        }

        assert!(dir.join("last.json").exists());
    }

    #[test]
    fn write_snapshot_leaves_no_tempfile_behind() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("kodebar");
        write_snapshot_atomic(&dir, &empty_snapshot()).unwrap();

        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(leftovers, vec!["last.json".to_string()]);
    }

    #[test]
    fn poll_command_writes_cache_silently() {
        // `poll` only writes the cache file; it produces no stdout. We exercise
        // the writer path directly and assert the file exists with the right
        // shape, mirroring what the `poll` subcommand does.
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("kodebar");
        let snapshot = empty_snapshot();
        write_snapshot_atomic(&dir, &snapshot).unwrap();

        let path = dir.join("last.json");
        assert!(path.exists());
        let read_back: Snapshot =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(snapshot, read_back);
    }
}
