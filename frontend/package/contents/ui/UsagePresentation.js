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

function remaining(usage) {
    return Math.max(0, Math.min(100, 100 - Number(usage)));
}

function percentage(value) {
    const rounded = Math.round(Number(value) * 10) / 10;
    return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1);
}
