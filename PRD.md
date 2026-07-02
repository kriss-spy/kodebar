# PRD: Kodebar — AI Provider Usage Tracker for Linux

**Project name:** `kodebar`
**Status:** Draft v2 (major redesign)
**Author:** human + AI
**Reference:** [opgginc/opencode-bar](https://github.com/opgginc/opencode-bar) (macOS, Swift) — the OpenCode equivalent of CodexBar, already probes 20+ providers including Gemini CLI. Also: [wakamex/gemini-cli-usage](https://github.com/wakamex/gemini-cli-usage), [a-hariti/gusage](https://github.com/a-hariti/gusage) — standalone Gemini quota monitors that reverse-engineer the Code Assist API.

---

## 1. Problem statement

[OpenCode](https://opencode.ai) is the user's primary AI coding agent. It supports 75+ LLM providers through [Models.dev](https://models.dev), including Gemini, Claude, GPT, and OpenCode's own hosted models (Zen, Go). Understanding how much of a provider's quota or budget you've consumed is essential — but there is **no Linux-native usage tracker** for the OpenCode ecosystem.

[opencode-bar](https://github.com/opgginc/opencode-bar) solves this on macOS: it auto-detects providers from OpenCode's `auth.json`, probes each provider's quota/cost API in parallel, and renders a menu-bar dashboard. But it is:

- **macOS-only** (Swift, Sparkle, Keychain-dependent).
- **Not reusable** as a backend — the provider-probing logic is embedded in the Swift app, not exposed as a library or CLI for other platforms.
- **Keychain-coupled** for some provider auth paths (GitHub Copilot, Claude Code CLI) that don't apply on Linux.

There is also [steipete/CodexBar](https://github.com/steipete/CodexBar) (macOS menu-bar for Codex/Claude/etc.) and its community Linux ports, but those track a different set of providers and the user does not use Codex or Claude.

**Goal of this project:** a **Linux-native** usage tracker — a standalone backend that probes AI provider quota/cost APIs directly (no upstream CLI dependency), plus a KDE Plasma Plasmoid frontend. The backend is DE-agnostic and reusable by any Linux widget/bar.

---

## 2. Non-goals (v1)

- **Not wrapping an upstream CLI.** The original v1 design wrapped `codexbar` CLI; the redesign implements provider probes natively. No dependency on codex-bar, opencode-bar, or any provider's own CLI binary.
- **Not supporting Codex or Claude.** The user does not use them. Provider scope starts with Gemini and the OpenCode hosted ecosystem.
- **Not supporting browser-cookie-based providers** (Cursor, etc.) in v1 — these require decrypting browser "Safe Storage" keys (libsecret/kwallet on Linux), which is a v2 investigation.
- **Not building our own systray protocol handling** — Plasma's `Plasmoid` API and `StatusNotifierItem` conventions already cover this.
- **Not reimplementing OpenCode's session/cost accounting** in v1 — the backend probes live quota/cost APIs, not historical session token logs. Session-level cost tracking (reading OpenCode's SQLite DB) is a possible v2 feature.

---

## 3. Reference architecture (what opencode-bar actually does)

From opencode-bar's README and source structure, the macOS app:

1. **Token discovery** — reads OpenCode's `auth.json` (multi-path: `$XDG_DATA_HOME/opencode`, `~/.local/share/opencode`, `~/Library/Application Support/opencode`) to auto-detect which providers are configured.
2. **Multi-source account discovery** — for some providers, discovers accounts from multiple sources (OpenCode auth, plugin files, CLI config, browser cookies) and deduplicates.
3. **Parallel fetching** — queries all provider APIs simultaneously using TaskGroup.
4. **Smart caching** — falls back to cached data on network errors.
5. **Graceful degradation** — shows available providers even if some fail.

### 3.1 How Gemini CLI quota probing works (the critical path for v1)

Multiple open-source tools (`gusage`, `gemini-cli-usage`, OmniRoute) have reverse-engineered this:

- **Auth:** Gemini CLI stores OAuth credentials at `~/.gemini/oauth_creds.json` (`access_token`, `refresh_token`, `expiry_date`, `id_token`).
- **Token refresh:** uses Gemini CLI's installed OAuth client metadata (`GEMINI_OAUTH_CLIENT_ID`, `GEMINI_OAUTH_CLIENT_SECRET`), or the client ID baked into the Gemini CLI binary.
- **Quota API:** Google's Cloud Code Assist endpoint at `https://cloudcode-pa.googleapis.com/v1internal`:
  - `loadCodeAssist` — returns plan/tier info (`currentTier` / `paidTier`).
  - `retrieveUserQuota` — returns per-model quota buckets with `modelId`, `remainingFraction`, `remainingAmount`, `resetTime`.
- **Known gotchas:**
  - When quota is 100% (full), the API **omits** `remainingAmount` — only `remainingFraction: 1` is returned. Must handle this case explicitly.
  - The `retrieveUserQuota` endpoint itself can return 429 (rate-limited) — must back off, not crash.
  - Google has announced Gemini Code Assist for individuals / AI Pro / AI Ultra in Gemini CLI stops serving on **2026-06-18**, steering users to Antigravity. This may change the auth path.
- **Auth detection precedence:** environment variables → workspace `.gemini/settings.json` → global `~/.gemini/settings.json`.

### 3.2 OpenCode auth and provider discovery

- **`auth.json`** at `~/.local/share/opencode/auth.json` (or `$XDG_DATA_HOME/opencode/auth.json`) contains provider entries with OAuth tokens, API keys, or references to external credential stores.
- **OpenCode Zen** — pay-as-you-go, daily history (30 days), model breakdown. Probed via OpenCode's dashboard API.
- **OpenCode Go** — quota-based, 5h/weekly/monthly usage windows. Requires `OPENCODE_GO_WORKSPACE_ID` + `OPENCODE_GO_AUTH_COOKIE`, or a local config file at `~/.config/opencode-bar/opencode-go.json`.
- **OpenRouter** — pay-as-you-go, credits balance + daily/weekly/monthly cost. Probed via OpenRouter's API.

### 3.3 What this means for Kodebar

The novel work is: implement the provider-probe layer natively on Linux, reading the same on-disk credentials that OpenCode and Gemini CLI already write. No subprocess spawning, no upstream CLI dependency, no macOS-specific auth flows. The Plasmoid is a thin renderer over the backend's JSON snapshot — same split as the original design, just with a native backend instead of a CLI wrapper.

---

## 4. Proposed architecture

```
┌─────────────────────────────────────────────────┐
│  Kodebar backend (native, DE-agnostic)           │
│                                                  │
│  - Provider discovery: reads OpenCode auth.json  │
│    + ~/.gemini/oauth_creds.json for Gemini       │
│  - Probes each provider's quota/cost API         │
│    directly (no subprocess, no upstream CLI)     │
│  - Parallel fetch with per-provider isolation    │
│  - Merges responses into one JSON snapshot       │
│  - Writes ~/.cache/kodebar/last.json             │
│  - Marks providers `stale` on failure            │
│    instead of dropping them                      │
│  - Runs as systemd --user timer or long-lived    │
│    service                                       │
│  - Exposes a CLI: `kodebar status --json`        │
│    for debugging / non-KDE use                   │
└───────────────────┬─────────────────────────────┘
                    │ read (not spawn) — plasmoid
                    │ never shells out directly
                    ▼
┌─────────────────────────────────────────────────┐
│  Plasmoid (QML)                                  │
│  - compactRepresentation: panel text/icon        │
│  - fullRepresentation: popup, per-provider       │
│    card with usage bars + reset times            │
│  - Settings page: provider enable/disable,       │
│    refresh interval                              │
│  - polls the cache file on a Timer, or           │
│    subscribes to a D-Bus signal from the         │
│    backend for instant refresh                   │
└─────────────────────────────────────────────────┘
```

### 4.1 Why a separate backend process instead of QML probing directly

QML *can* make HTTP requests, but doing OAuth token refresh, parallel probing, retry/backoff, fallback-source logic, and disk caching is much easier to get right in a backend service than in QML/JS. It also means:

- The backend can run as a `systemd --user` timer independent of whether the widget is even added to a panel.
- Multiple UI surfaces (Plasmoid now, maybe a waybar/AGS script or KRunner plugin later) can share one cache file instead of each re-probing.
- The backend's `kodebar status --json` CLI is independently useful for scripts, notifications, or non-KDE environments.
- Debugging is `kodebar status` and reading stdout, not digging through Plasma's QML process logs.

### 4.2 IPC: cache file vs. D-Bus

Two refresh-signaling options, not mutually exclusive:

- **File-based (v1, simplest):** Plasmoid `Timer` re-reads `~/.cache/kodebar/last.json` every N seconds. Zero IPC code. Matches what opencode-bar and codexbar-waybar do with `last.json`.
- **D-Bus signal (v1.1, nicer UX):** backend emits a signal on `ai.kodebar` (or `org.kde.plasma.kodebar`) after each successful poll; plasmoid connects via QML's `DBusInterface` for instant updates instead of polling on a timer.

Start with file-based; it's a two-hour implementation and matches the proven prior art. Add D-Bus once the core widget is stable.

### 4.4 Polling interval

Default: **5 minutes**, configurable via `~/.config/kodebar/config.json`. Per-provider override supported (e.g. Antigravity at 5 min, OpenCode dashboard at 10 min) to respect different rate limits. The backend runs as a **systemd `--user` timer** (one-shot: probe → write cache → exit) for M1, transitioning to a **long-lived daemon** in M3 when D-Bus instant-refresh is added.

### 4.3 Backend language

**Rust.** See [ADR-0001](./docs/adr/0001-backend-language-rust.md). The backend is a long-lived service managing OAuth tokens and probing undocumented APIs — a single static binary with type safety is the best fit. Packaging is cleanest (AUR/Fedora/Flatpak all ship Rust binaries trivially), and there's no runtime dependency for users to install.

---

## 5. Provider data-source strategy (v1 scope: Antigravity + OpenCode Go + OpenCode Zen)

The user's actual setup (verified on-disk):
- **Antigravity 2.0** (replaces Gemini CLI) — Google OAuth creds at `~/.gemini/oauth_creds.json`, shared between Gemini CLI and Antigravity (which stores its data in `~/.gemini/antigravity/`, `~/.gemini/antigravity-cli/`, `~/.gemini/antigravity-ide/`). Active account: single Google account.
- **OpenCode Go** — API key in OpenCode `auth.json`, plus workspace ID (`wrk_...`) + auth cookie (`Fe26.2**...` Iron session cookie) for usage data from the OpenCode dashboard.
- **OpenCode Zen** — balance available on the workspace root page with the same auth cookie. No separate auth entry needed.
- Gemini via API key (in OpenCode `auth.json` under `google`) — **not tracked** (pay-per-use, no quota window).

| Provider | Auth source | Probe method | Data returned | Verified? |
|---|---|---|---|---|
| Antigravity (Gemini) | `~/.gemini/oauth_creds.json` | Google Code Assist API: `loadCodeAssist` + `retrieveUserQuota` | Per-model quotas, `remainingFraction`, `resetTime` | Path verified by `gusage`/`gemini-cli-usage`; creds confirmed present |
| OpenCode Go | Workspace ID + auth cookie | Dashboard scrape: `GET https://opencode.ai/workspace/<id>/go` | Rolling 5h / weekly / monthly `usagePercent` + `resetInSec` | ✅ Live-tested — returns 200 with usage data |
| OpenCode Zen | Same workspace ID + auth cookie | Dashboard scrape: `GET https://opencode.ai/workspace/<id>` | `balance`, `reloadAmount`, `reloadTrigger`, `useBalance` | ✅ Live-tested — returns 200 with balance data |

Explicitly **out of scope for v1:** Codex, Claude, OpenRouter (user doesn't use it for quota tracking), browser-cookie-based providers (Cursor, etc.).

### 5.1 Antigravity / Gemini probe detail

Antigravity shares `~/.gemini/oauth_creds.json` with Gemini CLI — the migration doesn't change the probe. Based on reverse-engineering by `gusage`, `gemini-cli-usage`, and OmniRoute:

1. Read `~/.gemini/oauth_creds.json` for `access_token`, `refresh_token`, `expiry_date`.
2. Read `~/.gemini/google_accounts.json` for account email(s).
3. If `access_token` is expired, refresh using the **hardcoded public OAuth client ID/secret** (same approach as `gusage` and `gemini-cli-usage` — it's a public installed-app OAuth client, the "secret" is in the downloadable Gemini CLI binary). If Google rotates the client ID, we update the hardcoded value in a new release. Env vars (`GEMINI_OAUTH_CLIENT_ID` / `GEMINI_OAUTH_CLIENT_SECRET`) override the hardcoded values if set.
4. Call `POST https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist` with the access token → get `currentTier` / `paidTier`.
5. Call `POST https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota` with the access token → get per-model quota buckets.
6. Parse buckets: `modelId`, `remainingFraction`, `remainingAmount` (may be absent at 100%), `resetTime`.
7. Compute usage percentage = `1 - remainingFraction`.
8. Handle 429 from the quota endpoint itself — back off, serve cached/stale data.

**Fallback path (if `oauth_creds.json` stops being maintained):** read OAuth tokens from Antigravity IDE's `state.vscdb` at `~/.config/Antigravity/User/globalStorage/state.vscdb`. This requires SQLite reading (with WAL merge) + protobuf decoding of `jetskiStateSync.agentManagerInitState`. More complex, but the same Google Code Assist API call. This is a v1.1 fallback, not M1.

**Deprecation watch:** Google announced Code Assist for individuals stops serving on 2026-06-18. If this breaks the `retrieveUserQuota` endpoint, the fallback is local LS probing (ConnectRPC to the running Antigravity language server, calling `GetUserStatus`). This is a v2 concern.

### 5.2 OpenCode Go probe detail

No official usage API exists yet (issues [#16017](https://github.com/anomalyco/opencode/issues/16017), [#31084](https://github.com/anomalyco/opencode/issues/31084)). PR [#16513](https://github.com/anomalyco/opencode/pull/16513) adds `GET /zen/go/v1/usage` but is not merged. Until then, dashboard scraping:

1. Read workspace ID and auth cookie from config (see §5.4).
2. `GET https://opencode.ai/workspace/<workspaceId>/go` with `Cookie: auth=<authCookie>`.
3. Parse SolidJS SSR hydration output for three usage windows:
   - Rolling 5h: `{status:"ok",resetInSec:...,usagePercent:...}`
   - Weekly: same shape
   - Monthly: same shape
4. Compute remaining = `100 - usagePercent`, reset time = `now + resetInSec`.

**When the official API lands** (`GET /zen/go/v1/usage` with API key auth), switch to it — cleaner, no cookie dependency, no scraping fragility.

### 5.3 OpenCode Zen probe detail

1. `GET https://opencode.ai/workspace/<workspaceId>` with `Cookie: auth=<authCookie>`.
2. Parse SSR hydration for: `balance` (in cents — negative means credit/prepaid), `reloadAmount`, `reloadTrigger`, `useBalance`.
3. Display balance as dollar amount, plus auto-reload status if configured.

### 5.4 Credential storage for OpenCode dashboard

The workspace ID and auth cookie are stored in `~/.config/kodebar/opencode-go.json` with `0600` file permissions:

```json
{
  "workspaceId": "wrk_01KDDQY25QH35YA78TEKMA2AFA",
  "authCookie": "Fe26.2**..."
}
```

- Env vars (`OPENCODE_GO_WORKSPACE_ID`, `OPENCODE_GO_AUTH_COOKIE`) take precedence if set, for CI/headless flexibility.
- The cookie is a session credential that expires — it's not a permanent secret like an API key. 0600 permissions are the standard Linux approach for session credentials (cf. SSH keys, `.netrc`).
- **Cookie expiration:** the backend detects 401/302 responses and surfaces a "session expired — re-login at opencode.ai" state in the cache file (`stale: true` + an error message), not a crash or silent stale-forever.

### 5.5 Cache file schema

Location: `~/.cache/kodebar/last.json`

Schema follows opencode-bar's `status --json` shape — a flat object keyed by provider ID, with `type` (`quota-based` or `pay-as-you-go`), per-provider fields, and Kodebar-specific extensions (`stale`, `lastUpdated`, `_meta`):

```json
{
  "_meta": {
    "lastUpdated": "2026-07-02T11:17:00Z",
    "version": 1
  },
  "antigravity": {
    "type": "quota-based",
    "usagePercentage": 42,
    "accounts": [
      {
        "email": "krisspy126@gmail.com",
        "remainingPercentage": 58,
        "modelBreakdown": {
          "gemini-2.5-pro": { "remainingPercentage": 58, "resetTime": "2026-07-02T14:00:00Z" },
          "gemini-2.5-flash": { "remainingPercentage": 92, "resetTime": "2026-07-02T14:00:00Z" }
        }
      }
    ],
    "stale": false,
    "lastUpdated": "2026-07-02T11:17:00Z"
  },
  "opencode_go": {
    "type": "quota-based",
    "windows": {
      "rolling":  { "usagePercent": 14, "resetInSec": 11302,   "resetAt": "2026-07-02T14:25:00Z", "status": "ok" },
      "weekly":   { "usagePercent":  9, "resetInSec": 332778,  "resetAt": "2026-07-06T08:00:00Z", "status": "ok" },
      "monthly":  { "usagePercent":  4, "resetInSec": 2282289, "resetAt": "2026-07-28T20:00:00Z", "status": "ok" }
    },
    "stale": false,
    "lastUpdated": "2026-07-02T11:17:00Z"
  },
  "opencode_zen": {
    "type": "pay-as-you-go",
    "balance": -1392399,
    "balanceFormatted": "$13.92",
    "useBalance": true,
    "reloadAmount": 20,
    "reloadTrigger": 5,
    "stale": false,
    "lastUpdated": "2026-07-02T11:17:00Z"
  }
}
```

- Each provider has `stale` (true when serving cached data after a probe failure) and `lastUpdated` (ISO 8601 timestamp of the last successful probe).
- `_meta.lastUpdated` is the overall snapshot timestamp. `_meta.version` is the schema version for forward compatibility.
- The `kodebar status --json` CLI outputs this same schema, making it a drop-in replacement for `opencodebar status --json` in scripts.

---

## 6. Plasmoid UX

### Compact representation (panel)
- Text/icon summary, e.g. `Antigravity 42% · Go 14% · Zen $13.92`, or the single highest-usage provider if space-constrained (configurable).
- Color state via Plasma's theme (ok/warning/critical) matching thresholds: `<70%` ok, `70–90%` warning, `≥90%` critical, plus a distinct `stale` state when serving cached data after a probe failure.

### Full representation (popup)
- Provider tab strip or list, one card per enabled provider.
- Per-provider: usage bars, reset countdown, credit balance if available, last-updated timestamp (important since data may be cached/stale).
- For Antigravity specifically: per-model breakdown (e.g. `gemini-2.5-pro: 42%`, `gemini-2.5-flash: 8%`).
- For OpenCode Go: three usage windows (rolling 5h / weekly / monthly) shown as stacked bars.
- For OpenCode Zen: balance in dollars, auto-reload status.
- Settings section inline or via `Plasmoid.configurationRequired`: enable/disable providers, refresh interval, which provider (or "highest") drives the compact view.

### Iconography
Reuse provider SVG marks from opencode-bar / CodexBar (both MIT-licensed, redistributed with NOTICE files) rather than re-drawing logos.

---

## 7. Resilience requirements

Given that provider APIs are undocumented, reverse-engineered, and prone to breaking:

1. **Never blank the UI on a single failed probe.** Serve last-known-good from cache, flagged stale.
2. **Timeout every probe** (e.g. 15s) — a hung API call must not stall the whole poll cycle.
3. **Per-provider isolation.** One provider failing must not block others from updating.
4. **Backoff on repeated failures**, not fixed-interval retries, to avoid hammering an already-erroring endpoint.
5. **Surface the failure state distinctly** (stale badge, dimmed icon) rather than silently showing wrong/old numbers as if current.
6. **Handle the `remainingAmount` omission** when Antigravity/Gemini quota is at 100% — compute from `remainingFraction` instead.
7. **Token refresh must be silent.** If the OAuth access token is expired, refresh it transparently before probing. Only surface an auth error if the refresh token itself is invalid.
8. **Detect OpenCode dashboard cookie expiration.** A 401 or redirect to login means the Iron session cookie expired. Surface a "session expired — re-login at opencode.ai" state, not a crash or silent stale-forever.

---

## 8. Packaging & distribution

- **Backend:** ship as a standalone package (`kodebar` or `kodebar-backend`) with a `systemd --user` unit, so it can be installed/updated independently of the widget and reused by other Linux DEs. The `kodebar status --json` CLI is included.
- **Plasmoid:** standard `metadata.json` + QML, installable via `kpackagetool6` or KDE Store (store.kde.org) — depends on the backend package.
- **No upstream CLI dependency.** The backend reads on-disk credentials that OpenCode and Gemini CLI already write — it does not shell out to either.
- **Project structure:** monorepo with `backend/` (Rust, single binary crate for M1) and `frontend/` (QML Plasmoid). Split into library + binary crates in M3 if the daemon needs code sharing.
- **License:** MIT, with a NOTICE-file approach for redistributed provider logos.

---

## 9. Milestones

See [`Milestones.md`](./Milestones.md).

---

## 10. Open questions (for grilling)

1. ~~Backend language~~ — Rust. See [ADR-0001](./docs/adr/0001-backend-language-rust.md).
2. ~~Repo rename~~ — done. Repo is `kriss-spy/kodebar`, local folder is `~/Projects/kodebar`.
3. ~~Provider discovery model~~ — auto-detect from OpenCode's `auth.json` (like opencode-bar). May add a `~/.config/kodebar/config.json` override overlay in the future for per-provider options (custom endpoints, disabling a provider), but v1 is zero-config auto-detect.
4. ~~Which providers does the user actually use?~~ — Antigravity (replaces Gemini CLI), OpenCode Go, OpenCode Zen. All three probe paths verified live. Gemini via API key explicitly not tracked (pay-per-use, no quota window).
5. ~~Cache file location and schema~~ — `~/.cache/kodebar/last.json`. Schema matches opencode-bar's `status --json` shape (flat object keyed by provider ID, `type` field, per-provider data) with Kodebar-specific extensions (`stale`, `lastUpdated`, `_meta`). See §5.5.
6. ~~D-Bus service name~~ — `ai.kodebar` (simple, not KDE-specific since the backend is DE-agnostic). Not blocking M1; only needed in M3 when D-Bus signals are added.
7. ~~Backend config location / credential storage~~ — `~/.config/kodebar/opencode-go.json` (0600 perms) for workspace ID + auth cookie. Env vars take precedence if set. No keyring dependency in v1.
8. ~~Antigravity awareness~~ — resolved: Antigravity shares `~/.gemini/oauth_creds.json` with Gemini CLI. The probe uses this file directly. `state.vscdb` fallback is M5.
9. ~~Session/cost tracking~~ — quota-only for v1. Live probing of current quota/balance is sufficient for a status bar widget. Historical session/cost tracking (reading OpenCode's SQLite DB) is a v2 feature.
10. ~~Multi-account support~~ — single-account for v1. The user has one Google account. The schema uses `accounts[]` arrays, so multi-account is a backward-compatible extension for v2.

---

## 11. Prerequisites for anyone testing this

Before the backend can probe anything, the user must already have authenticated locally:

```bash
# Antigravity / Gemini CLI OAuth (writes ~/.gemini/oauth_creds.json)
gemini login          # or agy login — both write to the same file

# OpenCode with providers configured
opencode auth

# OpenCode Go dashboard access (manual one-time setup):
# 1. Visit https://opencode.ai/workspace/<your-workspace-id>/go in a browser
# 2. Copy the workspace ID from the URL (wrk_...)
# 3. Open DevTools → Application → Cookies → opencode.ai → copy the "auth" cookie value
# 4. Write to ~/.config/kodebar/opencode-go.json:
#    { "workspaceId": "wrk_...", "authCookie": "Fe26.2**..." }
```

If `gemini login` works and `~/.gemini/oauth_creds.json` exists, the Antigravity probe is buildable. If the OpenCode dashboard cookie returns 200, the Go and Zen probes are buildable.
