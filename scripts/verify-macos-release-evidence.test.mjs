import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import test from "node:test";

import { verifyMacosReleaseEvidence } from "./verify-macos-release-evidence.mjs";

const execFileAsync = promisify(execFile);
const FACT_DATE = "2026-08-02";
const CREATED_AT_UTC = "2026-08-02T12:00:00Z";
const VERSION = "1.3.4";
const PLATFORM = "macos";
const TARGET = "aarch64-apple-darwin";
const COMMIT = "a".repeat(40);
const DMG_NAME = `huabang-business-system-${VERSION}-${TARGET}-${COMMIT.slice(0, 12)}.dmg`;
const MANIFEST_NAME = "macos-release-manifest.json";
const CHECKSUMS_NAME = "SHA256SUMS-macos.txt";
const SMOKE_NAME = "macos-smoke.log";
const BUILD_LOG_NAME = "macos-build.log";
const GATE_CHECKS_LOG_NAME = "macos-gate-checks.log";
const VERIFIER = fileURLToPath(new URL("./verify-macos-release-evidence.mjs", import.meta.url));
const WORKFLOW = fileURLToPath(new URL("../.github/workflows/build-macos.yml", import.meta.url));

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

async function fileRecord(root, name) {
  const value = await readFile(join(root, name));
  return { name, sizeBytes: value.length, sha256: sha256(value) };
}

async function writeJson(path, value) {
  await writeFile(path, JSON.stringify(value, null, 2) + "\n", "utf8");
}

async function createFixture(mutate = async () => {}) {
  const root = await mkdtemp(join(tmpdir(), "bsaigc-macos-evidence-"));
  await writeFile(join(root, DMG_NAME), "fixture dmg payload\n", "utf8");
  await writeFile(join(root, SMOKE_NAME), "startup smoke passed\n", "utf8");
  await writeFile(join(root, BUILD_LOG_NAME), "tauri build completed\n", "utf8");
  await writeFile(join(root, GATE_CHECKS_LOG_NAME), "codesign, spctl, and stapler passed\n", "utf8");
  const dmg = await fileRecord(root, DMG_NAME);
  await writeFile(join(root, `${DMG_NAME}.sha256`), `${dmg.sha256}  ${DMG_NAME}\n`, "utf8");
  const smokeLog = await fileRecord(root, SMOKE_NAME);
  const buildLog = await fileRecord(root, BUILD_LOG_NAME);
  const gateChecksLog = await fileRecord(root, GATE_CHECKS_LOG_NAME);

  const manifest = {
    schemaVersion: 1,
    artifactKind: "macos-release-gate-evidence",
    distributionAllowed: false,
    factDate: FACT_DATE,
    status: "passed",
    product: "Huabang Entertainment Business System",
    version: VERSION,
    gitCommit: COMMIT,
    platform: PLATFORM,
    target: TARGET,
    appBundleId: "com.banshan.workbench",
    dmg: { ...dmg },
    gates: {
      codesign: "passed",
      notarization: "passed",
      gatekeeper: "passed",
      appStapler: "passed",
      dmgStapler: "passed",
      sidecar: {
        status: "passed",
        executable: true,
        version: "codex-cli 0.144.5",
      },
      startupSmoke: {
        status: "passed",
        durationSeconds: 12,
        log: smokeLog,
      },
    },
    evidence: { buildLog, gateChecksLog },
    workflow: { runId: "123", runAttempt: "1", ref: "refs/heads/main" },
    checksumFile: CHECKSUMS_NAME,
    files: [dmg, await fileRecord(root, `${DMG_NAME}.sha256`), smokeLog, buildLog, gateChecksLog],
    createdAtUtc: CREATED_AT_UTC,
  };

  const state = { root, manifest };
  await mutate(state);
  await writeJson(join(root, MANIFEST_NAME), manifest);
  let checksums = "";
  for (const name of [DMG_NAME, `${DMG_NAME}.sha256`, MANIFEST_NAME, SMOKE_NAME, BUILD_LOG_NAME, GATE_CHECKS_LOG_NAME]) {
    checksums += `${(await fileRecord(root, name)).sha256}  ${name}\n`;
  }
  await writeFile(join(root, CHECKSUMS_NAME), checksums, "utf8");
  return state;
}

async function withFixture(mutate, callback) {
  const fixture = await createFixture(mutate);
  try {
    await callback(fixture);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
}

function verify(root, options = {}) {
  return verifyMacosReleaseEvidence({ releaseRoot: root, ...options });
}

test("accepts the workflow schema and exact evidence closure", async () => {
  await withFixture(async () => {}, async ({ root, manifest }) => {
    assert.equal(manifest.artifactKind, "macos-release-gate-evidence");
    assert.equal(manifest.distributionAllowed, false);
    assert.equal(manifest.gitCommit, COMMIT);
    assert.equal(manifest.platform, PLATFORM);
    assert.deepEqual(await verify(root), {
      factDate: FACT_DATE,
      version: VERSION,
      gitCommit: COMMIT,
      platform: PLATFORM,
      target: TARGET,
      distributionAllowed: false,
      status: "passed",
      dmg: await fileRecord(root, DMG_NAME),
      filesVerified: 6,
      checksumsVerified: 6,
    });
  });
});

test("runs with no arguments from the evidence directory", async () => {
  await withFixture(async () => {}, async ({ root }) => {
    const { stdout } = await execFileAsync(process.execPath, [VERIFIER], { cwd: root });
    assert.equal(JSON.parse(stdout).status, "passed");
  });
});

test("ignores unrelated repository files outside the evidence closure", async () => {
  await withFixture(async ({ root }) => {
    await writeFile(join(root, "package.json"), "{}\n", "utf8");
  }, async ({ root }) => {
    assert.equal((await verify(root)).status, "passed");
  });
});

test("uploads every file required by the macOS evidence closure", async () => {
  const workflow = await readFile(WORKFLOW, "utf8");
  const uploadStep = workflow.match(/- name: 留存禁止分发的 macOS 门禁证据[\s\S]*?(?=\n\s{6}- name:|\s*$)/u)?.[0] ?? "";
  for (const file of [BUILD_LOG_NAME, GATE_CHECKS_LOG_NAME, MANIFEST_NAME, CHECKSUMS_NAME, SMOKE_NAME]) {
    assert.match(uploadStep, new RegExp(`^\\s+${file.replaceAll(".", "\\.")}\\s*$`, "mu"));
  }
});

test("records an explicit success marker after the macOS startup smoke survives", async () => {
  const workflow = await readFile(WORKFLOW, "utf8");
  assert.match(
    workflow,
    /kill -0 "\$PID"[\s\S]{0,240}printf 'startup smoke passed: pid=%s durationSeconds=12\\n' "\$PID" >> "\$RUNNER_TEMP\/macos-smoke\.log"/u,
  );
});
test("rejects publishable or non-gate artifacts", async () => {
  await withFixture(async ({ manifest }) => { manifest.distributionAllowed = true; }, async ({ root }) => {
    await assert.rejects(verify(root), /evidence manifest.distributionAllowed mismatch/u);
  });
  await withFixture(async ({ manifest }) => { manifest.artifactKind = "macos-release-candidate"; }, async ({ root }) => {
    await assert.rejects(verify(root), /evidence manifest.artifactKind mismatch/u);
  });
});

test("rejects a fact date that does not match creation time", async () => {
  await withFixture(async ({ manifest }) => { manifest.factDate = "2026-08-03"; }, async ({ root }) => {
    await assert.rejects(verify(root), /evidence manifest.factDate mismatch/u);
  });
});

test("rejects impossible UTC timestamps and invalid workflow identity", async () => {
  await withFixture(async ({ manifest }) => {
    manifest.factDate = "2026-02-30";
    manifest.createdAtUtc = "2026-02-30T12:00:00Z";
  }, async ({ root }) => {
    await assert.rejects(verify(root), /createdAtUtc must be a real UTC timestamp/u);
  });
  await withFixture(async ({ manifest }) => { manifest.workflow.runAttempt = "0"; }, async ({ root }) => {
    await assert.rejects(verify(root), /workflow.runAttempt must be a positive decimal string/u);
  });
  await withFixture(async ({ manifest }) => { manifest.workflow.ref = "main"; }, async ({ root }) => {
    await assert.rejects(verify(root), /workflow.ref must be a refs\/ path/u);
  });
});

test("rejects DMG names that do not bind version, target, and commit", async () => {
  await withFixture(async ({ manifest }) => { manifest.dmg.name = "renamed.dmg"; }, async ({ root }) => {
    await assert.rejects(verify(root), /evidence manifest.dmg.name mismatch/u);
  });
});

test("rejects empty evidence records", async () => {
  await withFixture(async ({ manifest }) => { manifest.dmg.sizeBytes = 0; }, async ({ root }) => {
    await assert.rejects(verify(root), /evidence manifest.dmg.sizeBytes must be a positive safe integer/u);
  });
});

test("rejects the wrong platform or target", async () => {
  await withFixture(async ({ manifest }) => { manifest.platform = "windows"; }, async ({ root }) => {
    await assert.rejects(verify(root), /evidence manifest.platform mismatch/u);
  });
  await withFixture(async ({ manifest }) => { manifest.target = "x86_64-apple-darwin"; }, async ({ root }) => {
    await assert.rejects(verify(root), /evidence manifest.target mismatch/u);
  });
});

test("rejects requested version or commit mismatches", async () => {
  await withFixture(async () => {}, async ({ root }) => {
    await assert.rejects(verify(root, { version: "1.3.5" }), /evidence manifest.version mismatch/u);
    await assert.rejects(verify(root, { commit: "b".repeat(40) }), /evidence manifest.gitCommit mismatch/u);
  });
});

test("rejects an invalid Git commit field", async () => {
  await withFixture(async ({ manifest }) => { manifest.gitCommit = "main"; }, async ({ root }) => {
    await assert.rejects(verify(root), /gitCommit must be a lowercase 40-character Git SHA/u);
  });
});

test("rejects failed signing, notarization, Gatekeeper, or smoke gates", async () => {
  await withFixture(async ({ manifest }) => { manifest.gates.codesign = "failed"; }, async ({ root }) => {
    await assert.rejects(verify(root), /gates.codesign mismatch/u);
  });
  await withFixture(async ({ manifest }) => { manifest.gates.notarization = "failed"; }, async ({ root }) => {
    await assert.rejects(verify(root), /gates.notarization mismatch/u);
  });
  await withFixture(async ({ manifest }) => { manifest.gates.gatekeeper = "failed"; }, async ({ root }) => {
    await assert.rejects(verify(root), /gates.gatekeeper mismatch/u);
  });
  await withFixture(async ({ manifest }) => { manifest.gates.startupSmoke.status = "failed"; }, async ({ root }) => {
    await assert.rejects(verify(root), /startupSmoke.status mismatch/u);
  });
});

test("rejects sidecar evidence gaps", async () => {
  await withFixture(async ({ manifest }) => { manifest.gates.sidecar.executable = false; }, async ({ root }) => {
    await assert.rejects(verify(root), /sidecar.executable mismatch/u);
  });
  await withFixture(async ({ manifest }) => { manifest.gates.sidecar.version = ""; }, async ({ root }) => {
    await assert.rejects(verify(root), /sidecar.version must be non-empty/u);
  });
});

test("rejects mismatched DMG hash and size", async () => {
  await withFixture(async ({ manifest }) => { manifest.dmg.sha256 = "0".repeat(64); }, async ({ root }) => {
    await assert.rejects(verify(root), /evidence manifest.dmg.sha256 mismatch/u);
  });
  await withFixture(async ({ manifest }) => { manifest.dmg.sizeBytes += 1; }, async ({ root }) => {
    await assert.rejects(verify(root), /evidence manifest.dmg.sizeBytes mismatch/u);
  });
});

test("rejects an inexact manifest file collection", async () => {
  await withFixture(async ({ manifest }) => { manifest.files.pop(); }, async ({ root }) => {
    await assert.rejects(verify(root), /evidence manifest.files set mismatch/u);
  });
});

test("rejects an inexact SHA256SUMS collection", async () => {
  await withFixture(async ({ root }) => {
    await writeFile(join(root, "extra.txt"), "extra\n", "utf8");
  }, async ({ root }) => {
    const path = join(root, CHECKSUMS_NAME);
    const current = await readFile(path, "utf8");
    await writeFile(path, current.split(/\r?\n/u).filter((line) => !line.endsWith(SMOKE_NAME)).join("\n") + "\n", "utf8");
    await assert.rejects(verify(root), /SHA256SUMS file set mismatch/u);
  });
});

test("rejects path traversal in manifest and SHA256SUMS", async () => {
  await withFixture(async ({ manifest }) => { manifest.files[2].name = "../macos-smoke.log"; }, async ({ root }) => {
    await assert.rejects(verify(root), /contains path traversal/u);
  });
  await withFixture(async () => {}, async ({ root }) => {
    const path = join(root, CHECKSUMS_NAME);
    const current = await readFile(path, "utf8");
    await writeFile(path, current.replace(SMOKE_NAME, `../${SMOKE_NAME}`), "utf8");
    await assert.rejects(verify(root), /contains path traversal/u);
  });
});

test("rejects a symbolic-link release root", async (context) => {
  const fixture = await createFixture();
  const aliasParent = await mkdtemp(join(tmpdir(), "bsaigc-macos-evidence-link-"));
  const alias = join(aliasParent, "evidence");
  try {
    try {
      await symlink(fixture.root, alias, process.platform === "win32" ? "junction" : "dir");
    } catch (error) {
      context.skip(`symbolic links unavailable: ${error.code}`);
      return;
    }
    await assert.rejects(verify(alias), /invalid release root/u);
  } finally {
    await rm(aliasParent, { recursive: true, force: true });
    await rm(fixture.root, { recursive: true, force: true });
  }
});
