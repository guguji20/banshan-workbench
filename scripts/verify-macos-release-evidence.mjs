import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, readFile, stat } from "node:fs/promises";
import { isAbsolute, posix, relative, resolve, sep, win32 } from "node:path";
import { fileURLToPath } from "node:url";

const PLATFORM = "macos";
const TARGET = "aarch64-apple-darwin";
const MANIFEST_NAME = "macos-release-manifest.json";
const CHECKSUMS_NAME = "SHA256SUMS-macos.txt";
const SHA_PATTERN = /^[0-9a-f]{64}$/;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/;
const VERSION_PATTERN = /^\d+\.\d+\.\d+$/;
const DATE_PATTERN = /^\d{4}-\d{2}-\d{2}$/;
const UTC_PATTERN = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/;

function fail(message) {
  throw new Error(message);
}

function equal(actual, expected, label) {
  if (actual !== expected) fail(`${label} mismatch: expected ${expected}, got ${actual}`);
}

function object(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail(`${label} must be an object`);
  return value;
}

function validSha(value, label) {
  if (typeof value !== "string" || !SHA_PATTERN.test(value)) fail(`${label} must be a lowercase SHA-256`);
  return value;
}

function validSize(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) fail(`${label} must be a positive safe integer`);
  return value;
}

function validUtcTimestamp(value, label) {
  if (typeof value !== "string" || !UTC_PATTERN.test(value)) fail(`${label} must be UTC to second precision`);
  const parsed = new Date(value);
  if (!Number.isFinite(parsed.getTime()) || parsed.toISOString() !== value.replace(/Z$/u, ".000Z")) {
    fail(`${label} must be a real UTC timestamp`);
  }
  return value;
}

function positiveDecimalString(value, label) {
  if (typeof value !== "string" || !/^[1-9]\d*$/u.test(value)) fail(`${label} must be a positive decimal string`);
  return value;
}

function safeRelativePath(value, label) {
  if (typeof value !== "string" || !value || value !== value.trim()) fail(`${label} must be a relative path`);
  if (value.includes("\0") || value.includes("\\") || isAbsolute(value) || win32.isAbsolute(value)) {
    fail(`${label} must be a safe POSIX path`);
  }
  const normalized = posix.normalize(value);
  const invalidSegment = value.split("/").some((segment) => !segment || segment === "." || segment === "..");
  if (normalized !== value || normalized.startsWith("../") || invalidSegment) fail(`${label} contains path traversal: ${value}`);
  return normalized;
}

function contained(root, value, label) {
  const relativePath = safeRelativePath(value, label);
  const path = resolve(root, ...relativePath.split("/"));
  const relation = relative(root, path);
  if (relation === ".." || relation.startsWith(`..${sep}`) || isAbsolute(relation)) fail(`${label} escapes release root`);
  return { path, relativePath };
}

async function regularFile(root, value, label) {
  const { path, relativePath } = contained(root, value, label);
  let current = root;
  for (const segment of relativePath.split("/")) {
    current = resolve(current, segment);
    const metadata = await lstat(current).catch(() => null);
    if (!metadata) fail(`${label} is missing: ${relativePath}`);
    if (metadata.isSymbolicLink()) fail(`${label} must not be a symbolic link: ${relativePath}`);
  }
  const metadata = await stat(path).catch(() => null);
  if (!metadata?.isFile()) fail(`${label} is not a regular file: ${relativePath}`);
  return { path, relativePath, metadata };
}

async function readJson(root, value, label) {
  const file = await regularFile(root, value, label);
  try {
    return { file, value: JSON.parse(await readFile(file.path, "utf8")) };
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

async function digest(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

function sameSet(actual, expected, label) {
  const left = [...actual].sort();
  const right = [...expected].sort();
  if (JSON.stringify(left) === JSON.stringify(right)) return;
  const expectedSet = new Set(right);
  const actualSet = new Set(left);
  const unexpected = left.filter((item) => !expectedSet.has(item));
  const missing = right.filter((item) => !actualSet.has(item));
  fail(`${label} mismatch (unexpected=${unexpected.join(",") || "none"}; missing=${missing.join(",") || "none"})`);
}

function parseChecksums(text) {
  const records = new Map();
  for (const [index, line] of text.split(/\r?\n/u).entries()) {
    if (!line) continue;
    const match = /^([0-9a-f]{64}) ([ *])(.+)$/u.exec(line);
    if (!match) fail(`SHA256SUMS line ${index + 1} is invalid`);
    const path = safeRelativePath(match[3], `SHA256SUMS line ${index + 1} path`);
    if (records.has(path)) fail(`SHA256SUMS contains duplicate path: ${path}`);
    records.set(path, match[1]);
  }
  if (!records.size) fail("SHA256SUMS is empty");
  return records;
}

function fileRecord(record, label) {
  object(record, label);
  return {
    name: safeRelativePath(record.name, `${label}.name`),
    sizeBytes: validSize(record.sizeBytes, `${label}.sizeBytes`),
    sha256: validSha(record.sha256, `${label}.sha256`),
  };
}

function identity(record, expectedVersion, expectedCommit) {
  object(record, "evidence manifest");
  equal(record.schemaVersion, 1, "evidence manifest.schemaVersion");
  equal(record.artifactKind, "macos-release-gate-evidence", "evidence manifest.artifactKind");
  equal(record.distributionAllowed, false, "evidence manifest.distributionAllowed");
  equal(record.status, "passed", "evidence manifest.status");
  equal(record.platform, PLATFORM, "evidence manifest.platform");
  equal(record.target, TARGET, "evidence manifest.target");
  if (typeof record.version !== "string" || !VERSION_PATTERN.test(record.version)) fail("evidence manifest.version must use x.y.z form");
  if (expectedVersion) equal(record.version, expectedVersion, "evidence manifest.version");
  if (typeof record.gitCommit !== "string" || !COMMIT_PATTERN.test(record.gitCommit)) {
    fail("evidence manifest.gitCommit must be a lowercase 40-character Git SHA");
  }
  if (expectedCommit) equal(record.gitCommit, expectedCommit, "evidence manifest.gitCommit");
  if (typeof record.factDate !== "string" || !DATE_PATTERN.test(record.factDate)) fail("evidence manifest.factDate must use YYYY-MM-DD form");
  validUtcTimestamp(record.createdAtUtc, "evidence manifest.createdAtUtc");
  equal(record.factDate, record.createdAtUtc.slice(0, 10), "evidence manifest.factDate");

  const workflow = object(record.workflow, "evidence manifest.workflow");
  positiveDecimalString(workflow.runId, "evidence manifest.workflow.runId");
  positiveDecimalString(workflow.runAttempt, "evidence manifest.workflow.runAttempt");
  if (typeof workflow.ref !== "string" || workflow.ref !== workflow.ref.trim() || !workflow.ref.startsWith("refs/")) {
    fail("evidence manifest.workflow.ref must be a refs/ path");
  }
}

export async function verifyMacosReleaseEvidence(options = {}) {
  const root = resolve(options.releaseRoot ?? process.cwd());
  const rootMetadata = await lstat(root).catch(() => null);
  if (!rootMetadata?.isDirectory() || rootMetadata.isSymbolicLink()) fail(`invalid release root: ${root}`);

  const manifestName = safeRelativePath(options.manifestPath ?? MANIFEST_NAME, "manifest path");
  const { file: manifestFile, value: manifest } = await readJson(root, manifestName, "evidence manifest");
  identity(manifest, options.version, options.commit);

  const gates = object(manifest.gates, "evidence manifest.gates");
  for (const field of ["codesign", "notarization", "gatekeeper", "appStapler", "dmgStapler"]) {
    equal(gates[field], "passed", `evidence manifest.gates.${field}`);
  }

  const sidecar = object(gates.sidecar, "evidence manifest.gates.sidecar");
  equal(sidecar.status, "passed", "evidence manifest.gates.sidecar.status");
  equal(sidecar.executable, true, "evidence manifest.gates.sidecar.executable");
  if (typeof sidecar.version !== "string" || !sidecar.version.trim()) {
    fail("evidence manifest.gates.sidecar.version must be non-empty");
  }

  const startupSmoke = object(gates.startupSmoke, "evidence manifest.gates.startupSmoke");
  equal(startupSmoke.status, "passed", "evidence manifest.gates.startupSmoke.status");
  if (!Number.isFinite(startupSmoke.durationSeconds) || startupSmoke.durationSeconds <= 0) {
    fail("evidence manifest.gates.startupSmoke.durationSeconds must be positive");
  }
  const smokeRecord = fileRecord(startupSmoke.log, "evidence manifest.gates.startupSmoke.log");

  const evidence = object(manifest.evidence, "evidence manifest.evidence");
  const buildLog = fileRecord(evidence.buildLog, "evidence manifest.evidence.buildLog");
  const gateChecksLog = fileRecord(evidence.gateChecksLog, "evidence manifest.evidence.gateChecksLog");
  for (const [record, label] of [[buildLog, "macOS build log"], [gateChecksLog, "macOS gate checks log"]]) {
    const actual = await regularFile(root, record.name, label);
    const content = await readFile(actual.path, "utf8");
    if (!content.trim()) fail(label + " must not be empty");
  }

  const dmg = fileRecord(manifest.dmg, "evidence manifest.dmg");
  const expectedDmgName = `huabang-business-system-${manifest.version}-${TARGET}-${manifest.gitCommit.slice(0, 12)}.dmg`;
  equal(dmg.name, expectedDmgName, "evidence manifest.dmg.name");

  equal(manifest.checksumFile, CHECKSUMS_NAME, "evidence manifest.checksumFile");
  if (!Array.isArray(manifest.files) || !manifest.files.length) fail("evidence manifest.files must be non-empty");
  const records = new Map();
  for (const [index, rawRecord] of manifest.files.entries()) {
    const record = fileRecord(rawRecord, `evidence manifest.files[${index}]`);
    if (records.has(record.name)) fail(`evidence manifest contains duplicate file: ${record.name}`);
    const actual = await regularFile(root, record.name, `evidence manifest file ${record.name}`);
    equal(actual.metadata.size, record.sizeBytes, `${record.name}.sizeBytes`);
    equal(await digest(actual.path), record.sha256, `${record.name}.sha256`);
    records.set(record.name, record);
  }

  const dmgFiles = [...records.keys()].filter((name) => name.endsWith(".dmg"));
  if (dmgFiles.length !== 1) fail("evidence manifest must contain exactly one DMG");
  equal(dmgFiles[0], dmg.name, "evidence manifest.dmg.name");
  const dmgRecord = records.get(dmg.name);
  equal(dmg.sizeBytes, dmgRecord.sizeBytes, "evidence manifest.dmg.sizeBytes");
  equal(dmg.sha256, dmgRecord.sha256, "evidence manifest.dmg.sha256");

  const expectedManifestFiles = [dmg.name, `${dmg.name}.sha256`, smokeRecord.name, buildLog.name, gateChecksLog.name];
  sameSet(records.keys(), expectedManifestFiles, "evidence manifest.files set");
  const manifestSmoke = records.get(smokeRecord.name);
  equal(smokeRecord.sizeBytes, manifestSmoke.sizeBytes, "startup smoke log.sizeBytes");
  equal(smokeRecord.sha256, manifestSmoke.sha256, "startup smoke log.sha256");

  const dmgShaFile = await regularFile(root, `${dmg.name}.sha256`, "DMG checksum file");
  const dmgShaMatch = /^([0-9a-f]{64}) [ *](.+)\r?\n?$/u.exec(await readFile(dmgShaFile.path, "utf8"));
  if (!dmgShaMatch) fail("DMG checksum file is invalid");
  equal(safeRelativePath(dmgShaMatch[2], "DMG checksum path"), dmg.name, "DMG checksum path");
  equal(dmgShaMatch[1], dmg.sha256, "DMG checksum hash");

  const checksumsFile = await regularFile(root, CHECKSUMS_NAME, "SHA256SUMS");
  const checksums = parseChecksums(await readFile(checksumsFile.path, "utf8"));
  const expectedChecksumFiles = [...records.keys(), manifestName];
  sameSet(checksums.keys(), expectedChecksumFiles, "SHA256SUMS file set");
  for (const [name, expectedHash] of checksums) {
    const file = await regularFile(root, name, `SHA256SUMS file ${name}`);
    equal(await digest(file.path), expectedHash, `SHA256SUMS ${name}`);
  }

  equal(manifestFile.relativePath, manifestName, "manifest path");
  return {
    factDate: manifest.factDate,
    version: manifest.version,
    gitCommit: manifest.gitCommit,
    platform: PLATFORM,
    target: TARGET,
    distributionAllowed: false,
    status: "passed",
    dmg,
    filesVerified: records.size + 1,
    checksumsVerified: checksums.size,
  };
}

function parseArguments(argv) {
  const options = {};
  const names = { "--release-root": "releaseRoot", "--version": "version", "--commit": "commit", "--manifest": "manifestPath" };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--help" || argv[index] === "-h") return { help: true };
    const key = names[argv[index]];
    if (!key) fail(`unknown argument: ${argv[index]}`);
    if (!argv[index + 1] || argv[index + 1].startsWith("--")) fail(`missing value for ${argv[index]}`);
    options[key] = argv[++index];
  }
  return options;
}

const isMain = process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url));
if (isMain) {
  try {
    const options = parseArguments(process.argv.slice(2));
    if (options.help) console.log("Usage: node scripts/verify-macos-release-evidence.mjs [--release-root <dir>] [--version <x.y.z>] [--commit <40-char-sha>]");
    else console.log(JSON.stringify(await verifyMacosReleaseEvidence(options), null, 2));
  } catch (error) {
    console.error(`macOS release gate evidence verification failed: ${error.message}`);
    process.exitCode = 1;
  }
}
