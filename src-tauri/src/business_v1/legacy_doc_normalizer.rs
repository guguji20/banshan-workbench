use crate::protocol::HostError;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zip::{CompressionMethod, ZipArchive};

pub(crate) const LEGACY_DOC_CONVERTER_ENGINE: &str = "microsoft-word-com";
pub(crate) const LEGACY_DOC_CONVERTER_POLICY_VERSION: &str = "word-only.v1";
pub(crate) const DEFAULT_CONVERSION_TIMEOUT: Duration = Duration::from_secs(120);

const MAX_LEGACY_DOC_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DOCX_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PACKAGE_ENTRIES: usize = 2_048;
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PROCESS_OUTPUT_BYTES: usize = 64 * 1024;
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(50);
const PRIVATE_WORK_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const PRIVATE_WORK_CLEANUP_INTERVAL: Duration = Duration::from_millis(50);
const OLE_COMPOUND_FILE_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const CONTENT_TYPES_PATH: &str = "[Content_Types].xml";
const ROOT_RELATIONSHIPS_PATH: &str = "_rels/.rels";
const DOCUMENT_PATH: &str = "word/document.xml";
const WORD_VERSION_MARKER: &str = "BSAIGC_WORD_VERSION=";
const WORD_UNAVAILABLE_EXIT_CODE: i32 = 20;

const WORD_AUTOMATION_CLEANUP_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$baseline = @(
    $env:BSAIGC_WORD_BASELINE_IDS -split ',' |
        Where-Object { $_ -match '^\d+$' } |
        ForEach-Object { [uint32]$_ }
)
Get-CimInstance Win32_Process -Filter "Name = 'WINWORD.EXE'" |
    Where-Object {
        $baseline -notcontains [uint32]$_.ProcessId -and
        $_.CommandLine -match '(?i)/Automation\s+-Embedding'
    } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
"#;

static WORK_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

const WORD_CONVERSION_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$source = [IO.Path]::GetFullPath($env:BSAIGC_LEGACY_DOC_SOURCE)
$output = [IO.Path]::GetFullPath($env:BSAIGC_LEGACY_DOC_OUTPUT)
$wordPidsFile = [IO.Path]::GetFullPath($env:BSAIGC_LEGACY_DOC_WORD_PIDS)
$wordBaselineFile = [IO.Path]::GetFullPath($env:BSAIGC_LEGACY_DOC_WORD_BASELINE)
$beforeWordPids = @(Get-Process -Name WINWORD -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
[IO.File]::WriteAllLines($wordBaselineFile, [string[]]$beforeWordPids)
$ownedWordPids = @()
$word = $null
$document = $null
$stage = 'create'
try {
    try {
        $word = New-Object -ComObject Word.Application
    } catch {
        exit 20
    }
    $stage = 'configure'
    $word.Visible = $false
    $word.DisplayAlerts = 0
    $word.AutomationSecurity = 3
    $word.Options.ConfirmConversions = $false
    $word.Options.UpdateLinksAtOpen = $false
    $word.Options.SaveNormalPrompt = $false
    $stage = 'open'
    $document = $word.Documents.Open($source, $false, $true, $false)
    $stage = 'ownership'
    for ($attempt = 0; $attempt -lt 100 -and $ownedWordPids.Count -eq 0; $attempt++) {
        $ownedWordPids = @(
            Get-Process -Name WINWORD -ErrorAction SilentlyContinue |
                Where-Object { $beforeWordPids -notcontains $_.Id } |
                ForEach-Object { $_.Id }
        )
        if ($ownedWordPids.Count -eq 0) { Start-Sleep -Milliseconds 20 }
    }
    if ($ownedWordPids.Count -eq 0) { throw 'Word process ownership could not be established' }
    [IO.File]::WriteAllLines($wordPidsFile, [string[]]$ownedWordPids)
    $stage = 'save'
    $document.SaveAs2($output, 16)
    $stage = 'complete'
    [Console]::Out.WriteLine('BSAIGC_WORD_VERSION=' + $word.Version)
    exit 0
} catch {
    [Console]::Out.WriteLine('BSAIGC_WORD_FAILURE_STAGE=' + $stage)
    [Console]::Out.WriteLine('BSAIGC_WORD_FAILURE_TYPE=' + $_.Exception.GetType().FullName)
    [Console]::Out.WriteLine('BSAIGC_WORD_FAILURE_HRESULT=' + $_.Exception.HResult)
    exit 21
} finally {
    if ($ownedWordPids.Count -eq 0) {
        $ownedWordPids = @(
            Get-Process -Name WINWORD -ErrorAction SilentlyContinue |
                Where-Object { $beforeWordPids -notcontains $_.Id } |
                ForEach-Object { $_.Id }
        )
        try { [IO.File]::WriteAllLines($wordPidsFile, [string[]]$ownedWordPids) } catch {}
    }
    if ($null -ne $document) {
        try { $document.Close($false) } catch {}
        try { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($document) } catch {}
    }
    if ($null -ne $word) {
        try { $word.Quit($false) } catch {}
        try { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($word) } catch {}
    }
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
    $deadline = [DateTime]::UtcNow.AddSeconds(3)
    while ([DateTime]::UtcNow -lt $deadline) {
        $remaining = @($ownedWordPids | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
        if ($remaining.Count -eq 0) { break }
        Start-Sleep -Milliseconds 50
    }
    foreach ($ownedWordPid in $ownedWordPids) {
        try { Stop-Process -Id $ownedWordPid -Force -ErrorAction SilentlyContinue } catch {}
    }
}
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegacyDocNormalizationResult {
    pub source_sha256: String,
    pub output_sha256: String,
    pub output_size_bytes: u64,
    pub converter_engine: String,
    pub converter_version: String,
    pub converter_policy_version: String,
}

#[derive(Debug)]
struct RunnerRequest<'a> {
    source: &'a Path,
    output: &'a Path,
    working_directory: &'a Path,
    timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunnerSuccess {
    converter_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunnerError {
    Unavailable,
    TimedOut,
    Failed,
}

trait LegacyDocRunner {
    fn run(&self, request: &RunnerRequest<'_>) -> Result<RunnerSuccess, RunnerError>;
}

struct WordComRunner;

pub(crate) fn normalize_legacy_doc(
    source: &Path,
    expected_source_sha256: &str,
    output: &Path,
) -> Result<LegacyDocNormalizationResult, HostError> {
    normalize_legacy_doc_with_timeout(
        source,
        expected_source_sha256,
        output,
        DEFAULT_CONVERSION_TIMEOUT,
    )
}

pub(crate) fn normalize_legacy_doc_with_timeout(
    source: &Path,
    expected_source_sha256: &str,
    output: &Path,
    timeout: Duration,
) -> Result<LegacyDocNormalizationResult, HostError> {
    normalize_with_runner(
        source,
        expected_source_sha256,
        output,
        timeout,
        &WordComRunner,
    )
}

fn normalize_with_runner(
    source: &Path,
    expected_source_sha256: &str,
    output: &Path,
    timeout: Duration,
    runner: &dyn LegacyDocRunner,
) -> Result<LegacyDocNormalizationResult, HostError> {
    validate_request(source, expected_source_sha256, output, timeout)?;
    let source_bytes = read_file_limited(source, MAX_LEGACY_DOC_BYTES).map_err(|_| {
        error(
            "BUSINESS_LEGACY_DOC_SOURCE_INVALID",
            "legacy DOC source could not be read within the size limit",
            false,
        )
    })?;
    if !source_bytes.starts_with(&OLE_COMPOUND_FILE_MAGIC) {
        return Err(error(
            "BUSINESS_LEGACY_DOC_SOURCE_INVALID",
            "legacy DOC source is not an OLE compound document",
            false,
        ));
    }
    let source_sha256 = sha256_bytes(&source_bytes);
    if !source_sha256.eq_ignore_ascii_case(expected_source_sha256) {
        return Err(error(
            "BUSINESS_LEGACY_DOC_SOURCE_SHA_MISMATCH",
            "legacy DOC source does not match the registered SHA-256",
            false,
        ));
    }

    let work_directory = PrivateWorkDirectory::create(output)?;
    let staged_source = work_directory.path().join("source.doc");
    write_new_file(&staged_source, &source_bytes)?;

    let request = RunnerRequest {
        source: &staged_source,
        output,
        working_directory: work_directory.path(),
        timeout,
    };
    let runner_result = runner.run(&request);
    if let Err(runner_error) = runner_result {
        remove_output_if_present(output)?;
        work_directory.cleanup()?;
        return Err(map_runner_error(runner_error));
    }
    let runner_success = runner_result.expect("runner error handled above");

    let output_bytes = match read_file_limited(output, MAX_DOCX_BYTES) {
        Ok(bytes) => bytes,
        Err(_) => {
            remove_output_if_present(output)?;
            work_directory.cleanup()?;
            return Err(output_invalid(
                "Word did not produce a readable DOCX output",
            ));
        }
    };
    if let Err(validation_error) = validate_safe_docx(&output_bytes) {
        remove_output_if_present(output)?;
        work_directory.cleanup()?;
        return Err(validation_error);
    }
    let output_size_bytes = output_bytes.len() as u64;
    let output_sha256 = sha256_bytes(&output_bytes);
    work_directory.cleanup()?;

    Ok(LegacyDocNormalizationResult {
        source_sha256,
        output_sha256,
        output_size_bytes,
        converter_engine: LEGACY_DOC_CONVERTER_ENGINE.to_string(),
        converter_version: runner_success.converter_version,
        converter_policy_version: LEGACY_DOC_CONVERTER_POLICY_VERSION.to_string(),
    })
}

fn validate_request(
    source: &Path,
    expected_source_sha256: &str,
    output: &Path,
    timeout: Duration,
) -> Result<(), HostError> {
    if !source.is_absolute() || !output.is_absolute() {
        return Err(error(
            "BUSINESS_LEGACY_DOC_PATH_INVALID",
            "legacy DOC normalization requires absolute backend paths",
            false,
        ));
    }
    if !source.is_file() || !has_extension(source, "doc") {
        return Err(error(
            "BUSINESS_LEGACY_DOC_SOURCE_INVALID",
            "legacy DOC source must be an existing .doc file",
            false,
        ));
    }
    if !has_extension(output, "docx") || output.exists() {
        return Err(error(
            "BUSINESS_LEGACY_DOC_OUTPUT_INVALID",
            "legacy DOC output must be a new .docx file",
            false,
        ));
    }
    let output_parent = output
        .parent()
        .filter(|path| path.is_dir())
        .ok_or_else(|| {
            error(
                "BUSINESS_LEGACY_DOC_OUTPUT_INVALID",
                "legacy DOC output parent directory does not exist",
                false,
            )
        })?;
    let source_parent = source.parent().unwrap_or_else(|| Path::new(""));
    if source_parent == output_parent && source.file_stem() == output.file_stem() {
        return Err(error(
            "BUSINESS_LEGACY_DOC_PATH_INVALID",
            "legacy DOC source and output must use distinct backend paths",
            false,
        ));
    }
    if expected_source_sha256.len() != 64
        || !expected_source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(error(
            "BUSINESS_LEGACY_DOC_SOURCE_SHA_INVALID",
            "registered legacy DOC SHA-256 must contain 64 hexadecimal characters",
            false,
        ));
    }
    if timeout.is_zero() {
        return Err(error(
            "BUSINESS_LEGACY_DOC_TIMEOUT_INVALID",
            "legacy DOC conversion timeout must be positive",
            false,
        ));
    }
    Ok(())
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

impl LegacyDocRunner for WordComRunner {
    fn run(&self, request: &RunnerRequest<'_>) -> Result<RunnerSuccess, RunnerError> {
        run_word_com(request)
    }
}

#[cfg(windows)]
fn run_word_com(request: &RunnerRequest<'_>) -> Result<RunnerSuccess, RunnerError> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

    let system_root = std::env::var_os("SystemRoot").ok_or(RunnerError::Unavailable)?;
    let powershell = PathBuf::from(&system_root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    if !powershell.is_file() {
        return Err(RunnerError::Unavailable);
    }

    let encoded_script = encode_powershell_script(WORD_CONVERSION_SCRIPT);
    let word_pids_file = request.working_directory.join("owned-word-pids.txt");
    let word_baseline_file = request.working_directory.join("baseline-word-pids.txt");
    let mut command = Command::new(powershell);
    isolate_word_environment(&mut command, request.working_directory, &system_root);
    command
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-EncodedCommand")
        .arg(encoded_script)
        .current_dir(request.working_directory)
        .env("BSAIGC_LEGACY_DOC_SOURCE", request.source)
        .env("BSAIGC_LEGACY_DOC_OUTPUT", request.output)
        .env("BSAIGC_LEGACY_DOC_WORD_PIDS", &word_pids_file)
        .env("BSAIGC_LEGACY_DOC_WORD_BASELINE", &word_baseline_file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);

    let mut child = command.spawn().map_err(|_| RunnerError::Unavailable)?;
    let stdout = child.stdout.take().ok_or(RunnerError::Failed)?;
    let stderr = child.stderr.take().ok_or(RunnerError::Failed)?;
    let stdout_reader = thread::spawn(move || read_process_stream(stdout));
    let stderr_reader = thread::spawn(move || read_process_stream(stderr));
    let started = Instant::now();

    let status = loop {
        if started.elapsed() >= request.timeout {
            terminate_process_tree(&mut child, &system_root);
            terminate_word_automation(
                &word_pids_file,
                &word_baseline_file,
                request.working_directory,
                &system_root,
            );
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(RunnerError::TimedOut);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(PROCESS_POLL_INTERVAL),
            Err(_) => {
                terminate_process_tree(&mut child, &system_root);
                terminate_word_automation(
                    &word_pids_file,
                    &word_baseline_file,
                    request.working_directory,
                    &system_root,
                );
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(RunnerError::Failed);
            }
        }
    };

    let stdout = stdout_reader.join().map_err(|_| RunnerError::Failed)??;
    let _ = stderr_reader.join();
    terminate_word_automation(
        &word_pids_file,
        &word_baseline_file,
        request.working_directory,
        &system_root,
    );
    if status.code() == Some(WORD_UNAVAILABLE_EXIT_CODE) {
        return Err(RunnerError::Unavailable);
    }
    if !status.success() || !request.output.is_file() {
        return Err(RunnerError::Failed);
    }
    let converter_version = parse_word_version(&stdout).ok_or(RunnerError::Failed)?;
    Ok(RunnerSuccess { converter_version })
}

#[cfg(not(windows))]
fn run_word_com(_request: &RunnerRequest<'_>) -> Result<RunnerSuccess, RunnerError> {
    Err(RunnerError::Unavailable)
}

#[cfg(windows)]
fn isolate_word_environment(command: &mut Command, working_directory: &Path, system_root: &OsStr) {
    command.env_clear();
    command
        .env("SystemRoot", system_root)
        .env("WINDIR", system_root)
        .env("TEMP", working_directory)
        .env("TMP", working_directory);
    for name in [
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "HOMEDRIVE",
        "HOMEPATH",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

#[cfg(windows)]
fn terminate_process_tree(child: &mut Child, system_root: &OsStr) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let taskkill = PathBuf::from(system_root)
        .join("System32")
        .join("taskkill.exe");
    if taskkill.is_file() {
        let _ = Command::new(taskkill)
            .arg("/PID")
            .arg(child.id().to_string())
            .arg("/T")
            .arg("/F")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
fn terminate_word_automation(
    word_pids_file: &Path,
    word_baseline_file: &Path,
    working_directory: &Path,
    system_root: &OsStr,
) {
    if terminate_owned_word_processes(word_pids_file, system_root) {
        return;
    }
    terminate_new_word_automation_processes(word_baseline_file, working_directory, system_root);
}

#[cfg(windows)]
fn terminate_owned_word_processes(word_pids_file: &Path, system_root: &OsStr) -> bool {
    let Ok(contents) = fs::read_to_string(word_pids_file) else {
        return false;
    };
    let process_ids = contents
        .lines()
        .take(8)
        .filter_map(|value| value.trim().parse::<u32>().ok())
        .filter(|process_id| *process_id != 0 && *process_id != std::process::id())
        .collect::<Vec<_>>();
    for process_id in &process_ids {
        terminate_process_id(*process_id, system_root);
    }
    !process_ids.is_empty()
}

#[cfg(windows)]
fn terminate_new_word_automation_processes(
    word_baseline_file: &Path,
    working_directory: &Path,
    system_root: &OsStr,
) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(contents) = fs::read_to_string(word_baseline_file) else {
        return;
    };
    let baseline = contents
        .lines()
        .take(64)
        .filter_map(|value| value.trim().parse::<u32>().ok())
        .filter(|process_id| *process_id != 0)
        .map(|process_id| process_id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let powershell = PathBuf::from(system_root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    if !powershell.is_file() {
        return;
    }
    let mut command = Command::new(powershell);
    isolate_word_environment(&mut command, working_directory, system_root);
    let _ = command
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-EncodedCommand")
        .arg(encode_powershell_script(WORD_AUTOMATION_CLEANUP_SCRIPT))
        .current_dir(working_directory)
        .env("BSAIGC_WORD_BASELINE_IDS", baseline)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

#[cfg(windows)]
fn terminate_process_id(process_id: u32, system_root: &OsStr) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let taskkill = PathBuf::from(system_root)
        .join("System32")
        .join("taskkill.exe");
    if taskkill.is_file() {
        let _ = Command::new(taskkill)
            .arg("/PID")
            .arg(process_id.to_string())
            .arg("/T")
            .arg("/F")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
}

fn read_process_stream(mut stream: impl Read) -> Result<Vec<u8>, RunnerError> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4 * 1024];
    loop {
        let read = stream.read(&mut buffer).map_err(|_| RunnerError::Failed)?;
        if read == 0 {
            return Ok(output);
        }
        let remaining = MAX_PROCESS_OUTPUT_BYTES.saturating_sub(output.len());
        output.extend_from_slice(&buffer[..read.min(remaining)]);
    }
}

fn encode_powershell_script(script: &str) -> String {
    let bytes = script
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    BASE64_STANDARD.encode(bytes)
}

fn parse_word_version(stdout: &[u8]) -> Option<String> {
    String::from_utf8_lossy(stdout).lines().find_map(|line| {
        let version = line.trim().strip_prefix(WORD_VERSION_MARKER)?.trim();
        (!version.is_empty()
            && version.len() <= 64
            && version
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')))
        .then(|| version.to_string())
    })
}

fn map_runner_error(error_kind: RunnerError) -> HostError {
    match error_kind {
        RunnerError::Unavailable => error(
            "BUSINESS_WORD_AUTOMATION_UNAVAILABLE",
            "Microsoft Word automation is unavailable",
            false,
        ),
        RunnerError::TimedOut => error(
            "BUSINESS_LEGACY_DOC_CONVERSION_TIMEOUT",
            "Microsoft Word legacy DOC conversion timed out",
            true,
        ),
        RunnerError::Failed => error(
            "BUSINESS_LEGACY_DOC_CONVERSION_FAILED",
            "Microsoft Word could not normalize the legacy DOC",
            true,
        ),
    }
}

struct PrivateWorkDirectory {
    path: Option<PathBuf>,
}

impl PrivateWorkDirectory {
    fn create(output: &Path) -> Result<Self, HostError> {
        let parent = output.parent().ok_or_else(|| {
            error(
                "BUSINESS_LEGACY_DOC_OUTPUT_INVALID",
                "legacy DOC output parent directory is missing",
                false,
            )
        })?;
        let epoch_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| cleanup_error("system clock is unavailable"))?
            .as_nanos();
        for _ in 0..32 {
            let counter = WORK_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
            let candidate = parent.join(format!(
                ".legacy-doc-normalize.{}.{}.{}",
                std::process::id(),
                epoch_nanos,
                counter
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => {
                    return Ok(Self {
                        path: Some(candidate),
                    })
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => {
                    return Err(cleanup_error(
                        "could not create the private legacy DOC work directory",
                    ));
                }
            }
        }
        Err(cleanup_error(
            "could not allocate the private legacy DOC work directory",
        ))
    }

    fn path(&self) -> &Path {
        self.path
            .as_deref()
            .expect("private work directory must exist before cleanup")
    }

    fn cleanup(mut self) -> Result<(), HostError> {
        if let Some(path) = self.path.take() {
            remove_private_work_directory(&path).map_err(|_| {
                cleanup_error("could not remove the private legacy DOC work directory")
            })?;
        }
        Ok(())
    }
}

impl Drop for PrivateWorkDirectory {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = remove_private_work_directory(&path);
        }
    }
}

fn remove_private_work_directory(path: &Path) -> Result<(), std::io::Error> {
    let started = Instant::now();
    loop {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) if started.elapsed() >= PRIVATE_WORK_CLEANUP_TIMEOUT => return Err(error),
            Err(_) => thread::sleep(PRIVATE_WORK_CLEANUP_INTERVAL),
        }
    }
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), HostError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| cleanup_error("could not stage the registered legacy DOC source"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| cleanup_error("could not persist the staged legacy DOC source"))
}

fn remove_output_if_present(output: &Path) -> Result<(), HostError> {
    match fs::remove_file(output) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(cleanup_error(
            "could not remove the failed legacy DOC conversion output",
        )),
    }
}

fn read_file_limited(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, std::io::Error> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take(maximum_bytes + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file exceeds the configured size limit",
        ));
    }
    Ok(bytes)
}

fn validate_safe_docx(bytes: &[u8]) -> Result<(), HostError> {
    if bytes.len() as u64 > MAX_DOCX_BYTES {
        return Err(output_invalid("normalized DOCX exceeds the size limit"));
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| output_invalid("normalized DOCX is not a valid ZIP package"))?;
    if archive.is_empty() || archive.len() > MAX_PACKAGE_ENTRIES {
        return Err(output_invalid(
            "normalized DOCX contains an invalid number of package entries",
        ));
    }

    let mut names = HashSet::with_capacity(archive.len());
    let mut required = HashSet::new();
    let mut total_uncompressed = 0_u64;
    let mut xml_entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| output_invalid("normalized DOCX package entry could not be read"))?;
        let name = entry.name().to_string();
        validate_package_entry_name(&name)?;
        if entry.encrypted() || entry.is_symlink() {
            return Err(output_invalid(
                "normalized DOCX contains an encrypted or linked package entry",
            ));
        }
        if !matches!(
            entry.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err(output_invalid(
                "normalized DOCX uses an unsupported compression method",
            ));
        }
        if entry.size() > MAX_ENTRY_BYTES {
            return Err(output_invalid(
                "normalized DOCX contains an oversized entry",
            ));
        }
        total_uncompressed = total_uncompressed
            .checked_add(entry.size())
            .ok_or_else(|| output_invalid("normalized DOCX package size overflowed"))?;
        if total_uncompressed > MAX_PACKAGE_BYTES {
            return Err(output_invalid(
                "normalized DOCX expands beyond the package size limit",
            ));
        }
        if !names.insert(name.to_ascii_lowercase()) {
            return Err(output_invalid(
                "normalized DOCX contains duplicate package entries",
            ));
        }
        let lower_name = name.to_ascii_lowercase();
        if is_forbidden_package_part(&lower_name) {
            return Err(output_invalid(
                "normalized DOCX contains macro, OLE, or ActiveX content",
            ));
        }
        if matches!(
            name.as_str(),
            CONTENT_TYPES_PATH | ROOT_RELATIONSHIPS_PATH | DOCUMENT_PATH
        ) {
            required.insert(name.clone());
        }
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let mut contents = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut contents)
                .map_err(|_| output_invalid("normalized DOCX XML entry could not be read"))?;
            xml_entries.push((name, contents));
        }
    }
    if required.len() != 3 {
        return Err(output_invalid(
            "normalized DOCX is missing required package entries",
        ));
    }

    let mut standard_main_document = false;
    for (name, contents) in &xml_entries {
        if name == CONTENT_TYPES_PATH {
            standard_main_document = validate_content_types(contents)?;
        } else if name.ends_with(".rels") {
            validate_relationships(contents)?;
        }
        if name.starts_with("word/") && name.ends_with(".xml") {
            validate_word_xml(contents)?;
        }
    }
    if !standard_main_document {
        return Err(output_invalid(
            "normalized DOCX does not declare a standard macro-free main document",
        ));
    }
    Ok(())
}

fn validate_package_entry_name(name: &str) -> Result<(), HostError> {
    if name.is_empty()
        || name.contains('\0')
        || name.contains('\\')
        || name.starts_with('/')
        || has_windows_drive_prefix(name)
    {
        return Err(output_invalid(
            "normalized DOCX contains an unsafe package path",
        ));
    }
    let trimmed = name.trim_end_matches('/');
    if trimmed.is_empty()
        || trimmed
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(output_invalid(
            "normalized DOCX contains a traversing package path",
        ));
    }
    Ok(())
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn is_forbidden_package_part(lower_name: &str) -> bool {
    lower_name.ends_with("vbaproject.bin")
        || lower_name.contains("/macros/")
        || lower_name.contains("/activex/")
        || lower_name.contains("/embeddings/")
        || lower_name.contains("oleobject")
        || lower_name.ends_with(".docm")
}

fn validate_content_types(xml: &[u8]) -> Result<bool, HostError> {
    let mut reader = xml_reader(xml);
    let mut buffer = Vec::new();
    let mut standard_main_document = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start) | Event::Empty(start)) => {
                let local = start.local_name();
                if !matches!(local.as_ref(), b"Default" | b"Override") {
                    buffer.clear();
                    continue;
                }
                let content_type = attribute(&start, b"ContentType")?.unwrap_or_default();
                let lower = content_type.to_ascii_lowercase();
                if lower.contains("macroenabled")
                    || lower.contains("vbaproject")
                    || lower.contains("activex")
                    || lower.contains("oleobject")
                {
                    return Err(output_invalid(
                        "normalized DOCX content types declare active content",
                    ));
                }
                if local.as_ref() == b"Override"
                    && attribute(&start, b"PartName")?.as_deref() == Some("/word/document.xml")
                {
                    standard_main_document = content_type
                        == "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(output_invalid("normalized DOCX contains malformed XML")),
        }
        buffer.clear();
    }
    Ok(standard_main_document)
}

fn validate_relationships(xml: &[u8]) -> Result<(), HostError> {
    let mut reader = xml_reader(xml);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start) | Event::Empty(start))
                if start.local_name().as_ref() == b"Relationship" =>
            {
                let target_mode = attribute(&start, b"TargetMode")?.unwrap_or_default();
                let relationship_type = attribute(&start, b"Type")?.unwrap_or_default();
                let target = attribute(&start, b"Target")?.unwrap_or_default();
                let lower_type = relationship_type.to_ascii_lowercase();
                if target_mode.eq_ignore_ascii_case("external")
                    || relationship_target_is_absolute(&target)
                {
                    return Err(output_invalid(
                        "normalized DOCX contains an external relationship",
                    ));
                }
                if lower_type.contains("macro")
                    || lower_type.contains("vbaproject")
                    || lower_type.contains("activex")
                    || lower_type.ends_with("/oleobject")
                    || lower_type.ends_with("/package")
                {
                    return Err(output_invalid(
                        "normalized DOCX relationship declares active content",
                    ));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => {
                return Err(output_invalid(
                    "normalized DOCX contains malformed relationships XML",
                ));
            }
        }
        buffer.clear();
    }
    Ok(())
}

fn validate_word_xml(xml: &[u8]) -> Result<(), HostError> {
    let mut reader = xml_reader(xml);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start) | Event::Empty(start)) => {
                let local = start.local_name();
                if matches!(
                    local.as_ref(),
                    b"altChunk" | b"object" | b"oleObject" | b"control"
                ) {
                    return Err(output_invalid(
                        "normalized DOCX contains altChunk, OLE, or ActiveX markup",
                    ));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => {
                return Err(output_invalid(
                    "normalized DOCX contains malformed Word XML",
                ))
            }
        }
        buffer.clear();
    }
    Ok(())
}

fn xml_reader(xml: &[u8]) -> Reader<&[u8]> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().check_end_names = true;
    reader
}

fn attribute(start: &BytesStart<'_>, name: &[u8]) -> Result<Option<String>, HostError> {
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute
            .map_err(|_| output_invalid("normalized DOCX contains a malformed XML attribute"))?;
        if attribute.key.local_name().as_ref() == name {
            return attribute
                .unescape_value()
                .map(|value| Some(value.into_owned()))
                .map_err(|_| {
                    output_invalid("normalized DOCX contains an invalid XML attribute value")
                });
        }
    }
    Ok(None)
}

fn relationship_target_is_absolute(target: &str) -> bool {
    let trimmed = target.trim();
    if trimmed.starts_with('/') || trimmed.starts_with('\\') || has_windows_drive_prefix(trimmed) {
        return true;
    }
    let bytes = trimmed.as_bytes();
    if let Some(colon) = bytes.iter().position(|byte| *byte == b':') {
        return colon > 0
            && bytes[..colon]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'));
    }
    false
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:X}", hasher.finalize())
}

fn output_invalid(message: impl Into<String>) -> HostError {
    error("BUSINESS_LEGACY_DOC_OUTPUT_INVALID", message, false)
}

fn cleanup_error(message: impl Into<String>) -> HostError {
    error("BUSINESS_LEGACY_DOC_CLEANUP_FAILED", message, true)
}

fn error(code: &'static str, message: impl Into<String>, retryable: bool) -> HostError {
    HostError::new(code, message, retryable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    const REGISTERED_SOURCE_SHA256: &str =
        "E1BF122AFDF3EF15017F3D82E9CAB5DA1C8D3BE38FEA40299906EE61538D5072";

    struct FakeRunner {
        outcome: Result<RunnerSuccess, RunnerError>,
        output: Option<Vec<u8>>,
        observed: Mutex<Vec<(PathBuf, PathBuf, PathBuf)>>,
    }

    impl FakeRunner {
        fn success(output: Vec<u8>) -> Self {
            Self {
                outcome: Ok(RunnerSuccess {
                    converter_version: "16.0".to_string(),
                }),
                output: Some(output),
                observed: Mutex::new(Vec::new()),
            }
        }

        fn failure(error: RunnerError, output: Option<Vec<u8>>) -> Self {
            Self {
                outcome: Err(error),
                output,
                observed: Mutex::new(Vec::new()),
            }
        }
    }

    impl LegacyDocRunner for FakeRunner {
        fn run(&self, request: &RunnerRequest<'_>) -> Result<RunnerSuccess, RunnerError> {
            self.observed.lock().unwrap().push((
                request.source.to_path_buf(),
                request.output.to_path_buf(),
                request.working_directory.to_path_buf(),
            ));
            assert_eq!(request.source.parent(), Some(request.working_directory));
            assert_eq!(request.source.extension(), Some(OsStr::new("doc")));
            assert!(request.source.is_file());
            assert!(!request.timeout.is_zero());
            if let Some(output) = &self.output {
                fs::write(request.output, output).unwrap();
            }
            self.outcome.clone()
        }
    }

    #[test]
    fn word_unavailable_returns_stable_error_and_cleans_working_files() {
        let fixture = Fixture::new();
        let runner = FakeRunner::failure(RunnerError::Unavailable, None);
        let error = fixture.normalize(&runner).unwrap_err();
        assert_eq!(error.code, "BUSINESS_WORD_AUTOMATION_UNAVAILABLE");
        fixture.assert_only_source_remains();
    }

    #[test]
    fn timeout_removes_partial_output_and_private_work_directory() {
        let fixture = Fixture::new();
        let runner = FakeRunner::failure(RunnerError::TimedOut, Some(b"partial".to_vec()));
        let error = fixture.normalize(&runner).unwrap_err();
        assert_eq!(error.code, "BUSINESS_LEGACY_DOC_CONVERSION_TIMEOUT");
        assert!(!fixture.output.exists());
        fixture.assert_only_source_remains();
    }

    #[test]
    fn conversion_error_removes_partial_output() {
        let fixture = Fixture::new();
        let runner = FakeRunner::failure(RunnerError::Failed, Some(b"partial".to_vec()));
        let error = fixture.normalize(&runner).unwrap_err();
        assert_eq!(error.code, "BUSINESS_LEGACY_DOC_CONVERSION_FAILED");
        fixture.assert_only_source_remains();
    }

    #[test]
    fn validates_registered_sha_before_invoking_runner() {
        let fixture = Fixture::new();
        let runner = FakeRunner::success(safe_docx_fixture(FixtureOptions::default()));
        let error = normalize_with_runner(
            &fixture.source,
            &"0".repeat(64),
            &fixture.output,
            Duration::from_secs(1),
            &runner,
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_LEGACY_DOC_SOURCE_SHA_MISMATCH");
        assert!(runner.observed.lock().unwrap().is_empty());
        fixture.assert_only_source_remains();
    }

    #[test]
    fn rejects_invalid_ole_source_before_invoking_runner() {
        let fixture = Fixture::with_source(b"not an OLE document".to_vec());
        let runner = FakeRunner::success(safe_docx_fixture(FixtureOptions::default()));
        let error = fixture.normalize(&runner).unwrap_err();
        assert_eq!(error.code, "BUSINESS_LEGACY_DOC_SOURCE_INVALID");
        assert!(runner.observed.lock().unwrap().is_empty());
    }

    #[test]
    fn successful_conversion_returns_versions_and_output_sha() {
        let fixture = Fixture::new();
        let output_bytes = safe_docx_fixture(FixtureOptions::default());
        let expected_output_sha = sha256_bytes(&output_bytes);
        let runner = FakeRunner::success(output_bytes.clone());
        let result = fixture.normalize(&runner).unwrap();
        assert_eq!(result.output_sha256, expected_output_sha);
        assert_eq!(result.output_size_bytes, output_bytes.len() as u64);
        assert_eq!(result.converter_engine, LEGACY_DOC_CONVERTER_ENGINE);
        assert_eq!(result.converter_version, "16.0");
        assert_eq!(
            result.converter_policy_version,
            LEGACY_DOC_CONVERTER_POLICY_VERSION
        );
        assert_eq!(fs::read(&fixture.output).unwrap(), output_bytes);
        assert_eq!(fs::read_dir(fixture.directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn rejects_active_or_external_docx_and_removes_output() {
        for options in [
            FixtureOptions {
                external_relationship: true,
                ..FixtureOptions::default()
            },
            FixtureOptions {
                forbidden_part: Some("word/vbaProject.bin"),
                ..FixtureOptions::default()
            },
            FixtureOptions {
                forbidden_part: Some("word/embeddings/oleObject1.bin"),
                ..FixtureOptions::default()
            },
            FixtureOptions {
                forbidden_part: Some("word/activeX/activeX1.xml"),
                ..FixtureOptions::default()
            },
            FixtureOptions {
                active_element: Some("w:altChunk"),
                ..FixtureOptions::default()
            },
        ] {
            let fixture = Fixture::new();
            let runner = FakeRunner::success(safe_docx_fixture(options));
            let error = fixture.normalize(&runner).unwrap_err();
            assert_eq!(error.code, "BUSINESS_LEGACY_DOC_OUTPUT_INVALID");
            fixture.assert_only_source_remains();
        }
    }

    #[test]
    fn powershell_command_is_fixed_and_paths_are_not_encoded_into_it() {
        let encoded = encode_powershell_script(WORD_CONVERSION_SCRIPT);
        let bytes = BASE64_STANDARD.decode(encoded).unwrap();
        let utf16 = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        let decoded = String::from_utf16(&utf16).unwrap();
        assert_eq!(decoded, WORD_CONVERSION_SCRIPT);
        assert!(decoded.contains("AutomationSecurity = 3"));
        assert!(decoded.contains("Documents.Open($source, $false, $true, $false)"));
        assert!(decoded.contains("SaveAs2($output, 16)"));
        assert!(decoded.contains("BSAIGC_LEGACY_DOC_SOURCE"));
        assert!(decoded.contains("BSAIGC_LEGACY_DOC_WORD_PIDS"));
        assert!(decoded.contains("BSAIGC_LEGACY_DOC_WORD_BASELINE"));
        assert!(decoded.contains("Get-Process -Name WINWORD"));
        assert!(decoded.contains("WriteAllLines"));
        assert!(decoded.contains("Stop-Process -Id $ownedWordPid"));
        assert!(!decoded.contains("C:\\"));
    }

    #[test]
    #[ignore = "requires Microsoft Word and the read-only registered payment template"]
    fn real_payment_template_normalizes_and_preserves_package_structure() {
        let source = std::env::var_os("BSAIGC_PAYMENT_LEGACY_DOC_TEMPLATE")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(
                    "tests/fixtures/synthetic/business-v1/templates/synthetic-payment-application.doc",
                )
            });
        assert!(
            source.is_file(),
            "registered payment DOC template is missing"
        );
        let original = fs::read(&source).unwrap();
        assert_eq!(sha256_bytes(&original), REGISTERED_SOURCE_SHA256);
        let directory = TempDir::new().unwrap();
        let output = directory.path().join("normalized-payment-template.docx");
        let result = normalize_legacy_doc(&source, REGISTERED_SOURCE_SHA256, &output).unwrap();
        assert_eq!(result.source_sha256, REGISTERED_SOURCE_SHA256);
        assert_eq!(fs::read(&source).unwrap(), original);
        let bytes = fs::read(&output).unwrap();
        validate_safe_docx(&bytes).unwrap();
        let structure = inspect_document_structure(&bytes);
        assert_eq!(structure.tables, 2);
        assert!(structure.rows >= 18);
        assert!(structure.has_explicit_page_break);
    }

    struct Fixture {
        directory: TempDir,
        source: PathBuf,
        output: PathBuf,
        source_sha256: String,
    }

    impl Fixture {
        fn new() -> Self {
            let mut source = OLE_COMPOUND_FILE_MAGIC.to_vec();
            source.extend_from_slice(b"registered legacy DOC fixture");
            Self::with_source(source)
        }

        fn with_source(source_bytes: Vec<u8>) -> Self {
            let directory = TempDir::new().unwrap();
            let source = directory.path().join("source.doc");
            let output = directory.path().join("output.docx");
            fs::write(&source, &source_bytes).unwrap();
            Self {
                directory,
                source,
                output,
                source_sha256: sha256_bytes(&source_bytes),
            }
        }

        fn normalize(
            &self,
            runner: &dyn LegacyDocRunner,
        ) -> Result<LegacyDocNormalizationResult, HostError> {
            normalize_with_runner(
                &self.source,
                &self.source_sha256,
                &self.output,
                Duration::from_secs(1),
                runner,
            )
        }

        fn assert_only_source_remains(&self) {
            assert_eq!(fs::read_dir(self.directory.path()).unwrap().count(), 1);
            assert!(self.source.is_file());
        }
    }

    #[derive(Debug, Clone, Copy, Default)]
    struct FixtureOptions {
        external_relationship: bool,
        forbidden_part: Option<&'static str>,
        active_element: Option<&'static str>,
    }

    fn safe_docx_fixture(options: FixtureOptions) -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut output);
            let file_options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(0o600);
            writer.start_file(CONTENT_TYPES_PATH, file_options).unwrap();
            writer.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#).unwrap();
            writer
                .start_file(ROOT_RELATIONSHIPS_PATH, file_options)
                .unwrap();
            writer.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#).unwrap();
            writer.start_file(DOCUMENT_PATH, file_options).unwrap();
            let active_element = options
                .active_element
                .map(|element| format!("<{element} r:id=\"rId9\"/>"))
                .unwrap_or_default();
            writer.write_all(format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p/><w:tbl><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/></w:tbl><w:br w:type="page"/><w:tbl><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/></w:tbl>{active_element}<w:sectPr/></w:body></w:document>"#).as_bytes()).unwrap();
            if options.external_relationship {
                writer
                    .start_file("word/_rels/document.xml.rels", file_options)
                    .unwrap();
                writer.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.invalid" TargetMode="External"/></Relationships>"#).unwrap();
            }
            if let Some(part) = options.forbidden_part {
                writer.start_file(part, file_options).unwrap();
                writer.write_all(b"active-content").unwrap();
            }
            writer.finish().unwrap();
        }
        output.into_inner()
    }

    #[derive(Default)]
    struct DocumentStructure {
        tables: usize,
        rows: usize,
        has_explicit_page_break: bool,
    }

    fn inspect_document_structure(docx: &[u8]) -> DocumentStructure {
        let mut archive = ZipArchive::new(Cursor::new(docx)).unwrap();
        let mut document = Vec::new();
        archive
            .by_name(DOCUMENT_PATH)
            .unwrap()
            .read_to_end(&mut document)
            .unwrap();
        let mut structure = DocumentStructure::default();
        let mut reader = xml_reader(&document);
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer).unwrap() {
                Event::Start(start) | Event::Empty(start) => match start.local_name().as_ref() {
                    b"tbl" => structure.tables += 1,
                    b"tr" => structure.rows += 1,
                    b"br" => {
                        structure.has_explicit_page_break |=
                            attribute(&start, b"type").unwrap().as_deref() == Some("page");
                    }
                    _ => {}
                },
                Event::Eof => break,
                _ => {}
            }
            buffer.clear();
        }
        structure
    }
}
