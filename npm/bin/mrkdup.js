#!/usr/bin/env node
// Runs the binary install.js unpacked into ../vendor/, passing every
// argument through and mirroring its exit status.
"use strict";
const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const exe = process.platform === "win32" ? "mrkdup.exe" : "mrkdup";
const bin = path.join(__dirname, "..", "vendor", exe);
if (!fs.existsSync(bin)) {
  process.stderr.write(
    "mrkdup: binary not found — the install step did not run or failed. " +
      "Reinstall with `npm install -g mrkdup`, or use `cargo install mrkdup`.\n"
  );
  process.exit(1);
}
const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  process.stderr.write(`mrkdup: ${result.error.message}\n`);
  process.exit(1);
}
process.exit(result.status === null ? 1 : result.status);
