//! OpenCode Zen credit-balance Probe.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::ProbeError;
use super::opencode_dashboard::{DashboardClient, DashboardCredentials, SESSION_EXPIRED_MESSAGE};

const MICROCENTS_PER_CENT: u64 = 1_000_000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeZenPayload {
    #[serde(rename = "type")]
    r#type: String,
    pub balance: i64,
    pub balance_formatted: String,
    pub use_balance: bool,
    pub reload_amount: i64,
    pub reload_trigger: i64,
}

impl OpenCodeZenPayload {
    pub fn empty() -> Self {
        Self {
            r#type: "pay-as-you-go".into(),
            balance: 0,
            balance_formatted: "$0.00".into(),
            use_balance: false,
            reload_amount: 0,
            reload_trigger: 0,
        }
    }
}

pub fn run<C: DashboardClient>(
    client: &C,
    credentials_path: &Path,
) -> Result<OpenCodeZenPayload, ProbeError> {
    let credentials = DashboardCredentials::load(credentials_path)?;
    let response = client.get(
        &credentials,
        &format!("workspace/{}", credentials.workspace_id()),
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

    let hydration = solid_hydration(&response.body)?;
    let balance = parse_signed_field(hydration, "balance")?;
    Ok(OpenCodeZenPayload {
        r#type: "pay-as-you-go".into(),
        balance,
        balance_formatted: format_balance(balance),
        use_balance: parse_bool_field(hydration, "useBalance")?,
        reload_amount: parse_signed_field(hydration, "reloadAmount")?,
        reload_trigger: parse_signed_field(hydration, "reloadTrigger")?,
    })
}

fn solid_hydration(html: &str) -> Result<&str, ProbeError> {
    let start = html.find("_$HY").ok_or_else(|| {
        ProbeError::Parse("OpenCode Zen dashboard missing Solid hydration".into())
    })?;
    let hydration = &html[start..];
    Ok(hydration
        .split_once("</script>")
        .map_or(hydration, |(script, _)| script))
}

fn value_after_field<'a>(text: &'a str, field: &str) -> Result<&'a str, ProbeError> {
    for (start, _) in text.match_indices(field) {
        if text[..start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            continue;
        }

        let mut remainder = text[start + field.len()..].trim_start();
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

    Err(ProbeError::Parse(format!(
        "OpenCode Zen dashboard missing {field}"
    )))
}

fn parse_signed_field(text: &str, field: &str) -> Result<i64, ProbeError> {
    let value = value_after_field(text, field)?;
    let number: String = value
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '-')
        .collect();
    number.parse().map_err(|_| {
        ProbeError::Parse(format!(
            "OpenCode Zen dashboard {field} is not a signed integer"
        ))
    })
}

fn parse_bool_field(text: &str, field: &str) -> Result<bool, ProbeError> {
    let value = value_after_field(text, field)?;
    if value.starts_with("true") {
        Ok(true)
    } else if value.starts_with("false") {
        Ok(false)
    } else {
        Err(ProbeError::Parse(format!(
            "OpenCode Zen dashboard {field} is not a boolean"
        )))
    }
}

fn format_balance(balance_microcents: i64) -> String {
    let absolute = balance_microcents.unsigned_abs();
    let cents = absolute.saturating_add(MICROCENTS_PER_CENT / 2) / MICROCENTS_PER_CENT;
    format!("${}.{:02}", cents / 100, cents % 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::opencode_dashboard::{
        DashboardClient, DashboardCredentials, DashboardResponse,
    };

    struct MockDashboardClient {
        response: DashboardResponse,
    }

    impl DashboardClient for MockDashboardClient {
        fn get(
            &self,
            _credentials: &DashboardCredentials,
            _page_path: &str,
        ) -> Result<DashboardResponse, crate::probe::ProbeError> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn returns_zen_balance_from_workspace_hydration() {
        let client = MockDashboardClient {
            response: DashboardResponse {
                status: 200,
                body: r#"<script>window._$HY={balance:-1392399000,reloadAmount:20,reloadTrigger:5,useBalance:true}</script>"#.into(),
                location: None,
            },
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let credentials_path = tmp.path().join("opencode-go.json");
        std::fs::write(
            &credentials_path,
            r#"{"workspaceId":"wrk_test","authCookie":"cookie"}"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&credentials_path, std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }

        let payload = run(&client, &credentials_path).unwrap();

        assert_eq!(payload.balance, -1_392_399_000);
        assert_eq!(payload.balance_formatted, "$13.92");
        assert!(payload.use_balance);
        assert_eq!(payload.reload_amount, 20);
        assert_eq!(payload.reload_trigger, 5);
    }

    #[test]
    fn ignores_similarly_named_hydration_fields() {
        let client = MockDashboardClient {
            response: DashboardResponse {
                status: 200,
                body: r#"window._$HY={"reloadAmountMin":10,"reloadAmount":20,"reloadTriggerMin":5,"reloadTrigger":7,"balance":-1392399000,"useBalance":true}"#.into(),
                location: None,
            },
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let credentials_path = tmp.path().join("opencode-go.json");
        std::fs::write(
            &credentials_path,
            r#"{"workspaceId":"wrk_test","authCookie":"cookie"}"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&credentials_path, std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }

        let payload = run(&client, &credentials_path).unwrap();

        assert_eq!(payload.reload_amount, 20);
        assert_eq!(payload.reload_trigger, 7);
    }

    #[test]
    fn ignores_balance_shaped_objects_outside_solid_hydration() {
        let client = MockDashboardClient {
            response: DashboardResponse {
                status: 200,
                body: r#"<script>const unrelated={balance:-9900000000,reloadAmount:99,reloadTrigger:99,useBalance:false}</script><script>window._$HY={balance:-1392399000,reloadAmount:20,reloadTrigger:5,useBalance:true}</script>"#.into(),
                location: None,
            },
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let credentials_path = tmp.path().join("opencode-go.json");
        std::fs::write(
            &credentials_path,
            r#"{"workspaceId":"wrk_test","authCookie":"cookie"}"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&credentials_path, std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }

        let payload = run(&client, &credentials_path).unwrap();

        assert_eq!(payload.balance, -1_392_399_000);
        assert_eq!(payload.reload_amount, 20);
        assert!(payload.use_balance);
    }

    #[test]
    fn surfaces_an_expired_dashboard_session() {
        let client = MockDashboardClient {
            response: DashboardResponse {
                status: 303,
                body: String::new(),
                location: Some("https://opencode.ai/login".into()),
            },
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let credentials_path = tmp.path().join("opencode-go.json");
        std::fs::write(
            &credentials_path,
            r#"{"workspaceId":"wrk_test","authCookie":"cookie"}"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&credentials_path, std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }

        let error = run(&client, &credentials_path).unwrap_err();

        assert!(
            matches!(error, ProbeError::SessionExpired(ref message) if message == SESSION_EXPIRED_MESSAGE)
        );
    }
}
