mod login;
mod probe;
mod snapshot_signal;

use clap::{Parser, Subcommand, ValueEnum};
use login::{LoginOutcome, SystemBrowser, TerminalSecretReader};
use probe::antigravity::{self, AntigravityPayload};
use probe::chatgpt::{self, ChatGptClient, ChatGptPayload};
use probe::opencode_dashboard::DashboardClient;
use probe::opencode_go::{self, OpenCodeGoClient, OpenCodeGoPayload};
use probe::opencode_zen::{self, OpenCodeZenPayload};
use probe::{CodeAssistClient, ProbeError};
use serde::{Deserialize, Serialize};
use snapshot_signal::{SessionBusSnapshotNotifier, SnapshotNotifier};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

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
    /// Configure a provider through a guided browser flow.
    Login {
        /// Provider to configure.
        #[arg(value_enum)]
        provider: LoginProvider,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum LoginProvider {
    Opencode,
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
    ChatGpt(ChatGptPayload),
    OpenCodeGo(OpenCodeGoPayload),
    OpenCodeZen(OpenCodeZenPayload),
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
    /// Actionable reason the latest Probe failed, when one is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// Number of consecutive failed Probes, used to calculate backoff.
    #[serde(default, skip_serializing_if = "is_zero")]
    consecutive_failures: u32,
    /// Earliest time another Probe should be attempted after repeated failures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retry_after: Option<String>,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

impl ProviderEntry {
    /// Build a fresh (non-stale) entry from a successful Probe.
    fn fresh(payload: ProviderPayload, now: String) -> Self {
        Self {
            payload,
            stale: false,
            last_updated: Some(now),
            error: None,
            consecutive_failures: 0,
            retry_after: None,
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
            error: prior.error,
            consecutive_failures: prior.consecutive_failures,
            retry_after: prior.retry_after,
        }
    }
}

/// The merged JSON result of all provider probes, written to
/// `~/.cache/kodebar/last.json`. The single boundary between Backend and
/// Frontend. See PRD §5.5 and backend/CONTEXT.md "Snapshot".
#[derive(Serialize, Debug, Clone, PartialEq)]
struct Snapshot {
    _meta: SnapshotMeta,
    #[serde(flatten)]
    providers: BTreeMap<String, ProviderEntry>,
}

#[derive(Deserialize)]
struct SnapshotWire {
    _meta: SnapshotMeta,
    /// Compatibility with snapshots written before the documented flat
    /// provider map was implemented.
    #[serde(default)]
    providers: BTreeMap<String, ProviderEntry>,
    #[serde(flatten)]
    flat_providers: BTreeMap<String, ProviderEntry>,
}

impl<'de> Deserialize<'de> for Snapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut wire = SnapshotWire::deserialize(deserializer)?;
        wire.providers.append(&mut wire.flat_providers);
        Ok(Self {
            _meta: wire._meta,
            providers: wire.providers,
        })
    }
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
    cache_dir_from_env(
        std::env::var("XDG_CACHE_HOME").ok(),
        std::env::var("HOME").ok(),
    )
}

fn cache_dir_from_env(
    xdg_cache_home: Option<String>,
    home: Option<String>,
) -> Result<PathBuf, String> {
    if let Some(path) = xdg_cache_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Ok(path.join("kodebar"));
    }
    if let Some(path) = home.map(PathBuf::from).filter(|path| path.is_absolute()) {
        return Ok(path.join(".cache/kodebar"));
    }
    Err("cannot locate the Snapshot without an absolute XDG_CACHE_HOME or HOME".into())
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

fn persist_snapshot<N: SnapshotNotifier>(
    dir: &Path,
    snapshot: &Snapshot,
    notifier: &N,
) -> Result<(), String> {
    write_snapshot_atomic(dir, snapshot)?;
    if let Err(error) = notifier.snapshot_updated() {
        eprintln!("kodebar: Snapshot persisted, but refresh notification failed: {error}");
    }
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

impl ProviderEntry {
    /// Mark a fresh-built empty entry as stale with no prior successful
    /// Probe (so `last_updated` is `None`).
    fn stale_with_no_prior(self) -> Self {
        Self {
            payload: self.payload,
            stale: true,
            last_updated: None,
            error: self.error,
            consecutive_failures: self.consecutive_failures,
            retry_after: self.retry_after,
        }
    }

    fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    fn after_failure(mut self, now: chrono::DateTime<chrono::Utc>) -> Self {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        let exponent = self.consecutive_failures.saturating_sub(1).min(4);
        let delay_seconds = (300_i64 * (1_i64 << exponent)).min(3_600);
        self.retry_after = Some(
            (now + chrono::TimeDelta::seconds(delay_seconds))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        );
        self
    }

    fn is_backing_off(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        self.retry_after
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .is_some_and(|retry_after| retry_after > now)
    }
}

fn probe_error_message(error: &ProbeError) -> String {
    error.user_message()
}

fn merge_probe_result_at(
    provider_id: &str,
    result: Result<ProviderPayload, ProbeError>,
    prior: Option<ProviderEntry>,
    empty_payload: ProviderPayload,
    providers: &mut BTreeMap<String, ProviderEntry>,
    now: chrono::DateTime<chrono::Utc>,
) {
    let now_iso = || now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    match result {
        Ok(payload) => {
            providers.insert(provider_id.into(), ProviderEntry::fresh(payload, now_iso()));
        }
        Err(ProbeError::NoCredentials(message)) if prior.is_none() => {
            eprintln!("kodebar: {provider_id} skipped: {message}");
        }
        Err(error) => {
            let message = probe_error_message(&error);
            let entry = match prior {
                Some(prior) => ProviderEntry::stale_from_prior(prior),
                None => ProviderEntry::fresh(empty_payload, now_iso()).stale_with_no_prior(),
            }
            .with_error(message)
            .after_failure(now);
            providers.insert(provider_id.into(), entry);
        }
    }
}

type ProbeRun = Box<dyn FnOnce() -> Result<ProviderPayload, ProbeError> + Send + 'static>;

struct ProbeTask {
    provider_id: &'static str,
    prior: Option<ProviderEntry>,
    empty_payload: ProviderPayload,
    run: ProbeRun,
}

impl ProbeTask {
    fn new<F>(
        provider_id: &'static str,
        prior: Option<ProviderEntry>,
        empty_payload: ProviderPayload,
        run: F,
    ) -> Self
    where
        F: FnOnce() -> Result<ProviderPayload, ProbeError> + Send + 'static,
    {
        Self {
            provider_id,
            prior,
            empty_payload,
            run: Box::new(run),
        }
    }
}

fn run_probe_tasks(tasks: Vec<ProbeTask>, timeout: Duration) -> BTreeMap<String, ProviderEntry> {
    run_probe_tasks_at(tasks, timeout, chrono::Utc::now())
}

fn run_probe_tasks_at(
    tasks: Vec<ProbeTask>,
    timeout: Duration,
    now: chrono::DateTime<chrono::Utc>,
) -> BTreeMap<String, ProviderEntry> {
    let deadline = Instant::now() + timeout;
    let (sender, receiver) = mpsc::channel();
    let mut pending = BTreeMap::new();
    let mut providers = BTreeMap::new();

    for task in tasks {
        if task
            .prior
            .as_ref()
            .is_some_and(|prior| prior.is_backing_off(now))
        {
            providers.insert(task.provider_id.into(), task.prior.unwrap());
            continue;
        }
        let provider_id = task.provider_id;
        pending.insert(provider_id, (task.prior, task.empty_payload));
        let sender = sender.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task.run))
                .unwrap_or_else(|_| Err(ProbeError::Io("Probe panicked".into())));
            let _ = sender.send((provider_id, result));
        });
    }
    drop(sender);

    while !pending.is_empty() {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        let Ok((provider_id, result)) = receiver.recv_timeout(remaining) else {
            break;
        };
        if let Some((prior, empty_payload)) = pending.remove(provider_id) {
            merge_probe_result_at(
                provider_id,
                result,
                prior,
                empty_payload,
                &mut providers,
                now,
            );
        }
    }

    for (provider_id, (prior, empty_payload)) in pending {
        merge_probe_result_at(
            provider_id,
            Err(ProbeError::Io(format!("Probe timed out after {timeout:?}"))),
            prior,
            empty_payload,
            &mut providers,
            now,
        );
    }
    providers
}

/// Build the current Snapshot by probing every configured provider. Probes
/// are independent and isolated — one failing must not block others (PRD
/// §7.3). The prior Snapshot (read from the cache file) supplies
/// last-known-good data for the Stale path.
fn build_snapshot<C, D, G, H>(
    code_assist_client: Arc<C>,
    dashboard_client: Arc<D>,
    go_client: Arc<G>,
    chatgpt_client: Arc<H>,
    dashboard_credentials_path: &Path,
    opencode_auth_path: Result<PathBuf, ProbeError>,
    chatgpt_auth_path: Result<PathBuf, ProbeError>,
) -> Snapshot
where
    C: CodeAssistClient + Send + Sync + 'static,
    D: DashboardClient + Send + Sync + 'static,
    G: OpenCodeGoClient + Send + Sync + 'static,
    H: ChatGptClient + Send + Sync + 'static,
{
    let mut prior = load_prior_snapshot().providers;
    let antigravity_client = Arc::clone(&code_assist_client);
    let zen_credentials_path = dashboard_credentials_path.to_owned();
    let tasks = vec![
        ProbeTask::new(
            "antigravity",
            prior.remove("antigravity"),
            ProviderPayload::Antigravity(AntigravityPayload::empty()),
            move || {
                antigravity::run(
                    antigravity_client.as_ref(),
                    &antigravity::gemini_dir(),
                    true,
                )
                .map(ProviderPayload::Antigravity)
            },
        ),
        ProbeTask::new(
            "chatgpt",
            prior.remove("chatgpt"),
            ProviderPayload::ChatGpt(ChatGptPayload::empty()),
            move || match chatgpt_auth_path {
                Ok(path) => {
                    chatgpt::run(chatgpt_client.as_ref(), &path).map(ProviderPayload::ChatGpt)
                }
                Err(error) => Err(error),
            },
        ),
        ProbeTask::new(
            "opencode_go",
            prior.remove("opencode_go"),
            ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()),
            move || match opencode_auth_path {
                Ok(path) => {
                    opencode_go::run(go_client.as_ref(), &path).map(ProviderPayload::OpenCodeGo)
                }
                Err(error) => Err(error),
            },
        ),
        ProbeTask::new(
            "opencode_zen",
            prior.remove("opencode_zen"),
            ProviderPayload::OpenCodeZen(OpenCodeZenPayload::empty()),
            move || {
                opencode_zen::run(dashboard_client.as_ref(), &zen_credentials_path)
                    .map(ProviderPayload::OpenCodeZen)
            },
        ),
    ];
    let providers = run_probe_tasks(tasks, PROBE_TIMEOUT);

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
    if let Some(Command::Login {
        provider: LoginProvider::Opencode,
    }) = &cli.command
    {
        println!("Opening {}", login::OPENCODE_KEY_URL);
        println!("Sign in, create or copy an API key, then return here.");
        let client = probe::opencode_go::ReqwestOpenCodeGoClient::new()
            .map_err(|e| format!("failed to init OpenCode Go HTTP client: {e:?}"))?;
        let auth_path =
            probe::opencode_auth::default_auth_path().map_err(|error| error.user_message())?;
        let outcome = login::guided_opencode_login(
            &SystemBrowser,
            &TerminalSecretReader,
            &client,
            &auth_path,
        )?;
        match outcome {
            LoginOutcome::ActiveGoSubscription => {
                println!("OpenCode login saved and Go usage verified.");
            }
            LoginOutcome::NoGoSubscription => {
                println!("OpenCode login saved; this workspace has no Go subscription.");
            }
        }
        return Ok(());
    }

    let code_assist_client = antigravity::ReqwestClient::new()
        .map_err(|e| format!("failed to init Code Assist HTTP client: {e:?}"))?;
    let dashboard_client = probe::opencode_dashboard::ReqwestDashboardClient::new()
        .map_err(|e| format!("failed to init OpenCode dashboard HTTP client: {e:?}"))?;
    let go_client = probe::opencode_go::ReqwestOpenCodeGoClient::new()
        .map_err(|e| format!("failed to init OpenCode Go HTTP client: {e:?}"))?;
    let chatgpt_client = probe::chatgpt::ReqwestChatGptClient::new()
        .map_err(|e| format!("failed to init ChatGPT HTTP client: {e:?}"))?;
    let snapshot = build_snapshot(
        Arc::new(code_assist_client),
        Arc::new(dashboard_client),
        Arc::new(go_client),
        Arc::new(chatgpt_client),
        &probe::opencode_dashboard::default_credentials_path(),
        probe::opencode_auth::default_auth_path(),
        probe::chatgpt::default_auth_path(),
    );
    match cli.command {
        Some(Command::Status { json }) => {
            let dir = default_cache_dir()?;
            persist_snapshot(&dir, &snapshot, &SessionBusSnapshotNotifier)?;
            if json {
                let encoded = serde_json::to_string(&snapshot)
                    .map_err(|e| format!("failed to encode snapshot: {e}"))?;
                println!("{encoded}");
            } else {
                println!("{}", render_human(&snapshot));
            }
        }
        Some(Command::Poll) => {
            let dir = default_cache_dir()?;
            persist_snapshot(&dir, &snapshot, &SessionBusSnapshotNotifier)?;
        }
        Some(Command::Login { .. }) => unreachable!("login returned before probing"),
        // Bare `kodebar` defaults to the status invocation: write the cache
        // then print the human-readable summary.
        None => {
            let dir = default_cache_dir()?;
            persist_snapshot(&dir, &snapshot, &SessionBusSnapshotNotifier)?;
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
    use probe::opencode_dashboard::{DashboardClient, DashboardCredentials, DashboardResponse};
    use probe::opencode_go::{GoApiResponse, OpenCodeGoClient};
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::time::Duration;
    use tempfile::TempDir;

    struct MockDashboardClient {
        response: DashboardResponse,
    }

    impl DashboardClient for MockDashboardClient {
        fn get(
            &self,
            _credentials: &DashboardCredentials,
            _page_path: &str,
        ) -> Result<DashboardResponse, ProbeError> {
            Ok(self.response.clone())
        }
    }

    struct MockGoClient {
        response: GoApiResponse,
    }

    impl OpenCodeGoClient for MockGoClient {
        fn get_usage(&self, _api_key: &str) -> Result<GoApiResponse, ProbeError> {
            Ok(self.response.clone())
        }
    }

    fn write_dashboard_credentials(dir: &Path) -> PathBuf {
        let path = dir.join("opencode-go.json");
        fs::write(&path, r#"{"workspaceId":"wrk_test","authCookie":"cookie"}"#).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        path
    }

    #[test]
    fn status_json_is_valid_snapshot_with_version_one() {
        let snapshot = empty_snapshot();
        let encoded = serde_json::to_string(&snapshot).unwrap();
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value["_meta"]["version"], 1);
        assert!(value.get("providers").is_none());
    }

    #[test]
    fn status_json_matches_expected_shape() {
        let snapshot = empty_snapshot();
        let encoded = serde_json::to_string(&snapshot).unwrap();
        let expected = r#"{"_meta":{"version":1,"lastUpdated":null}}"#;
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
            error: None,
            consecutive_failures: 0,
            retry_after: None,
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
    fn opencode_zen_entry_serializes_as_a_stale_provider_with_an_actionable_error() {
        let entry = ProviderEntry {
            payload: ProviderPayload::OpenCodeZen(probe::opencode_zen::OpenCodeZenPayload::empty()),
            stale: true,
            last_updated: None,
            error: Some("session expired — re-login at opencode.ai".into()),
            consecutive_failures: 1,
            retry_after: Some("2026-08-22T01:05:00Z".into()),
        };

        let value = serde_json::to_value(entry).unwrap();

        assert_eq!(value["type"], "pay-as-you-go");
        assert_eq!(value["balanceFormatted"], "$0.00");
        assert_eq!(value["stale"], true);
        assert_eq!(value["error"], "session expired — re-login at opencode.ai");
    }

    #[test]
    fn chatgpt_entry_serializes_plan_identity_and_common_state() {
        let entry = ProviderEntry::fresh(
            ProviderPayload::ChatGpt(ChatGptPayload::empty()),
            "2026-08-23T00:00:00Z".into(),
        );

        let value = serde_json::to_value(entry).unwrap();

        assert_eq!(value["type"], "quota-based");
        assert_eq!(value["planType"], "unknown");
        assert!(value["limits"]["codex"].is_object());
        assert_eq!(value["stale"], false);
    }

    #[test]
    fn successful_zen_probe_is_merged_into_the_snapshot_providers() {
        let tmp = TempDir::new().unwrap();
        let credentials_path = write_dashboard_credentials(tmp.path());
        let client = MockDashboardClient {
            response: DashboardResponse {
                status: 200,
                body: r#"window._$HY={balance:-1392399000,reloadAmount:20,reloadTrigger:5,useBalance:true}"#
                    .into(),
                location: None,
            },
        };
        let mut providers = BTreeMap::new();

        merge_probe_result_at(
            "opencode_zen",
            opencode_zen::run(&client, &credentials_path).map(ProviderPayload::OpenCodeZen),
            None,
            ProviderPayload::OpenCodeZen(OpenCodeZenPayload::empty()),
            &mut providers,
            chrono::Utc::now(),
        );

        let value = serde_json::to_value(&providers["opencode_zen"]).unwrap();
        assert_eq!(value["balanceFormatted"], "$13.92");
        assert_eq!(value["stale"], false);
        assert!(value.get("error").is_none());
    }

    #[test]
    fn successful_go_probe_is_merged_into_the_snapshot_providers() {
        let tmp = TempDir::new().unwrap();
        let credentials_path = tmp.path().join("auth.json");
        fs::write(
            &credentials_path,
            r#"{"opencode":{"type":"api","key":"oc-secret"}}"#,
        )
        .unwrap();
        let client = MockGoClient {
            response: GoApiResponse {
                status: 200,
                body: r#"{"usage":{"rolling":{"status":"ok","percent":14,"resetsAt":"2099-08-22T00:01:00Z"},"weekly":{"status":"ok","percent":9,"resetsAt":"2099-08-22T00:02:00Z"},"monthly":{"status":"ok","percent":4,"resetsAt":"2099-08-22T00:03:00Z"}}}"#.into(),
            },
        };
        let mut providers = BTreeMap::new();

        merge_probe_result_at(
            "opencode_go",
            opencode_go::run(&client, &credentials_path).map(ProviderPayload::OpenCodeGo),
            None,
            ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()),
            &mut providers,
            chrono::Utc::now(),
        );

        let value = serde_json::to_value(&providers["opencode_go"]).unwrap();
        assert_eq!(value["windows"]["rolling"]["usagePercent"], 14);
        assert_eq!(value["stale"], false);
    }

    #[test]
    fn missing_dashboard_credentials_preserve_prior_zen_data_as_stale() {
        let client = MockDashboardClient {
            response: DashboardResponse {
                status: 200,
                body: String::new(),
                location: None,
            },
        };
        let prior = ProviderEntry::fresh(
            ProviderPayload::OpenCodeZen(OpenCodeZenPayload::empty()),
            "2026-08-22T01:02:03Z".into(),
        );
        let mut providers = BTreeMap::new();

        merge_probe_result_at(
            "opencode_zen",
            opencode_zen::run(&client, Path::new("/definitely/missing/opencode-go.json"))
                .map(ProviderPayload::OpenCodeZen),
            Some(prior),
            ProviderPayload::OpenCodeZen(OpenCodeZenPayload::empty()),
            &mut providers,
            chrono::Utc::now(),
        );

        let value = serde_json::to_value(&providers["opencode_zen"]).unwrap();
        assert_eq!(value["stale"], true);
        assert_eq!(value["lastUpdated"], "2026-08-22T01:02:03Z");
        assert!(value["error"].as_str().unwrap().contains("does not exist"));
    }

    #[test]
    fn probe_tasks_start_concurrently_and_isolate_failure_and_timeout() {
        let barrier = Arc::new(Barrier::new(3));
        let tasks = ["one", "two", "three"].map(|provider_id| {
            let barrier = Arc::clone(&barrier);
            ProbeTask::new(
                provider_id,
                None,
                ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()),
                move || {
                    barrier.wait();
                    match provider_id {
                        "two" => Err(ProbeError::Io("provider unavailable".into())),
                        "three" => {
                            std::thread::sleep(Duration::from_millis(200));
                            Ok(ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()))
                        }
                        _ => Ok(ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty())),
                    }
                },
            )
        });

        let providers = run_probe_tasks(tasks.into(), Duration::from_millis(100));

        assert!(!providers["one"].stale);
        assert!(providers["two"].stale);
        assert_eq!(
            providers["two"].error.as_deref(),
            Some("provider unavailable")
        );
        assert!(providers["three"].stale);
        assert!(
            providers["three"]
                .error
                .as_deref()
                .unwrap()
                .contains("timed out")
        );
    }

    #[test]
    fn timed_out_probe_preserves_last_successful_data_without_delaying_snapshot() {
        let prior = ProviderEntry::fresh(
            ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()),
            "2026-08-22T01:02:03Z".into(),
        );
        let task = ProbeTask::new(
            "opencode_go",
            Some(prior),
            ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()),
            || {
                std::thread::sleep(Duration::from_secs(1));
                Ok(ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()))
            },
        );
        let started = Instant::now();

        let providers = run_probe_tasks(vec![task], Duration::from_millis(100));

        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(providers["opencode_go"].stale);
        assert_eq!(
            providers["opencode_go"].last_updated.as_deref(),
            Some("2026-08-22T01:02:03Z")
        );
        assert!(
            providers["opencode_go"]
                .error
                .as_deref()
                .unwrap()
                .contains("timed out")
        );
    }

    #[test]
    fn repeated_failure_backoff_skips_probe_until_retry_time() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-08-22T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let failed = ProbeTask::new(
            "opencode_go",
            None,
            ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()),
            || Err(ProbeError::Io("network down".into())),
        );
        let first = run_probe_tasks_at(vec![failed], Duration::from_secs(1), now);
        let prior: ProviderEntry =
            serde_json::from_value(serde_json::to_value(&first["opencode_go"]).unwrap()).unwrap();
        assert_eq!(prior.consecutive_failures, 1);
        assert_eq!(prior.retry_after.as_deref(), Some("2026-08-22T00:05:00Z"));

        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_in_probe = Arc::clone(&attempts);
        let retry = ProbeTask::new(
            "opencode_go",
            Some(prior),
            ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()),
            move || {
                attempts_in_probe.fetch_add(1, Ordering::SeqCst);
                Ok(ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()))
            },
        );

        let providers = run_probe_tasks_at(
            vec![retry],
            Duration::from_secs(1),
            now + chrono::TimeDelta::minutes(1),
        );

        assert_eq!(attempts.load(Ordering::SeqCst), 0);
        assert!(providers["opencode_go"].stale);
        assert_eq!(providers["opencode_go"].consecutive_failures, 1);

        let fails_again = ProbeTask::new(
            "opencode_go",
            Some(providers["opencode_go"].clone()),
            ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()),
            || Err(ProbeError::Io("still down".into())),
        );
        let second = run_probe_tasks_at(
            vec![fails_again],
            Duration::from_secs(1),
            now + chrono::TimeDelta::minutes(5),
        );

        assert_eq!(second["opencode_go"].consecutive_failures, 2);
        assert_eq!(
            second["opencode_go"].retry_after.as_deref(),
            Some("2026-08-22T00:15:00Z")
        );
    }

    #[test]
    fn repeated_failure_backoff_is_capped_at_one_hour() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-08-22T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let prior = ProviderEntry {
            consecutive_failures: 4,
            ..ProviderEntry::fresh(
                ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()),
                "2026-08-21T00:00:00Z".into(),
            )
        };
        let failure = ProbeTask::new(
            "opencode_go",
            Some(prior),
            ProviderPayload::OpenCodeGo(OpenCodeGoPayload::empty()),
            || Err(ProbeError::Io("still down".into())),
        );

        let result = run_probe_tasks_at(vec![failure], Duration::from_secs(1), now);

        assert_eq!(result["opencode_go"].consecutive_failures, 5);
        assert_eq!(
            result["opencode_go"].retry_after.as_deref(),
            Some("2026-08-22T01:00:00Z")
        );
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

    #[derive(Default)]
    struct RecordingSnapshotNotifier {
        notifications: AtomicUsize,
    }

    impl SnapshotNotifier for RecordingSnapshotNotifier {
        fn snapshot_updated(&self) -> Result<(), String> {
            self.notifications.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn successful_snapshot_persistence_emits_updated_notification() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("kodebar");
        let notifier = RecordingSnapshotNotifier::default();

        persist_snapshot(&dir, &empty_snapshot(), &notifier).unwrap();

        assert!(dir.join("last.json").exists());
        assert_eq!(notifier.notifications.load(Ordering::SeqCst), 1);
    }

    struct FailingSnapshotNotifier;

    impl SnapshotNotifier for FailingSnapshotNotifier {
        fn snapshot_updated(&self) -> Result<(), String> {
            Err("session bus unavailable".into())
        }
    }

    #[test]
    fn snapshot_persistence_succeeds_when_notification_fails() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("kodebar");

        persist_snapshot(&dir, &empty_snapshot(), &FailingSnapshotNotifier).unwrap();

        assert!(dir.join("last.json").exists());
    }

    #[test]
    fn failed_snapshot_persistence_does_not_emit_notification() {
        let tmp = TempDir::new().unwrap();
        let unusable_dir = tmp.path().join("not-a-directory");
        fs::write(&unusable_dir, "occupied").unwrap();
        let notifier = RecordingSnapshotNotifier::default();

        assert!(persist_snapshot(&unusable_dir, &empty_snapshot(), &notifier).is_err());

        assert_eq!(notifier.notifications.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn cache_dir_ignores_relative_xdg_path_and_uses_absolute_home() {
        assert_eq!(
            cache_dir_from_env(Some("relative/cache".into()), Some("/home/tester".into())).unwrap(),
            PathBuf::from("/home/tester/.cache/kodebar")
        );
    }

    #[test]
    fn cache_dir_rejects_relative_xdg_path_and_relative_home() {
        let error = cache_dir_from_env(Some("relative/cache".into()), Some("relative/home".into()))
            .unwrap_err();

        assert!(error.contains("absolute"));
    }

    #[test]
    fn reads_legacy_nested_provider_snapshots() {
        let entry = ProviderEntry::fresh(
            ProviderPayload::OpenCodeZen(OpenCodeZenPayload::empty()),
            "2026-08-22T01:02:03Z".into(),
        );
        let legacy = serde_json::json!({
            "_meta": {"version": 1, "lastUpdated": "2026-08-22T01:02:03Z"},
            "providers": {"opencode_zen": entry}
        });

        let snapshot: Snapshot = serde_json::from_value(legacy).unwrap();

        assert!(snapshot.providers.contains_key("opencode_zen"));
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
