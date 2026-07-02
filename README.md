# Kodebar

> Linux-native AI provider usage tracker for the OpenCode ecosystem. Standalone backend + KDE Plasma Plasmoid frontend. No upstream CLI dependency.

**Status:** Pre-M1 (design/PRD phase, no code yet).

---

## Why

[OpenCode](https://opencode.ai) supports 75+ LLM providers (Gemini, Claude, GPT, OpenCode Zen/Go, …), but there is **no Linux-native usage tracker** for the OpenCode ecosystem. [opencode-bar](https://github.com/opgginc/opencode-bar) solves this on macOS — it auto-detects providers from OpenCode's `auth.json`, probes each provider's quota/cost API, and renders a menu-bar dashboard — but it is macOS-only Swift with no reusable backend.

Kodebar is a **Linux-native** rewrite: a standalone backend that probes provider APIs directly (reading on-disk credentials that OpenCode and Gemini CLI already write), plus a KDE Plasmoid frontend. The backend is DE-agnostic and reusable by any Linux widget/bar.

See [`PRD.md`](./PRD.md) for the full design rationale, architecture, and milestones.

## How it works

```
Kodebar backend (native, DE-agnostic)            →  Plasmoid (QML)
  reads ~/.gemini/oauth_creds.json                  reads ~/.cache/kodebar/last.json
  reads ~/.local/share/opencode/auth.json           on a Timer (or D-Bus signal)
  probes provider quota/cost APIs directly
  writes ~/.cache/kodebar/last.json
  exposes `kodebar status --json`
```

- The backend discovers providers, refreshes OAuth tokens, probes each provider's quota/cost API in parallel, merges results into a cached JSON snapshot, and marks providers `stale` on failure instead of dropping them.
- The Plasmoid renders compact panel text (`Gemini 42% · Zen $12`) and a per-provider popup with usage bars, reset countdowns, and last-updated timestamps.
- Why a separate backend? Token refresh, retries, parallel probing, and disk caching is far easier to get right in a backend service than in QML — and the cache + CLI are reusable by other UI surfaces (waybar, AGS, scripts).

## Provider scope

| Provider | Auth source | Probe method | Verified |
|---|---|---|---|
| Antigravity (Gemini) | `~/.gemini/oauth_creds.json` | Google Code Assist API (`retrieveUserQuota`) | Path confirmed by prior art |
| OpenCode Go | Workspace ID + auth cookie | OpenCode dashboard scrape | ✅ Live-tested |
| OpenCode Zen | Same workspace ID + auth cookie | OpenCode workspace page scrape | ✅ Live-tested |

Antigravity (replacing Gemini CLI) and OpenCode Go are the primary providers. OpenCode Zen balance is shown in the panel. Codex, Claude, and OpenRouter are out of scope (the user doesn't use them). Gemini via API key is not tracked (pay-per-use, no quota window). Browser-cookie-based providers (Cursor, etc.) are v2.

## Prerequisites

Before the backend can probe anything, you must already have authenticated locally:

```bash
gemini login      # or agy login — both write ~/.gemini/oauth_creds.json
opencode auth     # configures providers in ~/.local/share/opencode/auth.json

# OpenCode Go/Zen dashboard access (one-time):
# 1. Visit https://opencode.ai/workspace/<your-workspace-id>/go in a browser
# 2. Copy workspace ID (wrk_...) from URL, and "auth" cookie from DevTools
# 3. Write to ~/.config/kodebar/opencode-go.json:
#    { "workspaceId": "wrk_...", "authCookie": "Fe26.2**..." }
```

## Milestones

- **M1** — Backend: Antigravity + OpenCode Go + Zen probes, CLI output, file cache (testable from terminal)
- **M2** — Minimal Plasmoid: compact panel text reading the cache
- **M3** — Full popup + in-widget settings + D-Bus instant-refresh
- **M4** — Polish: provider logos, KDE Store packaging, troubleshooting doc
- **M5** — Provider expansion (API-key providers, browser-cookie providers via libsecret/kwallet, `state.vscdb` Antigravity fallback)

See [`Milestones.md`](./Milestones.md) for details.

## License

MIT. Provider marks redistributed under the same NOTICE-file approach as [opencode-bar](https://github.com/opgginc/opencode-bar) and [CodexBar](https://github.com/steipete/CodexBar).
