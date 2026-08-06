import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const workflowRelativePath = ".github/workflows/promote-business-workbench-1.0.yml";

function requirePattern(text, pattern, message) {
  if (!pattern.test(text)) {
    throw new Error(`${workflowRelativePath}: ${message}`);
  }
}

function requireCount(text, pattern, expected, message) {
  const matches = text.match(pattern) ?? [];
  if (matches.length !== expected) {
    throw new Error(`${workflowRelativePath}: ${message}; expected ${expected}, found ${matches.length}`);
  }
}

function findCleanupOwnershipBeforeRelease(text) {
  let inactiveFalseBranchDepth = 0;
  const assignments = [];
  let releaseFound = false;
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line === "" || line.startsWith("#")) continue;
    if (/^if\s+false\s*;\s*then$/.test(line)) {
      inactiveFalseBranchDepth += 1;
      continue;
    }
    if (inactiveFalseBranchDepth > 0) {
      if (/^fi$/.test(line)) inactiveFalseBranchDepth -= 1;
      continue;
    }
    if (/^gh\s+release\s+create\s+"\$TAG"(?:\s|$)/.test(line)) {
      releaseFound = true;
      break;
    }
    const assignment = line.match(/^RELEASE_OR_TAG_CLEANUP_REQUIRED=(0|1)(?:\s+#.*)?$/);
    if (assignment) assignments.push(Number(assignment[1]));
  }
  return { releaseFound, assignments };
}

export async function verifyR2ReleaseTransaction({ rootDir } = {}) {
  const repositoryRoot = rootDir ?? path.resolve(fileURLToPath(new URL("..", import.meta.url)));
  const workflowPath = path.join(repositoryRoot, workflowRelativePath);
  const text = await fs.readFile(workflowPath, "utf8");

  requirePattern(
    text,
    /concurrency:\r?\n  group: business-workbench-1\.0-formal-release\r?\n  cancel-in-progress: false/,
    "formal publication must use one non-cancelling repository concurrency group",
  );
  requirePattern(text, /^    environment: formal-release$/m, "formal publication must use the protected formal-release environment");

  requirePattern(text, /head-object[^\n]+>"\$head_file"/, "current manifest snapshot must persist head-object metadata");
  requirePattern(text, /jq -er '\.ETag[^\n]+> "\$etag_file"/, "current manifest snapshot must persist the previous ETag");
  requirePattern(text, /--if-match "\$\(cat "\$etag_file"\)"/, "current manifest snapshot download must be conditional on its ETag");

  requirePattern(text, /upload_immutable_object\(\)/, "immutable assets must use the owned-object uploader");
  requirePattern(text, /--body "\$file" \\\r?\n              --if-none-match '\*'/, "immutable asset creation must reject an existing object");
  requirePattern(text, /printf '%s\\t%s\\n' "\$key" "\$etag" >> "\$OWNED_IMMUTABLE_OBJECTS"/, "immutable uploads must record owned keys and ETags");
  requirePattern(text, /--if-match "\$etag"/, "immutable readback must use the ETag returned by the upload");
  if (/aws s3 rm[^\n]*\$PREFIX[^\n]*--recursive/.test(text)) {
    throw new Error(`${workflowRelativePath}: rollback must not recursively delete the immutable prefix`);
  }
  requirePattern(text, /if \[ "\$current_etag" != "\$expected_etag" \]/, "immutable rollback must refuse objects no longer owned by this run");
  requirePattern(text, /delete-object --bucket "\$R2_BUCKET" --key "\$key"/, "immutable rollback must delete exact owned keys only");

  requirePattern(text, /publish_current_manifest\(\)/, "current manifests must use the conditional publisher");
  requirePattern(text, /--if-match "\$\(cat "\$state_file\.etag"\)"/, "existing current manifests must use If-Match");
  requirePattern(text, /--body "\$source_file" \\\r?\n                --if-none-match '\*'/, "initial current manifests must use If-None-Match");
  requireCount(text, /^          publish_current_manifest "version(?:-mac)?\.json"/gm, 2, "both current manifests must use the conditional publisher");
  requireCount(text, /published-current\/version(?:-mac)?\.json\.etag/g, 2, "both current readbacks must pin the published ETag");

  requirePattern(text, /if \[ ! -f "\$published_etag_file" \]/, "rollback must skip current manifests not written by this run");
  requirePattern(text, /--if-match "\$\(cat "\$published_etag_file"\)"/, "rollback restore must only replace the ETag written by this run");
  requirePattern(text, /Rollback refused to delete a current manifest changed by another publisher/, "rollback delete must refuse a changed current manifest");

  const cleanupOwnership = findCleanupOwnershipBeforeRelease(text);
  if (!cleanupOwnership.releaseFound || cleanupOwnership.assignments.at(-1) !== 1) {
    throw new Error(`${workflowRelativePath}: GitHub release cleanup ownership must be armed before release creation starts and remain armed`);
  }

  return {
    valid: true,
    workflow: workflowRelativePath,
    concurrencyGroup: "business-workbench-1.0-formal-release",
    environment: "formal-release",
    immutableWrite: "if-none-match",
    currentWrite: "etag-cas",
    rollback: "owned-objects-only",
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const result = await verifyR2ReleaseTransaction();
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
}
