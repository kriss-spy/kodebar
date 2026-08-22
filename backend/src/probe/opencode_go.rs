//! OpenCode Go subscription usage Probe.

use std::path::Path;

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use super::ProbeError;
pub use super::opencode_dashboard::SESSION_EXPIRED_MESSAGE;
use super::opencode_dashboard::{DashboardClient, DashboardCredentials};

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
struct ParsedWindow {
    status: String,
    usage_percent: u32,
    reset_in_sec: i64,
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

pub fn run<C: DashboardClient>(
    client: &C,
    credentials_path: &Path,
) -> Result<OpenCodeGoPayload, ProbeError> {
    run_at(client, credentials_path, Utc::now())
}

fn run_at<C: DashboardClient>(
    client: &C,
    credentials_path: &Path,
    now: DateTime<Utc>,
) -> Result<OpenCodeGoPayload, ProbeError> {
    let credentials = DashboardCredentials::load(credentials_path)?;
    let response = client.get(
        &credentials,
        &format!("workspace/{}/go", credentials.workspace_id()),
    )?;
    if response.status == 401 || response.is_login_redirect() {
        return Err(ProbeError::SessionExpired(SESSION_EXPIRED_MESSAGE.into()));
    }
    if response.status != 200 {
        return Err(ProbeError::Http {
            status: response.status,
            body: response.body,
        });
    }

    let windows = parse_usage_windows(&response.body)?;
    let to_window = |window: ParsedWindow| {
        if window.status != "ok" {
            return Err(ProbeError::Parse(format!(
                "OpenCode Go usage window status is {:?}; expected ok",
                window.status
            )));
        }
        let reset_at = now
            .checked_add_signed(TimeDelta::seconds(window.reset_in_sec))
            .ok_or_else(|| ProbeError::Parse("OpenCode Go reset time overflow".into()))?;
        Ok(QuotaWindow {
            usage_percent: window.usage_percent,
            reset_in_sec: window.reset_in_sec,
            reset_at: reset_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            status: window.status,
        })
    };

    let [rolling, weekly, monthly] = windows;

    Ok(OpenCodeGoPayload {
        r#type: "quota-based".into(),
        windows: GoWindows {
            rolling: to_window(rolling)?,
            weekly: to_window(weekly)?,
            monthly: to_window(monthly)?,
        },
    })
}

fn parse_usage_windows(html: &str) -> Result<[ParsedWindow; 3], ProbeError> {
    let hydration_start = html
        .find("_$HY")
        .ok_or_else(|| ProbeError::Parse("OpenCode Go dashboard missing Solid hydration".into()))?;
    let hydration = &html[hydration_start..];
    let hydration = hydration
        .split_once("</script>")
        .map_or(hydration, |(script, _)| script);
    let mut windows = Vec::new();
    let mut object_starts = Vec::new();

    for (index, ch) in hydration.char_indices() {
        match ch {
            '{' => object_starts.push(index),
            '}' => {
                let Some(start) = object_starts.pop() else {
                    continue;
                };
                let object = &hydration[start..=index];
                if !object.contains("usagePercent") || !object.contains("resetInSec") {
                    continue;
                }
                let usage = u32::try_from(parse_unsigned_field(object, "usagePercent")?)
                    .map_err(|_| ProbeError::Parse("usagePercent is too large".into()))?;
                if usage > 100 {
                    return Err(ProbeError::Parse(
                        "OpenCode Go usagePercent is outside 0..=100".into(),
                    ));
                }
                let reset = i64::try_from(parse_unsigned_field(object, "resetInSec")?)
                    .map_err(|_| ProbeError::Parse("resetInSec is too large".into()))?;
                windows.push(ParsedWindow {
                    status: parse_string_field(object, "status")?,
                    usage_percent: usage,
                    reset_in_sec: reset,
                });
            }
            _ => {}
        }
    }

    if windows.len() != 3 {
        return Err(ProbeError::Parse(format!(
            "OpenCode Go dashboard contained {} usage windows; expected 3",
            windows.len()
        )));
    }
    windows.try_into().map_err(|windows: Vec<_>| {
        ProbeError::Parse(format!(
            "OpenCode Go dashboard contained {} usage windows; expected 3",
            windows.len()
        ))
    })
}

fn parse_unsigned_field(object: &str, field: &str) -> Result<u64, ProbeError> {
    let digits: String = value_after_field(object, field)?
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    digits
        .parse()
        .map_err(|_| ProbeError::Parse(format!("usage window {field} is not an unsigned integer")))
}

fn parse_string_field(object: &str, field: &str) -> Result<String, ProbeError> {
    let value = value_after_field(object, field)?;
    let value = value.strip_prefix('\\').unwrap_or(value);
    let value = value
        .strip_prefix('"')
        .or_else(|| value.strip_prefix('\''))
        .ok_or_else(|| ProbeError::Parse(format!("usage window {field} is not a string")))?;
    let parsed: String = value
        .chars()
        .take_while(|ch| *ch != '"' && *ch != '\'' && *ch != '\\')
        .collect();
    if parsed.is_empty() {
        return Err(ProbeError::Parse(format!("usage window {field} is empty")));
    }
    Ok(parsed)
}

fn value_after_field<'a>(object: &'a str, field: &str) -> Result<&'a str, ProbeError> {
    for (start, _) in object.match_indices(field) {
        if object[..start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            continue;
        }

        let mut remainder = object[start + field.len()..].trim_start();
        if remainder
            .chars()
            .next()
            .is_some_and(|ch| ch == '"' || ch == '\'')
        {
            remainder = remainder[1..].trim_start();
        }
        if let Some(value) = remainder.strip_prefix(':') {
            return Ok(value.trim_start());
        }
    }

    Err(ProbeError::Parse(format!("usage window missing {field}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::opencode_dashboard::{DashboardCredentials, DashboardResponse};
    use std::sync::Mutex;

    struct MockClient {
        response: Mutex<Option<DashboardResponse>>,
        request: Mutex<Option<(String, String)>>,
    }

    impl DashboardClient for MockClient {
        fn get(
            &self,
            credentials: &DashboardCredentials,
            page_path: &str,
        ) -> Result<DashboardResponse, ProbeError> {
            *self.request.lock().unwrap() =
                Some((credentials.auth_cookie().into(), page_path.into()));
            Ok(self.response.lock().unwrap().take().unwrap())
        }
    }

    fn credentials_file() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("opencode-go.json");
        std::fs::write(
            &path,
            r#"{"workspaceId":"wrk_test","authCookie":"cookie-test"}"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        tmp
    }

    #[test]
    fn parses_three_quota_windows_from_ssr_hydration() {
        let html = r#"<script>window._$HY=[{status:"ok",resetInSec:11302,usagePercent:14},{status:"ok",resetInSec:332778,usagePercent:9},{status:"ok",resetInSec:2282289,usagePercent:4}]</script>"#;

        let windows = parse_usage_windows(html).unwrap();

        assert_eq!(windows[0].usage_percent, 14);
        assert_eq!(windows[0].reset_in_sec, 11_302);
        assert_eq!(windows[1].usage_percent, 9);
        assert_eq!(windows[1].reset_in_sec, 332_778);
        assert_eq!(windows[2].usage_percent, 4);
        assert_eq!(windows[2].reset_in_sec, 2_282_289);
    }

    #[test]
    fn ignores_usage_shaped_objects_outside_solid_hydration() {
        let html = r#"<script>const unrelated={status:"ok",resetInSec:1,usagePercent:99}</script><script>window._$HY=[{status:"ok",resetInSec:60,usagePercent:14},{status:"ok",resetInSec:120,usagePercent:9},{status:"ok",resetInSec:180,usagePercent:4}]</script>"#;

        let windows = parse_usage_windows(html).unwrap();

        assert_eq!(windows[0].usage_percent, 14);
        assert_eq!(windows[1].usage_percent, 9);
        assert_eq!(windows[2].usage_percent, 4);
    }

    #[test]
    fn ignores_similarly_named_window_fields() {
        let html = r#"window._$HY=[{status:"ok",usagePercentMax:100,usagePercent:14,resetInSecMax:999,resetInSec:60},{status:"ok",usagePercent:9,resetInSec:120},{status:"ok",usagePercent:4,resetInSec:180}]"#;

        let windows = parse_usage_windows(html).unwrap();

        assert_eq!(windows[0].usage_percent, 14);
        assert_eq!(windows[0].reset_in_sec, 60);
    }

    #[test]
    fn tolerates_nested_metadata_inside_usage_windows() {
        let html = r#"window._$HY=[{metadata:{tier:"go"},status:"ok",usagePercent:14,resetInSec:60},{metadata:{tier:"go"},status:"ok",usagePercent:9,resetInSec:120},{metadata:{tier:"go"},status:"ok",usagePercent:4,resetInSec:180}]"#;

        let windows = parse_usage_windows(html).unwrap();

        assert_eq!(windows[0].usage_percent, 14);
        assert_eq!(windows[2].reset_in_sec, 180);
    }

    #[test]
    fn probe_returns_named_windows_with_absolute_reset_times() {
        let tmp = credentials_file();
        let client = MockClient {
            response: Mutex::new(Some(DashboardResponse {
                status: 200,
                body: r#"window._$HY=[{status:"ok",resetInSec:60,usagePercent:14},{status:"ok",resetInSec:120,usagePercent:9},{status:"ok",resetInSec:180,usagePercent:4}]"#.into(),
                location: None,
            })),
            request: Mutex::new(None),
        };
        let now = DateTime::parse_from_rfc3339("2026-08-22T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let payload = run_at(&client, &tmp.path().join("opencode-go.json"), now).unwrap();
        let value = serde_json::to_value(payload).unwrap();

        assert_eq!(value["type"], "quota-based");
        assert_eq!(value["windows"]["rolling"]["usagePercent"], 14);
        assert_eq!(
            value["windows"]["rolling"]["resetAt"],
            "2026-08-22T00:01:00Z"
        );
        assert_eq!(value["windows"]["weekly"]["resetInSec"], 120);
        assert_eq!(value["windows"]["monthly"]["status"], "ok");
        assert_eq!(
            client.request.lock().unwrap().as_ref().unwrap(),
            &("cookie-test".into(), "workspace/wrk_test/go".into())
        );
    }

    #[test]
    fn probe_surfaces_expired_dashboard_session() {
        let tmp = credentials_file();
        let client = MockClient {
            response: Mutex::new(Some(DashboardResponse {
                status: 307,
                body: String::new(),
                location: Some("/login".into()),
            })),
            request: Mutex::new(None),
        };

        let error = run(&client, &tmp.path().join("opencode-go.json")).unwrap_err();
        assert!(
            matches!(error, ProbeError::SessionExpired(ref message) if message == SESSION_EXPIRED_MESSAGE)
        );
    }

    #[test]
    fn rejects_a_non_ok_usage_window() {
        let tmp = credentials_file();
        let client = MockClient {
            response: Mutex::new(Some(DashboardResponse {
                status: 200,
                body: r#"window._$HY=[{status:"unavailable",resetInSec:0,usagePercent:0},{status:"ok",resetInSec:1,usagePercent:1},{status:"ok",resetInSec:2,usagePercent:2}]"#.into(),
                location: None,
            })),
            request: Mutex::new(None),
        };

        let error = run(&client, &tmp.path().join("opencode-go.json")).unwrap_err();
        assert!(matches!(error, ProbeError::Parse(message) if message.contains("unavailable")));
    }
}
