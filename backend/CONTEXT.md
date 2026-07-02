# Backend

A native Linux service (Rust) that probes AI provider quota/cost APIs directly, reading on-disk credentials that OpenCode and Gemini CLI / Antigravity already write. No upstream CLI dependency, no subprocess spawning.

## Language

**Probe**:
A single provider API call (or pair of calls) that fetches current quota or balance data. Each probe is independent and isolated — one failing must not block others.
_Avoid_: fetch, poll, request, call

**Provider**:
An AI service whose usage/quota/cost Kodebar tracks. v1 scope: Antigravity (Google Gemini quota), OpenCode Go (subscription usage windows), OpenCode Zen (credit balance).
_Avoid_: source, service, account (use Provider for the service, Account for a credential identity within it)

**Snapshot**:
The merged JSON result of all provider probes, written to `~/.cache/kodebar/last.json`. The single boundary between Backend and Frontend.
_Avoid_: cache (use Snapshot for the file's content; "cache" implies a copy of something else, but the snapshot IS the authoritative current state)

**Stale**:
A provider's data in the Snapshot that is being served from the last successful Probe because the most recent Probe failed. Stale data is still displayed, flagged with a badge, not blanked.
_Avoid_: cached, old, expired

**Quota Window**:
A time-bounded usage limit for a Provider. Antigravity has per-model windows with reset times. OpenCode Go has three windows: rolling 5h, weekly, monthly. Each window has a usage percentage and a reset countdown.
_Avoid_: limit, period, tier

**Credential File**:
A file on disk containing authentication tokens that a Probe reads. Antigravity: `~/.gemini/oauth_creds.json`. OpenCode dashboard: `~/.config/kodebar/opencode-go.json` (workspace ID + auth cookie, 0600 perms). OpenCode providers: `~/.local/share/opencode/auth.json`.
_Avoid_: auth file, token file

**Token Refresh**:
The process of exchanging a refresh token for a new access token before it expires. Must be transparent — the user should never see a token error unless the refresh token itself is invalid.
_Avoid_: re-auth, re-login (those imply user action; refresh is automatic)

## Relationships

- A **Probe** reads a **Credential File**, optionally performs **Token Refresh**, calls the provider's API, and returns quota/balance data
- All **Probes** run in parallel; results are merged into one **Snapshot**
- A **Provider** can have multiple **Quota Windows** (e.g. OpenCode Go has rolling/weekly/monthly)
- A **Stale** flag is set per-provider when the latest **Probe** fails

## Example dialogue

> **Dev:** "When the Antigravity **Probe** gets a 429 from `retrieveUserQuota`, do we mark the **Snapshot** **Stale** or retry?"
> **Domain expert:** "Mark it stale and serve the last-good data. Don't retry immediately — back off. The user doesn't need real-time quota, they need to know roughly where they stand."

## Flagged ambiguities

- "cache" was used to mean both the Snapshot file and the conceptual last-known-good data — resolved: the Snapshot is the file; Stale is the state flag for serving old data within it.