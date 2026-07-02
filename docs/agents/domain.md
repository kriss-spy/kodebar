# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

This is a **multi-context** repo: a DE-agnostic native backend (provider probing, token refresh, caching, resilience — no upstream CLI dependency) and a KDE-specific QML frontend (Plasmoid rendering). They communicate through one boundary — the cached JSON snapshot at `~/.cache/kodebar/last.json`.

## Before exploring, read these

- **`CONTEXT-MAP.md`** at the repo root — it points at one `CONTEXT.md` per context. Read each one relevant to the topic:
  - `backend/CONTEXT.md` — provider auth, OAuth/CLI fallback, polling, caching, resilience
  - `frontend/CONTEXT.md` — Plasma APIs, QML rendering, panel/popup UX, KDE config
- **`docs/adr/`** — system-wide decisions that span both contexts (e.g. the cache-file IPC boundary).
- **`backend/docs/adr/`** and **`frontend/docs/adr/`** — context-scoped decisions.

If any of these files don't exist, **proceed silently**. Don't flag their absence; don't suggest creating them upfront. The producer skill (`/grill-with-docs`) creates them lazily when terms or decisions actually get resolved.

## File structure

```
/
├── CONTEXT-MAP.md            ← points to each context's CONTEXT.md
├── docs/adr/                 ← system-wide decisions (e.g. cache-file IPC boundary)
├── backend/
│   ├── CONTEXT.md            ← backend ubiquitous language
│   └── docs/adr/             ← backend-only decisions
└── frontend/
    ├── CONTEXT.md            ← frontend ubiquitous language
    └── docs/adr/             ← frontend-only decisions
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in the relevant `CONTEXT.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal — either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/grill-with-docs`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 (event-sourced orders) — but worth reopening because…_
