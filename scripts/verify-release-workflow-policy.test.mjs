import assert from "node:assert/strict";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { verifyReleaseWorkflowPolicy } from "./verify-release-workflow-policy.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workflowPaths = Object.freeze({
  windows: ".github/workflows/build-windows.yml",
  macos: ".github/workflows/build-macos.yml",
  promote: ".github/workflows/promote-business-workbench-1.0.yml",
});

async function createFixture(context) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "bsaigc-release-workflow-policy-"));
  context.after(() => fs.rm(root, { recursive: true, force: true }));
  await fs.mkdir(path.join(root, ".github", "workflows"), { recursive: true });
  const repositoryWorkflowPaths = (await fs.readdir(path.join(repositoryRoot, ".github", "workflows")))
    .filter((name) => /\.ya?ml$/i.test(name))
    .map((name) => `.github/workflows/${name}`);
  await Promise.all(repositoryWorkflowPaths.map((relativePath) => fs.copyFile(
    path.join(repositoryRoot, relativePath),
    path.join(root, relativePath),
  )));
  return root;
}

async function writeWorkflow(root, name, step) {
  const relativePath = `.github/workflows/${name}`;
  await fs.writeFile(path.join(root, relativePath), `name: Auxiliary workflow\non:\n  workflow_dispatch:\njobs:\n  verify:\n    runs-on: ubuntu-latest\n    steps:\n${step}\n`);
  return relativePath;
}

async function mutateWorkflow(root, workflow, mutate) {
  const filePath = path.join(root, workflowPaths[workflow]);
  const original = await fs.readFile(filePath, "utf8");
  const updated = mutate(original);
  assert.notEqual(updated, original, `Mutation for ${workflow} must change the workflow.`);
  await fs.writeFile(filePath, updated);
}

test("accepts the repository release workflow policy", async () => {
  const result = await verifyReleaseWorkflowPolicy({ rootDir: repositoryRoot });
  assert.equal(result.valid, true);
  assert.equal(result.publisher, workflowPaths.promote);
  assert.deepEqual(result.workflows, workflowPaths);
  assert.deepEqual(result.scannedWorkflows, [workflowPaths.macos, workflowPaths.windows, workflowPaths.promote].sort());
});

test("rejects push and tag triggers in a build workflow", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "windows", (text) => text.replace(
    /on:\r?\n  workflow_dispatch:\r?\n/,
    "on:\n  workflow_dispatch:\n  push:\n    tags:\n      - 'v*'\n",
  ));
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /build-windows\.yml: trigger policy requires only workflow_dispatch/,
  );
});

test("rejects a build workflow that invokes GitHub Release", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "macos", (text) => `${text}\n      - name: Forbidden side publisher\n        run: gh release create v9.9.9\n`);
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /build-macos\.yml: build workflows must not create or publish GitHub Releases|only .*promote-business-workbench-1\.0\.yml may create or publish GitHub Releases|Only .*promote-business-workbench-1\.0\.yml may publish GitHub Releases/,
  );
});

test("rejects a newly added workflow that publishes a GitHub Release", async (context) => {
  const root = await createFixture(context);
  await writeWorkflow(root, "side-release.yml", "      - name: Forbidden publisher\n        run: gh release create v9.9.9");
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /side-release\.yml:\d+: only .*promote-business-workbench-1\.0\.yml may create or publish GitHub Releases/,
  );
});

test("rejects a newly added workflow that uploads any R2 release object", async (context) => {
  const root = await createFixture(context);
  await writeWorkflow(root, "side-r2.yml", "      - name: Forbidden R2 writer\n        run: aws s3api put-object --bucket releases --key rogue.bin --body rogue.bin");
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /side-r2\.yml:\d+: only .*promote-business-workbench-1\.0\.yml may upload release objects to R2\/S3/,
  );
});

test("accepts an auxiliary verification-only workflow", async (context) => {
  const root = await createFixture(context);
  const relativePath = await writeWorkflow(root, "verify-only.yml", "      - name: Verify only\n        run: echo distributionAllowed=false");
  const result = await verifyReleaseWorkflowPolicy({ rootDir: root });
  assert.equal(result.valid, true);
  assert.ok(result.scannedWorkflows.includes(relativePath));
});

test("rejects a build workflow that updates an R2 current manifest", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "windows", (text) => `${text}\n      - name: Forbidden current manifest write\n        run: aws s3 cp version.json "s3://$R2_BUCKET/version.json"\n`);
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /build-windows\.yml:\d+: build workflows must not update R2 current manifests/,
  );
});

test("rejects promotion without the exact manual confirmation gate", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "promote", (text) => text.replace(
    'test "$INPUT_CONFIRMATION" = "PROMOTE-BUSINESS-WORKBENCH-1.0"',
    'test -n "$INPUT_CONFIRMATION"',
  ));
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /promotion must enforce the exact manual confirmation token/,
  );
});

test("rejects promotion when confirmation is not required", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "promote", (text) => text.replace(
    /(      confirmation:\r?\n(?: {8}.*\r?\n)*? {8}required:) true/,
    "$1 false",
  ));
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /confirmation must be a required workflow_dispatch input/,
  );
});

test("rejects promotion without same-commit workflow evidence", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "promote", (text) => text.replace(
    'test "$(jq -r \'.head_sha\' <<<"$payload")" = "$RELEASE_COMMIT"',
    'test -n "$payload"',
  ));
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /workflow run head_sha must equal release_commit/,
  );
});

test("rejects promotion that rewrites native macOS evidence into a compatibility schema", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "promote", (text) => text
    .replace(
      "          MACOS_EVIDENCE_DIR: .runtime/formal-release/macos\n",
      "          MACOS_EVIDENCE_DIR: .runtime/formal-release/macos\n          MACOS_COMPAT_DIR: .runtime/formal-release/macos-formal-contract\n",
    )
    .replace(
      "            macosDir: process.env.MACOS_EVIDENCE_DIR,",
      "            native.platform = 'macos-arm64';\n            macosDir: process.env.MACOS_COMPAT_DIR,",
    ));
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /must pass MACOS_EVIDENCE_DIR directly|must not copy or rewrite the native macOS evidence manifest/,
  );
});

test("rejects current manifest publication before immutable assets and draft release", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "promote", (text) => {
    const withoutCurrentState = text.replace(/^          CURRENT_UPDATE_STARTED=1\r?\n/m, "");
    return withoutCurrentState.replace(
      /^          IMMUTABLE_UPLOAD_STARTED=1\r?\n/m,
      "          CURRENT_UPDATE_STARTED=1\n          IMMUTABLE_UPLOAD_STARTED=1\n",
    );
  });
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /publish order must be immutable assets -> draft release -> formal GitHub Release -> current manifests/,
  );
});

test("rejects current manifest publication before the formal GitHub Release", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "promote", (text) => {
    const releaseBlock = [
      '          gh release edit "$TAG" --draft=false --latest',
      '          test "$(gh release view "$TAG" --json isDraft --jq \'.isDraft\')" = "false"',
      '          test "$(gh release view "$TAG" --json tagName --jq \'.tagName\')" = "$TAG"',
    ].join("\n");
    const withoutReleaseBlock = text.replace(`${releaseBlock}\n\n`, "");
    return withoutReleaseBlock.replace(
      /^          trap - ERR INT TERM$/m,
      `${releaseBlock}\n          trap - ERR INT TERM`,
    );
  });
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /publish order must be immutable assets -> draft release -> formal GitHub Release -> current manifests/,
  );
});

test("rejects release creation without pre-armed cleanup ownership", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "promote", (text) => {
    const arm = "          RELEASE_OR_TAG_CLEANUP_REQUIRED=1\n";
    const create = '          gh release create "$TAG" ' + "\\" + "\n";
    if (!text.includes(arm) || !text.includes(create)) throw new Error("release block not found");
    return text.replace(arm + create, create).replace("            .runtime/formal-release/publish/*\n", "            .runtime/formal-release/publish/*\n" + arm);
  });
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /release\/tag cleanup ownership must be armed before draft GitHub Release creation/,
  );
});

test("rejects cleanup ownership hidden in a disabled branch", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "promote", (text) => {
    const arm = "          RELEASE_OR_TAG_CLEANUP_REQUIRED=1\n";
    if (!text.includes(arm)) throw new Error("release block not found");
    return text.replace(arm, "          if false; then\n" + arm + "          fi\n");
  });
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /cleanup ownership must be armed before draft GitHub Release creation/,
  );
});

test("rejects cleanup ownership reset before release creation", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "promote", (text) => {
    const arm = "          RELEASE_OR_TAG_CLEANUP_REQUIRED=1\n";
    const reset = "          RELEASE_OR_TAG_CLEANUP_REQUIRED=0\n";
    if (!text.includes(arm)) throw new Error("release block not found");
    return text.replace(arm, arm + reset);
  });
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /cleanup ownership must be armed before draft GitHub Release creation/,
  );
});

test("rejects rollback that deletes the release before the immutable prefix", async (context) => {
  const root = await createFixture(context);
  await mutateWorkflow(root, "promote", (text) => {
    const immutableCall = "              cleanup_immutable_prefix || ROLLBACK_FAILED=1";
    const releaseCall = "              delete_release_and_tag || ROLLBACK_FAILED=1";
    return text
      .replace(immutableCall, "__IMMUTABLE_ROLLBACK_CALL__")
      .replace(releaseCall, immutableCall)
      .replace("__IMMUTABLE_ROLLBACK_CALL__", releaseCall);
  });
  await assert.rejects(
    verifyReleaseWorkflowPolicy({ rootDir: root }),
    /rollback order must be current manifests -> immutable prefix -> release\/tag/,
  );
});
