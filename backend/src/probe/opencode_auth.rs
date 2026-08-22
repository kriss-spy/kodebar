//! OpenCode API-key discovery and persistence.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::ProbeError;

const PROVIDER_IDS: [&str; 2] = ["opencode", "opencode-go"];

#[derive(Clone, PartialEq, Eq)]
struct ApiKey(String);

impl ApiKey {
    fn new(value: &str) -> Option<Self> {
        let value = value.trim();
        (!value.is_empty()).then(|| Self(value.to_owned()))
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ApiKey([REDACTED])")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenCodeCredentials {
    api_key: ApiKey,
}

impl OpenCodeCredentials {
    pub fn load_with_override(path: &Path, api_key: Option<String>) -> Result<Self, ProbeError> {
        if let Some(api_key) = api_key.as_deref().and_then(ApiKey::new) {
            return Ok(Self { api_key });
        }

        let text = std::fs::read_to_string(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ProbeError::NoCredentials(format!("{} does not exist", path.display()))
            } else {
                ProbeError::Io(format!("failed to read {}: {error}", path.display()))
            }
        })?;
        let root: Value = serde_json::from_str(&text).map_err(|error| {
            ProbeError::Parse(format!("failed to parse {}: {error}", path.display()))
        })?;
        let api_key = PROVIDER_IDS
            .iter()
            .find_map(|provider| {
                let entry = root.get(provider)?;
                (entry.get("type")?.as_str()? == "api")
                    .then(|| entry.get("key")?.as_str())
                    .flatten()
                    .and_then(ApiKey::new)
            })
            .ok_or_else(|| {
                ProbeError::NoCredentials(format!(
                    "{} has no OpenCode API key; run `kodebar login opencode`",
                    path.display()
                ))
            })?;
        Ok(Self { api_key })
    }

    pub fn api_key(&self) -> &str {
        self.api_key.expose()
    }
}

pub fn default_auth_path() -> Result<PathBuf, ProbeError> {
    default_auth_path_with_env(
        std::env::var("XDG_DATA_HOME").ok(),
        std::env::var("HOME").ok(),
    )
}

fn default_auth_path_with_env(
    data_home: Option<String>,
    home: Option<String>,
) -> Result<PathBuf, ProbeError> {
    if let Some(data_home) = data_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Ok(data_home.join("opencode/auth.json"));
    }
    if let Some(home) = home.map(PathBuf::from).filter(|path| path.is_absolute()) {
        return Ok(home.join(".local/share/opencode/auth.json"));
    }
    Err(ProbeError::Io(
        "cannot locate OpenCode auth.json: XDG_DATA_HOME and HOME are not absolute paths".into(),
    ))
}

pub fn save_api_key(path: &Path, api_key: &str) -> Result<(), ProbeError> {
    let api_key = ApiKey::new(api_key)
        .ok_or_else(|| ProbeError::Parse("OpenCode API key is empty".into()))?;
    let mut root = match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str::<Map<String, Value>>(&text).map_err(|error| {
            ProbeError::Parse(format!("failed to parse {}: {error}", path.display()))
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(error) => {
            return Err(ProbeError::Io(format!(
                "failed to read {}: {error}",
                path.display()
            )));
        }
    };
    root.insert(
        "opencode".into(),
        json!({"type": "api", "key": api_key.expose()}),
    );

    let parent = path.parent().ok_or_else(|| {
        ProbeError::Io(format!("credential path {} has no parent", path.display()))
    })?;
    std::fs::create_dir_all(parent).map_err(|error| {
        ProbeError::Io(format!("failed to create {}: {error}", parent.display()))
    })?;
    let encoded = serde_json::to_string_pretty(&root)
        .map_err(|error| ProbeError::Parse(format!("failed to encode credentials: {error}")))?;
    let temporary = path.with_extension(format!("json.tmp.{}", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|error| {
        ProbeError::Io(format!("failed to create {}: {error}", temporary.display()))
    })?;
    use std::io::Write;
    if let Err(error) = file
        .write_all(encoded.as_bytes())
        .and_then(|_| file.sync_all())
    {
        drop(file);
        let _ = std::fs::remove_file(&temporary);
        return Err(ProbeError::Io(format!(
            "failed to write {}: {error}",
            temporary.display()
        )));
    }
    drop(file);
    std::fs::rename(&temporary, path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        ProbeError::Io(format!("failed to install {}: {error}", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_the_supported_opencode_provider_key() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("auth.json");
        std::fs::write(
            &path,
            r#"{"google":{"type":"api","key":"google-secret"},"opencode":{"type":"api","key":"oc-secret"}}"#,
        )
        .unwrap();

        let credentials = OpenCodeCredentials::load_with_override(&path, None).unwrap();

        assert_eq!(credentials.api_key(), "oc-secret");
    }

    #[test]
    fn accepts_the_legacy_opencode_go_provider_key() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("auth.json");
        std::fs::write(&path, r#"{"opencode-go":{"type":"api","key":"go-secret"}}"#).unwrap();

        let credentials = OpenCodeCredentials::load_with_override(&path, None).unwrap();

        assert_eq!(credentials.api_key(), "go-secret");
    }

    #[test]
    fn saves_the_key_without_overwriting_other_provider_credentials() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("nested/auth.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"google":{"type":"api","key":"keep-me"}}"#).unwrap();

        save_api_key(&path, "new-secret").unwrap();

        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["google"]["key"], "keep-me");
        assert_eq!(value["opencode"]["type"], "api");
        assert_eq!(value["opencode"]["key"], "new-secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn refuses_to_put_credentials_in_the_working_directory() {
        let error = default_auth_path_with_env(None, None).unwrap_err();

        assert!(matches!(error, ProbeError::Io(message) if message.contains("absolute")));
    }

    #[test]
    fn ignores_a_relative_xdg_data_home_when_home_is_safe() {
        let path =
            default_auth_path_with_env(Some("relative".into()), Some("/home/test".into())).unwrap();

        assert_eq!(
            path,
            Path::new("/home/test/.local/share/opencode/auth.json")
        );
    }

    #[test]
    fn credential_debug_output_is_redacted() {
        let credentials = OpenCodeCredentials {
            api_key: ApiKey::new("super-secret").unwrap(),
        };

        let debug = format!("{credentials:?}");

        assert!(!debug.contains("super-secret"));
        assert!(debug.contains("REDACTED"));
    }
}
