const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const helperPath = path.join(__dirname, "../package/contents/ui/UsagePresentation.js");
const source = fs.readFileSync(helperPath, "utf8").replace(/^\.pragma library\s*/, "");
const context = {};
vm.createContext(context);
vm.runInContext(source, context, { filename: helperPath });

assert.equal(context.state(0, false), "ok");
assert.equal(context.state(69.99, false), "ok");
assert.equal(context.state(70, false), "warning");
assert.equal(context.state(89.99, false), "warning");
assert.equal(context.state(90, false), "critical");
assert.equal(context.state(100, false), "critical");
assert.equal(context.state(95, true), "stale");
assert.equal(context.state(null, false), "unknown");
assert.equal(context.state(Number.NaN, false), "unknown");
