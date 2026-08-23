//! ChatGPT subscription-plan quota Probe.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};

use super::ProbeError;

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const USER_AGENT: &str = concat!("kodebar/", env!("CARGO_PKG_VERSION"));

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptPayload {
    #[serde(rename = "type")]
    r#type: String,
    plan_type: String,
    limits: BTreeMap<String, ChatGptLimit>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptLimit {
    #[serde(skip_serializing_if = "Option::is_none")]
    limit_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary: Option<ChatGptQuotaWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary: Option<ChatGptQuotaWindow>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptQuotaWindow {
    usage_percent: f64,
    window_duration_sec: i64,
    reset_at: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChatGptUsageResponse {
    pub status: u16,
    pub body: String,
}

impl std::fmt::Debug for ChatGptUsageResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChatGptUsageResponse")
            .field("status", &self.status)
            .field("body", &"[REDACTED]")
            .finish()
    }
}

pub trait ChatGptClient {
    fn get_usage(
        &self,
        access_token: &str,
        account_id: &str,
    ) -> Result<ChatGptUsageResponse, ProbeError>;
}

pub struct ReqwestChatGptClient {
    http: reqwest::blocking::Client,
}

impl ReqwestChatGptClient {
    pub fn new() -> Result<Self, ProbeError> {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|error| {
                ProbeError::Io(format!("failed to build ChatGPT usage client: {error}"))
            })?;
        Ok(Self { http })
    }
}

impl ChatGptClient for ReqwestChatGptClient {
    fn get_usage(
        &self,
        access_token: &str,
        account_id: &str,
    ) -> Result<ChatGptUsageResponse, ProbeError> {
        let response = self
            .http
            .get(USAGE_URL)
            .bearer_auth(access_token)
            .header("ChatGPT-Account-ID", account_id)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .map_err(|error| ProbeError::Io(format!("ChatGPT usage request failed: {error}")))?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|error| {
            ProbeError::Io(format!("failed to read ChatGPT usage response: {error}"))
        })?;
        Ok(ChatGptUsageResponse { status, body })
    }
}

struct Secret(String);

impl Secret {
    fn new(value: String, name: &str) -> Result<Self, ProbeError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(ProbeError::Parse(format!(
                "ChatGPT credential {name} is empty"
            )));
        }
        Ok(Self(value.to_owned()))
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Debug)]
struct ChatGptCredentials {
    access_token: Secret,
    account_id: Secret,
}

#[derive(Deserialize)]
struct AuthFile {
    tokens: AuthTokens,
}

#[derive(Deserialize)]
struct AuthTokens {
    access_token: String,
    account_id: String,
}

impl ChatGptCredentials {
    fn load(path: &Path) -> Result<Self, ProbeError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    ProbeError::NoCredentials(format!("{} does not exist", path.display()))
                } else {
                    ProbeError::Io(format!("failed to inspect {}: {error}", path.display()))
                }
            })?;
            if !metadata.is_file() {
                return Err(ProbeError::Io(format!(
                    "{} is not a regular credential file",
                    path.display()
                )));
            }
            if metadata.permissions().mode() & 0o777 != 0o600 {
                return Err(ProbeError::Io(format!(
                    "{} must have 0600 permissions",
                    path.display()
                )));
            }
        }
        let text = std::fs::read_to_string(path).map_err(|error| {
            ProbeError::Io(format!("failed to read {}: {error}", path.display()))
        })?;
        let auth: AuthFile = serde_json::from_str(&text).map_err(|error| {
            ProbeError::Parse(format!("failed to parse {}: {error}", path.display()))
        })?;
        Ok(Self {
            access_token: Secret::new(auth.tokens.access_token, "access_token")?,
            account_id: Secret::new(auth.tokens.account_id, "account_id")?,
        })
    }
}

pub fn default_auth_path() -> Result<PathBuf, ProbeError> {
    default_auth_path_with_home(std::env::var("HOME").ok())
}

fn default_auth_path_with_home(home: Option<String>) -> Result<PathBuf, ProbeError> {
    if let Some(home) = home.map(PathBuf::from).filter(|path| path.is_absolute()) {
        return Ok(home.join(".codex/auth.json"));
    }
    Err(ProbeError::Io(
        "cannot locate ChatGPT credentials without an absolute HOME".into(),
    ))
}

pub fn run<C: ChatGptClient>(client: &C, auth_path: &Path) -> Result<ChatGptPayload, ProbeError> {
    let credentials = ChatGptCredentials::load(auth_path)?;
    let response = client.get_usage(
        credentials.access_token.expose(),
        credentials.account_id.expose(),
    )?;
    match response.status {
        200 => parse_usage(&response.body),
        401 => Err(ProbeError::InvalidCredentials(
            "ChatGPT session was rejected; sign in with ChatGPT in Codex again".into(),
        )),
        429 => Err(ProbeError::RateLimited),
        status => Err(ProbeError::Http {
            status,
            // Do not retain an identity-bearing ChatGPT response body.
            body: String::new(),
        }),
    }
}

#[derive(Deserialize)]
struct UsageResponse {
    plan_type: String,
    #[serde(default)]
    rate_limit: Option<RateLimit>,
    #[serde(default)]
    additional_rate_limits: Option<Vec<AdditionalRateLimit>>,
}

#[derive(Deserialize, Default)]
struct RateLimit {
    #[serde(default)]
    primary_window: Option<UsageWindow>,
    #[serde(default)]
    secondary_window: Option<UsageWindow>,
}

#[derive(Deserialize)]
struct AdditionalRateLimit {
    limit_name: String,
    metered_feature: String,
    #[serde(default)]
    rate_limit: Option<RateLimit>,
}

#[derive(Deserialize)]
struct UsageWindow {
    used_percent: f64,
    limit_window_seconds: i64,
    reset_at: i64,
}

fn parse_usage(body: &str) -> Result<ChatGptPayload, ProbeError> {
    let response: UsageResponse = serde_json::from_str(body)
        .map_err(|error| ProbeError::Parse(format!("invalid ChatGPT usage response: {error}")))?;
    let mut limits = BTreeMap::new();
    limits.insert(
        "codex".into(),
        convert_limit(None, response.rate_limit.unwrap_or_default())?,
    );
    for additional in response.additional_rate_limits.unwrap_or_default() {
        if additional.metered_feature.trim().is_empty() {
            return Err(ProbeError::Parse(
                "ChatGPT additional rate limit has an empty metered_feature".into(),
            ));
        }
        if limits.contains_key(&additional.metered_feature) {
            return Err(ProbeError::Parse(format!(
                "ChatGPT usage response contains duplicate limit {:?}",
                additional.metered_feature
            )));
        }
        limits.insert(
            additional.metered_feature,
            convert_limit(
                Some(additional.limit_name),
                additional.rate_limit.unwrap_or_default(),
            )?,
        );
    }
    Ok(ChatGptPayload {
        r#type: "quota-based".into(),
        plan_type: response.plan_type,
        limits,
    })
}

impl ChatGptPayload {
    /// Empty payload used when the first Probe fails before any last-known-good data exists.
    pub fn empty() -> Self {
        Self {
            r#type: "quota-based".into(),
            plan_type: "unknown".into(),
            limits: BTreeMap::from([(
                "codex".into(),
                ChatGptLimit {
                    limit_name: None,
                    primary: None,
                    secondary: None,
                },
            )]),
        }
    }
}

fn convert_limit(
    limit_name: Option<String>,
    rate_limit: RateLimit,
) -> Result<ChatGptLimit, ProbeError> {
    Ok(ChatGptLimit {
        limit_name,
        primary: rate_limit.primary_window.map(convert_window).transpose()?,
        secondary: rate_limit
            .secondary_window
            .map(convert_window)
            .transpose()?,
    })
}

fn convert_window(window: UsageWindow) -> Result<ChatGptQuotaWindow, ProbeError> {
    if !window.used_percent.is_finite() || !(0.0..=100.0).contains(&window.used_percent) {
        return Err(ProbeError::Parse(
            "ChatGPT used_percent is outside 0..=100".into(),
        ));
    }
    if window.limit_window_seconds < 0 {
        return Err(ProbeError::Parse(
            "ChatGPT limit_window_seconds is negative".into(),
        ));
    }
    let reset_at = Utc
        .timestamp_opt(window.reset_at, 0)
        .single()
        .ok_or_else(|| ProbeError::Parse("ChatGPT reset_at is out of range".into()))?;
    Ok(ChatGptQuotaWindow {
        usage_percent: window.used_percent,
        window_duration_sec: window.limit_window_seconds,
        reset_at: reset_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::ProbeError;
    use serde_json::json;
    use std::sync::Mutex;

    #[test]
    fn uses_the_live_chatgpt_usage_compatibility_route() {
        assert_eq!(USAGE_URL, "https://chatgpt.com/backend-api/wham/usage");
    }

    struct MockClient {
        response: Mutex<Option<ChatGptUsageResponse>>,
    }

    impl ChatGptClient for MockClient {
        fn get_usage(
            &self,
            access_token: &str,
            account_id: &str,
        ) -> Result<ChatGptUsageResponse, ProbeError> {
            assert_eq!(access_token, "access-secret");
            assert_eq!(account_id, "account-secret");
            Ok(self.response.lock().unwrap().take().unwrap())
        }
    }

    fn private_auth_file() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("auth.json");
        std::fs::write(
            &path,
            json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": "access-secret",
                    "account_id": "account-secret",
                    "id_token": "unused",
                    "refresh_token": "unused"
                }
            })
            .to_string(),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        tmp
    }

    #[test]
    fn returns_default_and_additional_subscription_windows() {
        let tmp = private_auth_file();
        let client = MockClient {
            response: Mutex::new(Some(ChatGptUsageResponse {
                status: 200,
                body: json!({
                    "plan_type": "plus",
                    "rate_limit": {
                        "primary_window": {
                            "used_percent": 25.5,
                            "limit_window_seconds": 18000,
                            "reset_at": 1787446800
                        },
                        "secondary_window": {
                            "used_percent": 40,
                            "limit_window_seconds": 604800,
                            "reset_at": 1788051600
                        }
                    },
                    "additional_rate_limits": [{
                        "limit_name": "code review",
                        "metered_feature": "codex_review",
                        "rate_limit": {
                            "primary_window": {
                                "used_percent": 10,
                                "limit_window_seconds": 3600,
                                "reset_at": 1787432400
                            }
                        }
                    }]
                })
                .to_string(),
            })),
        };

        let payload = run(&client, &tmp.path().join("auth.json")).unwrap();
        let value = serde_json::to_value(payload).unwrap();

        assert_eq!(value["type"], "quota-based");
        assert_eq!(value["planType"], "plus");
        assert_eq!(value["limits"]["codex"]["primary"]["usagePercent"], 25.5);
        assert_eq!(
            value["limits"]["codex"]["primary"]["windowDurationSec"],
            18_000
        );
        assert_eq!(value["limits"]["codex_review"]["limitName"], "code review");
        assert_eq!(
            value["limits"]["codex_review"]["primary"]["usagePercent"],
            10.0
        );
    }

    #[test]
    fn accepts_null_optional_limits_without_inventing_windows() {
        let tmp = private_auth_file();
        let client = MockClient {
            response: Mutex::new(Some(ChatGptUsageResponse {
                status: 200,
                body: json!({
                    "plan_type": "plus",
                    "rate_limit": null,
                    "additional_rate_limits": null,
                    "credits": null,
                    "email": "must-not-be-copied@example.test",
                    "account_id": "must-not-be-copied"
                })
                .to_string(),
            })),
        };

        let payload = run(&client, &tmp.path().join("auth.json")).unwrap();
        let value = serde_json::to_value(payload).unwrap();

        assert_eq!(value["limits"]["codex"], json!({}));
        assert!(value.get("credits").is_none());
        assert!(value.get("email").is_none());
        assert!(value.get("accountId").is_none());
    }

    #[test]
    fn empty_payload_serializes_as_a_stable_quota_placeholder() {
        let value = serde_json::to_value(ChatGptPayload::empty()).unwrap();

        assert_eq!(
            value,
            json!({
                "type": "quota-based",
                "planType": "unknown",
                "limits": {
                    "codex": {}
                }
            })
        );
    }

    #[test]
    fn rejected_session_requests_chatgpt_login_without_exposing_the_body() {
        let tmp = private_auth_file();
        let client = MockClient {
            response: Mutex::new(Some(ChatGptUsageResponse {
                status: 401,
                body: "access-secret must never appear in the error".into(),
            })),
        };

        let error = run(&client, &tmp.path().join("auth.json")).unwrap_err();
        let message = error.user_message();

        assert!(matches!(error, ProbeError::InvalidCredentials(_)));
        assert!(message.contains("sign in with ChatGPT in Codex again"));
        assert!(!message.contains("access-secret"));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_read_a_chatgpt_credential_file_that_is_not_0600() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = private_auth_file();
        let path = tmp.path().join("auth.json");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let client = MockClient {
            response: Mutex::new(None),
        };

        let error = run(&client, &path).unwrap_err();

        assert!(matches!(error, ProbeError::Io(message) if message.contains("0600")));
    }

    #[test]
    fn default_auth_path_requires_an_absolute_home() {
        assert_eq!(
            default_auth_path_with_home(Some("/home/test".into())).unwrap(),
            Path::new("/home/test/.codex/auth.json")
        );
        assert!(matches!(
            default_auth_path_with_home(Some("relative".into())),
            Err(ProbeError::Io(message)) if message.contains("absolute HOME")
        ));
        assert!(matches!(
            default_auth_path_with_home(None),
            Err(ProbeError::Io(message)) if message.contains("absolute HOME")
        ));
    }

    #[test]
    fn credential_debug_output_is_redacted() {
        let credentials = ChatGptCredentials {
            access_token: Secret::new("access-secret".into(), "access_token").unwrap(),
            account_id: Secret::new("account-secret".into(), "account_id").unwrap(),
        };

        let debug = format!("{credentials:?}");

        assert!(!debug.contains("access-secret"));
        assert!(!debug.contains("account-secret"));
        assert!(debug.contains("[REDACTED]"));
    }
}
