//! Antigravity / Gemini probe.
//!
//! Reads `~/.gemini/oauth_creds.json`, refreshes the OAuth access token
//! transparently when it is expired, and calls the Google Cloud Code Assist
//! API (`loadCodeAssist` + `retrieveUserQuota`) to fetch per-model quota
//! buckets. See PRD §5.1 and issue #3.
//!
//! The probe is split into a pure, injectable core (this module's `run`
//! function + the [`CodeAssistClient`] trait) and a thin reqwest-backed
//! production client ([`ReqwestClient`]). Unit tests drive the core with a
//! mock client so no network is required.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{HttpResponse, ProbeError};

/// The public Gemini CLI OAuth client. This is an installed-app OAuth client
/// whose "secret" ships in the downloadable Gemini CLI binary (see
/// `packages/core/src/code_assist/oauth2.ts`); embedding it here is the same
/// approach used by `gusage` and `gemini-cli-usage`. Env vars
/// `GEMINI_OAUTH_CLIENT_ID` / `GEMINI_OAUTH_CLIENT_SECRET` override these
/// hardcoded values if set (PRD §5.1 step 3).
const DEFAULT_OAUTH_CLIENT_ID: &str =
    "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
const DEFAULT_OAUTH_CLIENT_SECRET: &str = "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";

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const CODE_ASSIST_BASE: &str = "https://cloudcode-pa.googleapis.com";

/// A small skew so we refresh a token slightly before it actually expires,
/// avoiding a failed probe on a token that expires mid-request.
const EXPIRY_SKEW_MS: i64 = 60_000;

/// The Code Assist API surface the probe depends on.
///
/// Production uses [`ReqwestClient`]; tests implement this with canned
/// responses (including 429 simulation) so the probe logic is exercised with
/// zero network access.
pub trait CodeAssistClient {
    /// Exchange a refresh token for a fresh access token.
    fn refresh_token(
        &self,
        refresh_token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<HttpResponse, ProbeError>;

    /// `POST …/v1internal:loadCodeAssist`.
    fn load_code_assist(
        &self,
        access_token: &str,
        request: &Value,
    ) -> Result<HttpResponse, ProbeError>;

    /// `POST …/v1internal:retrieveUserQuota`.
    fn retrieve_user_quota(
        &self,
        access_token: &str,
        request: &Value,
    ) -> Result<HttpResponse, ProbeError>;
}

/// reqwest-backed production client. Construct with [`ReqwestClient::new`].
pub struct ReqwestClient {
    http: reqwest::blocking::Client,
}

impl ReqwestClient {
    /// Build a client with a 15-second per-request timeout (PRD §7.2).
    pub fn new() -> Result<Self, ProbeError> {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| ProbeError::Io(format!("failed to build HTTP client: {e}")))?;
        Ok(Self { http })
    }

    fn post_bearer_json(
        &self,
        url: &str,
        access_token: &str,
        body: &Value,
    ) -> Result<HttpResponse, ProbeError> {
        let resp = self
            .http
            .post(url)
            .bearer_auth(access_token)
            .json(body)
            .send()
            .map_err(|e| ProbeError::Io(format!("HTTP request failed: {e}")))?;
        let status = resp.status().as_u16();
        let text = resp
            .text()
            .map_err(|e| ProbeError::Io(format!("failed to read response body: {e}")))?;
        let body: Value = serde_json::from_str(&text).unwrap_or(Value::String(text));
        Ok(HttpResponse { status, body })
    }
}

impl CodeAssistClient for ReqwestClient {
    fn refresh_token(
        &self,
        refresh_token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<HttpResponse, ProbeError> {
        let resp = self
            .http
            .post(TOKEN_URL)
            .form(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .map_err(|e| ProbeError::Io(format!("token refresh request failed: {e}")))?;
        let status = resp.status().as_u16();
        let text = resp
            .text()
            .map_err(|e| ProbeError::Io(format!("failed to read token response: {e}")))?;
        let body: Value = serde_json::from_str(&text).unwrap_or(Value::String(text));
        Ok(HttpResponse { status, body })
    }

    fn load_code_assist(
        &self,
        access_token: &str,
        request: &Value,
    ) -> Result<HttpResponse, ProbeError> {
        self.post_bearer_json(
            &format!("{CODE_ASSIST_BASE}/v1internal:loadCodeAssist"),
            access_token,
            request,
        )
    }

    fn retrieve_user_quota(
        &self,
        access_token: &str,
        request: &Value,
    ) -> Result<HttpResponse, ProbeError> {
        self.post_bearer_json(
            &format!("{CODE_ASSIST_BASE}/v1internal:retrieveUserQuota"),
            access_token,
            request,
        )
    }
}

/// The provider-specific payload for the `antigravity` entry in the Snapshot
/// — everything *except* the common `stale` / `last_updated` fields, which
/// are carried by [`crate::ProviderEntry`] via `#[serde(flatten)]`. Together
/// they serialize to the shape documented in PRD §5.5.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityPayload {
    #[serde(rename = "type")]
    r#type: String,
    usage_percentage: u32,
    accounts: Vec<AntigravityAccount>,
}

impl AntigravityPayload {
    pub fn new(usage_percentage: u32, accounts: Vec<AntigravityAccount>) -> Self {
        Self {
            r#type: "quota-based".to_string(),
            usage_percentage,
            accounts,
        }
    }

    /// An empty payload used when the probe fails and there is no prior data
    /// to serve (PRD §7.1). The orchestration wraps it with `stale: true`.
    pub fn empty() -> Self {
        Self {
            r#type: "quota-based".to_string(),
            usage_percentage: 0,
            accounts: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityAccount {
    email: String,
    remaining_percentage: u32,
    model_breakdown: BTreeMap<String, ModelQuota>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelQuota {
    remaining_percentage: u32,
    reset_time: Option<String>,
}

/// OAuth credentials as stored by Gemini CLI / Antigravity at
/// `~/.gemini/oauth_creds.json`. The file may carry additional fields
/// (`id_token`, `scope`, …) which we preserve when writing back after a
/// refresh, hence the parent [`run`] function works on a raw [`Value`].
#[derive(Deserialize, Debug, Clone)]
struct CredsFile {
    access_token: Option<String>,
    refresh_token: Option<String>,
    /// Milliseconds since the Unix epoch (JS `Date.now()` convention), as
    /// written by Gemini CLI.
    expiry_date: Option<f64>,
}

/// `~/.gemini/google_accounts.json` — single account for v1 (PRD open
/// question 10).
#[derive(Deserialize, Debug, Clone)]
struct GoogleAccountsFile {
    active: Option<String>,
}

/// Load the credential file as a raw JSON value (preserving unknown fields
/// for write-back) plus the typed projection.
fn read_creds(path: &Path) -> Result<(Value, CredsFile), ProbeError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        ProbeError::NoCredentials(format!("failed to read {}: {e}", path.display()))
    })?;
    let value: Value =
        serde_json::from_str(&text).map_err(|e| ProbeError::NoCredentials(e.to_string()))?;
    let typed: CredsFile = serde_json::from_value(value.clone())
        .map_err(|e| ProbeError::NoCredentials(e.to_string()))?;
    Ok((value, typed))
}

fn read_account_email(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<GoogleAccountsFile>(&text)
        .ok()
        .and_then(|a| a.active)
}

/// Resolve the OAuth client ID/secret, honouring the env-var override.
fn oauth_client_id() -> String {
    std::env::var("GEMINI_OAUTH_CLIENT_ID").unwrap_or_else(|_| DEFAULT_OAUTH_CLIENT_ID.to_string())
}
fn oauth_client_secret() -> String {
    std::env::var("GEMINI_OAUTH_CLIENT_SECRET")
        .unwrap_or_else(|_| DEFAULT_OAUTH_CLIENT_SECRET.to_string())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn is_expired(expiry_date: Option<f64>) -> bool {
    match expiry_date {
        Some(ms) => now_ms() >= (ms as i64) - EXPIRY_SKEW_MS,
        None => true,
    }
}

/// A single quota bucket from `retrieveUserQuota`. `remaining_amount` is a
/// stringified int64 and is **omitted** by Google when the quota is at 100%
/// (PRD §5.1 gotcha, §7.6) — in that case `remaining_fraction` is `1`.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct BucketInfo {
    model_id: Option<String>,
    remaining_fraction: Option<f64>,
    /// Present except when quota is at 100% (PRD §7.6). Retained for
    /// completeness/future use; percentages derive from `remaining_fraction`.
    #[allow(dead_code)]
    remaining_amount: Option<String>,
    reset_time: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
struct RetrieveUserQuotaResponse {
    #[serde(default)]
    buckets: Vec<BucketInfo>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct LoadCodeAssistResponse {
    cloudaicompanion_project: Option<String>,
    #[allow(dead_code)]
    current_tier: Option<Value>,
    #[allow(dead_code)]
    paid_tier: Option<Value>,
}

/// Run the Antigravity probe against an injectable client.
///
/// `gemini_dir` is the directory containing `oauth_creds.json` and
/// `google_accounts.json` (typically `~/.gemini`). When `writeback` is true,
/// a refreshed token is written back to `oauth_creds.json` (preserving any
/// unknown fields) so Gemini CLI and Antigravity see the same live token
/// (PRD §3.1). Tests pass `writeback = true` to assert persistence.
pub fn run<C: CodeAssistClient>(
    client: &C,
    gemini_dir: &Path,
    writeback: bool,
) -> Result<AntigravityPayload, ProbeError> {
    let creds_file = gemini_dir.join("oauth_creds.json");
    let accounts_file = gemini_dir.join("google_accounts.json");

    let (mut creds_value, mut creds) = read_creds(&creds_file)?;
    let email = read_account_email(&accounts_file).unwrap_or_else(|| "unknown".to_string());

    // Refresh transparently if the access token is missing or expired
    // (PRD §7.7). Only surface an auth error if the refresh token itself is
    // invalid.
    if is_expired(creds.expiry_date) {
        let refresh_token = creds
            .refresh_token
            .clone()
            .ok_or_else(|| ProbeError::InvalidRefreshToken("no refresh_token on file".into()))?;
        let resp =
            client.refresh_token(&refresh_token, &oauth_client_id(), &oauth_client_secret())?;
        apply_refresh(&mut creds_value, &resp)?;
        creds = serde_json::from_value(creds_value.clone())
            .map_err(|e| ProbeError::Parse(format!("refreshed creds did not parse: {e}")))?;
        if writeback && let Err(e) = write_back_creds(&creds_file, &creds_value) {
            // Non-fatal: the fresh token is still usable this cycle.
            eprintln!("kodebar: failed to persist refreshed gemini token: {e}");
        }
    }

    let access_token = creds
        .access_token
        .ok_or_else(|| ProbeError::InvalidRefreshToken("no access_token after refresh".into()))?;

    // 1. loadCodeAssist → tier + cloudaicompanionProject.
    let lca_req = json!({
        "metadata": { "ideType": "GEMINI_CLI", "pluginType": "GEMINI" }
    });
    let project = match client.load_code_assist(&access_token, &lca_req) {
        Ok(resp) if resp.is_rate_limited() => return Err(ProbeError::RateLimited),
        Ok(resp) if resp.status >= 200 && resp.status < 300 => {
            serde_json::from_value::<LoadCodeAssistResponse>(resp.body.clone())
                .ok()
                .and_then(|r| r.cloudaicompanion_project)
                .unwrap_or_else(|| " ".to_string())
        }
        Ok(resp) if resp.status == 401 || resp.status == 403 => {
            return Err(ProbeError::InvalidRefreshToken(format!(
                "loadCodeAssist auth failed ({})",
                resp.status
            )));
        }
        Ok(resp) => {
            // Non-fatal: fall back to the default project and continue to the
            // quota call.
            eprintln!(
                "kodebar: loadCodeAssist returned {} — using default project",
                resp.status
            );
            " ".to_string()
        }
        Err(ProbeError::RateLimited) => return Err(ProbeError::RateLimited),
        Err(e) => return Err(e),
    };

    // 2. retrieveUserQuota → per-model buckets.
    let quota_req = json!({ "project": project });
    let resp = client.retrieve_user_quota(&access_token, &quota_req)?;
    if resp.is_rate_limited() {
        // Back off and serve last-known-good from cache (PRD §5.1, §7.4).
        return Err(ProbeError::RateLimited);
    }
    if resp.status == 401 || resp.status == 403 {
        return Err(ProbeError::InvalidRefreshToken(format!(
            "retrieveUserQuota auth failed ({})",
            resp.status
        )));
    }
    if resp.status != 200 {
        return Err(ProbeError::Http {
            status: resp.status,
            body: resp.body.to_string(),
        });
    }

    let parsed: RetrieveUserQuotaResponse = serde_json::from_value(resp.body.clone())
        .map_err(|e| ProbeError::Parse(format!("quota response: {e}")))?;

    let mut model_breakdown: BTreeMap<String, ModelQuota> = BTreeMap::new();
    let mut min_remaining_pct: Option<u32> = None;
    let mut max_usage_pct: u32 = 0;
    for bucket in &parsed.buckets {
        let model = bucket
            .model_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let remaining_pct = remaining_percentage(bucket);
        let usage_pct = 100u32.saturating_sub(remaining_pct);
        max_usage_pct = max_usage_pct.max(usage_pct);
        min_remaining_pct = Some(match min_remaining_pct {
            Some(m) => m.min(remaining_pct),
            None => remaining_pct,
        });
        model_breakdown.insert(
            model,
            ModelQuota {
                remaining_percentage: remaining_pct,
                reset_time: bucket.reset_time.clone(),
            },
        );
    }

    // If no buckets were returned, the API is degenerate for this account —
    // treat as a parse failure so the orchestration serves stale data.
    if model_breakdown.is_empty() {
        return Err(ProbeError::Parse("no quota buckets in response".into()));
    }

    let account_remaining = min_remaining_pct.unwrap_or(0);
    let accounts = vec![AntigravityAccount {
        email,
        remaining_percentage: account_remaining,
        model_breakdown,
    }];

    Ok(AntigravityPayload::new(max_usage_pct, accounts))
}

/// Compute the per-model remaining percentage, handling the `remainingAmount`
/// omission at 100% quota (PRD §7.6). The `remainingFraction` is the source of
/// truth for percentages; `remainingAmount` is an absolute count with no
/// total to divide against, so we derive `round(remainingFraction * 100)`.
fn remaining_percentage(bucket: &BucketInfo) -> u32 {
    let fraction = bucket.remaining_fraction.unwrap_or(1.0).clamp(0.0, 1.0);
    (fraction * 100.0).round() as u32
}

/// Resolve `~/.gemini` (honouring `GEMINI_DIR` if set for hermetic/test use
/// or custom installs). Production callers use this; tests pass an explicit
/// directory into [`run`].
pub fn gemini_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("GEMINI_DIR") {
        return std::path::PathBuf::from(d);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".gemini")
}

/// Overlay a refresh-token response onto the credential JSON and return the
/// new access token. Detects an invalid/revoked refresh token.
fn apply_refresh(creds: &mut Value, resp: &HttpResponse) -> Result<String, ProbeError> {
    if resp.status == 400 || resp.status == 401 {
        let msg = resp
            .body
            .get("error_description")
            .and_then(|v| v.as_str())
            .or_else(|| resp.body.get("error").and_then(|v| v.as_str()))
            .unwrap_or("refresh token rejected by Google")
            .to_string();
        return Err(ProbeError::InvalidRefreshToken(msg));
    }
    if resp.status != 200 {
        return Err(ProbeError::Http {
            status: resp.status,
            body: resp.body.to_string(),
        });
    }
    let obj = creds
        .as_object_mut()
        .ok_or_else(|| ProbeError::Parse("oauth_creds.json root is not an object".into()))?;
    let new_access = resp
        .body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProbeError::Parse("refresh response missing access_token".into()))?
        .to_string();
    let new_expiry = resp
        .body
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .map(|secs| (now_ms() + (secs as i64 * 1000)) as f64)
        .or_else(|| resp.body.get("expiry_date").and_then(|v| v.as_f64()));
    let new_refresh = resp
        .body
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            obj.get("refresh_token")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    obj.insert(
        "access_token".to_string(),
        Value::String(new_access.clone()),
    );
    if let Some(exp) = new_expiry {
        obj.insert(
            "expiry_date".to_string(),
            Value::Number(
                serde_json::Number::from_f64(exp).unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );
    }
    if let Some(rt) = new_refresh {
        obj.insert("refresh_token".to_string(), Value::String(rt));
    }
    Ok(new_access)
}

/// Write the (refreshed) credentials back to disk with 0600 permissions.
fn write_back_creds(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| Path::new(".").to_path_buf());
    if !parent.exists() {
        std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    }
    let encoded = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let tmp = parent.join(format!(".oauth_creds.json.tmp.{}", std::process::id()));
    std::fs::write(&tmp, &encoded).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// A mock client that returns canned responses. Each slot is a
    /// `Mutex<Option<…>>` so a single probe run can take its value across
    /// `&self` borrows.
    struct MockClient {
        refresh: Mutex<Option<Result<HttpResponse, ProbeError>>>,
        load_code_assist: Mutex<Option<Result<HttpResponse, ProbeError>>>,
        retrieve_user_quota: Mutex<Option<Result<HttpResponse, ProbeError>>>,
    }

    impl MockClient {
        fn new() -> Self {
            Self {
                refresh: Mutex::new(None),
                load_code_assist: Mutex::new(None),
                retrieve_user_quota: Mutex::new(None),
            }
        }
    }

    impl CodeAssistClient for MockClient {
        fn refresh_token(
            &self,
            _rt: &str,
            _id: &str,
            _secret: &str,
        ) -> Result<HttpResponse, ProbeError> {
            self.refresh
                .lock()
                .unwrap()
                .take()
                .expect("refresh not configured")
        }
        fn load_code_assist(&self, _tok: &str, _req: &Value) -> Result<HttpResponse, ProbeError> {
            self.load_code_assist
                .lock()
                .unwrap()
                .take()
                .expect("loadCodeAssist not configured")
        }
        fn retrieve_user_quota(
            &self,
            _tok: &str,
            _req: &Value,
        ) -> Result<HttpResponse, ProbeError> {
            self.retrieve_user_quota
                .lock()
                .unwrap()
                .take()
                .expect("retrieveUserQuota not configured")
        }
    }

    fn write_creds(dir: &Path, access: &str, refresh: &str, expiry_ms: i64) -> PathBuf {
        let creds_path = dir.join("oauth_creds.json");
        let body = json!({
            "access_token": access,
            "refresh_token": refresh,
            "expiry_date": expiry_ms,
            "id_token": "preserved-id-token",
            "scope": "https://www.googleapis.com/auth/cloud-platform"
        });
        std::fs::write(&creds_path, body.to_string()).unwrap();

        let accounts_path = dir.join("google_accounts.json");
        std::fs::write(
            &accounts_path,
            json!({ "active": "krisspy126@gmail.com", "old": [] }).to_string(),
        )
        .unwrap();
        creds_path
    }

    fn fresh_expiry() -> i64 {
        now_ms() + 3_600_000
    }

    #[test]
    fn parses_quota_buckets_into_model_breakdown() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_creds(tmp.path(), "tok-abc", "rt-xyz", fresh_expiry());

        let client = MockClient::new();
        *client.load_code_assist.lock().unwrap() = Some(Ok(HttpResponse {
            status: 200,
            body: json!({
                "cloudaicompanionProject": "proj-123",
                "currentTier": { "id": "free-tier" }
            }),
        }));
        *client.retrieve_user_quota.lock().unwrap() = Some(Ok(HttpResponse {
            status: 200,
            body: json!({
                "buckets": [
                    { "modelId": "gemini-2.5-pro", "remainingFraction": 0.58, "resetTime": "2026-07-02T14:00:00Z" },
                    { "modelId": "gemini-2.5-flash", "remainingFraction": 0.92 }
                ]
            }),
        }));

        let snap = run(&client, tmp.path(), false).unwrap();
        let account = &snap.accounts[0];
        assert_eq!(account.email, "krisspy126@gmail.com");
        assert_eq!(account.remaining_percentage, 58); // min across models
        assert_eq!(snap.usage_percentage, 42); // 100 - 58
        assert_eq!(
            account.model_breakdown["gemini-2.5-pro"],
            ModelQuota {
                remaining_percentage: 58,
                reset_time: Some("2026-07-02T14:00:00Z".to_string()),
            }
        );
        assert_eq!(
            account.model_breakdown["gemini-2.5-flash"].remaining_percentage,
            92
        );
    }

    #[test]
    fn handles_missing_remaining_amount_at_100_percent() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_creds(tmp.path(), "tok", "rt", fresh_expiry());

        let client = MockClient::new();
        *client.load_code_assist.lock().unwrap() = Some(Ok(HttpResponse {
            status: 200,
            body: json!({ "cloudaicompanionProject": "p" }),
        }));
        // remaining_fraction: 1 with NO remainingAmount — the documented 100%
        // gotcha. We must not panic or NaN; remaining_percentage == 100.
        *client.retrieve_user_quota.lock().unwrap() = Some(Ok(HttpResponse {
            status: 200,
            body: json!({
                "buckets": [
                    { "modelId": "gemini-2.5-pro", "remainingFraction": 1 }
                ]
            }),
        }));

        let snap = run(&client, tmp.path(), false).unwrap();
        assert_eq!(snap.usage_percentage, 0);
        let account = &snap.accounts[0];
        assert_eq!(account.remaining_percentage, 100);
        assert_eq!(
            account.model_breakdown["gemini-2.5-pro"].remaining_percentage,
            100
        );
    }

    #[test]
    fn refreshes_expired_token_before_probing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let expiry = now_ms() - 60_000; // already expired
        let creds_path = write_creds(tmp.path(), "stale-tok", "rt", expiry);

        let client = MockClient::new();
        *client.refresh.lock().unwrap() = Some(Ok(HttpResponse {
            status: 200,
            body: json!({
                "access_token": "fresh-tok",
                "expires_in": 3600,
                "refresh_token": "rotated-rt"
            }),
        }));
        *client.load_code_assist.lock().unwrap() = Some(Ok(HttpResponse {
            status: 200,
            body: json!({ "cloudaicompanionProject": "p" }),
        }));
        *client.retrieve_user_quota.lock().unwrap() = Some(Ok(HttpResponse {
            status: 200,
            body: json!({
                "buckets": [
                    { "modelId": "gemini-2.5-pro", "remainingFraction": 0.5 }
                ]
            }),
        }));

        let snap = run(&client, tmp.path(), true).unwrap();
        assert_eq!(snap.usage_percentage, 50); // remainingFraction 0.5

        // The refreshed token was persisted back, preserving unknown fields.
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&creds_path).unwrap()).unwrap();
        assert_eq!(written["access_token"], "fresh-tok");
        assert_eq!(written["refresh_token"], "rotated-rt");
        assert_eq!(written["id_token"], "preserved-id-token"); // preserved
        assert!(written["expiry_date"].as_f64().unwrap() > now_ms() as f64);
    }

    #[test]
    fn surfaces_invalid_refresh_token_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let expiry = now_ms() - 60_000; // expired
        write_creds(tmp.path(), "stale-tok", "bad-rt", expiry);

        let client = MockClient::new();
        *client.refresh.lock().unwrap() = Some(Ok(HttpResponse {
            status: 400,
            body: json!({
                "error": "invalid_grant",
                "error_description": "Bad refresh token"
            }),
        }));

        let err = run(&client, tmp.path(), false).unwrap_err();
        match err {
            ProbeError::InvalidRefreshToken(msg) => assert!(msg.contains("Bad refresh token")),
            other => panic!("expected InvalidRefreshToken, got {:?}", other),
        }
    }

    #[test]
    fn returns_rate_limited_on_429_from_quota_endpoint() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_creds(tmp.path(), "tok", "rt", fresh_expiry());

        let client = MockClient::new();
        *client.load_code_assist.lock().unwrap() = Some(Ok(HttpResponse {
            status: 200,
            body: json!({ "cloudaicompanionProject": "p" }),
        }));
        *client.retrieve_user_quota.lock().unwrap() = Some(Ok(HttpResponse {
            status: 429,
            body: json!({ "error": "rate limited" }),
        }));

        let err = run(&client, tmp.path(), false).unwrap_err();
        assert!(matches!(err, ProbeError::RateLimited));
    }

    #[test]
    fn returns_rate_limited_on_429_from_load_code_assist() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_creds(tmp.path(), "tok", "rt", fresh_expiry());
        let client = MockClient::new();
        *client.load_code_assist.lock().unwrap() = Some(Ok(HttpResponse {
            status: 429,
            body: json!({}),
        }));

        let err = run(&client, tmp.path(), false).unwrap_err();
        assert!(matches!(err, ProbeError::RateLimited));
    }

    #[test]
    fn no_credentials_when_creds_file_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let client = MockClient::new();
        let err = run(&client, tmp.path(), false).unwrap_err();
        assert!(matches!(err, ProbeError::NoCredentials(_)));
    }

    #[test]
    fn payload_serializes_to_prd_shape_without_common_fields() {
        let mut models = BTreeMap::new();
        models.insert(
            "gemini-2.5-pro".to_string(),
            ModelQuota {
                remaining_percentage: 58,
                reset_time: Some("2026-07-02T14:00:00Z".to_string()),
            },
        );
        let account = AntigravityAccount {
            email: "krisspy126@gmail.com".to_string(),
            remaining_percentage: 58,
            model_breakdown: models,
        };
        let payload = AntigravityPayload::new(42, vec![account]);
        let v: Value = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["type"], "quota-based");
        assert_eq!(v["usagePercentage"], 42);
        assert_eq!(v["accounts"][0]["email"], "krisspy126@gmail.com");
        assert_eq!(v["accounts"][0]["remainingPercentage"], 58);
        assert_eq!(
            v["accounts"][0]["modelBreakdown"]["gemini-2.5-pro"]["remainingPercentage"],
            58
        );
        // Common fields are owned by ProviderEntry, not the payload.
        assert!(v.get("stale").is_none());
        assert!(v.get("lastUpdated").is_none());
    }

    #[test]
    fn empty_payload_has_documented_defaults() {
        let payload = AntigravityPayload::empty();
        assert_eq!(payload.usage_percentage, 0);
        assert!(payload.accounts.is_empty());
    }
}
