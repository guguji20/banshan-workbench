import { promises as fs } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const POLICY_NAME = "business-workbench-release-workflow-policy";
const WORKFLOW_DIRECTORY = ".github/workflows";
const WORKFLOW_PATHS = Object.freeze({
  windows: ".github/workflows/build-windows.yml",
  macos: ".github/workflows/build-macos.yml",
  promote: ".github/workflows/promote-business-workbench-1.0.yml",
});

function fail(violations) {
  const error = new Error(`Release workflow policy verification failed:\n- ${violations.join("\n- ")}`);
  error.violations = violations;
  throw error;
}

function activeLines(text) {
  return text.split(/\r?\n/).filter((line) => !line.trimStart().startsWith("#"));
}

function activeText(text) {
  return activeLines(text).join("\n");
}

function parseWorkflowEvents(text, workflowPath, violations) {
  const lines = text.split(/\r?\n/);
  const onIndexes = lines.flatMap((line, index) => (/^on:\s*(?:#.*)?$/.test(line) ? [index] : []));
  if (onIndexes.length !== 1) {
    violations.push(`${workflowPath}: expected exactly one block-style top-level on: mapping.`);
    return [];
  }

  const events = [];
  for (let index = onIndexes[0] + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim() === "" || line.trimStart().startsWith("#")) continue;
    const indentation = line.match(/^\s*/)[0].length;
    if (indentation === 0) break;
    const match = line.match(/^ {2}([A-Za-z_][A-Za-z0-9_-]*):(?:\s|$)/);
    if (match) events.push(match[1]);
  }
  return events;
}

function requireOnlyWorkflowDispatch(text, workflowPath, violations) {
  const events = parseWorkflowEvents(text, workflowPath, violations);
  if (events.length !== 1 || events[0] !== "workflow_dispatch") {
    violations.push(`${workflowPath}: trigger policy requires only workflow_dispatch; found ${events.join(", ") || "none"}.`);
  }
}

function findLineMatches(text, patterns) {
  const results = [];
  text.split(/\r?\n/).forEach((line, index) => {
    if (line.trimStart().startsWith("#")) return;
    if (patterns.some((pattern) => pattern.test(line))) results.push({ line: index + 1, command: line.trim() });
  });
  return results;
}

function findGitHubReleaseMutations(text) {
  return findLineMatches(text, [
    /\bgh\s+release\s+(?:create|edit|delete|upload)\b/i,
    /\bgh\s+api\b[^\n]*\/releases(?:\/|\b)/i,
    /\b(?:curl|wget)\b[^\n]*api\.github\.com\/repos\/[^\s]+\/releases(?:\/|\b)/i,
    /\b(?:github|octokit)(?:\.rest)?\.repos\.(?:create|update|delete)Release\b/i,
    /uses:\s*(?:actions\/(?:create-release|upload-release-asset)|softprops\/action-gh-release|ncipollo\/release-action|marvinpinto\/action-automatic-releases)@/i,
  ]);
}

function findTagMutations(text) {
  const results = findLineMatches(text, [
    /\bgit\s+tag\b/i,
    /\bgit\s+push\b[^\n]*(?:--tags|--follow-tags|refs\/tags)/i,
    /\bgh\s+api\b[^\n]*git\/refs?\/tags/i,
    /uses:\s*(?:anothrNick\/github-tag-action|mathieudutour\/github-tag-action|EndBug\/latest-tag)@/i,
  ]);
  const scriptMutation = activeText(text).match(/(?:createRef|updateRef|deleteRef)[\s\S]{0,240}refs\/tags|refs\/tags[\s\S]{0,240}(?:createRef|updateRef|deleteRef)/i);
  if (scriptMutation) {
    results.push({
      line: activeText(text).slice(0, scriptMutation.index).split("\n").length,
      command: scriptMutation[0].replace(/\s+/g, " ").trim(),
    });
  }
  return results;
}

function unquoteShellToken(token) {
  if ((token.startsWith('"') && token.endsWith('"')) || (token.startsWith("'") && token.endsWith("'"))) return token.slice(1, -1);
  return token;
}

function findCurrentManifestWrites(text) {
  const results = [];
  const token = `(?:"[^"\\r\\n]*"|'[^'\\r\\n]*'|\\S+)`;
  const copyPattern = new RegExp(`\\baws\\s+s3\\s+cp\\s+(${token})\\s+(${token})`, "i");
  const syncPattern = new RegExp(`\\baws\\s+s3\\s+sync\\s+(${token})\\s+(${token})`, "i");
  const currentManifestDestination = /^s3:\/\/.+\/version(?:-mac)?\.json$/i;

  text.split(/\r?\n/).forEach((line, index) => {
    if (line.trimStart().startsWith("#")) return;
    const copy = line.match(copyPattern);
    if (copy && currentManifestDestination.test(unquoteShellToken(copy[2]))) {
      results.push({ line: index + 1, command: line.trim() });
      return;
    }
    const sync = line.match(syncPattern);
    if (sync && /^s3:\/\//i.test(unquoteShellToken(sync[2]))) {
      results.push({ line: index + 1, command: line.trim() });
      return;
    }
    if (/\baws\s+s3api\s+put-object\b/i.test(line)) {
      const key = line.match(new RegExp(`--key\\s+(${token})`, "i"));
      if (key && /(?:^|\/)version(?:-mac)?\.json$/i.test(unquoteShellToken(key[1]))) {
        results.push({ line: index + 1, command: line.trim() });
      }
    }
  });
  return results;
}

function logicalCommandLines(text) {
  const results = [];
  let command = "";
  let startLine = 0;
  text.split(/\r?\n/).forEach((line, index) => {
    if (line.trimStart().startsWith("#")) return;
    const trimmed = line.trim();
    if (!trimmed) return;
    if (!command) startLine = index + 1;
    command = `${command} ${trimmed}`.trim();
    if (/(?:\\|`)\s*$/.test(trimmed)) {
      command = command.replace(/(?:\\|`)\s*$/, "");
      return;
    }
    results.push({ line: startLine, command });
    command = "";
  });
  if (command) results.push({ line: startLine, command });
  return results;
}

function findR2ObjectWrites(text) {
  const results = [];
  const token = `(?:"[^"\\r\\n]*"|'[^'\\r\\n]*'|\\S+)`;
  const transferPattern = new RegExp(`\\baws\\s+s3\\s+(?:cp|sync|mv)\\s+(${token})\\s+(${token})`, "i");
  for (const candidate of logicalCommandLines(text)) {
    const transfer = candidate.command.match(transferPattern);
    if (transfer && /^s3:\/\//i.test(unquoteShellToken(transfer[2]))) {
      results.push(candidate);
      continue;
    }
    if (/\baws\s+s3api\s+put-object\b/i.test(candidate.command)
      || /\bwrangler\s+r2\s+object\s+put\b/i.test(candidate.command)
      || /\brclone\s+(?:copy|copyto|move|moveto|sync)\b[^\n]*(?:\br2:|\bs3:)/i.test(candidate.command)
      || /uses:\s*(?:jakejarvis\/s3-sync-action|shallwefootball\/s3-upload-action|Reggionick\/s3-deploy)@/i.test(candidate.command)) {
      results.push(candidate);
    }
  }
  return results;
}

function hasDistributionAllowedTrue(text) {
  return /\bdistributionAllowed\s*(?::|=)\s*(?:true|\$true)\b/i.test(activeText(text));
}

function requirePattern(text, pattern, message, violations) {
  if (!pattern.test(text)) violations.push(message);
}

function extractYamlEntryBlock(text, indentation, key) {
  const lines = text.split(/\r?\n/);
  const prefix = " ".repeat(indentation);
  const start = lines.findIndex((line) => line === `${prefix}${key}:`);
  if (start < 0) return null;
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (lines[index].trim() === "" || lines[index].trimStart().startsWith("#")) continue;
    if (lines[index].match(/^\s*/)[0].length <= indentation) {
      end = index;
      break;
    }
  }
  return lines.slice(start, end).join("\n");
}

function extractIndentedFunction(text, functionName) {
  const lines = text.split(/\r?\n/);
  const declaration = `${functionName}() {`;
  const start = lines.findIndex((line) => line.trim() === declaration);
  if (start < 0) return null;
  const indentation = lines[start].match(/^\s*/)[0].length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (lines[index].trim() === "}" && lines[index].match(/^\s*/)[0].length === indentation) {
      return lines.slice(start, index + 1).join("\n");
    }
  }
  return null;
}

function validateBuildWorkflow(text, workflowPath, violations) {
  requireOnlyWorkflowDispatch(text, workflowPath, violations);
  if (findGitHubReleaseMutations(text).length > 0) violations.push(`${workflowPath}: build workflows must not create or publish GitHub Releases.`);
  if (findTagMutations(text).length > 0) violations.push(`${workflowPath}: build workflows must not create, push, or delete release tags.`);
  const currentWrites = findCurrentManifestWrites(text);
  if (currentWrites.length > 0) {
    violations.push(`${workflowPath}:${currentWrites[0].line}: build workflows must not update R2 current manifests.`);
  }
  requirePattern(
    text,
    /\bdistributionAllowed\s*(?::|=)\s*(?:false|\$false)\b/i,
    `${workflowPath}: single-platform workflows must emit distributionAllowed=false evidence.`,
    violations,
  );
  if (hasDistributionAllowedTrue(text)) violations.push(`${workflowPath}: single-platform evidence must never set distributionAllowed=true.`);
}

function validateManualConfirmation(text, workflowPath, violations) {
  const confirmation = extractYamlEntryBlock(text, 6, "confirmation");
  if (!confirmation || !/^ {8}required:\s*true\s*$/m.test(confirmation)) {
    violations.push(`${workflowPath}: confirmation must be a required workflow_dispatch input.`);
  }
  requirePattern(
    text,
    /test "\$INPUT_CONFIRMATION" = "PROMOTE-BUSINESS-WORKBENCH-1\.0"/,
    `${workflowPath}: promotion must enforce the exact manual confirmation token.`,
    violations,
  );
}

function validateSameCommitEvidence(text, workflowPath, violations) {
  const requiredMarkers = [
    [/\[\[ "\$INPUT_RELEASE_COMMIT" =~ \^\[0-9a-f\]\{40\}\$ \]\]/, "release_commit must be validated as a full lowercase commit SHA"],
    [/test "\$\(jq -r '\.head_sha' <<<"\$payload"\)" = "\$RELEASE_COMMIT"/, "workflow run head_sha must equal release_commit"],
    [/verify_run "\$WINDOWS_RUN_ID" "\.github\/workflows\/build-windows\.yml"/, "Windows evidence run must be verified"],
    [/verify_run "\$MACOS_RUN_ID" "\.github\/workflows\/build-macos\.yml"/, "macOS evidence run must be verified"],
    [/ref:\s*\$\{\{ inputs\.release_commit \}\}/, "checkout must use release_commit"],
    [/pattern:\s*windows-release-gate-evidence-v\*-\$\{\{ inputs\.release_commit \}\}-\*/, "Windows artifact selection must include release_commit"],
    [/pattern:\s*huabang-business-system-\*-aarch64-apple-darwin-\$\{\{ inputs\.release_commit \}\}-release-gate-evidence/, "macOS artifact selection must include release_commit"],
    [/EXPECTED_COMMIT:\s*\$\{\{ inputs\.release_commit \}\}/, "formal evidence verification must receive release_commit"],
  ];
  for (const [pattern, description] of requiredMarkers) {
    requirePattern(text, pattern, `${workflowPath}: ${description}.`, violations);
  }
}

function validateNativeMacosEvidencePassthrough(text, workflowPath, violations) {
  requirePattern(
    text,
    /macosDir:\s*process\.env\.MACOS_EVIDENCE_DIR/,
    `${workflowPath}: formal verification must pass MACOS_EVIDENCE_DIR directly without schema conversion.`,
    violations,
  );
  const forbiddenPatterns = [
    /\bMACOS_COMPAT_DIR\b/,
    /macos-formal-contract/,
    /native\.nativePlatform\s*=/,
    /native\.nativeGates\s*=/,
    /native\.platform\s*=\s*['"]macos-arm64['"]/,
  ];
  if (forbiddenPatterns.some((pattern) => pattern.test(activeText(text)))) {
    violations.push(`${workflowPath}: promotion must not copy or rewrite the native macOS evidence manifest.`);
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

function validatePublishOrder(text, workflowPath, violations) {
  const markers = [
    ["IMMUTABLE_UPLOAD_STARTED=1", "immutable upload state"],
    ['          for file in .runtime/formal-release/publish/*; do', "immutable R2 asset loop"],
    ['            upload_immutable_object "$file"', "immutable R2 asset upload"],
    ['gh release create "$TAG"', "draft GitHub Release creation"],
    ["            --draft \\", "draft flag"],
    ['gh release edit "$TAG" --draft=false --latest', "formal GitHub Release publication"],
    ["CURRENT_UPDATE_STARTED=1", "current manifest update state"],
    ['publish_current_manifest "version.json"', "version.json current manifest update"],
    ['publish_current_manifest "version-mac.json"', "version-mac.json current manifest update"],
  ];
  let previousIndex = -1;
  for (const [marker, label] of markers) {
    const index = text.indexOf(marker);
    if (index < 0) {
      violations.push(`${workflowPath}: missing ${label} marker.`);
      continue;
    }
    if (index <= previousIndex) violations.push(`${workflowPath}: publish order must be immutable assets -> draft release -> formal GitHub Release -> current manifests; ${label} is out of order.`);
    previousIndex = Math.max(previousIndex, index);
  }

  const cleanupOwnership = findCleanupOwnershipBeforeRelease(text);
  if (!cleanupOwnership.releaseFound || cleanupOwnership.assignments.at(-1) !== 1) {
    violations.push(`${workflowPath}: release/tag cleanup ownership must be armed before draft GitHub Release creation and remain armed.`);
  }

  const currentState = text.indexOf("CURRENT_UPDATE_STARTED=1");
  const currentWrites = findCurrentManifestWrites(text);
  const writtenNames = new Set();
  for (const write of currentWrites) {
    if (write.command.includes("version-mac.json")) writtenNames.add("version-mac.json");
    else if (write.command.includes("version.json")) writtenNames.add("version.json");
    const commandIndex = text.indexOf(write.command);
    if (currentState >= 0 && commandIndex >= 0 && commandIndex < currentState) {
      violations.push(`${workflowPath}:${write.line}: R2 current manifest write occurs before CURRENT_UPDATE_STARTED=1.`);
    }
  }
  for (const name of ["version.json", "version-mac.json"]) {
    if (text.includes(`publish_current_manifest "${name}"`)) writtenNames.add(name);
  }
  for (const name of ["version.json", "version-mac.json"]) {
    if (!writtenNames.has(name)) violations.push(`${workflowPath}: formal publisher must update R2 ${name}.`);
  }
}

function validateRollbackOrder(text, workflowPath, violations) {
  const rollback = extractIndentedFunction(text, "rollback");
  if (!rollback) {
    violations.push(`${workflowPath}: rollback function is missing or malformed.`);
    return;
  }
  const markers = [
    ['restore_current_manifest "version.json"', "restore version.json"],
    ['restore_current_manifest "version-mac.json"', "restore version-mac.json"],
    ["cleanup_immutable_prefix", "clean immutable prefix"],
    ["delete_release_and_tag", "delete GitHub Release/tag"],
  ];
  let previousIndex = -1;
  for (const [marker, label] of markers) {
    const index = rollback.indexOf(marker);
    if (index < 0) {
      violations.push(`${workflowPath}: rollback must ${label}.`);
      continue;
    }
    if (index <= previousIndex) violations.push(`${workflowPath}: rollback order must be current manifests -> immutable prefix -> release/tag; ${label} is out of order.`);
    previousIndex = Math.max(previousIndex, index);
  }
  for (const signal of ["ERR", "INT", "TERM"]) {
    requirePattern(text, new RegExp(`trap 'rollback [^']+' ${signal}`), `${workflowPath}: rollback trap for ${signal} is missing.`, violations);
  }
}

function validatePromoteWorkflow(text, workflowPath, violations) {
  requireOnlyWorkflowDispatch(text, workflowPath, violations);
  validateManualConfirmation(text, workflowPath, violations);
  validateSameCommitEvidence(text, workflowPath, violations);
  validateNativeMacosEvidencePassthrough(text, workflowPath, violations);
  validatePublishOrder(text, workflowPath, violations);
  validateRollbackOrder(text, workflowPath, violations);
  requirePattern(text, /\bgh\s+release\s+create\b/, `${workflowPath}: formal publisher must create the GitHub Release.`, violations);
}

function validateNonPromoteWorkflow(workflow, violations) {
  const releaseMutations = findGitHubReleaseMutations(workflow.text);
  const tagMutations = findTagMutations(workflow.text);
  const currentWrites = findCurrentManifestWrites(workflow.text);
  const r2Writes = findR2ObjectWrites(workflow.text);
  if (releaseMutations.length > 0) {
    violations.push(`${workflow.relativePath}:${releaseMutations[0].line}: only ${WORKFLOW_PATHS.promote} may create or publish GitHub Releases.`);
  }
  if (tagMutations.length > 0) {
    violations.push(`${workflow.relativePath}:${tagMutations[0].line}: only ${WORKFLOW_PATHS.promote} may create, push, or delete release tags.`);
  }
  if (currentWrites.length > 0) {
    violations.push(`${workflow.relativePath}:${currentWrites[0].line}: only ${WORKFLOW_PATHS.promote} may write R2 current manifests.`);
  }
  if (r2Writes.length > 0) {
    violations.push(`${workflow.relativePath}:${r2Writes[0].line}: only ${WORKFLOW_PATHS.promote} may upload release objects to R2/S3.`);
  }
  if (hasDistributionAllowedTrue(workflow.text)) {
    violations.push(`${workflow.relativePath}: non-promotion workflows must not set distributionAllowed=true.`);
  }
}

function validateExclusivePublisher(workflows, violations) {
  const publishers = workflows
    .filter((workflow) => findGitHubReleaseMutations(workflow.text).length > 0
      || findTagMutations(workflow.text).length > 0
      || findCurrentManifestWrites(workflow.text).length > 0
      || findR2ObjectWrites(workflow.text).length > 0)
    .map((workflow) => workflow.relativePath);
  if (publishers.length !== 1 || publishers[0] !== WORKFLOW_PATHS.promote) {
    violations.push(`Only ${WORKFLOW_PATHS.promote} may publish GitHub Releases, tags, or R2 objects; found ${publishers.join(", ") || "none"}.`);
  }
}

async function readWorkflow(rootDir, relativePath) {
  const absolutePath = path.resolve(rootDir, relativePath);
  let text;
  try {
    text = await fs.readFile(absolutePath, "utf8");
  } catch (error) {
    throw new Error(`Unable to read workflow ${absolutePath}: ${error.message}`);
  }
  return { relativePath, absolutePath, text };
}

async function readAllWorkflows(rootDir) {
  const workflowDirectory = path.resolve(rootDir, WORKFLOW_DIRECTORY);
  let entries;
  try {
    entries = await fs.readdir(workflowDirectory, { withFileTypes: true });
  } catch (error) {
    throw new Error(`Unable to read workflow directory ${workflowDirectory}: ${error.message}`);
  }
  const relativePaths = entries
    .filter((entry) => (entry.isFile() || entry.isSymbolicLink()) && /\.ya?ml$/i.test(entry.name))
    .map((entry) => `${WORKFLOW_DIRECTORY}/${entry.name}`)
    .sort();
  return Promise.all(relativePaths.map((relativePath) => readWorkflow(rootDir, relativePath)));
}

export async function verifyReleaseWorkflowPolicy(options = {}) {
  const rootDir = path.resolve(options.rootDir ?? process.cwd());
  const scannedWorkflows = await readAllWorkflows(rootDir);
  const workflowsByPath = new Map(scannedWorkflows.map((workflow) => [workflow.relativePath, workflow]));
  const workflows = Object.fromEntries(Object.entries(WORKFLOW_PATHS).map(([name, relativePath]) => [name, workflowsByPath.get(relativePath)]));
  const violations = [];

  for (const [name, relativePath] of Object.entries(WORKFLOW_PATHS)) {
    if (!workflows[name]) violations.push(`Required workflow is missing: ${relativePath}.`);
  }
  if (workflows.windows) validateBuildWorkflow(workflows.windows.text, workflows.windows.relativePath, violations);
  if (workflows.macos) validateBuildWorkflow(workflows.macos.text, workflows.macos.relativePath, violations);
  if (workflows.promote) validatePromoteWorkflow(workflows.promote.text, workflows.promote.relativePath, violations);
  for (const workflow of scannedWorkflows) {
    if (workflow.relativePath !== WORKFLOW_PATHS.promote) validateNonPromoteWorkflow(workflow, violations);
  }
  validateExclusivePublisher(scannedWorkflows, violations);

  if (violations.length > 0) fail(violations);
  return {
    schemaVersion: 1,
    policy: POLICY_NAME,
    valid: true,
    publisher: WORKFLOW_PATHS.promote,
    workflows: WORKFLOW_PATHS,
    scannedWorkflows: scannedWorkflows.map((workflow) => workflow.relativePath),
  };
}

function parseCliArguments(argumentsList) {
  const options = {};
  for (let index = 0; index < argumentsList.length; index += 1) {
    if (argumentsList[index] === "--root") {
      if (!argumentsList[index + 1]) throw new Error("--root requires a directory path.");
      options.rootDir = argumentsList[index + 1];
      index += 1;
    } else {
      throw new Error(`Unknown argument: ${argumentsList[index]}`);
    }
  }
  return options;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const result = await verifyReleaseWorkflowPolicy(parseCliArguments(process.argv.slice(2)));
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
