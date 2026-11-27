const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const helperPath = path.join(__dirname, "../package/contents/ui/ProviderCards.js");
const source = fs.readFileSync(helperPath, "utf8").replace(/^\.pragma library\s*/, "");
const context = { qsTr: value => value };
vm.createContext(context);
vm.runInContext(source, context, { filename: helperPath });

const cards = context.cards({
    _meta: { version: 1 },
    chatgpt: {
        type: "quota-based",
        planType: "plus",
        limits: {
            codex: {
                primary: { usagePercent: 25.5, windowDurationSec: 18000, resetAt: "2026-08-23T06:00:00Z" },
                secondary: { usagePercent: 40, windowDurationSec: 604800, resetAt: "2026-08-25T01:00:00Z" },
            },
        },
        stale: false,
        lastUpdated: "2026-08-23T00:00:00Z",
    },
}, Date.parse("2026-08-23T01:00:00Z"));

assert.equal(cards.length, 1);
assert.equal(cards[0].providerId, "chatgpt");
assert.equal(cards[0].title, "ChatGPT");
assert.equal(cards[0].detailText, "Plus plan");
assert.equal(cards[0].rows.length, 2);
assert.deepEqual(JSON.parse(JSON.stringify(cards[0].rows[0])), {
    label: "Session",
    usagePercent: 25.5,
    resetText: "Resets in 5h",
    statusText: "",
});
assert.equal(cards[0].rows[1].usagePercent, 40);

const goCard = context.cards({
    _meta: { version: 1 },
    opencode_go: {
        type: "quota-based",
        windows: {
            rolling: { usagePercent: 14, resetAt: "2026-08-23T01:30:00Z", status: "ok" },
            weekly: { usagePercent: 47, resetAt: "2026-08-25T01:00:00Z", status: "ok" },
            monthly: { usagePercent: 100, resetAt: "2026-09-23T01:00:00Z", status: "rate-limited" },
        },
        stale: false,
        lastUpdated: "2026-08-23T00:55:00Z",
    },
}, Date.parse("2026-08-23T01:00:00Z"))[0];

assert.equal(goCard.providerId, "opencode_go");
assert.equal(goCard.title, "OpenCode Go");
assert.deepEqual(Array.from(goCard.rows, row => row.label), ["Rolling 5 hours", "Weekly", "Monthly"]);
assert.deepEqual(Array.from(goCard.rows, row => row.usagePercent), [14, 47, 100]);
assert.equal(goCard.rows[0].resetText, "Resets in 30m");
assert.equal(goCard.rows[2].statusText, "Rate limited");

const missingGoWindows = context.cards({
    opencode_go: {
        type: "quota-based",
        windows: {},
        stale: true,
        error: "OpenCode API key was rejected; run login",
    },
}, Date.now())[0];
assert.equal(missingGoWindows.rows.length, 0);
assert.equal(missingGoWindows.connected, false);
assert.equal(missingGoWindows.actionLabel, "Reconnect OpenCode");

const filteredCards = context.cards({
    chatgpt: { type: "quota-based", limits: {} },
    opencode_go: { type: "quota-based", windows: {} },
}, Date.now(), {
    antigravity: false,
    chatgpt: false,
    x_premium: false,
    fish_audio: false,
    opencode_go: true,
    opencode_zen: false,
});
assert.deepEqual(JSON.parse(JSON.stringify(filteredCards.map(card => card.providerId))), ["opencode_go"]);

assert.equal(context.countdown("not-a-timestamp", Date.now()), "No reset time");
assert.equal(context.ageText("not-a-timestamp", Date.now()), "Never");

const setupCards = context.cards({ _meta: { version: 1 } }, Date.now(), {
    antigravity: false,
    chatgpt: false,
    x_premium: false,
    fish_audio: false,
    opencode_go: true,
    opencode_zen: false,
});
assert.equal(setupCards.length, 1);
assert.equal(setupCards[0].providerId, "opencode_go");
assert.equal(setupCards[0].detailText, "Connect your OpenCode account to see Go plan limits");
assert.equal(setupCards[0].actionLabel, "Start guided login");
assert.equal(setupCards[0].stale, false);

const zenCard = context.cards({
    _meta: { version: 1 },
    opencode_zen: {
        type: "pay-as-you-go",
        balance: -1392399000,
        balanceFormatted: "$13.92",
        useBalance: true,
        reloadAmount: 20,
        reloadTrigger: 5,
        stale: false,
    },
}, Date.now())[0];

assert.equal(zenCard.providerId, "opencode_zen");
assert.equal(zenCard.title, "OpenCode Zen");
assert.equal(zenCard.detailText, "Balance $13.92");
assert.equal(zenCard.secondaryText, "Auto-reload $20 at $5");
assert.equal(zenCard.rows.length, 0);

const chatGptCard = context.cards({
    _meta: { version: 1 },
    chatgpt: {
        type: "quota-based",
        planType: "plus",
        limits: {
            codex: {
                primary: { usagePercent: 25.5, windowDurationSec: 18000, resetAt: "2026-08-23T06:00:00Z" },
                secondary: { usagePercent: 40, windowDurationSec: 604800, resetAt: "2026-08-25T01:00:00Z" },
            },
            codex_review: {
                limitName: "Code review",
                primary: { usagePercent: 71, windowDurationSec: 86400, resetAt: "2026-08-24T01:00:00Z" },
            },
        },
        stale: true,
        lastUpdated: "2026-08-23T00:55:00Z",
        error: "provider returned HTTP 403",
    },
}, Date.parse("2026-08-23T01:00:00Z"))[0];

assert.equal(chatGptCard.providerId, "chatgpt");
assert.equal(chatGptCard.title, "ChatGPT");
assert.equal(chatGptCard.detailText, "Plus plan");
assert.deepEqual(Array.from(chatGptCard.rows, row => row.label), [
    "Session",
    "Weekly",
    "Code review · Session",
]);
assert.deepEqual(Array.from(chatGptCard.rows, row => row.usagePercent), [25.5, 40, 71]);
assert.equal(chatGptCard.rows[2].resetText, "Resets in 1d");
assert.equal(chatGptCard.stale, true);
assert.equal(chatGptCard.lastUpdated, "2026-08-23T00:55:00Z");
assert.equal(chatGptCard.error, "Sign-in expired. Reconnect to resume updates.");
assert.equal(chatGptCard.actionLabel, "Reconnect ChatGPT");

assert.equal(context.ageText("2026-08-23T00:55:00Z", Date.parse("2026-08-23T01:00:00Z")), "5m ago");
assert.equal(context.ageText("", Date.now()), "Never");
assert.equal(context.providerCompactTitle("opencode_go"), "Go");
assert.equal(context.providerCompactTitle("opencode_zen"), "Zen");
assert.equal(context.iconSource("unknown"), "view-statistics");

const antigravity = context.cards({ antigravity: {
    type: "quota-based", usagePercentage: 80,
    quotaGroups: [{ label: "Gemini", windows: {
        session: { label: "5-hour", usagePercent: 25, resetAt: "2026-08-23T06:00:00Z" },
        weekly: { label: "Weekly", usagePercent: 80 },
    } }], stale: true, lastUpdated: "2026-08-23T00:00:00Z",
}}, Date.parse("2026-08-23T01:00:00Z"))[0];
assert.equal(antigravity.title, "Antigravity");
assert.equal(antigravity.rows[0].label, "Gemini · 5-hour");
assert.equal(antigravity.rows[0].resetText, "Resets in 5h");
assert.equal(antigravity.rows[1].usagePercent, 80);
assert.equal(antigravity.stale, true);
assert.equal(context.setupCard("antigravity").actionLabel, "Sign in with agy");

const xCard = context.card("x_premium", {
    windows: { weekly: { usagePercent: 6.7, resetAt: "2026-12-01T12:48:28Z" } },
    products: [{ product: "grok-bot", usagePercent: 5.7 }, { product: "grok-chat", usagePercent: 1 }],
}, Date.parse("2026-11-27T12:48:28Z"));
assert.equal(xCard.rows.length, 1);
assert.equal(xCard.rows[0].label, "Weekly");
assert.equal(xCard.rows[0].usagePercent, 6.7);
assert.match(xCard.secondaryText, /Grok Bot: 5.7% of weekly allowance/);
assert.match(xCard.secondaryText, /Chat: 1% of weekly allowance/);

const fish = context.card("fish_audio", { plan: "plus", creditsRemaining: 250555, extraCredits: 1000,
    teamName: "My Team", resetAt: "2026-12-27T12:00:30Z" }, Date.parse("2026-11-27T12:00:30Z"));
assert.equal(fish.rows.length, 1);
assert.equal(fish.rows[0].usagePercent, 0);
assert.equal(fish.rows[0].statusText, "Balance exceeds plan allowance");
assert.equal(fish.detailText, "Plus plan · 250555 credits remaining");
assert.match(fish.secondaryText, /1000 extra credits/);
assert.equal(context.card("fish_audio", {stale: true}, Date.now()).detailText, "Unavailable");

const plusCredits = { plan: "plus", creditsRemaining: 249622, extraCredits: 1000 };
assert.ok(Math.abs(context.fishAudioRows(plusCredits, Date.now())[0].usagePercent - 0.1512) < 0.000001);
assert.equal(context.fishAudioRows({ plan: "custom", creditsRemaining: 10 }, Date.now()).length, 0);
assert.equal(context.fishAudioRows({ plan: "plus", creditsRemaining: 0 }, Date.now())[0].usagePercent, 100);
assert.equal(context.fishAudioRows({ ...plusCredits, extraCredits: 2000000 }, Date.now())[0].usagePercent,
    context.fishAudioRows(plusCredits, Date.now())[0].usagePercent);

const reorderedIds = context.providerIds("chatgpt,fish_audio,chatgpt,unknown");
assert.equal(reorderedIds[0], "chatgpt");
assert.equal(reorderedIds[1], "fish_audio");
assert.equal(reorderedIds.filter(id => id === "chatgpt").length, 1);
assert.equal(reorderedIds.includes("unknown"), false);
assert.equal(reorderedIds.includes("x_premium"), true);
const reorderedCards = context.cards({ chatgpt: {limits: {}}, fish_audio: {creditsRemaining: 1} }, Date.now(), null, "chatgpt,fish_audio");
assert.equal(reorderedCards[0].providerId, "chatgpt");
assert.equal(reorderedCards[1].providerId, "fish_audio");
