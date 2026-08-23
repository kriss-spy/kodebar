.pragma library

function isPercentage(value) {
    return typeof value === "number" && isFinite(value) && value >= 0 && value <= 100;
}

function quotaUsage(provider) {
    if (!provider || provider.type !== "quota-based") {
        return null;
    }
    if (provider.stale === true && !provider.lastUpdated) {
        return null;
    }
    if (isPercentage(provider.usagePercentage)) {
        return provider.usagePercentage;
    }
    const codex = provider.limits && provider.limits.codex;
    const chatGptWindow = codex && (codex.primary || codex.secondary);
    if (chatGptWindow && isPercentage(chatGptWindow.usagePercent)) {
        return chatGptWindow.usagePercent;
    }
    const rolling = provider.windows && provider.windows.rolling;
    if (rolling && (rolling.status === undefined || rolling.status === "ok" || rolling.status === "rate-limited")
            && isPercentage(rolling.usagePercent)) {
        return rolling.usagePercent;
    }
    return null;
}

function providerMap(snapshot) {
    if (!snapshot || typeof snapshot !== "object" || Array.isArray(snapshot)) {
        return {};
    }
    if (snapshot.providers && typeof snapshot.providers === "object"
            && !Array.isArray(snapshot.providers)) {
        return snapshot.providers;
    }
    return snapshot;
}

function selectProvider(snapshot) {
    const providers = providerMap(snapshot);
    const providerIds = Object.keys(providers).filter(function(providerId) {
        return providerId !== "_meta";
    }).sort();
    let selected = null;
    let zenFallback = null;

    providerIds.forEach(function(providerId) {
        const provider = providers[providerId];
        const usage = quotaUsage(provider);
        if (usage !== null && (!selected || usage > selected.usage)) {
            selected = {
                providerId: providerId,
                value: Math.round(usage) + "%",
                usage: usage,
                stale: provider.stale === true,
            };
        }
        if (!zenFallback && providerId === "opencode_zen"
                && typeof provider.balanceFormatted === "string"
                && provider.balanceFormatted.length > 0) {
            zenFallback = {
                providerId: providerId,
                value: provider.balanceFormatted,
                usage: null,
                stale: provider.stale === true,
            };
        }
    });

    return selected || zenFallback || {
        providerId: "",
        value: "",
        usage: null,
        stale: false,
    };
}
