# PRD: AI Provider Usage Tracker for KDE Plasma

**Working name:** `CodexBarKDE` (rename before release)
**Status:** Draft v1
**Author:** human + Claude
**Reference:** [steipete/CodexBar](https://github.com/steipete/CodexBar) (macOS, Swift), [Marouan-chak/codexbar-waybar](https://github.com/Marouan-chak/codexbar-waybar) (Wayland/Waybar port, Python)

---

## 1. Problem statement

[CodexBar](https://github.com/steipete/CodexBar) tracks usage/quota/reset windows across 40+ AI coding providers (Codex, Claude, Copilot, Gemini, etc.) and is genuinely useful, but it is a **macOS menu-bar app**. The project ships a Linux CLI (`codexbar`), but:

- The CLI's `--source web` path is **hard-blocked on non-macOS** (`Error: --source web/auto is only supported on macOS`) — it depends on macOS Keychain-decrypted browser cookies.
- The CLI's `--source cli` path spawns a provider's own CLI (`codex`, `claude`) as a subprocess and talks to it over an internal RPC/app-server protocol. This path is flaky even on macOS (multiple open CodexBar issues cite `RPCWireError`, "app-server closed stdout") and is *more* fragile on Linux because the provider CLIs' local RPC servers are primarily tested against macOS.
- Some Linux packages (e.g. the current `codexbar-cli` AUR build) don't even expose the `serve`/`cost`/`config` subcommands documented upstream — only `usage`.
- There is **no first-party KDE/Plasma UI**. There's a Waybar module (Wayland/wlroots-specific), a GNOME Shell extension, and a Quickshell/Noctalia plugin — nothing for Plasma.
- The existing community attempt, [KodexBar](https://github.com/gengurke/KodexBar), is incomplete.

**Goal of this project:** a native KDE Plasma widget (Plasmoid) that surfaces the same usage/quota/reset information, built *Linux-first*, using data-source strategies that are known to work reliably outside macOS — not a straight port of macOS-specific auth flows.

---

## 2. Non-goals (v1)

- Not reimplementing all 40+ providers on day one. Start with Codex and Claude (the two the upstream README leads with), add others incrementally using the same adapter pattern.
- Not reimplementing CodexBar's provider auth logic in QML/JS (this is very likely why KodexBar stalled — auth/parsing is genuinely complex and best left to the upstream CLI/binary where it already works).
- Not supporting browser-cookie-based providers in v1 (Cursor, OpenCode, Manus, etc.) — these depend on decrypting browser "Safe Storage" keys, which is a macOS-Keychain-specific flow upstream; a Linux equivalent (libsecret/kwallet) is a v2 investigation, not a blocker for v1.
- Not building our own systray protocol handling from scratch — Plasma's `Plasmoid` API and `StatusNotifierItem` conventions already cover this.

---

## 3. Reference architecture (what upstream actually does)

From CodexBar's own `docs/architecture.md`, the macOS app splits into:

| Module | Responsibility |
|---|---|
| `CodexBarCore` | fetch + parse: Codex RPC, PTY runner, Claude probes, OpenAI web scraping, status polling |
| `CodexBar` (app) | state + UI: UsageStore, SettingsStore, StatusItemController, menus, icon rendering |
| `CodexBarCLI` | bundled CLI exposing `usage`/`status` as text or JSON |
| `CodexBarWidget` | WidgetKit extension, reads the shared snapshot |

Data flow: **Background refresh → provider probe → UsageStore (shared state) → menu/icon/widgets.**

The key insight for us: **the CLI already *is* the cross-platform core.** The UI layer (menu bar / widget / systray) is a thin renderer over whatever the CLI's `usage --format json` returns. We should mirror that split, not the Swift internals.

### 3.1 What the Waybar port proves works on Linux

`codexbar-waybar` is the closest prior art and its README documents real, tested findings:

- **`--source oauth` is the correct default on Linux** for Codex and Claude — not `--source cli`, not `--source web` (blocked outright). OAuth reads locally-cached provider credentials (`~/.codex`, `~/.claude` config) rather than spawning an RPC subprocess or decrypting browser cookies.
- **Fallback chain for Claude specifically**: OAuth → local Claude CLI source, only on error (e.g. a 429 from Anthropic's OAuth endpoint). This is a *targeted* fallback, not a default.
- **Provider config lives in `~/.codexbar/config.json`** — read this to know which providers are enabled rather than hardcoding a provider list.
- **Poll per-provider, sequentially, with a stagger** (`codexbar usage --provider <p> --format json`, ~0.5s apart) rather than one combined `--provider all` call — this avoids bursting rate limits across providers that share infra.
- **Cache last-good response** (`~/.cache/<app>/last.json`) and mark stale on failure instead of blanking the UI. A transient 429 or RPC hiccup should never zero out the display.
- **Response schema is stable JSON**: primary/secondary/tertiary usage windows, reset timestamps, credit balances, error info — the same payload shape whether it came from OAuth, CLI fallback, or cache.
- Distro packaging footguns exist (e.g. Arch's `libxml2.so.16` vs. the CLI's `libxml2.so.2` dependency, fixed via `libxml2-legacy`) — worth a troubleshooting doc from day one.

This means the actual novel work for a KDE port is small: a QML/Plasmoid frontend plus a thin polling/caching layer, not a new provider-auth implementation.

---

## 4. Proposed architecture

```
┌─────────────────────────────────────────────┐
│  codexbar CLI (upstream binary, unmodified)  │
│  codexbar usage --provider X --source oauth  │
│    --format json                             │
└───────────────────┬───────────────────────────┘
                    │ spawned per-provider, staggered
                    ▼
┌─────────────────────────────────────────────┐
│  Backend poller (Python, systemd --user      │
│  timer or long-lived service)                │
│  - reads ~/.codexbar/config.json for         │
│    enabled providers                         │
│  - calls CLI per provider with correct       │
│    --source, with OAuth→CLI fallback for     │
│    Claude only                               │
│  - merges responses into one JSON snapshot   │
│  - writes ~/.cache/plasma-codexbar/last.json │
│  - marks providers `stale` on failure        │
│    instead of dropping them                  │
└───────────────────┬───────────────────────────┘
                    │ read (not spawn) — plasmoid
                    │ never shells out directly
                    ▼
┌─────────────────────────────────────────────┐
│  Plasmoid (QML)                              │
│  - compactRepresentation: panel text/icon,   │
│    e.g. "Codex 42% · Claude 8%"              │
│  - fullRepresentation: popup, per-provider    │
│    tab/list with usage bars + reset times     │
│  - Settings page: provider enable/disable,    │
│    writes back to ~/.codexbar/config.json     │
│  - polls the cache file on a Timer, or        │
│    subscribes to a D-Bus signal from the      │
│    backend for instant refresh                │
└─────────────────────────────────────────────┘
```

### 4.1 Why a separate backend process instead of QML calling the CLI directly

QML *can* spawn processes via `Plasma5Support.DataSource`'s executable engine, which was my original suggestion — and it's viable for a v0 prototype. But given how flaky the CLI is (per section 3), doing retries, staggering, fallback-source logic, and disk caching is much easier to get right in a small Python script than in QML/JS. It also means:

- The backend can run as a `systemd --user` timer independent of whether the widget is even added to a panel.
- Multiple UI surfaces (Plasmoid now, maybe a KRunner plugin or notification daemon later) can share one cache file instead of each re-polling the CLI.
- Debugging is `python3 poller.py` and reading stdout, not digging through Plasma's QML process logs.

### 4.2 IPC: cache file vs. D-Bus

Two refresh-signaling options, not mutually exclusive:

- **File-based (v1, simplest):** Plasmoid `Timer` re-reads `~/.cache/plasma-codexbar/last.json` every N seconds. Zero IPC code. Matches what codexbar-waybar does with `last.json`.
- **D-Bus signal (v1.1, nicer UX):** backend emits a signal on `org.kde.plasma.codexbar` after each successful poll; plasmoid connects via QML's `DBusInterface` for instant updates instead of polling on a timer. This is the KDE-native equivalent of waybar's `pkill -RTMIN+8 waybar` signal-refresh trick.

Start with file-based; it's a two-hour implementation and already matches the proven waybar design. Add D-Bus once the core widget is stable.

---

## 5. Provider data-source strategy (v1 scope: Codex + Claude)

| Provider | Correct Linux source | Fallback | Notes |
|---|---|---|---|
| Codex | `--source oauth` | none in v1 | Reads local OpenAI OAuth-cached credentials, no browser cookies needed |
| Claude | `--source oauth` | `--source cli` on error (rate-limit/429 only) | Matches codexbar-waybar's documented behavior |

Explicitly **do not** default to `--source cli` (RPC-based, flaky) or attempt `--source web`/`auto` (hard-blocked outside macOS). Every other provider stays out of scope until v1 ships and we can validate each one's Linux-viable source individually — the README's per-provider docs (`docs/<provider>.md` in upstream) list the auth mechanism for each, and several (API-token providers like z.ai, ElevenLabs, OpenRouter, DeepSeek) should just work anywhere since they're pure API-key HTTP calls with no macOS dependency at all — those are good v1.5 candidates before browser-cookie providers.

---

## 6. Plasmoid UX

### Compact representation (panel)
- Text/icon summary, e.g. `🤖 Codex 42% · Claude 8%`, or the single highest-usage provider if space-constrained (configurable — mirrors waybar's "Highest" vs. "pinned provider" toggle).
- Color state via Plasma's theme (ok/warning/critical) matching thresholds already established by the waybar port: `<70%` ok, `70–90%` warning, `≥90%` critical, plus a distinct `stale` state when serving cached data after a poll failure.

### Full representation (popup)
- Provider tab strip or list, one card per enabled provider.
- Per-provider: session/weekly/monthly usage bars, reset countdown, credit balance if available, last-updated timestamp (important since data may be cached/stale).
- Settings section inline or via `Plasmoid.configurationRequired`: enable/disable providers, refresh interval, which provider (or "highest") drives the compact view.

### Iconography
Upstream's provider SVG marks are MIT-licensed and already redistributed by the waybar port with a NOTICE file — reuse those rather than re-drawing logos.

---

## 7. Resilience requirements (learned from upstream's open issues)

Given how many CodexBar issues are RPC/auth flakiness even on macOS, the Linux port should treat failure as the common case, not the exception:

1. **Never blank the UI on a single failed poll.** Serve last-known-good from cache, flagged stale.
2. **Timeout every CLI invocation** (e.g. `timeout 15s codexbar ...`) — a hung app-server RPC handshake must not stall the whole poll cycle.
3. **Per-provider isolation.** One provider failing must not block others from updating.
4. **Backoff on repeated failures**, not fixed-interval retries, to avoid hammering an already-erroring OAuth endpoint.
5. **Surface the failure state distinctly** (stale badge, dimmed icon) rather than silently showing wrong/old numbers as if current.

---

## 8. Packaging & distribution

- Plasmoid: standard `metadata.json` + QML, installable via `kpackagetool6` or KDE Store (store.kde.org) — no separate binary needed if we bundle the Python poller as a companion package.
- Backend poller: ship as a separate package (`plasma-codexbar-backend` or similar) with a `systemd --user` unit, so it can be installed/updated independently of the widget and reused by other Linux DEs later if desired.
- Dependency: the upstream `codexbar` CLI itself (AUR `codexbar-cli`, or the release tarball) — this project wraps it, doesn't vendor it. Document the `libxml2` compat gotcha from the waybar README up front.
- License: MIT, matching upstream, with the same NOTICE-file approach for redistributed provider logos.

---

## 9. Milestones

**M1 — Backend poller, CLI-only, file cache**
Python script: reads `~/.codexbar/config.json`, polls Codex + Claude via `--source oauth` (+ Claude CLI fallback), writes `~/.cache/plasma-codexbar/last.json`, runs under a systemd user timer. Testable entirely from the terminal before any QML exists.

**M2 — Minimal Plasmoid**
Compact representation reads the cache file on a `Timer`, shows highest-usage provider as panel text. No settings UI yet — config via editing `~/.codexbar/config.json` by hand.

**M3 — Full popup + settings**
Per-provider cards, reset countdowns, stale-state styling. In-widget provider toggle UI that writes back to `~/.codexbar/config.json`.

**M4 — Polish**
D-Bus instant-refresh signal, provider logos, KDE Store packaging, troubleshooting doc (libxml2, OAuth login prerequisites, etc.).

**M5 — Provider expansion**
Add pure API-key providers (z.ai, OpenRouter, DeepSeek, ElevenLabs...) since they need no OAuth/RPC/cookie complexity at all — lowest-risk expansion before tackling browser-cookie-based providers.

---

## 10. Open questions

- Should the backend poller be reusable across DEs (i.e., publish it as a standalone "codexbar-linux-core" package that both this Plasmoid and, say, a future AGS/eww widget could depend on), or keep it Plasma-specific for now? Leaning toward standalone given codexbar-waybar already proves there's cross-compositor demand.
- Do we need libsecret/KWallet-based cookie decryption for browser-auth providers eventually, or is that permanently out of scope? Depends on how many users actually want Cursor/OpenCode/Manus tracking vs. just Codex/Claude.
- Should provider-enable state live only in `~/.codexbar/config.json` (shared with the upstream CLI, simplest) or duplicate into a Plasma-specific config (more idiomatic KConfig, but now two sources of truth)? Leaning toward the former for v1 to stay a thin wrapper.

---

## 11. Prerequisites for anyone testing this

Before the widget can show anything, the upstream CLI must already work standalone for you:

```bash
codex login          # or however Codex OAuth is bootstrapped locally
claude /login         # Claude Code CLI login
codexbar usage --provider codex --source oauth --format json --pretty
codexbar usage --provider claude --source oauth --format json --pretty
```

If those two commands return usable JSON, the rest of this PRD is buildable. If they don't, that's an upstream CLI/auth issue to resolve first (or report to steipete/CodexBar), independent of anything Plasma-specific.
