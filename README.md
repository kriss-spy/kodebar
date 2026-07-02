# CodexBarKDE

> Native KDE Plasma widget for tracking AI provider usage/quota — Linux-first, built on the upstream [`codexbar`](https://github.com/steipete/CodexBar) CLI.

**Status:** Pre-M1 (design/PRD phase, no code yet).

---

## Why

[CodexBar](https://github.com/steipete/CodexBar) tracks usage/quota/reset windows across 40+ AI coding providers (Codex, Claude, Copilot, Gemini, …), but it is a **macOS menu-bar app**. The bundled Linux CLI is limited:

- `--source web` is hard-blocked outside macOS (depends on Keychain-decrypted browser cookies).
- `--source cli` spawns provider CLIs over a flaky RPC protocol.
- There is **no first-party KDE/Plasma UI** — only Waybar, GNOME Shell, and Quickshell ports exist.

CodexBarKDE is a native Plasmoid that surfaces the same usage info, built Linux-first using data-source strategies known to work reliably outside macOS (OAuth reading local `~/.codex`/`~/.claude` credentials).

See [`PRD.md`](./PRD.md) for the full design rationale, architecture, and milestones.

## How it works

```
codexbar CLI (upstream, unmodified)  →  Backend poller (Python, systemd --user)  →  Plasmoid (QML)
                                         writes ~/.cache/plasma-codexbar/last.json     reads cache on a Timer
```

- The backend reads `~/.codexbar/config.json`, polls each enabled provider with a stagger, merges results into a cached JSON snapshot, and marks providers `stale` on failure instead of dropping them.
- The Plasmoid renders compact panel text (`Codex 42% · Claude 8%`) and a per-provider popup with usage bars, reset countdowns, and last-updated timestamps.
- Why a separate backend? Retries, staggering, fallback-source logic, and disk caching is far easier to get right in a small Python service than in QML — and the cache is reusable by other UI surfaces later.

## Provider scope

| Provider | Source | Fallback |
|---|---|---|
| Codex | `--source oauth` | none (v1) |
| Claude | `--source oauth` | `--source cli` on 429/rate-limit only |

Browser-cookie-based providers (Cursor, OpenCode, Manus…) are v2. Pure API-key providers (z.ai, OpenRouter, DeepSeek, ElevenLabs) are low-risk v1.5 candidates.

## Prerequisites

Before the widget can show anything, the upstream CLI must work for you standalone:

```bash
codex login
claude /login
codexbar usage --provider codex  --source oauth --format json --pretty
codexbar usage --provider claude --source oauth --format json --pretty
```

If those return usable JSON, the rest is buildable. If not, resolve the upstream auth/CLI issue first.

## Milestones

- **M1** — Backend poller, CLI-only, file cache (testable from terminal)
- **M2** — Minimal Plasmoid: compact panel text reading the cache
- **M3** — Full popup + in-widget provider settings
- **M4** — Polish: D-Bus instant-refresh, provider logos, KDE Store packaging, troubleshooting doc
- **M5** — Provider expansion (API-key providers first)

## License

MIT, matching upstream. Provider marks redistributed under the same NOTICE-file approach as the upstream and the [waybar port](https://github.com/Marouan-chak/codexbar-waybar).