import { createHash } from "node:crypto";
import { readFile, readdir, stat } from "node:fs/promises";
import { resolve, dirname, relative, sep, posix } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const skillRoot = resolve(repoRoot, "src-tauri", "resources", "business-skills");
const bundlePath = resolve(skillRoot, "bundle.json");
const idPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const pinnedCodexVersion = "0.144.5";

function fail(message) {
  throw new Error(`[business-skills] ${message}`);
}

function normalizeBundlePath(value, label) {
  if (typeof value !== "string" || value.length === 0) fail(`${label} must be a non-empty path`);
  if (value.includes("\\") || value.startsWith("/") || /^[A-Za-z]:/.test(value)) {
    fail(`${label} must use a portable relative path: ${value}`);
  }
  const normalized = posix.normalize(value);
  if (normalized !== value || normalized === "." || normalized === ".." || normalized.startsWith("../")) {
    fail(`${label} is not a normalized safe relative path: ${value}`);
  }
  return normalized;
}

function assertSafeRelativePath(value, label) {
  const normalized = normalizeBundlePath(value, label);
  const resolved = resolve(skillRoot, ...normalized.split("/"));
  const rel = relative(skillRoot, resolved);
  if (rel === "" || rel.startsWith(`..${sep}`) || rel === ".." || rel.includes(`..${sep}`)) {
    fail(`${label} escapes the business skill root: ${value}`);
  }
  return resolved;
}

function compareVersions(left, right) {
  const parse = (value, label) => {
    if (typeof value !== "string" || !/^\d+\.\d+\.\d+$/.test(value)) fail(`${label} must be x.y.z`);
    return value.split(".").map(Number);
  };
  const a = parse(left, "version");
  const b = parse(right, "version");
  for (let index = 0; index < 3; index += 1) {
    if (a[index] !== b[index]) return a[index] - b[index];
  }
  return 0;
}

async function listManagedFiles(directory, prefix = "") {
  const entries = await readdir(directory, { withFileTypes: true });
  const paths = [];
  for (const entry of entries) {
    if (entry.isSymbolicLink()) fail(`symbolic links are not allowed: ${posix.join(prefix, entry.name)}`);
    const relativePath = posix.join(prefix, entry.name);
    const absolutePath = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      paths.push(...await listManagedFiles(absolutePath, relativePath));
    } else if (entry.isFile() && relativePath !== "bundle.json") {
      paths.push(relativePath);
    }
  }
  return paths.sort();
}

const bundle = JSON.parse(await readFile(bundlePath, "utf8"));
if (bundle.schemaVersion !== "1.0") fail(`unsupported bundle schema ${bundle.schemaVersion}`);
if (!Array.isArray(bundle.skills) || bundle.skills.length === 0) fail("bundle has no skills");
if (bundle.versionAuthority !== "bundle.json") fail("bundle.versionAuthority must be bundle.json");
if (compareVersions(pinnedCodexVersion, bundle.minimumCodexVersion) < 0) {
  fail(`pinned Codex ${pinnedCodexVersion} is older than minimum ${bundle.minimumCodexVersion}`);
}

const ids = new Set();
const unionTools = new Set();
for (const item of bundle.skills) {
  if (!idPattern.test(item.id)) fail(`invalid skill id ${item.id}`);
  if (ids.has(item.id)) fail(`duplicate skill id ${item.id}`);
  ids.add(item.id);

  const manifestPath = assertSafeRelativePath(item.manifest, `${item.id}.manifest`);
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  if (manifest.id !== item.id) fail(`${item.id} manifest id mismatch`);
  if (manifest.version !== item.version) fail(`${item.id} version mismatch`);
  if (manifest.schemaVersion !== "1.0") fail(`${item.id} unsupported manifest schema`);
  if (!manifest.humanApprovalRequired) fail(`${item.id} must require human approval`);
  if (!Array.isArray(manifest.requiredTools) || manifest.requiredTools.length === 0) fail(`${item.id} has no tools`);
  if (!Array.isArray(manifest.permissions) || manifest.permissions.length === 0) fail(`${item.id} has no permissions`);
  if (!Array.isArray(manifest.outputArtifacts) || manifest.outputArtifacts.length === 0) fail(`${item.id} has no artifacts`);

  const manifestDir = dirname(manifestPath);
  const entryPath = resolve(manifestDir, manifest.entry);
  const entryRel = relative(manifestDir, entryPath);
  if (entryRel.startsWith(`..${sep}`) || entryRel === "..") fail(`${item.id} entry escapes its directory`);
  const markdown = await readFile(entryPath, "utf8");
  const expectedName = `name: ${item.id}`;
  if (!/^---\r?\n/.test(markdown) || !markdown.includes(expectedName)) fail(`${item.id} has invalid SKILL.md frontmatter`);
  for (const heading of ["## 触发条件", "## 人工确认边界", "## 输出 Artifact", "## 所需工具与权限"]) {
    if (!markdown.includes(heading)) fail(`${item.id} is missing ${heading}`);
  }

  const declared = JSON.stringify(manifest.requiredTools);
  const bundled = JSON.stringify(item.requiredTools);
  if (declared !== bundled) fail(`${item.id} requiredTools mismatch`);
  for (const tool of item.requiredTools) unionTools.add(tool);
}

const bundleTools = [...bundle.requiredTools].sort();
const expectedTools = [...unionTools].sort();
if (JSON.stringify(bundleTools) !== JSON.stringify(expectedTools)) fail("bundle.requiredTools is not the exact skill tool union");

const directories = (await readdir(skillRoot, { withFileTypes: true }))
  .filter((entry) => entry.isDirectory() && !entry.name.startsWith("_"))
  .map((entry) => entry.name)
  .sort();
const bundleIds = [...ids].sort();
if (JSON.stringify(directories) !== JSON.stringify(bundleIds)) fail("bundle skill list does not match skill directories");

if (!Array.isArray(bundle.files) || bundle.files.length === 0) fail("bundle.files is empty");
const declaredFiles = new Set();
for (const item of bundle.files) {
  const normalized = normalizeBundlePath(item.path, "bundle.files.path");
  if (declaredFiles.has(normalized)) fail(`duplicate bundle file ${normalized}`);
  declaredFiles.add(normalized);
  const filePath = assertSafeRelativePath(normalized, `bundle.files[${normalized}]`);
  const contents = await readFile(filePath);
  const metadata = await stat(filePath);
  if (!metadata.isFile()) fail(`bundle file is not a regular file: ${normalized}`);
  if (!Number.isSafeInteger(item.bytes) || item.bytes < 0 || contents.byteLength !== item.bytes) {
    fail(`${normalized} byte length mismatch: expected ${item.bytes}, got ${contents.byteLength}`);
  }
  if (typeof item.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(item.sha256)) {
    fail(`${normalized} has invalid sha256`);
  }
  const digest = createHash("sha256").update(contents).digest("hex");
  if (digest !== item.sha256) fail(`${normalized} sha256 mismatch`);
}

const actualFiles = await listManagedFiles(skillRoot);
const expectedFiles = [...declaredFiles].sort();
if (JSON.stringify(actualFiles) !== JSON.stringify(expectedFiles)) {
  const missing = actualFiles.filter((file) => !declaredFiles.has(file));
  const stale = expectedFiles.filter((file) => !actualFiles.includes(file));
  fail(`bundle.files does not match disk (undeclared=${missing.join(",") || "none"}; missing=${stale.join(",") || "none"})`);
}

console.log(`business skill bundle ${bundle.version}: ${bundle.skills.length} skills, ${bundle.requiredTools.length} tools, ${bundle.files.length} verified files, validation passed`);
