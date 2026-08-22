//! OpenCode Go subscription usage Probe using the official bearer-key API.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::ProbeError;
use super::opencode_auth::OpenCodeCredentials;

const USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeGoPayload {
    #[serde(rename = "type")]
    r#type: String,
    windows: GoWindows,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GoWindows {
    rolling: QuotaWindow,
    weekly: QuotaWindow,
    monthly: QuotaWindow,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    usage_percent: u32,
    reset_in_sec: i64,
    reset_at: String,
    status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoApiResponse {
    pub status: u16,
    pub body: String,
}

pub trait OpenCodeGoClient {
    fn get_usage(&self, api_key: &str) -> Result<GoApiResponse, ProbeError>;
}

pub struct ReqwestOpenCodeGoClient {
    http: reqwest::blocking::Client,
}

impl ReqwestOpenCodeGoClient {
    pub fn new() -> Result<Self, ProbeError> {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|error| {
                ProbeError::Io(format!("failed to build OpenCode Go client: {error}"))
            })?;
        Ok(Self { http })
    }
}

impl OpenCodeGoClient for ReqwestOpenCodeGoClient {
    fn get_usage(&self, api_key: &str) -> Result<GoApiResponse, ProbeError> {
        let response = self
            .http
            .get(USAGE_URL)
            .bearer_auth(api_key)
            .send()
            .map_err(|error| ProbeError::Io(format!("OpenCode Go request failed: {error}")))?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|error| {
            ProbeError::Io(format!("failed to read OpenCode Go response: {error}"))
        })?;
        Ok(GoApiResponse { status, body })
    }
}

impl OpenCodeGoPayload {
    pub fn empty() -> Self {
        let empty = QuotaWindow {
            usage_percent: 0,
            reset_in_sec: 0,
            reset_at: String::new(),
            status: "unavailable".into(),
        };
        Self {
            r#type: "quota-based".into(),
            windows: GoWindows {
                rolling: empty.clone(),
                weekly: empty.clone(),
                monthly: empty,
            },
        }
    }
}

pub fn run<C: OpenCodeGoClient>(
    client: &C,
    auth_path: &Path,
) -> Result<OpenCodeGoPayload, ProbeError> {
    run_at(
        client,
        auth_path,
        Utc::now(),
        std::env::var("OPENCODE_API_KEY").ok(),
    )
}

fn run_at<C: OpenCodeGoClient>(
    client: &C,
    auth_path: &Path,
    now: DateTime<Utc>,
    api_key_override: Option<String>,
) -> Result<OpenCodeGoPayload, ProbeError> {
    let credentials = OpenCodeCredentials::load_with_override(auth_path, api_key_override)?;
    let response = client.get_usage(credentials.api_key())?;
    match response.status {
        200 => parse_api_usage(&response.body, now),
        401 => Err(ProbeError::InvalidCredentials(
            "OpenCode API key was rejected; run `kodebar login opencode`".into(),
        )),
        403 => Err(ProbeError::InvalidCredentials(
            "OpenCode Go subscription required for this API key".into(),
        )),
        status => Err(ProbeError::Http {
            status,
            body: response.body,
        }),
    }
}

/// Confirm that a key belongs to an OpenCode workspace. A 403 still proves
/// authentication succeeded; it only means that workspace has no Go plan.
pub fn validate_api_key<C: OpenCodeGoClient>(
    client: &C,
    api_key: &str,
) -> Result<bool, ProbeError> {
    let response = client.get_usage(api_key)?;
    match response.status {
        200 => {
            parse_api_usage(&response.body, Utc::now())?;
            Ok(true)
        }
        403 if is_missing_go_entitlement(&response.body) => Ok(false),
        403 => Err(ProbeError::Http {
            status: 403,
            body: response.body,
        }),
        401 => Err(ProbeError::InvalidCredentials(
            "OpenCode rejected that API key".into(),
        )),
        status => Err(ProbeError::Http {
            status,
            body: response.body,
        }),
    }
}

fn is_missing_go_entitlement(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.pointer("/error/type")?.as_str().map(str::to_owned))
        .as_deref()
        == Some("EntitlementError")
}

#[derive(Deserialize)]
struct ApiUsageResponse {
    usage: ApiWindows,
}

#[derive(Deserialize)]
struct ApiWindows {
    rolling: ApiWindow,
    weekly: ApiWindow,
    monthly: ApiWindow,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiWindow {
    status: String,
    percent: u32,
    resets_at: String,
}

fn parse_api_usage(body: &str, now: DateTime<Utc>) -> Result<OpenCodeGoPayload, ProbeError> {
    let response: ApiUsageResponse = serde_json::from_str(body).map_err(|error| {
        ProbeError::Parse(format!("invalid OpenCode Go usage response: {error}"))
    })?;
    let to_window = |window: ApiWindow| -> Result<QuotaWindow, ProbeError> {
        if window.percent > 100 {
            return Err(ProbeError::Parse(
                "OpenCode Go percent is outside 0..=100".into(),
            ));
        }
        if window.status != "ok" && window.status != "rate-limited" {
            return Err(ProbeError::Parse(format!(
                "OpenCode Go usage window status is {:?}",
                window.status
            )));
        }
        let reset_at = DateTime::parse_from_rfc3339(&window.resets_at)
            .map_err(|error| ProbeError::Parse(format!("invalid OpenCode Go reset time: {error}")))?
            .with_timezone(&Utc);
        Ok(QuotaWindow {
            usage_percent: window.percent,
            reset_in_sec: (reset_at - now).num_seconds().max(0),
            reset_at: reset_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            status: window.status,
        })
    };
    Ok(OpenCodeGoPayload {
        r#type: "quota-based".into(),
        windows: GoWindows {
            rolling: to_window(response.usage.rolling)?,
            weekly: to_window(response.usage.weekly)?,
            monthly: to_window(response.usage.monthly)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockClient {
        response: Mutex<Option<GoApiResponse>>,
        key: Mutex<Option<String>>,
    }

    impl OpenCodeGoClient for MockClient {
        fn get_usage(&self, api_key: &str) -> Result<GoApiResponse, ProbeError> {
            *self.key.lock().unwrap() = Some(api_key.into());
            Ok(self.response.lock().unwrap().take().unwrap())
        }
    }

    fn auth_file() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("auth.json"),
            r#"{"opencode":{"type":"api","key":"oc-secret"}}"#,
        )
        .unwrap();
        tmp
    }

    fn usage_body() -> String {
        r#"{"usage":{"rolling":{"status":"ok","percent":14,"resetsAt":"2026-08-22T00:01:00Z"},"weekly":{"status":"ok","percent":9,"resetsAt":"2026-08-22T00:02:00Z"},"monthly":{"status":"rate-limited","percent":100,"resetsAt":"2026-08-22T00:03:00Z"}}}"#.into()
    }

    #[test]
    fn probe_uses_the_auth_json_key_and_official_response() {
        let tmp = auth_file();
        let client = MockClient {
            response: Mutex::new(Some(GoApiResponse {
                status: 200,
                body: usage_body(),
            })),
            key: Mutex::new(None),
        };
        let now = DateTime::parse_from_rfc3339("2026-08-22T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let payload = run_at(&client, &tmp.path().join("auth.json"), now, None).unwrap();
        let value = serde_json::to_value(payload).unwrap();

        assert_eq!(client.key.lock().unwrap().as_deref(), Some("oc-secret"));
        assert_eq!(value["windows"]["rolling"]["usagePercent"], 14);
        assert_eq!(value["windows"]["rolling"]["resetInSec"], 60);
        assert_eq!(value["windows"]["monthly"]["status"], "rate-limited");
    }

    #[test]
    fn rejects_an_unauthorized_api_key() {
        let tmp = auth_file();
        let client = MockClient {
            response: Mutex::new(Some(GoApiResponse {
                status: 401,
                body: String::new(),
            })),
            key: Mutex::new(None),
        };

        let error = run(&client, &tmp.path().join("auth.json")).unwrap_err();

        assert!(
            matches!(error, ProbeError::InvalidCredentials(message) if message.contains("login"))
        );
    }

    #[test]
    fn validation_accepts_a_real_key_without_a_go_subscription() {
        let client = MockClient {
            response: Mutex::new(Some(GoApiResponse {
                status: 403,
                body: r#"{"type":"error","error":{"type":"EntitlementError","message":"OpenCode Go subscription required."}}"#.into(),
            })),
            key: Mutex::new(None),
        };

        assert!(!validate_api_key(&client, "valid-key").unwrap());
    }

    #[test]
    fn validation_rejects_an_unrecognized_forbidden_response() {
        let client = MockClient {
            response: Mutex::new(Some(GoApiResponse {
                status: 403,
                body: "blocked by an intermediary".into(),
            })),
            key: Mutex::new(None),
        };

        assert!(matches!(
            validate_api_key(&client, "unknown-key"),
            Err(ProbeError::Http { status: 403, .. })
        ));
    }
}
