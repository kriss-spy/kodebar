//! Fish Audio web subscription credits through its OAuth-authenticated MCP server.
use super::ProbeError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MCP_URL: &str = "https://api.fish.audio/mcp";
const TOKEN_URL: &str = "https://api.fish.audio/oauth/token";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FishAudioPayload {
    #[serde(rename = "type")]
    kind: String,
    pub team_id: String,
    pub team_name: String,
    pub plan: String,
    pub credits_remaining: u64,
    pub extra_credits: u64,
    pub reset_at: Option<String>,
}
impl FishAudioPayload {
    pub fn empty() -> Self {
        Self {
            kind: "credit-balance".into(),
            team_id: String::new(),
            team_name: String::new(),
            plan: String::new(),
            credits_remaining: 0,
            extra_credits: 0,
            reset_at: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub client_id: String,
    pub expires_at: i64,
    pub team_id: String,
}

pub fn credential_path() -> Result<PathBuf, ProbeError> {
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(".config"))
        })
        .ok_or_else(|| ProbeError::Io("cannot locate Fish Audio credentials".into()))?;
    Ok(root.join("kodebar/fish-audio.json"))
}

pub fn save_credentials(path: &Path, credentials: &Credentials) -> Result<(), ProbeError> {
    use std::{
        io::Write,
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
    };
    let parent = path
        .parent()
        .ok_or_else(|| ProbeError::Io("invalid Fish Audio credential path".into()))?;
    fs::create_dir_all(parent)
        .map_err(|_| ProbeError::Io("cannot create Fish Audio credential directory".into()))?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .map_err(|_| ProbeError::Io("cannot protect Fish Audio credential directory".into()))?;
    let temporary = parent.join(format!(".fish-audio-{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| ProbeError::Io("cannot create Fish Audio credential file".into()))?;
        serde_json::to_writer(&mut file, credentials)
            .map_err(|_| ProbeError::Io("cannot encode Fish Audio credentials".into()))?;
        file.flush()
            .and_then(|_| file.sync_all())
            .map_err(|_| ProbeError::Io("cannot write Fish Audio credentials".into()))?;
        fs::rename(&temporary, path)
            .map_err(|_| ProbeError::Io("cannot save Fish Audio credentials".into()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

pub fn http_client() -> Result<reqwest::blocking::Client, ProbeError> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ProbeError::Io("cannot create Fish Audio HTTP client".into()))
}

pub fn check_status(
    response: reqwest::blocking::Response,
) -> Result<reqwest::blocking::Response, ProbeError> {
    match response.status().as_u16() {
        200..=299 => Ok(response),
        401 => Err(ProbeError::InvalidCredentials(
            "Fish Audio authorization expired; run kodebar login fish-audio".into(),
        )),
        403 => Err(ProbeError::Unsupported(
            "Fish Audio denied access to this team".into(),
        )),
        429 => Err(ProbeError::RateLimited),
        status => Err(ProbeError::Http {
            status,
            body: String::new(),
        }),
    }
}

pub fn token_credentials(
    value: Value,
    client_id: String,
    previous_refresh: String,
    team_id: String,
) -> Result<Credentials, ProbeError> {
    #[derive(Deserialize)]
    struct Token {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: u32,
    }
    let token: Token = serde_json::from_value(value)
        .map_err(|_| ProbeError::Parse("invalid Fish Audio token response".into()))?;
    let refresh_token = token.refresh_token.unwrap_or(previous_refresh);
    if token.access_token.is_empty() || refresh_token.is_empty() || token.expires_in == 0 {
        return Err(ProbeError::Parse(
            "incomplete Fish Audio token response".into(),
        ));
    }
    Ok(Credentials {
        access_token: token.access_token,
        refresh_token,
        client_id,
        expires_at: Utc::now().timestamp() + i64::from(token.expires_in),
        team_id,
    })
}

pub fn run() -> Result<FishAudioPayload, ProbeError> {
    use std::os::unix::fs::PermissionsExt;
    let path = credential_path()?;
    let metadata = fs::metadata(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ProbeError::NoCredentials("Run kodebar login fish-audio to connect Fish Audio".into())
        } else {
            ProbeError::Io("cannot inspect Fish Audio credentials".into())
        }
    })?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o777 != 0o600 {
        return Err(ProbeError::Io(
            "Fish Audio credential file must have 0600 permissions".into(),
        ));
    }
    let mut credentials: Credentials = serde_json::from_slice(
        &fs::read(&path)
            .map_err(|_| ProbeError::Io("cannot read Fish Audio credentials".into()))?,
    )
    .map_err(|_| ProbeError::Parse("invalid Fish Audio credential file".into()))?;
    let http = http_client()?;
    if credentials.expires_at <= Utc::now().timestamp() + 60 {
        let response = http
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", credentials.refresh_token.as_str()),
                ("client_id", credentials.client_id.as_str()),
            ])
            .send()
            .map_err(|_| ProbeError::Io("Fish Audio token refresh failed".into()))?;
        let value = check_status(response)?
            .json()
            .map_err(|_| ProbeError::Parse("invalid Fish Audio token response".into()))?;
        credentials = token_credentials(
            value,
            credentials.client_id,
            credentials.refresh_token,
            credentials.team_id,
        )?;
        save_credentials(&path, &credentials)?;
    }
    let payload = credits(&http, &credentials.access_token)?;
    if payload.team_id != credentials.team_id {
        return Err(ProbeError::Unsupported(
            "Fish Audio team changed; run kodebar login fish-audio to select a team".into(),
        ));
    }
    Ok(payload)
}

fn rpc_json(body: &str, id: u64) -> Result<Value, ProbeError> {
    let parse = |text: &str| {
        serde_json::from_str::<Value>(text)
            .ok()
            .filter(|v| v.get("id").and_then(Value::as_u64) == Some(id))
    };
    let value = parse(body)
        .or_else(|| {
            body.lines()
                .filter_map(|l| l.strip_prefix("data:"))
                .find_map(|l| parse(l.trim()))
        })
        .ok_or_else(|| ProbeError::Parse("invalid Fish Audio MCP response".into()))?;
    if value.get("error").is_some() {
        return Err(ProbeError::Parse("Fish Audio MCP request failed".into()));
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| ProbeError::Parse("missing Fish Audio MCP result".into()))
}

pub fn credits(
    http: &reqwest::blocking::Client,
    access_token: &str,
) -> Result<FishAudioPayload, ProbeError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(35);
    let post = |body: Value, session: Option<&str>| {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(ProbeError::Io("Fish Audio credit request timed out".into()));
        }
        let mut request = http
            .post(MCP_URL)
            .timeout(remaining)
            .bearer_auth(access_token)
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2025-03-26")
            .json(&body);
        if let Some(session) = session {
            request = request.header("Mcp-Session-Id", session);
        }
        check_status(
            super::transport::send_read_until(request, deadline).map_err(|error| {
                ProbeError::Io(
                    if error.is_timeout() {
                        "Fish Audio credit request timed out"
                    } else {
                        "Fish Audio credit connection failed"
                    }
                    .into(),
                )
            })?,
        )
    };
    let response = post(
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
        "params":{"name":"get_credit_balance","arguments":{}}}),
        None,
    )?;
    let result = rpc_json(
        &response
            .text()
            .map_err(|_| ProbeError::Parse("cannot read Fish Audio credits".into()))?,
        2,
    )?;
    parse_credits(&result)
}

fn parse_credits(result: &Value) -> Result<FishAudioPayload, ProbeError> {
    let invalid = || ProbeError::Parse("invalid Fish Audio subscription credits".into());
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        return Err(invalid());
    }
    let data = result
        .get("structuredContent")
        .cloned()
        .or_else(|| {
            result
                .get("content")?
                .as_array()?
                .iter()
                .filter_map(|c| c.get("text")?.as_str())
                .find_map(|t| serde_json::from_str::<Value>(t).ok())
        })
        .ok_or_else(invalid)?;
    #[derive(Deserialize)]
    struct Balance {
        team_id: String,
        team_name: String,
        plan: String,
        package_balance: u64,
        package_extra_balance: u64,
        package_cycle_ends_at: Option<String>,
    }
    let balance: Balance = serde_json::from_value(data).map_err(|_| invalid())?;
    if balance.team_id.is_empty() {
        return Err(invalid());
    }
    if let Some(reset) = &balance.package_cycle_ends_at {
        DateTime::parse_from_rfc3339(reset).map_err(|_| invalid())?;
    }
    Ok(FishAudioPayload {
        kind: "credit-balance".into(),
        team_id: balance.team_id,
        team_name: balance.team_name,
        plan: balance.plan,
        credits_remaining: balance.package_balance,
        extra_credits: balance.package_extra_balance,
        reset_at: balance.package_cycle_ends_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subscription_credits_are_separate_from_api_dollars() {
        let result = json!({"structuredContent":{"team_id":"team","team_name":"My Team","plan":"plus",
            "package_balance":250555,"package_extra_balance":1000,"package_cycle_ends_at":"2026-12-27T12:00:30Z","api_credit_usd":"5.0"}});
        let p = parse_credits(&result).unwrap();
        assert_eq!(p.credits_remaining, 250555);
        assert_eq!(p.extra_credits, 1000);
        assert!(!serde_json::to_string(&p).unwrap().contains("api_credit"));
        assert!(
            parse_credits(&json!({"isError":true,"structuredContent":result["structuredContent"]}))
                .is_err()
        );
        assert!(
            parse_credits(&json!({"structuredContent":{"team_id":"team","api_credit_usd":"5.0"}}))
                .is_err()
        );
    }
    #[test]
    fn sse_selects_matching_response_and_rejects_errors() {
        assert_eq!(
            rpc_json(
                "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"ok\":true}}\n\n",
                2
            )
            .unwrap()["ok"],
            true
        );
        assert!(rpc_json("{\"id\":1,\"result\":{}}", 2).is_err());
        assert!(rpc_json("{\"id\":2,\"error\":{\"message\":\"secret\"}}", 2).is_err());
    }
}
