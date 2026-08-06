import assert from "node:assert/strict";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { verifyR2ReleaseTransaction } from "./verify-r2-release-transaction.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workflowPath = ".github/workflows/promote-business-workbench-1.0.yml";

async function withWorkflowMutation(context, mutate) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "bsaigc-r2-release-transaction-"));
  context.after(() => fs.rm(root, { recursive: true, force: true }));
  const destination = path.join(root, workflowPath);
  await fs.mkdir(path.dirname(destination), { recursive: true });
  const original = await fs.readFile(path.join(repositoryRoot, workflowPath), "utf8");
  const updated = mutate(original);
  assert.notEqual(updated, original, "Mutation must change the workflow.");
  await fs.writeFile(destination, updated);
  return root;
}

test("accepts the repository R2 release transaction", async () => {
  const result = await verifyR2ReleaseTransaction({ rootDir: repositoryRoot });
  assert.equal(result.valid, true);
  assert.equal(result.immutableWrite, "if-none-match");
  assert.equal(result.currentWrite, "etag-cas");
  assert.equal(result.rollback, "owned-objects-only");
});

test("rejects an overwrite-capable immutable upload", async (context) => {
  const root = await withWorkflowMutation(context, (text) => text.replace(
    '              --body "$file" \\\n              --if-none-match \'*\' \\',
    '              --body "$file" \\\n              --metadata-directive REPLACE \\',
  ));
  await assert.rejects(verifyR2ReleaseTransaction({ rootDir: root }), /immutable asset creation must reject an existing object/);
});

test("rejects an unconditional current manifest update", async (context) => {
  const root = await withWorkflowMutation(context, (text) => text.replace('                --if-match "$(cat "$state_file.etag")" \\', '                --metadata-directive REPLACE \\'));
  await assert.rejects(verifyR2ReleaseTransaction({ rootDir: root }), /existing current manifests must use If-Match/);
});

test("rejects recursive immutable rollback deletion", async (context) => {
  const root = await withWorkflowMutation(context, (text) => text.replace("          cleanup_immutable_prefix() {", "          cleanup_immutable_prefix() {\n            aws s3 rm \"s3://$R2_BUCKET/$PREFIX/\" --recursive"));
  await assert.rejects(verifyR2ReleaseTransaction({ rootDir: root }), /rollback must not recursively delete the immutable prefix/);
});

test("rejects rollback that can overwrite another current publisher", async (context) => {
  const root = await withWorkflowMutation(context, (text) => text.replace('                --if-match "$(cat "$published_etag_file")" \\', '                --metadata-directive REPLACE \\'));
  await assert.rejects(verifyR2ReleaseTransaction({ rootDir: root }), /rollback restore must only replace the ETag written by this run/);
});


test("rejects cleanup ownership hidden in a disabled branch", async (context) => {
  const root = await withWorkflowMutation(context, (text) => {
    const arm = "          RELEASE_OR_TAG_CLEANUP_REQUIRED=1\n";
    if (!text.includes(arm)) throw new Error("release block not found");
    return text.replace(arm, "          if false; then\n" + arm + "          fi\n");
  });
  await assert.rejects(verifyR2ReleaseTransaction({ rootDir: root }), /cleanup ownership must be armed before release creation starts/);
});

test("rejects cleanup ownership reset before release creation", async (context) => {
  const root = await withWorkflowMutation(context, (text) => {
    const arm = "          RELEASE_OR_TAG_CLEANUP_REQUIRED=1\n";
    const reset = "          RELEASE_OR_TAG_CLEANUP_REQUIRED=0\n";
    if (!text.includes(arm)) throw new Error("release block not found");
    return text.replace(arm, arm + reset);
  });
  await assert.rejects(verifyR2ReleaseTransaction({ rootDir: root }), /cleanup ownership must be armed before release creation starts/);
});

test("rejects release cleanup ownership armed after creation starts", async (context) => {
  const root = await withWorkflowMutation(context, (text) => {
    const arm = "          RELEASE_OR_TAG_CLEANUP_REQUIRED=1\n";
    const finalAsset = "            .runtime/formal-release/publish/*\n";
    if (!text.includes(arm) || !text.includes(finalAsset)) throw new Error("release block not found");
    return text.replace(arm, "").replace(finalAsset, finalAsset + arm);
  });
  await assert.rejects(verifyR2ReleaseTransaction({ rootDir: root }), /cleanup ownership must be armed before release creation starts/);
});
