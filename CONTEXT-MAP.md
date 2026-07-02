# Context Map

## Contexts

- [Backend](./backend/CONTEXT.md) — native Linux provider-probe service (Rust): credential reading, OAuth token refresh, API probing, cache writing
- [Frontend](./frontend/CONTEXT.md) — KDE Plasmoid (QML): panel display, popup cards, settings

## Relationships

- **Backend → Frontend**: Backend writes a JSON snapshot to `~/.cache/kodebar/last.json`; Frontend reads it on a Timer (M1–M2) or subscribes to a D-Bus signal (M3+)
- **Backend ↔ Frontend**: Shared schema for the cache file (see PRD §5.5). The schema is opencode-bar-compatible with Kodebar-specific extensions.