const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const repositoryRoot = path.join(__dirname, "../..");
const outputDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "kodebar-package-test-"));

try {
    const result = spawnSync(
        path.join(repositoryRoot, "frontend/scripts/package-plasmoid.sh"),
        [outputDirectory],
        { cwd: repositoryRoot, encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);

    const artifact = result.stdout.trim();
    assert.equal(path.basename(artifact), "kodebar-0.2.0.plasmoid");

    const listing = spawnSync("unzip", ["-Z1", artifact], { encoding: "utf8" });
    assert.equal(listing.status, 0, listing.stderr);
    const entries = listing.stdout.trim().split("\n");
    for (const requiredEntry of [
        "metadata.json",
        "LICENSE",
        "NOTICE",
        "contents/ui/main.qml",
        "contents/images/provider-chatgpt.svg",
    ]) {
        assert.ok(entries.includes(requiredEntry), `${requiredEntry} missing from package`);
    }
    assert.ok(entries.every(entry => !entry.startsWith("frontend/package/")));

    const metadata = spawnSync("unzip", ["-p", artifact, "metadata.json"], { encoding: "utf8" });
    assert.equal(metadata.status, 0, metadata.stderr);
    assert.equal(JSON.parse(metadata.stdout).KPlugin.Version, "0.2.0");

    const installRoot = path.join(outputDirectory, "plasma/plasmoids");
    const install = spawnSync("kpackagetool6", [
        "--type", "Plasma/Applet",
        "--packageroot", installRoot,
        "--install", artifact,
    ], { encoding: "utf8" });
    assert.equal(install.status, 0, install.stderr || install.stdout);
    assert.ok(fs.existsSync(path.join(installRoot, "io.github.kriss_spy.kodebar/metadata.json")));
} finally {
    fs.rmSync(outputDirectory, { recursive: true, force: true });
}
