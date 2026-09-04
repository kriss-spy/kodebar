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
Implemented Plasma 6 package: the Snapshot Reader reads the Snapshot on a `Timer`, and the Compact Representation shows the highest actionable quota Provider with Zen fallback and Stale styling.

**M3 — Full popup + settings**
Implemented per-Provider Cards, reset countdowns, Stale Badges, Antigravity per-model breakdown, Go three-window bars, Zen balance, and ChatGPT plan usage. Plasma settings control Provider visibility, the Snapshot check interval, and automatic or pinned Compact selection. The backend broadcasts `ai.kodebar.SnapshotUpdated` after persistence; the Snapshot Reader refreshes immediately while retaining its Timer fallback.

**M4 — Polish**
Implemented theme-aware Provider marks in the Compact and Full Representations, Plasma usage-state colors, accessible decorative-icon behavior, wheel navigation between eligible Compact Providers, current Plasma 6 metadata, packaged MIT attribution, a versioned `.plasmoid` release script, isolated archive-install coverage, and a release checklist. Troubleshooting covers guided login, optional Zen cookie expiration, ChatGPT session recovery, D-Bus refresh, and the Antigravity deprecation watch.

**M5 — Provider expansion**
Add pure API-key providers (z.ai, DeepSeek, etc.) and browser-cookie-based providers (Cursor, etc.) via libsecret/kwallet credential reading. Add `state.vscdb` fallback for Antigravity if `oauth_creds.json` stops being maintained.
