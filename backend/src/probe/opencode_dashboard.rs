//! Shared credential and HTTP boundary for OpenCode dashboard Probes.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::ProbeError;

const DASHBOARD_BASE: &str = "https://opencode.ai";
pub const SESSION_EXPIRED_MESSAGE: &str = "session expired — re-login at opencode.ai";

#[derive(Clone, PartialEq, Eq)]
pub struct DashboardCredentials {
    workspace_id: String,
    auth_cookie: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialFile {
    workspace_id: String,
    auth_cookie: String,
}

impl DashboardCredentials {
    pub fn load(path: &Path) -> Result<Self, ProbeError> {
        Self::load_with_overrides(
            path,
            std::env::var("OPENCODE_GO_WORKSPACE_ID").ok(),
            std::env::var("OPENCODE_GO_AUTH_COOKIE").ok(),
        )
    }

    pub fn load_with_overrides(
        path: &Path,
        workspace_id: Option<String>,
        auth_cookie: Option<String>,
    ) -> Result<Self, ProbeError> {
        let file = if workspace_id.is_none() || auth_cookie.is_none() {
            Some(read_credential_file(path)?)
        } else {
            None
        };
        let workspace_id = workspace_id
            .or_else(|| file.as_ref().map(|f| f.workspace_id.clone()))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ProbeError::Parse("OpenCode workspace ID is empty".into()))?;
        let auth_cookie = auth_cookie
            .or_else(|| file.map(|f| f.auth_cookie))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ProbeError::Parse("OpenCode auth cookie is empty".into()))?;
        Ok(Self {
            workspace_id,
            auth_cookie,
        })
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub fn auth_cookie(&self) -> &str {
        &self.auth_cookie
    }
}

fn read_credential_file(path: &Path) -> Result<CredentialFile, ProbeError> {
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
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err(ProbeError::Io(format!(
                "{} must have 0600 permissions",
                path.display()
            )));
        }
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| ProbeError::Io(format!("failed to read {}: {error}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|error| ProbeError::Parse(format!("failed to parse {}: {error}", path.display())))
}

pub fn default_credentials_path() -> PathBuf {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("kodebar/opencode-go.json");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/kodebar/opencode-go.json")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardResponse {
    pub status: u16,
    pub body: String,
    pub location: Option<String>,
}

impl DashboardResponse {
    pub fn is_login_redirect(&self) -> bool {
        (300..400).contains(&self.status)
            && self.location.as_deref().is_some_and(|location| {
                let location = location.to_ascii_lowercase();
                location.contains("login")
                    || location.contains("sign-in")
                    || location.contains("signin")
                    || location.contains("auth")
            })
    }
}

pub trait DashboardClient {
    fn get(
        &self,
        credentials: &DashboardCredentials,
        page_path: &str,
    ) -> Result<DashboardResponse, ProbeError>;
}

pub struct ReqwestDashboardClient {
    http: reqwest::blocking::Client,
}

impl ReqwestDashboardClient {
    pub fn new() -> Result<Self, ProbeError> {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| ProbeError::Io(format!("failed to build HTTP client: {e}")))?;
        Ok(Self { http })
    }
}

impl DashboardClient for ReqwestDashboardClient {
    fn get(
        &self,
        credentials: &DashboardCredentials,
        page_path: &str,
    ) -> Result<DashboardResponse, ProbeError> {
        let page_path = page_path.trim_start_matches('/');
        let response = self
            .http
            .get(format!("{DASHBOARD_BASE}/{page_path}"))
            .header(
                reqwest::header::COOKIE,
                format!("auth={}", credentials.auth_cookie()),
            )
            .send()
            .map_err(|e| ProbeError::Io(format!("OpenCode dashboard request failed: {e}")))?;
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response
            .text()
            .map_err(|e| ProbeError::Io(format!("failed to read dashboard response: {e}")))?;
        Ok(DashboardResponse {
            status,
            body,
            location,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_dashboard_credentials_from_a_private_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("opencode-go.json");
        std::fs::write(
            &path,
            r#"{"workspaceId":"wrk_file","authCookie":"cookie-file"}"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }

        let credentials = DashboardCredentials::load_with_overrides(&path, None, None).unwrap();

        assert_eq!(credentials.workspace_id(), "wrk_file");
        assert_eq!(credentials.auth_cookie(), "cookie-file");
    }

    #[test]
    fn environment_values_take_precedence_without_a_file() {
        let missing = Path::new("/definitely/missing/opencode-go.json");
        let credentials = DashboardCredentials::load_with_overrides(
            missing,
            Some("wrk_env".into()),
            Some("cookie-env".into()),
        )
        .unwrap();

        assert_eq!(credentials.workspace_id(), "wrk_env");
        assert_eq!(credentials.auth_cookie(), "cookie-env");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_credential_file_that_is_not_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("opencode-go.json");
        std::fs::write(&path, r#"{"workspaceId":"wrk","authCookie":"cookie"}"#).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let error = match DashboardCredentials::load_with_overrides(&path, None, None) {
            Ok(_) => panic!("insecure credential file should be rejected"),
            Err(error) => error,
        };

        assert!(matches!(error, ProbeError::Io(message) if message.contains("0600")));
    }
}
