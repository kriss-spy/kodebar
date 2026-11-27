//! Shared X subscription usage through the Grok CLI billing API.
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::ProbeError;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

const USAGE_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XPremiumPayload {
    #[serde(rename = "type")]
    kind: String,
    windows: BTreeMap<String, Window>,
    products: Vec<Product>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Window {
    usage_percent: f64,
    reset_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Product {
    product: String,
    usage_percent: f64,
}

impl XPremiumPayload {
    pub fn empty() -> Self {
        Self {
            kind: "quota-based".into(),
            windows: BTreeMap::new(),
            products: vec![],
        }
    }
}

#[derive(Deserialize)]
struct Billing {
    config: Credits,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Credits {
    credit_usage_percent: f64,
    current_period: Period,
    #[serde(default)]
    product_usage: Vec<Product>,
}
#[derive(Deserialize)]
struct Period {
    start: String,
    end: String,
    #[serde(rename = "type")]
    kind: String,
}

pub fn parse_usage(body: &str) -> Result<XPremiumPayload, ProbeError> {
    let invalid = || ProbeError::Parse("X Premium billing response is invalid".into());
    let billing: Billing = serde_json::from_str(body).map_err(|_| invalid())?;
    let config = billing.config;
    let start =
        DateTime::parse_from_rfc3339(&config.current_period.start).map_err(|_| invalid())?;
    let end = DateTime::parse_from_rfc3339(&config.current_period.end).map_err(|_| invalid())?;
    if config.current_period.kind != "USAGE_PERIOD_TYPE_WEEKLY"
        || end <= start
        || !config.credit_usage_percent.is_finite()
        || config.credit_usage_percent < 0.0
    {
        return Err(invalid());
    }
    let mut payload = XPremiumPayload::empty();
    payload.windows.insert(
        "weekly".into(),
        Window {
            usage_percent: config.credit_usage_percent.min(100.0),
            reset_at: config.current_period.end,
        },
    );
    // Breakdown entries are shares of the same quota, never separate limits.
    let mut seen = std::collections::BTreeSet::new();
    let valid = config.product_usage.iter().all(|p| {
        p.usage_percent.is_finite() && p.usage_percent >= 0.0 && seen.insert(p.product.clone())
    });
    let sum: f64 = config.product_usage.iter().map(|p| p.usage_percent).sum();
    if valid && (sum - config.credit_usage_percent).abs() <= 1.0 {
        payload.products = config
            .product_usage
            .into_iter()
            .map(|mut p| {
                p.product = match p.product.as_str() {
                    "GrokBot" => "grok-bot",
                    "GrokChat" => "grok-chat",
                    "Cursor" => "cursor",
                    other => other,
                }
                .to_owned();
                p
            })
            .collect();
    }
    Ok(payload)
}

fn auth_path() -> Result<PathBuf, ProbeError> {
    let root = std::env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(".grok"))
        })
        .ok_or_else(|| ProbeError::Io("cannot locate Grok credentials".into()))?;
    Ok(root.join("auth.json"))
}

#[derive(Deserialize)]
struct Credentials {
    key: String,
    expires_at: Option<String>,
}

fn read_token(path: &std::path::Path) -> Result<String, ProbeError> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ProbeError::NoCredentials("Run grok login to connect X Premium".into())
        } else {
            ProbeError::Io("cannot inspect Grok credentials".into())
        }
    })?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o777 != 0o600 {
        return Err(ProbeError::Io(
            "Grok auth.json must have 0600 permissions".into(),
        ));
    }
    let text = std::fs::read_to_string(path)
        .map_err(|_| ProbeError::Io("cannot read Grok credentials".into()))?;
    let mut entries: BTreeMap<String, serde_json::Value> = serde_json::from_str(&text)
        .map_err(|_| ProbeError::Parse("invalid Grok credential file".into()))?;
    entries.retain(|scope, _| scope.starts_with("https://auth.x.ai::"));
    if entries.len() != 1 {
        return Err(ProbeError::InvalidCredentials(
            "Grok login must contain one OIDC account; run grok login".into(),
        ));
    }
    let credentials: Credentials = serde_json::from_value(entries.into_values().next().unwrap())
        .map_err(|_| ProbeError::Parse("invalid Grok login credentials".into()))?;
    if credentials.key.trim().is_empty() {
        return Err(ProbeError::InvalidCredentials(
            "Grok access token is empty; run grok login".into(),
        ));
    }
    if let Some(expiry) = credentials.expires_at {
        let expiry = DateTime::parse_from_rfc3339(&expiry)
            .map_err(|_| ProbeError::Parse("invalid Grok token expiry".into()))?;
        if expiry <= chrono::Utc::now() {
            return Err(ProbeError::InvalidCredentials(
                "Grok access token expired; run grok login".into(),
            ));
        }
    }
    Ok(credentials.key)
}

pub fn run() -> Result<XPremiumPayload, ProbeError> {
    let token = read_token(&auth_path()?)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ProbeError::Io("cannot create X Premium client".into()))?;
    let response = super::transport::send_read(
        client
            .get(USAGE_URL)
            .bearer_auth(token)
            .header("x-xai-token-auth", "xai-grok-cli")
            .header(reqwest::header::ACCEPT, "application/json"),
    )
    .map_err(|_| ProbeError::Io("X Premium billing request failed".into()))?;
    match response.status().as_u16() {
        200 => parse_usage(
            &response
                .text()
                .map_err(|_| ProbeError::Parse("cannot read X Premium billing response".into()))?,
        ),
        401 => Err(ProbeError::InvalidCredentials(
            "Grok login was rejected; run grok login".into(),
        )),
        403 => Err(ProbeError::Unsupported(
            "Grok billing access denied for this account".into(),
        )),
        429 => Err(ProbeError::RateLimited),
        status => Err(ProbeError::Http {
            status,
            body: String::new(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = r#"{"config":{"creditUsagePercent":6.0,"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","start":"2026-11-24T12:48:28.973+00:00","end":"2026-12-01T12:48:28.973+00:00"},"productUsage":[{"product":"GrokBot","usagePercent":5},{"product":"GrokChat","usagePercent":1}]}}"#;
    #[test]
    fn reads_published_shared_quota_and_product_shares() {
        let payload = parse_usage(FIXTURE).unwrap();
        assert_eq!(payload.windows["weekly"].usage_percent, 6.0);
        assert_eq!(payload.products[0].product, "grok-bot");
        assert_eq!(payload.products[1].usage_percent, 1.0);
    }
    #[test]
    fn missing_quota_is_not_inferred_from_money_or_reset_time() {
        for body in [
            "{}".to_owned(),
            FIXTURE.replace("\"creditUsagePercent\":6.0,", ""),
            FIXTURE.replace("2026-12-01", "2026-11-01"),
        ] {
            assert!(parse_usage(&body).is_err());
        }
    }
    #[test]
    fn unrelated_breakdown_does_not_override_shared_quota() {
        let payload =
            parse_usage(&FIXTURE.replace("\"usagePercent\":5", "\"usagePercent\":50")).unwrap();
        assert_eq!(payload.windows["weekly"].usage_percent, 6.0);
        assert!(payload.products.is_empty());
    }
    #[test]
    fn reads_existing_grok_login_without_writing_it() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("auth.json");
        let contents = r#"{"https://auth.x.ai::test":{"key":"test-token","expires_at":"2099-01-01T00:00:00Z"}}"#;
        std::fs::write(&path, contents).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(read_token(&path).unwrap(), "test-token");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), contents);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_token(&path).is_err());
    }
}
