use crate::asset_service::{get_asset, import_file, resolve_original_path};
use crate::media_engine::{
    AudioExtractionOptions, AudioExtractionResult, AudioOutputFormat,
    CancellationToken as MediaCancellationToken, MediaEngine, MediaError, MediaExecutionControl,
    MediaProbe, ThumbnailFormat, ThumbnailOptions, ThumbnailResult,
};
use crate::protocol::{AssetRecord, HostError};
use crate::task_runner::{
    HandlerContext, TaskHandler, TaskHandlerError, TaskHandlerResult, TaskRunner,
};
use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(crate) const MEDIA_PROBE_TASK_KIND: &str = "media.probe";
pub(crate) const MEDIA_THUMBNAIL_TASK_KIND: &str = "media.thumbnail";
pub(crate) const MEDIA_EXTRACT_AUDIO_TASK_KIND: &str = "media.extractAudio";

const MIN_TIMEOUT_MS: u64 = 100;
const DEFAULT_PROBE_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_TRANSFORM_TIMEOUT_MS: u64 = 5 * 60_000;
const MAX_TIMEOUT_MS: u64 = 30 * 60_000;
const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(20);
const MEDIA_STAGING_DIRECTORY: &str = ".media-staging";

impl From<MediaError> for TaskHandlerError {
    fn from(error: MediaError) -> Self {
        let host_error: HostError = error.into();
        host_error.into()
    }
}

#[derive(Clone)]
pub(crate) struct MediaTaskServices {
    media: Arc<MediaEngine>,
    asset_connection: Arc<Mutex<Connection>>,
    vault_root: Arc<PathBuf>,
}

impl MediaTaskServices {
    pub fn new(
        media: Arc<MediaEngine>,
        asset_connection: Arc<Mutex<Connection>>,
        vault_root: PathBuf,
    ) -> Self {
        Self {
            media,
            asset_connection,
            vault_root: Arc::new(vault_root),
        }
    }
}

#[derive(Clone)]
pub(crate) struct MediaProbeTaskHandler {
    services: MediaTaskServices,
}

impl MediaProbeTaskHandler {
    pub fn new(services: MediaTaskServices) -> Self {
        Self { services }
    }
}

impl TaskHandler for MediaProbeTaskHandler {
    fn execute(&self, context: HandlerContext) -> TaskHandlerResult {
        let input: ProbeTaskInput = decode_input(context.input.clone(), MEDIA_PROBE_TASK_KIND)?;
        let _options = input.options;
        let timeout = task_timeout(input.timeout_ms, DEFAULT_PROBE_TIMEOUT_MS)?;
        ensure_active(&context)?;

        let source = resolve_source(
            &self.services,
            context.project.as_deref(),
            input.asset_id.as_str(),
        )?;
        context.progress.report(10)?;

        let (control, _cancellation_bridge) = execution_control(&context, timeout)?;
        let metadata = self.services.media.probe(&source.path, &control)?;
        ensure_active(&context)?;
        context.progress.report(95)?;

        serialize_output(MediaProbeTaskOutput {
            asset_id: source.asset.id,
            metadata,
        })
    }
}

#[derive(Clone)]
pub(crate) struct MediaThumbnailTaskHandler {
    services: MediaTaskServices,
}

impl MediaThumbnailTaskHandler {
    pub fn new(services: MediaTaskServices) -> Self {
        Self { services }
    }
}

impl TaskHandler for MediaThumbnailTaskHandler {
    fn execute(&self, context: HandlerContext) -> TaskHandlerResult {
        let input: ThumbnailTaskInput =
            decode_input(context.input.clone(), MEDIA_THUMBNAIL_TASK_KIND)?;
        let timeout = task_timeout(input.timeout_ms, DEFAULT_TRANSFORM_TIMEOUT_MS)?;
        let options: ThumbnailOptions = input.options.into();
        ensure_active(&context)?;

        let source = resolve_source(
            &self.services,
            context.project.as_deref(),
            input.asset_id.as_str(),
        )?;
        context.progress.report(5)?;

        let staged = StagedMediaFile::allocate(
            self.services.vault_root.as_ref(),
            "thumbnail",
            thumbnail_extension(options.format),
        )?;
        let (control, _cancellation_bridge) = execution_control(&context, timeout)?;
        let metadata = self.services.media.generate_thumbnail(
            &source.path,
            staged.path(),
            &options,
            &control,
        )?;
        ensure_active(&context)?;
        context.progress.report(85)?;

        let asset = import_derived_asset(&self.services, &source, staged.path())?;
        ensure_active(&context)?;
        context.progress.report(95)?;

        serialize_output(MediaThumbnailTaskOutput {
            asset_id: asset.id,
            metadata,
        })
    }
}

#[derive(Clone)]
pub(crate) struct MediaExtractAudioTaskHandler {
    services: MediaTaskServices,
}

impl MediaExtractAudioTaskHandler {
    pub fn new(services: MediaTaskServices) -> Self {
        Self { services }
    }
}

impl TaskHandler for MediaExtractAudioTaskHandler {
    fn execute(&self, context: HandlerContext) -> TaskHandlerResult {
        let input: ExtractAudioTaskInput =
            decode_input(context.input.clone(), MEDIA_EXTRACT_AUDIO_TASK_KIND)?;
        let timeout = task_timeout(input.timeout_ms, DEFAULT_TRANSFORM_TIMEOUT_MS)?;
        let options: AudioExtractionOptions = input.options.into();
        ensure_active(&context)?;

        let source = resolve_source(
            &self.services,
            context.project.as_deref(),
            input.asset_id.as_str(),
        )?;
        context.progress.report(5)?;

        let staged = StagedMediaFile::allocate(
            self.services.vault_root.as_ref(),
            "audio",
            audio_extension(options.format),
        )?;
        let (control, _cancellation_bridge) = execution_control(&context, timeout)?;
        let metadata =
            self.services
                .media
                .extract_audio(&source.path, staged.path(), &options, &control)?;
        ensure_active(&context)?;
        context.progress.report(85)?;

        let asset = import_derived_asset(&self.services, &source, staged.path())?;
        ensure_active(&context)?;
        context.progress.report(95)?;

        serialize_output(MediaExtractAudioTaskOutput {
            asset_id: asset.id,
            metadata,
        })
    }
}

/// Installs all native media handlers into the durable runner. Registration only grants handlers
/// backend capabilities; task input never receives a filesystem path or process argument list.
pub(crate) fn register_media_task_handlers(
    runner: &TaskRunner,
    services: MediaTaskServices,
) -> Result<(), HostError> {
    runner.register_handler(
        MEDIA_PROBE_TASK_KIND,
        MediaProbeTaskHandler::new(services.clone()),
    )?;
    runner.register_handler(
        MEDIA_THUMBNAIL_TASK_KIND,
        MediaThumbnailTaskHandler::new(services.clone()),
    )?;
    runner.register_handler(
        MEDIA_EXTRACT_AUDIO_TASK_KIND,
        MediaExtractAudioTaskHandler::new(services),
    )?;
    Ok(())
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProbeTaskOptions {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProbeTaskInput {
    asset_id: String,
    #[serde(default)]
    options: ProbeTaskOptions,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct ThumbnailTaskOptions {
    max_width: u32,
    max_height: u32,
    seek_ms: u64,
    quality: u8,
    format: ThumbnailFormat,
}

impl Default for ThumbnailTaskOptions {
    fn default() -> Self {
        let defaults = ThumbnailOptions::default();
        Self {
            max_width: defaults.max_width,
            max_height: defaults.max_height,
            seek_ms: defaults.seek_ms,
            quality: defaults.quality,
            format: defaults.format,
        }
    }
}

impl From<ThumbnailTaskOptions> for ThumbnailOptions {
    fn from(options: ThumbnailTaskOptions) -> Self {
        Self {
            max_width: options.max_width,
            max_height: options.max_height,
            seek_ms: options.seek_ms,
            quality: options.quality,
            format: options.format,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ThumbnailTaskInput {
    asset_id: String,
    #[serde(default)]
    options: ThumbnailTaskOptions,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct ExtractAudioTaskOptions {
    format: AudioOutputFormat,
    sample_rate_hz: Option<u32>,
    channels: Option<u8>,
    bit_rate_kbps: Option<u32>,
}

impl Default for ExtractAudioTaskOptions {
    fn default() -> Self {
        let defaults = AudioExtractionOptions::default();
        Self {
            format: defaults.format,
            sample_rate_hz: defaults.sample_rate_hz,
            channels: defaults.channels,
            bit_rate_kbps: defaults.bit_rate_kbps,
        }
    }
}

impl From<ExtractAudioTaskOptions> for AudioExtractionOptions {
    fn from(options: ExtractAudioTaskOptions) -> Self {
        Self {
            format: options.format,
            sample_rate_hz: options.sample_rate_hz,
            channels: options.channels,
            bit_rate_kbps: options.bit_rate_kbps,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExtractAudioTaskInput {
    asset_id: String,
    #[serde(default)]
    options: ExtractAudioTaskOptions,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaProbeTaskOutput {
    asset_id: String,
    metadata: MediaProbe,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaThumbnailTaskOutput {
    asset_id: String,
    metadata: ThumbnailResult,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaExtractAudioTaskOutput {
    asset_id: String,
    metadata: AudioExtractionResult,
}

struct ResolvedSource {
    asset: AssetRecord,
    path: PathBuf,
}

fn resolve_source(
    services: &MediaTaskServices,
    task_project: Option<&str>,
    asset_id: &str,
) -> Result<ResolvedSource, TaskHandlerError> {
    let connection = lock_asset_connection(services)?;
    let asset = get_asset(&connection, asset_id)?;
    enforce_project_scope(task_project, asset.project_id.as_deref())?;
    let path = resolve_original_path(&connection, services.vault_root.as_ref(), &asset.id)?;
    Ok(ResolvedSource { asset, path })
}

fn enforce_project_scope(
    task_project: Option<&str>,
    asset_project: Option<&str>,
) -> Result<(), TaskHandlerError> {
    if task_project != asset_project {
        return Err(TaskHandlerError::structured(
            "MEDIA_ASSET_PROJECT_MISMATCH",
            "task project does not own the requested asset",
            false,
        ));
    }
    Ok(())
}

fn import_derived_asset(
    services: &MediaTaskServices,
    source: &ResolvedSource,
    staged_path: &Path,
) -> Result<AssetRecord, TaskHandlerError> {
    let mut connection = lock_asset_connection(services)?;
    import_file(
        &mut connection,
        services.vault_root.as_ref(),
        source.asset.project_id.as_deref(),
        staged_path,
    )
    .map_err(Into::into)
}

fn lock_asset_connection(
    services: &MediaTaskServices,
) -> Result<MutexGuard<'_, Connection>, TaskHandlerError> {
    services.asset_connection.lock().map_err(|_| {
        TaskHandlerError::structured(
            "MEDIA_ASSET_STORE_UNAVAILABLE",
            "asset store is temporarily unavailable",
            true,
        )
    })
}

fn decode_input<T: DeserializeOwned>(
    input: Value,
    task_kind: &'static str,
) -> Result<T, TaskHandlerError> {
    serde_json::from_value(input).map_err(|_| {
        TaskHandlerError::structured(
            "MEDIA_TASK_INVALID_INPUT",
            format!("{task_kind} input must contain only assetId, options and timeoutMs"),
            false,
        )
    })
}

fn task_timeout(requested_ms: Option<u64>, default_ms: u64) -> Result<Duration, TaskHandlerError> {
    let timeout_ms = requested_ms.unwrap_or(default_ms);
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(TaskHandlerError::structured(
            "MEDIA_TIMEOUT_INVALID",
            "media timeoutMs must be between 100 and 1800000",
            false,
        ));
    }
    Ok(Duration::from_millis(timeout_ms))
}

fn ensure_active(context: &HandlerContext) -> Result<(), TaskHandlerError> {
    if context.cancel.is_cancelled() {
        return Err(TaskHandlerError::structured(
            "MEDIA_OPERATION_CANCELED",
            "media operation was canceled",
            false,
        ));
    }
    Ok(())
}

fn execution_control(
    context: &HandlerContext,
    timeout: Duration,
) -> Result<(MediaExecutionControl, CancellationBridge), TaskHandlerError> {
    let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
        TaskHandlerError::structured(
            "MEDIA_TIMEOUT_INVALID",
            "media timeout cannot be represented by this host",
            false,
        )
    })?;
    let media_cancellation = MediaCancellationToken::new();
    if context.cancel.is_cancelled() {
        media_cancellation.cancel();
    }
    let bridge = CancellationBridge::spawn(context.cancel.clone(), media_cancellation.clone())?;
    Ok((
        MediaExecutionControl::until(deadline, media_cancellation),
        bridge,
    ))
}

struct CancellationBridge {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl CancellationBridge {
    fn spawn(
        task_cancellation: crate::task_runner::CancellationToken,
        media_cancellation: MediaCancellationToken,
    ) -> Result<Self, TaskHandlerError> {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("bsaigc-media-cancel".to_string())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) {
                    if task_cancellation.wait_cancelled(CANCELLATION_POLL_INTERVAL) {
                        media_cancellation.cancel();
                        break;
                    }
                }
            })
            .map_err(|_| {
                TaskHandlerError::structured(
                    "MEDIA_CANCELLATION_BRIDGE_UNAVAILABLE",
                    "could not start media cancellation monitor",
                    true,
                )
            })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}

impl Drop for CancellationBridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct StagedMediaFile {
    path: PathBuf,
}

impl StagedMediaFile {
    fn allocate(
        vault_root: &Path,
        label: &'static str,
        extension: &'static str,
    ) -> Result<Self, TaskHandlerError> {
        fs::create_dir_all(vault_root).map_err(|_| staging_error())?;
        let resolved_vault = fs::canonicalize(vault_root).map_err(|_| staging_error())?;
        let staging_root = resolved_vault.join(MEDIA_STAGING_DIRECTORY);
        fs::create_dir_all(&staging_root).map_err(|_| staging_error())?;
        let resolved_staging = fs::canonicalize(&staging_root).map_err(|_| staging_error())?;
        if !resolved_staging.starts_with(&resolved_vault) {
            return Err(TaskHandlerError::structured(
                "MEDIA_STAGING_OUTSIDE_VAULT",
                "media staging directory escaped the Vault",
                false,
            ));
        }
        let path = resolved_staging.join(format!("{label}-{}.{}", Uuid::new_v4(), extension));
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagedMediaFile {
    fn drop(&mut self) {
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => eprintln!("media staging cleanup failed"),
        }
    }
}

fn staging_error() -> TaskHandlerError {
    TaskHandlerError::structured(
        "MEDIA_STAGING_UNAVAILABLE",
        "Vault media staging is temporarily unavailable",
        true,
    )
}

fn thumbnail_extension(format: ThumbnailFormat) -> &'static str {
    match format {
        ThumbnailFormat::Jpeg => "jpg",
        ThumbnailFormat::Png => "png",
        ThumbnailFormat::Webp => "webp",
    }
}

fn audio_extension(format: AudioOutputFormat) -> &'static str {
    match format {
        AudioOutputFormat::Wav => "wav",
        AudioOutputFormat::Aac => "aac",
    }
}

fn serialize_output<T: Serialize>(output: T) -> TaskHandlerResult {
    serde_json::to_value(output).map_err(|_| {
        TaskHandlerError::structured(
            "MEDIA_OUTPUT_SERIALIZATION_FAILED",
            "media task output could not be serialized",
            true,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_service;
    use crate::media_engine::{MediaKind, MediaStreamKind, MediaStreamProbe};
    use crate::protocol::{CreateTaskPayload, TaskPriority, TaskReplayPolicy, TaskStatus};
    use crate::task_engine::TaskEngine;
    use rusqlite::Connection;
    use serde_json::json;
    use tempfile::tempdir;

    fn services(vault_root: &Path) -> MediaTaskServices {
        let connection = Connection::open_in_memory().unwrap();
        asset_service::migrate(&connection).unwrap();
        MediaTaskServices::new(
            Arc::new(MediaEngine::discover()),
            Arc::new(Mutex::new(connection)),
            vault_root.to_path_buf(),
        )
    }

    #[test]
    fn inputs_reject_paths_raw_arguments_and_unknown_options() {
        let source_path = json!({
            "assetId": Uuid::new_v4().to_string(),
            "sourcePath": "C:\\secret\\source.mp4",
            "options": {},
            "timeoutMs": 1_000
        });
        let raw_arguments = json!({
            "assetId": Uuid::new_v4().to_string(),
            "options": { "rawArgs": ["-i", "secret.mp4"] },
            "timeoutMs": 1_000
        });
        let destination = json!({
            "assetId": Uuid::new_v4().to_string(),
            "options": { "destination": "C:\\secret\\thumb.jpg" },
            "timeoutMs": 1_000
        });

        assert_eq!(
            decode_input::<ProbeTaskInput>(source_path, MEDIA_PROBE_TASK_KIND)
                .unwrap_err()
                .code(),
            "MEDIA_TASK_INVALID_INPUT"
        );
        assert_eq!(
            decode_input::<ThumbnailTaskInput>(raw_arguments, MEDIA_THUMBNAIL_TASK_KIND)
                .unwrap_err()
                .code(),
            "MEDIA_TASK_INVALID_INPUT"
        );
        assert_eq!(
            decode_input::<ExtractAudioTaskInput>(destination, MEDIA_EXTRACT_AUDIO_TASK_KIND)
                .unwrap_err()
                .code(),
            "MEDIA_TASK_INVALID_INPUT"
        );
    }

    #[test]
    fn structured_options_apply_defaults_and_timeout_bounds() {
        let input: ThumbnailTaskInput = decode_input(
            json!({
                "assetId": Uuid::new_v4().to_string(),
                "options": { "maxWidth": 640, "format": "png" },
                "timeoutMs": 15_000
            }),
            MEDIA_THUMBNAIL_TASK_KIND,
        )
        .unwrap();
        let options: ThumbnailOptions = input.options.into();
        assert_eq!(options.max_width, 640);
        assert_eq!(options.max_height, 1_280);
        assert_eq!(options.format, ThumbnailFormat::Png);
        assert_eq!(
            task_timeout(input.timeout_ms, 1_000).unwrap(),
            Duration::from_secs(15)
        );

        assert_eq!(
            task_timeout(Some(99), 1_000).unwrap_err().code(),
            "MEDIA_TIMEOUT_INVALID"
        );
        assert_eq!(
            task_timeout(Some(MAX_TIMEOUT_MS + 1), 1_000)
                .unwrap_err()
                .code(),
            "MEDIA_TIMEOUT_INVALID"
        );
    }

    #[test]
    fn staging_is_inside_vault_and_is_cleaned_on_drop() {
        let temporary = tempdir().unwrap();
        let staged = StagedMediaFile::allocate(temporary.path(), "thumbnail", "jpg").unwrap();
        let path = staged.path().to_path_buf();
        assert!(path.starts_with(temporary.path().canonicalize().unwrap()));
        fs::write(&path, b"generated").unwrap();
        assert!(path.exists());
        drop(staged);
        assert!(!path.exists());
    }

    #[test]
    fn derived_asset_keeps_original_project_and_staging_is_removed() {
        let temporary = tempdir().unwrap();
        let services = services(temporary.path());
        let source_path = temporary.path().join("source.mp4");
        fs::write(&source_path, b"source-media").unwrap();
        let source_asset = {
            let mut connection = services.asset_connection.lock().unwrap();
            import_file(
                &mut connection,
                services.vault_root.as_ref(),
                Some("project-alpha"),
                &source_path,
            )
            .unwrap()
        };
        let source = resolve_source(&services, Some("project-alpha"), &source_asset.id).unwrap();
        let staged = StagedMediaFile::allocate(temporary.path(), "thumbnail", "jpg").unwrap();
        fs::write(staged.path(), [0xff, 0xd8, 0xff, 0xd9]).unwrap();

        let derived = import_derived_asset(&services, &source, staged.path()).unwrap();
        assert_eq!(derived.project_id.as_deref(), Some("project-alpha"));
        let staged_path = staged.path().to_path_buf();
        drop(staged);
        assert!(!staged_path.exists());
        assert!(resolve_original_path(
            &services.asset_connection.lock().unwrap(),
            services.vault_root.as_ref(),
            &derived.id
        )
        .unwrap()
        .is_file());
    }

    #[test]
    fn project_scope_rejects_cross_project_asset_access() {
        assert_eq!(
            enforce_project_scope(Some("project-a"), Some("project-b"))
                .unwrap_err()
                .code(),
            "MEDIA_ASSET_PROJECT_MISMATCH"
        );
        assert_eq!(
            enforce_project_scope(None, Some("project-b"))
                .unwrap_err()
                .code(),
            "MEDIA_ASSET_PROJECT_MISMATCH"
        );
        assert!(enforce_project_scope(Some("project-a"), Some("project-a")).is_ok());
    }

    #[test]
    fn task_outputs_only_expose_asset_id_and_structured_metadata() {
        let output = serialize_output(MediaProbeTaskOutput {
            asset_id: "asset-stable".to_string(),
            metadata: MediaProbe {
                kind: MediaKind::Video,
                duration_ms: Some(1_000),
                width: Some(1_920),
                height: Some(1_080),
                fps: Some(25.0),
                sample_rate: None,
                channels: None,
                codec: Some("h264".to_string()),
                container: Some("mp4".to_string()),
                rotation: None,
                format_names: vec!["mp4".to_string()],
                size_bytes: Some(42),
                bit_rate_bps: Some(800_000),
                streams: vec![MediaStreamProbe {
                    index: 0,
                    kind: MediaStreamKind::Video,
                    codec_name: Some("h264".to_string()),
                    duration_ms: Some(1_000),
                    bit_rate_bps: Some(800_000),
                    width: Some(1_920),
                    height: Some(1_080),
                    pixel_format: Some("yuv420p".to_string()),
                    frame_rate_millihertz: Some(25_000),
                    sample_rate_hz: None,
                    channels: None,
                    channel_layout: None,
                    attached_picture: false,
                    rotation: None,
                }],
            },
        })
        .unwrap();

        assert_eq!(output["assetId"], "asset-stable");
        assert!(output.get("metadata").is_some());
        assert_eq!(output.as_object().unwrap().len(), 2);
        let serialized = output.to_string().to_ascii_lowercase();
        assert!(!serialized.contains("path"));
        assert!(!serialized.contains("rawargs"));
    }

    #[test]
    fn unavailable_native_tools_remain_retryable() {
        let ffmpeg = TaskHandlerError::from(HostError::new(
            "MEDIA_FFMPEG_UNAVAILABLE",
            "FFmpeg is unavailable; install or restore the media runtime and retry",
            true,
        ));
        let ffprobe = TaskHandlerError::from(HostError::new(
            "MEDIA_FFPROBE_UNAVAILABLE",
            "ffprobe is unavailable; install or restore the media runtime and retry",
            true,
        ));
        assert!(ffmpeg.retryable());
        assert!(ffprobe.retryable());
    }

    #[test]
    fn registration_installs_all_handlers_without_starting_workers() {
        let temporary = tempdir().unwrap();
        let task_engine =
            Arc::new(TaskEngine::from_connection(Connection::open_in_memory().unwrap()).unwrap());
        let runner = TaskRunner::new(task_engine, 2).unwrap();
        register_media_task_handlers(&runner, services(temporary.path())).unwrap();

        assert!(runner.unregister_handler(MEDIA_PROBE_TASK_KIND));
        assert!(runner.unregister_handler(MEDIA_THUMBNAIL_TASK_KIND));
        assert!(runner.unregister_handler(MEDIA_EXTRACT_AUDIO_TASK_KIND));
    }

    #[test]
    #[ignore = "requires the bootstrapped FFmpeg runtime"]
    fn real_runner_probes_a_vault_asset_with_bundled_ffprobe() {
        let temporary = tempdir().unwrap();
        let services = services(temporary.path());
        assert!(services.media.health().ffprobe_available);
        let source_path = temporary.path().join("smoke.wav");
        let sample_bytes = 1_600_u32;
        let mut wav = Vec::from(*b"RIFF");
        wav.extend_from_slice(&(36_u32 + sample_bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&16_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&sample_bytes.to_le_bytes());
        wav.resize(wav.len() + sample_bytes as usize, 0);
        fs::write(&source_path, wav).unwrap();
        let asset = {
            let mut connection = services.asset_connection.lock().unwrap();
            import_file(
                &mut connection,
                services.vault_root.as_ref(),
                Some("project-media-smoke"),
                &source_path,
            )
            .unwrap()
        };

        let task_engine =
            Arc::new(TaskEngine::from_connection(Connection::open_in_memory().unwrap()).unwrap());
        let runner = TaskRunner::new(Arc::clone(&task_engine), 2).unwrap();
        register_media_task_handlers(&runner, services).unwrap();
        let task = task_engine
            .create_task(CreateTaskPayload {
                kind: MEDIA_PROBE_TASK_KIND.to_string(),
                project_id: Some("project-media-smoke".to_string()),
                input: json!({ "assetId": asset.id.clone(), "options": {}, "timeoutMs": 10_000 }),
                priority: TaskPriority::Normal,
                replay_policy: TaskReplayPolicy::Safe,
                max_attempts: 1,
                dependency_task_ids: Vec::new(),
            })
            .unwrap();
        runner.start().unwrap();

        let deadline = Instant::now() + Duration::from_secs(15);
        let completed = loop {
            let current = task_engine.get_task(&task.id).unwrap();
            if current.status != TaskStatus::Queued && current.status != TaskStatus::Running {
                break current;
            }
            assert!(Instant::now() < deadline, "media probe task timed out");
            thread::sleep(Duration::from_millis(20));
        };
        runner.shutdown().unwrap();
        assert_eq!(
            completed.status,
            TaskStatus::Succeeded,
            "{}",
            completed.last_error.as_deref().unwrap_or("no task error")
        );
        assert_eq!(completed.output.unwrap()["assetId"], asset.id);
    }
}
