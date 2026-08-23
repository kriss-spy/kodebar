const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const packageRoot = path.join(__dirname, "../package");
const metadata = JSON.parse(fs.readFileSync(path.join(packageRoot, "metadata.json"), "utf8"));

assert.equal(metadata.KPackageStructure, "Plasma/Applet");
assert.equal(metadata.KPlugin.Id, "io.github.kriss_spy.kodebar");
assert.equal(metadata["X-Plasma-API-Minimum-Version"], "6.0");
assert.equal(metadata["X-Plasma-MainScript"], undefined);
assert.match(metadata.KPlugin.Version, /^\d+\.\d+\.\d+$/);
assert.equal(metadata.KPlugin.License, "MIT");
assert.equal(metadata.KPlugin.BugReportUrl, "https://github.com/kriss-spy/kodebar/issues");
assert.ok(fs.existsSync(path.join(packageRoot, "LICENSE")));
assert.ok(fs.existsSync(path.join(packageRoot, "NOTICE")));
