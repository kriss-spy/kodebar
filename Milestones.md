# Milestones

**M1 — Backend: Antigravity + OpenCode Go + Zen probes, CLI output, file cache**
Native Rust backend that:
- Reads `~/.gemini/oauth_creds.json`, refreshes Google OAuth tokens, probes Code Assist API (`loadCodeAssist` + `retrieveUserQuota`) for Antigravity/Gemini per-model quotas.
- Reads workspace ID + auth cookie from `~/.config/kodebar/opencode-go.json` (or env vars), scrapes OpenCode dashboard for Go usage windows (rolling/weekly/monthly) and Zen balance.
- Merges all probe results into `~/.cache/kodebar/last.json`.
- Exposes `kodebar status --json` for terminal testing.
- Runs under a `systemd --user` timer.
Testable entirely from the terminal before any QML exists.

**M2 — Minimal Plasmoid**
Compact representation reads the cache file on a `Timer`, shows highest-usage provider as panel text. No settings UI yet — config via editing `~/.config/kodebar/` files by hand.

**M3 — Full popup + settings**
Per-provider cards, reset countdowns, stale-state styling, Antigravity per-model breakdown, Go three-window bars, Zen balance. In-widget provider toggle UI. D-Bus instant-refresh signal.

**M4 — Polish**
Provider logos, KDE Store packaging, troubleshooting doc (OAuth login prerequisites, cookie expiration handling, Antigravity deprecation watch, official `/zen/go/v1/usage` API migration path).

**M5 — Provider expansion**
Add pure API-key providers (z.ai, DeepSeek, etc.) and browser-cookie-based providers (Cursor, etc.) via libsecret/kwallet credential reading. Add `state.vscdb` fallback for Antigravity if `oauth_creds.json` stops being maintained.