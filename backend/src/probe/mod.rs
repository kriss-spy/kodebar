//! Provider probes — each probe reads a Credential File, optionally performs
//! Token Refresh, calls the provider's API, and returns quota/balance data.
//!
//! See `backend/CONTEXT.md` for the ubiquitous language (Probe, Provider,
//! Snapshot, Stale, Quota Window, Credential File, Token Refresh).

pub mod antigravity;
pub mod opencode_auth;
pub mod opencode_dashboard;
pub mod opencode_go;
pub mod opencode_zen;

pub use antigravity::CodeAssistClient;

use serde_json::Value;

/// The common error type for all probes.
///
/// `NoCredentials` means the provider is not configured on this machine —
/// the orchestration omits the provider from the Snapshot rather than
/// rendering a stale entry. Every other variant means the provider *is*
/// configured but this Probe failed: the orchestration serves last-known-good
/// from the prior Snapshot, flagged `stale: true` (PRD §7.1).
#[allow(dead_code)] // variant payloads are surfaced via Debug, not always read.
#[derive(Debug)]
pub enum ProbeError {
    /// The Credential File is absent or unreadable. The provider is simply
    /// not configured — do not render a stale entry.
    NoCredentials(String),
    /// The OAuth refresh token itself is invalid or revoked. This is the one
    /// auth error that is user-facing (PRD §7.7).
    InvalidRefreshToken(String),
    /// A configured API key or access token was rejected.
    InvalidCredentials(String),
    /// `retrieveUserQuota` (or an equivalent quota endpoint) returned 429.
    /// Back off and serve stale data (PRD §5.1 gotcha, §7.4).
    RateLimited,
    /// Any other HTTP failure from the provider API.
    Http { status: u16, body: String },
    /// The response did not match the documented shape.
    Parse(String),
    /// A browser-backed dashboard rejected or redirected the saved session.
    SessionExpired(String),
    /// A local I/O failure (reading the Credential File, writing back a
    /// refreshed token, …).
    Io(String),
}

impl ProbeError {
    /// Render an actionable message without including response bodies or credentials.
    pub fn user_message(&self) -> String {
        match self {
            Self::NoCredentials(message)
            | Self::InvalidRefreshToken(message)
            | Self::InvalidCredentials(message)
            | Self::Parse(message)
            | Self::SessionExpired(message)
            | Self::Io(message) => message.clone(),
            Self::RateLimited => "provider rate limited the Probe".into(),
            Self::Http { status, .. } => format!("provider returned HTTP {status}"),
        }
    }
}

/// An HTTP-level response captured for the probe logic to inspect (notably
/// the status code, for 429 detection).
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Value,
}

impl HttpResponse {
    pub fn is_rate_limited(&self) -> bool {
        self.status == 429
    }
}
