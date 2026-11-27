//! Native Antigravity quota summary Probe. Shared credentials remain read-only.
//!
//! Protocol reference: CodexBar's AntigravityRemoteUsageFetcher and
//! AntigravityQuotaSummaryParser. Do not use Gemini CLI's retired quota route.
use super::ProbeError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Duration;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value as BusValue};

const BASE: &str = "https://daily-cloudcode-pa.googleapis.com/v1internal:";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityPayload {
    #[serde(rename = "type")]
    kind: String,
    usage_percentage: f64,
    quota_groups: Vec<QuotaGroup>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct QuotaGroup {
    label: String,
    windows: BTreeMap<String, QuotaWindow>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct QuotaWindow {
    label: String,
    usage_percent: f64,
    reset_at: Option<String>,
}

impl AntigravityPayload {
    pub fn empty() -> Self {
        Self {
            kind: "quota-based".into(),
            usage_percentage: 0.0,
            quota_groups: vec![],
        }
    }
}

pub trait AntigravityClient {
    fn refresh(
        &self,
        token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<Value, ProbeError>;
    fn post(&self, method: &str, token: &str, body: &Value) -> Result<Value, ProbeError>;
}

pub struct ReqwestAntigravityClient(reqwest::blocking::Client);

impl ReqwestAntigravityClient {
    pub fn new() -> Result<Self, ProbeError> {
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "amd64"
        };
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(format!("antigravity/hub/2.9.1 linux/{arch}"))
            .build()
            .map(Self)
            .map_err(|_| ProbeError::Io("failed to build Antigravity HTTP client".into()))
    }
}

fn response(response: reqwest::blocking::Response) -> Result<Value, ProbeError> {
    match response.status().as_u16() {
        200 => response
            .json()
            .map_err(|_| ProbeError::Parse("invalid Antigravity response".into())),
        401 => Err(ProbeError::InvalidCredentials(
            "Antigravity session expired; sign in with agy".into(),
        )),
        403 => Err(ProbeError::Unsupported(
            "Antigravity quota access denied; check your account eligibility in Antigravity".into(),
        )),
        429 => Err(ProbeError::RateLimited),
        status => Err(ProbeError::Http {
            status,
            body: String::new(),
        }),
    }
}

impl AntigravityClient for ReqwestAntigravityClient {
    fn refresh(
        &self,
        token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<Value, ProbeError> {
        let result = self
            .0
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", token),
                ("client_id", client_id),
                ("client_secret", client_secret),
            ])
            .send()
            .map_err(|_| ProbeError::Io("Antigravity token refresh request failed".into()))?;
        if result.status().as_u16() == 400 {
            return Err(ProbeError::InvalidRefreshToken(
                "Antigravity token refresh rejected; sign in with agy".into(),
            ));
        }
        response(result)
    }

    fn post(&self, method: &str, token: &str, body: &Value) -> Result<Value, ProbeError> {
        response(
            super::transport::send_read(
                self.0
                    .post(format!("{BASE}{method}"))
                    .bearer_auth(token)
                    .json(body),
            )
            .map_err(|_| ProbeError::Io("Antigravity quota request failed".into()))?,
        )
    }
}

/// Read only unlocked items. Never prompt, unlock, or change the user's keyring.
fn keyring_credentials() -> Result<Option<Value>, ProbeError> {
    let connection = match zbus::blocking::Connection::session() {
        Ok(connection) => connection,
        Err(_) => return Ok(None),
    };
    let service = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.secrets",
        "/org/freedesktop/secrets",
        "org.freedesktop.Secret.Service",
    )
    .map_err(|_| ProbeError::Io("could not access Antigravity keyring".into()))?;
    let attrs = HashMap::from([("service", "gemini"), ("username", "antigravity")]);
    let (unlocked, locked): (Vec<OwnedObjectPath>, Vec<OwnedObjectPath>) =
        match service.call("SearchItems", &(attrs,)) {
            Ok(items) => items,
            Err(_) => return Ok(None),
        };
    if !locked.is_empty() {
        return Err(ProbeError::Io(
            "Antigravity keyring is locked; unlock your desktop wallet".into(),
        ));
    }
    if unlocked.is_empty() {
        return Ok(None);
    }
    if unlocked.len() != 1 {
        return Err(ProbeError::Io(
            "multiple Antigravity keyring sessions found; select an account in agy".into(),
        ));
    }
    let (_, session): (OwnedValue, OwnedObjectPath) = service
        .call("OpenSession", &("plain", BusValue::from("")))
        .map_err(|_| ProbeError::Io("could not open Antigravity keyring session".into()))?;
    let result = (|| {
        let item = zbus::blocking::Proxy::new(
            &connection,
            "org.freedesktop.secrets",
            unlocked[0].as_str(),
            "org.freedesktop.Secret.Item",
        )
        .map_err(|_| ProbeError::Io("could not read Antigravity keyring item".into()))?;
        let (_, _, bytes, _): (OwnedObjectPath, Vec<u8>, Vec<u8>, String) = item
            .call("GetSecret", &(&session,))
            .map_err(|_| ProbeError::Io("could not read Antigravity keyring secret".into()))?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| ProbeError::Parse("invalid Antigravity keyring credentials".into()))
    })();
    if let Ok(proxy) = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.secrets",
        session.as_str(),
        "org.freedesktop.Secret.Session",
    ) {
        let _: Result<(), _> = proxy.call("Close", &());
    }
    result
}

fn file_credentials(home: &Path) -> Result<Value, ProbeError> {
    let path = home.join(".gemini/antigravity-cli/antigravity-oauth-token");
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| ProbeError::Parse("invalid Antigravity CLI token file".into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(
            ProbeError::NoCredentials("sign in with agy to configure Antigravity".into()),
        ),
        Err(_) => Err(ProbeError::Io(
            "could not read Antigravity CLI token file".into(),
        )),
    }
}

pub fn run<C: AntigravityClient>(client: &C) -> Result<AntigravityPayload, ProbeError> {
    let credentials = match keyring_credentials()? {
        Some(credentials) => credentials,
        None => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| ProbeError::NoCredentials("HOME is not set".into()))?;
            file_credentials(Path::new(&home))?
        }
    };
    probe(client, &credentials, configured_oauth_client)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OAuthClient {
    client_id: String,
    client_secret: String,
}

impl OAuthClient {
    fn validate(self) -> Result<Self, ProbeError> {
        if self.client_id.trim().is_empty() || self.client_secret.trim().is_empty() {
            return Err(ProbeError::Io(
                "Antigravity OAuth client metadata is incomplete".into(),
            ));
        }
        Ok(self)
    }
}

fn read_oauth_client(path: &Path) -> Result<OAuthClient, ProbeError> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(path).map_err(|_| ProbeError::Io(
        "configure Antigravity OAuth client metadata in ~/.config/kodebar/antigravity-oauth.json or set both ANTIGRAVITY_OAUTH_CLIENT_ID and ANTIGRAVITY_OAUTH_CLIENT_SECRET".into()
    ))?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o777 != 0o600 {
        return Err(ProbeError::Io(
            "Antigravity OAuth client config must be a regular file with 0600 permissions".into(),
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|_| ProbeError::Io("could not read Antigravity OAuth client config".into()))?;
    let client: OAuthClient = serde_json::from_slice(&bytes)
        .map_err(|_| ProbeError::Parse("invalid Antigravity OAuth client config".into()))?;
    client.validate()
}

fn configured_oauth_client() -> Result<OAuthClient, ProbeError> {
    match (
        std::env::var("ANTIGRAVITY_OAUTH_CLIENT_ID"),
        std::env::var("ANTIGRAVITY_OAUTH_CLIENT_SECRET"),
    ) {
        (Ok(client_id), Ok(client_secret)) => OAuthClient {
            client_id,
            client_secret,
        }
        .validate(),
        (Err(std::env::VarError::NotPresent), Err(std::env::VarError::NotPresent)) => {
            let directory = std::env::var_os("XDG_CONFIG_HOME")
                .map(std::path::PathBuf::from)
                .filter(|path| path.is_absolute())
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(std::path::PathBuf::from)
                        .filter(|path| path.is_absolute())
                        .map(|home| home.join(".config"))
                })
                .ok_or_else(|| {
                    ProbeError::Io("cannot locate Antigravity OAuth client config".into())
                })?;
            read_oauth_client(&directory.join("kodebar/antigravity-oauth.json"))
        }
        _ => Err(ProbeError::Io(
            "set both ANTIGRAVITY_OAUTH_CLIENT_ID and ANTIGRAVITY_OAUTH_CLIENT_SECRET".into(),
        )),
    }
}

fn probe<C: AntigravityClient>(
    client: &C,
    credentials: &Value,
    oauth_client: impl Fn() -> Result<OAuthClient, ProbeError>,
) -> Result<AntigravityPayload, ProbeError> {
    let token = credentials.get("token").unwrap_or(credentials);
    let access = token
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty());
    let expiry = token
        .get("expiry")
        .and_then(Value::as_str)
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|v| v.timestamp_millis())
        .or_else(|| token.get("expiry_date").and_then(Value::as_i64));
    let expired = expiry.is_some_and(|ms| ms <= chrono::Utc::now().timestamp_millis() + 60_000);
    let refresh = || -> Result<String, ProbeError> {
        let refresh_token = token
            .get("refresh_token")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                ProbeError::InvalidCredentials(
                    "Antigravity session expired; sign in with agy".into(),
                )
            })?;
        let metadata = oauth_client()?;
        let refreshed =
            client.refresh(refresh_token, &metadata.client_id, &metadata.client_secret)?;
        refreshed
            .get("access_token")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                ProbeError::Parse("Antigravity refresh response has no access token".into())
            })
    };
    let refreshed = expired || access.is_none();
    let access = match access {
        Some(access) if !expired => access.to_owned(),
        _ => refresh()?,
    };
    match probe_with_token(client, &access) {
        Err(ProbeError::InvalidCredentials(_)) if !refreshed => {
            probe_with_token(client, &refresh()?)
        }
        result => result,
    }
}

fn probe_with_token<C: AntigravityClient>(
    client: &C,
    access: &str,
) -> Result<AntigravityPayload, ProbeError> {
    let setup = client.post(
        "loadCodeAssist",
        access,
        &json!({"metadata": {
            "ideType": "ANTIGRAVITY", "platform": "PLATFORM_UNSPECIFIED", "pluginType": "GEMINI"
        }}),
    )?;
    let project = setup
        .get("cloudaicompanionProject")
        .and_then(|v| v.as_str().or_else(|| v.get("id").and_then(Value::as_str)));
    let rejected_client = project.is_none()
        && setup.get("currentTier").is_none_or(Value::is_null)
        && setup
            .get("ineligibleTiers")
            .and_then(Value::as_array)
            .is_some_and(|tiers| {
                tiers.iter().any(|tier| {
                    tier.get("reasonCode").and_then(Value::as_str)
                        == Some("GOOGLE_TOS_NOT_SUPPORTED_BY_CLIENT")
                })
            });
    if rejected_client {
        return Err(ProbeError::InvalidCredentials(
            "Antigravity OAuth client cannot read quota; sign in with agy".into(),
        ));
    }
    let body = project
        .map(|p| json!({"project": p}))
        .unwrap_or_else(|| json!({}));
    let summary = client.post("retrieveUserQuotaSummary", access, &body)?;
    parse_summary(&summary)
}

fn parse_summary(value: &Value) -> Result<AntigravityPayload, ProbeError> {
    let root = value
        .get("response")
        .or_else(|| value.get("summary"))
        .unwrap_or(value);
    let groups = root
        .get("groups")
        .and_then(Value::as_array)
        .ok_or_else(|| ProbeError::Parse("Antigravity response has no quota groups".into()))?;
    let mut payload = AntigravityPayload::empty();
    for group in groups {
        let label = group
            .get("displayName")
            .or_else(|| group.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("Quota");
        let mut windows = BTreeMap::new();
        for bucket in group
            .get("buckets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if bucket.get("disabled").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let Some(id) = bucket
                .get("bucketId")
                .or_else(|| bucket.get("id"))
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            let fraction = bucket
                .get("remainingFraction")
                .or_else(|| bucket.pointer("/remaining/remainingFraction"))
                .or_else(|| {
                    if bucket.pointer("/remaining/case").and_then(Value::as_str)
                        == Some("remainingFraction")
                    {
                        bucket.pointer("/remaining/value")
                    } else {
                        None
                    }
                })
                .and_then(Value::as_f64);
            let Some(fraction) = fraction.filter(|f| f.is_finite() && (0.0..=1.0).contains(f))
            else {
                continue;
            };
            let usage_percent = (1.0 - fraction) * 100.0;
            payload.usage_percentage = payload.usage_percentage.max(usage_percent);
            let window = QuotaWindow {
                label: bucket
                    .get("displayName")
                    .or_else(|| bucket.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .into(),
                usage_percent,
                reset_at: bucket
                    .get("resetTime")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            };
            // Never let a duplicate bucket conceal the more exhausted window.
            if windows
                .get(id)
                .is_none_or(|prior: &QuotaWindow| prior.usage_percent < usage_percent)
            {
                windows.insert(id.into(), window);
            }
        }
        if !windows.is_empty() {
            payload.quota_groups.push(QuotaGroup {
                label: label.into(),
                windows,
            });
        }
    }
    if payload.quota_groups.is_empty() {
        return Err(ProbeError::Parse(
            "Antigravity returned no measured quota windows".into(),
        ));
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    fn test_oauth_client() -> Result<OAuthClient, ProbeError> {
        Ok(OAuthClient {
            client_id: "test-client".into(),
            client_secret: "test-secret".into(),
        })
    }

    #[test]
    fn oauth_client_config_requires_private_complete_metadata() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oauth.json");
        assert!(read_oauth_client(&path).is_err());
        std::fs::write(
            &path,
            r#"{"clientId":"test-client","clientSecret":"test-secret"}"#,
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_oauth_client(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let metadata = read_oauth_client(&path).unwrap();
        assert_eq!(metadata.client_id, "test-client");
        assert_eq!(metadata.client_secret, "test-secret");
        std::fs::write(&path, r#"{"clientId":"test-client","clientSecret":""}"#).unwrap();
        assert!(read_oauth_client(&path).is_err());
        std::fs::write(&path, "invalid").unwrap();
        assert!(read_oauth_client(&path).is_err());
    }

    fn summary() -> Value {
        json!({"groups": [{"displayName": "Gemini", "buckets": [
            {"bucketId": "session", "displayName": "5-hour", "remainingFraction": 0.75,
             "resetTime": "2026-11-27T18:00:00Z"},
            {"bucketId": "weekly", "displayName": "Weekly", "remaining": {
                "case": "remainingFraction", "value": 0.2}},
            {"bucketId": "disabled", "remainingFraction": 0.0, "disabled": true},
            {"bucketId": "unknown"}, {"bucketId": "invalid", "remainingFraction": 2.0}
        ]}]})
    }

    #[test]
    fn parses_measured_groups_and_uses_worst_window() {
        for body in [
            summary(),
            json!({"response": summary()}),
            json!({"summary": summary()}),
        ] {
            let payload = parse_summary(&body).unwrap();
            assert!((payload.usage_percentage - 80.0).abs() < 0.001);
            let windows = &payload.quota_groups[0].windows;
            assert_eq!(windows.len(), 2);
            assert_eq!(
                windows["session"].reset_at.as_deref(),
                Some("2026-11-27T18:00:00Z")
            );
            let encoded = serde_json::to_value(&payload).unwrap();
            assert_eq!(encoded["type"], "quota-based");
            assert!(encoded.get("quotaGroups").is_some());
        }
    }

    #[test]
    fn unknown_quota_is_never_reported_as_full() {
        for value in [
            json!({}),
            json!({"groups": []}),
            json!({"groups": [{"buckets": [
                {"bucketId": "unknown"}, {"bucketId": "disabled", "remainingFraction": 1.0, "disabled": true}
            ]}]}),
        ] {
            assert!(parse_summary(&value).is_err());
        }
    }

    struct Mock {
        replies: RefCell<VecDeque<Result<Value, ProbeError>>>,
        calls: RefCell<Vec<(String, String, Value)>>,
    }

    impl AntigravityClient for Mock {
        fn refresh(&self, token: &str, _: &str, _: &str) -> Result<Value, ProbeError> {
            assert_eq!(token, "refresh-test");
            self.calls
                .borrow_mut()
                .push(("refresh".into(), "".into(), json!({})));
            Ok(json!({"access_token": "fresh-test"}))
        }
        fn post(&self, method: &str, token: &str, body: &Value) -> Result<Value, ProbeError> {
            self.calls
                .borrow_mut()
                .push((method.into(), token.into(), body.clone()));
            self.replies
                .borrow_mut()
                .pop_front()
                .expect("unexpected request")
        }
    }

    #[test]
    fn refreshes_expired_go_token_without_mutating_shared_credentials() {
        let creds = json!({"token": {"access_token": "expired-test", "refresh_token": "refresh-test",
            "expiry": "2020-01-01T00:00:00Z"}});
        let before = creds.clone();
        let client = Mock {
            replies: RefCell::new(VecDeque::from([
                Ok(json!({"cloudaicompanionProject": {"id": "test-project"}})),
                Ok(summary()),
            ])),
            calls: RefCell::new(vec![]),
        };
        probe(&client, &creds, test_oauth_client).unwrap();
        assert_eq!(creds, before);
        let calls = client.calls.borrow();
        assert_eq!(calls[0].0, "refresh");
        assert_eq!(calls[1].1, "fresh-test");
        assert_eq!(calls[2].0, "retrieveUserQuotaSummary");
        assert_eq!(calls[2].2, json!({"project": "test-project"}));
    }

    #[test]
    fn retries_rejected_access_token_once_and_propagates_rate_limit() {
        let client = Mock {
            replies: RefCell::new(VecDeque::from([
                Err(ProbeError::InvalidCredentials("expired".into())),
                Ok(json!({})),
                Err(ProbeError::RateLimited),
            ])),
            calls: RefCell::new(vec![]),
        };
        let result = probe(
            &client,
            &json!({"token": {
                "access_token": "test", "refresh_token": "refresh-test"
            }}),
            test_oauth_client,
        );
        assert!(matches!(result, Err(ProbeError::RateLimited)));
        assert_eq!(
            client
                .calls
                .borrow()
                .iter()
                .filter(|call| call.0 == "refresh")
                .count(),
            1
        );
    }

    #[test]
    fn reads_cli_file_and_distinguishes_missing_from_malformed() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            file_credentials(dir.path()),
            Err(ProbeError::NoCredentials(_))
        ));
        let folder = dir.path().join(".gemini/antigravity-cli");
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("antigravity-oauth-token");
        std::fs::write(&file, "invalid").unwrap();
        assert!(matches!(
            file_credentials(dir.path()),
            Err(ProbeError::Parse(_))
        ));
        std::fs::write(&file, r#"{"token":{"access_token":"test"}}"#).unwrap();
        assert_eq!(
            file_credentials(dir.path()).unwrap()["token"]["access_token"],
            "test"
        );
    }
}
