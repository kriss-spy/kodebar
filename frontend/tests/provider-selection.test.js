const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const helperPath = path.join(__dirname, "../package/contents/ui/ProviderSelection.js");
const source = fs.readFileSync(helperPath, "utf8").replace(/^\.pragma library\s*/, "");
const context = {};
vm.createContext(context);
vm.runInContext(source, context, { filename: helperPath });

const selected = context.selectProvider({
    _meta: { version: 1 },
    chatgpt: { type: "quota-based", limits: { codex: { primary: { usagePercent: 42 } } }, stale: false },
    opencode_go: {
        type: "quota-based",
        windows: { rolling: { usagePercent: 71 } },
        stale: false,
    },
    opencode_zen: { type: "pay-as-you-go", balanceFormatted: "$13.92", stale: false },
});

assert.equal(selected.providerId, "opencode_go");
assert.equal(selected.value, "29% left");
assert.equal(selected.stale, false);

const zenOnly = context.selectProvider({
    _meta: { version: 1 },
    opencode_zen: { type: "pay-as-you-go", balanceFormatted: "$8.50", stale: true },
});
assert.equal(zenOnly.providerId, "opencode_zen");
assert.equal(zenOnly.value, "$8.50");
assert.equal(zenOnly.stale, true);

assert.equal(context.selectProvider(null).providerId, "");
assert.equal(context.selectProvider({ _meta: { version: 1 } }).providerId, "");

const deterministicTie = context.selectProvider({
    _meta: { version: 1 },
    opencode_go: {
        type: "quota-based",
        windows: { rolling: { usagePercent: 50, status: "ok" } },
    },
    chatgpt: { type: "quota-based", limits: { codex: { primary: { usagePercent: 50 } } } },
});
assert.equal(deterministicTie.providerId, "chatgpt");

const noPriorData = context.selectProvider({
    _meta: { version: 1 },
    chatgpt: {
        type: "quota-based",
        limits: { codex: { primary: { usagePercent: 0 } } },
        stale: true,
        lastUpdated: null,
    },
    opencode_zen: {
        type: "pay-as-you-go",
        balanceFormatted: "$4.00",
        stale: false,
        lastUpdated: "2026-08-23T00:00:00Z",
    },
});
assert.equal(noPriorData.providerId, "opencode_zen");

const chatGptHighest = context.selectProvider({
    _meta: { version: 1 },
    chatgpt: {
        type: "quota-based",
        planType: "plus",
        limits: { codex: { primary: { usagePercent: 82.5 } } },
        stale: false,
    },
    opencode_go: {
        type: "quota-based",
        windows: { rolling: { usagePercent: 71, status: "ok" } },
        stale: false,
    },
});
assert.equal(chatGptHighest.providerId, "chatgpt");
assert.equal(chatGptHighest.value, "17.5% left");

const exhaustedGo = context.selectProvider({
    _meta: { version: 1 },
    opencode_go: {
        type: "quota-based",
        windows: { rolling: { usagePercent: 100, status: "rate-limited" } },
        stale: false,
    },
    chatgpt: { type: "quota-based", limits: { codex: { primary: { usagePercent: 60 } } }, stale: false },
});
assert.equal(exhaustedGo.providerId, "opencode_go");
assert.equal(exhaustedGo.value, "0% left");

const disabledHighest = context.selectProvider({
    chatgpt: { type: "quota-based", limits: { codex: { primary: { usagePercent: 90 } } }, stale: false },
    opencode_go: {
        type: "quota-based",
        windows: { rolling: { usagePercent: 40, status: "ok" } },
        stale: false,
    },
}, {
    enabledProviders: { chatgpt: false, opencode_go: true },
    compactProvider: "highest",
});
assert.equal(disabledHighest.providerId, "opencode_go");
assert.equal(disabledHighest.value, "60% left");

const pinnedProvider = context.selectProvider({
    chatgpt: { type: "quota-based", limits: { codex: { primary: { usagePercent: 90 } } }, stale: false },
    opencode_go: {
        type: "quota-based",
        windows: { rolling: { usagePercent: 40, status: "ok" } },
        stale: false,
    },
}, {
    enabledProviders: { chatgpt: true, opencode_go: true },
    compactProvider: "opencode_go",
});
assert.equal(pinnedProvider.providerId, "opencode_go");
assert.equal(pinnedProvider.value, "60% left");

const disabledPinFallsBack = context.selectProvider({
    chatgpt: { type: "quota-based", limits: { codex: { primary: { usagePercent: 90 } } }, stale: false },
    chatgpt: {
        type: "quota-based",
        limits: { codex: { primary: { usagePercent: 65 } } },
        stale: false,
    },
}, {
    enabledProviders: { chatgpt: false, chatgpt: true },
    compactProvider: "chatgpt",
});
assert.equal(disabledPinFallsBack.providerId, "chatgpt");
assert.equal(disabledPinFallsBack.value, "35% left");

const unavailablePinFallsBack = context.selectProvider({
    chatgpt: { type: "quota-based", limits: { codex: { primary: { usagePercent: 55 } } }, stale: false },
}, {
    enabledProviders: { chatgpt: true, opencode_go: true },
    compactProvider: "opencode_go",
});
assert.equal(unavailablePinFallsBack.providerId, "chatgpt");

const allDisabled = context.selectProvider({
    chatgpt: { type: "quota-based", limits: { codex: { primary: { usagePercent: 55 } } }, stale: false },
}, {
    enabledProviders: { chatgpt: false },
    compactProvider: "highest",
});
assert.equal(allDisabled.providerId, "");

const fractionalUsage = context.selectProvider({
    chatgpt: { type: "quota-based", limits: { codex: { primary: { usagePercent: 46.7 } } }, stale: false },
}, {
    enabledProviders: { chatgpt: true },
    compactProvider: "chatgpt",
});
assert.equal(fractionalUsage.value, "53.3% left");

// Regression: compact selection must use the worst window, matching the
// card header (mostUsed). A healthy rolling window must not mask a
// depleted weekly window, and a healthy primary must not mask secondary.
const worstWindowWins = context.selectProvider({
    _meta: { version: 1 },
    opencode_go: {
        type: "quota-based",
        windows: {
            rolling: { usagePercent: 4, status: "ok" },
            weekly: { usagePercent: 100, status: "rate-limited" },
            monthly: { usagePercent: 51, status: "ok" },
        },
        stale: false,
    },
    chatgpt: {
        type: "quota-based",
        limits: { codex: { primary: { usagePercent: 0 }, secondary: { usagePercent: 57 } } },
        stale: false,
    },
});
assert.equal(worstWindowWins.providerId, "opencode_go");
assert.equal(worstWindowWins.value, "0% left");
assert.equal(worstWindowWins.usage, 100);
