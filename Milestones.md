# Milestones

**M1 — Backend: Antigravity + OpenCode Go + Zen probes, CLI output, file cache**
Native Rust backend that:
- Reads `~/.gemini/oauth_creds.json`, refreshes Google OAuth tokens, probes Code Assist API (`loadCodeAssist` + `retrieveUserQuota`) for Antigravity/Gemini per-model quotas.
- Reads the OpenCode API key from `~/.local/share/opencode/auth.json` (or `OPENCODE_API_KEY`) and calls the official Go usage API for rolling/weekly/monthly windows. Zen balance remains an optional dashboard-cookie Probe.
- Merges all probe results into `~/.cache/kodebar/last.json`.
- Exposes `kodebar status --json` for terminal testing.
- Runs under a `systemd --user` timer.
Testable entirely from the terminal before any QML exists.

**M1.1 — ChatGPT subscription plans**
Implemented as a native, read-only Probe of the current file-backed Codex ChatGPT session. The Snapshot exposes plan identity and default/additional quota windows. Kodebar neither refreshes nor writes the shared session; rejected sessions become actionable Stale state. The validated direct endpoint is first-party but internal, so response parsing remains a compatibility boundary. This tracks ChatGPT subscription usage, not pay-as-you-go OpenAI API billing.

**M2 — Minimal Plasmoid**
Implemented Plasma 6 package: the Snapshot Reader reads the cache file on a `Timer`, and the Compact Representation shows the highest actionable quota Provider (with Zen fallback and Stale styling). No settings UI yet.

**M3 — Full popup + settings**
Per-provider cards, reset countdowns, stale-state styling, Antigravity per-model breakdown, Go three-window bars, Zen balance, and ChatGPT plan usage. In-widget provider toggle UI. D-Bus instant-refresh signal.

**M4 — Polish**
Provider logos, KDE Store packaging, troubleshooting doc (guided login, optional Zen cookie expiration handling, Antigravity deprecation watch).

**M5 — Provider expansion**
Add pure API-key providers (z.ai, DeepSeek, etc.) and browser-cookie-based providers (Cursor, etc.) via libsecret/kwallet credential reading. Add `state.vscdb` fallback for Antigravity if `oauth_creds.json` stops being maintained.
