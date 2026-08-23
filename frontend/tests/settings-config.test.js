const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const packageRoot = path.join(__dirname, "../package/contents");
const schema = fs.readFileSync(path.join(packageRoot, "config/main.xml"), "utf8");
const page = fs.readFileSync(path.join(packageRoot, "ui/configGeneral.qml"), "utf8");

for (const providerKey of [
    "enabledAntigravity",
    "enabledChatGPT",
    "enabledOpenCodeGo",
    "enabledOpenCodeZen",
]) {
    assert.match(schema, new RegExp(`<entry name="${providerKey}" type="Bool">`));
    assert.match(page, new RegExp(`property alias cfg_${providerKey}:`));
}

assert.match(schema, /<entry name="refreshIntervalSeconds" type="Int">/);
assert.match(schema, /<entry name="compactProvider" type="String">/);
assert.match(schema, /<default>highest<\/default>/);
assert.match(page, /property alias cfg_refreshIntervalSeconds:/);
assert.match(page, /property string cfg_compactProvider/);
