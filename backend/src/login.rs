//! Guided interactive login flows.

use std::path::Path;

use crate::probe::ProbeError;
use crate::probe::opencode_auth::save_api_key;
use crate::probe::opencode_go::{OpenCodeGoClient, validate_api_key};

pub const OPENCODE_KEY_URL: &str = "https://opencode.ai/auth";

pub trait BrowserOpener {
    fn open(&self, url: &str) -> Result<(), String>;
}

pub trait SecretReader {
    fn read_api_key(&self) -> Result<String, String>;
}

pub struct SystemBrowser;

impl BrowserOpener for SystemBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        webbrowser::open(url).map_err(|error| format!("failed to open browser: {error}"))
    }
}

pub struct TerminalSecretReader;

impl SecretReader for TerminalSecretReader {
    fn read_api_key(&self) -> Result<String, String> {
        rpassword::prompt_password("Paste the OpenCode API key (input hidden): ")
            .map_err(|error| format!("failed to read API key: {error}"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginOutcome {
    ActiveGoSubscription,
    NoGoSubscription,
}

pub fn guided_opencode_login<B, R, C>(
    browser: &B,
    reader: &R,
    client: &C,
    auth_path: &Path,
) -> Result<LoginOutcome, String>
where
    B: BrowserOpener,
    R: SecretReader,
    C: OpenCodeGoClient,
{
    browser.open(OPENCODE_KEY_URL)?;
    let api_key = reader.read_api_key()?;
    let has_go = validate_api_key(client, api_key.trim()).map_err(format_probe_error)?;
    save_api_key(auth_path, api_key.trim()).map_err(format_probe_error)?;
    Ok(if has_go {
        LoginOutcome::ActiveGoSubscription
    } else {
        LoginOutcome::NoGoSubscription
    })
}

fn format_probe_error(error: ProbeError) -> String {
    error.user_message()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::ProbeError;
    use crate::probe::opencode_go::{GoApiResponse, OpenCodeGoClient};
    use std::cell::RefCell;

    struct Browser<'a>(&'a RefCell<Vec<&'static str>>);
    impl BrowserOpener for Browser<'_> {
        fn open(&self, url: &str) -> Result<(), String> {
            assert_eq!(url, OPENCODE_KEY_URL);
            self.0.borrow_mut().push("browser");
            Ok(())
        }
    }

    struct Reader<'a>(&'a RefCell<Vec<&'static str>>);
    impl SecretReader for Reader<'_> {
        fn read_api_key(&self) -> Result<String, String> {
            self.0.borrow_mut().push("prompt");
            Ok("oc-secret".into())
        }
    }

    struct Client<'a>(&'a RefCell<Vec<&'static str>>, u16);
    impl OpenCodeGoClient for Client<'_> {
        fn get_usage(&self, api_key: &str) -> Result<GoApiResponse, ProbeError> {
            assert_eq!(api_key, "oc-secret");
            self.0.borrow_mut().push("validate");
            Ok(GoApiResponse {
                status: self.1,
                body: if self.1 == 403 {
                    r#"{"type":"error","error":{"type":"EntitlementError"}}"#.into()
                } else {
                    String::new()
                },
            })
        }
    }

    #[test]
    fn guided_login_opens_browser_prompts_validates_then_saves() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("auth.json");
        let events = RefCell::new(Vec::new());

        let outcome = guided_opencode_login(
            &Browser(&events),
            &Reader(&events),
            &Client(&events, 403),
            &path,
        )
        .unwrap();

        assert_eq!(events.into_inner(), ["browser", "prompt", "validate"]);
        assert_eq!(outcome, LoginOutcome::NoGoSubscription);
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(value["opencode"]["key"], "oc-secret");
    }

    #[test]
    fn rejected_key_is_not_saved() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("auth.json");
        let events = RefCell::new(Vec::new());

        let error = guided_opencode_login(
            &Browser(&events),
            &Reader(&events),
            &Client(&events, 401),
            &path,
        )
        .unwrap_err();

        assert!(error.contains("rejected"));
        assert!(!path.exists());
    }
}
