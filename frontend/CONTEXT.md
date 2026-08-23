# Frontend

A KDE Plasma Plasmoid (QML) that renders AI provider usage/quota data from the Backend's Snapshot. Pure QML — no Rust dependency, no direct API calls.

## Language

**Compact Representation**:
The panel element shown when the Plasmoid is collapsed. Displays a brief text/icon summary driven by the highest actionable quota Provider, a user-pinned Provider, or the Zen balance fallback.
_Avoid_: tray icon, bar text, status text

**Full Representation**:
The popup element shown when the Plasmoid is expanded. Displays per-provider cards with usage bars, reset countdowns, balance, and last-updated timestamps.
_Avoid_: popup, dropdown, menu, window

**Provider Card**:
A UI element within the Full Representation, one per enabled provider. Shows the provider's quota windows, usage percentages, reset countdowns, and stale state.
_Avoid_: tab, row, entry

**Snapshot Reader**:
The QML component that reads `~/.cache/kodebar/last.json` on a Timer and refreshes immediately after the M3 D-Bus signal. The single data source for the Plasmoid.
_Avoid_: cache reader, file watcher, poller

**Stale Badge**:
A visual indicator on a Provider Card (dimmed icon, greyed text, or a "stale" label) that signals the displayed data is from a failed probe, not current.
_Avoid_: error icon, warning, outdated marker

## Relationships

- The **Snapshot Reader** reads the file written by the Backend and distributes data to **Provider Cards**
- Each **Provider** in the Snapshot gets one **Provider Card** in the **Full Representation**
- The **Compact Representation** is driven by the highest-usage provider or a user-pinned provider from the Snapshot
- A **Stale Badge** appears on a **Provider Card** when the `stale` flag is true for that provider

## Example dialogue

> **Dev:** "Should the **Compact Representation** show the Go rolling 5h percentage or the weekly?"
> **Domain expert:** "Rolling 5h — it's the most volatile and most actionable. The weekly and monthly are context for the **Provider Card** in the **Full Representation**."

## Flagged ambiguities

- "popup" was used to mean both the Full Representation and a settings dialog — resolved: Full Representation is the data popup; settings are a separate configuration page accessible from within it.
