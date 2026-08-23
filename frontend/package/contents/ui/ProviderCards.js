.pragma library

const PROVIDERS = [
    { id: "antigravity", title: qsTr("Antigravity"), compactTitle: qsTr("Antigravity"), icon: "../images/provider-antigravity.svg", setup: qsTr("Sign in with Gemini CLI or Antigravity") },
    { id: "opencode_go", title: qsTr("OpenCode Go"), compactTitle: qsTr("Go"), icon: "../images/provider-opencode-go.svg", setup: qsTr("Run kodebar login opencode") },
    { id: "opencode_zen", title: qsTr("OpenCode Zen"), compactTitle: qsTr("Zen"), icon: "../images/provider-opencode.svg", setup: qsTr("Configure optional OpenCode Zen credentials") },
    { id: "chatgpt", title: qsTr("ChatGPT"), compactTitle: qsTr("ChatGPT"), icon: "../images/provider-chatgpt.svg", setup: qsTr("Sign in with ChatGPT in Codex") },
];

function providerDefinition(providerId) {
    return PROVIDERS.find(function(provider) {
        return provider.id === providerId;
    });
}

function providerCompactTitle(providerId) {
    const definition = providerDefinition(providerId);
    return definition ? definition.compactTitle : providerId;
}

function iconSource(providerId) {
    const definition = providerDefinition(providerId);
    return definition ? definition.icon : "view-statistics";
}

function countdown(resetTime, nowMs) {
    if (!resetTime)
        return qsTr("No reset time");

    const resetMs = Date.parse(resetTime);
    if (!isFinite(resetMs))
        return qsTr("No reset time");
    const remainingSec = Math.max(0, Math.ceil((resetMs - nowMs) / 1000));
    if (remainingSec === 0)
        return qsTr("Reset due");
    if (remainingSec < 60)
        return qsTr("Resets in %1s").replace("%1", remainingSec);
    if (remainingSec < 3600)
        return qsTr("Resets in %1m").replace("%1", Math.ceil(remainingSec / 60));
    if (remainingSec < 86400)
        return qsTr("Resets in %1h").replace("%1", Math.ceil(remainingSec / 3600));
    return qsTr("Resets in %1d").replace("%1", Math.ceil(remainingSec / 86400));
}

function ageText(timestamp, nowMs) {
    if (!timestamp)
        return qsTr("Never");
    const updatedMs = Date.parse(timestamp);
    if (!isFinite(updatedMs))
        return qsTr("Never");
    const elapsedSec = Math.max(0, Math.floor((nowMs - updatedMs) / 1000));
    if (elapsedSec < 60)
        return qsTr("Just now");
    if (elapsedSec < 3600)
        return qsTr("%1m ago").replace("%1", Math.floor(elapsedSec / 60));
    if (elapsedSec < 86400)
        return qsTr("%1h ago").replace("%1", Math.floor(elapsedSec / 3600));
    return qsTr("%1d ago").replace("%1", Math.floor(elapsedSec / 86400));
}

function antigravityRows(provider, nowMs) {
    const rows = [];
    const accounts = Array.isArray(provider.accounts) ? provider.accounts : [];
    accounts.forEach(function(account) {
        const breakdown = account && account.modelBreakdown;
        if (!breakdown || typeof breakdown !== "object")
            return;
        Object.keys(breakdown).sort().forEach(function(modelId) {
            const quota = breakdown[modelId] || {};
            const remaining = Number(quota.remainingPercentage);
            rows.push({
                label: modelId,
                usagePercent: isFinite(remaining) ? Math.max(0, Math.min(100, 100 - remaining)) : 0,
                resetText: countdown(quota.resetTime, nowMs),
                statusText: "",
            });
        });
    });
    return rows;
}

function goRows(provider, nowMs) {
    const windows = provider.windows || {};
    return [
        { key: "rolling", label: qsTr("Rolling 5 hours") },
        { key: "weekly", label: qsTr("Weekly") },
        { key: "monthly", label: qsTr("Monthly") },
    ].map(function(definition) {
        const window = windows[definition.key] || {};
        return {
            label: definition.label,
            usagePercent: Number(window.usagePercent) || 0,
            resetText: countdown(window.resetAt, nowMs),
            statusText: window.status === "rate-limited" ? qsTr("Rate limited") : "",
        };
    });
}

function planName(planType) {
    switch (String(planType || "").toLowerCase()) {
    case "free": return qsTr("Free plan");
    case "plus": return qsTr("Plus plan");
    case "pro": return qsTr("Pro plan");
    case "team": return qsTr("Team plan");
    case "business": return qsTr("Business plan");
    case "enterprise": return qsTr("Enterprise plan");
    default: return qsTr("Subscription plan");
    }
}

function chatGptRows(provider, nowMs) {
    const limits = provider.limits || {};
    const keys = Object.keys(limits).sort(function(left, right) {
        if (left === "codex") return -1;
        if (right === "codex") return 1;
        return left.localeCompare(right);
    });
    const rows = [];
    keys.forEach(function(key) {
        const limit = limits[key] || {};
        const limitLabel = key === "codex" ? qsTr("Codex") : (limit.limitName || key);
        [
            { key: "primary", label: qsTr("Primary") },
            { key: "secondary", label: qsTr("Secondary") },
        ].forEach(function(definition) {
            const window = limit[definition.key];
            if (!window)
                return;
            rows.push({
                label: qsTr("%1 · %2").replace("%1", limitLabel).replace("%2", definition.label),
                usagePercent: Number(window.usagePercent) || 0,
                resetText: countdown(window.resetAt, nowMs),
                statusText: "",
            });
        });
    });
    return rows;
}

function card(providerId, provider, nowMs) {
    const definition = providerDefinition(providerId);
    if (providerId === "antigravity") {
        return {
            providerId: providerId,
            title: definition.title,
            rows: antigravityRows(provider, nowMs),
            stale: provider.stale === true,
            lastUpdated: provider.lastUpdated || "",
            error: provider.error || "",
            detailText: "",
            secondaryText: "",
        };
    }
    if (providerId === "opencode_go") {
        return {
            providerId: providerId,
            title: definition.title,
            rows: goRows(provider, nowMs),
            stale: provider.stale === true,
            lastUpdated: provider.lastUpdated || "",
            error: provider.error || "",
            detailText: "",
            secondaryText: "",
        };
    }
    if (providerId === "opencode_zen") {
        return {
            providerId: providerId,
            title: definition.title,
            rows: [],
            stale: provider.stale === true,
            lastUpdated: provider.lastUpdated || "",
            error: provider.error || "",
            detailText: qsTr("Balance %1").replace("%1", provider.balanceFormatted || qsTr("Unavailable")),
            secondaryText: provider.useBalance
                ? qsTr("Auto-reload $%1 at $%2").replace("%1", provider.reloadAmount).replace("%2", provider.reloadTrigger)
                : qsTr("Auto-reload off"),
        };
    }
    if (providerId === "chatgpt") {
        return {
            providerId: providerId,
            title: definition.title,
            rows: chatGptRows(provider, nowMs),
            stale: provider.stale === true,
            lastUpdated: provider.lastUpdated || "",
            error: provider.error || "",
            detailText: planName(provider.planType),
            secondaryText: "",
        };
    }
    return null;
}

function setupCard(providerId) {
    const definition = providerDefinition(providerId);
    return {
        providerId: providerId,
        title: definition.title,
        rows: [],
        stale: false,
        lastUpdated: "",
        error: "",
        detailText: definition.setup,
        secondaryText: qsTr("No Snapshot data yet"),
    };
}

function cards(snapshot, nowMs, enabledProviders) {
    if (!snapshot || typeof snapshot !== "object")
        snapshot = {};
    return PROVIDERS.map(function(provider) {
        return provider.id;
    }).filter(function(providerId) {
        if (enabledProviders)
            return enabledProviders[providerId] !== false;
        return snapshot[providerId] !== undefined;
    }).map(function(providerId) {
        return snapshot[providerId] === undefined
            ? setupCard(providerId)
            : card(providerId, snapshot[providerId], nowMs);
    });
}
