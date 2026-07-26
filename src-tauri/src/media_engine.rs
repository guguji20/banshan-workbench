use crate::protocol::HostError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(3);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(20);
const MAX_CAPTURE_BYTES: usize = 8 * 1024 * 1024;
const MIN_IMAGE_EDGE: u32 = 16;
const MAX_IMAGE_EDGE: u32 = 8_192;
const SAFE_SAMPLE_RATES: &[u32] = &[16_000, 22_050, 24_000, 32_000, 44_100, 48_000, 96_000];
const SAFE_AAC_BITRATES_KBPS: &[u32] = &[64, 96, 128, 160, 192, 256, 320];
const MAX_SEEK_MILLIS: u64 = 24 * 60 * 60 * 1_000;
const TEMP_NAME_ATTEMPTS: u64 = 128;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MediaToolSource {
    EnvironmentOverride,
    BundledRuntime,
    SystemPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MediaEngineHealth {
    pub ffmpeg_available: bool,
    pub ffprobe_available: bool,
    pub ffmpeg_source: Option<MediaToolSource>,
    pub ffprobe_source: Option<MediaToolSource>,
}

#[derive(Clone)]
pub(crate) struct MediaEngine {
    ffmpeg: Option<ResolvedTool>,
    ffprobe: Option<ResolvedTool>,
}

impl fmt::Debug for MediaEngine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MediaEngine")
            .field("health", &self.health())
            .finish()
    }
}

#[derive(Clone)]
struct ResolvedTool {
    path: PathBuf,
    source: MediaToolSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MediaError {
    pub code: &'static str,
    pub message: &'static str,
    pub retryable: bool,
}

impl MediaError {
    fn new(code: &'static str, message: &'static str, retryable: bool) -> Self {
        Self {
            code,
            message,
            retryable,
        }
    }

    fn ffmpeg_unavailable() -> Self {
        Self::new(
            "MEDIA_FFMPEG_UNAVAILABLE",
            "FFmpeg is unavailable; install or restore the media runtime and retry",
            true,
        )
    }

    fn ffprobe_unavailable() -> Self {
        Self::new(
            "MEDIA_FFPROBE_UNAVAILABLE",
            "ffprobe is unavailable; install or restore the media runtime and retry",
            true,
        )
    }

    fn canceled() -> Self {
        Self::new(
            "MEDIA_OPERATION_CANCELED",
            "media operation was canceled",
            false,
        )
    }

    fn deadline_exceeded() -> Self {
        Self::new(
            "MEDIA_DEADLINE_EXCEEDED",
            "media operation deadline elapsed",
            true,
        )
    }

    fn invalid_request(message: &'static str) -> Self {
        Self::new("MEDIA_INVALID_REQUEST", message, false)
    }
}

impl fmt::Display for MediaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for MediaError {}

impl From<MediaError> for HostError {
    fn from(error: MediaError) -> Self {
        HostError::new(error.code, error.message, error.retryable)
    }
}

#[derive(Clone, Default)]
pub(crate) struct CancellationToken {
    canceled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Release);
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }
}

impl fmt::Debug for CancellationToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MediaCancellationToken")
            .field("canceled", &self.is_canceled())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MediaExecutionControl {
    pub cancellation: CancellationToken,
    pub deadline: Option<Instant>,
}

impl Default for MediaExecutionControl {
    fn default() -> Self {
        Self::unlimited()
    }
}

impl MediaExecutionControl {
    pub fn unlimited() -> Self {
        Self {
            cancellation: CancellationToken::new(),
            deadline: None,
        }
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            cancellation: CancellationToken::new(),
            deadline: Instant::now().checked_add(timeout),
        }
    }

    pub fn until(deadline: Instant, cancellation: CancellationToken) -> Self {
        Self {
            cancellation,
            deadline: Some(deadline),
        }
    }

    fn checkpoint(&self) -> Result<(), MediaError> {
        if self.cancellation.is_canceled() {
            return Err(MediaError::canceled());
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(MediaError::deadline_exceeded());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MediaKind {
    Image,
    Video,
    Audio,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MediaStreamKind {
    Video,
    Audio,
    Subtitle,
    Data,
    Attachment,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaStreamProbe {
    pub index: u32,
    pub kind: MediaStreamKind,
    pub codec_name: Option<String>,
    pub duration_ms: Option<u64>,
    pub bit_rate_bps: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pixel_format: Option<String>,
    pub frame_rate_millihertz: Option<u64>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
    pub channel_layout: Option<String>,
    pub attached_picture: bool,
    pub rotation: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaProbe {
    pub kind: MediaKind,
    pub duration_ms: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub codec: Option<String>,
    pub container: Option<String>,
    pub rotation: Option<i32>,
    // These safe details remain backend-useful while paths, URLs, tags and raw
    // ffprobe fields are intentionally omitted.
    pub format_names: Vec<String>,
    pub size_bytes: Option<u64>,
    pub bit_rate_bps: Option<u64>,
    pub streams: Vec<MediaStreamProbe>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ThumbnailFormat {
    Jpeg,
    Png,
    Webp,
}

impl ThumbnailFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }

    fn mime_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
        }
    }

    fn encoder(self) -> &'static str {
        match self {
            Self::Jpeg => "mjpeg",
            Self::Png => "png",
            Self::Webp => "libwebp",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThumbnailOptions {
    pub max_width: u32,
    pub max_height: u32,
    pub seek_ms: u64,
    pub quality: u8,
    pub format: ThumbnailFormat,
}

impl Default for ThumbnailOptions {
    fn default() -> Self {
        Self {
            max_width: 1_280,
            max_height: 1_280,
            seek_ms: 0,
            quality: 85,
            format: ThumbnailFormat::Jpeg,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThumbnailResult {
    pub mime_type: String,
    pub size_bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AudioOutputFormat {
    Wav,
    Aac,
}

impl AudioOutputFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Aac => "aac",
        }
    }

    fn mime_type(self) -> &'static str {
        match self {
            Self::Wav => "audio/wav",
            Self::Aac => "audio/aac",
        }
    }

    fn encoder(self) -> &'static str {
        match self {
            Self::Wav => "pcm_s16le",
            Self::Aac => "aac",
        }
    }

    fn muxer(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Aac => "adts",
        }
    }

    fn is_lossy(self) -> bool {
        matches!(self, Self::Aac)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AudioExtractionOptions {
    pub format: AudioOutputFormat,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
    pub bit_rate_kbps: Option<u32>,
}

impl Default for AudioExtractionOptions {
    fn default() -> Self {
        Self {
            format: AudioOutputFormat::Wav,
            sample_rate_hz: Some(48_000),
            channels: Some(2),
            bit_rate_kbps: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AudioExtractionResult {
    pub mime_type: String,
    pub size_bytes: u64,
    pub duration_ms: Option<u64>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
}

impl MediaEngine {
    /// Discovers validated native tools without exposing their filesystem paths.
    /// Environment overrides are checked first, then side-by-side bundled files,
    /// then PATH. Missing tools remain a recoverable runtime capability state.
    pub fn discover() -> Self {
        Self {
            ffmpeg: discover_tool(MediaToolKind::Ffmpeg),
            ffprobe: discover_tool(MediaToolKind::Ffprobe),
        }
    }

    pub fn health(&self) -> MediaEngineHealth {
        MediaEngineHealth {
            ffmpeg_available: self.ffmpeg.is_some(),
            ffprobe_available: self.ffprobe.is_some(),
            ffmpeg_source: self.ffmpeg.as_ref().map(|tool| tool.source),
            ffprobe_source: self.ffprobe.as_ref().map(|tool| tool.source),
        }
    }

    pub fn probe(
        &self,
        source: &Path,
        control: &MediaExecutionControl,
    ) -> Result<MediaProbe, HostError> {
        self.probe_internal(source, control).map_err(Into::into)
    }

    pub fn generate_thumbnail(
        &self,
        source: &Path,
        destination: &Path,
        options: &ThumbnailOptions,
        control: &MediaExecutionControl,
    ) -> Result<ThumbnailResult, HostError> {
        control.checkpoint()?;
        validate_source(source)?;
        validate_destination(destination)?;
        validate_thumbnail_options(options)?;
        let ffmpeg = self
            .ffmpeg
            .as_ref()
            .ok_or_else(MediaError::ffmpeg_unavailable)?;
        let temporary = TemporaryOutput::allocate(destination, options.format.extension())?;

        let arguments = build_thumbnail_arguments(source, temporary.path(), options);

        let output = run_process(
            MediaToolKind::Ffmpeg,
            &ffmpeg.path,
            &arguments,
            false,
            control,
        )?;
        if !output.success {
            return Err(MediaError::new(
                "MEDIA_THUMBNAIL_FAILED",
                "FFmpeg could not generate a thumbnail from this asset",
                false,
            )
            .into());
        }
        control.checkpoint()?;
        let size_bytes = validate_generated_output(temporary.path())?;

        let (width, height) = self
            .probe_if_available(temporary.path(), control)?
            .map(|probe| (probe.width, probe.height))
            .unwrap_or((None, None));

        temporary.commit(destination)?;
        Ok(ThumbnailResult {
            mime_type: options.format.mime_type().to_string(),
            size_bytes,
            width,
            height,
        })
    }

    pub fn extract_audio(
        &self,
        source: &Path,
        destination: &Path,
        options: &AudioExtractionOptions,
        control: &MediaExecutionControl,
    ) -> Result<AudioExtractionResult, HostError> {
        control.checkpoint()?;
        validate_source(source)?;
        validate_destination(destination)?;
        validate_audio_options(options)?;
        let ffmpeg = self
            .ffmpeg
            .as_ref()
            .ok_or_else(MediaError::ffmpeg_unavailable)?;
        let temporary = TemporaryOutput::allocate(destination, options.format.extension())?;

        let arguments = build_audio_arguments(source, temporary.path(), options);

        let output = run_process(
            MediaToolKind::Ffmpeg,
            &ffmpeg.path,
            &arguments,
            false,
            control,
        )?;
        if !output.success {
            return Err(MediaError::new(
                "MEDIA_AUDIO_EXTRACTION_FAILED",
                "FFmpeg could not extract an audio track from this asset",
                false,
            )
            .into());
        }
        control.checkpoint()?;
        let size_bytes = validate_generated_output(temporary.path())?;

        let media_probe = self.probe_if_available(temporary.path(), control)?;
        let duration_ms = media_probe.as_ref().and_then(|probe| probe.duration_ms);
        let sample_rate_hz = media_probe
            .as_ref()
            .and_then(|probe| probe.sample_rate)
            .or(options.sample_rate_hz);
        let channels = media_probe
            .as_ref()
            .and_then(|probe| probe.channels)
            .or(options.channels);

        temporary.commit(destination)?;
        Ok(AudioExtractionResult {
            mime_type: options.format.mime_type().to_string(),
            size_bytes,
            duration_ms,
            sample_rate_hz,
            channels,
        })
    }

    fn probe_if_available(
        &self,
        source: &Path,
        control: &MediaExecutionControl,
    ) -> Result<Option<MediaProbe>, MediaError> {
        if self.ffprobe.is_none() {
            return Ok(None);
        }
        self.probe_internal(source, control).map(Some)
    }

    fn probe_internal(
        &self,
        source: &Path,
        control: &MediaExecutionControl,
    ) -> Result<MediaProbe, MediaError> {
        control.checkpoint()?;
        validate_source(source)?;
        let ffprobe = self
            .ffprobe
            .as_ref()
            .ok_or_else(MediaError::ffprobe_unavailable)?;
        let mut arguments = os_arguments(&[
            "-hide_banner",
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ]);
        arguments.push(source.as_os_str().to_owned());
        let output = run_process(
            MediaToolKind::Ffprobe,
            &ffprobe.path,
            &arguments,
            true,
            control,
        )?;
        if !output.success {
            return Err(MediaError::new(
                "MEDIA_PROBE_FAILED",
                "ffprobe could not inspect this asset",
                false,
            ));
        }
        parse_probe_output(&output.stdout)
    }

    #[cfg(test)]
    fn unavailable_for_test() -> Self {
        Self {
            ffmpeg: None,
            ffprobe: None,
        }
    }
}

impl Default for MediaEngine {
    fn default() -> Self {
        Self::discover()
    }
}

#[derive(Debug, Clone, Copy)]
enum MediaToolKind {
    Ffmpeg,
    Ffprobe,
}

impl MediaToolKind {
    fn executable_name(self) -> &'static str {
        match self {
            Self::Ffmpeg => "ffmpeg",
            Self::Ffprobe => "ffprobe",
        }
    }

    fn override_variable(self) -> &'static str {
        match self {
            Self::Ffmpeg => "BSAIGC_FFMPEG_BIN",
            Self::Ffprobe => "BSAIGC_FFPROBE_BIN",
        }
    }

    fn unavailable_error(self) -> MediaError {
        match self {
            Self::Ffmpeg => MediaError::ffmpeg_unavailable(),
            Self::Ffprobe => MediaError::ffprobe_unavailable(),
        }
    }
}

fn discover_tool(kind: MediaToolKind) -> Option<ResolvedTool> {
    discovery_candidates(kind)
        .into_iter()
        .find(|candidate| validate_tool_candidate(kind, &candidate.path))
}

fn discovery_candidates(kind: MediaToolKind) -> Vec<ResolvedTool> {
    let mut candidates = Vec::new();
    if let Some(path) = nonempty_env_path(kind.override_variable()) {
        push_candidate(&mut candidates, path, MediaToolSource::EnvironmentOverride);
    }
    if let Some(directory) = nonempty_env_path("BSAIGC_MEDIA_BIN_DIR") {
        push_tool_in_directory(
            &mut candidates,
            &directory,
            kind,
            MediaToolSource::EnvironmentOverride,
        );
    }

    let development_runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("media-runtime");
    push_tool_in_directory(
        &mut candidates,
        &development_runtime,
        kind,
        MediaToolSource::BundledRuntime,
    );

    if let Ok(current_executable) = env::current_exe() {
        if let Some(executable_directory) = current_executable.parent() {
            for directory in [
                executable_directory.to_path_buf(),
                executable_directory.join("media-runtime"),
                executable_directory.join("resources").join("media-runtime"),
                executable_directory
                    .join("resources")
                    .join("ffmpeg")
                    .join("bin"),
            ] {
                push_tool_in_directory(
                    &mut candidates,
                    &directory,
                    kind,
                    MediaToolSource::BundledRuntime,
                );
            }
        }
    }

    if let Some(path_value) = env::var_os("PATH") {
        for directory in env::split_paths(&path_value) {
            push_tool_in_directory(
                &mut candidates,
                &directory,
                kind,
                MediaToolSource::SystemPath,
            );
        }
    }
    candidates
}

fn nonempty_env_path(variable: &str) -> Option<PathBuf> {
    env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn push_tool_in_directory(
    candidates: &mut Vec<ResolvedTool>,
    directory: &Path,
    kind: MediaToolKind,
    source: MediaToolSource,
) {
    #[cfg(windows)]
    let names = [
        format!("{}.exe", kind.executable_name()),
        kind.executable_name().to_string(),
    ];
    #[cfg(not(windows))]
    let names = [kind.executable_name().to_string()];

    for name in names {
        push_candidate(candidates, directory.join(name), source);
    }
}

fn push_candidate(candidates: &mut Vec<ResolvedTool>, path: PathBuf, source: MediaToolSource) {
    if !path.is_file() {
        return;
    }
    let normalized = fs::canonicalize(&path).unwrap_or(path);
    if candidates
        .iter()
        .any(|candidate| candidate.path == normalized)
    {
        return;
    }
    candidates.push(ResolvedTool {
        path: normalized,
        source,
    });
}

fn validate_tool_candidate(kind: MediaToolKind, path: &Path) -> bool {
    let control = MediaExecutionControl::with_timeout(DISCOVERY_TIMEOUT);
    let arguments = os_arguments(&["-version"]);
    let Ok(output) = run_process(kind, path, &arguments, true, &control) else {
        return false;
    };
    if !output.success {
        return false;
    }
    let banner = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    banner.contains(&format!("{} version", kind.executable_name()))
}

struct ProcessOutput {
    success: bool,
    stdout: Vec<u8>,
}

#[derive(Default)]
struct OutputCaptureState {
    limit_exceeded: AtomicBool,
    read_failed: AtomicBool,
}

fn run_process(
    kind: MediaToolKind,
    executable: &Path,
    arguments: &[OsString],
    capture_stdout: bool,
    control: &MediaExecutionControl,
) -> Result<ProcessOutput, MediaError> {
    run_process_with_limit(
        kind,
        executable,
        arguments,
        capture_stdout,
        control,
        MAX_CAPTURE_BYTES,
    )
}

fn run_process_with_limit(
    kind: MediaToolKind,
    executable: &Path,
    arguments: &[OsString],
    capture_stdout: bool,
    control: &MediaExecutionControl,
    capture_limit_bytes: usize,
) -> Result<ProcessOutput, MediaError> {
    control.checkpoint()?;
    if capture_stdout && capture_limit_bytes == 0 {
        return Err(MediaError::invalid_request(
            "media process output limit must be positive",
        ));
    }
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(if capture_stdout {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    configure_background_process(&mut command);
    let mut child = command.spawn().map_err(|_| kind.unavailable_error())?;
    let capture_state = Arc::new(OutputCaptureState::default());
    let stdout_reader = child.stdout.take().map(|mut stdout| {
        let capture_state = Arc::clone(&capture_state);
        thread::spawn(move || {
            let mut captured = Vec::new();
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                match stdout.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => {
                        let remaining = capture_limit_bytes.saturating_sub(captured.len());
                        captured.extend_from_slice(&buffer[..read.min(remaining)]);
                        if read > remaining {
                            capture_state.limit_exceeded.store(true, Ordering::Release);
                        }
                    }
                    Err(_) => {
                        capture_state.read_failed.store(true, Ordering::Release);
                        break;
                    }
                }
            }
            captured
        })
    });

    loop {
        if control.cancellation.is_canceled() {
            terminate_process(&mut child);
            join_stdout_reader(stdout_reader)?;
            return Err(MediaError::canceled());
        }
        if control
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            terminate_process(&mut child);
            join_stdout_reader(stdout_reader)?;
            return Err(MediaError::deadline_exceeded());
        }
        if capture_state.limit_exceeded.load(Ordering::Acquire) {
            terminate_process(&mut child);
            join_stdout_reader(stdout_reader)?;
            return Err(MediaError::new(
                "MEDIA_PROCESS_OUTPUT_LIMIT_EXCEEDED",
                "native media process exceeded its output limit",
                false,
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = join_stdout_reader(stdout_reader)?;
                if capture_state.read_failed.load(Ordering::Acquire) {
                    return Err(MediaError::new(
                        "MEDIA_PROCESS_IO_FAILED",
                        "native media process output could not be read",
                        true,
                    ));
                }
                if capture_state.limit_exceeded.load(Ordering::Acquire) {
                    return Err(MediaError::new(
                        "MEDIA_PROCESS_OUTPUT_LIMIT_EXCEEDED",
                        "native media process exceeded its output limit",
                        false,
                    ));
                }
                return Ok(ProcessOutput {
                    success: status.success(),
                    stdout,
                });
            }
            Ok(None) => thread::sleep(PROCESS_POLL_INTERVAL),
            Err(_) => {
                terminate_process(&mut child);
                join_stdout_reader(stdout_reader)?;
                return Err(MediaError::new(
                    "MEDIA_PROCESS_IO_FAILED",
                    "native media process state could not be read",
                    true,
                ));
            }
        }
    }
}

fn join_stdout_reader(reader: Option<thread::JoinHandle<Vec<u8>>>) -> Result<Vec<u8>, MediaError> {
    match reader {
        Some(reader) => reader.join().map_err(|_| {
            MediaError::new(
                "MEDIA_PROCESS_IO_FAILED",
                "native media process output could not be read",
                true,
            )
        }),
        None => Ok(Vec::new()),
    }
}

fn terminate_process(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
fn configure_background_process(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_background_process(_command: &mut Command) {}

fn parse_probe_output(bytes: &[u8]) -> Result<MediaProbe, MediaError> {
    let root: Value = serde_json::from_slice(bytes).map_err(|_| {
        MediaError::new(
            "MEDIA_PROBE_OUTPUT_INVALID",
            "ffprobe returned an invalid structured response",
            false,
        )
    })?;
    let object = root.as_object().ok_or_else(|| {
        MediaError::new(
            "MEDIA_PROBE_OUTPUT_INVALID",
            "ffprobe returned an invalid structured response",
            false,
        )
    })?;

    let streams = object
        .get("streams")
        .and_then(Value::as_array)
        .map(|streams| {
            streams
                .iter()
                .filter_map(parse_stream_probe)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let format = object.get("format").and_then(Value::as_object);
    let format_names = format
        .and_then(|format| format.get("format_name"))
        .and_then(Value::as_str)
        .map(|names| {
            names
                .split(',')
                .filter_map(safe_identifier)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let format_duration_ms = format
        .and_then(|format| format.get("duration"))
        .and_then(parse_duration_ms);
    let duration_ms = streams
        .iter()
        .filter_map(|stream| stream.duration_ms)
        .fold(format_duration_ms, |maximum, duration| {
            Some(maximum.map_or(duration, |current| current.max(duration)))
        });
    let video = streams
        .iter()
        .find(|stream| stream.kind == MediaStreamKind::Video && !stream.attached_picture)
        .or_else(|| {
            let has_audio = streams
                .iter()
                .any(|stream| stream.kind == MediaStreamKind::Audio);
            (!has_audio).then(|| {
                streams
                    .iter()
                    .find(|stream| stream.kind == MediaStreamKind::Video)
            })?
        });
    let audio = streams
        .iter()
        .find(|stream| stream.kind == MediaStreamKind::Audio);
    let kind = match (video, audio) {
        (Some(_), None) if format_names.iter().any(|name| is_image_container(name)) => {
            MediaKind::Image
        }
        (Some(_), _) => MediaKind::Video,
        (None, Some(_)) => MediaKind::Audio,
        (None, None) => MediaKind::Unknown,
    };
    let codec = video
        .and_then(|stream| stream.codec_name.clone())
        .or_else(|| audio.and_then(|stream| stream.codec_name.clone()));

    Ok(MediaProbe {
        kind,
        duration_ms,
        width: video.and_then(|stream| stream.width),
        height: video.and_then(|stream| stream.height),
        fps: video
            .and_then(|stream| stream.frame_rate_millihertz)
            .map(|rate| rate as f64 / 1_000.0),
        sample_rate: audio.and_then(|stream| stream.sample_rate_hz),
        channels: audio.and_then(|stream| stream.channels),
        codec,
        container: format_names.first().cloned(),
        rotation: video.and_then(|stream| stream.rotation),
        format_names,
        size_bytes: format
            .and_then(|format| format.get("size"))
            .and_then(parse_u64),
        bit_rate_bps: format
            .and_then(|format| format.get("bit_rate"))
            .and_then(parse_u64),
        streams,
    })
}

fn parse_stream_probe(value: &Value) -> Option<MediaStreamProbe> {
    let stream = value.as_object()?;
    let index = stream.get("index").and_then(parse_u64)?;
    let index = u32::try_from(index).ok()?;
    let kind = match stream
        .get("codec_type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "video" => MediaStreamKind::Video,
        "audio" => MediaStreamKind::Audio,
        "subtitle" => MediaStreamKind::Subtitle,
        "data" => MediaStreamKind::Data,
        "attachment" => MediaStreamKind::Attachment,
        _ => MediaStreamKind::Other,
    };
    let attached_picture = stream
        .get("disposition")
        .and_then(Value::as_object)
        .and_then(|disposition| disposition.get("attached_pic"))
        .and_then(parse_u64)
        .is_some_and(|value| value == 1);

    Some(MediaStreamProbe {
        index,
        kind,
        codec_name: stream
            .get("codec_name")
            .and_then(Value::as_str)
            .and_then(safe_identifier),
        duration_ms: stream.get("duration").and_then(parse_duration_ms),
        bit_rate_bps: stream.get("bit_rate").and_then(parse_u64),
        width: stream
            .get("width")
            .and_then(parse_u64)
            .and_then(|value| u32::try_from(value).ok()),
        height: stream
            .get("height")
            .and_then(parse_u64)
            .and_then(|value| u32::try_from(value).ok()),
        pixel_format: stream
            .get("pix_fmt")
            .and_then(Value::as_str)
            .and_then(safe_identifier),
        frame_rate_millihertz: stream
            .get("avg_frame_rate")
            .and_then(Value::as_str)
            .and_then(parse_frame_rate_millihertz),
        sample_rate_hz: stream
            .get("sample_rate")
            .and_then(parse_u64)
            .and_then(|value| u32::try_from(value).ok()),
        channels: stream
            .get("channels")
            .and_then(parse_u64)
            .and_then(|value| u8::try_from(value).ok()),
        channel_layout: stream
            .get("channel_layout")
            .and_then(Value::as_str)
            .and_then(safe_label),
        attached_picture,
        rotation: parse_stream_rotation(stream),
    })
}

fn parse_stream_rotation(stream: &serde_json::Map<String, Value>) -> Option<i32> {
    stream
        .get("tags")
        .and_then(Value::as_object)
        .and_then(|tags| tags.get("rotate"))
        .and_then(parse_i32)
        .or_else(|| {
            stream
                .get("side_data_list")
                .and_then(Value::as_array)
                .and_then(|entries| {
                    entries.iter().find_map(|entry| {
                        entry
                            .as_object()
                            .and_then(|entry| entry.get("rotation"))
                            .and_then(parse_i32)
                    })
                })
        })
}

fn parse_i32(value: &Value) -> Option<i32> {
    value
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
        .or_else(|| value.as_str()?.trim().parse().ok())
        .or_else(|| {
            let value = value.as_f64()?;
            (value.is_finite() && value >= i32::MIN as f64 && value <= i32::MAX as f64)
                .then(|| value.round() as i32)
        })
}

fn is_image_container(name: &str) -> bool {
    matches!(
        name,
        "image2" | "image2pipe" | "jpeg_pipe" | "png_pipe" | "webp_pipe" | "bmp_pipe"
    )
}

fn parse_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.trim().parse().ok())
}

fn parse_duration_ms(value: &Value) -> Option<u64> {
    let seconds = value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())?;
    if !seconds.is_finite() || seconds.is_sign_negative() {
        return None;
    }
    let milliseconds = seconds * 1_000.0;
    (milliseconds <= u64::MAX as f64).then_some(milliseconds.round() as u64)
}

fn parse_frame_rate_millihertz(value: &str) -> Option<u64> {
    let (numerator, denominator) = value.trim().split_once('/')?;
    let numerator = numerator.parse::<u64>().ok()?;
    let denominator = denominator.parse::<u64>().ok()?;
    if denominator == 0 {
        return None;
    }
    numerator
        .checked_mul(1_000)?
        .checked_add(denominator / 2)?
        .checked_div(denominator)
}

fn safe_identifier(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 80
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
    {
        return None;
    }
    Some(value.to_string())
}

fn safe_label(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 80
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || " ._()-".contains(character))
    {
        return None;
    }
    Some(value.to_string())
}

fn validate_source(source: &Path) -> Result<(), MediaError> {
    let metadata = fs::metadata(source).map_err(|_| {
        MediaError::new(
            "MEDIA_SOURCE_UNAVAILABLE",
            "media source is unavailable",
            true,
        )
    })?;
    if !metadata.is_file() {
        return Err(MediaError::invalid_request(
            "media source must be a regular file",
        ));
    }
    Ok(())
}

fn validate_destination(destination: &Path) -> Result<(), MediaError> {
    if destination.as_os_str().is_empty() || destination.file_name().is_none() {
        return Err(MediaError::invalid_request(
            "media destination must name a file",
        ));
    }
    if destination.exists() {
        return Err(MediaError::new(
            "MEDIA_DESTINATION_EXISTS",
            "media destination already exists",
            false,
        ));
    }
    Ok(())
}

fn validate_thumbnail_options(options: &ThumbnailOptions) -> Result<(), MediaError> {
    if !(MIN_IMAGE_EDGE..=MAX_IMAGE_EDGE).contains(&options.max_width)
        || !(MIN_IMAGE_EDGE..=MAX_IMAGE_EDGE).contains(&options.max_height)
    {
        return Err(MediaError::invalid_request(
            "thumbnail dimensions must be between 16 and 8192 pixels",
        ));
    }
    if !(1..=100).contains(&options.quality) {
        return Err(MediaError::invalid_request(
            "thumbnail quality must be between 1 and 100",
        ));
    }
    if options.seek_ms > MAX_SEEK_MILLIS {
        return Err(MediaError::invalid_request(
            "thumbnail seek position must be within the first 24 hours",
        ));
    }
    Ok(())
}

fn validate_audio_options(options: &AudioExtractionOptions) -> Result<(), MediaError> {
    if options
        .sample_rate_hz
        .is_some_and(|sample_rate| !SAFE_SAMPLE_RATES.contains(&sample_rate))
    {
        return Err(MediaError::invalid_request("unsupported audio sample rate"));
    }
    if options
        .channels
        .is_some_and(|channels| !matches!(channels, 1 | 2))
    {
        return Err(MediaError::invalid_request(
            "audio channels must be mono or stereo",
        ));
    }
    if options
        .bit_rate_kbps
        .is_some_and(|bit_rate| !SAFE_AAC_BITRATES_KBPS.contains(&bit_rate))
    {
        return Err(MediaError::invalid_request("unsupported AAC bitrate"));
    }
    if !options.format.is_lossy() && options.bit_rate_kbps.is_some() {
        return Err(MediaError::invalid_request(
            "audio bitrate only applies to lossy output formats",
        ));
    }
    Ok(())
}

fn validate_generated_output(path: &Path) -> Result<u64, MediaError> {
    let metadata = fs::metadata(path).map_err(|_| {
        MediaError::new(
            "MEDIA_OUTPUT_INVALID",
            "native media output was not created",
            true,
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(MediaError::new(
            "MEDIA_OUTPUT_INVALID",
            "native media output is empty or invalid",
            true,
        ));
    }
    Ok(metadata.len())
}

fn build_thumbnail_arguments(
    source: &Path,
    destination: &Path,
    options: &ThumbnailOptions,
) -> Vec<OsString> {
    let filter = format!(
        "scale=w={}:h={}:force_original_aspect_ratio=decrease",
        options.max_width, options.max_height
    );
    let seek = milliseconds_as_ffmpeg_timestamp(options.seek_ms);
    let mut arguments =
        os_arguments(&["-hide_banner", "-loglevel", "error", "-nostdin", "-n", "-i"]);
    arguments.push(source.as_os_str().to_owned());
    arguments.extend(os_arguments(&[
        "-ss",
        &seek,
        "-map",
        "0:v:0",
        "-frames:v",
        "1",
        "-an",
        "-sn",
        "-map_metadata",
        "-1",
        "-vf",
        &filter,
        "-c:v",
        options.format.encoder(),
        "-f",
        "image2",
    ]));
    append_thumbnail_quality(&mut arguments, options);
    arguments.push(destination.as_os_str().to_owned());
    arguments
}

fn build_audio_arguments(
    source: &Path,
    destination: &Path,
    options: &AudioExtractionOptions,
) -> Vec<OsString> {
    let mut arguments =
        os_arguments(&["-hide_banner", "-loglevel", "error", "-nostdin", "-n", "-i"]);
    arguments.push(source.as_os_str().to_owned());
    arguments.extend(os_arguments(&[
        "-map",
        "0:a:0",
        "-vn",
        "-sn",
        "-dn",
        "-map_metadata",
        "-1",
        "-c:a",
        options.format.encoder(),
    ]));
    if let Some(sample_rate_hz) = options.sample_rate_hz {
        arguments.extend(os_arguments(&["-ar", &sample_rate_hz.to_string()]));
    }
    if let Some(channels) = options.channels {
        arguments.extend(os_arguments(&["-ac", &channels.to_string()]));
    }
    if options.format.is_lossy() {
        let bit_rate_kbps = options.bit_rate_kbps.unwrap_or(192);
        arguments.extend(os_arguments(&["-b:a", &format!("{bit_rate_kbps}k")]));
    }
    arguments.extend(os_arguments(&["-f", options.format.muxer()]));
    arguments.push(destination.as_os_str().to_owned());
    arguments
}

fn append_thumbnail_quality(arguments: &mut Vec<OsString>, options: &ThumbnailOptions) {
    match options.format {
        ThumbnailFormat::Jpeg => {
            // FFmpeg's JPEG scale is 2 (best) through 31 (worst).
            let quantizer = 31_u32.saturating_sub(u32::from(options.quality) * 29 / 100);
            arguments.extend(os_arguments(&["-q:v", &quantizer.clamp(2, 31).to_string()]));
        }
        ThumbnailFormat::Webp => {
            arguments.extend(os_arguments(&["-quality", &options.quality.to_string()]));
        }
        ThumbnailFormat::Png => {}
    }
}

fn milliseconds_as_ffmpeg_timestamp(milliseconds: u64) -> String {
    format!("{}.{:03}", milliseconds / 1_000, milliseconds % 1_000)
}

fn os_arguments(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

struct TemporaryOutput {
    path: PathBuf,
    committed: bool,
}

impl TemporaryOutput {
    fn allocate(destination: &Path, extension: &str) -> Result<Self, MediaError> {
        let parent = destination.parent().ok_or_else(|| {
            MediaError::invalid_request("media destination must have a parent directory")
        })?;
        fs::create_dir_all(parent).map_err(|_| {
            MediaError::new(
                "MEDIA_OUTPUT_PREPARE_FAILED",
                "media output directory could not be prepared",
                true,
            )
        })?;
        let process_id = std::process::id();
        let epoch_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..TEMP_NAME_ATTEMPTS {
            let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let name = format!(".bsaigc-media-{process_id}-{epoch_nanos}-{sequence}.{extension}");
            let path = parent.join(name);
            if !path.exists() {
                return Ok(Self {
                    path,
                    committed: false,
                });
            }
        }
        Err(MediaError::new(
            "MEDIA_OUTPUT_PREPARE_FAILED",
            "a temporary media output could not be allocated",
            true,
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn commit(mut self, destination: &Path) -> Result<(), MediaError> {
        if destination.exists() {
            return Err(MediaError::new(
                "MEDIA_DESTINATION_EXISTS",
                "media destination already exists",
                false,
            ));
        }
        fs::OpenOptions::new()
            .write(true)
            .open(&self.path)
            .and_then(|file| file.sync_all())
            .map_err(|_| {
                MediaError::new(
                    "MEDIA_OUTPUT_SYNC_FAILED",
                    "media output could not be synchronized",
                    true,
                )
            })?;
        fs::rename(&self.path, destination).map_err(|_| {
            MediaError::new(
                "MEDIA_OUTPUT_COMMIT_FAILED",
                "media output could not be committed atomically",
                true,
            )
        })?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new(name: &str) -> Self {
            let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "bsaigc-media-engine-test-{}-{name}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn source_file(directory: &TestDirectory) -> PathBuf {
        let path = directory.path.join("private-customer-source.mp4");
        File::create(&path)
            .unwrap()
            .write_all(b"not real media")
            .unwrap();
        path
    }

    #[test]
    fn parses_whitelisted_probe_fields_without_filename_or_tags() {
        let json = br#"
        {
          "streams": [
            {
              "index": 0,
              "codec_name": "h264",
              "codec_type": "video",
              "width": 3840,
              "height": 2160,
              "pix_fmt": "yuv420p",
              "avg_frame_rate": "30000/1001",
              "duration": "12.345",
              "bit_rate": "8000000",
              "disposition": {"attached_pic": 0},
              "tags": {"private_path": "C:\\secret\\customer.mp4"}
            },
            {
              "index": 1,
              "codec_name": "aac",
              "codec_type": "audio",
              "sample_rate": "48000",
              "channels": 2,
              "channel_layout": "stereo"
            }
          ],
          "format": {
            "filename": "C:\\secret\\customer.mp4",
            "format_name": "mov,mp4,m4a,3gp,3g2,mj2",
            "duration": "12.345",
            "size": "12345678",
            "bit_rate": "8000123",
            "tags": {"comment": "secret"}
          }
        }
        "#;
        let probe = parse_probe_output(json).unwrap();
        assert_eq!(probe.duration_ms, Some(12_345));
        assert_eq!(probe.size_bytes, Some(12_345_678));
        assert_eq!(probe.format_names[0], "mov");
        assert_eq!(probe.streams.len(), 2);
        assert_eq!(probe.streams[0].kind, MediaStreamKind::Video);
        assert_eq!(probe.streams[0].width, Some(3_840));
        assert_eq!(probe.streams[0].frame_rate_millihertz, Some(29_970));
        assert_eq!(probe.streams[1].kind, MediaStreamKind::Audio);
        assert_eq!(probe.streams[1].sample_rate_hz, Some(48_000));
        assert_eq!(probe.streams[1].channel_layout.as_deref(), Some("stereo"));

        let debug = format!("{probe:?}");
        assert!(!debug.contains("secret"));
        assert!(!debug.contains("customer.mp4"));
        assert!(!debug.contains("filename"));
        assert!(!debug.contains("tags"));
    }

    #[test]
    fn rejects_invalid_probe_json_with_static_safe_error() {
        let error = parse_probe_output(br#"{"streams": ["#).unwrap_err();
        assert_eq!(error.code, "MEDIA_PROBE_OUTPUT_INVALID");
        assert!(!error.retryable);
        assert!(!error.to_string().contains('{'));
    }

    #[test]
    fn unavailable_tools_report_explicit_recoverable_errors_without_paths() {
        let directory = TestDirectory::new("unavailable");
        let source = source_file(&directory);
        let destination = directory.path.join("preview.jpg");
        let engine = MediaEngine::unavailable_for_test();
        let control = MediaExecutionControl::unlimited();

        let probe_error = engine.probe(&source, &control).unwrap_err();
        assert_eq!(probe_error.code, "MEDIA_FFPROBE_UNAVAILABLE");
        assert!(probe_error.retryable);
        let thumbnail_error = engine
            .generate_thumbnail(
                &source,
                &destination,
                &ThumbnailOptions::default(),
                &control,
            )
            .unwrap_err();
        assert_eq!(thumbnail_error.code, "MEDIA_FFMPEG_UNAVAILABLE");
        assert!(thumbnail_error.retryable);
        for error in [probe_error, thumbnail_error] {
            assert!(!error.message.contains("private-customer-source"));
            assert!(!error.message.contains("preview.jpg"));
            assert!(!error.message.contains("-i"));
        }

        for format in [ThumbnailFormat::Png, ThumbnailFormat::Webp] {
            let options = ThumbnailOptions {
                format,
                ..ThumbnailOptions::default()
            };
            let error = engine
                .generate_thumbnail(&source, &destination, &options, &control)
                .unwrap_err();
            assert_eq!(error.code, "MEDIA_FFMPEG_UNAVAILABLE");
        }

        for format in [AudioOutputFormat::Wav, AudioOutputFormat::Aac] {
            let options = AudioExtractionOptions {
                format,
                bit_rate_kbps: format.is_lossy().then_some(128),
                ..AudioExtractionOptions::default()
            };
            let error = engine
                .extract_audio(
                    &source,
                    &directory.path.join(format!("audio.{}", format.extension())),
                    &options,
                    &control,
                )
                .unwrap_err();
            assert_eq!(error.code, "MEDIA_FFMPEG_UNAVAILABLE");
        }

        let result_shape = AudioExtractionResult {
            mime_type: AudioOutputFormat::Wav.mime_type().to_string(),
            size_bytes: 1,
            duration_ms: Some(1),
            sample_rate_hz: Some(48_000),
            channels: Some(2),
        };
        assert_eq!(result_shape.mime_type, "audio/wav");
    }

    #[test]
    fn cancellation_and_expired_deadline_win_before_tool_discovery() {
        let directory = TestDirectory::new("control");
        let source = source_file(&directory);
        let engine = MediaEngine::unavailable_for_test();

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let canceled =
            MediaExecutionControl::until(Instant::now() + Duration::from_secs(1), cancellation);
        let error = engine.probe(&source, &canceled).unwrap_err();
        assert_eq!(error.code, "MEDIA_OPERATION_CANCELED");

        let expired = MediaExecutionControl::until(
            Instant::now() - Duration::from_millis(1),
            CancellationToken::new(),
        );
        let error = engine.probe(&source, &expired).unwrap_err();
        assert_eq!(error.code, "MEDIA_DEADLINE_EXCEEDED");
        assert!(error.retryable);
    }

    #[test]
    fn cooperative_cancellation_stops_a_running_native_process() {
        let (executable, arguments) = sleeping_process();
        let cancellation = CancellationToken::new();
        let control = MediaExecutionControl::until(
            Instant::now() + Duration::from_secs(10),
            cancellation.clone(),
        );
        let cancel_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            cancellation.cancel();
        });
        let started = Instant::now();
        let error = match run_process(
            MediaToolKind::Ffmpeg,
            &executable,
            &arguments,
            false,
            &control,
        ) {
            Err(error) => error,
            Ok(_) => panic!("sleeping process was not canceled"),
        };
        cancel_thread.join().unwrap();
        assert_eq!(error.code, "MEDIA_OPERATION_CANCELED");
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn deadline_stops_a_running_native_process() {
        let (executable, arguments) = sleeping_process();
        let control = MediaExecutionControl::with_timeout(Duration::from_millis(150));
        let started = Instant::now();
        let error = match run_process(
            MediaToolKind::Ffprobe,
            &executable,
            &arguments,
            false,
            &control,
        ) {
            Err(error) => error,
            Ok(_) => panic!("sleeping process outlived its deadline"),
        };
        assert_eq!(error.code, "MEDIA_DEADLINE_EXCEEDED");
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn temporary_output_commits_with_rename_and_cleans_up_on_drop() {
        let directory = TestDirectory::new("atomic-output");
        let destination = directory.path.join("result.jpg");
        let temporary_path = {
            let temporary = TemporaryOutput::allocate(&destination, "jpg").unwrap();
            let path = temporary.path().to_path_buf();
            fs::write(&path, b"generated image").unwrap();
            assert_eq!(
                path.extension().and_then(|value| value.to_str()),
                Some("jpg")
            );
            temporary.commit(&destination).unwrap();
            path
        };
        assert_eq!(fs::read(&destination).unwrap(), b"generated image");
        assert!(!temporary_path.exists());

        let abandoned =
            TemporaryOutput::allocate(&directory.path.join("abandoned.wav"), "wav").unwrap();
        let abandoned_path = abandoned.path().to_path_buf();
        fs::write(&abandoned_path, b"partial").unwrap();
        drop(abandoned);
        assert!(!abandoned_path.exists());
    }

    #[test]
    fn existing_destination_is_never_replaced() {
        let directory = TestDirectory::new("no-replace");
        let destination = directory.path.join("existing.flac");
        fs::write(&destination, b"authoritative").unwrap();
        let error = validate_destination(&destination).unwrap_err();
        assert_eq!(error.code, "MEDIA_DESTINATION_EXISTS");
        assert_eq!(fs::read(&destination).unwrap(), b"authoritative");
    }

    #[test]
    fn media_options_enforce_native_safety_bounds() {
        let invalid_thumbnail = ThumbnailOptions {
            max_width: 15,
            ..ThumbnailOptions::default()
        };
        assert_eq!(
            validate_thumbnail_options(&invalid_thumbnail)
                .unwrap_err()
                .code,
            "MEDIA_INVALID_REQUEST"
        );

        let invalid_seek = ThumbnailOptions {
            seek_ms: MAX_SEEK_MILLIS + 1,
            ..ThumbnailOptions::default()
        };
        assert_eq!(
            validate_thumbnail_options(&invalid_seek).unwrap_err().code,
            "MEDIA_INVALID_REQUEST"
        );

        let invalid_audio = AudioExtractionOptions {
            format: AudioOutputFormat::Wav,
            bit_rate_kbps: Some(128),
            ..AudioExtractionOptions::default()
        };
        assert_eq!(
            validate_audio_options(&invalid_audio).unwrap_err().code,
            "MEDIA_INVALID_REQUEST"
        );
    }

    #[test]
    fn timestamps_and_frame_rates_are_deterministic() {
        assert_eq!(milliseconds_as_ffmpeg_timestamp(0), "0.000");
        assert_eq!(milliseconds_as_ffmpeg_timestamp(61_234), "61.234");
        assert_eq!(parse_frame_rate_millihertz("24000/1001"), Some(23_976));
        assert_eq!(parse_frame_rate_millihertz("1/0"), None);
    }

    #[test]
    fn discovered_engine_debug_output_never_contains_tool_paths() {
        let engine = MediaEngine::discover();
        let debug = format!("{engine:?}");
        assert!(debug.contains("ffmpeg_available"));
        assert!(!debug.contains(".exe"));
        assert!(!debug.contains("\\"));
        assert!(!debug.contains('/'));
    }

    #[cfg(windows)]
    fn sleeping_process() -> (PathBuf, Vec<OsString>) {
        let executable =
            PathBuf::from(env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe");
        let arguments = os_arguments(&[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Seconds 10",
        ]);
        (executable, arguments)
    }

    #[cfg(not(windows))]
    fn sleeping_process() -> (PathBuf, Vec<OsString>) {
        (PathBuf::from("/bin/sh"), os_arguments(&["-c", "sleep 10"]))
    }
}
