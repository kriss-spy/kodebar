.pragma library

function isPercentage(value) {
    return typeof value === "number" && isFinite(value) && value >= 0 && value <= 100;
}

function remainingText(usage) {
    const remaining = Math.round((100 - usage) * 10) / 10;
    return (Number.isInteger(remaining) ? String(remaining) : remaining.toFixed(1)) + "% left";
}

function quotaUsage(provider) {
    if (!provider || provider.type !== "quota-based") {
        return null;
    }
    if (provider.stale === true && !provider.lastUpdated) {
        return null;
    }
    let worst = null;
    function consider(value) {
        if (isPercentage(value) && (worst === null || value > worst)) {
            worst = value;
        }
    }
    consider(provider.usagePercentage);
    const limits = provider.limits;
    if (limits && typeof limits === "object") {
        Object.keys(limits).forEach(function(key) {
            const limit = limits[key] || {};
            ["primary", "secondary"].forEach(function(slot) {
                const window = limit[slot];
                if (window)
                    consider(window.usagePercent);
            });
        });
    }
    const windows = provider.windows;
    if (windows && typeof windows === "object") {
        Object.keys(windows).forEach(function(key) {
            const window = windows[key] || {};
            if (window.status === undefined || window.status === "ok" || window.status === "rate-limited")
                consider(window.usagePercent);
        });
    }
    return worst;
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

function selectProvider(snapshot, options) {
    const providers = providerMap(snapshot);
    const enabledProviders = options && options.enabledProviders;
    const compactProvider = options && options.compactProvider;
    const providerIds = Object.keys(providers).filter(function(providerId) {
        return providerId !== "_meta"
            && (!enabledProviders || enabledProviders[providerId] !== false);
    }).sort();
    let selected = null;
    let zenFallback = null;
    let pinned = null;

    providerIds.forEach(function(providerId) {
        const provider = providers[providerId];
        const usage = quotaUsage(provider);
        if (usage !== null && (!selected || usage > selected.usage)) {
            selected = {
                providerId: providerId,
                value: remainingText(usage),
                usage: usage,
                stale: provider.stale === true,
            };
        }
        if (providerId === compactProvider && usage !== null) {
            pinned = {
                providerId: providerId,
                value: remainingText(usage),
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
            if (providerId === compactProvider) {
                pinned = zenFallback;
            }
        }
    });

    return pinned || selected || zenFallback || {
        providerId: "",
        value: "",
        usage: null,
        stale: false,
    };
}
