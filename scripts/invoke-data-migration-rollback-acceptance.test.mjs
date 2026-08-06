import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  copyFile,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  stat,
  symlink,
  unlink,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptsRoot = path.dirname(fileURLToPath(import.meta.url));
const sourceScriptPath = path.join(
  scriptsRoot,
  'invoke-data-migration-rollback-acceptance.ps1',
);

function findPowerShell() {
  for (const executable of ['pwsh.exe', 'powershell.exe', 'pwsh', 'powershell']) {
    const probe = spawnSync(
      executable,
      ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command', '$PSVersionTable.PSVersion.ToString()'],
      { encoding: 'utf8', windowsHide: true, timeout: 15_000 },
    );
    if (!probe.error && probe.status === 0) {
      return executable;
    }
  }
  throw new Error('PowerShell is required for the migration rollback acceptance tests.');
}

const powerShellExecutable = findPowerShell();

function runPowerShellFile(scriptPath, args, options = {}) {
  const result = spawnSync(
    powerShellExecutable,
    [
      '-NoLogo',
      '-NoProfile',
      '-NonInteractive',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      scriptPath,
      ...args,
    ],
    {
      cwd: options.cwd,
      env: options.env,
      encoding: 'utf8',
      windowsHide: true,
      timeout: 30_000,
      maxBuffer: 4 * 1024 * 1024,
    },
  );
  if (result.error) {
    throw result.error;
  }
  return result;
}

function parseJsonOutput(stdout) {
  const start = stdout.indexOf('{');
  const end = stdout.lastIndexOf('}');
  assert.notEqual(start, -1, `PowerShell output did not contain JSON:\n${stdout}`);
  assert.ok(end > start, `PowerShell output contained incomplete JSON:\n${stdout}`);
  return JSON.parse(stdout.slice(start, end + 1));
}

async function sha256(filePath) {
  return createHash('sha256').update(await readFile(filePath)).digest('hex');
}

async function snapshotTree(root) {
  const records = [];

  async function visit(currentRoot, relativeRoot = '') {
    const entries = await readdir(currentRoot, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name, 'en'));
    for (const entry of entries) {
      const relativePath = path.join(relativeRoot, entry.name);
      const absolutePath = path.join(currentRoot, entry.name);
      if (entry.isDirectory()) {
        await visit(absolutePath, relativePath);
      } else if (entry.isFile()) {
        const metadata = await stat(absolutePath);
        records.push({
          relativePath: relativePath.replaceAll('\\', '/'),
          length: metadata.size,
          sha256: await sha256(absolutePath),
        });
      } else {
        records.push({
          relativePath: relativePath.replaceAll('\\', '/'),
          type: entry.isSymbolicLink() ? 'symbolic-link' : 'other',
        });
      }
    }
  }

  await visit(root);
  return records;
}

async function createHarness(t) {
  const root = await mkdtemp(path.join(tmpdir(), 'bsaigc-migration-rollback-'));
  const copiedScriptsRoot = path.join(root, 'scripts');
  const copiedScriptPath = path.join(
    copiedScriptsRoot,
    'invoke-data-migration-rollback-acceptance.ps1',
  );
  await mkdir(copiedScriptsRoot, { recursive: true });
  await copyFile(sourceScriptPath, copiedScriptPath);

  const redirectedProfileRoot = path.join(root, 'redirected-user-profile');
  const redirectedRoamingRoot = path.join(redirectedProfileRoot, 'AppData', 'Roaming');
  const redirectedLocalRoot = path.join(redirectedProfileRoot, 'AppData', 'Local');
  const tripwireRoots = [redirectedProfileRoot, redirectedRoamingRoot, redirectedLocalRoot];
  const tripwirePaths = [];
  for (const [index, tripwireRoot] of tripwireRoots.entries()) {
    await mkdir(tripwireRoot, { recursive: true });
    const tripwirePath = path.join(tripwireRoot, `.migration-rollback-tripwire-${index}.txt`);
    await writeFile(tripwirePath, `must remain unchanged:${index}\n`, 'utf8');
    tripwirePaths.push(tripwirePath);
  }

  const environment = {
    ...process.env,
    APPDATA: redirectedRoamingRoot,
    LOCALAPPDATA: redirectedLocalRoot,
    USERPROFILE: redirectedProfileRoot,
    HOME: redirectedProfileRoot,
    POWERSHELL_TELEMETRY_OPTOUT: '1',
  };
  const tripwireSnapshot = await Promise.all(
    tripwirePaths.map(async (tripwirePath) => ({
      tripwirePath,
      sha256: await sha256(tripwirePath),
    })),
  );

  t.after(async () => {
    await rm(root, { recursive: true, force: true });
  });

  return {
    root,
    copiedScriptPath,
    environment,
    acceptanceRoot: path.join(root, '.runtime', 'data-migration-rollback'),
    redirectedRoamingRoot,
    tripwireSnapshot,
  };
}

async function assertTripwiresUnchanged(harness) {
  for (const tripwire of harness.tripwireSnapshot) {
    assert.equal(
      await sha256(tripwire.tripwirePath),
      tripwire.sha256,
      `tripwire changed: ${tripwire.tripwirePath}`,
    );
  }
  await assert.rejects(
    stat(path.join(harness.redirectedRoamingRoot, 'com.banshan.aigc.desktop')),
    { code: 'ENOENT' },
  );
}

function assertPathBelow(candidate, parent, label) {
  const relative = path.relative(path.resolve(parent), path.resolve(candidate));
  assert.ok(
    relative && !relative.startsWith('..') && !path.isAbsolute(relative),
    `${label} escaped ${parent}`,
  );
}

test('PowerShell Parser accepts the script and exposes the expected parameters', () => {
  const escapedScriptPath = sourceScriptPath.replaceAll("'", "''");
  const command = `
$scriptPath = '${escapedScriptPath}'
$tokens = $null
$errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($scriptPath, [ref]$tokens, [ref]$errors)
$result = [ordered]@{
  errors = @($errors | ForEach-Object { $_.Message })
  parameters = @($ast.ParamBlock.Parameters | ForEach-Object { $_.Name.VariablePath.UserPath })
}
$result | ConvertTo-Json -Depth 5
`;
  const result = spawnSync(
    powerShellExecutable,
    ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command', command],
    { encoding: 'utf8', windowsHide: true, timeout: 15_000 },
  );
  if (result.error) {
    throw result.error;
  }
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const parsed = parseJsonOutput(result.stdout);
  assert.deepEqual(parsed.errors, []);
  assert.deepEqual(parsed.parameters, ['RunId', 'DryRun', 'KeepArtifacts']);
});

test('source contract roots all activity in the copied repository runtime', async () => {
  const source = await readFile(sourceScriptPath, 'utf8');
  assert.match(source, /Join-Path\s+\$repoRoot\s+'\.runtime'/);
  assert.match(source, /Join-Path\s+\$runtimeRoot\s+'data-migration-rollback'/);
  assert.match(source, /\^\[A-Za-z0-9\]\[A-Za-z0-9\._-\]\{0,63\}\$/);
  assert.match(source, /function\s+Test-DescendantPath/);
  assert.match(source, /function\s+Assert-SafeRuntimePath/);
  assert.match(source, /FileAttributes\]::ReparsePoint/);
  assert.doesNotMatch(source, /\$env:(?:APPDATA|LOCALAPPDATA|USERPROFILE|HOME)\b/i);
  assert.doesNotMatch(source, /Environment\]::GetFolderPath|SpecialFolder/i);
  assert.doesNotMatch(source, /KnownFolder|SHGetKnownFolderPath/i);
});

test('DryRun reports the complete isolated plan and creates no artifacts', async (t) => {
  const harness = await createHarness(t);
  const runId = `dry-run-${process.pid}`;
  const result = runPowerShellFile(
    harness.copiedScriptPath,
    ['-DryRun', '-RunId', runId],
    { cwd: harness.root, env: harness.environment },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const summary = parseJsonOutput(result.stdout);
  assert.equal(summary.status, 'planned');
  assert.equal(summary.dryRun, true);
  assert.equal(summary.runId, runId);
  assert.deepEqual(
    summary.steps.map(({ name, status }) => ({ name, status })),
    ['path-safety', 'fixture', 'upgrade', 'backup', 'rollback', 'uninstall'].map((name) => ({
      name,
      status: 'planned',
    })),
  );
  assertPathBelow(summary.runRoot, harness.acceptanceRoot, 'dry-run runRoot');
  await assert.rejects(stat(summary.runRoot), { code: 'ENOENT' });
  await assertTripwiresUnchanged(harness);
});

test('full run preserves fixture data through upgrade, backup, rollback, and uninstall', async (t) => {
  const harness = await createHarness(t);
  const runId = `full-run-${process.pid}`;
  const result = runPowerShellFile(
    harness.copiedScriptPath,
    ['-RunId', runId, '-KeepArtifacts'],
    { cwd: harness.root, env: harness.environment },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const summary = parseJsonOutput(result.stdout);
  assert.equal(summary.status, 'passed');
  assert.equal(summary.dryRun, false);
  assert.deepEqual(
    summary.steps.map(({ name, status }) => ({ name, status })),
    ['fixture', 'upgrade', 'backup', 'rollback', 'uninstall'].map((name) => ({
      name,
      status: 'passed',
    })),
  );

  for (const [label, candidate] of Object.entries({
    runRoot: summary.runRoot,
    profileRoot: summary.profileRoot,
    dataRoot: summary.dataRoot,
    installRoot: summary.installRoot,
    rollbackBackupRoot: summary.rollbackBackupRoot,
    rollbackQuarantineRoot: summary.rollbackQuarantineRoot,
  })) {
    assertPathBelow(candidate, harness.acceptanceRoot, label);
  }

  const persistedSummary = JSON.parse(
    await readFile(path.join(summary.runRoot, 'migration-rollback-summary.json'), 'utf8'),
  );
  assert.equal(persistedSummary.status, 'passed');
  assert.equal(persistedSummary.runId, runId);

  const marker = JSON.parse(await readFile(path.join(summary.runRoot, 'run-marker.json'), 'utf8'));
  assert.deepEqual(marker, {
    schemaVersion: 1,
    runId,
    purpose: 'isolated data migration rollback acceptance',
  });

  const restoredSnapshot = await snapshotTree(summary.dataRoot);
  const backupSnapshot = await snapshotTree(summary.rollbackBackupRoot);
  assert.equal(restoredSnapshot.length, 10);
  assert.deepEqual(restoredSnapshot, backupSnapshot);
  await assert.rejects(stat(summary.installRoot), { code: 'ENOENT' });
  assert.equal(
    await readFile(
      path.join(
        summary.rollbackQuarantineRoot,
        'ledger',
        '.nsis-acceptance-sqlite-sentinel.json',
      ),
      'utf8',
    ),
    'corrupted by simulated migration failure',
  );
  await assert.rejects(
    stat(
      path.join(
        summary.rollbackQuarantineRoot,
        'credentials',
        '.nsis-acceptance-credentials-sentinel.json',
      ),
    ),
    { code: 'ENOENT' },
  );
  await assertTripwiresUnchanged(harness);
});

test('backup-manifest.json records verifiable SHA-256 values for every backup file', async (t) => {
  const harness = await createHarness(t);
  const runId = `manifest-${process.pid}`;
  const result = runPowerShellFile(
    harness.copiedScriptPath,
    ['-RunId', runId, '-KeepArtifacts'],
    { cwd: harness.root, env: harness.environment },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const summary = parseJsonOutput(result.stdout);
  const manifestPath = path.join(path.dirname(summary.rollbackBackupRoot), 'backup-manifest.json');
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  assert.ok(Array.isArray(manifest) && manifest.length > 0, 'backup manifest must contain files');

  const backupSnapshot = await snapshotTree(summary.rollbackBackupRoot);
  assert.equal(manifest.length, backupSnapshot.length);
  const seenRelativePaths = new Set();
  for (const record of manifest) {
    assert.equal(typeof record.relativePath, 'string');
    assert.match(record.sha256, /^[a-f0-9]{64}$/);
    assert.equal(seenRelativePaths.has(record.relativePath), false, `duplicate manifest entry: ${record.relativePath}`);
    seenRelativePaths.add(record.relativePath);
    const normalizedRelativePath = record.relativePath.replaceAll('/', path.sep);
    const backupFilePath = path.resolve(summary.rollbackBackupRoot, normalizedRelativePath);
    assertPathBelow(backupFilePath, summary.rollbackBackupRoot, `manifest entry ${record.relativePath}`);
    const metadata = await stat(backupFilePath);
    assert.equal(metadata.size, record.length);
    assert.equal(await sha256(backupFilePath), record.sha256);
  }
  await assertTripwiresUnchanged(harness);
});

test('successful default run removes only its isolated run directory', async (t) => {
  const harness = await createHarness(t);
  const runId = `cleanup-${process.pid}`;
  const result = runPowerShellFile(
    harness.copiedScriptPath,
    ['-RunId', runId],
    { cwd: harness.root, env: harness.environment },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const summary = parseJsonOutput(result.stdout);
  assert.equal(summary.status, 'passed');
  await assert.rejects(stat(summary.runRoot), { code: 'ENOENT' });
  await assertTripwiresUnchanged(harness);
});

test('RunId rejects traversal, absolute, drive-qualified, UNC, and mixed-separator paths', async (t) => {
  const harness = await createHarness(t);
  const absoluteEscape = path.join(harness.root, 'absolute-escape');
  const invalidRunIds = [
    '',
    '.',
    '..',
    '../escape',
    '..\\escape',
    'safe/../escape',
    'safe\\..\\escape',
    'safe/mixed\\escape',
    absoluteEscape,
    'C:drive-relative-escape',
    'C:\\absolute-escape',
    '\\\\invalid-host\\share\\escape',
    '/rooted-escape',
    '-leading-hyphen',
    'contains space',
    'é',
    'a'.repeat(65),
  ];

  for (const runId of invalidRunIds) {
    const result = runPowerShellFile(
      harness.copiedScriptPath,
      ['-RunId', runId, '-KeepArtifacts'],
      { cwd: harness.root, env: harness.environment },
    );
    assert.notEqual(result.status, 0, `unsafe RunId unexpectedly succeeded: ${JSON.stringify(runId)}`);
  }

  await assert.rejects(stat(absoluteEscape), { code: 'ENOENT' });
  await assert.rejects(stat(harness.acceptanceRoot), { code: 'ENOENT' });
  await assertTripwiresUnchanged(harness);
});

test('existing junction in the runtime path is rejected before DryRun', async (t) => {
  const harness = await createHarness(t);
  const runtimeRoot = path.join(harness.root, '.runtime');
  const junctionTarget = path.join(harness.root, 'junction-target-outside-runtime');
  await mkdir(runtimeRoot, { recursive: true });
  await mkdir(junctionTarget, { recursive: true });

  try {
    await symlink(junctionTarget, harness.acceptanceRoot, 'junction');
  } catch (error) {
    if (['EPERM', 'EACCES', 'ENOSYS'].includes(error?.code)) {
      t.skip(`junction creation unavailable on this host: ${error.code}`);
      return;
    }
    throw error;
  }

  const result = runPowerShellFile(
    harness.copiedScriptPath,
    ['-DryRun', '-RunId', `junction-${process.pid}`],
    { cwd: harness.root, env: harness.environment },
  );
  assert.notEqual(result.status, 0);
  assert.match(`${result.stdout}\n${result.stderr}`, /reparse point rejected/i);
  assert.deepEqual(await readdir(junctionTarget), []);
  await assertTripwiresUnchanged(harness);

  if ((await lstat(harness.acceptanceRoot)).isSymbolicLink()) {
    await unlink(harness.acceptanceRoot);
  }
});