# Troubleshooting Kodebar

Kodebar reads credentials created by provider sign-in flows. Never paste real
API keys, OAuth tokens, or browser cookies into bug reports, terminal output,
or screenshots. The examples below use placeholders deliberately.

Start with a direct Probe and inspect the resulting Snapshot:

```bash
kodebar status --json
stat "${XDG_CACHE_HOME:-$HOME/.cache}/kodebar/last.json"
```

The Snapshot is normally at `~/.cache/kodebar/last.json`; `XDG_CACHE_HOME`
changes the base directory. Provider setup and data sources are described in
[PRD §5](../PRD.md#5-provider-data-source-strategy-initial-scope-antigravity--opencode-go--opencode-zen-then-chatgpt-plans).

## Antigravity does not appear

Run `agy` in a terminal and complete Google sign-in. Kodebar reads the unlocked
Linux Secret Service item with `service=gemini` and `username=antigravity`.
If no item exists, it reads
`~/.gemini/antigravity-cli/antigravity-oauth-token`.

If your desktop wallet is locked, unlock it before refreshing Kodebar.
Kodebar does not show wallet prompts or change shared credentials. Expired
access tokens refresh in memory. If Google rejects the refresh token, sign
in through `agy` again.

Run `kodebar status --json` and look for the top-level `antigravity` entry.
Its `quotaGroups` contain the measured quota windows and reset times.
The Compact Representation uses the highest usage across these windows.
Unknown and disabled quota windows are excluded. Probe failures preserve the
last successful Snapshot data and mark it stale.

Kodebar uses the internal `retrieveUserQuotaSummary` API with the Antigravity
Hub client identity. The protocol may change. A quota access denial can mean
account eligibility restrictions; signing in again does not guarantee access.
Legacy `~/.gemini/oauth_creds.json`, IDE SQLite state, and local language-server
Probes are not used by this implementation.

Token refresh requires OAuth client metadata matching the installed Antigravity
application. Kodebar does not bundle an OAuth client ID or client secret.
Store the matching installed-app values outside the repository:

```json
{
  "clientId": "<installed-app client ID>",
  "clientSecret": "<installed-app client secret>"
}
```

Save this as `~/.config/kodebar/antigravity-oauth.json` with `0600` permissions.
Kodebar honors an absolute `XDG_CONFIG_HOME` instead of `~/.config` when set.
Alternatively, set both `ANTIGRAVITY_OAUTH_CLIENT_ID` and
`ANTIGRAVITY_OAUTH_CLIENT_SECRET` in the backend environment. Set both values
from the same installed-app client. Environment variables take precedence.
An access token that is still valid can be probed without this metadata.


## OpenCode Go login or usage fails

OpenCode Go uses an API key and the official
`GET https://opencode.ai/zen/go/v1/usage` endpoint. It does **not** use the
workspace dashboard cookie. Run the guided setup:

```bash
kodebar login opencode
```

Kodebar opens the OpenCode key page, reads the pasted key without terminal
echo, validates it, and updates OpenCode's standard Credential File at
`${XDG_DATA_HOME:-$HOME/.local/share}/opencode/auth.json`. Other provider
entries are preserved and the file is written with mode `0600`.

- A 401 or “API key was rejected” error means to run the guided login again.
- A 403 stating that a Go subscription is required means the key is valid,
  but its workspace does not have an active Go entitlement.
- For headless environments, `OPENCODE_API_KEY` overrides the Credential File.
  Avoid printing the environment or recording the value in shell history.

See [PRD §5.2](../PRD.md#52-opencode-go-probe-detail) and
[PRD §5.4](../PRD.md#54-credential-storage-for-opencode).

## Optional OpenCode Zen balance

Zen is the OpenCode Probe that uses the browser workspace
session. If Zen balance is wanted:

1. Sign in at `https://opencode.ai` and open
   `https://opencode.ai/workspace/<your-workspace-id>`.
2. Copy the `wrk_...` workspace ID from the URL.
3. In browser developer tools, find the `auth` cookie for `opencode.ai`.
4. Create `~/.config/kodebar/opencode-go.json` using placeholders like this:

   ```json
   {
     "workspaceId": "<workspace-id>",
     "authCookie": "<browser-session-cookie>"
   }
   ```

5. Protect the Credential File before polling:

   ```bash
   chmod 0600 ~/.config/kodebar/opencode-go.json
   ```

If the Snapshot reports `session expired — re-login at opencode.ai`, sign in
again, replace the saved Zen cookie with the new browser-session value, retain
mode `0600`, and wait until `retryAfter` before checking again. This cookie is
optional and is unrelated to Go API-key authentication. See
[PRD §5.3](../PRD.md#53-opencode-zen-probe-detail).

## Fish Audio subscription credits

Run `kodebar login fish-audio` to open Fish Audio authorization in your browser.
Select the team whose subscription credits you want to track.
Kodebar uses OAuth with PKCE and a temporary callback bound to `127.0.0.1`.
The login verifies credit access before saving credentials.

Kodebar stores its own session in
`${XDG_CONFIG_HOME:-$HOME/.config}/kodebar/fish-audio.json` with mode `0600`.
It refreshes this session automatically before the access token expires.
It never reads or changes another MCP client's credentials.
Run the login command again to select another team or replace a revoked session.

The Probe calls `get_credit_balance` on `https://api.fish.audio/mcp`.
The Provider Card shows remaining package credits, extra credits, and the package reset date.
It does not combine those credits with the separate Developer API dollar balance.
The tool identifies the plan but does not return the account's original allowance.
For standard Free, Plus, and Pro plans, the bar uses the published monthly allowance
(8,000, 250,000, and 2,000,000 credits respectively).
It labels this reference as monthly plan credits and keeps extra credits separate.
Balances above the plan allowance produce a full bar while the exact count stays visible.
Unknown plans show the exact credit count without an inferred bar.
See [Fish Audio plans](https://fish.audio/plan/).
You can pin Fish Audio in settings or select it by scrolling over the panel.
It also appears when no quota Provider has data.

## X Premium shared subscription

Kodebar reads the existing Grok CLI login from `~/.grok/auth.json`.
An absolute `GROK_HOME` overrides that directory.
The Credential File must have mode `0600`.
Kodebar never writes or refreshes the shared Grok credentials.

1. If Grok is not signed in, run `grok login`.
2. Run `kodebar login x-premium` to verify billing access.
3. Run `kodebar poll` to update the Snapshot.

When the access token expires, sign in with Grok again.
Kodebar reports an expired token instead of changing the shared session.

The Probe sends the access token to
`https://cli-chat-proxy.grok.com/v1/billing?format=credits`.
It uses Bearer authentication and `x-xai-token-auth: xai-grok-cli`.
Live testing confirmed that this endpoint accepts the existing Grok login.
The Provider Card shows the published weekly usage and reset countdown.
Product percentages show shares of the same weekly allowance.
They are not separate quotas or cash balances.
Legacy X-only Premium plans are not verified.

The initial dashboard-cookie approach was removed after Cloudflare blocked native requests.
No browser cookie is needed. Any manually created `kodebar/x-premium.json` file is unused.

## Intermittent connection failures

Provider reads retry transport failures up to three times within a bounded deadline.
Each attempt allows up to 12 seconds, within the remaining retry budget.
These retries do not repeat HTTP authentication failures or rate-limit responses.
Token exchanges are not repeated automatically.
Provider reads have a 20-second retry budget. Fish Audio uses one stateless MCP credit read with a 35-second retry budget.
The outer Probe deadline limits the complete poll to 45 seconds.
If all attempts fail, Kodebar retains the last successful data and applies backoff.

## Stale data and backoff

`"stale": true` means the latest Probe failed and Kodebar retained the last
successful Provider data instead of blanking it. Check these adjacent fields:

- `lastUpdated`: time of the last successful Probe, not the failed attempt.
- `error`: safe, actionable failure summary.
- `consecutiveFailures`: number of failures since the last success.
- `retryAfter`: earliest time Kodebar will attempt that Provider again.

Repeated failures back off for 5, 10, 20, and 40 minutes, then at most one
hour. Polls during that interval keep the existing Stale entry without calling
the Provider. A successful Probe clears the failure count and backoff. This is
the resilience behavior specified by [PRD §7](../PRD.md#7-resilience-requirements).

## Check the systemd user timer

The shipped timer starts two minutes after activation and normally polls every
five minutes. Inspect it without using `sudo`:

```bash
systemctl --user status kodebar.timer kodebar.service
systemctl --user list-timers kodebar.timer
systemctl --user cat kodebar.service kodebar.timer
journalctl --user -u kodebar.service -n 100 --no-pager
```

Trigger one poll and re-check the journal when diagnosing a failure:

```bash
systemctl --user start kodebar.service
journalctl --user -u kodebar.service -n 50 --no-pager
```

If the unit cannot find Kodebar, compare its `ExecStart` with the result of
`command -v kodebar`. After installing or changing unit files, run:

```bash
systemctl --user daemon-reload
systemctl --user enable --now kodebar.timer
```

## Snapshot is missing or invalid

A missing Snapshot usually means no poll has completed or the service could
not create its directory. Run `kodebar poll`, then inspect the command's exit
status, the user journal, and the Snapshot path.

Validate JSON without displaying credential files:

```bash
jq empty "${XDG_CACHE_HOME:-$HOME/.cache}/kodebar/last.json"
```

If the Snapshot is malformed, preserve it for diagnosis and let a new poll
replace it atomically:

```bash
mv "${XDG_CACHE_HOME:-$HOME/.cache}/kodebar/last.json" \
  "${XDG_CACHE_HOME:-$HOME/.cache}/kodebar/last.json.invalid"
kodebar poll
```

The Snapshot should contain `_meta` plus top-level Provider entries. Do not
place credentials in the Snapshot. Its schema is documented in
[PRD §5.5](../PRD.md#55-cache-file-schema).

## Plasmoid settings or instant refresh do not behave as expected

The Plasma settings page controls which Providers are displayed, which
Provider drives the Compact Representation, and how often the Snapshot Reader
checks the file as a fallback. It does not change the backend's five-minute
systemd Probe schedule.

After a successful Snapshot write, Kodebar broadcasts the session-bus signal
`ai.kodebar.SnapshotUpdated`. The Plasmoid reacts immediately and also keeps
its Timer fallback, so a missed or unavailable D-Bus signal should delay—not
prevent—the next display update. To observe the signal while triggering a
poll in another terminal:

```bash
dbus-monitor --session \
  "type='signal',path='/ai/kodebar',interface='ai.kodebar',member='SnapshotUpdated'"
```

If the Snapshot changes but no signal appears, inspect the user-service
journal for a refresh-notification failure and verify that both commands run
in the same desktop session.

## ChatGPT plan-usage caveat

ChatGPT support concerns subscription-plan quota, not OpenAI API billing. The
Probe reads the file-backed ChatGPT session at `~/.codex/auth.json` and calls
`https://chatgpt.com/backend-api/wham/usage`. That compatibility endpoint is
internal and undocumented, so it can change without notice.

Do not copy, extract, or paste ChatGPT access tokens into Kodebar configuration.
The Credential File must come from the normal ChatGPT sign-in flow and must
remain mode `0600`. If it is missing, or the Probe receives 401, sign in with
ChatGPT in Codex again and let that application replace its session file.
Kodebar does not ask users to manage the token themselves. See
[PRD §5.3.1](../PRD.md#531-chatgpt-subscription-plan-probe).
