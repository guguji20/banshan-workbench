import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { access, copyFile, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const BUNDLE_SCRIPT = fileURLToPath(new URL("./new-windows-secondary-machine-acceptance-bundle.ps1", import.meta.url));
const RUNNER_SCRIPT = fileURLToPath(new URL("./invoke-windows-secondary-machine-acceptance.ps1", import.meta.url));
const POWERSHELL = join(process.env.SystemRoot ?? "C:\\Windows", "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
const UNSIGNED_CURRENT_PE = join(process.env.ProgramFiles ?? "C:\\Program Files", "Git", "usr", "bin", "true.exe");
const UNSIGNED_PREVIOUS_PE = join(process.env.ProgramFiles ?? "C:\\Program Files", "Git", "usr", "bin", "printf.exe");
const CURRENT_VERSION = "2.0.0";
const PREVIOUS_VERSION = "1.9.0";
const FACT_DATE = "2026-08-03";
const DATE_TOKEN = FACT_DATE.replaceAll("-", "");
const PRODUCT_NAME = "华邦互娱商务系统";
const TEMP_PREFIX = "bwr-";
const TEMP_MARKER = ".owned-by-secondary-runner-test";
const ownedRoots = new Map();

const ENGINE_STUB = String.raw`[CmdletBinding()]
param(
  [string]$InstallerPath,
  [string]$PreviousInstallerPath,
  [string]$Version,
  [int]$ColdStartCount,
  [int]$StartupObservationSeconds,
  [int]$ProcessExitTimeoutSeconds,
  [int]$InstallerTimeoutSeconds,
  [string]$RunId,
  [switch]$DryRun,
  [switch]$AllowExistingProductRegistration,
  [switch]$InjectFailureAfterUpgrade
)
$ErrorActionPreference = 'Stop'
if ($DryRun) { return }
$utf8 = New-Object System.Text.UTF8Encoding($false)
$bundleRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$runRoot = Join-Path $bundleRoot ('.runtime\nsis-acceptance\' + $RunId)
$manifestPath = Join-Path $runRoot 'data-rollback\backup-manifest.json'
$summaryPath = Join-Path $runRoot 'acceptance-summary.json'
New-Item -ItemType Directory -Path (Split-Path -Parent $manifestPath) -Force | Out-Null
$manifest = @([ordered]@{ relativePath = 'ledger/bsaigc.sqlite3'; length = 7; sha256 = ('a' * 64) })
[IO.File]::WriteAllText($manifestPath, (($manifest | ConvertTo-Json -Depth 8) + [Environment]::NewLine), $utf8)
if ($InjectFailureAfterUpgrade) {
  $steps = @('preflight','initial-install','data-backup','first-start','upgrade','data-rollback') | ForEach-Object { [ordered]@{ name = $_; status = 'passed' } }
  $steps += [ordered]@{ name = 'acceptance'; status = 'failed' }
  $summary = [ordered]@{
    status = 'failed'
    error = 'Injected failure after overwrite upgrade to verify isolated data rollback.'
    dataPreservation = [ordered]@{ injectFailureAfterUpgrade = $true; backupCreated = $true; rollbackAttempted = $true; rollbackCompleted = $true; rollbackError = $null; manifestPath = $manifestPath }
    uninstallCompleted = $true
    registryRestored = $true
    steps = $steps
  }
  [IO.File]::WriteAllText($summaryPath, (($summary | ConvertTo-Json -Depth 12) + [Environment]::NewLine), $utf8)
  throw 'Injected failure after overwrite upgrade to verify isolated data rollback.'
}
$steps = @('preflight','initial-install','data-backup','first-start','upgrade','restart','uninstall','registry-restore') | ForEach-Object { [ordered]@{ name = $_; status = 'passed' } }
$summary = [ordered]@{
  status = 'passed'
  upgradeKind = 'cross-version-upgrade'
  initialProductVersion = '${PREVIOUS_VERSION}'
  finalProductVersion = $Version
  coldStartCount = $ColdStartCount
  dataPreservation = [ordered]@{ backupCreated = $true; rollbackAttempted = $false; manifestPath = $manifestPath }
  steps = $steps
}
[IO.File]::WriteAllText($summaryPath, (($summary | ConvertTo-Json -Depth 12) + [Environment]::NewLine), $utf8)
`;

function sha256(content) {
  return createHash("sha256").update(content).digest("hex");
}

async function sha256File(path) {
  return sha256(await readFile(path));
}

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
  assert.match(basename(root), /^bwr-[^\\/]+$/u);
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

async function writeJson(path, value) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

async function writeChecksums(root, relativePaths) {
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
  const outputRoot = join(repo, ".runtime", "windows-secondary-machine");
  const currentInstallerName = `huabang-business-system-v${CURRENT_VERSION}-windows-x64-setup-unsigned.exe`;
  const previousInstallerName = `${PRODUCT_NAME}_${PREVIOUS_VERSION}_x64-setup.exe`;
  const currentInstaller = join(currentRelease, currentInstallerName);
  const previousInstaller = join(previousRelease, previousInstallerName);

  await mkdir(scripts, { recursive: true });
  await mkdir(docs, { recursive: true });
  await mkdir(currentRelease, { recursive: true });
  await mkdir(previousRelease, { recursive: true });
  await mkdir(outputRoot, { recursive: true });
  await copyFile(BUNDLE_SCRIPT, join(scripts, "new-windows-secondary-machine-acceptance-bundle.ps1"));
  await copyFile(RUNNER_SCRIPT, join(scripts, "invoke-windows-secondary-machine-acceptance.ps1"));
  await writeFile(join(scripts, "invoke-nsis-release-acceptance.ps1"), ENGINE_STUB, "utf8");
  await writeFile(join(docs, "WINDOWS_SECONDARY_MACHINE_ACCEPTANCE_20260729.md"), "# Synthetic acceptance guide\n", "utf8");
  await copyFile(UNSIGNED_CURRENT_PE, currentInstaller);
  await copyFile(UNSIGNED_PREVIOUS_PE, previousInstaller);

  const currentBytes = await readFile(currentInstaller);
  await writeJson(join(currentRelease, "release-manifest.json"), {
    schemaVersion: 1,
    version: CURRENT_VERSION,
    installer: { name: currentInstallerName, sizeBytes: currentBytes.length, sha256: sha256(currentBytes) },
  });
  await writeChecksums(currentRelease, [currentInstallerName, "release-manifest.json"]);
  await writeChecksums(previousRelease, [previousInstallerName]);

  const build = await run(POWERSHELL, [
    "-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", join(scripts, "new-windows-secondary-machine-acceptance-bundle.ps1"),
    "-Version", CURRENT_VERSION, "-PreviousVersion", PREVIOUS_VERSION, "-FactDate", FACT_DATE, "-OutputRoot", outputRoot,
  ], repo);
  assert.equal(build.code, 0, build.output);
  const buildResult = JSON.parse(build.stdout.trim());
  const bundleRoot = buildResult.bundleRoot;
  const manifestPath = join(bundleRoot, "bundle-manifest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.origin.machineIdSha256 = sha256("definitely-another-machine");
  manifest.origin.userSidSha256 = sha256("definitely-another-user");
  await writeJson(manifestPath, manifest);
  const checksumPath = join(bundleRoot, "SHA256SUMS.txt");
  const checksumLines = (await readFile(checksumPath, "utf8")).trim().split(/\r?\n/u);
  const updatedChecksums = checksumLines.map((line) => line.endsWith(" *bundle-manifest.json") ? `${sha256(JSON.stringify(manifest, null, 2) + "\n")} *bundle-manifest.json` : line);
  await writeFile(checksumPath, `${updatedChecksums.join("\n")}\n`, "utf8");
  return { root, repo, bundleRoot, runner: join(bundleRoot, "scripts", "invoke-windows-secondary-machine-acceptance.ps1") };
}

async function withFixture(callback) {
  const fixture = await createFixture();
  try {
    await callback(fixture);
  } finally {
    await removeOwnedRoot(fixture.root);
  }
}

async function runRunner(fixture, extraArgs = []) {
  return await run(POWERSHELL, [
    "-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", fixture.runner,
    "-FactDate", FACT_DATE, "-StartupObservationSeconds", "1", "-ProcessExitTimeoutSeconds", "1", "-InstallerTimeoutSeconds", "1",
    ...extraArgs,
  ], fixture.bundleRoot);
}

test("Windows prerequisites and PowerShell parser are available", async () => {
  assert.equal(process.platform, "win32");
  for (const path of [POWERSHELL, UNSIGNED_CURRENT_PE, UNSIGNED_PREVIOUS_PE, BUNDLE_SCRIPT, RUNNER_SCRIPT]) await access(path);
  const parsed = await run(POWERSHELL, [
    "-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
    `$tokens=$null;$errors=$null;[void][Management.Automation.Language.Parser]::ParseFile('${RUNNER_SCRIPT.replaceAll("'", "''")}',[ref]$tokens,[ref]$errors);if($errors.Count){exit 1}`,
  ], process.cwd());
  assert.equal(parsed.code, 0, parsed.output);
});

test("DryRun never marks Both, Upgrade, or Rollback as release-gate eligible", { timeout: 60_000 }, async () => {
  await withFixture(async (fixture) => {
    for (const mode of ["Both", "Upgrade", "Rollback"]) {
      const result = await runRunner(fixture, ["-Mode", mode, "-RunIdPrefix", `dry-${mode.toLowerCase()}`, "-ColdStartCount", "20", "-DryRun"]);
      assert.equal(result.code, 0, result.output);
      const summary = JSON.parse(result.stdout.trim());
      assert.equal(summary.mode, mode);
      assert.equal(summary.dryRun, true);
      assert.equal(summary.releaseGateEligible, false);
    }
    await assert.rejects(access(join(fixture.bundleRoot, "evidence")));
  });
});

test("Both mode copies both backup manifests, binds hashes, and becomes gate eligible", { timeout: 60_000 }, async () => {
  await withFixture(async (fixture) => {
    const prefix = "runner-pass";
    const result = await runRunner(fixture, ["-Mode", "Both", "-RunIdPrefix", prefix, "-ColdStartCount", "20"]);
    assert.equal(result.code, 0, result.output);
    const summary = JSON.parse(result.stdout.trim());
    assert.equal(summary.status, "passed");
    assert.equal(summary.releaseGateEligible, true);
    const evidenceRoot = join(fixture.bundleRoot, "evidence", `${prefix}-${CURRENT_VERSION}-${DATE_TOKEN}`);
    assert.equal(summary.evidenceRoot, evidenceRoot);
    const evidence = JSON.parse(await readFile(join(evidenceRoot, "secondary-machine-evidence.json"), "utf8"));
    assert.equal(evidence.releaseGateEligible, true);
    for (const [name, field] of [
      ["upgrade-backup-manifest.json", "upgradeBackupManifestSha256"],
      ["rollback-backup-manifest.json", "rollbackBackupManifestSha256"],
    ]) {
      const path = join(evidenceRoot, name);
      assert.equal(await sha256File(path), evidence[field]);
    }
    const checksumLines = (await readFile(join(evidenceRoot, "SHA256SUMS.txt"), "utf8")).trim().split(/\r?\n/u);
    assert.equal(checksumLines.length, 5);
    assert.ok(checksumLines.some((line) => line.endsWith(" *upgrade-backup-manifest.json")));
    assert.ok(checksumLines.some((line) => line.endsWith(" *rollback-backup-manifest.json")));
  });
});

test("existing evidence is rejected before the acceptance engine runs", { timeout: 60_000 }, async () => {
  await withFixture(async (fixture) => {
    const prefix = "runner-conflict";
    const evidenceRoot = join(fixture.bundleRoot, "evidence", `${prefix}-${CURRENT_VERSION}-${DATE_TOKEN}`);
    await mkdir(evidenceRoot, { recursive: true });
    await writeFile(join(evidenceRoot, "sentinel.txt"), "existing evidence", "utf8");
    const result = await runRunner(fixture, ["-Mode", "Both", "-RunIdPrefix", prefix, "-ColdStartCount", "20"]);
    assert.notEqual(result.code, 0, result.output);
    assert.match(result.output, /Evidence directory already exists/iu);
    await assert.rejects(access(join(fixture.bundleRoot, ".runtime", "nsis-acceptance", `${prefix}-${CURRENT_VERSION}-${DATE_TOKEN}-upgrade`)));
    assert.equal(await readFile(join(evidenceRoot, "sentinel.txt"), "utf8"), "existing evidence");
  });
});

test("cold-start and reserved-name gates reject unsafe runs", { timeout: 60_000 }, async () => {
  await withFixture(async (fixture) => {
    const coldStarts = await runRunner(fixture, ["-Mode", "Both", "-RunIdPrefix", "too-few", "-ColdStartCount", "19", "-DryRun"]);
    assert.notEqual(coldStarts.code, 0, coldStarts.output);
    assert.match(coldStarts.output, /ColdStartCount must be at least/iu);
    const reserved = await runRunner(fixture, ["-Mode", "Both", "-RunIdPrefix", "NUL", "-ColdStartCount", "20", "-DryRun"]);
    assert.notEqual(reserved.code, 0, reserved.output);
    assert.match(reserved.output, /reserved Windows device name/iu);
  });
});

test("source contract preserves preflight ordering and manifest lineage", async () => {
  const source = await readFile(RUNNER_SCRIPT, "utf8");
  assert.ok(source.indexOf("Evidence directory already exists") < source.indexOf("& $EnginePath @upgradeArgs"));
  assert.match(source, /releaseGateEligible = \$false/u);
  assert.match(source, /upgrade-backup-manifest\.json/u);
  assert.match(source, /rollback-backup-manifest\.json/u);
  assert.match(source, /Copied upgrade backup manifest SHA-256 mismatch/u);
  assert.match(source, /Copied rollback backup manifest SHA-256 mismatch/u);
});
