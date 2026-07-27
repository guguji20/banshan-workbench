// These service APIs intentionally include the next Native Media integration surface.
mod ai_credential_service;
#[allow(dead_code)]
mod asset_service;
mod asset_source_registry;
mod auth_service;
#[allow(dead_code)]
mod backup_outbox;
mod brain_host;
mod brain_store;
mod brain_workspace_registry;
mod business_closure_service;
mod business_skill_service;
mod business_tool_host_adapter;
mod business_tool_registry;
mod business_workspace_service;
mod case_library;
mod codex_host;
mod contract_review_agent;
#[allow(dead_code)]
mod contract_review_rules;
#[allow(dead_code)]
mod contract_review_runtime;
#[allow(dead_code)]
mod contract_review_service;
mod desktop_settings_service;
#[allow(dead_code)]
mod document_intelligence;
// Vendor wire variants keep their official names; server-response support lands with approvals.
#[allow(dead_code, clippy::enum_variant_names)]
mod codex_runtime;
#[path = "diagnostic.rs"]
// Upload acknowledgement and suppression are reserved for the future Sync host.
#[allow(dead_code)]
mod diagnostic_service;
mod document_engine;
mod execution_brief_service;
mod host;
mod media_engine;
mod media_tasks;
mod memory_service;
mod module_registry;
pub mod protocol;
mod r2_backup;
mod remembered_auth;
mod requirement_brief_service;
#[allow(dead_code)]
mod review_report;
mod security;
// Worker lifecycle APIs are consumed by the durable runner in the next integration slice.
#[allow(dead_code)]
mod task_engine;
#[allow(dead_code)]
mod task_runner;

use ai_credential_service::AiCredentialService;
use asset_source_registry::AssetSourceRegistry;
use backup_outbox::BackupOutbox;
use base64::Engine;
use brain_host::{BrainExecutionAttachment, BrainExecutionContext, BrainHost, BrainSubscription};
use brain_workspace_registry::BrainWorkspaceRegistry;
use business_tool_host_adapter::BusinessToolHostAdapter;
use business_tool_registry::BusinessToolRegistry;
use desktop_settings_service::DesktopSettingsService;
use diagnostic_service::DiagnosticOutbox;
use host::BackendHost;
use media_engine::{MediaEngine, MediaToolSource};
use media_tasks::{register_media_task_handlers, MediaTaskServices};
use protocol::{
    AiCredentialCommandEnvelope, AiCredentialCommandResponse, ApprovalRecord, AssetBackupRecord,
    AssetCommandEnvelope, AssetCommandResponse, AssetDomainEvent, AssetRecord,
    AssetSourceSelection, AuthChangePasswordPayload, AuthCreateUserPayload, AuthCredentials,
    AuthDeleteUserPayload, AuthResetPasswordPayload, AuthStatus, AuthUsersSnapshot,
    BackupCommandEnvelope, BackupCommandResponse, BackupDomainEvent, BrainAttachmentPreview,
    BrainDroppedItems, BrainHostHealth, BrainThreadRecord, BrainTurnContext, BrainTurnRecord,
    BrainTurnStartResult, BrainWorkspaceSelection, BusinessCustomerReceivableSummary,
    BusinessWorkspaceCommandEnvelope, BusinessWorkspaceCommandResponse,
    BusinessWorkspaceDomainEvent, BusinessWorkspacePrefillCandidate,
    BusinessWorkspacePrefillPreview, BusinessWorkspaceRecord, CaseCommandEnvelope,
    CaseCommandResponse, CaseDomainEvent, CaseRecord, CodexProbeStatus, CommandEnvelope,
    CommandResponse, ContractReviewCommandEnvelope, ContractReviewCommandResponse,
    ContractReviewDomainEvent, ContractReviewRecord, DesktopSettingsCommandEnvelope,
    DesktopSettingsCommandResponse, DiagnosticRecord, DiagnosticReportPayload, DomainEvent,
    EvidenceContext, ExecutionBriefCommandEnvelope, ExecutionBriefCommandResponse,
    ExecutionBriefDomainEvent, ExecutionBriefRecord, GetContractReviewRequest,
    GetEvidenceContextRequest, HostError, HostStatus, InterruptBrainTurnRequest,
    ListBusinessCustomersRequest, ListBusinessWorkspacePrefillCandidatesRequest,
    ListContractReviewsRequest, ListRemoteBrainThreadsRequest, ListReviewFindingsRequest,
    MemoryRecord, MemoryScope, ModuleAvailability, NativeMediaHealth, OperationContext,
    PreviewBusinessWorkspacePrefillRequest, ProjectRecord, QueueAssetBackupPayload,
    RemoteBrainThreadPage, ReplayEventsRequest, RequirementBriefCommandEnvelope,
    RequirementBriefCommandResponse, RequirementBriefDomainEvent, RequirementBriefRecord,
    ResolveApprovalPayload, ResumeBrainThreadRequest, ReviewFindingRecord,
    StageClipboardImageRequest, StartBrainThreadRequest, StartBrainTurnRequest,
    TaskCommandEnvelope, TaskCommandResponse, TaskDomainEvent, TaskRecord, ASSET_EVENT_CHANNEL,
    BACKUP_EVENT_CHANNEL, BACKUP_PROTOCOL_VERSION, BRAIN_EVENT_CHANNEL,
    BUSINESS_WORKSPACE_EVENT_CHANNEL, CASE_EVENT_CHANNEL, CONTRACT_REVIEW_EVENT_CHANNEL,
    DOMAIN_EVENT_CHANNEL, EXECUTION_BRIEF_EVENT_CHANNEL, REQUIREMENT_BRIEF_EVENT_CHANNEL,
    TASK_EVENT_CHANNEL,
};
use r2_backup::R2BackupWorker;
use rusqlite::Connection;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use task_engine::TaskEngine;
use task_runner::{TaskLifecycleEventSink, TaskRunner};
use tauri::{Emitter, Manager, State};
use uuid::Uuid;

struct AppState {
    auth: auth_service::AuthService,
    host: BackendHost,
    tasks: Arc<TaskEngine>,
    task_runner: TaskRunner,
    media_engine: Arc<MediaEngine>,
    asset_connection: Arc<Mutex<Connection>>,
    asset_sources: AssetSourceRegistry,
    brain_workspaces: BrainWorkspaceRegistry,
    vault_root: PathBuf,
    generated_staging_root: PathBuf,
    chat_attachment_staging_root: PathBuf,
    backup_outbox: Arc<BackupOutbox>,
    backup_worker: R2BackupWorker,
    diagnostics: DiagnosticOutbox,
    memory_connection: Mutex<Connection>,
    brain: BrainHost,
    ai_credentials: Arc<AiCredentialService>,
    desktop_settings: DesktopSettingsService,
    _brain_subscription: BrainSubscription,
}

impl Drop for AppState {
    fn drop(&mut self) {
        self.backup_worker.shutdown();
        self.brain.shutdown();
    }
}

#[tauri::command]
fn auth_status(state: State<'_, AppState>) -> Result<AuthStatus, HostError> {
    Ok(state.auth.status())
}

#[tauri::command]
fn auth_initialize_admin(
    state: State<'_, AppState>,
    credentials: AuthCredentials,
) -> Result<AuthStatus, HostError> {
    state.auth.initialize_admin(credentials)
}

#[tauri::command]
fn auth_login(
    state: State<'_, AppState>,
    credentials: AuthCredentials,
) -> Result<AuthStatus, HostError> {
    state.auth.login(credentials)
}

#[tauri::command]
fn auth_logout(state: State<'_, AppState>) -> Result<AuthStatus, HostError> {
    Ok(state.auth.logout())
}

#[tauri::command]
fn auth_remembered_credentials() -> Result<Option<AuthCredentials>, HostError> {
    remembered_auth::load()
}

#[tauri::command]
fn auth_remember_credentials(credentials: AuthCredentials) -> Result<(), HostError> {
    remembered_auth::save(credentials)
}

#[tauri::command]
fn auth_forget_credentials() -> Result<(), HostError> {
    remembered_auth::clear()
}

#[tauri::command]
fn auth_change_password(
    state: State<'_, AppState>,
    payload: AuthChangePasswordPayload,
) -> Result<AuthStatus, HostError> {
    state.auth.change_password(payload)
}

#[tauri::command]
fn auth_list_users(state: State<'_, AppState>) -> Result<AuthUsersSnapshot, HostError> {
    state.auth.list_users()
}

#[tauri::command]
fn auth_create_user(
    state: State<'_, AppState>,
    payload: AuthCreateUserPayload,
) -> Result<AuthUsersSnapshot, HostError> {
    state.auth.create_user(payload)
}

#[tauri::command]
fn auth_reset_password(
    state: State<'_, AppState>,
    payload: AuthResetPasswordPayload,
) -> Result<AuthUsersSnapshot, HostError> {
    state.auth.reset_password(payload)
}

#[tauri::command]
fn auth_delete_user(
    state: State<'_, AppState>,
    payload: AuthDeleteUserPayload,
) -> Result<AuthUsersSnapshot, HostError> {
    state.auth.delete_user(payload)
}

#[tauri::command]
fn auth_refresh_registry(state: State<'_, AppState>) -> Result<AuthStatus, HostError> {
    Ok(state.auth.refresh_registry())
}

#[tauri::command]
fn execute_command(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    command: CommandEnvelope,
) -> Result<CommandResponse, HostError> {
    let outcome = state.host.execute(command)?;
    for event in &outcome.emitted_events {
        // SQLite is authoritative. A WebView delivery failure must not turn an
        // already committed command into a business failure; replay_events heals it.
        if let Err(error) = app.emit(DOMAIN_EVENT_CHANNEL, event) {
            eprintln!(
                "domain event delivery failed after commit: event_id={} trace_id={} error={error}",
                event.event_id, event.trace_id
            );
        }
    }
    Ok(outcome.response)
}

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectRecord>, HostError> {
    state.host.list_projects()
}

#[tauri::command]
fn replay_events(
    state: State<'_, AppState>,
    request: ReplayEventsRequest,
) -> Result<Vec<DomainEvent>, HostError> {
    state
        .host
        .replay_events(request.after_sequence, request.limit)
}

#[tauri::command]
fn execute_task_command(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    command: TaskCommandEnvelope,
) -> Result<TaskCommandResponse, HostError> {
    let outcome = state.tasks.execute_command(command)?;
    for event in &outcome.emitted_events {
        state.task_runner.notify_event(event);
    }
    emit_after_commit(&app, TASK_EVENT_CHANNEL, &outcome.emitted_events);
    Ok(outcome.response)
}

#[tauri::command]
fn list_tasks(state: State<'_, AppState>) -> Result<Vec<TaskRecord>, HostError> {
    state.tasks.list()
}

#[tauri::command]
fn replay_task_events(
    state: State<'_, AppState>,
    request: ReplayEventsRequest,
) -> Result<Vec<TaskDomainEvent>, HostError> {
    state
        .tasks
        .replay_events(request.after_sequence, request.limit)
}

#[tauri::command]
async fn select_asset_source(
    state: State<'_, AppState>,
) -> Result<Option<AssetSourceSelection>, HostError> {
    let selected = tauri::async_runtime::spawn_blocking(|| rfd::FileDialog::new().pick_file())
        .await
        .map_err(|error| HostError::internal(format!("asset picker task failed: {error}")))?;
    selected
        .map(|path| state.asset_sources.issue(path))
        .transpose()
}

#[tauri::command]
async fn select_asset_sources(
    state: State<'_, AppState>,
) -> Result<Vec<AssetSourceSelection>, HostError> {
    let selected = tauri::async_runtime::spawn_blocking(|| rfd::FileDialog::new().pick_files())
        .await
        .map_err(|error| HostError::internal(format!("asset picker task failed: {error}")))?;
    selected
        .unwrap_or_default()
        .into_iter()
        .take(20)
        .map(|path| state.asset_sources.issue(path))
        .collect()
}

#[tauri::command]
async fn select_brain_workspace(
    state: State<'_, AppState>,
) -> Result<Option<BrainWorkspaceSelection>, HostError> {
    let selected = tauri::async_runtime::spawn_blocking(|| rfd::FileDialog::new().pick_folder())
        .await
        .map_err(|error| HostError::internal(format!("workspace picker task failed: {error}")))?;
    selected
        .map(|path| state.brain_workspaces.issue(path))
        .transpose()
}

#[tauri::command]
fn register_brain_dropped_paths(
    state: State<'_, AppState>,
    paths: Vec<PathBuf>,
) -> Result<BrainDroppedItems, HostError> {
    if paths.len() > 20 {
        return Err(HostError::validation("drop at most 20 items at a time"));
    }
    let mut files = Vec::new();
    let mut workspace = None;
    for path in paths {
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            HostError::new(
                "BRAIN_DROP_UNAVAILABLE",
                format!("dropped item is unavailable: {error}"),
                false,
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(HostError::new(
                "BRAIN_DROP_INVALID",
                "symbolic links cannot be dropped into Brain",
                false,
            ));
        }
        if metadata.is_file() {
            files.push(state.asset_sources.issue(path)?);
        } else if metadata.is_dir() && workspace.is_none() {
            workspace = Some(state.brain_workspaces.issue(path)?);
        }
    }
    Ok(BrainDroppedItems { files, workspace })
}

#[tauri::command]
fn stage_clipboard_image(
    state: State<'_, AppState>,
    request: StageClipboardImageRequest,
) -> Result<AssetSourceSelection, HostError> {
    const MAX_CLIPBOARD_IMAGE_BYTES: usize = 20 * 1024 * 1024;
    let extension = match request.mime_type.as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        _ => {
            return Err(HostError::validation(
                "clipboard item must be a supported image",
            ))
        }
    };
    if request.bytes.is_empty() || request.bytes.len() > MAX_CLIPBOARD_IMAGE_BYTES {
        return Err(HostError::validation(
            "clipboard image must be between 1 byte and 20 MiB",
        ));
    }
    fs::create_dir_all(&state.chat_attachment_staging_root).map_err(|error| {
        HostError::internal(format!(
            "create chat attachment staging directory failed: {error}"
        ))
    })?;
    let requested_stem = Path::new(&request.file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("clipboard-image")
        .chars()
        .filter(|character| character.is_alphanumeric() || matches!(character, '-' | '_'))
        .take(48)
        .collect::<String>();
    let stem = if requested_stem.is_empty() {
        "clipboard-image"
    } else {
        requested_stem.as_str()
    };
    let path =
        state
            .chat_attachment_staging_root
            .join(format!("{stem}-{}.{}", Uuid::new_v4(), extension));
    fs::write(&path, request.bytes)
        .map_err(|error| HostError::internal(format!("stage clipboard image failed: {error}")))?;
    match state.asset_sources.issue_temporary(path.clone()) {
        Ok(selection) => Ok(selection),
        Err(error) => {
            let _ = fs::remove_file(path);
            Err(error)
        }
    }
}

#[tauri::command]
fn get_brain_attachment_preview(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<Option<BrainAttachmentPreview>, HostError> {
    const MAX_PREVIEW_BYTES: u64 = 20 * 1024 * 1024;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("asset SQLite lock is poisoned"))?;
    let (asset, path) =
        asset_service::verify_ready_asset_integrity(&connection, &state.vault_root, &asset_id)?;
    if asset.kind != protocol::AssetKind::Image || asset.size_bytes as u64 > MAX_PREVIEW_BYTES {
        return Ok(None);
    }
    let bytes = fs::read(path)
        .map_err(|error| HostError::internal(format!("read attachment preview failed: {error}")))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(Some(BrainAttachmentPreview {
        mime_type: asset.mime_type.clone(),
        data_url: format!("data:{};base64,{encoded}", asset.mime_type),
    }))
}

#[tauri::command]
fn execute_asset_command(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    command: AssetCommandEnvelope,
) -> Result<AssetCommandResponse, HostError> {
    let (source_token, backup_context) = match &command {
        AssetCommandEnvelope::Import {
            context, payload, ..
        } => (payload.source_token.clone(), context.clone()),
    };
    let mut connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("asset SQLite lock is poisoned"))?;
    let mut temporary_source = None;
    let outcome = asset_service::execute_import_command_with_resolver(
        &mut connection,
        &state.vault_root,
        command,
        || {
            let consumed = state.asset_sources.consume(&source_token)?;
            if consumed.delete_after_consume {
                temporary_source = Some(consumed.path.clone());
            }
            Ok(consumed.path)
        },
    );
    if let Some(path) = temporary_source {
        let _ = fs::remove_file(path);
    }
    let outcome = outcome?;
    let backup_asset = outcome.response.asset.clone();
    drop(connection);
    emit_after_commit(&app, ASSET_EVENT_CHANNEL, &outcome.emitted_events);
    let (backup_events, backup_warnings) = queue_assets_for_backup(
        &state.backup_outbox,
        &backup_context,
        std::iter::once(&backup_asset),
    );
    emit_after_commit(&app, BACKUP_EVENT_CHANNEL, &backup_events);
    if !backup_events.is_empty() {
        state.backup_worker.wake();
    }
    for warning in backup_warnings {
        eprintln!("asset import local success with deferred backup warning: {warning}");
    }
    Ok(outcome.response)
}

#[tauri::command]
fn list_assets(state: State<'_, AppState>) -> Result<Vec<AssetRecord>, HostError> {
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("asset SQLite lock is poisoned"))?;
    asset_service::list_assets(&connection, None)
}

#[tauri::command]
fn replay_asset_events(
    state: State<'_, AppState>,
    request: ReplayEventsRequest,
) -> Result<Vec<AssetDomainEvent>, HostError> {
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("asset SQLite lock is poisoned"))?;
    asset_service::replay_asset_events(&connection, request.after_sequence, request.limit)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetActionCapabilities {
    asset_id: String,
    can_open: bool,
    can_export: bool,
    reason: Option<String>,
}

#[tauri::command]
fn get_asset_action_capabilities(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<AssetActionCapabilities, HostError> {
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("asset SQLite lock is poisoned"))?;
    let asset = asset_service::get_asset(&connection, &asset_id)?;
    if asset.status != protocol::AssetStatus::Ready {
        return Ok(AssetActionCapabilities {
            asset_id,
            can_open: false,
            can_export: false,
            reason: Some("资产尚未就绪".to_string()),
        });
    }
    match asset_service::resolve_original_path(&connection, &state.vault_root, &asset.id) {
        Ok(_) => Ok(AssetActionCapabilities {
            asset_id,
            can_open: true,
            can_export: true,
            reason: None,
        }),
        Err(error) => Ok(AssetActionCapabilities {
            asset_id,
            can_open: false,
            can_export: false,
            reason: Some(error.message),
        }),
    }
}

#[tauri::command]
async fn open_asset(state: State<'_, AppState>, asset_id: String) -> Result<(), HostError> {
    let connection = Arc::clone(&state.asset_connection);
    let vault_root = state.vault_root.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let connection = connection
            .lock()
            .map_err(|_| HostError::internal("asset SQLite lock is poisoned"))?;
        let (_, path) =
            asset_service::verify_ready_asset_integrity(&connection, &vault_root, &asset_id)?;
        drop(connection);

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer.exe")
                .arg(&path)
                .spawn()
                .map_err(|error| {
                    HostError::new(
                        "ASSET_OPEN_FAILED",
                        format!("open asset with Windows failed: {error}"),
                        true,
                    )
                })?;
            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = path;
            Err(HostError::new(
                "ASSET_OPEN_UNSUPPORTED",
                "native asset opening is currently supported only by DesktopHostAdapter on Windows",
                false,
            ))
        }
    })
    .await
    .map_err(|error| HostError::internal(format!("asset open task failed: {error}")))?
}

#[tauri::command]
async fn export_asset(state: State<'_, AppState>, asset_id: String) -> Result<bool, HostError> {
    let connection = Arc::clone(&state.asset_connection);
    let vault_root = state.vault_root.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let original_name = {
            let connection = connection
                .lock()
                .map_err(|_| HostError::internal("asset SQLite lock is poisoned"))?;
            let asset = asset_service::get_asset(&connection, &asset_id)?;
            if asset.status != protocol::AssetStatus::Ready {
                return Err(HostError::new(
                    "ASSET_NOT_READY",
                    "asset is not ready for export",
                    true,
                ));
            }
            asset.original_name
        };
        let Some(destination) = rfd::FileDialog::new()
            .set_file_name(&original_name)
            .save_file()
        else {
            return Ok(false);
        };
        let connection = connection
            .lock()
            .map_err(|_| HostError::internal("asset SQLite lock is poisoned"))?;
        business_workspace_service::verify_archive_package_for_export(
            &connection,
            &vault_root,
            &asset_id,
        )?;
        asset_service::export_verified_asset_to_path(
            &connection,
            &vault_root,
            &asset_id,
            &destination,
        )?;
        Ok(true)
    })
    .await
    .map_err(|error| HostError::internal(format!("asset export task failed: {error}")))?
}

#[tauri::command]
fn execute_case_command(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    command: CaseCommandEnvelope,
) -> Result<CaseCommandResponse, HostError> {
    let mut connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("asset/case SQLite lock is poisoned"))?;
    let outcome = case_library::execute_command(&mut connection, command)?;
    emit_after_commit(&app, CASE_EVENT_CHANNEL, &outcome.emitted_events);
    Ok(outcome.response)
}

#[tauri::command]
fn list_cases(state: State<'_, AppState>) -> Result<Vec<CaseRecord>, HostError> {
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("asset/case SQLite lock is poisoned"))?;
    case_library::list(&connection)
}

#[tauri::command]
fn replay_case_events(
    state: State<'_, AppState>,
    request: ReplayEventsRequest,
) -> Result<Vec<CaseDomainEvent>, HostError> {
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("asset/case SQLite lock is poisoned"))?;
    case_library::replay_events(&connection, request.after_sequence, request.limit)
}

#[tauri::command]
fn execute_requirement_brief_command(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    command: RequirementBriefCommandEnvelope,
) -> Result<RequirementBriefCommandResponse, HostError> {
    authorize_requirement_brief_command(&state.host, &command)?;
    let mut connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    let outcome = requirement_brief_service::execute_command(&mut connection, command)?;
    emit_after_commit(
        &app,
        REQUIREMENT_BRIEF_EVENT_CHANNEL,
        &outcome.emitted_events,
    );
    Ok(outcome.response)
}

#[tauri::command]
fn list_requirement_briefs(
    state: State<'_, AppState>,
) -> Result<Vec<RequirementBriefRecord>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "requirementBrief.read",
        "requirementBrief",
        None,
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    requirement_brief_service::list(&connection)
}

#[tauri::command]
fn replay_requirement_brief_events(
    state: State<'_, AppState>,
    request: ReplayEventsRequest,
) -> Result<Vec<RequirementBriefDomainEvent>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "requirementBrief.read",
        "requirementBrief",
        None,
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    requirement_brief_service::replay_events(&connection, request.after_sequence, request.limit)
}

fn authorize_requirement_brief_command(
    host: &BackendHost,
    command: &RequirementBriefCommandEnvelope,
) -> Result<(), HostError> {
    let (actor_id, operation, resource_id) = match command {
        RequirementBriefCommandEnvelope::Create {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "requirementBrief.write",
            payload.project_id.as_str(),
        ),
        RequirementBriefCommandEnvelope::Update {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "requirementBrief.write",
            payload.brief_id.as_str(),
        ),
        RequirementBriefCommandEnvelope::ChangeStatus {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "requirementBrief.confirm",
            payload.brief_id.as_str(),
        ),
    };
    ensure_allowed(host.authorize_operation(
        actor_id,
        operation,
        "requirementBrief",
        Some(resource_id),
        security::OperationEffect::ReversibleWrite,
        None,
    )?)
}

#[tauri::command]
fn execute_business_workspace_command(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    command: BusinessWorkspaceCommandEnvelope,
) -> Result<BusinessWorkspaceCommandResponse, HostError> {
    authorize_business_workspace_command(&state.host, &command)?;
    let backup_context = business_workspace_command_context(&command).clone();
    let mut connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    let outcome =
        business_workspace_service::execute_command(&mut connection, &state.vault_root, command)?;
    let backup_assets = outcome
        .emitted_asset_events
        .iter()
        .map(|event| event.asset.clone())
        .collect::<Vec<_>>();
    drop(connection);
    emit_after_commit(&app, ASSET_EVENT_CHANNEL, &outcome.emitted_asset_events);
    emit_after_commit(
        &app,
        BUSINESS_WORKSPACE_EVENT_CHANNEL,
        &outcome.emitted_events,
    );
    let (backup_events, backup_warnings) =
        queue_assets_for_backup(&state.backup_outbox, &backup_context, backup_assets.iter());
    emit_after_commit(&app, BACKUP_EVENT_CHANNEL, &backup_events);
    if !backup_events.is_empty() {
        state.backup_worker.wake();
    }
    for warning in backup_warnings {
        eprintln!("business local success with deferred backup warning: {warning}");
    }
    Ok(outcome.response)
}

fn business_workspace_command_context(
    command: &BusinessWorkspaceCommandEnvelope,
) -> &OperationContext {
    match command {
        BusinessWorkspaceCommandEnvelope::Create { context, .. }
        | BusinessWorkspaceCommandEnvelope::UpdateProfile { context, .. }
        | BusinessWorkspaceCommandEnvelope::CreateDocument { context, .. }
        | BusinessWorkspaceCommandEnvelope::PromoteReviewedContract { context, .. }
        | BusinessWorkspaceCommandEnvelope::ChangeDocumentStatus { context, .. }
        | BusinessWorkspaceCommandEnvelope::GenerateDocument { context, .. }
        | BusinessWorkspaceCommandEnvelope::UpsertPayment { context, .. }
        | BusinessWorkspaceCommandEnvelope::ConfirmQuote { context, .. }
        | BusinessWorkspaceCommandEnvelope::RecordReceipt { context, .. }
        | BusinessWorkspaceCommandEnvelope::ReverseReceipt { context, .. }
        | BusinessWorkspaceCommandEnvelope::AdoptLatestConfirmedRequirement { context, .. }
        | BusinessWorkspaceCommandEnvelope::UpsertCustomer { context, .. }
        | BusinessWorkspaceCommandEnvelope::AssignCustomer { context, .. }
        | BusinessWorkspaceCommandEnvelope::UpsertMilestone { context, .. }
        | BusinessWorkspaceCommandEnvelope::RegisterDeliverableVersion { context, .. }
        | BusinessWorkspaceCommandEnvelope::RecordDeliverySent { context, .. }
        | BusinessWorkspaceCommandEnvelope::RecordDeliverySignoff { context, .. }
        | BusinessWorkspaceCommandEnvelope::RecordInvoiceIssued { context, .. }
        | BusinessWorkspaceCommandEnvelope::RecordInvoiceRedCorrection { context, .. }
        | BusinessWorkspaceCommandEnvelope::AttachInvoiceAsset { context, .. }
        | BusinessWorkspaceCommandEnvelope::CreateArchiveSnapshot { context, .. }
        | BusinessWorkspaceCommandEnvelope::ChangeStatus { context, .. } => context,
    }
}

fn queue_assets_for_backup<'a>(
    backup_outbox: &BackupOutbox,
    context: &OperationContext,
    assets: impl Iterator<Item = &'a AssetRecord>,
) -> (Vec<BackupDomainEvent>, Vec<String>) {
    let mut events = Vec::new();
    let mut warnings = Vec::new();
    for asset in assets {
        let command = BackupCommandEnvelope::Queue {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: context.clone(),
            payload: QueueAssetBackupPayload {
                asset_id: asset.id.clone(),
            },
            idempotency_key: format!("auto-backup:{}:{}", asset.id, asset.sha256),
            expected_revision: None,
            deadline_at: None,
        };
        match backup_outbox.queue(command, &asset.sha256) {
            Ok(outcome) => events.extend(outcome.emitted_events),
            Err(error) => warnings.push(format!("{}: {}", asset.id, error)),
        }
    }
    (events, warnings)
}

#[tauri::command]
fn list_business_workspaces(
    state: State<'_, AppState>,
) -> Result<Vec<BusinessWorkspaceRecord>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "businessWorkspace.read",
        "businessWorkspace",
        None,
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    business_workspace_service::list(&connection)
}

#[tauri::command]
fn list_business_customers(
    state: State<'_, AppState>,
    request: ListBusinessCustomersRequest,
) -> Result<Vec<BusinessCustomerReceivableSummary>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "businessWorkspace.read",
        "businessCustomer",
        None,
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    business_workspace_service::list_customers(&connection, &request)
}

#[tauri::command]
fn list_business_workspace_prefill_candidates(
    state: State<'_, AppState>,
    request: ListBusinessWorkspacePrefillCandidatesRequest,
) -> Result<Vec<BusinessWorkspacePrefillCandidate>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "businessWorkspace.read",
        "businessWorkspace",
        Some(request.target_project_id.as_str()),
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    business_workspace_service::list_prefill_candidates(&connection, &request)
}

#[tauri::command]
fn preview_business_workspace_prefill(
    state: State<'_, AppState>,
    request: PreviewBusinessWorkspacePrefillRequest,
) -> Result<BusinessWorkspacePrefillPreview, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "businessWorkspace.read",
        "businessWorkspace",
        Some(request.target_project_id.as_str()),
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    business_workspace_service::preview_prefill(&connection, &request)
}

#[tauri::command]
fn replay_business_workspace_events(
    state: State<'_, AppState>,
    request: ReplayEventsRequest,
) -> Result<Vec<BusinessWorkspaceDomainEvent>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "businessWorkspace.read",
        "businessWorkspace",
        None,
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    business_workspace_service::replay_events(&connection, request.after_sequence, request.limit)
}

fn authorize_business_workspace_command(
    host: &BackendHost,
    command: &BusinessWorkspaceCommandEnvelope,
) -> Result<(), HostError> {
    let (actor_id, operation, resource_id) = match command {
        BusinessWorkspaceCommandEnvelope::Create {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.project_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::UpdateProfile {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::CreateDocument {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::UpsertPayment {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::ConfirmQuote {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.approve",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::RecordReceipt {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::ReverseReceipt {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.approve",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::AdoptLatestConfirmedRequirement {
            context,
            payload,
            ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::UpsertCustomer {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::AssignCustomer {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::UpsertMilestone {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::RegisterDeliverableVersion {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::RecordDeliverySent {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::RecordInvoiceIssued {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::AttachInvoiceAsset {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.write",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::RecordDeliverySignoff {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.approve",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::RecordInvoiceRedCorrection {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.approve",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::CreateArchiveSnapshot {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.approve",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::ChangeStatus {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            if payload.status == protocol::BusinessWorkspaceStatus::Archived {
                "businessWorkspace.approve"
            } else {
                "businessWorkspace.write"
            },
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::PromoteReviewedContract {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.approve",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::ChangeDocumentStatus {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.approve",
            payload.workspace_id.as_str(),
        ),
        BusinessWorkspaceCommandEnvelope::GenerateDocument {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "businessWorkspace.generate",
            payload.workspace_id.as_str(),
        ),
    };
    let effect = if operation == "businessWorkspace.approve" {
        security::OperationEffect::Irreversible
    } else {
        security::OperationEffect::ReversibleWrite
    };
    ensure_allowed(host.authorize_operation(
        actor_id,
        operation,
        "businessWorkspace",
        Some(resource_id),
        effect,
        None,
    )?)
}

#[tauri::command]
fn execute_ai_credential_command(
    state: State<'_, AppState>,
    command: AiCredentialCommandEnvelope,
) -> Result<AiCredentialCommandResponse, HostError> {
    let (actor_id, operation, resource_id, effect, refresh_runtime) = match &command {
        AiCredentialCommandEnvelope::Status { context, .. } => (
            context.actor_id.as_str(),
            "aiCredentials.read",
            "providers",
            security::OperationEffect::Read,
            false,
        ),
        AiCredentialCommandEnvelope::UpsertProvider {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "aiCredentials.provider.upsert",
            payload.provider_id.as_deref().unwrap_or("new-provider"),
            security::OperationEffect::ReversibleWrite,
            true,
        ),
        AiCredentialCommandEnvelope::RemoveProvider {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "aiCredentials.provider.remove",
            payload.provider_id.as_str(),
            security::OperationEffect::ReversibleWrite,
            true,
        ),
        AiCredentialCommandEnvelope::SelectProvider {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "aiCredentials.provider.select",
            payload.provider_id.as_str(),
            security::OperationEffect::ReversibleWrite,
            true,
        ),
        AiCredentialCommandEnvelope::TestProvider {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "aiCredentials.provider.test",
            payload.provider_id.as_str(),
            security::OperationEffect::ReversibleWrite,
            false,
        ),
        AiCredentialCommandEnvelope::DiscoverModels {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "aiCredentials.provider.discoverModels",
            payload.provider_id.as_deref().unwrap_or("draft-provider"),
            security::OperationEffect::ReversibleWrite,
            false,
        ),
        AiCredentialCommandEnvelope::ClearProviderApiKey {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "aiCredentials.provider.clearApiKey",
            payload.provider_id.as_str(),
            security::OperationEffect::ReversibleWrite,
            true,
        ),
        AiCredentialCommandEnvelope::SaveBsaigcApiKey { context, .. } => (
            context.actor_id.as_str(),
            "aiCredentials.write",
            "bsaigc-provider-api-key",
            security::OperationEffect::ReversibleWrite,
            true,
        ),
        AiCredentialCommandEnvelope::ClearBsaigcApiKey { context, .. } => (
            context.actor_id.as_str(),
            "aiCredentials.write",
            "bsaigc-provider-api-key",
            security::OperationEffect::ReversibleWrite,
            true,
        ),
    };
    ensure_allowed(state.host.authorize_operation(
        actor_id,
        operation,
        "aiCredentials",
        Some(resource_id),
        effect,
        None,
    )?)?;
    let response = state.ai_credentials.execute(command)?;
    if refresh_runtime && response.status.applies_on_next_runtime_start {
        state.brain.refresh_credentials();
    }
    Ok(response)
}

#[tauri::command]
fn execute_desktop_settings_command(
    state: State<'_, AppState>,
    command: DesktopSettingsCommandEnvelope,
) -> Result<DesktopSettingsCommandResponse, HostError> {
    let (context, operation, effect) = match &command {
        DesktopSettingsCommandEnvelope::Status { context, .. } => {
            (context, "settings.read", security::OperationEffect::Read)
        }
        DesktopSettingsCommandEnvelope::OpenStorageLocation { context, .. } => (
            context,
            "settings.openStorageLocation",
            security::OperationEffect::Read,
        ),
        DesktopSettingsCommandEnvelope::ClearCache { context, .. } => (
            context,
            "settings.clearCache",
            security::OperationEffect::ReversibleWrite,
        ),
        DesktopSettingsCommandEnvelope::CheckForUpdates { context, .. } => (
            context,
            "settings.checkForUpdates",
            security::OperationEffect::ReversibleWrite,
        ),
    };
    ensure_allowed(state.host.authorize_operation(
        &context.actor_id,
        operation,
        "desktopSettings",
        Some("desktop-settings"),
        effect,
        None,
    )?)?;
    state.desktop_settings.execute(command)
}

#[tauri::command]
fn execute_contract_review_command(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    command: ContractReviewCommandEnvelope,
) -> Result<ContractReviewCommandResponse, HostError> {
    authorize_contract_review_command(&state.host, &command)?;
    let mut connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    let contract_agent = contract_review_agent::CodexContractAgent::new(&state.brain);
    let outcome = contract_review_runtime::execute_contract_review_command_with_agent(
        &mut connection,
        &state.vault_root,
        &state.generated_staging_root,
        &state.backup_outbox,
        command,
        &contract_agent,
    )?;
    emit_after_commit(
        &app,
        CONTRACT_REVIEW_EVENT_CHANNEL,
        &outcome.contract_events,
    );
    emit_after_commit(&app, BACKUP_EVENT_CHANNEL, &outcome.backup_events);
    if !outcome.backup_events.is_empty() {
        state.backup_worker.wake();
    }
    for warning in outcome.backup_warnings {
        eprintln!("contract review local success with deferred backup warning: {warning}");
    }
    Ok(outcome.response)
}

#[tauri::command]
fn get_contract_review(
    state: State<'_, AppState>,
    request: GetContractReviewRequest,
) -> Result<ContractReviewRecord, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "contractReview.read",
        "contractReview",
        Some(request.review_id.as_str()),
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    contract_review_service::get_review(&connection, &request.review_id)
}

#[tauri::command]
fn list_contract_reviews(
    state: State<'_, AppState>,
    request: ListContractReviewsRequest,
) -> Result<Vec<ContractReviewRecord>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "contractReview.read",
        "contractReview",
        request.workspace_id.as_deref(),
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    contract_review_service::list_reviews(&connection, &request)
}

#[tauri::command]
fn list_review_findings(
    state: State<'_, AppState>,
    request: ListReviewFindingsRequest,
) -> Result<Vec<ReviewFindingRecord>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "contractReview.read",
        "contractReview",
        Some(request.review_id.as_str()),
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    contract_review_service::list_review_findings(&connection, &request.review_id, request.status)
}

#[tauri::command]
fn get_evidence_context(
    state: State<'_, AppState>,
    request: GetEvidenceContextRequest,
) -> Result<EvidenceContext, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "contractReview.readEvidence",
        "contractEvidence",
        Some(request.evidence_id.as_str()),
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    contract_review_service::get_evidence_context(&connection, &request.evidence_id)
}

#[tauri::command]
fn replay_contract_review_events(
    state: State<'_, AppState>,
    request: ReplayEventsRequest,
) -> Result<Vec<ContractReviewDomainEvent>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "contractReview.read",
        "contractReview",
        None,
        security::OperationEffect::Read,
        None,
    )?)?;
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    contract_review_service::replay_events(&connection, request.after_sequence, request.limit)
}

fn authorize_contract_review_command(
    host: &BackendHost,
    command: &ContractReviewCommandEnvelope,
) -> Result<(), HostError> {
    let (actor_id, operation, resource_id) = match command {
        ContractReviewCommandEnvelope::Create {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "contractReview.create",
            payload.workspace_id.as_str(),
        ),
        ContractReviewCommandEnvelope::Start {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "contractReview.run",
            payload.review_id.as_str(),
        ),
        ContractReviewCommandEnvelope::Cancel {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "contractReview.cancel",
            payload.review_id.as_str(),
        ),
        ContractReviewCommandEnvelope::DecideFinding {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "contractReview.decideFinding",
            payload.review_id.as_str(),
        ),
        ContractReviewCommandEnvelope::GenerateReport {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "contractReview.generateReport",
            payload.review_id.as_str(),
        ),
        ContractReviewCommandEnvelope::RetryStage {
            context, payload, ..
        } => (
            context.actor_id.as_str(),
            "contractReview.retry",
            payload.review_id.as_str(),
        ),
    };
    ensure_allowed(host.authorize_operation(
        actor_id,
        operation,
        "contractReview",
        Some(resource_id),
        security::OperationEffect::ReversibleWrite,
        None,
    )?)
}

#[tauri::command]
fn execute_backup_command(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    command: BackupCommandEnvelope,
) -> Result<BackupCommandResponse, HostError> {
    authorize_backup_command(&state.host, &command)?;
    if matches!(&command, BackupCommandEnvelope::Restore { .. }) {
        let outcome = state.backup_worker.restore(command)?;
        emit_after_commit(&app, BACKUP_EVENT_CHANNEL, &outcome.emitted_events);
        return Ok(outcome.response);
    }
    let should_wake_worker = matches!(
        &command,
        BackupCommandEnvelope::Queue { .. } | BackupCommandEnvelope::Retry { .. }
    );
    let cancelled_asset_id = match &command {
        BackupCommandEnvelope::Cancel { payload, .. } => Some(payload.asset_id.clone()),
        _ => None,
    };
    let queue_sha256 = match &command {
        BackupCommandEnvelope::Queue { payload, .. } => {
            let connection = state
                .asset_connection
                .lock()
                .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
            Some(asset_service::get_asset(&connection, &payload.asset_id)?.sha256)
        }
        BackupCommandEnvelope::Retry { .. }
        | BackupCommandEnvelope::Cancel { .. }
        | BackupCommandEnvelope::Restore { .. } => None,
    };
    let outcome = state
        .backup_outbox
        .execute_command(command, queue_sha256.as_deref())?;
    emit_after_commit(&app, BACKUP_EVENT_CHANNEL, &outcome.emitted_events);
    if let Some(asset_id) = cancelled_asset_id {
        state.backup_worker.cancel(&asset_id);
    } else if should_wake_worker {
        state.backup_worker.wake();
    }
    Ok(outcome.response)
}

#[tauri::command]
fn list_asset_backups(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<AssetBackupRecord>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "backup.read",
        "assetBackup",
        None,
        security::OperationEffect::Read,
        None,
    )?)?;
    state.backup_outbox.list(limit.unwrap_or(200) as usize)
}

#[tauri::command]
fn replay_backup_events(
    state: State<'_, AppState>,
    request: ReplayEventsRequest,
) -> Result<Vec<BackupDomainEvent>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "backup.read",
        "assetBackup",
        None,
        security::OperationEffect::Read,
        None,
    )?)?;
    state
        .backup_outbox
        .replay_events(request.after_sequence, request.limit as usize)
}

fn authorize_backup_command(
    host: &BackendHost,
    command: &BackupCommandEnvelope,
) -> Result<(), HostError> {
    let (context, operation, asset_id) = match command {
        BackupCommandEnvelope::Queue {
            context, payload, ..
        } => (context, "backup.queue", payload.asset_id.as_str()),
        BackupCommandEnvelope::Retry {
            context, payload, ..
        } => (context, "backup.retry", payload.asset_id.as_str()),
        BackupCommandEnvelope::Cancel {
            context, payload, ..
        } => (context, "backup.cancel", payload.asset_id.as_str()),
        BackupCommandEnvelope::Restore {
            context, payload, ..
        } => (context, "backup.restore", payload.asset_id.as_str()),
    };
    ensure_allowed(host.authorize_operation(
        context.actor_id.as_str(),
        operation,
        "assetBackup",
        Some(asset_id),
        security::OperationEffect::ReversibleWrite,
        None,
    )?)
}

#[tauri::command]
fn execute_execution_brief_command(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    command: ExecutionBriefCommandEnvelope,
) -> Result<ExecutionBriefCommandResponse, HostError> {
    let mut connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    let outcome = execution_brief_service::execute_command(&mut connection, command)?;
    emit_after_commit(&app, EXECUTION_BRIEF_EVENT_CHANNEL, &outcome.emitted_events);
    Ok(outcome.response)
}

#[tauri::command]
fn list_execution_briefs(
    state: State<'_, AppState>,
) -> Result<Vec<ExecutionBriefRecord>, HostError> {
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    execution_brief_service::list(&connection)
}

#[tauri::command]
fn replay_execution_brief_events(
    state: State<'_, AppState>,
    request: ReplayEventsRequest,
) -> Result<Vec<ExecutionBriefDomainEvent>, HostError> {
    let connection = state
        .asset_connection
        .lock()
        .map_err(|_| HostError::internal("domain SQLite lock is poisoned"))?;
    execution_brief_service::replay_events(&connection, request.after_sequence, request.limit)
}

#[tauri::command]
fn report_diagnostic(
    state: State<'_, AppState>,
    payload: DiagnosticReportPayload,
) -> Result<DiagnosticRecord, HostError> {
    state.diagnostics.report(payload)
}

#[tauri::command]
fn list_diagnostics(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<DiagnosticRecord>, HostError> {
    state.diagnostics.list_queued(limit.unwrap_or(100))
}

#[tauri::command]
fn upsert_memory(
    state: State<'_, AppState>,
    record: MemoryRecord,
    expected_revision: Option<i64>,
) -> Result<MemoryRecord, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "memory.put",
        "memory",
        Some(&record.id),
        security::OperationEffect::ReversibleWrite,
        None,
    )?)?;
    let connection = state
        .memory_connection
        .lock()
        .map_err(|_| HostError::internal("memory SQLite lock is poisoned"))?;
    memory_service::upsert(&connection, &record, expected_revision)
}

#[tauri::command]
fn list_memories(
    state: State<'_, AppState>,
    scope: Option<MemoryScope>,
    scope_id: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<MemoryRecord>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "memory.list",
        "memory",
        scope_id.as_deref(),
        security::OperationEffect::Read,
        None,
    )?)?;
    let query = memory_query(scope, scope_id, offset, limit)?;
    let connection = state
        .memory_connection
        .lock()
        .map_err(|_| HostError::internal("memory SQLite lock is poisoned"))?;
    memory_service::list(&connection, &query)
}

#[tauri::command]
fn search_memories(
    state: State<'_, AppState>,
    text: String,
    scope: Option<MemoryScope>,
    scope_id: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<MemoryRecord>, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "memory.search",
        "memory",
        scope_id.as_deref(),
        security::OperationEffect::Read,
        None,
    )?)?;
    let query = memory_query(scope, scope_id, offset, limit)?;
    let connection = state
        .memory_connection
        .lock()
        .map_err(|_| HostError::internal("memory SQLite lock is poisoned"))?;
    memory_service::search(&connection, &text, &query)
}

#[tauri::command]
fn delete_memory(
    state: State<'_, AppState>,
    id: String,
    expected_revision: i64,
    approval_id: Option<String>,
) -> Result<MemoryRecord, HostError> {
    ensure_allowed(state.host.authorize_operation(
        "local-operator",
        "memory.delete",
        "memory",
        Some(&id),
        security::OperationEffect::Irreversible,
        approval_id.as_deref(),
    )?)?;
    let connection = state
        .memory_connection
        .lock()
        .map_err(|_| HostError::internal("memory SQLite lock is poisoned"))?;
    memory_service::delete(&connection, &id, expected_revision)
}

#[tauri::command]
async fn brain_thread_start(
    state: State<'_, AppState>,
    request: StartBrainThreadRequest,
) -> Result<BrainThreadRecord, HostError> {
    let brain = state.brain.clone();
    run_brain(move || brain.start_thread(request)).await
}

#[tauri::command]
async fn brain_thread_resume(
    state: State<'_, AppState>,
    request: ResumeBrainThreadRequest,
) -> Result<BrainThreadRecord, HostError> {
    let brain = state.brain.clone();
    run_brain(move || brain.resume_thread(request)).await
}

#[tauri::command]
async fn brain_thread_list_remote(
    state: State<'_, AppState>,
    request: ListRemoteBrainThreadsRequest,
) -> Result<RemoteBrainThreadPage, HostError> {
    let brain = state.brain.clone();
    run_brain(move || brain.list_remote_threads(request)).await
}

#[tauri::command]
async fn brain_turn_start(
    state: State<'_, AppState>,
    request: StartBrainTurnRequest,
    context: Option<BrainTurnContext>,
) -> Result<BrainTurnStartResult, HostError> {
    let context = context.unwrap_or_default();
    if context.attachment_asset_ids.len() > 20 {
        return Err(HostError::validation("attach at most 20 files to one turn"));
    }
    let workspace_root = context
        .workspace_token
        .as_deref()
        .map(|token| state.brain_workspaces.resolve(token))
        .transpose()?;
    let attachments = {
        let connection = state
            .asset_connection
            .lock()
            .map_err(|_| HostError::internal("asset SQLite lock is poisoned"))?;
        let mut attachments = Vec::with_capacity(context.attachment_asset_ids.len());
        for asset_id in &context.attachment_asset_ids {
            let (asset, path) = asset_service::verify_ready_asset_integrity(
                &connection,
                &state.vault_root,
                asset_id,
            )?;
            attachments.push(BrainExecutionAttachment {
                display_name: asset.original_name,
                mime_type: asset.mime_type,
                path,
                is_image: asset.kind == protocol::AssetKind::Image,
            });
        }
        attachments
    };
    let execution_context = BrainExecutionContext {
        workspace_root,
        access_mode: context.access_mode,
        attachments,
    };
    let brain = state.brain.clone();
    run_brain(move || brain.start_turn_with_context(request, execution_context)).await
}

#[tauri::command]
async fn brain_turn_interrupt(
    state: State<'_, AppState>,
    request: InterruptBrainTurnRequest,
) -> Result<BrainTurnRecord, HostError> {
    let brain = state.brain.clone();
    run_brain(move || brain.interrupt_turn(request)).await
}

#[tauri::command]
fn brain_list_local_threads(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<Vec<BrainThreadRecord>, HostError> {
    state.brain.list_local_threads(project_id.as_deref())
}

#[tauri::command]
fn brain_thread_archive(
    state: State<'_, AppState>,
    thread_id: String,
    archived: bool,
) -> Result<BrainThreadRecord, HostError> {
    state.brain.archive_local_thread(&thread_id, archived)
}

#[tauri::command]
fn brain_thread_rename(
    state: State<'_, AppState>,
    thread_id: String,
    title: String,
) -> Result<BrainThreadRecord, HostError> {
    state.brain.rename_local_thread(&thread_id, &title)
}

#[tauri::command]
fn brain_thread_delete(state: State<'_, AppState>, thread_id: String) -> Result<(), HostError> {
    state.brain.delete_local_thread(&thread_id)
}

#[tauri::command]
fn brain_list_local_turns(
    state: State<'_, AppState>,
    thread_id: String,
) -> Result<Vec<BrainTurnRecord>, HostError> {
    state.brain.list_local_turns(&thread_id)
}

#[tauri::command]
fn get_brain_health(state: State<'_, AppState>) -> BrainHostHealth {
    state.brain.health()
}

#[tauri::command]
fn get_native_media_health(state: State<'_, AppState>) -> NativeMediaHealth {
    let health = state.media_engine.health();
    NativeMediaHealth {
        state: if health.ffmpeg_available && health.ffprobe_available {
            "ready"
        } else if health.ffmpeg_available || health.ffprobe_available {
            "degraded"
        } else {
            "unavailable"
        }
        .to_string(),
        ffmpeg_available: health.ffmpeg_available,
        ffprobe_available: health.ffprobe_available,
        ffmpeg_source: health.ffmpeg_source.map(media_tool_source_name),
        ffprobe_source: health.ffprobe_source.map(media_tool_source_name),
    }
}

fn media_tool_source_name(source: MediaToolSource) -> String {
    match source {
        MediaToolSource::EnvironmentOverride => "environment",
        MediaToolSource::BundledRuntime => "bundled",
        MediaToolSource::SystemPath => "systemPath",
    }
    .to_string()
}

async fn run_brain<T, F>(operation: F) -> Result<T, HostError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, HostError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| HostError::internal(format!("brain host task failed: {error}")))?
}

fn memory_query(
    scope: Option<MemoryScope>,
    scope_id: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<memory_service::MemoryQuery, HostError> {
    let (project_id, thread_id) = match scope.as_ref() {
        Some(MemoryScope::Project) => (scope_id, None),
        Some(MemoryScope::Thread) => (None, scope_id),
        Some(MemoryScope::Global) => {
            if scope_id.is_some() {
                return Err(HostError::validation("global memory rejects scopeId"));
            }
            (None, None)
        }
        None => {
            if scope_id.is_some() {
                return Err(HostError::validation("scopeId requires scope"));
            }
            (None, None)
        }
    };
    Ok(memory_service::MemoryQuery {
        scope,
        project_id,
        thread_id,
        offset: offset.unwrap_or(0),
        limit: limit.unwrap_or(100),
    })
}

fn ensure_allowed(decision: protocol::PermissionDecision) -> Result<(), HostError> {
    if decision.allowed {
        return Ok(());
    }
    let approval = decision
        .approval_id
        .map(|id| format!(" approvalId={id}"))
        .unwrap_or_default();
    Err(HostError::new(
        "APPROVAL_REQUIRED",
        format!(
            "{}{}",
            decision
                .reason
                .unwrap_or_else(|| "operation requires approval".to_string()),
            approval
        ),
        false,
    ))
}

#[tauri::command]
fn get_host_status(state: State<'_, AppState>) -> Result<HostStatus, HostError> {
    state.host.status()
}

#[tauri::command]
fn list_pending_approvals(state: State<'_, AppState>) -> Result<Vec<ApprovalRecord>, HostError> {
    state.host.list_pending_approvals()
}

#[tauri::command]
fn resolve_approval(
    state: State<'_, AppState>,
    payload: ResolveApprovalPayload,
) -> Result<ApprovalRecord, HostError> {
    state.host.resolve_approval(&payload)
}

#[tauri::command]
async fn probe_codex() -> Result<CodexProbeStatus, HostError> {
    tauri::async_runtime::spawn_blocking(codex_host::probe_codex)
        .await
        .map_err(|error| HostError::internal(format!("Codex 探测任务失败: {error}")))
}

fn emit_after_commit<S: serde::Serialize>(app: &tauri::AppHandle, channel: &str, events: &[S]) {
    for event in events {
        if let Err(error) = app.emit(channel, event) {
            eprintln!("event delivery failed after commit: channel={channel} error={error}");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_root = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("无法解析应用数据目录: {error}"))?;
            let resource_dir = app
                .path()
                .resource_dir()
                .map_err(|error| format!("无法解析应用资源目录: {error}"))?;
            let skill_install = business_skill_service::install_bundled_business_skills(
                &data_root,
                &resource_dir,
            )
            .map_err(|error| format!("内置商务 Skills 安装失败: {error}"))?;
            eprintln!(
                "business skill bundle installed: version={} skills={} files={}",
                skill_install.bundle_version,
                skill_install.skill_count,
                skill_install.file_count
            );
            let database_path = data_root.join("ledger").join("bsaigc.sqlite3");
            let vault_path = data_root.join("vault");
            let generated_staging_root = data_root.join("staging").join("contract-review");
            let chat_attachment_staging_root =
                data_root.join("staging").join("chat-attachments");
            let desktop_settings = DesktopSettingsService::open(
                &data_root,
                codex_host::REQUIRED_CODEX_VERSION,
            )
            .map_err(|error| error.to_string())?;
            let mut host = BackendHost::open(&database_path, &vault_path)
                .map_err(|error| error.to_string())?;
            let tasks = Arc::new(
                TaskEngine::open(&database_path).map_err(|error| error.to_string())?,
            );
            let media_engine = Arc::new(MediaEngine::discover());

            let mut asset_connection = Connection::open(&database_path)
                .map_err(|error| format!("open asset SQLite failed: {error}"))?;
            asset_connection
                .busy_timeout(std::time::Duration::from_secs(5))
                .map_err(|error| format!("configure asset SQLite failed: {error}"))?;
            asset_service::migrate(&asset_connection).map_err(|error| error.to_string())?;
            if let Err(error) =
                asset_service::reconcile_pending_imports(&asset_connection, &vault_path)
            {
                eprintln!("asset import startup reconciliation deferred: {error}");
            }
            case_library::migrate(&asset_connection).map_err(|error| error.to_string())?;
            requirement_brief_service::migrate(&asset_connection)
                .map_err(|error| error.to_string())?;
            execution_brief_service::migrate(&asset_connection)
                .map_err(|error| error.to_string())?;
            business_workspace_service::migrate(&asset_connection)
                .map_err(|error| error.to_string())?;
            contract_review_service::migrate(&asset_connection)
                .map_err(|error| error.to_string())?;
            if let Err(error) = business_workspace_service::reconcile_generated_assets(
                &mut asset_connection,
                &vault_path,
            ) {
                eprintln!("business document startup reconciliation deferred: {error}");
            }
            let asset_connection = Arc::new(Mutex::new(asset_connection));
            let backup_outbox = Arc::new(
                BackupOutbox::open(&database_path).map_err(|error| error.to_string())?,
            );
            let backup_event_app = app.handle().clone();
            let backup_worker = R2BackupWorker::start_from_env(
                Arc::clone(&backup_outbox),
                database_path.clone(),
                vault_path.clone(),
                Arc::new(move |events| {
                    emit_after_commit(&backup_event_app, BACKUP_EVENT_CHANNEL, events)
                }),
            );
            if let Some(reason) = backup_worker.degraded_reason() {
                eprintln!("R2 backup sidecar degraded: {reason}");
            }
            let diagnostics =
                DiagnosticOutbox::open(&database_path).map_err(|error| error.to_string())?;
            let memory_connection = Connection::open(&database_path)
                .map_err(|error| format!("open memory SQLite failed: {error}"))?;
            memory_connection
                .busy_timeout(std::time::Duration::from_secs(5))
                .map_err(|error| format!("configure memory SQLite failed: {error}"))?;
            memory_service::migrate(&memory_connection).map_err(|error| error.to_string())?;
            let ai_credentials = Arc::new(
                AiCredentialService::open(&data_root).map_err(|error| error.to_string())?,
            );
            let business_tool_host = BackendHost::open(&database_path, &vault_path)
                .map_err(|error| error.to_string())?;
            let business_tool_adapter = BusinessToolHostAdapter::new(
                business_tool_host,
                Arc::clone(&asset_connection),
                vault_path.clone(),
                data_root.join("staging").join("brain-artifacts"),
                Arc::clone(&backup_outbox),
                Arc::clone(&tasks),
            )
            .map_err(|error| error.to_string())?;
            let brain = BrainHost::open_with_services(
                &database_path,
                &data_root.join("brain-workspace"),
                Arc::clone(&ai_credentials),
                BusinessToolRegistry::new(business_tool_adapter),
            )
            .map_err(|error| error.to_string())?;
            let brain_app = app.handle().clone();
            let brain_subscription = brain.subscribe(move |event| {
                if let Err(error) = brain_app.emit(BRAIN_EVENT_CHANNEL, event) {
                    eprintln!("brain event delivery failed: {error}");
                }
            });

            let task_event_app = app.handle().clone();
            let task_event_sink: TaskLifecycleEventSink = Arc::new(move |event| {
                if let Err(error) = task_event_app.emit(TASK_EVENT_CHANNEL, &event) {
                    eprintln!(
                        "task event delivery failed after commit: event_id={} trace_id={} error={error}",
                        event.event_id, event.trace_id
                    );
                }
                Ok(())
            });
            let task_runner = TaskRunner::with_event_sink(
                Arc::clone(&tasks),
                4,
                task_event_sink,
            )
            .map_err(|error| error.to_string())?;
            register_media_task_handlers(
                &task_runner,
                MediaTaskServices::new(
                    Arc::clone(&media_engine),
                    Arc::clone(&asset_connection),
                    vault_path.clone(),
                ),
            )
            .map_err(|error| error.to_string())?;
            task_runner.start().map_err(|error| error.to_string())?;

            host.set_module_availability("task.engine", ModuleAvailability::Ready);
            host.set_module_availability("asset.vault", ModuleAvailability::Ready);
            host.set_module_availability("diagnostic.outbox", ModuleAvailability::Ready);
            host.set_module_availability("memory.local", ModuleAvailability::Ready);
            host.set_module_availability(
                "intake.requirementBrief",
                ModuleAvailability::Ready,
            );
            host.set_module_availability(
                "creative.caseLibrary",
                ModuleAvailability::Ready,
            );
            host.set_module_availability(
                "production.executionBrief",
                ModuleAvailability::Ready,
            );
            host.set_module_availability(
                "business.documentCenter",
                ModuleAvailability::Ready,
            );
            host.set_module_availability("document.intelligence", ModuleAvailability::Ready);
            host.set_module_availability("business.contractReview", ModuleAvailability::Ready);
            host.set_module_availability("business.reviewReport", ModuleAvailability::Ready);
            host.set_module_availability("brain.credentials", ModuleAvailability::Ready);
            host.set_module_availability("desktop.settings", ModuleAvailability::Ready);
            host.set_module_availability("business.toolRegistry", ModuleAvailability::Ready);
            host.set_module_availability(
                "vault.backup.r2",
                if backup_worker.is_ready() {
                    ModuleAvailability::Ready
                } else {
                    ModuleAvailability::Degraded
                },
            );
            let media_health = media_engine.health();
            host.set_module_availability(
                "media.native",
                if media_health.ffmpeg_available && media_health.ffprobe_available {
                    ModuleAvailability::Ready
                } else {
                    ModuleAvailability::Degraded
                },
            );

            let auth = auth_service::AuthService::new(asset_connection.clone())?;

            app.manage(AppState {
                auth,
                host,
                tasks,
                task_runner,
                media_engine,
                asset_connection,
                asset_sources: AssetSourceRegistry::default(),
                brain_workspaces: BrainWorkspaceRegistry::default(),
                vault_root: vault_path,
                generated_staging_root,
                chat_attachment_staging_root,
                backup_outbox,
                backup_worker,
                diagnostics,
                memory_connection: Mutex::new(memory_connection),
                brain,
                ai_credentials,
                desktop_settings,
                _brain_subscription: brain_subscription,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth_status,
            auth_initialize_admin,
            auth_login,
            auth_logout,
            auth_remembered_credentials,
            auth_remember_credentials,
            auth_forget_credentials,
            auth_change_password,
            auth_list_users,
            auth_create_user,
            auth_reset_password,
            auth_delete_user,
            auth_refresh_registry,
            execute_command,
            list_projects,
            replay_events,
            execute_task_command,
            list_tasks,
            replay_task_events,
            select_asset_source,
            select_asset_sources,
            select_brain_workspace,
            register_brain_dropped_paths,
            stage_clipboard_image,
            get_brain_attachment_preview,
            execute_asset_command,
            list_assets,
            replay_asset_events,
            get_asset_action_capabilities,
            open_asset,
            export_asset,
            execute_case_command,
            list_cases,
            replay_case_events,
            execute_requirement_brief_command,
            list_requirement_briefs,
            replay_requirement_brief_events,
            execute_business_workspace_command,
            execute_ai_credential_command,
            execute_desktop_settings_command,
            list_business_workspaces,
            list_business_customers,
            list_business_workspace_prefill_candidates,
            preview_business_workspace_prefill,
            replay_business_workspace_events,
            execute_contract_review_command,
            get_contract_review,
            list_contract_reviews,
            list_review_findings,
            get_evidence_context,
            replay_contract_review_events,
            execute_backup_command,
            list_asset_backups,
            replay_backup_events,
            execute_execution_brief_command,
            list_execution_briefs,
            replay_execution_brief_events,
            report_diagnostic,
            list_diagnostics,
            upsert_memory,
            list_memories,
            search_memories,
            delete_memory,
            brain_thread_start,
            brain_thread_resume,
            brain_thread_list_remote,
            brain_turn_start,
            brain_turn_interrupt,
            brain_list_local_threads,
            brain_list_local_turns,
            brain_thread_archive,
            brain_thread_rename,
            brain_thread_delete,
            get_brain_health,
            get_native_media_health,
            get_host_status,
            list_pending_approvals,
            resolve_approval,
            probe_codex
        ])
        .run(tauri::generate_context!())
        .expect("error while running BSAIGC desktop application");
}
