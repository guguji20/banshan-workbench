import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { access, copyFile, mkdir, mkdtemp, readFile, readdir, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const TEMP_PREFIX = "bsaigc-source-snapshot-test-";
const TEMP_MARKER = ".owned-by-create-source-snapshot-test";
const SOURCE_SCRIPT = fileURLToPath(new URL("./create-source-snapshot.ps1", import.meta.url));
const POWERSHELL = join(process.env.SystemRoot ?? "C:\\Windows", "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
const ownedTempRoots = new Map();

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

async function runOrThrow(command, args, cwd) {
  const result = await run(command, args, cwd);
  assert.equal(result.code, 0, `${command} failed:\n${result.output}`);
}

async function createOwnedTempRoot() {
  const tempParent = await realpath(tmpdir());
  const root = await realpath(await mkdtemp(join(tempParent, TEMP_PREFIX)));
  const marker = randomUUID();
  assert.equal(dirname(root), tempParent);
  assert.match(basename(root), /^bsaigc-source-snapshot-test-[^\\/]+$/u);
  await writeFile(join(root, TEMP_MARKER), marker, "utf8");
  ownedTempRoots.set(root, marker);
  return root;
}

async function removeOwnedTempRoot(root) {
  const resolvedRoot = await realpath(root);
  const marker = ownedTempRoots.get(resolvedRoot);
  assert.ok(marker, `refusing to remove unowned temp root: ${resolvedRoot}`);
  const tempParent = await realpath(tmpdir());
  const relativeRoot = relative(tempParent, resolvedRoot);
  assert.equal(dirname(resolvedRoot), tempParent);
  assert.ok(relativeRoot.length > 0);
  assert.equal(isAbsolute(relativeRoot), false);
  assert.equal(relativeRoot.startsWith(".."), false);
  assert.match(basename(resolvedRoot), /^bsaigc-source-snapshot-test-[^\\/]+$/u);
  assert.equal(await readFile(join(resolvedRoot, TEMP_MARKER), "utf8"), marker);
  await rm(resolvedRoot, { recursive: true, force: true });
  ownedTempRoots.delete(resolvedRoot);
}

async function writeFixtureFile(repo, relativePath, content) {
  const destination = join(repo, ...relativePath.split("/"));
  await mkdir(dirname(destination), { recursive: true });
  await writeFile(destination, content);
}

async function withFixture(files, callback) {
  const root = await createOwnedTempRoot();
  const repo = join(root, "repo");
  const output = join(root, "output");
  try {
    await mkdir(join(repo, "scripts"), { recursive: true });
    await mkdir(output, { recursive: true });
    await copyFile(SOURCE_SCRIPT, join(repo, "scripts", "create-source-snapshot.ps1"));
    await writeFixtureFile(repo, "README.md", "# source snapshot fixture\n");
    for (const [relativePath, fileContent] of Object.entries(files)) {
      await writeFixtureFile(repo, relativePath, fileContent);
    }
    await runOrThrow("git.exe", ["init", "--quiet"], repo);
    await runOrThrow("git.exe", ["config", "user.name", "Snapshot Test"], repo);
    await runOrThrow("git.exe", ["config", "user.email", "snapshot@test.invalid"], repo);
    await runOrThrow("git.exe", ["add", "--all"], repo);
    await runOrThrow("git.exe", ["commit", "--quiet", "-m", "fixture"], repo);
    await callback({ repo, output });
  } finally {
    await removeOwnedTempRoot(root);
  }
}

async function runSnapshot({ repo, output, dryRun = true }) {
  const args = ["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", join(repo, "scripts", "create-source-snapshot.ps1"), "-Version", "test-1.0.0", "-OutputDirectory", output, "-LargeBinaryThresholdMiB", "1"];
  if (dryRun) args.push("-DryRun");
  return await run(POWERSHELL, args, repo);
}

test("Windows prerequisites are available", async () => {
  assert.equal(process.platform, "win32");
  await access(POWERSHELL);
  await access(SOURCE_SCRIPT);
});

test("allows synthetic business-material fixtures", { timeout: 30_000 }, async () => {
  await withFixture({
    "src/synthetic-fixture.rs": 'const FIXTURE: &str = "真实需求 synthetic fixture sample.docx";\n',
  }, async ({ repo, output }) => {
    const result = await runSnapshot({ repo, output });
    assert.equal(result.code, 0, result.output);
    assert.match(result.stdout, /Sensitive findings: 0/u);
    assert.match(result.stdout, /DRY_RUN_OK/u);
  });
});

test("blocks real embedded business material", { timeout: 30_000 }, async () => {
  const realMaterial = ["真", "实需求：白鹅潭项目最终交付文件.docx"].join("");
  await withFixture({
    "src/customer-material.rs": `const MATERIAL: &str = "${realMaterial}";\n`,
  }, async ({ repo, output }) => {
    const result = await runSnapshot({ repo, output });
    assert.notEqual(result.code, 0, result.output);
    assert.match(result.output, /BLOCKED src\/customer-material\.rs:\d+ \[embedded-business-material\]/u);
  });
});

test("blocks Baidu Pan links and pickup codes", { timeout: 30_000 }, async () => {
  const shareUrl = ["https://pan.", "baidu.com/s/1AbCdEfGh"].join("");
  const pickupCode = ["提取", "码：a1B2"].join("");
  await withFixture({
    "docs/customer-share.txt": `下载：${shareUrl}\n${pickupCode}\n`,
  }, async ({ repo, output }) => {
    const result = await runSnapshot({ repo, output });
    assert.notEqual(result.code, 0, result.output);
    assert.match(result.output, /\[baidu-pan-share-url\]/u);
    assert.match(result.output, /\[baidu-pan-pickup-code\]/u);
  });
});

test("blocks Windows and Unix user directories", { timeout: 30_000 }, async () => {
  const windowsPath = ["C:", "Users", "alice", "Desktop", "客户资料", "合同.docx"].join("\\");
  const unixPath = ["", "home", "alice", "projects", "customer", "contract.pdf"].join("/");
  await withFixture({
    "docs/local-paths.txt": `Windows: ${windowsPath}\nUnix: ${unixPath}\n`,
  }, async ({ repo, output }) => {
    const result = await runSnapshot({ repo, output });
    assert.notEqual(result.code, 0, result.output);
    assert.match(result.output, /\[windows-user-home-path\]/u);
    assert.match(result.output, /\[unix-user-home-path\]/u);
  });
});

test("excludes tracked docs/visual binaries", { timeout: 30_000 }, async () => {
  await withFixture({
    "docs/visual/customer-screen.png": Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x01]),
    "src/index.txt": "included source\n",
  }, async ({ repo, output }) => {
    await writeFixtureFile(repo, "docs/visual/customer-screen.png", Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x02]));
    const result = await runSnapshot({ repo, output, dryRun: false });
    assert.equal(result.code, 0, result.output);
    assert.match(result.stdout, /SNAPSHOT_OK/u);
    const snapshotDirectories = (await readdir(output, { withFileTypes: true })).filter((entry) => entry.isDirectory());
    assert.equal(snapshotDirectories.length, 1);
    const manifest = JSON.parse(await readFile(join(output, snapshotDirectories[0].name, "source-manifest.json"), "utf8"));
    assert.ok(manifest.excluded.some((entry) => entry.relativePath === "docs/visual" && entry.reason === "excluded-root:docs/visual" && entry.kind === "directory"));
    assert.equal(manifest.files.some((entry) => entry.relativePath === "docs/visual/customer-screen.png"), false);
    assert.ok(manifest.files.some((entry) => entry.relativePath === "src/index.txt"));
    const patch = await readFile(join(output, snapshotDirectories[0].name, "git-diff.binary.patch"), "utf8");
    assert.doesNotMatch(patch, /docs\/visual/u);
  });
});

test("blocks business document binaries", { timeout: 30_000 }, async () => {
  await withFixture({
    "src/customer-contract.docx": Buffer.from([0x50, 0x4b, 0x03, 0x04, 0x00, 0x01]),
  }, async ({ repo, output }) => {
    const result = await runSnapshot({ repo, output });
    assert.notEqual(result.code, 0, result.output);
    assert.match(result.output, /BLOCKED src\/customer-contract\.docx:0 \[business-binary-material\]/u);
  });
});

test("blocks images outside trusted asset roots", { timeout: 30_000 }, async () => {
  await withFixture({
    "src/customer-screen.png": Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x01]),
  }, async ({ repo, output }) => {
    const result = await runSnapshot({ repo, output });
    assert.notEqual(result.code, 0, result.output);
    assert.match(result.output, /BLOCKED src\/customer-screen\.png:0 \[untrusted-image-material\]/u);
  });
});

test("blocks operator user directories", { timeout: 30_000 }, async () => {
  const operatorPath = ["C:", "Users", "operator", "Desktop", "customer", "contract.pdf"].join("\\");
  await withFixture({
    "docs/operator-path.txt": `Path: ${operatorPath}\n`,
  }, async ({ repo, output }) => {
    const result = await runSnapshot({ repo, output });
    assert.notEqual(result.code, 0, result.output);
    assert.match(result.output, /\[windows-user-home-path\]/u);
  });
});

test("redacts sensitive values removed from the Git diff", { timeout: 30_000 }, async () => {
  const oldPath = ["C:", "Users", "operator", "Desktop", "customer", "contract.pdf"].join("\\");
  await withFixture({
    "src/legacy-path.txt": `Path: ${oldPath}\n`,
  }, async ({ repo, output }) => {
    await writeFixtureFile(repo, "src/legacy-path.txt", "Path: [removed]\n");
    const result = await runSnapshot({ repo, output, dryRun: false });
    assert.equal(result.code, 0, result.output);
    assert.match(result.stdout, /Git diff redactions: 1/u);
    const snapshotDirectories = (await readdir(output, { withFileTypes: true })).filter((entry) => entry.isDirectory());
    assert.equal(snapshotDirectories.length, 1);
    const patch = await readFile(join(output, snapshotDirectories[0].name, "git-diff.binary.patch"), "utf8");
    assert.doesNotMatch(patch, new RegExp(oldPath.replaceAll("\\", "\\\\"), "u"));
    assert.match(patch, /\[REDACTED_DIFF_VALUE\]/u);
    const manifest = JSON.parse(await readFile(join(output, snapshotDirectories[0].name, "source-manifest.json"), "utf8"));
    assert.equal(manifest.security.gitDiffRedactions, 1);
  });
});
