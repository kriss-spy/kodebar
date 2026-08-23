.pragma library

function state(usage, stale) {
    if (stale)
        return "stale";
    if (typeof usage !== "number" || !isFinite(usage))
        return "unknown";
    if (usage >= 90)
        return "critical";
    if (usage >= 70)
        return "warning";
    return "ok";
}
