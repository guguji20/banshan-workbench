import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { verifyFormalReleaseEvidence } from "./verify-formal-release-evidence.mjs";

const commit = "0123456789abcdef0123456789abcdef01234567";
const version = "1.3.4";
const dmgName = `huabang-business-system-${version}-aarch64-apple-darwin-${commit.slice(0, 12)}.dmg`;

async function digest(filePath) {
  return createHash("sha256").update(await fs.readFile(filePath)).digest("hex");
}

async function writeAsset(root, name, content) {
  const filePath = path.join(root, name);
  await fs.writeFile(filePath, content);
  return { name, sizeBytes: Buffer.byteLength(content), sha256: await digest(filePath) };
}

async function fileRecord(root, name) {
  const filePath = path.join(root, name);
  const metadata = await fs.stat(filePath);
  return { name, sizeBytes: metadata.size, sha256: await digest(filePath) };
}

async function writeJson(filePath, value) {
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

async function createFixture() {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "bsaigc-formal-release-"));
  const windowsDir = path.join(root, "windows");
  const macosDir = path.join(root, "macos");
  const evidenceDir = path.join(root, "release-evidence");
  const docsDir = path.join(root, "docs");
  await Promise.all([windowsDir, macosDir, evidenceDir, docsDir].map((directory) => fs.mkdir(directory, { recursive: true })));

  const installer = await writeAsset(windowsDir, "banshan-setup.exe", "signed-windows-installer");
  const sourceSnapshot = await writeAsset(windowsDir, "banshan-source.zip", "source-snapshot");
  const dmg = await writeAsset(macosDir, dmgName, "signed-notarized-dmg");
  await fs.writeFile(path.join(macosDir, `${dmgName}.sha256`), `${dmg.sha256}  ${dmgName}\n`);
  const smokeLog = await writeAsset(macosDir, "macos-smoke.log", "startup smoke passed");
  const buildLog = await writeAsset(macosDir, "macos-build.log", "tauri build completed");
  const gateChecksLog = await writeAsset(macosDir, "macos-gate-checks.log", "codesign, spctl, and stapler passed");
  const dmgChecksum = await fileRecord(macosDir, `${dmgName}.sha256`);
  const windowsManifest = {
    schemaVersion: 1,
    artifactKind: "windows-release-gate-evidence",
    distributionAllowed: false,
    version,
    gitCommit: commit,
    platform: "windows-x86_64",
    installer: { ...installer, authenticode: "Valid" },
    sourceSnapshot,
    gates: { releaseVerify: true, authenticode: true, nsisInstall: true, launchSmoke: true, sourceSnapshotSecurity: true },
  };
  const macosGates = {
    codesign: "passed",
    notarization: "passed",
    gatekeeper: "passed",
    appStapler: "passed",
    dmgStapler: "passed",
    sidecar: { status: "passed", executable: true, version: "codex-cli 0.144.5" },
    startupSmoke: { status: "passed", durationSeconds: 12, log: smokeLog },
  };
  const macosManifest = {
    schemaVersion: 1,
    artifactKind: "macos-release-gate-evidence",
    distributionAllowed: false,
    factDate: "2026-08-05",
    status: "passed",
    product: "华邦互娱商务系统",
    version,
    gitCommit: commit,
    platform: "macos",
    target: "aarch64-apple-darwin",
    appBundleId: "com.banshan.workbench",
    dmg,
    gates: macosGates,
    evidence: { buildLog, gateChecksLog },
    workflow: { runId: "123", runAttempt: "1", ref: "refs/heads/main" },
    checksumFile: "SHA256SUMS-macos.txt",
    files: [dmg, dmgChecksum, smokeLog, buildLog, gateChecksLog],
    createdAtUtc: "2026-08-05T12:00:00Z",
  };
  const windowsManifestPath = path.join(windowsDir, "windows-gate-evidence.json");
  const macosManifestPath = path.join(macosDir, "macos-release-manifest.json");
  await writeJson(windowsManifestPath, windowsManifest);
  await writeJson(macosManifestPath, macosManifest);
  const checksumNames = [dmgName, `${dmgName}.sha256`, "macos-release-manifest.json", "macos-smoke.log", "macos-build.log", "macos-gate-checks.log"];
  const checksumLines = [];
  for (const name of checksumNames) checksumLines.push(`${(await fileRecord(macosDir, name)).sha256}  ${name}`);
  await fs.writeFile(path.join(macosDir, "SHA256SUMS-macos.txt"), `${checksumLines.join("\n")}\n`);

  const acceptance = {
    schemaVersion: 1,
    artifactKind: "business-workbench-1.0-final-acceptance",
    productRelease: "business-workbench-1.0",
    binaryVersion: version,
    releaseTag: `v${version}`,
    gitCommit: commit,
    releaseAuthorized: true,
    releaseNotesPath: "docs/BUSINESS_WORKBENCH_1.0_FORMAL_RELEASE_NOTES.md",
    gates: {
      windowsSecondaryMachine: { passed: true, metrics: { machineCount: 2 }, evidence: ["windows-machine-2.json"] },
      windowsDifferentUser: { passed: true, metrics: { userCount: 2 }, evidence: ["windows-user-2.json"] },
      windowsColdStarts: { passed: true, metrics: { totalColdStarts: 20 }, evidence: ["windows-cold-starts.json"] },
      upgradeRollback: { passed: true, evidence: ["upgrade-rollback.json"] },
      dataIntegrity: { passed: true, evidence: ["data-integrity.json"] },
      macosArm64: { passed: true, evidence: ["macos-smoke.json"] },
      businessPilot: { passed: true, metrics: { caseCount: 5 }, evidence: ["business-pilot.json"] },
      securityReview: { passed: true, evidence: ["security-review.json"] },
      releaseNotes: { passed: true, evidence: ["release-notes-review.json"] },
    },
  };
  for (const gate of Object.values(acceptance.gates)) {
    for (const evidence of gate.evidence) await fs.writeFile(path.join(evidenceDir, evidence), `${evidence}: verified\n`);
  }
  const acceptanceFile = path.join(evidenceDir, "business-workbench-1.0-final-gates.json");
  const releaseNotesFile = path.join(docsDir, "BUSINESS_WORKBENCH_1.0_FORMAL_RELEASE_NOTES.md");
  await writeJson(acceptanceFile, acceptance);
  await fs.writeFile(releaseNotesFile, "# Business Workbench 1.0\n\n- `releaseStatus: formal-1.0-approved`\n");
  return {
    root,
    windowsDir,
    macosDir,
    acceptanceFile,
    releaseNotesFile,
    windowsManifestPath,
    macosManifestPath,
    acceptance,
    windowsManifest,
    macosManifest,
  };
}

function verifyOptions(fixture, overrides = {}) {
  return {
    windowsDir: fixture.windowsDir,
    macosDir: fixture.macosDir,
    acceptanceFile: fixture.acceptanceFile,
    expectedVersion: version,
    expectedCommit: commit,
    output: path.join(fixture.root, "output.json"),
    ...overrides,
  };
}

async function useFixture(context) {
  const fixture = await createFixture();
  context.after(() => fs.rm(fixture.root, { recursive: true, force: true }));
  return fixture;
}

test("accepts a complete same-commit formal release evidence set", async (context) => {
  const fixture = await useFixture(context);
  const output = path.join(fixture.root, "formal-release-manifest.json");
  const verification = await verifyFormalReleaseEvidence(verifyOptions(fixture, { output }));
  assert.equal(verification.result.distributionAllowed, true);
  assert.equal(verification.result.assets.windows.name, "banshan-setup.exe");
  assert.equal(verification.result.assets.macos.name, dmgName);
  assert.equal(Object.hasOwn(verification.result.assets.windows, "path"), false);
  assert.equal(path.isAbsolute(verification.windows.installer.path), true);
  assert.equal(verification.result.evidence.acceptanceArtifacts.length, 9);
  assert.deepEqual(JSON.parse(await fs.readFile(output, "utf8")), verification.result);
});

test("rejects cross-commit platform evidence", async (context) => {
  const fixture = await useFixture(context);
  fixture.windowsManifest.gitCommit = "f".repeat(40);
  await writeJson(fixture.windowsManifestPath, fixture.windowsManifest);
  await assert.rejects(verifyFormalReleaseEvidence(verifyOptions(fixture)), /gitCommit mismatch/);
});

test("rejects an incomplete human acceptance gate", async (context) => {
  const fixture = await useFixture(context);
  fixture.acceptance.gates.businessPilot.metrics.caseCount = 4;
  await writeJson(fixture.acceptanceFile, fixture.acceptance);
  await assert.rejects(verifyFormalReleaseEvidence(verifyOptions(fixture)), /caseCount must be an integer >= 5/);
});

test("rejects missing or path-traversing human evidence references", async (context) => {
  const fixture = await useFixture(context);
  fixture.acceptance.gates.businessPilot.evidence = ["missing-business-pilot.json"];
  await writeJson(fixture.acceptanceFile, fixture.acceptance);
  await assert.rejects(verifyFormalReleaseEvidence(verifyOptions(fixture)), /is missing: missing-business-pilot\.json/);

  fixture.acceptance.gates.businessPilot.evidence = ["../outside.json"];
  await writeJson(fixture.acceptanceFile, fixture.acceptance);
  await assert.rejects(verifyFormalReleaseEvidence(verifyOptions(fixture)), /contains path traversal/);
});

test("rejects release notes that only mention the approval marker in prose", async (context) => {
  const fixture = await useFixture(context);
  await fs.writeFile(fixture.releaseNotesFile, "# Business Workbench 1.0\n\nreleaseStatus: blocked-until-all-gates-pass\n\nAfter completion use `releaseStatus: formal-1.0-approved`.\n");
  await assert.rejects(verifyFormalReleaseEvidence(verifyOptions(fixture)), /Release notes are not formally approved/);
});

test("rejects unsafe release notes paths", async (context) => {
  const fixture = await useFixture(context);
  fixture.acceptance.releaseNotesPath = "../outside.md";
  await writeJson(fixture.acceptanceFile, fixture.acceptance);
  await assert.rejects(verifyFormalReleaseEvidence(verifyOptions(fixture)), /releaseNotesPath contains path traversal/);
});

test("rejects uppercase commit and asset hashes instead of normalizing them", async (context) => {
  const fixture = await useFixture(context);
  await assert.rejects(verifyFormalReleaseEvidence(verifyOptions(fixture, { expectedCommit: commit.toUpperCase() })), /expectedCommit must be a full lowercase Git SHA/);

  fixture.windowsManifest.installer.sha256 = fixture.windowsManifest.installer.sha256.toUpperCase();
  await writeJson(fixture.windowsManifestPath, fixture.windowsManifest);
  await assert.rejects(verifyFormalReleaseEvidence(verifyOptions(fixture)), /installer\.sha256 must be lowercase SHA-256/);
});

test("rejects broken native macOS gate lineage", async (context) => {
  const fixture = await useFixture(context);
  fixture.macosManifest.gates.notarization = "failed";
  await writeJson(fixture.macosManifestPath, fixture.macosManifest);
  await assert.rejects(verifyFormalReleaseEvidence(verifyOptions(fixture)), /gates\.notarization mismatch/);

  fixture.macosManifest.gates.notarization = "passed";
  fixture.macosManifest.dmg.name = "renamed.dmg";
  await writeJson(fixture.macosManifestPath, fixture.macosManifest);
  await assert.rejects(verifyFormalReleaseEvidence(verifyOptions(fixture)), /evidence manifest\.dmg\.name mismatch/);
});
