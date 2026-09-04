const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const helperPath = path.join(__dirname, "../package/contents/ui/WheelNavigation.js");
const source = fs.readFileSync(helperPath, "utf8").replace(/^\.pragma library\s*/, "");
const context = {};
vm.createContext(context);
vm.runInContext(source, context, { filename: helperPath });

assert.deepEqual(
    JSON.parse(JSON.stringify(context.consumeDelta(120, 0, 120))),
    { steps: 1, remainder: 0 },
);
assert.deepEqual(
    JSON.parse(JSON.stringify(context.consumeDelta(-240, 0, 120))),
    { steps: -2, remainder: 0 },
);
assert.deepEqual(
    JSON.parse(JSON.stringify(context.consumeDelta(15, 90, 120))),
    { steps: 0, remainder: 105 },
);
assert.deepEqual(
    JSON.parse(JSON.stringify(context.consumeDelta(20, 105, 120))),
    { steps: 1, remainder: 5 },
);
assert.deepEqual(
    JSON.parse(JSON.stringify(context.consumeDelta(-20, -105, 120))),
    { steps: -1, remainder: -5 },
);
