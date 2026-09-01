.pragma library

const PROVIDERS = [
    { id: "opencode_go", title: qsTr("OpenCode Go"), compactTitle: qsTr("Go"), icon: "../images/provider-opencode-go.svg", setup: qsTr("Connect your OpenCode account to see Go plan limits") },
    { id: "opencode_zen", title: qsTr("OpenCode Zen"), compactTitle: qsTr("Zen"), icon: "../images/provider-opencode.svg", setup: qsTr("Configure optional OpenCode Zen credentials") },
    { id: "chatgpt", title: qsTr("ChatGPT"), compactTitle: qsTr("ChatGPT"), icon: "../images/provider-chatgpt.svg", setup: qsTr("Connect your ChatGPT account to see Codex plan limits") },
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

function goRows(provider, nowMs) {
    const windows = provider.windows || {};
    return [
        { key: "rolling", label: qsTr("Rolling 5 hours") },
        { key: "weekly", label: qsTr("Weekly") },
        { key: "monthly", label: qsTr("Monthly") },
    ].filter(function(definition) {
        const window = windows[definition.key];
        return window && typeof window.usagePercent === "number" && isFinite(window.usagePercent);
    }).map(function(definition) {
        const window = windows[definition.key];
        return {
            label: definition.label,
            usagePercent: Number(window.usagePercent) || 0,
            resetText: countdown(window.resetAt, nowMs),
            statusText: window.status === "rate-limited" ? qsTr("Rate limited") : "",
        };
    });
}

function quotaWindowLabel(window, fallback) {
    const duration = Number(window && window.windowDurationSec);
    if (!isFinite(duration) || duration <= 0)
        return fallback;
    if (duration <= 86400)
        return qsTr("Session");
    if (duration <= 8 * 86400)
        return qsTr("Weekly");
    if (duration <= 32 * 86400)
        return qsTr("Monthly");
    return fallback;
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

function isAuthenticationError(error) {
    const message = String(error || "").toLowerCase();
    return message.includes("auth")
        || message.includes("login")
        || message.includes("credential")
        || message.includes("token")
        || message.includes("401")
        || message.includes("403");
}

function errorText(error) {
    return isAuthenticationError(error)
        ? qsTr("Sign-in expired. Reconnect to resume updates.")
        : String(error || "");
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
            if (!window || typeof window.usagePercent !== "number" || !isFinite(window.usagePercent))
                return;
            const windowLabel = quotaWindowLabel(window, definition.label);
            rows.push({
                label: key === "codex"
                    ? windowLabel
                    : qsTr("%1 · %2").replace("%1", limitLabel).replace("%2", windowLabel),
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
    if (providerId === "opencode_go") {
        return {
            providerId: providerId,
            title: definition.title,
            rows: goRows(provider, nowMs),
            stale: provider.stale === true,
            lastUpdated: provider.lastUpdated || "",
            error: errorText(provider.error),
            detailText: "",
            secondaryText: "",
            connected: !isAuthenticationError(provider.error),
            actionLabel: isAuthenticationError(provider.error) ? qsTr("Reconnect OpenCode") : "",
        };
    }
    if (providerId === "opencode_zen") {
        return {
            providerId: providerId,
            title: definition.title,
            rows: [],
            stale: provider.stale === true,
            lastUpdated: provider.lastUpdated || "",
            error: errorText(provider.error),
            detailText: qsTr("Balance %1").replace("%1", provider.balanceFormatted || qsTr("Unavailable")),
            secondaryText: provider.useBalance
                ? qsTr("Auto-reload $%1 at $%2").replace("%1", provider.reloadAmount).replace("%2", provider.reloadTrigger)
                : qsTr("Auto-reload off"),
            connected: !isAuthenticationError(provider.error),
            actionLabel: "",
        };
    }
    if (providerId === "chatgpt") {
        return {
            providerId: providerId,
            title: definition.title,
            rows: chatGptRows(provider, nowMs),
            stale: provider.stale === true,
            lastUpdated: provider.lastUpdated || "",
            error: errorText(provider.error),
            detailText: planName(provider.planType),
            secondaryText: "",
            connected: !isAuthenticationError(provider.error),
            actionLabel: isAuthenticationError(provider.error) ? qsTr("Reconnect ChatGPT") : "",
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
        connected: false,
        actionLabel: providerId === "chatgpt" ? qsTr("Sign in with ChatGPT")
            : providerId === "opencode_go" ? qsTr("Start guided login")
            : "",
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
