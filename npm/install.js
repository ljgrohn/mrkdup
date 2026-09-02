#!/usr/bin/env node
// postinstall: fetch the prebuilt mrkdup binary for this platform from
// the GitHub release that matches this package's version, and unpack it
// into ./vendor/. bin/mrkdup.js runs it from there.
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const { execFileSync } = require("child_process");
const { version } = require("./package.json");

const REPO = "ljgrohn/mrkdup";

function target() {
  const arch = { x64: "x86_64", arm64: "aarch64" }[process.arch];
  if (!arch) throw new Error(`unsupported CPU architecture: ${process.arch}`);
  switch (process.platform) {
    case "darwin":
      return { triple: `${arch}-apple-darwin`, ext: "tar.gz", exe: "mrkdup" };
    case "linux":
      return { triple: `${arch}-unknown-linux-musl`, ext: "tar.gz", exe: "mrkdup" };
    case "win32":
      if (arch !== "x86_64") throw new Error("only x64 Windows builds are published");
      return { triple: "x86_64-pc-windows-msvc", ext: "zip", exe: "mrkdup.exe" };
    default:
      throw new Error(`unsupported platform: ${process.platform}`);
  }
}

async function main() {
  const { triple, ext, exe } = target();
  const name = `mrkdup-v${version}-${triple}`;
  const url = `https://github.com/${REPO}/releases/download/v${version}/${name}.${ext}`;
  const vendor = path.join(__dirname, "vendor");
  fs.mkdirSync(vendor, { recursive: true });
  const archive = path.join(vendor, `${name}.${ext}`);

  process.stderr.write(`mrkdup: downloading ${url}\n`);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`download failed: ${res.status} ${res.statusText} for ${url}`);
  fs.writeFileSync(archive, Buffer.from(await res.arrayBuffer()));

  // bsdtar on macOS and Windows 10+, GNU tar on Linux: all read both formats
  execFileSync("tar", ["-xf", archive, "-C", vendor], { stdio: "inherit" });
  const from = path.join(vendor, name, exe);
  const to = path.join(vendor, exe);
  fs.copyFileSync(from, to);
  if (process.platform !== "win32") fs.chmodSync(to, 0o755);
  fs.rmSync(path.join(vendor, name), { recursive: true, force: true });
  fs.rmSync(archive, { force: true });
  process.stderr.write(`mrkdup: installed ${to}\n`);
}

main().catch((err) => {
  process.stderr.write(`mrkdup: could not install the prebuilt binary: ${err.message}\n`);
  process.stderr.write(
    `mrkdup: install it another way instead — \`cargo install mrkdup\`, or grab a binary from https://github.com/${REPO}/releases\n`
  );
  process.exit(1);
});
