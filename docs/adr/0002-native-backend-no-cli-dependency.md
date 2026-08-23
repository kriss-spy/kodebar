# Native backend — no upstream CLI dependency

The original v1 design wrapped the upstream `codexbar` CLI (spawning it per-provider). The redesign implements all provider probes natively in Rust, reading on-disk credentials directly. This was driven by three factors: (1) the user switched from Codex/Claude to OpenCode + Antigravity, and there is no equivalent upstream CLI for the OpenCode ecosystem that works on Linux — opencode-bar is macOS-only Swift; (2) spawning a subprocess per probe is inherently fragile (RPC errors, binary version drift, distro packaging issues); (3) the credential files (`~/.gemini/oauth_creds.json`, OpenCode `auth.json`, and the optional Zen dashboard cookie) are already on disk, making native probing simpler than wrapping a provider CLI.

The `kodebar login opencode` setup command is a narrow exception: it asks the desktop URL handler to open `https://opencode.ai/auth`, then accepts the API key through a hidden terminal prompt. It does not invoke an upstream provider CLI, and normal Probes remain subprocess-free.

The Plasma 6 frontend has a second, read-only compatibility exception. A pure-QML
Plasmoid has no general local-file content API, while QML XHR blocks `file:` reads
unless the host opts into them globally. `SnapshotReader.qml` therefore uses
Plasma's executable data engine to run the fixed command `/usr/bin/cat -- <quoted
Snapshot path>`. The path is generated from Qt's cache location (or supplied by
the test harness), is shell-quoted, and no Snapshot or Provider value becomes an
executable. This adapter only transports the cached JSON into QML; it performs no
Provider discovery, authentication, probing, or network access.
