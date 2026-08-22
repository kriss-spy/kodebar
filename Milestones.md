# Milestones

**M1 — Backend: Antigravity + OpenCode Go + Zen probes, CLI output, file cache**
Native Rust backend that:
- Reads `~/.gemini/oauth_creds.json`, refreshes Google OAuth tokens, probes Code Assist API (`loadCodeAssist` + `retrieveUserQuota`) for Antigravity/Gemini per-model quotas.
- Reads workspace ID + auth cookie from `~/.config/kodebar/opencode-go.json` (or env vars), scrapes OpenCode dashboard for Go usage windows (rolling/weekly/monthly) and Zen balance.
- Merges all probe results into `~/.cache/kodebar/last.json`.
- Exposes `kodebar status --json` for terminal testing.
- Runs under a `systemd --user` timer.
Testable entirely from the terminal before any QML exists.

**M1.1 — ChatGPT subscription plans**
Immediately after the OpenCode backend is complete, validate the locally available OpenAI session credentials and the current plan-usage data source, then add a native ChatGPT Probe. The Snapshot should expose the signed-in plan identity, available quota windows, usage percentages, and reset times when the source provides them. This tracks ChatGPT subscription-plan usage (such as Plus or Pro), not pay-as-you-go OpenAI API billing. The Probe must preserve Kodebar's no-upstream-CLI boundary and Stale behavior.

**M2 — Minimal Plasmoid**
Compact representation reads the cache file on a `Timer`, shows highest-usage provider as panel text. No settings UI yet — config via editing `~/.config/kodebar/` files by hand.

**M3 — Full popup + settings**
Per-provider cards, reset countdowns, stale-state styling, Antigravity per-model breakdown, Go three-window bars, Zen balance, and ChatGPT plan usage. In-widget provider toggle UI. D-Bus instant-refresh signal.

**M4 — Polish**
Provider logos, KDE Store packaging, troubleshooting doc (OAuth login prerequisites, cookie expiration handling, Antigravity deprecation watch, official `/zen/go/v1/usage` API migration path).

**M5 — Provider expansion**
Add pure API-key providers (z.ai, DeepSeek, etc.) and browser-cookie-based providers (Cursor, etc.) via libsecret/kwallet credential reading. Add `state.vscdb` fallback for Antigravity if `oauth_creds.json` stops being maintained.
