import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { verifyMacosReleaseEvidence } from "./verify-macos-release-evidence.mjs";

const WINDOWS_KIND = "windows-release-gate-evidence";
const MACOS_KIND = "macos-release-gate-evidence";
const ACCEPTANCE_KIND = "business-workbench-1.0-final-acceptance";
const PRODUCT_RELEASE = "business-workbench-1.0";
const SHA_PATTERN = /^[a-f0-9]{64}$/;
const COMMIT_PATTERN = /^[a-f0-9]{40}$/;
const VERSION_PATTERN = /^\d+\.\d+\.\d+$/;
const PLACEHOLDER_PATTERN = /__PENDING__|\bTODO\b|\bTBD\b|\u5f85\u8865|\u5f85\u5b9a|\u672a\u5b8c\u6210/i;

function fail(message) {
  throw new Error(message);
}

function requireString(value, label) {
  if (typeof value !== "string" || value.trim() === "") fail(`${label} must be a non-empty string.`);
  return value.trim();
}

function requireBoolean(value, label, expected = true) {
  if (value !== expected) fail(`${label} must be ${expected}.`);
}

function requireIntegerAtLeast(value, minimum, label) {
  if (!Number.isInteger(value) || value < minimum) fail(`${label} must be an integer >= ${minimum}.`);
}

function assertNoPlaceholders(value, label) {
  if (PLACEHOLDER_PATTERN.test(JSON.stringify(value))) fail(`${label} contains a pending placeholder.`);
}

function safeRelativePath(value, label) {
  const normalizedValue = requireString(value, label);
  if (value !== normalizedValue || value.includes("\0") || value.includes("\\") || path.isAbsolute(value) || path.win32.isAbsolute(value)) {
    fail(`${label} must be a safe relative POSIX path.`);
  }
  const normalized = path.posix.normalize(value);
  const invalidSegment = value.split("/").some((segment) => !segment || segment === "." || segment === "..");
  if (normalized !== value || normalized.startsWith("../") || invalidSegment) fail(`${label} contains path traversal: ${value}`);
  return normalized;
}

function contained(root, relativePath, label) {
  const safePath = safeRelativePath(relativePath, label);
  const resolved = path.resolve(root, ...safePath.split("/"));
  const relation = path.relative(root, resolved);
  if (relation === ".." || relation.startsWith(`..${path.sep}`) || path.isAbsolute(relation)) fail(`${label} escapes evidence root.`);
  return { path: resolved, relativePath: safePath };
}

async function requireDirectory(value, label) {
  const resolved = path.resolve(requireString(value, label));
  const metadata = await fs.lstat(resolved).catch(() => null);
  if (!metadata?.isDirectory() || metadata.isSymbolicLink()) fail(`${label} must be a non-symbolic-link directory: ${resolved}`);
  return resolved;
}

async function requireRegularFile(filePath, label) {
  const metadata = await fs.lstat(filePath).catch(() => null);
  if (!metadata?.isFile() || metadata.isSymbolicLink()) fail(`${label} must be a non-symbolic-link regular file: ${filePath}`);
  return metadata;
}

async function requireContainedRegularFile(root, relativePath, label) {
  const resolved = contained(root, relativePath, label);
  let current = root;
  for (const segment of resolved.relativePath.split("/")) {
    current = path.resolve(current, segment);
    const metadata = await fs.lstat(current).catch(() => null);
    if (!metadata) fail(`${label} is missing: ${resolved.relativePath}`);
    if (metadata.isSymbolicLink()) fail(`${label} must not traverse a symbolic link: ${resolved.relativePath}`);
  }
  const metadata = await requireRegularFile(resolved.path, label);
  return { ...resolved, metadata };
}

async function readJson(filePath) {
  try {
    return JSON.parse(await fs.readFile(filePath, "utf8"));
  } catch (error) {
    fail(`Unable to read JSON ${filePath}: ${error.message}`);
  }
}

async function listFiles(root) {
  const output = [];
  for (const entry of await fs.readdir(root, { withFileTypes: true })) {
    const fullPath = path.join(root, entry.name);
    if (entry.isSymbolicLink()) fail(`Evidence roots must not contain symbolic links: ${fullPath}`);
    if (entry.isDirectory()) output.push(...(await listFiles(fullPath)));
    else if (entry.isFile()) output.push(fullPath);
  }
  return output;
}

async function sha256(filePath) {
  const hash = createHash("sha256");
  const file = await fs.open(filePath, "r");
  try {
    for await (const chunk of file.createReadStream()) hash.update(chunk);
  } finally {
    await file.close();
  }
  return hash.digest("hex");
}

async function findEvidenceManifest(root, artifactKind) {
  const matches = [];
  for (const file of (await listFiles(root)).filter((item) => item.toLowerCase().endsWith(".json"))) {
    const value = await readJson(file);
    if (value?.artifactKind === artifactKind) matches.push({ file, value });
  }
  if (matches.length !== 1) fail(`Expected exactly one ${artifactKind} manifest under ${root}, found ${matches.length}.`);
  return matches[0];
}

function validateCommonEvidence(manifest, expectedKind, expectedVersion, expectedCommit, platform) {
  if (manifest.schemaVersion !== 1) fail(`${expectedKind}.schemaVersion must be 1.`);
  if (manifest.artifactKind !== expectedKind) fail(`Unexpected artifactKind: ${manifest.artifactKind}`);
  requireBoolean(manifest.distributionAllowed, `${expectedKind}.distributionAllowed`, false);
  if (manifest.version !== expectedVersion) fail(`${expectedKind}.version mismatch: expected ${expectedVersion}, got ${manifest.version}`);
  if (!VERSION_PATTERN.test(manifest.version)) fail(`${expectedKind}.version must use x.y.z form.`);
  if (manifest.gitCommit !== expectedCommit) fail(`${expectedKind}.gitCommit mismatch: expected ${expectedCommit}, got ${manifest.gitCommit}`);
  if (!COMMIT_PATTERN.test(manifest.gitCommit)) fail(`${expectedKind}.gitCommit must be a full lowercase Git SHA.`);
  if (manifest.platform !== platform) fail(`${expectedKind}.platform must be ${platform}.`);
  assertNoPlaceholders(manifest, expectedKind);
}

async function verifyAsset(root, asset, label) {
  const name = requireString(asset?.name, `${label}.name`);
  if (path.basename(name) !== name) fail(`${label}.name must be a file name, not a path.`);
  const expectedSha = requireString(asset?.sha256, `${label}.sha256`);
  if (!SHA_PATTERN.test(expectedSha)) fail(`${label}.sha256 must be lowercase SHA-256.`);
  requireIntegerAtLeast(asset?.sizeBytes, 1, `${label}.sizeBytes`);
  const matches = (await listFiles(root)).filter((file) => path.basename(file) === name);
  if (matches.length !== 1) fail(`Expected exactly one ${name} under ${root}, found ${matches.length}.`);
  const stat = await fs.stat(matches[0]);
  if (stat.size !== asset.sizeBytes) fail(`${label} size mismatch: expected ${asset.sizeBytes}, got ${stat.size}.`);
  const actualSha = await sha256(matches[0]);
  if (actualSha !== expectedSha) fail(`${label} SHA-256 mismatch: expected ${expectedSha}, got ${actualSha}.`);
  return { name, sizeBytes: stat.size, sha256: actualSha, path: matches[0] };
}

function publicAsset(asset) {
  return { name: asset.name, sizeBytes: asset.sizeBytes, sha256: asset.sha256 };
}

function requireGate(manifest, gateName, label) {
  requireBoolean(manifest?.gates?.[gateName], `${label}.gates.${gateName}`);
}

async function validateWindowsEvidence(root, expectedVersion, expectedCommit) {
  const located = await findEvidenceManifest(root, WINDOWS_KIND);
  const manifest = located.value;
  validateCommonEvidence(manifest, WINDOWS_KIND, expectedVersion, expectedCommit, "windows-x86_64");
  for (const gate of ["releaseVerify", "authenticode", "nsisInstall", "launchSmoke", "sourceSnapshotSecurity"]) requireGate(manifest, gate, WINDOWS_KIND);
  if (manifest.installer?.authenticode !== "Valid") fail(`${WINDOWS_KIND}.installer.authenticode must be Valid.`);
  const installer = await verifyAsset(root, manifest.installer, `${WINDOWS_KIND}.installer`);
  const sourceArchive = await verifyAsset(root, manifest.sourceSnapshot, `${WINDOWS_KIND}.sourceSnapshot`);
  return { manifestPath: located.file, manifest, installer, sourceArchive };
}

async function validateMacosEvidence(root, expectedVersion, expectedCommit) {
  const verification = await verifyMacosReleaseEvidence({ releaseRoot: root, version: expectedVersion, commit: expectedCommit });
  const manifestPath = path.join(root, "macos-release-manifest.json");
  const manifest = await readJson(manifestPath);
  if (manifest.artifactKind !== MACOS_KIND) fail(`Unexpected artifactKind: ${manifest.artifactKind}`);
  assertNoPlaceholders(manifest, MACOS_KIND);
  const dmg = await verifyAsset(root, manifest.dmg, `${MACOS_KIND}.dmg`);
  if (dmg.name !== verification.dmg.name || dmg.sha256 !== verification.dmg.sha256 || dmg.sizeBytes !== verification.dmg.sizeBytes) {
    fail(`${MACOS_KIND}.dmg does not match the verified macOS evidence closure.`);
  }
  return { manifestPath, manifest, dmg };
}

function requireAcceptanceGate(acceptance, gateName) {
  const gate = acceptance?.gates?.[gateName];
  if (!gate || gate.passed !== true) fail(`${ACCEPTANCE_KIND}.gates.${gateName}.passed must be true.`);
  if (!Array.isArray(gate.evidence) || gate.evidence.length === 0) {
    fail(`${ACCEPTANCE_KIND}.gates.${gateName}.evidence must contain at least one reference.`);
  }
  const evidence = gate.evidence.map((item, index) => safeRelativePath(item, `${ACCEPTANCE_KIND}.gates.${gateName}.evidence[${index}]`));
  if (new Set(evidence).size !== evidence.length) fail(`${ACCEPTANCE_KIND}.gates.${gateName}.evidence must not contain duplicates.`);
  return { ...gate, evidence };
}

async function validateAcceptance(filePath, expectedVersion, expectedCommit) {
  const acceptance = await readJson(filePath);
  if (acceptance.schemaVersion !== 1) fail(`${ACCEPTANCE_KIND}.schemaVersion must be 1.`);
  if (acceptance.artifactKind !== ACCEPTANCE_KIND) fail(`${filePath} is not ${ACCEPTANCE_KIND}.`);
  if (acceptance.productRelease !== PRODUCT_RELEASE) fail(`productRelease must be ${PRODUCT_RELEASE}.`);
  if (acceptance.binaryVersion !== expectedVersion) fail(`binaryVersion must be ${expectedVersion}.`);
  if (acceptance.gitCommit !== expectedCommit) fail(`acceptance gitCommit must be ${expectedCommit}.`);
  if (acceptance.releaseTag !== `v${expectedVersion}`) fail(`releaseTag must be v${expectedVersion}.`);
  requireBoolean(acceptance.releaseAuthorized, `${ACCEPTANCE_KIND}.releaseAuthorized`);
  safeRelativePath(acceptance.releaseNotesPath, `${ACCEPTANCE_KIND}.releaseNotesPath`);
  assertNoPlaceholders(acceptance, ACCEPTANCE_KIND);

  const gates = {
    windowsSecondaryMachine: requireAcceptanceGate(acceptance, "windowsSecondaryMachine"),
    windowsDifferentUser: requireAcceptanceGate(acceptance, "windowsDifferentUser"),
    windowsColdStarts: requireAcceptanceGate(acceptance, "windowsColdStarts"),
    upgradeRollback: requireAcceptanceGate(acceptance, "upgradeRollback"),
    dataIntegrity: requireAcceptanceGate(acceptance, "dataIntegrity"),
    macosArm64: requireAcceptanceGate(acceptance, "macosArm64"),
    businessPilot: requireAcceptanceGate(acceptance, "businessPilot"),
    securityReview: requireAcceptanceGate(acceptance, "securityReview"),
    releaseNotes: requireAcceptanceGate(acceptance, "releaseNotes"),
  };
  requireIntegerAtLeast(gates.windowsSecondaryMachine.metrics?.machineCount, 2, "windowsSecondaryMachine.metrics.machineCount");
  requireIntegerAtLeast(gates.windowsDifferentUser.metrics?.userCount, 2, "windowsDifferentUser.metrics.userCount");
  requireIntegerAtLeast(gates.windowsColdStarts.metrics?.totalColdStarts, 20, "windowsColdStarts.metrics.totalColdStarts");
  requireIntegerAtLeast(gates.businessPilot.metrics?.caseCount, 5, "businessPilot.metrics.caseCount");

  const evidenceRecords = [];
  for (const [gateName, gate] of Object.entries(gates)) {
    for (const [index, reference] of gate.evidence.entries()) {
      const file = await requireContainedRegularFile(path.dirname(filePath), reference, `${ACCEPTANCE_KIND}.gates.${gateName}.evidence[${index}]`);
      if (path.resolve(file.path) === path.resolve(filePath)) fail(`${ACCEPTANCE_KIND}.gates.${gateName}.evidence must not reference the acceptance manifest itself.`);
      if (file.metadata.size <= 0) fail(`${ACCEPTANCE_KIND}.gates.${gateName}.evidence[${index}] must not be empty.`);
      evidenceRecords.push({ gate: gateName, name: reference, sizeBytes: file.metadata.size, sha256: await sha256(file.path) });
    }
  }
  evidenceRecords.sort((left, right) => left.gate.localeCompare(right.gate) || left.name.localeCompare(right.name));
  return { manifest: acceptance, evidenceRecords };
}

function formalReleaseStatus(text, label) {
  const statuses = text.split(/\r?\n/u).flatMap((line) => {
    const match = /^\s*(?:-\s*)?`?releaseStatus:\s*([a-z0-9.-]+)`?\s*$/u.exec(line);
    return match ? [match[1]] : [];
  });
  if (statuses.length !== 1) fail(`${label} must contain exactly one releaseStatus field.`);
  return statuses[0];
}

export async function verifyFormalReleaseEvidence(options = {}) {
  const windowsDir = await requireDirectory(options.windowsDir, "windowsDir");
  const macosDir = await requireDirectory(options.macosDir, "macosDir");
  const acceptanceFile = path.resolve(requireString(options.acceptanceFile, "acceptanceFile"));
  await requireRegularFile(acceptanceFile, "acceptanceFile");
  const expectedVersion = requireString(options.expectedVersion, "expectedVersion");
  if (!VERSION_PATTERN.test(expectedVersion)) fail("expectedVersion must use x.y.z form.");
  const expectedCommit = requireString(options.expectedCommit, "expectedCommit");
  if (!COMMIT_PATTERN.test(expectedCommit)) fail("expectedCommit must be a full lowercase Git SHA.");

  const windows = await validateWindowsEvidence(windowsDir, expectedVersion, expectedCommit);
  const macos = await validateMacosEvidence(macosDir, expectedVersion, expectedCommit);
  const acceptanceVerification = await validateAcceptance(acceptanceFile, expectedVersion, expectedCommit);
  const acceptance = acceptanceVerification.manifest;
  const releaseNotesRelativePath = safeRelativePath(acceptance.releaseNotesPath, `${ACCEPTANCE_KIND}.releaseNotesPath`);
  const releaseNotes = await requireContainedRegularFile(path.resolve(path.dirname(acceptanceFile), ".."), releaseNotesRelativePath, `${ACCEPTANCE_KIND}.releaseNotesPath`);
  const releaseNotesText = await fs.readFile(releaseNotes.path, "utf8");
  if (formalReleaseStatus(releaseNotesText, "Release notes") !== "formal-1.0-approved") fail(`Release notes are not formally approved: ${releaseNotes.path}`);
  if (PLACEHOLDER_PATTERN.test(releaseNotesText)) fail(`Release notes contain a pending placeholder: ${releaseNotes.path}`);

  const result = {
    schemaVersion: 1,
    artifactKind: "business-workbench-1.0-formal-release",
    productRelease: PRODUCT_RELEASE,
    binaryVersion: expectedVersion,
    releaseTag: `v${expectedVersion}`,
    gitCommit: expectedCommit,
    distributionAllowed: true,
    createdAtUtc: new Date().toISOString(),
    releaseNotes: { path: releaseNotesRelativePath, sha256: await sha256(releaseNotes.path) },
    evidence: {
      acceptanceSha256: await sha256(acceptanceFile),
      windowsManifestSha256: await sha256(windows.manifestPath),
      macosManifestSha256: await sha256(macos.manifestPath),
      acceptanceArtifacts: acceptanceVerification.evidenceRecords,
    },
    assets: {
      windows: publicAsset(windows.installer),
      macos: publicAsset(macos.dmg),
      source: publicAsset(windows.sourceArchive),
    },
  };
  if (options.output) {
    const outputPath = path.resolve(requireString(options.output, "output"));
    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    await fs.writeFile(outputPath, `${JSON.stringify(result, null, 2)}\n`);
  }
  return { result, windows, macos, acceptance };
}

function parseCliArgs(argv) {
  const options = {};
  const names = new Set(["windows-dir", "macos-dir", "acceptance-file", "expected-version", "expected-commit", "output"]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) fail(`Unexpected argument: ${argument}`);
    const name = argument.slice(2);
    if (!names.has(name)) fail(`Unknown argument: --${name}`);
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) fail(`Missing value for --${name}`);
    const key = name.replaceAll("-", "");
    if (Object.hasOwn(options, key)) fail(`Duplicate argument: --${name}`);
    options[key] = value;
    index += 1;
  }
  return {
    windowsDir: options.windowsdir,
    macosDir: options.macosdir,
    acceptanceFile: options.acceptancefile,
    expectedVersion: options.expectedversion,
    expectedCommit: options.expectedcommit,
    output: options.output,
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const verification = await verifyFormalReleaseEvidence(parseCliArgs(process.argv.slice(2)));
    process.stdout.write(`${JSON.stringify(verification.result, null, 2)}\n`);
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
