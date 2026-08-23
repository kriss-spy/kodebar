# Kodebar

> Linux-native AI provider usage tracker for the OpenCode ecosystem. Standalone backend + KDE Plasma Plasmoid frontend. No upstream CLI dependency.

**Status:** M4 implemented; M5 Provider expansion is next

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
| OpenCode Go | API key in OpenCode `auth.json` | Official `GET /zen/go/v1/usage` API | ✅ Live-tested |
| OpenCode Zen | Same workspace ID + auth cookie | OpenCode workspace page scrape | ✅ Live-tested |
| ChatGPT subscription plans | Read-only `~/.codex/auth.json` session | Native ChatGPT quota Probe | ✅ Live source validated; internal endpoint |

Antigravity (replacing Gemini CLI), OpenCode Go, and ChatGPT plan usage are the primary quota Providers. OpenCode Zen balance is optional. ChatGPT tracks subscription quota, not pay-as-you-go OpenAI API usage. Claude and OpenRouter remain out of scope. Gemini via API key is not tracked (pay-per-use, no quota window). Browser-cookie-based providers (Cursor, etc.) are v2.

## Prerequisites

Before the backend can probe anything, you must already have authenticated locally:

```bash
gemini login      # or agy login — both write ~/.gemini/oauth_creds.json
kodebar login opencode  # opens the browser, validates the key, writes OpenCode auth.json

# ChatGPT: sign in with ChatGPT in Codex once. Kodebar reads the current
# ~/.codex/auth.json session without refreshing or modifying it.

# Optional: OpenCode Zen dashboard balance still needs a browser session:
# 1. Visit https://opencode.ai/workspace/<your-workspace-id> in a browser
# 2. Copy workspace ID (wrk_...) and the "auth" cookie from DevTools
# 3. Write ~/.config/kodebar/opencode-go.json:
#    { "workspaceId": "wrk_...", "authCookie": "Fe26.2**..." }
```

## Autostart

After installing the `kodebar` binary at `/usr/bin/kodebar`, install and enable
the systemd user timer:

```bash
mkdir -p ~/.config/systemd/user
cp backend/assets/kodebar.service backend/assets/kodebar.timer ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now kodebar.timer
```

The first poll runs after a two-minute startup delay, then every five minutes.
Check the timer and recent Probe logs with:

```bash
systemctl --user list-timers kodebar.timer
journalctl --user -u kodebar.service
stat ~/.cache/kodebar/last.json
```

## Install the Plasma widget

For a development checkout, install the Plasmoid directly:

```bash
kpackagetool6 --type Plasma/Applet --install frontend/package
```

M2/M3 development builds used the temporary Plasmoid ID `ai.kodebar`. Before
installing M4/0.1.0, remove that package and add Kodebar to the panel again;
Plasma does not migrate the old widget instance or its settings:

```bash
kpackagetool6 --type Plasma/Applet --remove ai.kodebar
```

After installing 0.1.0 or newer, use `--upgrade` for subsequent versions. To
create and install the same versioned archive used for KDE Store releases:

```bash
frontend/scripts/package-plasmoid.sh
kpackagetool6 --type Plasma/Applet --install dist/kodebar-0.1.0.plasmoid
```

The Plasmoid requires the native backend and reads its
`~/.cache/kodebar/last.json` Snapshot. See
[`docs/releasing.md`](./docs/releasing.md) for the release checklist.

## Milestones

- **M1** — Backend: Antigravity + OpenCode Go + Zen probes, CLI output, file cache (testable from terminal)
- **M1.1** — ChatGPT plans: native read-only session Probe with plan quota windows
- **M2** — Minimal Plasmoid: compact panel text reading the Snapshot (implemented)
- **M3** — Full Representation, Provider settings, and D-Bus instant refresh (implemented)
- **M4** — Provider identity, KDE Store packaging, and release polish (implemented)
- **M5** — Provider expansion (API-key providers, browser-cookie providers via libsecret/kwallet, `state.vscdb` Antigravity fallback)

See [`Milestones.md`](./Milestones.md) for details.

## License

MIT. Provider marks redistributed under the same NOTICE-file approach as [opencode-bar](https://github.com/opgginc/opencode-bar) and [CodexBar](https://github.com/steipete/CodexBar).
