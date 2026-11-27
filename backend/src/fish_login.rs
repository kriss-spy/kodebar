//! Dedicated Fish Audio OAuth setup with PKCE and a loopback callback.
use crate::probe::fish_audio::{self, Credentials};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

fn random_string() -> Result<String, String> {
    let mut bytes = [0; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| "cannot generate OAuth random value")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub fn run() -> Result<(), String> {
    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "cannot create OAuth client")?;
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|_| "cannot start Fish Audio login callback")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "cannot configure Fish Audio callback")?;
    let redirect = format!(
        "http://127.0.0.1:{}/callback",
        listener
            .local_addr()
            .map_err(|_| "cannot read callback address")?
            .port()
    );
    let response = http.post("https://api.fish.audio/oauth/register").json(&json!({
        "client_name":"Kodebar", "redirect_uris":[redirect], "grant_types":["authorization_code","refresh_token"],
        "response_types":["code"], "token_endpoint_auth_method":"none"
    })).send().map_err(|_|"Fish Audio OAuth registration failed")?;
    let registration: Value = fish_audio::check_status(response)
        .map_err(|e| e.user_message())?
        .json()
        .map_err(|_| "invalid Fish Audio OAuth registration")?;
    let client_id = registration
        .get("client_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("Fish Audio did not return an OAuth client ID")?
        .to_owned();
    let verifier = random_string()?;
    let state = random_string()?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = reqwest::Url::parse("https://fish.audio/oauth/authorize").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", &client_id),
        ("redirect_uri", &redirect),
        ("scope", "mcp"),
        ("state", &state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("resource", "https://api.fish.audio/mcp"),
    ]);
    println!("Opening Fish Audio. Sign in, select your team, and authorize Kodebar.");
    webbrowser::open(url.as_str()).map_err(|_| "cannot open Fish Audio authorization page")?;
    let started = Instant::now();
    let code = loop {
        if started.elapsed() > Duration::from_secs(240) {
            return Err("Fish Audio login timed out".into());
        }
        match listener.accept() {
            Ok((mut socket, _)) => {
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .map_err(|_| "cannot read OAuth callback")?;
                let mut bytes = [0; 8192];
                let count = socket
                    .read(&mut bytes)
                    .map_err(|_| "cannot read OAuth callback")?;
                let request = String::from_utf8_lossy(&bytes[..count]);
                let target = request
                    .lines()
                    .next()
                    .and_then(|l| l.strip_prefix("GET "))
                    .and_then(|l| l.split(' ').next())
                    .unwrap_or("");
                let callback = reqwest::Url::parse(&format!("http://127.0.0.1{target}"));
                let code = callback.ok().and_then(|u| callback_code(&u, &state));
                let message = if code.is_some() {
                    "Fish Audio authorization received. Return to Kodebar."
                } else {
                    "Invalid OAuth callback."
                };
                let status = if code.is_some() {
                    "200 OK"
                } else {
                    "400 Bad Request"
                };
                let _ = write!(
                    socket,
                    "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{message}",
                    message.len()
                );
                if let Some(code) = code {
                    break code;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(_) => return Err("Fish Audio callback failed".into()),
        }
    };
    let response = http
        .post("https://api.fish.audio/oauth/token")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("code_verifier", verifier.as_str()),
            ("resource", "https://api.fish.audio/mcp"),
        ])
        .send()
        .map_err(|_| "Fish Audio token exchange failed")?;
    let token = fish_audio::check_status(response)
        .map_err(|e| e.user_message())?
        .json()
        .map_err(|_| "invalid Fish Audio token exchange response")?;
    let mut credentials: Credentials =
        fish_audio::token_credentials(token, client_id, String::new(), String::new())
            .map_err(|e| e.user_message())?;
    let balance =
        fish_audio::credits(&http, &credentials.access_token).map_err(|e| e.user_message())?;
    credentials.team_id = balance.team_id;
    fish_audio::save_credentials(
        &fish_audio::credential_path().map_err(|e| e.user_message())?,
        &credentials,
    )
    .map_err(|e| e.user_message())?;
    println!("Fish Audio login saved and subscription credits verified.");
    Ok(())
}

fn callback_code(url: &reqwest::Url, expected_state: &str) -> Option<String> {
    if url.path() != "/callback" {
        return None;
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    if pairs.iter().filter(|(k, _)| k == "state").count() != 1
        || pairs.iter().filter(|(k, _)| k == "code").count() != 1
    {
        return None;
    }
    if pairs
        .iter()
        .any(|(k, v)| k == "error" || (k == "iss" && v != "https://api.fish.audio"))
    {
        return None;
    }
    if !pairs
        .iter()
        .any(|(k, v)| k == "state" && v == expected_state)
    {
        return None;
    }
    pairs
        .iter()
        .find(|(k, v)| k == "code" && !v.is_empty())
        .map(|(_, v)| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_requires_matching_state_and_issuer() {
        let parse = |query: &str| {
            reqwest::Url::parse(&format!("http://127.0.0.1/callback?{query}")).unwrap()
        };
        assert_eq!(
            callback_code(&parse("state=expected&code=test"), "expected"),
            Some("test".into())
        );
        for q in [
            "state=wrong&code=test",
            "state=expected&code=test&iss=https://evil.example",
            "state=expected&code=a&code=b",
        ] {
            assert_eq!(callback_code(&parse(q), "expected"), None);
        }
    }
}
