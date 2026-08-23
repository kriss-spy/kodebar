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
    antigravity: { type: "quota-based", usagePercentage: 42, stale: false },
    opencode_go: {
        type: "quota-based",
        windows: { rolling: { usagePercent: 71 } },
        stale: false,
    },
    opencode_zen: { type: "pay-as-you-go", balanceFormatted: "$13.92", stale: false },
});

assert.equal(selected.providerId, "opencode_go");
assert.equal(selected.value, "71%");
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
    antigravity: { type: "quota-based", usagePercentage: 50 },
});
assert.equal(deterministicTie.providerId, "antigravity");

const noPriorData = context.selectProvider({
    _meta: { version: 1 },
    antigravity: {
        type: "quota-based",
        usagePercentage: 0,
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
assert.equal(chatGptHighest.value, "83%");

const exhaustedGo = context.selectProvider({
    _meta: { version: 1 },
    opencode_go: {
        type: "quota-based",
        windows: { rolling: { usagePercent: 100, status: "rate-limited" } },
        stale: false,
    },
    antigravity: { type: "quota-based", usagePercentage: 60, stale: false },
});
assert.equal(exhaustedGo.providerId, "opencode_go");
assert.equal(exhaustedGo.value, "100%");
