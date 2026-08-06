import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { access, copyFile, mkdir, mkdtemp, readFile, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const SOURCE_SCRIPT = fileURLToPath(new URL("./new-windows-secondary-machine-acceptance-bundle.ps1", import.meta.url));
const RUNNER_SCRIPT = fileURLToPath(new URL("./invoke-windows-secondary-machine-acceptance.ps1", import.meta.url));
const POWERSHELL = join(process.env.SystemRoot ?? "C:\\Windows", "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
const UNSIGNED_CURRENT_PE = join(process.env.ProgramFiles ?? "C:\\Program Files", "Git", "usr", "bin", "true.exe");
const UNSIGNED_PREVIOUS_PE = join(process.env.ProgramFiles ?? "C:\\Program Files", "Git", "usr", "bin", "printf.exe");
const TEMP_PREFIX = "bsaigc-secondary-bundle-test-";
const TEMP_MARKER = ".owned-by-secondary-bundle-test";
const CURRENT_VERSION = "2.0.0";
const PREVIOUS_VERSION = "1.9.0";
const FACT_DATE = "2026-08-03";
const PRODUCT_NAME = "华邦互娱商务系统";
const ownedRoots = new Map();

async function run(command, args, cwd) {
  return await new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, env: process.env, shell: false, windowsHide: true });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", reject);
    child.on("close", (code, signal) => resolve({ code, signal, stdout, stderr, output: `${stdout}\n${stderr}` }));
  });
}

async function createOwnedRoot() {
  const parent = await realpath(tmpdir());
  const root = await realpath(await mkdtemp(join(parent, TEMP_PREFIX)));
  const marker = randomUUID();
  assert.equal(dirname(root), parent);
  assert.match(basename(root), /^bsaigc-secondary-bundle-test-[^\\/]+$/u);
  await writeFile(join(root, TEMP_MARKER), marker, "utf8");
  ownedRoots.set(root, marker);
  return root;
}

async function removeOwnedRoot(root) {
  const marker = ownedRoots.get(root);
  assert.ok(marker, `refusing to remove unowned temp root: ${root}`);
  const parent = await realpath(tmpdir());
  const relativeRoot = relative(parent, root);
  assert.equal(dirname(root), parent);
  assert.equal(isAbsolute(relativeRoot), false);
  assert.equal(relativeRoot.startsWith(".."), false);
  assert.equal(await readFile(join(root, TEMP_MARKER), "utf8"), marker);
  await rm(root, { recursive: true, force: true });
  ownedRoots.delete(root);
}

function sha256(content) {
  return createHash("sha256").update(content).digest("hex");
}

async function sha256File(path) {
  return sha256(await readFile(path));
}

async function writeJson(path, value) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

async function writeChecksumFile(root, relativePaths) {
  const lines = [];
  for (const relativePath of relativePaths) {
    lines.push(`${await sha256File(join(root, ...relativePath.split("/")))} *${relativePath}`);
  }
  await writeFile(join(root, "SHA256SUMS.txt"), `${lines.join("\n")}\n`, "utf8");
}
async function createFixture() {
  const root = await createOwnedRoot();
  const repo = join(root, "repo");
  const scripts = join(repo, "scripts");
  const docs = join(repo, "docs");
  const currentRelease = join(repo, "release", CURRENT_VERSION);
  const previousRelease = join(repo, "release", PREVIOUS_VERSION);
  const outputRoot = join(repo, ".runtime", "windows-secondary-machine", "tests");
  const currentInstallerName = `huabang-business-system-v${CURRENT_VERSION}-windows-x64-setup-unsigned.exe`;
  const previousInstallerName = `${PRODUCT_NAME}_${PREVIOUS_VERSION}_x64-setup.exe`;
  const currentInstaller = join(currentRelease, currentInstallerName);
  const previousInstaller = join(previousRelease, previousInstallerName);

  await mkdir(scripts, { recursive: true });
  await mkdir(docs, { recursive: true });
  await mkdir(currentRelease, { recursive: true });
  await mkdir(previousRelease, { recursive: true });
  await mkdir(outputRoot, { recursive: true });
  await copyFile(SOURCE_SCRIPT, join(scripts, "new-windows-secondary-machine-acceptance-bundle.ps1"));
  await copyFile(RUNNER_SCRIPT, join(scripts, "invoke-windows-secondary-machine-acceptance.ps1"));
  await writeFile(join(scripts, "invoke-nsis-release-acceptance.ps1"), "param()\n", "utf8");
  await writeFile(join(docs, "WINDOWS_SECONDARY_MACHINE_ACCEPTANCE_20260729.md"), "# Synthetic acceptance guide\n", "utf8");
  await copyFile(UNSIGNED_CURRENT_PE, currentInstaller);
  await copyFile(UNSIGNED_PREVIOUS_PE, previousInstaller);

  const currentBytes = await readFile(currentInstaller);
  await writeJson(join(currentRelease, "release-manifest.json"), {
    schemaVersion: 1,
    version: CURRENT_VERSION,
    installer: { name: currentInstallerName, sizeBytes: currentBytes.length, sha256: sha256(currentBytes) },
  });
  await writeChecksumFile(currentRelease, [currentInstallerName, "release-manifest.json"]);
  await writeChecksumFile(previousRelease, [previousInstallerName]);
  return { root, repo, currentRelease, outputRoot, script: join(scripts, "new-windows-secondary-machine-acceptance-bundle.ps1") };
}

async function withFixture(callback) {
  const fixture = await createFixture();
  try {
    await callback(fixture);
  } finally {
    await removeOwnedRoot(fixture.root);
  }
}

async function runBundle(fixture, extraArgs = []) {
  return await run(POWERSHELL, [
    "-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", fixture.script,
    "-Version", CURRENT_VERSION, "-PreviousVersion", PREVIOUS_VERSION, "-FactDate", FACT_DATE,
    "-OutputRoot", fixture.outputRoot, ...extraArgs,
  ], fixture.repo);
}

test("Windows prerequisites are available", async () => {
  assert.equal(process.platform, "win32");
  await access(POWERSHELL);
  await access(UNSIGNED_CURRENT_PE);
  await access(UNSIGNED_PREVIOUS_PE);
  await access(SOURCE_SCRIPT);
  await access(RUNNER_SCRIPT);
  for (const unsignedPe of [UNSIGNED_CURRENT_PE, UNSIGNED_PREVIOUS_PE]) {
    const signature = await run(POWERSHELL, [
      "-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
      `(Get-AuthenticodeSignature -LiteralPath '${unsignedPe.replaceAll("'", "''")}').Status.ToString()`,
    ], process.cwd());
    assert.equal(signature.code, 0, signature.output);
    assert.equal(signature.stdout.trim(), "NotSigned");
  }
});
test("DryRun remains repeatable when the planned bundle and ZIP already exist", { timeout: 30_000 }, async () => {
  await withFixture(async (fixture) => {
    const bundleName = `windows-secondary-machine-acceptance-${CURRENT_VERSION}-${FACT_DATE.replaceAll("-", "")}`;
    const bundleRoot = join(fixture.outputRoot, bundleName);
    const zipPath = `${bundleRoot}.zip`;
    await mkdir(bundleRoot, { recursive: true });
    await writeFile(join(bundleRoot, "sentinel.txt"), "must remain untouched", "utf8");
    await writeFile(zipPath, "existing synthetic zip", "utf8");

    for (let attempt = 0; attempt < 2; attempt += 1) {
      const result = await runBundle(fixture, ["-DryRun"]);
      assert.equal(result.code, 0, result.output);
      const plan = JSON.parse(result.stdout.trim());
      assert.equal(plan.dryRun, true);
      assert.equal(plan.bundleRoot, bundleRoot);
      assert.equal(plan.zipPath, zipPath);
    }
    assert.equal(await readFile(join(bundleRoot, "sentinel.txt"), "utf8"), "must remain untouched");
    assert.equal(await readFile(zipPath, "utf8"), "existing synthetic zip");
  });
});

test("release checksum paths cannot escape the release directory", { timeout: 30_000 }, async () => {
  await withFixture(async (fixture) => {
    const outsidePath = join(fixture.repo, "release", "outside.bin");
    await writeFile(outsidePath, "outside", "utf8");
    const checksumPath = join(fixture.currentRelease, "SHA256SUMS.txt");
    const validChecksums = await readFile(checksumPath, "utf8");
    await writeFile(checksumPath, `${validChecksums}${await sha256File(outsidePath)} *../outside.bin\n`, "utf8");
    const result = await runBundle(fixture, ["-DryRun"]);
    assert.notEqual(result.code, 0, result.output);
    assert.match(result.output, /path escapes/iu);
  });
});

test("duplicate release checksum paths are rejected", { timeout: 30_000 }, async () => {
  await withFixture(async (fixture) => {
    const checksumPath = join(fixture.currentRelease, "SHA256SUMS.txt");
    const checksums = await readFile(checksumPath, "utf8");
    const firstLine = checksums.trim().split(/\r?\n/u)[0];
    await writeFile(checksumPath, `${checksums}${firstLine}\n`, "utf8");
    const result = await runBundle(fixture, ["-DryRun"]);
    assert.notEqual(result.code, 0, result.output);
    assert.match(result.output, /Duplicate release checksum path/iu);
  });
});

test("bundle output through a reparse point is rejected", { timeout: 30_000 }, async () => {
  await withFixture(async (fixture) => {
    const target = join(fixture.repo, ".runtime", "junction-target");
    const junction = join(fixture.repo, ".runtime", "windows-secondary-machine", "junction-output");
    await mkdir(target, { recursive: true });
    await symlink(target, junction, "junction");
    fixture.outputRoot = junction;
    const result = await runBundle(fixture, ["-DryRun"]);
    assert.notEqual(result.code, 0, result.output);
    assert.match(result.output, /Reparse point rejected/iu);
  });
});
