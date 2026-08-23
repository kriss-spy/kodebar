const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const packageRoot = path.join(__dirname, "../package");
const helperPath = path.join(packageRoot, "contents/ui/ProviderCards.js");
const source = fs.readFileSync(helperPath, "utf8").replace(/^\.pragma library\s*/, "");
const context = { qsTr: value => value };
vm.createContext(context);
vm.runInContext(source, context, { filename: helperPath });

for (const providerId of ["antigravity", "opencode_go", "opencode_zen", "chatgpt"]) {
    const assetPath = path.resolve(path.dirname(helperPath), context.iconSource(providerId));
    const svg = fs.readFileSync(assetPath, "utf8");
    assert.match(svg, /^<svg /);
    assert.match(svg, /<style id="current-color-scheme" type="text\/css">/);
    assert.match(svg, /class="ColorScheme-Text"/);
    assert.match(svg, /fill="currentColor"/);
}

const notice = fs.readFileSync(path.join(packageRoot, "NOTICE"), "utf8");
assert.match(notice, /CodexBar/);
assert.match(notice, /27c7f334e3c46c96ff8c063afbe0c7944ba5e0b7/);
assert.match(notice, /MIT License/);
