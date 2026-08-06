use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const PROTOCOL_VERSION: &str = "1.5";
pub const PREVIOUS_PROTOCOL_VERSION: &str = "1.4";
pub const PROTOCOL_1_3_VERSION: &str = "1.3";
pub const LEGACY_PROTOCOL_VERSION: &str = "1.2";
/// Business Workspace was introduced in protocol 1.4. Protocol 1.5 extends
/// creation with an optional, explicit historical master-data prefill source.
pub const BUSINESS_WORKSPACE_PROTOCOL_VERSION: &str = "1.6";
pub const BUSINESS_WORKSPACE_PREVIOUS_PROTOCOL_VERSION: &str = "1.5";
pub const BUSINESS_WORKSPACE_LEGACY_PROTOCOL_VERSION: &str = "1.4";
pub const CONTRACT_REVIEW_PROTOCOL_VERSION: &str = PROTOCOL_VERSION;
pub const BACKUP_PROTOCOL_VERSION: &str = PROTOCOL_VERSION;
pub const SHARED_CASE_PROTOCOL_VERSION: &str = PROTOCOL_VERSION;
pub const AI_CREDENTIAL_PROTOCOL_VERSION: &str = PROTOCOL_VERSION;
pub const DESKTOP_SETTINGS_PROTOCOL_VERSION: &str = PROTOCOL_VERSION;

/// Compatibility gate for command surfaces that existed before protocol 1.3.
/// These surfaces accept protocol 1.2, 1.3, 1.4, and the current protocol 1.5.
pub fn is_legacy_surface_protocol_supported(protocol_version: &str) -> bool {
    protocol_version == PROTOCOL_VERSION
        || protocol_version == PREVIOUS_PROTOCOL_VERSION
        || protocol_version == PROTOCOL_1_3_VERSION
        || protocol_version == LEGACY_PROTOCOL_VERSION
}

/// Compatibility gate for command surfaces introduced in protocol 1.3.
/// These surfaces accept protocol 1.3, 1.4, and the current protocol 1.5.
pub fn is_protocol_1_3_surface_supported(protocol_version: &str) -> bool {
    protocol_version == PROTOCOL_VERSION
        || protocol_version == PREVIOUS_PROTOCOL_VERSION
        || protocol_version == PROTOCOL_1_3_VERSION
}

pub const DOMAIN_EVENT_CHANNEL: &str = "bsaigc://domain-event";
pub const TASK_EVENT_CHANNEL: &str = "bsaigc://task-event";
pub const ASSET_EVENT_CHANNEL: &str = "bsaigc://asset-event";
pub const CASE_EVENT_CHANNEL: &str = "bsaigc://case-event";
pub const SHARED_CASE_EVENT_CHANNEL: &str = "bsaigc://shared-case-event";
pub const EXECUTION_BRIEF_EVENT_CHANNEL: &str = "bsaigc://execution-brief-event";
pub const REQUIREMENT_BRIEF_EVENT_CHANNEL: &str = "bsaigc://requirement-brief-event";
pub const BUSINESS_WORKSPACE_EVENT_CHANNEL: &str = "bsaigc://business-workspace-event";
pub const CONTRACT_REVIEW_EVENT_CHANNEL: &str = "bsaigc://contract-review-event";
pub const BACKUP_EVENT_CHANNEL: &str = "bsaigc://backup-event";
pub const BRAIN_EVENT_CHANNEL: &str = "bsaigc://brain-event";

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct OperationContext {
    pub actor_id: String,
    pub account_id: Option<String>,
    pub project_id: Option<String>,
    pub window_id: String,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BriefRecord {
    pub objective: String,
    pub audience: String,
    pub deliverables: Vec<String>,
    pub style_keywords: Vec<String>,
    pub mandatory_items: Vec<String>,
    pub constraints: Vec<String>,
    pub risks: Vec<String>,
    pub reference_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ProjectStage {
    Intake,
    Briefing,
    Creative,
    Production,
    PostProduction,
    Review,
    Delivery,
    Closed,
}

impl ProjectStage {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Intake => "intake",
            Self::Briefing => "briefing",
            Self::Creative => "creative",
            Self::Production => "production",
            Self::PostProduction => "postProduction",
            Self::Review => "review",
            Self::Delivery => "delivery",
            Self::Closed => "closed",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "intake" => Self::Intake,
            "briefing" => Self::Briefing,
            "creative" => Self::Creative,
            "production" => Self::Production,
            "postProduction" => Self::PostProduction,
            "review" => Self::Review,
            "delivery" => Self::Delivery,
            "closed" => Self::Closed,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub client_name: String,
    pub brief: BriefRecord,
    pub stage: ProjectStage,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CreateProjectPayload {
    pub name: String,
    pub client_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpdateProjectBriefPayload {
    pub project_id: String,
    pub brief: BriefRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ChangeProjectStagePayload {
    pub project_id: String,
    pub stage: ProjectStage,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum TaskStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Canceled,
    AwaitingApproval,
}

impl TaskStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
            Self::AwaitingApproval => "awaitingApproval",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "canceled" => Self::Canceled,
            "awaitingApproval" => Self::AwaitingApproval,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TaskPriority {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "low" => Self::Low,
            "normal" => Self::Normal,
            "high" => Self::High,
            "critical" => Self::Critical,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum TaskReplayPolicy {
    Safe,
    Manual,
    Never,
}

impl TaskReplayPolicy {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Manual => "manual",
            Self::Never => "never",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "safe" => Self::Safe,
            "manual" => Self::Manual,
            "never" => Self::Never,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct TaskDependency {
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    pub kind: String,
    pub project_id: Option<String>,
    #[ts(type = "unknown")]
    pub input: serde_json::Value,
    #[ts(type = "unknown | null")]
    pub output: Option<serde_json::Value>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub replay_policy: TaskReplayPolicy,
    pub progress: u8,
    pub attempt: u32,
    pub max_attempts: u32,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    #[ts(type = "number | null")]
    pub started_at: Option<i64>,
    #[ts(type = "number | null")]
    pub finished_at: Option<i64>,
    pub last_error: Option<String>,
    pub dependencies: Vec<TaskDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CreateTaskPayload {
    pub kind: String,
    pub project_id: Option<String>,
    #[ts(type = "unknown")]
    pub input: serde_json::Value,
    pub priority: TaskPriority,
    pub replay_policy: TaskReplayPolicy,
    pub max_attempts: u32,
    pub dependency_task_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CancelTaskPayload {
    pub task_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RetryTaskPayload {
    pub task_id: String,
    pub approved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum TaskCommandEnvelope {
    #[serde(rename = "task.create", rename_all = "camelCase")]
    #[ts(rename = "task.create")]
    Create {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CreateTaskPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "task.cancel", rename_all = "camelCase")]
    #[ts(rename = "task.cancel")]
    Cancel {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CancelTaskPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "task.retry", rename_all = "camelCase")]
    #[ts(rename = "task.retry")]
    Retry {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RetryTaskPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct TaskCommandResponse {
    pub receipt: CommandReceipt,
    pub task: TaskRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum TaskEventType {
    #[serde(rename = "task.created")]
    #[ts(rename = "task.created")]
    Created,
    #[serde(rename = "task.canceled")]
    #[ts(rename = "task.canceled")]
    Canceled,
    #[serde(rename = "task.retried")]
    #[ts(rename = "task.retried")]
    Retried,
    #[serde(rename = "task.progressed")]
    #[ts(rename = "task.progressed")]
    Progressed,
    #[serde(rename = "task.succeeded")]
    #[ts(rename = "task.succeeded")]
    Succeeded,
    #[serde(rename = "task.failed")]
    #[ts(rename = "task.failed")]
    Failed,
    #[serde(rename = "task.awaitingApproval")]
    #[ts(rename = "task.awaitingApproval")]
    AwaitingApproval,
    #[serde(rename = "task.recovered")]
    #[ts(rename = "task.recovered")]
    Recovered,
}

impl TaskEventType {
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Created => "task.created",
            Self::Canceled => "task.canceled",
            Self::Retried => "task.retried",
            Self::Progressed => "task.progressed",
            Self::Succeeded => "task.succeeded",
            Self::Failed => "task.failed",
            Self::AwaitingApproval => "task.awaitingApproval",
            Self::Recovered => "task.recovered",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct TaskDomainEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_id: String,
    pub event_type: TaskEventType,
    pub aggregate_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub trace_id: String,
    pub task: TaskRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum AssetKind {
    Image,
    Video,
    Audio,
    Document,
    Other,
}

impl AssetKind {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Document => "document",
            Self::Other => "other",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "image" => Self::Image,
            "video" => Self::Video,
            "audio" => Self::Audio,
            "document" => Self::Document,
            "other" => Self::Other,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum AssetStatus {
    Ready,
    Failed,
}

impl AssetStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AssetRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub original_name: String,
    pub kind: AssetKind,
    pub mime_type: String,
    #[ts(type = "number")]
    pub size_bytes: i64,
    pub sha256: String,
    pub status: AssetStatus,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    pub preview_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AssetSourceSelection {
    pub source_token: String,
    pub display_name: String,
    pub detected_kind: AssetKind,
    #[ts(type = "number")]
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BrainWorkspaceSelection {
    pub workspace_token: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BrainProjectWorkspaceBinding {
    pub project_id: String,
    pub workspace_token: String,
    pub display_name: String,
    #[ts(type = "number")]
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BrainDroppedItems {
    pub files: Vec<AssetSourceSelection>,
    pub workspace: Option<BrainWorkspaceSelection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct StageClipboardImageRequest {
    pub file_name: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BrainAttachmentPreview {
    pub mime_type: String,
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ImportAssetPayload {
    pub source_token: String,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum AssetCommandEnvelope {
    #[serde(rename = "asset.import", rename_all = "camelCase")]
    #[ts(rename = "asset.import")]
    Import {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: ImportAssetPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AssetCommandResponse {
    pub receipt: CommandReceipt,
    pub asset: AssetRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum AssetEventType {
    #[serde(rename = "asset.imported")]
    #[ts(rename = "asset.imported")]
    Imported,
}

impl AssetEventType {
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Imported => "asset.imported",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AssetDomainEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_id: String,
    pub event_type: AssetEventType,
    pub aggregate_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub trace_id: String,
    pub asset: AssetRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum CaseContentType {
    Brand,
    Property,
    Interview,
    Lifestyle,
    Product,
    Event,
    Documentary,
    Narrative,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum CasePresentation {
    LiveAction,
    Animation,
    MixedMedia,
    Aigc,
    Graphic,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum CaseQualityTier {
    Reference,
    Featured,
    Premium,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CaseRecord {
    pub id: String,
    pub asset_id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub client_name: String,
    pub content_type: CaseContentType,
    pub presentation: CasePresentation,
    pub has_actors: bool,
    pub is_aigc: bool,
    pub quality_tier: CaseQualityTier,
    pub tags: Vec<String>,
    pub notes: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CreateCasePayload {
    pub asset_id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub client_name: String,
    pub content_type: CaseContentType,
    pub presentation: CasePresentation,
    pub has_actors: bool,
    pub is_aigc: bool,
    pub quality_tier: CaseQualityTier,
    pub tags: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpdateCasePayload {
    pub case_id: String,
    pub title: String,
    pub client_name: String,
    pub content_type: CaseContentType,
    pub presentation: CasePresentation,
    pub has_actors: bool,
    pub is_aigc: bool,
    pub quality_tier: CaseQualityTier,
    pub tags: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum CaseCommandEnvelope {
    #[serde(rename = "case.create", rename_all = "camelCase")]
    #[ts(rename = "case.create")]
    Create {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CreateCasePayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "case.update", rename_all = "camelCase")]
    #[ts(rename = "case.update")]
    Update {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: UpdateCasePayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CaseCommandResponse {
    pub receipt: CommandReceipt,
    pub case_record: CaseRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum CaseEventType {
    #[serde(rename = "case.created")]
    #[ts(rename = "case.created")]
    Created,
    #[serde(rename = "case.updated")]
    #[ts(rename = "case.updated")]
    Updated,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CaseDomainEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_id: String,
    pub event_type: CaseEventType,
    pub aggregate_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub trace_id: String,
    pub case_record: CaseRecord,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum SharedCasePermission {
    Discover,
    Preview,
    Reference,
    Download,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum SharedCasePublicationStatus {
    PendingBackup,
    Published,
    Withdrawn,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct SharedCaseGrant {
    pub username: String,
    pub permissions: Vec<SharedCasePermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct SharedCasePublicationRecord {
    pub id: String,
    pub case_id: String,
    pub asset_id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub client_name: String,
    pub content_sha256: String,
    pub remote_object_key: Option<String>,
    pub remote_etag: Option<String>,
    pub status: SharedCasePublicationStatus,
    pub publisher_username: String,
    pub grants: Vec<SharedCaseGrant>,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    #[ts(type = "number | null")]
    pub published_at: Option<i64>,
    #[ts(type = "number | null")]
    pub withdrawn_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct PublishSharedCasePayload {
    pub case_id: String,
    pub grants: Vec<SharedCaseGrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpdateSharedCaseGrantsPayload {
    pub publication_id: String,
    pub grants: Vec<SharedCaseGrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct WithdrawSharedCasePayload {
    pub publication_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum SharedCaseCommandEnvelope {
    #[serde(rename = "sharedCase.publish", rename_all = "camelCase")]
    #[ts(rename = "sharedCase.publish")]
    Publish {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: PublishSharedCasePayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "sharedCase.updateGrants", rename_all = "camelCase")]
    #[ts(rename = "sharedCase.updateGrants")]
    UpdateGrants {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: UpdateSharedCaseGrantsPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "sharedCase.withdraw", rename_all = "camelCase")]
    #[ts(rename = "sharedCase.withdraw")]
    Withdraw {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: WithdrawSharedCasePayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct SharedCaseCommandResponse {
    pub receipt: CommandReceipt,
    pub publication: SharedCasePublicationRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum SharedCaseEventType {
    #[serde(rename = "sharedCase.published")]
    #[ts(rename = "sharedCase.published")]
    Published,
    #[serde(rename = "sharedCase.grantsUpdated")]
    #[ts(rename = "sharedCase.grantsUpdated")]
    GrantsUpdated,
    #[serde(rename = "sharedCase.withdrawn")]
    #[ts(rename = "sharedCase.withdrawn")]
    Withdrawn,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct SharedCaseDomainEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_id: String,
    pub event_type: SharedCaseEventType,
    pub aggregate_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub trace_id: String,
    pub publication: SharedCasePublicationRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ExecutionBriefStatus {
    Draft,
    Ready,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ExecutionBriefContent {
    #[ts(type = "number | null")]
    pub shoot_at: Option<i64>,
    pub client_goal: String,
    pub visual_style: String,
    pub primary_shots: Vec<String>,
    pub secondary_shots: Vec<String>,
    pub required_shots: Vec<String>,
    pub fallback_shots: Vec<String>,
    pub risk_points: Vec<String>,
    pub waiting_time_actions: Vec<String>,
    pub equipment_notes: String,
    pub post_shoot_highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ExecutionBriefRecord {
    pub id: String,
    pub project_id: String,
    pub content: ExecutionBriefContent,
    pub status: ExecutionBriefStatus,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CreateExecutionBriefPayload {
    pub project_id: String,
    pub content: ExecutionBriefContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpdateExecutionBriefPayload {
    pub brief_id: String,
    pub content: ExecutionBriefContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ChangeExecutionBriefStatusPayload {
    pub brief_id: String,
    pub status: ExecutionBriefStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum ExecutionBriefCommandEnvelope {
    #[serde(rename = "executionBrief.create", rename_all = "camelCase")]
    #[ts(rename = "executionBrief.create")]
    Create {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CreateExecutionBriefPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "executionBrief.update", rename_all = "camelCase")]
    #[ts(rename = "executionBrief.update")]
    Update {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: UpdateExecutionBriefPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "executionBrief.changeStatus", rename_all = "camelCase")]
    #[ts(rename = "executionBrief.changeStatus")]
    ChangeStatus {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: ChangeExecutionBriefStatusPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ExecutionBriefCommandResponse {
    pub receipt: CommandReceipt,
    pub execution_brief: ExecutionBriefRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum ExecutionBriefEventType {
    #[serde(rename = "executionBrief.created")]
    #[ts(rename = "executionBrief.created")]
    Created,
    #[serde(rename = "executionBrief.updated")]
    #[ts(rename = "executionBrief.updated")]
    Updated,
    #[serde(rename = "executionBrief.statusChanged")]
    #[ts(rename = "executionBrief.statusChanged")]
    StatusChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ExecutionBriefDomainEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_id: String,
    pub event_type: ExecutionBriefEventType,
    pub aggregate_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub trace_id: String,
    pub execution_brief: ExecutionBriefRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum RequirementBriefStatus {
    Interviewing,
    Review,
    Confirmed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum RequirementAnswerDisposition {
    Unanswered,
    Answered,
    FollowUp,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RequirementQuestionAnswer {
    pub question_id: String,
    pub prompt: String,
    pub required: bool,
    pub answer: String,
    pub disposition: RequirementAnswerDisposition,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RequirementAnswerInput {
    pub question_id: String,
    pub answer: String,
    pub disposition: RequirementAnswerDisposition,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RequirementBriefContent {
    pub objective: String,
    pub audience: String,
    pub key_message: String,
    pub deliverables: Vec<String>,
    pub channels: Vec<String>,
    pub style_keywords: Vec<String>,
    pub mandatory_items: Vec<String>,
    pub constraints: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub risks: Vec<String>,
    #[ts(type = "number | null")]
    pub deadline_at: Option<i64>,
    pub budget_notes: String,
    pub reference_case_ids: Vec<String>,
    pub reference_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RequirementBriefRecord {
    pub id: String,
    pub project_id: String,
    pub question_set_version: String,
    pub answers: Vec<RequirementQuestionAnswer>,
    pub content: RequirementBriefContent,
    pub status: RequirementBriefStatus,
    #[ts(type = "number | null")]
    pub confirmed_at: Option<i64>,
    pub confirmed_by: Option<String>,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CreateRequirementBriefPayload {
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpdateRequirementBriefPayload {
    pub brief_id: String,
    pub answers: Vec<RequirementAnswerInput>,
    pub content: RequirementBriefContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ChangeRequirementBriefStatusPayload {
    pub brief_id: String,
    pub status: RequirementBriefStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum RequirementBriefCommandEnvelope {
    #[serde(rename = "requirementBrief.create", rename_all = "camelCase")]
    #[ts(rename = "requirementBrief.create")]
    Create {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CreateRequirementBriefPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "requirementBrief.update", rename_all = "camelCase")]
    #[ts(rename = "requirementBrief.update")]
    Update {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: Box<UpdateRequirementBriefPayload>,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "requirementBrief.changeStatus", rename_all = "camelCase")]
    #[ts(rename = "requirementBrief.changeStatus")]
    ChangeStatus {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: ChangeRequirementBriefStatusPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RequirementBriefCommandResponse {
    pub receipt: CommandReceipt,
    pub requirement_brief: RequirementBriefRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum RequirementBriefEventType {
    #[serde(rename = "requirementBrief.created")]
    #[ts(rename = "requirementBrief.created")]
    Created,
    #[serde(rename = "requirementBrief.updated")]
    #[ts(rename = "requirementBrief.updated")]
    Updated,
    #[serde(rename = "requirementBrief.statusChanged")]
    #[ts(rename = "requirementBrief.statusChanged")]
    StatusChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RequirementBriefDomainEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_id: String,
    pub event_type: RequirementBriefEventType,
    pub aggregate_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub trace_id: String,
    pub requirement_brief: RequirementBriefRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessWorkspaceStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessLifecycleStage {
    #[default]
    Draft,
    Quoted,
    Contracted,
    PaymentRequested,
    Accepted,
    Paid,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessDocumentKind {
    Quote,
    Contract,
    PaymentRequest,
    Acceptance,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessDocumentStatus {
    Draft,
    InReview,
    Approved,
    Generated,
    Effective,
    Voided,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessDocumentFormat {
    Docx,
    Xlsx,
}

fn default_business_acceptance_output_format() -> BusinessDocumentFormat {
    BusinessDocumentFormat::Docx
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessPaymentStatus {
    Planned,
    Requested,
    PartiallyReceived,
    Received,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessEvidenceKind {
    QuoteConfirmation,
    ContractSignature,
    AcceptanceProof,
    ReceiptProof,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessEvidenceInput {
    pub asset_id: String,
    #[ts(type = "number | null")]
    pub occurred_at: Option<i64>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessEvidenceRecord {
    pub kind: BusinessEvidenceKind,
    pub asset_id: String,
    pub sha256: String,
    #[ts(type = "number | null")]
    pub occurred_at: Option<i64>,
    pub note: String,
    pub recorded_by: String,
    #[ts(type = "number")]
    pub recorded_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessManualWaiverInput {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessManualWaiverRecord {
    pub reason: String,
    pub approved_by: String,
    #[ts(type = "number")]
    pub approved_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessReceiptKind {
    Receipt,
    Reversal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessTaxMode {
    #[default]
    TaxExclusive,
    TaxInclusive,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessQuotationTotals {
    #[ts(type = "number")]
    pub original_total_cents: i64,
    #[ts(type = "number")]
    pub project_discount_cents: i64,
    #[ts(type = "number")]
    pub tax_exclusive_total_cents: i64,
    #[ts(type = "number")]
    pub tax_cents: i64,
    #[ts(type = "number")]
    pub final_total_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessLineItem {
    pub id: String,
    pub name: String,
    pub description: String,
    #[ts(type = "number")]
    pub quantity_millis: i64,
    pub unit: String,
    #[ts(type = "number")]
    pub unit_price_cents: i64,
    #[ts(type = "number")]
    pub tax_rate_bps: i64,
    #[ts(type = "number")]
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessLineItemInput {
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    #[ts(type = "number")]
    pub quantity_millis: i64,
    pub unit: String,
    #[ts(type = "number")]
    pub unit_price_cents: i64,
    #[ts(type = "number")]
    pub tax_rate_bps: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessProfile {
    pub project_title: String,
    pub project_code: String,
    pub customer_name: String,
    pub customer_legal_name: String,
    pub customer_tax_id: String,
    pub customer_address: String,
    pub customer_contact: String,
    pub customer_phone: String,
    pub customer_email: String,
    pub supplier_legal_name: String,
    pub supplier_tax_id: String,
    pub supplier_address: String,
    pub supplier_contact: String,
    pub supplier_phone: String,
    pub supplier_bank_name: String,
    pub supplier_bank_account: String,
    pub currency: String,
    #[ts(type = "number")]
    pub default_tax_rate_bps: i64,
    #[serde(default)]
    pub tax_mode: BusinessTaxMode,
    #[serde(default)]
    #[ts(type = "number")]
    pub project_discount_cents: i64,
    #[serde(default)]
    pub quotation_totals: Option<BusinessQuotationTotals>,
    #[ts(type = "number | null")]
    pub service_start_at: Option<i64>,
    #[ts(type = "number | null")]
    pub service_end_at: Option<i64>,
    pub delivery_summary: String,
    pub payment_terms: String,
    pub acceptance_terms: String,
    pub notes: String,
    pub line_items: Vec<BusinessLineItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessProfileInput {
    pub project_title: String,
    pub project_code: String,
    pub customer_name: String,
    pub customer_legal_name: String,
    pub customer_tax_id: String,
    pub customer_address: String,
    pub customer_contact: String,
    pub customer_phone: String,
    pub customer_email: String,
    pub supplier_legal_name: String,
    pub supplier_tax_id: String,
    pub supplier_address: String,
    pub supplier_contact: String,
    pub supplier_phone: String,
    pub supplier_bank_name: String,
    pub supplier_bank_account: String,
    pub currency: String,
    #[ts(type = "number")]
    pub default_tax_rate_bps: i64,
    #[serde(default)]
    pub tax_mode: BusinessTaxMode,
    #[serde(default)]
    #[ts(type = "number")]
    pub project_discount_cents: i64,
    #[ts(type = "number | null")]
    pub service_start_at: Option<i64>,
    #[ts(type = "number | null")]
    pub service_end_at: Option<i64>,
    pub delivery_summary: String,
    pub payment_terms: String,
    pub acceptance_terms: String,
    pub notes: String,
    pub line_items: Vec<BusinessLineItemInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessContractSettlementData {
    pub contract_title: String,
    pub contract_number: String,
    #[ts(type = "number")]
    pub original_contract_amount_cents: i64,
    #[ts(type = "number")]
    pub contract_adjustment_cents: i64,
    #[ts(type = "number | null")]
    pub retention_rate_bps: Option<u32>,
    #[ts(type = "number")]
    pub final_settlement_amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessServiceSettlementItemData {
    pub service_name: String,
    pub period: String,
    pub description: String,
    pub provided_as_required: Option<bool>,
    pub evidence_label: String,
    pub remarks: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessPaymentSettlementItemData {
    pub name: String,
    pub unit: String,
    #[ts(type = "number")]
    pub contract_unit_price_cents: i64,
    #[ts(type = "number")]
    pub original_quantity_millis: i64,
    #[ts(type = "number")]
    pub settlement_quantity_millis: i64,
    pub remarks: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessPaymentApplicationInput {
    pub payment_id: String,
    pub contract_title: String,
    pub contract_number: String,
    pub work_summary: String,
    pub payment_period_start: String,
    pub payment_period_end: String,
    pub settlement_period: String,
    #[ts(type = "number")]
    pub payment_sequence: u32,
    #[ts(type = "number")]
    pub invoice_amount_cents: i64,
    #[ts(type = "number")]
    pub cumulative_recognized_amount_cents: i64,
    #[ts(type = "number")]
    pub withheld_amount_cents: i64,
    pub application_date: String,
    pub supplier_bank_routing_number: String,
    pub settlement_items: Vec<BusinessPaymentSettlementItemData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessPaymentApplicationData {
    pub payment_id: String,
    pub contract_title: String,
    pub contract_number: String,
    pub work_summary: String,
    pub payment_period_start: String,
    pub payment_period_end: String,
    pub settlement_period: String,
    #[ts(type = "number")]
    pub payment_sequence: u32,
    #[ts(type = "number")]
    pub invoice_amount_cents: i64,
    #[ts(type = "number")]
    pub cumulative_recognized_amount_cents: i64,
    #[ts(type = "number")]
    pub withheld_amount_cents: i64,
    #[ts(type = "number")]
    pub cumulative_paid_cents: i64,
    #[ts(type = "number")]
    pub settlement_total_cents: i64,
    #[ts(type = "number")]
    pub remaining_payable_cents: i64,
    pub application_date: String,
    pub bank_account_profile_version: String,
    pub supplier_bank_routing_number: String,
    pub settlement_items: Vec<BusinessPaymentSettlementItemData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessDocumentSnapshot {
    #[ts(type = "number")]
    pub workspace_revision: i64,
    #[serde(default)]
    pub acceptance_batch_id: Option<String>,
    #[serde(default)]
    pub acceptance_output_spec_id: Option<String>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub acceptance_batch_revision: Option<i64>,
    #[serde(default)]
    pub material_bindings: Vec<BusinessAcceptanceMaterialBinding>,
    #[serde(default)]
    pub template_asset_id: Option<String>,
    #[serde(default)]
    pub template_source_sha256: Option<String>,
    #[serde(default)]
    pub template_mapping_version: String,
    #[serde(default)]
    pub contract_settlement: Option<BusinessContractSettlementData>,
    #[serde(default)]
    pub service_settlement_items: Vec<BusinessServiceSettlementItemData>,
    #[serde(default)]
    pub payment_application: Option<BusinessPaymentApplicationData>,
    #[serde(default)]
    #[ts(optional)]
    pub video_completion_acceptance: Option<BusinessVideoCompletionAcceptanceData>,
    #[serde(default)]
    #[ts(optional)]
    pub production_result_confirmation: Option<BusinessProductionResultConfirmationData>,
    #[serde(default)]
    pub customer_id: String,
    #[serde(default)]
    pub customer: BusinessCustomerRecord,
    pub profile: BusinessProfile,
    pub payment: Option<BusinessPaymentRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessDocumentRecord {
    pub id: String,
    pub kind: BusinessDocumentKind,
    #[ts(type = "number")]
    pub sequence_number: i64,
    pub document_number: String,
    pub title: String,
    pub template_key: String,
    pub status: BusinessDocumentStatus,
    pub snapshot: BusinessDocumentSnapshot,
    pub output_asset_id: Option<String>,
    pub output_format: Option<BusinessDocumentFormat>,
    #[serde(default)]
    pub source_asset_id: Option<String>,
    #[serde(default)]
    pub review_id: Option<String>,
    #[serde(default)]
    pub report_asset_id: Option<String>,
    #[serde(default)]
    pub evidence: Option<BusinessEvidenceRecord>,
    #[serde(default)]
    pub manual_waiver: Option<BusinessManualWaiverRecord>,
    #[ts(type = "number | null")]
    #[serde(default)]
    pub voided_at: Option<i64>,
    #[serde(default)]
    pub voided_by: Option<String>,
    #[serde(default)]
    pub void_reason: String,
    #[ts(type = "number | null")]
    pub approved_at: Option<i64>,
    pub approved_by: Option<String>,
    #[ts(type = "number | null")]
    pub generated_at: Option<i64>,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessPaymentRecord {
    pub id: String,
    pub label: String,
    #[ts(type = "number")]
    pub amount_cents: i64,
    #[ts(type = "number | null")]
    pub due_at: Option<i64>,
    #[ts(type = "number | null")]
    pub occurred_at: Option<i64>,
    pub status: BusinessPaymentStatus,
    pub reference: String,
    pub notes: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessPaymentInput {
    pub id: Option<String>,
    pub label: String,
    #[ts(type = "number")]
    pub amount_cents: i64,
    #[ts(type = "number | null")]
    pub due_at: Option<i64>,
    #[ts(type = "number | null")]
    pub occurred_at: Option<i64>,
    pub status: BusinessPaymentStatus,
    pub reference: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessQuoteConfirmationRecord {
    pub id: String,
    pub quote_document_id: String,
    #[ts(type = "number")]
    pub quote_document_revision: i64,
    pub quote_asset_id: String,
    pub quote_sha256: String,
    pub confirmation_version: String,
    pub customer_representative: String,
    pub evidence: BusinessEvidenceRecord,
    pub notes: String,
    pub confirmed_by: String,
    #[ts(type = "number")]
    pub confirmed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessReceiptRecord {
    pub id: String,
    pub payment_id: String,
    pub kind: BusinessReceiptKind,
    #[ts(type = "number")]
    pub amount_cents: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub reference: String,
    pub notes: String,
    pub reverses_receipt_id: Option<String>,
    pub evidence: Option<BusinessEvidenceRecord>,
    pub recorded_by: String,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessCustomerStatus {
    #[default]
    Active,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessCustomerRecord {
    pub id: String,
    pub display_name: String,
    pub legal_name: String,
    pub tax_id: String,
    pub billing_address: String,
    pub primary_contact_name: String,
    pub primary_phone: String,
    pub primary_email: String,
    pub notes: String,
    pub status: BusinessCustomerStatus,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
    pub archived_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessCustomerInput {
    pub display_name: String,
    pub legal_name: String,
    pub tax_id: String,
    pub billing_address: String,
    pub primary_contact_name: String,
    pub primary_phone: String,
    pub primary_email: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessArtifactRef {
    pub role: String,
    pub asset_id: String,
    pub sha256: String,
    #[ts(type = "number")]
    pub size_bytes: i64,
    pub original_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessMilestoneStatus {
    #[default]
    Planned,
    InProgress,
    Delivered,
    Accepted,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessDeliverableVersionStatus {
    #[default]
    Draft,
    Sent,
    Accepted,
    Rejected,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessDeliverySubmissionStatus {
    #[default]
    Sent,
    PartiallySigned,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessDeliverableVersionRecord {
    pub id: String,
    pub deliverable_id: String,
    pub milestone_id: String,
    pub name: String,
    pub required: bool,
    #[ts(type = "number")]
    pub version_number: i64,
    pub artifact: BusinessArtifactRef,
    pub status: BusinessDeliverableVersionStatus,
    pub notes: String,
    pub created_by: String,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessDeliverableRecord {
    pub id: String,
    pub milestone_id: String,
    pub name: String,
    pub required: bool,
    pub versions: Vec<BusinessDeliverableVersionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessMilestoneRecord {
    pub id: String,
    #[ts(type = "number")]
    pub sequence_number: i64,
    pub title: String,
    pub description: String,
    #[ts(type = "number | null")]
    pub due_at: Option<i64>,
    pub acceptance_criteria: String,
    pub required: bool,
    pub status: BusinessMilestoneStatus,
    pub deliverables: Vec<BusinessDeliverableRecord>,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessAcceptanceMaterialKind {
    Script,
    Video,
    Screenshot,
    BehindTheScenes,
    PublishingData,
    Invoice,
    Proof,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessAcceptanceBatchStatus {
    Collecting,
    DocumentsPrepared,
    Approved,
    Generated,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessTemplateVersionStatus {
    PendingReview,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessTemplateVersionRecord {
    pub id: String,
    pub workspace_id: String,
    pub source_asset_id: String,
    pub source_sha256: String,
    pub normalized_asset_id: String,
    pub normalized_sha256: String,
    pub template_key: String,
    pub mapping_version: String,
    pub converter_engine: String,
    pub converter_version: String,
    pub converter_policy_version: String,
    pub status: BusinessTemplateVersionStatus,
    pub reviewed_by: Option<String>,
    #[ts(type = "number | null")]
    pub reviewed_at: Option<i64>,
    pub review_note: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessAcceptanceRequirementRecord {
    pub id: String,
    pub label: String,
    pub kind: BusinessAcceptanceMaterialKind,
    #[ts(type = "number")]
    pub required_group_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessVideoCompletionAcceptanceAssetReference {
    pub asset_id: String,
    pub file_name: String,
    pub sha256: String,
    #[serde(default)]
    #[ts(optional)]
    pub external_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessVideoCompletionAcceptanceScreenshot {
    pub asset_id: String,
    pub sha256: String,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessVideoCompletionAcceptanceVideo {
    pub title: String,
    pub video_type: String,
    pub content: String,
    pub duration: String,
    pub asset_reference: BusinessVideoCompletionAcceptanceAssetReference,
    pub screenshots: Vec<BusinessVideoCompletionAcceptanceScreenshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessVideoCompletionAcceptanceDeliveryGroup {
    pub group_key: String,
    pub name: String,
    pub service_description: String,
    pub videos: Vec<BusinessVideoCompletionAcceptanceVideo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessVideoCompletionAcceptanceData {
    pub contract_title: String,
    pub project_title: String,
    pub completion_date: String,
    pub delivery_groups: Vec<BusinessVideoCompletionAcceptanceDeliveryGroup>,
    pub acceptance_conclusion: String,
    pub manually_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessProductionResultConfirmationAssetReference {
    pub asset_id: String,
    pub sha256: String,
    pub group_key: String,
    pub file_name: String,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessProductionResultConfirmationShot {
    pub shot_number: String,
    pub shot_description: String,
    pub images: Vec<BusinessProductionResultConfirmationAssetReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessProductionResultConfirmationStoryboard {
    pub storyboard_number: String,
    pub title: String,
    pub description: String,
    pub shots: Vec<BusinessProductionResultConfirmationShot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessProductionResultConfirmationDeliveryItem {
    pub item_key: String,
    pub title: String,
    pub deliverable_summary: String,
    pub evidence_images: Vec<BusinessProductionResultConfirmationAssetReference>,
    pub storyboards: Vec<BusinessProductionResultConfirmationStoryboard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessProductionResultConfirmationData {
    pub attachment_label: String,
    pub contract_title: String,
    pub project_title: String,
    pub category: String,
    #[ts(type = "number")]
    pub payment_amount_cents: i64,
    pub contract_deliverable_summary: String,
    pub supplier_legal_name: String,
    pub procurement_period: String,
    pub delivery_items: Vec<BusinessProductionResultConfirmationDeliveryItem>,
    pub acceptance_description: String,
    pub penalty_or_addition: String,
    pub completion_date: String,
    pub acceptance_date: String,
    pub clean_highlights_confirmed: bool,
    pub manually_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessAcceptanceOutputSpecRecord {
    pub id: String,
    #[serde(default)]
    pub output_code: String,
    pub document_number: String,
    pub title: String,
    pub template_key: String,
    #[serde(default)]
    pub template_asset_id: Option<String>,
    #[serde(default)]
    pub template_source_sha256: Option<String>,
    #[serde(default)]
    pub template_mapping_version: String,
    #[serde(default)]
    pub contract_settlement: Option<BusinessContractSettlementData>,
    #[serde(default)]
    pub service_settlement_items: Vec<BusinessServiceSettlementItemData>,
    #[serde(default)]
    pub payment_application: Option<BusinessPaymentApplicationData>,
    #[serde(default)]
    #[ts(optional)]
    pub video_completion_acceptance: Option<BusinessVideoCompletionAcceptanceData>,
    #[serde(default)]
    #[ts(optional)]
    pub production_result_confirmation: Option<BusinessProductionResultConfirmationData>,
    #[serde(default = "default_business_acceptance_output_format")]
    pub format: BusinessDocumentFormat,
    #[serde(default)]
    pub requirement_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessAcceptanceMaterialBinding {
    pub requirement_id: String,
    pub asset_id: String,
    pub sha256: String,
    pub group_key: String,
    pub kind: BusinessAcceptanceMaterialKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessAcceptanceMaterialRecord {
    pub id: String,
    pub batch_id: String,
    pub requirement_id: String,
    pub asset_id: String,
    pub kind: BusinessAcceptanceMaterialKind,
    pub group_key: String,
    pub confirmed: bool,
    pub duplicate_of_material_id: Option<String>,
    pub notes: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessAcceptanceBlocker {
    pub code: String,
    pub requirement_id: String,
    pub requirement_label: String,
    #[ts(type = "number")]
    pub required_group_count: u32,
    #[ts(type = "number")]
    pub provided_group_count: u32,
    #[ts(type = "number")]
    pub missing_group_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessAcceptanceReadiness {
    pub is_ready: bool,
    pub blockers: Vec<BusinessAcceptanceBlocker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessAcceptanceBatchRecord {
    pub id: String,
    pub workspace_id: String,
    pub label: String,
    pub requirements: Vec<BusinessAcceptanceRequirementRecord>,
    pub output_specs: Vec<BusinessAcceptanceOutputSpecRecord>,
    pub materials: Vec<BusinessAcceptanceMaterialRecord>,
    pub readiness: BusinessAcceptanceReadiness,
    pub document_ids: Vec<String>,
    pub status: BusinessAcceptanceBatchStatus,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessDeliverySignoffRecord {
    pub id: String,
    pub submission_id: String,
    pub accepted_version_ids: Vec<String>,
    pub rejected_version_ids: Vec<String>,
    pub customer_representative: String,
    pub evidence: Option<BusinessEvidenceRecord>,
    pub note: String,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub recorded_by: String,
    #[ts(type = "number")]
    pub recorded_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessSettlementCadence {
    Monthly,
    Quarterly,
    PerOrder,
    #[default]
    OneOff,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessSettlementBatchStatus {
    #[default]
    Draft,
    Confirmed,
    Voided,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessSettlementLineInput {
    pub deliverable_id: String,
    #[ts(type = "number")]
    pub contract_quantity_millis: i64,
    #[ts(type = "number")]
    pub cumulative_executed_millis: i64,
    #[ts(type = "number")]
    pub current_executed_millis: i64,
    #[ts(type = "number")]
    pub cumulative_accepted_millis: i64,
    #[ts(type = "number")]
    pub current_accepted_millis: i64,
    #[ts(type = "number")]
    pub current_settlement_millis: i64,
    pub unit: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessSettlementLineRecord {
    pub deliverable_id: String,
    pub milestone_id: String,
    pub deliverable_name: String,
    #[ts(type = "number")]
    pub contract_quantity_millis: i64,
    #[ts(type = "number")]
    pub cumulative_executed_millis: i64,
    #[ts(type = "number")]
    pub current_executed_millis: i64,
    #[ts(type = "number")]
    pub cumulative_accepted_millis: i64,
    #[ts(type = "number")]
    pub current_accepted_millis: i64,
    #[ts(type = "number")]
    pub cumulative_settled_millis: i64,
    #[ts(type = "number")]
    pub current_settlement_millis: i64,
    #[ts(type = "number")]
    pub remaining_quantity_millis: i64,
    pub unit: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessSettlementBatchInput {
    pub id: Option<String>,
    pub contract_number: String,
    pub settlement_period: String,
    pub cadence: BusinessSettlementCadence,
    pub status: BusinessSettlementBatchStatus,
    pub lines: Vec<BusinessSettlementLineInput>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessSettlementBatchRecord {
    pub id: String,
    pub workspace_id: String,
    pub contract_number: String,
    pub settlement_period: String,
    pub cadence: BusinessSettlementCadence,
    pub status: BusinessSettlementBatchStatus,
    pub lines: Vec<BusinessSettlementLineRecord>,
    pub notes: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    #[ts(type = "number | null")]
    pub voided_at: Option<i64>,
    pub voided_by: Option<String>,
    pub void_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessDeliverySubmissionRecord {
    pub id: String,
    pub milestone_id: String,
    #[ts(type = "number")]
    pub submission_number: i64,
    pub version_ids: Vec<String>,
    pub recipient: String,
    pub channel: String,
    pub note: String,
    #[ts(type = "number")]
    pub sent_at: i64,
    pub sent_by: String,
    pub status: BusinessDeliverySubmissionStatus,
    pub signoffs: Vec<BusinessDeliverySignoffRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessInvoiceKind {
    Issued,
    Reversal,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessInvoiceStatus {
    Issued,
    PartiallyReversed,
    FullyReversed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessInvoiceRecord {
    pub id: String,
    pub payment_id: Option<String>,
    pub kind: BusinessInvoiceKind,
    pub status: BusinessInvoiceStatus,
    pub invoice_code: String,
    pub invoice_number: String,
    pub issuer_tax_id: String,
    pub buyer_tax_id: String,
    pub currency: String,
    #[ts(type = "number")]
    pub amount_cents: i64,
    #[ts(type = "number")]
    pub tax_cents: i64,
    #[ts(type = "number")]
    pub issued_at: i64,
    pub original_invoice_id: Option<String>,
    pub reversal_reason: String,
    pub artifacts: Vec<BusinessArtifactRef>,
    pub recorded_by: String,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessArchiveIntegrityStatus {
    #[default]
    NotCaptured,
    Ready,
    Stale,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessArchiveEntryRecord {
    pub logical_path: String,
    pub role: String,
    pub source_entity_type: String,
    pub source_entity_id: String,
    pub artifact: BusinessArtifactRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessArchiveSnapshotRecord {
    pub id: String,
    #[ts(type = "number")]
    pub captured_workspace_revision: i64,
    #[ts(type = "number")]
    pub captured_customer_revision: i64,
    pub manifest_sha256: String,
    pub manifest_asset_id: Option<String>,
    pub package_asset_id: Option<String>,
    pub entries: Vec<BusinessArchiveEntryRecord>,
    pub generated_by: String,
    #[ts(type = "number")]
    pub generated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessFinancialSummary {
    #[ts(type = "number")]
    pub quoted_cents: i64,
    #[ts(type = "number")]
    pub contract_cents: i64,
    #[ts(type = "number")]
    pub scheduled_cents: i64,
    #[ts(type = "number")]
    pub requested_cents: i64,
    #[ts(type = "number")]
    pub received_cents: i64,
    #[ts(type = "number")]
    pub outstanding_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessCurrentDocuments {
    pub quote_document_id: Option<String>,
    pub contract_document_id: Option<String>,
    pub payment_request_document_id: Option<String>,
    pub acceptance_document_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessWorkspaceRecord {
    pub id: String,
    pub project_id: String,
    #[serde(default)]
    pub customer_id: String,
    #[serde(default)]
    pub customer: BusinessCustomerRecord,
    pub requirement_brief_id: Option<String>,
    #[ts(type = "number | null")]
    #[serde(default)]
    pub requirement_brief_revision: Option<i64>,
    pub prefill_source_workspace_id: Option<String>,
    pub profile: BusinessProfile,
    pub documents: Vec<BusinessDocumentRecord>,
    #[serde(default)]
    pub template_versions: Vec<BusinessTemplateVersionRecord>,
    pub payments: Vec<BusinessPaymentRecord>,
    #[serde(default)]
    pub quote_confirmations: Vec<BusinessQuoteConfirmationRecord>,
    #[serde(default)]
    pub receipts: Vec<BusinessReceiptRecord>,
    #[serde(default)]
    pub milestones: Vec<BusinessMilestoneRecord>,
    #[serde(default)]
    pub settlement_batches: Vec<BusinessSettlementBatchRecord>,
    #[serde(default)]
    pub acceptance_batches: Vec<BusinessAcceptanceBatchRecord>,
    #[serde(default)]
    pub delivery_submissions: Vec<BusinessDeliverySubmissionRecord>,
    #[serde(default)]
    pub invoices: Vec<BusinessInvoiceRecord>,
    #[serde(default)]
    pub archive_snapshots: Vec<BusinessArchiveSnapshotRecord>,
    #[serde(default)]
    pub archive_integrity_status: BusinessArchiveIntegrityStatus,
    pub status: BusinessWorkspaceStatus,
    #[ts(type = "number | null")]
    #[serde(default)]
    pub archived_at: Option<i64>,
    #[serde(default)]
    pub archived_by: Option<String>,
    #[serde(default)]
    pub lifecycle_stage: BusinessLifecycleStage,
    #[serde(default)]
    pub financial_summary: BusinessFinancialSummary,
    #[serde(default)]
    pub current_documents: BusinessCurrentDocuments,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessWorkspacePrefillField {
    CustomerLegalName,
    CustomerTaxId,
    CustomerAddress,
    CustomerContact,
    CustomerPhone,
    CustomerEmail,
    SupplierLegalName,
    SupplierTaxId,
    SupplierAddress,
    SupplierContact,
    SupplierPhone,
    SupplierBankName,
    SupplierBankAccount,
    Currency,
    DefaultTaxRateBps,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessWorkspacePrefillMatchKind {
    CustomerName,
    CustomerLegalName,
    Both,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BusinessWorkspacePrefillDecision {
    Unchanged,
    Filled,
    Replaced,
    Cleared,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessWorkspacePrefillCandidate {
    pub source_workspace_id: String,
    pub source_project_id: String,
    pub source_project_title: String,
    pub customer_name: String,
    pub customer_legal_name: String,
    pub supplier_legal_name: String,
    pub match_kind: BusinessWorkspacePrefillMatchKind,
    pub populated_fields: Vec<BusinessWorkspacePrefillField>,
    pub status: BusinessWorkspaceStatus,
    #[ts(type = "number")]
    pub source_revision: i64,
    #[ts(type = "number")]
    pub source_updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ListBusinessWorkspacePrefillCandidatesRequest {
    pub target_project_id: String,
    #[ts(type = "number | null")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct PreviewBusinessWorkspacePrefillRequest {
    pub target_project_id: String,
    pub source_workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessWorkspacePrefillChange {
    pub field: BusinessWorkspacePrefillField,
    pub target_value: String,
    pub source_value: String,
    pub result_value: String,
    pub decision: BusinessWorkspacePrefillDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessWorkspacePrefillPreview {
    pub target_project_id: String,
    pub target_project_title: String,
    pub target_customer_name: String,
    pub target_requirement_brief_id: Option<String>,
    pub source_workspace_id: String,
    pub source_project_id: String,
    pub source_project_title: String,
    pub match_kind: BusinessWorkspacePrefillMatchKind,
    #[ts(type = "number")]
    pub source_revision: i64,
    #[ts(type = "number")]
    pub source_updated_at: i64,
    pub changes: Vec<BusinessWorkspacePrefillChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessRequirementSyncPreview {
    pub workspace_id: String,
    pub current_requirement_brief_id: Option<String>,
    #[ts(type = "number | null")]
    pub current_requirement_brief_revision: Option<i64>,
    pub latest_requirement_brief_id: String,
    #[ts(type = "number")]
    pub latest_requirement_brief_revision: i64,
    pub has_changes: bool,
    pub current_profile: BusinessProfile,
    pub proposed_profile: BusinessProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ListBusinessCustomersRequest {
    pub query: String,
    #[ts(type = "number | null")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessCustomerReceivableSummary {
    #[serde(default)]
    pub customer_id: String,
    pub customer_key: String,
    pub customer_name: String,
    pub customer_legal_name: String,
    pub customer_tax_id: String,
    pub customer_contact: String,
    pub customer_phone: String,
    pub customer_email: String,
    #[serde(default)]
    pub customer_status: BusinessCustomerStatus,
    #[ts(type = "number")]
    #[serde(default)]
    pub customer_revision: i64,
    #[ts(type = "number")]
    pub workspace_count: i64,
    #[ts(type = "number")]
    pub active_workspace_count: i64,
    #[ts(type = "number")]
    pub contract_cents: i64,
    #[ts(type = "number")]
    pub requested_cents: i64,
    #[ts(type = "number")]
    pub received_cents: i64,
    #[ts(type = "number")]
    pub outstanding_cents: i64,
    pub workspace_ids: Vec<String>,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CreateBusinessWorkspacePayload {
    pub project_id: String,
    #[serde(default)]
    pub customer_id: Option<String>,
    pub prefill_source_workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpdateBusinessProfilePayload {
    pub workspace_id: String,
    pub profile: BusinessProfileInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CreateBusinessDocumentPayload {
    pub workspace_id: String,
    pub kind: BusinessDocumentKind,
    pub document_number: String,
    pub title: String,
    pub template_key: String,
    pub payment_id: Option<String>,
    #[serde(default)]
    pub acceptance_batch_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct PromoteReviewedContractPayload {
    pub workspace_id: String,
    pub review_id: String,
    pub report_asset_id: String,
    pub document_number: String,
    pub title: String,
    #[serde(default)]
    pub evidence: Option<BusinessEvidenceInput>,
    #[serde(default)]
    pub manual_waiver: Option<BusinessManualWaiverInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ChangeBusinessDocumentStatusPayload {
    pub workspace_id: String,
    pub document_id: String,
    pub status: BusinessDocumentStatus,
    #[serde(default)]
    pub evidence: Option<BusinessEvidenceInput>,
    #[serde(default)]
    pub manual_waiver: Option<BusinessManualWaiverInput>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct GenerateBusinessDocumentPayload {
    pub workspace_id: String,
    pub document_id: String,
    pub format: BusinessDocumentFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpsertBusinessPaymentPayload {
    pub workspace_id: String,
    pub payment: BusinessPaymentInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpsertBusinessSettlementBatchPayload {
    pub workspace_id: String,
    pub batch: BusinessSettlementBatchInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct VoidBusinessSettlementBatchPayload {
    pub workspace_id: String,
    pub batch_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ChangeBusinessWorkspaceStatusPayload {
    pub workspace_id: String,
    pub status: BusinessWorkspaceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpsertBusinessCustomerPayload {
    pub workspace_id: String,
    pub customer_id: Option<String>,
    pub customer: BusinessCustomerInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AssignBusinessCustomerPayload {
    pub workspace_id: String,
    pub customer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessMilestoneInput {
    pub id: Option<String>,
    pub title: String,
    pub description: String,
    #[ts(type = "number | null")]
    pub due_at: Option<i64>,
    pub acceptance_criteria: String,
    pub required: bool,
    pub status: BusinessMilestoneStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpsertBusinessMilestonePayload {
    pub workspace_id: String,
    pub milestone: BusinessMilestoneInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessAcceptanceRequirementInput {
    pub id: Option<String>,
    pub label: String,
    pub kind: BusinessAcceptanceMaterialKind,
    #[ts(type = "number")]
    pub required_group_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessAcceptanceOutputSpecInput {
    pub id: Option<String>,
    pub output_code: String,
    pub document_number: String,
    pub title: String,
    pub template_key: String,
    #[serde(default)]
    pub template_asset_id: Option<String>,
    #[serde(default)]
    pub template_source_sha256: Option<String>,
    #[serde(default)]
    pub template_mapping_version: String,
    #[serde(default)]
    pub contract_settlement: Option<BusinessContractSettlementData>,
    #[serde(default)]
    pub service_settlement_items: Vec<BusinessServiceSettlementItemData>,
    #[serde(default)]
    pub payment_application: Option<BusinessPaymentApplicationInput>,
    #[serde(default)]
    #[ts(optional)]
    pub video_completion_acceptance: Option<BusinessVideoCompletionAcceptanceData>,
    #[serde(default)]
    #[ts(optional)]
    pub production_result_confirmation: Option<BusinessProductionResultConfirmationData>,
    pub format: BusinessDocumentFormat,
    pub requirement_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessAcceptanceMaterialInput {
    pub id: Option<String>,
    pub requirement_id: String,
    pub asset_id: String,
    pub kind: BusinessAcceptanceMaterialKind,
    pub group_key: String,
    pub confirmed: bool,
    pub duplicate_of_material_id: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CreateBusinessAcceptanceBatchPayload {
    pub workspace_id: String,
    pub label: String,
    pub requirements: Vec<BusinessAcceptanceRequirementInput>,
    pub output_specs: Vec<BusinessAcceptanceOutputSpecInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpsertBusinessAcceptanceMaterialPayload {
    pub workspace_id: String,
    pub batch_id: String,
    pub material: BusinessAcceptanceMaterialInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct PrepareBusinessAcceptanceDocumentsPayload {
    pub workspace_id: String,
    pub batch_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RegisterBusinessDeliverableVersionPayload {
    pub workspace_id: String,
    pub milestone_id: String,
    pub deliverable_id: Option<String>,
    pub name: String,
    pub required: bool,
    pub asset_id: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RecordBusinessDeliverySentPayload {
    pub workspace_id: String,
    pub milestone_id: String,
    pub version_ids: Vec<String>,
    pub recipient: String,
    pub channel: String,
    #[ts(type = "number")]
    pub sent_at: i64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RecordBusinessDeliverySignoffPayload {
    pub workspace_id: String,
    pub submission_id: String,
    pub accepted_version_ids: Vec<String>,
    pub rejected_version_ids: Vec<String>,
    pub customer_representative: String,
    pub evidence: Option<BusinessEvidenceInput>,
    pub note: String,
    #[ts(type = "number")]
    pub occurred_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RecordBusinessInvoiceIssuedPayload {
    pub workspace_id: String,
    pub payment_id: Option<String>,
    pub invoice_code: String,
    pub invoice_number: String,
    #[ts(type = "number")]
    pub amount_cents: i64,
    #[ts(type = "number")]
    pub tax_cents: i64,
    #[ts(type = "number")]
    pub issued_at: i64,
    pub asset_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RecordBusinessInvoiceRedCorrectionPayload {
    pub workspace_id: String,
    pub original_invoice_id: String,
    pub invoice_code: String,
    pub invoice_number: String,
    #[ts(type = "number")]
    pub amount_cents: i64,
    #[ts(type = "number")]
    pub tax_cents: i64,
    #[ts(type = "number")]
    pub issued_at: i64,
    pub reason: String,
    pub asset_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AttachBusinessInvoiceAssetPayload {
    pub workspace_id: String,
    pub invoice_id: String,
    pub asset_id: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CreateBusinessArchiveSnapshotPayload {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ConfirmBusinessQuotePayload {
    pub workspace_id: String,
    pub quote_document_id: String,
    pub confirmation_version: String,
    pub customer_representative: String,
    pub evidence: BusinessEvidenceInput,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RecordBusinessReceiptPayload {
    pub workspace_id: String,
    pub payment_id: String,
    #[ts(type = "number")]
    pub amount_cents: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub reference: String,
    pub notes: String,
    pub evidence: Option<BusinessEvidenceInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ReverseBusinessReceiptPayload {
    pub workspace_id: String,
    pub receipt_id: String,
    #[ts(type = "number")]
    pub amount_cents: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub reference: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AdoptLatestConfirmedRequirementPayload {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct NormalizeBusinessLegacyTemplatePayload {
    pub workspace_id: String,
    pub source_asset_id: String,
    pub expected_source_sha256: String,
    pub template_key: String,
    pub mapping_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ApproveBusinessTemplateVersionPayload {
    pub workspace_id: String,
    pub template_version_id: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RejectBusinessTemplateVersionPayload {
    pub workspace_id: String,
    pub template_version_id: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum BusinessWorkspaceCommandEnvelope {
    #[serde(rename = "businessWorkspace.create", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.create")]
    Create {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CreateBusinessWorkspacePayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "businessWorkspace.updateProfile", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.updateProfile")]
    UpdateProfile {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: Box<UpdateBusinessProfilePayload>,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "businessWorkspace.createDocument", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.createDocument")]
    CreateDocument {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CreateBusinessDocumentPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.promoteReviewedContract",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.promoteReviewedContract")]
    PromoteReviewedContract {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: PromoteReviewedContractPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.changeDocumentStatus",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.changeDocumentStatus")]
    ChangeDocumentStatus {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: ChangeBusinessDocumentStatusPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.generateDocument",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.generateDocument")]
    GenerateDocument {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: GenerateBusinessDocumentPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "businessWorkspace.upsertPayment", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.upsertPayment")]
    UpsertPayment {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: UpsertBusinessPaymentPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },

    #[serde(
        rename = "businessWorkspace.upsertSettlementBatch",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.upsertSettlementBatch")]
    UpsertSettlementBatch {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: UpsertBusinessSettlementBatchPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.voidSettlementBatch",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.voidSettlementBatch")]
    VoidSettlementBatch {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: VoidBusinessSettlementBatchPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "businessWorkspace.confirmQuote", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.confirmQuote")]
    ConfirmQuote {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: ConfirmBusinessQuotePayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "businessWorkspace.recordReceipt", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.recordReceipt")]
    RecordReceipt {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RecordBusinessReceiptPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "businessWorkspace.reverseReceipt", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.reverseReceipt")]
    ReverseReceipt {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: ReverseBusinessReceiptPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.adoptLatestConfirmedRequirement",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.adoptLatestConfirmedRequirement")]
    AdoptLatestConfirmedRequirement {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: AdoptLatestConfirmedRequirementPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "businessWorkspace.upsertCustomer", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.upsertCustomer")]
    UpsertCustomer {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: UpsertBusinessCustomerPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "businessWorkspace.assignCustomer", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.assignCustomer")]
    AssignCustomer {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: AssignBusinessCustomerPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "businessWorkspace.upsertMilestone", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.upsertMilestone")]
    UpsertMilestone {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: UpsertBusinessMilestonePayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.createAcceptanceBatch",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.createAcceptanceBatch")]
    CreateAcceptanceBatch {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CreateBusinessAcceptanceBatchPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.prepareAcceptanceDocuments",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.prepareAcceptanceDocuments")]
    PrepareAcceptanceDocuments {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: PrepareBusinessAcceptanceDocumentsPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.upsertAcceptanceMaterial",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.upsertAcceptanceMaterial")]
    UpsertAcceptanceMaterial {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: UpsertBusinessAcceptanceMaterialPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.registerDeliverableVersion",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.registerDeliverableVersion")]
    RegisterDeliverableVersion {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RegisterBusinessDeliverableVersionPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.recordDeliverySent",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.recordDeliverySent")]
    RecordDeliverySent {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RecordBusinessDeliverySentPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.recordDeliverySignoff",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.recordDeliverySignoff")]
    RecordDeliverySignoff {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RecordBusinessDeliverySignoffPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.recordInvoiceIssued",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.recordInvoiceIssued")]
    RecordInvoiceIssued {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RecordBusinessInvoiceIssuedPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.recordInvoiceRedCorrection",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.recordInvoiceRedCorrection")]
    RecordInvoiceRedCorrection {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RecordBusinessInvoiceRedCorrectionPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.attachInvoiceAsset",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.attachInvoiceAsset")]
    AttachInvoiceAsset {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: AttachBusinessInvoiceAssetPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.createArchiveSnapshot",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.createArchiveSnapshot")]
    CreateArchiveSnapshot {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CreateBusinessArchiveSnapshotPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.normalizeLegacyTemplate",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.normalizeLegacyTemplate")]
    NormalizeLegacyTemplate {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: NormalizeBusinessLegacyTemplatePayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.approveTemplateVersion",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.approveTemplateVersion")]
    ApproveTemplateVersion {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: ApproveBusinessTemplateVersionPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(
        rename = "businessWorkspace.rejectTemplateVersion",
        rename_all = "camelCase"
    )]
    #[ts(rename = "businessWorkspace.rejectTemplateVersion")]
    RejectTemplateVersion {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RejectBusinessTemplateVersionPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "businessWorkspace.changeStatus", rename_all = "camelCase")]
    #[ts(rename = "businessWorkspace.changeStatus")]
    ChangeStatus {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: ChangeBusinessWorkspaceStatusPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessWorkspaceCommandResponse {
    pub receipt: CommandReceipt,
    pub business_workspace: BusinessWorkspaceRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum BusinessWorkspaceEventType {
    #[serde(rename = "businessWorkspace.created")]
    #[ts(rename = "businessWorkspace.created")]
    Created,
    #[serde(rename = "businessWorkspace.profileUpdated")]
    #[ts(rename = "businessWorkspace.profileUpdated")]
    ProfileUpdated,
    #[serde(rename = "businessWorkspace.documentCreated")]
    #[ts(rename = "businessWorkspace.documentCreated")]
    DocumentCreated,
    #[serde(rename = "businessWorkspace.reviewedContractPromoted")]
    #[ts(rename = "businessWorkspace.reviewedContractPromoted")]
    ReviewedContractPromoted,
    #[serde(rename = "businessWorkspace.documentStatusChanged")]
    #[ts(rename = "businessWorkspace.documentStatusChanged")]
    DocumentStatusChanged,
    #[serde(rename = "businessWorkspace.documentGenerated")]
    #[ts(rename = "businessWorkspace.documentGenerated")]
    DocumentGenerated,
    #[serde(rename = "businessWorkspace.paymentUpserted")]
    #[ts(rename = "businessWorkspace.paymentUpserted")]
    PaymentUpserted,
    #[serde(rename = "businessWorkspace.settlementBatchUpserted")]
    #[ts(rename = "businessWorkspace.settlementBatchUpserted")]
    SettlementBatchUpserted,
    #[serde(rename = "businessWorkspace.settlementBatchVoided")]
    #[ts(rename = "businessWorkspace.settlementBatchVoided")]
    SettlementBatchVoided,
    #[serde(rename = "businessWorkspace.quoteConfirmed")]
    #[ts(rename = "businessWorkspace.quoteConfirmed")]
    QuoteConfirmed,
    #[serde(rename = "businessWorkspace.receiptRecorded")]
    #[ts(rename = "businessWorkspace.receiptRecorded")]
    ReceiptRecorded,
    #[serde(rename = "businessWorkspace.receiptReversed")]
    #[ts(rename = "businessWorkspace.receiptReversed")]
    ReceiptReversed,
    #[serde(rename = "businessWorkspace.requirementAdopted")]
    #[ts(rename = "businessWorkspace.requirementAdopted")]
    RequirementAdopted,
    #[serde(rename = "businessWorkspace.customerUpserted")]
    #[ts(rename = "businessWorkspace.customerUpserted")]
    CustomerUpserted,
    #[serde(rename = "businessWorkspace.customerAssigned")]
    #[ts(rename = "businessWorkspace.customerAssigned")]
    CustomerAssigned,
    #[serde(rename = "businessWorkspace.milestoneUpserted")]
    #[ts(rename = "businessWorkspace.milestoneUpserted")]
    MilestoneUpserted,
    #[serde(rename = "businessWorkspace.acceptanceBatchCreated")]
    #[ts(rename = "businessWorkspace.acceptanceBatchCreated")]
    AcceptanceBatchCreated,
    #[serde(rename = "businessWorkspace.acceptanceDocumentsPrepared")]
    #[ts(rename = "businessWorkspace.acceptanceDocumentsPrepared")]
    AcceptanceDocumentsPrepared,
    #[serde(rename = "businessWorkspace.acceptanceMaterialUpserted")]
    #[ts(rename = "businessWorkspace.acceptanceMaterialUpserted")]
    AcceptanceMaterialUpserted,
    #[serde(rename = "businessWorkspace.deliverableVersionRegistered")]
    #[ts(rename = "businessWorkspace.deliverableVersionRegistered")]
    DeliverableVersionRegistered,
    #[serde(rename = "businessWorkspace.deliverySent")]
    #[ts(rename = "businessWorkspace.deliverySent")]
    DeliverySent,
    #[serde(rename = "businessWorkspace.deliverySignoffRecorded")]
    #[ts(rename = "businessWorkspace.deliverySignoffRecorded")]
    DeliverySignoffRecorded,
    #[serde(rename = "businessWorkspace.invoiceIssued")]
    #[ts(rename = "businessWorkspace.invoiceIssued")]
    InvoiceIssued,
    #[serde(rename = "businessWorkspace.invoiceRedCorrected")]
    #[ts(rename = "businessWorkspace.invoiceRedCorrected")]
    InvoiceRedCorrected,
    #[serde(rename = "businessWorkspace.invoiceAssetAttached")]
    #[ts(rename = "businessWorkspace.invoiceAssetAttached")]
    InvoiceAssetAttached,
    #[serde(rename = "businessWorkspace.archiveSnapshotPrepared")]
    #[ts(rename = "businessWorkspace.archiveSnapshotPrepared")]
    ArchiveSnapshotPrepared,
    #[serde(rename = "businessWorkspace.templateVersionNormalized")]
    #[ts(rename = "businessWorkspace.templateVersionNormalized")]
    TemplateVersionNormalized,
    #[serde(rename = "businessWorkspace.templateVersionApproved")]
    #[ts(rename = "businessWorkspace.templateVersionApproved")]
    TemplateVersionApproved,
    #[serde(rename = "businessWorkspace.templateVersionRejected")]
    #[ts(rename = "businessWorkspace.templateVersionRejected")]
    TemplateVersionRejected,
    #[serde(rename = "businessWorkspace.statusChanged")]
    #[ts(rename = "businessWorkspace.statusChanged")]
    StatusChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BusinessWorkspaceDomainEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_id: String,
    pub event_type: BusinessWorkspaceEventType,
    pub aggregate_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub trace_id: String,
    #[serde(default)]
    pub actor_id: String,
    #[serde(default)]
    pub command_id: String,
    #[serde(default)]
    pub reason: String,
    pub business_workspace: BusinessWorkspaceRecord,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ContractReviewStatus {
    Draft,
    Running,
    AwaitingConfirmation,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ContractReviewStage {
    Created,
    Extracting,
    AwaitingOcr,
    ReviewingRules,
    ReviewingAgent,
    MergingFindings,
    AwaitingConfirmation,
    GeneratingReport,
    Completed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum DocumentExtractionStatus {
    Pending,
    Running,
    AwaitingOcr,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ParserProvenance {
    pub name: String,
    pub version: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct OcrProvenance {
    pub engine: String,
    pub version: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct EvidenceBoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum DocumentBlockKind {
    Paragraph,
    Heading,
    ListItem,
    Table,
    Header,
    Footer,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DocumentPageRecord {
    pub id: String,
    pub extraction_id: String,
    #[ts(type = "number")]
    pub page_index: i64,
    pub text: String,
    pub text_sha256: String,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub preview_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DocumentBlockRecord {
    pub id: String,
    pub extraction_id: String,
    pub page_id: String,
    #[ts(type = "number")]
    pub page_index: i64,
    #[ts(type = "number")]
    pub order_index: i64,
    pub kind: DocumentBlockKind,
    pub text: String,
    #[ts(type = "number")]
    pub char_start: i64,
    #[ts(type = "number")]
    pub char_end: i64,
    pub bbox: Option<EvidenceBoundingBox>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DocumentTableRecord {
    pub id: String,
    pub extraction_id: String,
    pub page_id: String,
    #[ts(type = "number")]
    pub page_index: i64,
    #[ts(type = "number")]
    pub order_index: i64,
    pub markdown: String,
    pub data: serde_json::Value,
    pub bbox: Option<EvidenceBoundingBox>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ContractReviewFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub stage: ContractReviewStage,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DocumentExtractionRecord {
    pub id: String,
    pub review_id: String,
    pub source_asset_id: String,
    pub source_asset_sha256: String,
    pub parser: ParserProvenance,
    pub ocr: Option<OcrProvenance>,
    pub status: DocumentExtractionStatus,
    #[ts(type = "number")]
    pub page_count: i64,
    pub content_sha256: Option<String>,
    pub snapshot_asset_id: Option<String>,
    pub pages: Vec<DocumentPageRecord>,
    pub blocks: Vec<DocumentBlockRecord>,
    pub tables: Vec<DocumentTableRecord>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub completed_at: Option<i64>,
    pub failure: Option<ContractReviewFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct EvidenceAnchor {
    pub id: String,
    pub extraction_id: String,
    pub source_asset_id: String,
    #[ts(type = "number")]
    pub page_index: i64,
    pub block_id: Option<String>,
    #[ts(type = "number | null")]
    pub char_start: Option<i64>,
    #[ts(type = "number | null")]
    pub char_end: Option<i64>,
    pub bbox: Option<EvidenceBoundingBox>,
    pub quoted_text: String,
    pub quoted_text_sha256: String,
    pub context_before: String,
    pub context_after: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ReviewFindingSource {
    Rule,
    Agent,
    Merged,
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ReviewSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ReviewFindingStatus {
    Open,
    Decided,
    Superseded,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ReviewFindingDecision {
    Unreviewed,
    Confirmed,
    Rejected,
    AcceptedRisk,
    NeedsRevision,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ReviewFindingRecord {
    pub id: String,
    pub review_id: String,
    pub source: ReviewFindingSource,
    pub rule_id: Option<String>,
    pub rule_version: Option<String>,
    pub agent_run_id: Option<String>,
    pub category: String,
    pub severity: ReviewSeverity,
    pub title: String,
    pub description: String,
    pub recommendation: String,
    pub evidence_ids: Vec<String>,
    pub missing_evidence_reason: Option<String>,
    pub status: ReviewFindingStatus,
    pub decision: ReviewFindingDecision,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum RuleEvaluationStatus {
    Passed,
    Finding,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RuleEvaluationRecord {
    pub id: String,
    pub review_id: String,
    pub rule_id: String,
    pub rule_version: String,
    pub status: RuleEvaluationStatus,
    pub finding_ids: Vec<String>,
    pub details: String,
    #[ts(type = "number")]
    pub evaluated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct FindingDecisionRecord {
    pub id: String,
    pub review_id: String,
    pub finding_id: String,
    pub decision: ReviewFindingDecision,
    pub comment: String,
    pub actor_id: String,
    #[ts(type = "number")]
    pub finding_revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ReviewReportFormat {
    Json,
    Html,
    Docx,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ReviewReportRecord {
    pub id: String,
    pub review_id: String,
    #[ts(type = "number")]
    pub review_revision: i64,
    pub source_asset_id: String,
    pub source_asset_sha256: String,
    pub extraction_id: String,
    pub rule_set_version: String,
    pub agent_run_ids: Vec<String>,
    pub format: ReviewReportFormat,
    pub report_asset_id: String,
    pub report_asset_sha256: String,
    #[ts(type = "number")]
    pub generated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ContractReviewSessionRecord {
    pub id: String,
    pub workspace_id: String,
    pub source_asset_id: String,
    pub source_asset_sha256: String,
    pub source_file_name: String,
    pub status: ContractReviewStatus,
    pub stage: ContractReviewStage,
    pub extraction_id: Option<String>,
    pub report_asset_id: Option<String>,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    #[ts(type = "number | null")]
    pub completed_at: Option<i64>,
    pub failure: Option<ContractReviewFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ContractReviewRecord {
    pub session: ContractReviewSessionRecord,
    pub extraction: Option<DocumentExtractionRecord>,
    pub evidence: Vec<EvidenceAnchor>,
    pub findings: Vec<ReviewFindingRecord>,
    pub rule_evaluations: Vec<RuleEvaluationRecord>,
    pub decisions: Vec<FindingDecisionRecord>,
    pub reports: Vec<ReviewReportRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CreateContractReviewPayload {
    pub workspace_id: String,
    pub source_asset_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct StartContractReviewPayload {
    pub review_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CancelContractReviewPayload {
    pub review_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DecideReviewFindingPayload {
    pub review_id: String,
    pub finding_id: String,
    pub decision: ReviewFindingDecision,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct GenerateReviewReportPayload {
    pub review_id: String,
    pub format: ReviewReportFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RetryContractReviewStagePayload {
    pub review_id: String,
    pub stage: ContractReviewStage,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum ContractReviewCommandEnvelope {
    #[serde(rename = "contractReview.create", rename_all = "camelCase")]
    #[ts(rename = "contractReview.create")]
    Create {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CreateContractReviewPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "contractReview.start", rename_all = "camelCase")]
    #[ts(rename = "contractReview.start")]
    Start {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: StartContractReviewPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "contractReview.cancel", rename_all = "camelCase")]
    #[ts(rename = "contractReview.cancel")]
    Cancel {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CancelContractReviewPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "contractReview.decideFinding", rename_all = "camelCase")]
    #[ts(rename = "contractReview.decideFinding")]
    DecideFinding {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: DecideReviewFindingPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "contractReview.generateReport", rename_all = "camelCase")]
    #[ts(rename = "contractReview.generateReport")]
    GenerateReport {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: GenerateReviewReportPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "contractReview.retryStage", rename_all = "camelCase")]
    #[ts(rename = "contractReview.retryStage")]
    RetryStage {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RetryContractReviewStagePayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ContractReviewCommandResponse {
    pub receipt: CommandReceipt,
    pub contract_review: ContractReviewRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ListContractReviewsRequest {
    pub workspace_id: Option<String>,
    pub status: Option<ContractReviewStatus>,
    #[ts(type = "number | null")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct GetContractReviewRequest {
    pub review_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ListReviewFindingsRequest {
    pub review_id: String,
    pub status: Option<ReviewFindingStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct GetEvidenceContextRequest {
    pub evidence_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct EvidenceContext {
    pub evidence: EvidenceAnchor,
    pub page: DocumentPageRecord,
    pub block: Option<DocumentBlockRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum ContractReviewEventType {
    #[serde(rename = "contractReview.created")]
    #[ts(rename = "contractReview.created")]
    Created,
    #[serde(rename = "contractReview.started")]
    #[ts(rename = "contractReview.started")]
    Started,
    #[serde(rename = "contractReview.stageChanged")]
    #[ts(rename = "contractReview.stageChanged")]
    StageChanged,
    #[serde(rename = "contractReview.extractionCompleted")]
    #[ts(rename = "contractReview.extractionCompleted")]
    ExtractionCompleted,
    #[serde(rename = "contractReview.ocrRequired")]
    #[ts(rename = "contractReview.ocrRequired")]
    OcrRequired,
    #[serde(rename = "contractReview.findingAdded")]
    #[ts(rename = "contractReview.findingAdded")]
    FindingAdded,
    #[serde(rename = "contractReview.findingUpdated")]
    #[ts(rename = "contractReview.findingUpdated")]
    FindingUpdated,
    #[serde(rename = "contractReview.findingDecided")]
    #[ts(rename = "contractReview.findingDecided")]
    FindingDecided,
    #[serde(rename = "contractReview.reportGenerated")]
    #[ts(rename = "contractReview.reportGenerated")]
    ReportGenerated,
    #[serde(rename = "contractReview.completed")]
    #[ts(rename = "contractReview.completed")]
    Completed,
    #[serde(rename = "contractReview.failed")]
    #[ts(rename = "contractReview.failed")]
    Failed,
    #[serde(rename = "contractReview.cancelled")]
    #[ts(rename = "contractReview.cancelled")]
    Cancelled,
}

impl ContractReviewEventType {
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Created => "contractReview.created",
            Self::Started => "contractReview.started",
            Self::StageChanged => "contractReview.stageChanged",
            Self::ExtractionCompleted => "contractReview.extractionCompleted",
            Self::OcrRequired => "contractReview.ocrRequired",
            Self::FindingAdded => "contractReview.findingAdded",
            Self::FindingUpdated => "contractReview.findingUpdated",
            Self::FindingDecided => "contractReview.findingDecided",
            Self::ReportGenerated => "contractReview.reportGenerated",
            Self::Completed => "contractReview.completed",
            Self::Failed => "contractReview.failed",
            Self::Cancelled => "contractReview.cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ContractReviewDomainEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_id: String,
    pub event_type: ContractReviewEventType,
    pub aggregate_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub trace_id: String,
    pub contract_review: ContractReviewRecord,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BackupState {
    NotScheduled,
    Queued,
    Uploading,
    BackedUp,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AssetBackupRecord {
    pub asset_id: String,
    pub content_sha256: String,
    pub state: BackupState,
    #[ts(type = "number")]
    pub attempt_count: i64,
    #[ts(type = "number | null")]
    pub next_attempt_at: Option<i64>,
    pub last_error: Option<String>,
    pub remote_object_key: Option<String>,
    pub remote_etag: Option<String>,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    #[ts(type = "number | null")]
    pub backed_up_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct QueueAssetBackupPayload {
    pub asset_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RetryAssetBackupPayload {
    pub asset_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CancelAssetBackupPayload {
    pub asset_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RestoreAssetBackupPayload {
    pub asset_id: String,
    pub expected_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum BackupCommandEnvelope {
    #[serde(rename = "backup.queue", rename_all = "camelCase")]
    #[ts(rename = "backup.queue")]
    Queue {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: QueueAssetBackupPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "backup.retry", rename_all = "camelCase")]
    #[ts(rename = "backup.retry")]
    Retry {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RetryAssetBackupPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "backup.cancel", rename_all = "camelCase")]
    #[ts(rename = "backup.cancel")]
    Cancel {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CancelAssetBackupPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "backup.restore", rename_all = "camelCase")]
    #[ts(rename = "backup.restore")]
    Restore {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: RestoreAssetBackupPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BackupCommandResponse {
    pub receipt: CommandReceipt,
    pub backup: AssetBackupRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum AiCredentialProtection {
    WindowsDpapiCurrentUser,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum AiProviderKind {
    OpenAiCompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum AiProviderConnectionState {
    Untested,
    Ready,
    Warning,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AiProviderConnectionStatus {
    pub state: AiProviderConnectionState,
    pub message: String,
    #[ts(type = "number | null")]
    pub latency_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub tested_at: Option<i64>,
    pub discovered_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AiProviderRecord {
    pub id: String,
    pub name: String,
    pub kind: AiProviderKind,
    pub base_url: String,
    pub api_key_configured: bool,
    pub api_key_hint: Option<String>,
    pub models: Vec<String>,
    pub default_model: String,
    pub is_default: bool,
    pub enabled: bool,
    pub connection: AiProviderConnectionStatus,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AiCredentialStatus {
    /// Compatibility summary of the currently selected provider.
    pub provider: String,
    pub configured: bool,
    pub persisted: bool,
    pub protection: Option<AiCredentialProtection>,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number | null")]
    pub updated_at: Option<i64>,
    pub applies_on_next_runtime_start: bool,
    pub default_provider_id: Option<String>,
    pub default_model: Option<String>,
    pub providers: Vec<AiProviderRecord>,
}

#[derive(Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct SaveBsaigcProviderApiKeyPayload {
    pub api_key: String,
}

impl std::fmt::Debug for SaveBsaigcProviderApiKeyPayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SaveBsaigcProviderApiKeyPayload")
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl Serialize for SaveBsaigcProviderApiKeyPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("SaveBsaigcProviderApiKeyPayload", 1)?;
        state.serialize_field("apiKey", "[REDACTED]")?;
        state.end()
    }
}

#[derive(Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct UpsertAiProviderPayload {
    pub provider_id: Option<String>,
    pub name: String,
    pub kind: AiProviderKind,
    pub base_url: String,
    pub api_key: Option<String>,
    pub models: Vec<String>,
    pub default_model: String,
    pub set_default: bool,
    pub enabled: bool,
}

impl std::fmt::Debug for UpsertAiProviderPayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UpsertAiProviderPayload")
            .field("provider_id", &self.provider_id)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("models", &self.models)
            .field("default_model", &self.default_model)
            .field("set_default", &self.set_default)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl Serialize for UpsertAiProviderPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("UpsertAiProviderPayload", 9)?;
        state.serialize_field("providerId", &self.provider_id)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("baseUrl", &self.base_url)?;
        state.serialize_field("apiKey", &self.api_key.as_ref().map(|_| "[REDACTED]"))?;
        state.serialize_field("models", &self.models)?;
        state.serialize_field("defaultModel", &self.default_model)?;
        state.serialize_field("setDefault", &self.set_default)?;
        state.serialize_field("enabled", &self.enabled)?;
        state.end()
    }
}

#[derive(Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DiscoverAiProviderModelsPayload {
    pub provider_id: Option<String>,
    pub kind: AiProviderKind,
    pub base_url: String,
    pub api_key: Option<String>,
}

impl std::fmt::Debug for DiscoverAiProviderModelsPayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DiscoverAiProviderModelsPayload")
            .field("provider_id", &self.provider_id)
            .field("kind", &self.kind)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl Serialize for DiscoverAiProviderModelsPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("DiscoverAiProviderModelsPayload", 4)?;
        state.serialize_field("providerId", &self.provider_id)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("baseUrl", &self.base_url)?;
        state.serialize_field("apiKey", &self.api_key.as_ref().map(|_| "[REDACTED]"))?;
        state.end()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AiProviderIdPayload {
    pub provider_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct SelectAiProviderPayload {
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum AiCredentialCommandEnvelope {
    #[serde(rename = "aiCredentials.status", rename_all = "camelCase")]
    #[ts(rename = "aiCredentials.status")]
    Status {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "aiCredentials.upsertProvider", rename_all = "camelCase")]
    #[ts(rename = "aiCredentials.upsertProvider")]
    UpsertProvider {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: UpsertAiProviderPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "aiCredentials.removeProvider", rename_all = "camelCase")]
    #[ts(rename = "aiCredentials.removeProvider")]
    RemoveProvider {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: AiProviderIdPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "aiCredentials.selectProvider", rename_all = "camelCase")]
    #[ts(rename = "aiCredentials.selectProvider")]
    SelectProvider {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: SelectAiProviderPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "aiCredentials.testProvider", rename_all = "camelCase")]
    #[ts(rename = "aiCredentials.testProvider")]
    TestProvider {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: AiProviderIdPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "aiCredentials.discoverModels", rename_all = "camelCase")]
    #[ts(rename = "aiCredentials.discoverModels")]
    DiscoverModels {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: DiscoverAiProviderModelsPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "aiCredentials.clearProviderApiKey", rename_all = "camelCase")]
    #[ts(rename = "aiCredentials.clearProviderApiKey")]
    ClearProviderApiKey {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: AiProviderIdPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "aiCredentials.saveBsaigcApiKey", rename_all = "camelCase")]
    #[ts(rename = "aiCredentials.saveBsaigcApiKey")]
    SaveBsaigcApiKey {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: SaveBsaigcProviderApiKeyPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "aiCredentials.clearBsaigcApiKey", rename_all = "camelCase")]
    #[ts(rename = "aiCredentials.clearBsaigcApiKey")]
    ClearBsaigcApiKey {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AiCredentialCommandResponse {
    pub receipt: CommandReceipt,
    pub status: AiCredentialStatus,
    pub connection_test: Option<AiProviderConnectionStatus>,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum DesktopBuildChannel {
    Stable,
    Development,
    InternalPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum StorageLocationTarget {
    DataRoot,
    Ledger,
    Vault,
    Cache,
    Staging,
    Credentials,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct StorageLocationStatus {
    pub target: StorageLocationTarget,
    pub label: String,
    pub path: String,
    #[ts(type = "number")]
    pub size_bytes: i64,
    pub exists: bool,
    pub authoritative: bool,
    pub clearable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct StorageSettingsStatus {
    pub data_root: String,
    #[ts(type = "number")]
    pub total_bytes: i64,
    #[ts(type = "number")]
    pub cache_bytes: i64,
    pub locations: Vec<StorageLocationStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ChannelAdapterState {
    Planned,
    Available,
    Configured,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ChannelAdapterStatus {
    pub id: String,
    pub name: String,
    pub state: ChannelAdapterState,
    pub configured: bool,
    pub capabilities: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum CloudBackupMode {
    AsyncBackupOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CloudBackupStatus {
    pub provider: String,
    pub mode: CloudBackupMode,
    pub configured: bool,
    pub ready: bool,
    pub state: String,
    pub message: String,
    #[ts(type = "number")]
    pub pending_items: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DesktopUpdateStatus {
    pub current_version: String,
    pub build_channel: DesktopBuildChannel,
    pub build_version: String,
    pub codex_runtime_version: String,
    pub update_source_configured: bool,
    pub automatic_install_allowed: bool,
    pub state: String,
    pub message: String,
    #[serde(default)]
    pub latest_version: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[ts(type = "number | null")]
    pub last_checked_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DesktopSettingsSnapshot {
    pub storage: StorageSettingsStatus,
    pub channel_adapters: Vec<ChannelAdapterStatus>,
    pub cloud_backup: CloudBackupStatus,
    pub update: DesktopUpdateStatus,
    #[ts(type = "number")]
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct OpenStorageLocationPayload {
    pub target: StorageLocationTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CacheClearResult {
    #[ts(type = "number")]
    pub freed_bytes: i64,
    pub cleared_locations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum DesktopSettingsCommandEnvelope {
    #[serde(rename = "settings.status", rename_all = "camelCase")]
    #[ts(rename = "settings.status")]
    Status {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "settings.openStorageLocation", rename_all = "camelCase")]
    #[ts(rename = "settings.openStorageLocation")]
    OpenStorageLocation {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: OpenStorageLocationPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "settings.clearCache", rename_all = "camelCase")]
    #[ts(rename = "settings.clearCache")]
    ClearCache {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "settings.checkForUpdates", rename_all = "camelCase")]
    #[ts(rename = "settings.checkForUpdates")]
    CheckForUpdates {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DesktopSettingsCommandResponse {
    pub receipt: CommandReceipt,
    pub snapshot: DesktopSettingsSnapshot,
    pub cache_clear: Option<CacheClearResult>,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum BackupEventType {
    #[serde(rename = "backup.queued")]
    #[ts(rename = "backup.queued")]
    Queued,
    #[serde(rename = "backup.uploading")]
    #[ts(rename = "backup.uploading")]
    Uploading,
    #[serde(rename = "backup.backedUp")]
    #[ts(rename = "backup.backedUp")]
    BackedUp,
    #[serde(rename = "backup.failed")]
    #[ts(rename = "backup.failed")]
    Failed,
    #[serde(rename = "backup.cancelled")]
    #[ts(rename = "backup.cancelled")]
    Cancelled,
    #[serde(rename = "backup.restored")]
    #[ts(rename = "backup.restored")]
    Restored,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BackupDomainEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_id: String,
    pub event_type: BackupEventType,
    pub asset_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub trace_id: String,
    pub backup: AssetBackupRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BrainThreadStatus {
    Ready,
    Running,
    Error,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BrainThreadRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub title: Option<String>,
    pub model: Option<String>,
    pub status: BrainThreadStatus,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum BrainTurnStatus {
    Running,
    Completed,
    Interrupted,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BrainTurnRecord {
    pub id: String,
    pub thread_id: String,
    pub status: BrainTurnStatus,
    pub input_text: String,
    pub assistant_text: String,
    pub error: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BrainStreamEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_type: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub delta: Option<String>,
    #[ts(type = "unknown | null")]
    pub payload: Option<serde_json::Value>,
    #[ts(type = "number")]
    pub occurred_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct StartBrainThreadRequest {
    pub project_id: Option<String>,
    pub title: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ResumeBrainThreadRequest {
    pub thread_id: String,
    pub project_id: Option<String>,
    pub title: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ListRemoteBrainThreadsRequest {
    pub cursor: Option<String>,
    #[ts(type = "number | null")]
    pub limit: Option<u32>,
    pub archived: Option<bool>,
    pub search_term: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct StartBrainTurnRequest {
    pub thread_id: String,
    pub input_text: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum BrainAccessMode {
    #[default]
    RequestApproval,
    AutoApprove,
    FullAccess,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BrainTurnContext {
    pub workspace_token: Option<String>,
    pub access_mode: BrainAccessMode,
    #[ts(optional)]
    pub web_enabled: Option<bool>,
    pub attachment_asset_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct InterruptBrainTurnRequest {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct RemoteBrainThreadPage {
    pub threads: Vec<BrainThreadRecord>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BrainTurnStartResult {
    pub turn: BrainTurnRecord,
    pub remote_turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct BrainHostHealth {
    pub state: String,
    pub running: bool,
    pub initialized: bool,
    #[ts(type = "number")]
    pub pending_requests: usize,
    #[ts(type = "number")]
    pub subscribers: usize,
    #[ts(type = "number | null")]
    pub started_at: Option<i64>,
    #[ts(type = "number | null")]
    pub last_message_at: Option<i64>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum MemoryScope {
    Global,
    Project,
    Thread,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct MemoryRecord {
    pub id: String,
    pub scope: MemoryScope,
    pub scope_id: Option<String>,
    pub memory_type: String,
    pub content: String,
    #[ts(type = "unknown")]
    pub metadata: serde_json::Value,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ModuleAvailability {
    Ready,
    Degraded,
    Planned,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ModuleManifest {
    pub id: String,
    pub version: String,
    pub availability: ModuleAvailability,
    pub commands: Vec<String>,
    pub events: Vec<String>,
    pub permissions: Vec<String>,
    pub tools: Vec<String>,
    pub storage: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Consumed,
    Denied,
    Expired,
}

impl ApprovalStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Consumed => "consumed",
            Self::Denied => "denied",
            Self::Expired => "expired",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "pending" => Self::Pending,
            "approved" => Self::Approved,
            "consumed" => Self::Consumed,
            "denied" => Self::Denied,
            "expired" => Self::Expired,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ApprovalRecord {
    pub id: String,
    pub operation: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub actor_id: String,
    pub status: ApprovalStatus,
    pub reason: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub expires_at: i64,
    #[ts(type = "number | null")]
    pub resolved_at: Option<i64>,
    pub resolved_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct PermissionDecision {
    pub allowed: bool,
    pub approval_required: bool,
    pub approval_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ResolveApprovalPayload {
    pub approval_id: String,
    pub approved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl DiagnosticSeverity {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "info" => Self::Info,
            "warning" => Self::Warning,
            "error" => Self::Error,
            "critical" => Self::Critical,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum DiagnosticStatus {
    Queued,
    Uploaded,
    Suppressed,
}

impl DiagnosticStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Uploaded => "uploaded",
            Self::Suppressed => "suppressed",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "queued" => Self::Queued,
            "uploaded" => Self::Uploaded,
            "suppressed" => Self::Suppressed,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DiagnosticReportPayload {
    pub code: String,
    pub message: String,
    pub component: String,
    pub severity: DiagnosticSeverity,
    pub trace_id: Option<String>,
    pub project_id: Option<String>,
    #[ts(type = "unknown")]
    pub context: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DiagnosticRecord {
    pub id: String,
    pub fingerprint: String,
    pub code: String,
    pub message: String,
    pub component: String,
    pub severity: DiagnosticSeverity,
    pub status: DiagnosticStatus,
    pub trace_id: Option<String>,
    pub project_id: Option<String>,
    #[ts(type = "unknown")]
    pub context: serde_json::Value,
    pub occurrences: u32,
    #[ts(type = "number")]
    pub first_seen_at: i64,
    #[ts(type = "number")]
    pub last_seen_at: i64,
    #[ts(type = "number | null")]
    pub uploaded_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "commandType")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), tag = "commandType", rename_all_fields = "camelCase")]
pub enum CommandEnvelope {
    #[serde(rename = "project.create", rename_all = "camelCase")]
    #[ts(rename = "project.create")]
    ProjectCreate {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: CreateProjectPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "project.updateBrief", rename_all = "camelCase")]
    #[ts(rename = "project.updateBrief")]
    ProjectUpdateBrief {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: UpdateProjectBriefPayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
    #[serde(rename = "project.changeStage", rename_all = "camelCase")]
    #[ts(rename = "project.changeStage")]
    ProjectChangeStage {
        command_id: String,
        protocol_version: String,
        context: OperationContext,
        payload: ChangeProjectStagePayload,
        idempotency_key: String,
        #[ts(type = "number | null")]
        expected_revision: Option<i64>,
        #[ts(type = "number | null")]
        deadline_at: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CommandReceipt {
    pub command_id: String,
    pub idempotency_key: String,
    pub command_type: String,
    pub aggregate_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub last_event_sequence: i64,
    #[ts(type = "number")]
    pub completed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CommandResponse {
    pub receipt: CommandReceipt,
    pub project: ProjectRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"))]
pub enum ProjectEventType {
    #[serde(rename = "project.created")]
    #[ts(rename = "project.created")]
    ProjectCreated,
    #[serde(rename = "project.briefUpdated")]
    #[ts(rename = "project.briefUpdated")]
    ProjectBriefUpdated,
    #[serde(rename = "project.stageChanged")]
    #[ts(rename = "project.stageChanged")]
    ProjectStageChanged,
}

impl ProjectEventType {
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::ProjectCreated => "project.created",
            Self::ProjectBriefUpdated => "project.briefUpdated",
            Self::ProjectStageChanged => "project.stageChanged",
        }
    }

    pub fn from_wire_str(value: &str) -> Option<Self> {
        Some(match value {
            "project.created" => Self::ProjectCreated,
            "project.briefUpdated" => Self::ProjectBriefUpdated,
            "project.stageChanged" => Self::ProjectStageChanged,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct DomainEvent {
    #[ts(type = "number")]
    pub sequence: i64,
    pub event_id: String,
    pub event_type: ProjectEventType,
    pub aggregate_type: String,
    pub aggregate_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub trace_id: String,
    pub project: ProjectRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct ReplayEventsRequest {
    #[ts(type = "number")]
    pub after_sequence: i64,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct HostStatus {
    pub protocol_version: String,
    pub database_ready: bool,
    pub vault_ready: bool,
    #[ts(type = "number")]
    pub project_count: i64,
    #[ts(type = "number")]
    pub task_count: i64,
    #[ts(type = "number")]
    pub asset_count: i64,
    #[ts(type = "number")]
    pub last_event_sequence: i64,
    pub runtime: String,
    pub modules: Vec<ModuleManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct NativeMediaHealth {
    pub state: String,
    pub ffmpeg_available: bool,
    pub ffprobe_available: bool,
    pub ffmpeg_source: Option<String>,
    pub ffprobe_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct CodexProbeStatus {
    pub available: bool,
    pub runtime: String,
    pub transport: String,
    pub user_agent: Option<String>,
    pub platform_family: Option<String>,
    pub platform_os: Option<String>,
    pub codex_home_ready: bool,
    pub source: Option<String>,
    #[ts(type = "number | null")]
    pub handshake_at: Option<i64>,
    pub error: Option<String>,
}

impl CodexProbeStatus {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            available: false,
            runtime: "official-codex-app-server".to_string(),
            transport: "stdio/jsonl".to_string(),
            user_agent: None,
            platform_family: None,
            platform_os: None,
            codex_home_ready: false,
            source: None,
            handshake_at: None,
            error: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct HostError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

// === Local login & shared account registry (1.3) ===

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum AppUserRole {
    Admin,
    Member,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum AppUserStatus {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AppUserRecord {
    pub username: String,
    pub role: AppUserRole,
    pub status: AppUserStatus,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub enum AuthRegistrySync {
    LocalOnly,
    Synced,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AuthStatus {
    pub initialized: bool,
    pub current_user: Option<AppUserRecord>,
    pub registry_sync: AuthRegistrySync,
    pub registry_message: Option<String>,
    #[ts(type = "number")]
    pub registry_revision: i64,
    pub user_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AuthCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AuthCreateUserPayload {
    pub username: String,
    pub password: String,
    pub role: AppUserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AuthResetPasswordPayload {
    pub username: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AuthDeleteUserPayload {
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AuthChangePasswordPayload {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bsaigc/"), rename_all = "camelCase")]
pub struct AuthUsersSnapshot {
    pub users: Vec<AppUserRecord>,
    pub registry_sync: AuthRegistrySync,
    pub registry_message: Option<String>,
    #[ts(type = "number")]
    pub registry_revision: i64,
}

impl HostError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new("VALIDATION_FAILED", message, false)
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new("REVISION_CONFLICT", message, false)
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("HOST_INTERNAL", message, true)
    }
}

impl std::fmt::Display for HostError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for HostError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brain_turn_context_defaults_web_access_to_disabled() {
        let context: BrainTurnContext = serde_json::from_value(serde_json::json!({
            "workspaceToken": null,
            "accessMode": "requestApproval",
            "attachmentAssetIds": []
        }))
        .expect("deserialize legacy brain turn context");

        assert!(!context.web_enabled.unwrap_or(false));
    }

    #[test]
    fn legacy_acceptance_output_specs_and_document_snapshots_default_new_fields() {
        let output_spec: BusinessAcceptanceOutputSpecRecord =
            serde_json::from_value(serde_json::json!({
                "id": "legacy-output",
                "documentNumber": "ACC-1",
                "title": "Legacy acceptance",
                "templateKey": "business-acceptance-v1"
            }))
            .unwrap();
        assert!(output_spec.output_code.is_empty());
        assert_eq!(output_spec.format, BusinessDocumentFormat::Docx);
        assert!(output_spec.requirement_ids.is_empty());
        assert_eq!(output_spec.template_asset_id, None);
        assert_eq!(output_spec.template_source_sha256, None);
        assert!(output_spec.template_mapping_version.is_empty());
        assert_eq!(output_spec.contract_settlement, None);
        assert!(output_spec.service_settlement_items.is_empty());
        assert_eq!(output_spec.video_completion_acceptance, None);
        assert_eq!(output_spec.production_result_confirmation, None);

        let output_spec_input: BusinessAcceptanceOutputSpecInput =
            serde_json::from_value(serde_json::json!({
                "id": null,
                "outputCode": "legacy-output",
                "documentNumber": "ACC-1",
                "title": "Legacy acceptance",
                "templateKey": "business-acceptance-v1",
                "format": "docx",
                "requirementIds": []
            }))
            .unwrap();
        assert_eq!(output_spec_input.video_completion_acceptance, None);
        assert_eq!(output_spec_input.production_result_confirmation, None);

        let snapshot: BusinessDocumentSnapshot = serde_json::from_value(serde_json::json!({
            "workspaceRevision": 1,
            "profile": BusinessProfile::default(),
            "payment": null
        }))
        .unwrap();
        assert_eq!(snapshot.acceptance_output_spec_id, None);
        assert_eq!(snapshot.acceptance_batch_revision, None);
        assert!(snapshot.material_bindings.is_empty());
        assert_eq!(snapshot.template_asset_id, None);
        assert_eq!(snapshot.template_source_sha256, None);
        assert!(snapshot.template_mapping_version.is_empty());
        assert_eq!(snapshot.contract_settlement, None);
        assert!(snapshot.service_settlement_items.is_empty());
        assert_eq!(snapshot.video_completion_acceptance, None);
        assert_eq!(snapshot.production_result_confirmation, None);
    }

    #[test]
    fn video_completion_acceptance_contract_round_trips_with_camel_case_fields() {
        let contract = BusinessVideoCompletionAcceptanceData {
            contract_title: "年度视频制作服务合同".to_owned(),
            project_title: "白鹅潭项目".to_owned(),
            completion_date: "2026-07-29".to_owned(),
            delivery_groups: vec![BusinessVideoCompletionAcceptanceDeliveryGroup {
                group_key: "delivery-group-1".to_owned(),
                name: "第一批交付".to_owned(),
                service_description: "视频策划、剪辑与成片交付".to_owned(),
                videos: vec![BusinessVideoCompletionAcceptanceVideo {
                    title: "项目宣传片".to_owned(),
                    video_type: "横版宣传片".to_owned(),
                    content: "项目整体形象展示".to_owned(),
                    duration: "03:30".to_owned(),
                    asset_reference: BusinessVideoCompletionAcceptanceAssetReference {
                        asset_id: "asset-video-1".to_owned(),
                        file_name: "project-promo.mp4".to_owned(),
                        sha256: "A".repeat(64),
                        external_link: Some("https://example.invalid/project-promo".to_owned()),
                    },
                    screenshots: vec![BusinessVideoCompletionAcceptanceScreenshot {
                        asset_id: "asset-shot-1".to_owned(),
                        sha256: "B".repeat(64),
                        caption: "项目主视觉".to_owned(),
                    }],
                }],
            }],
            acceptance_conclusion: "本批次交付内容验收通过".to_owned(),
            manually_confirmed: true,
        };

        let serialized = serde_json::to_value(&contract).unwrap();
        assert_eq!(serialized["contractTitle"], "年度视频制作服务合同");
        assert_eq!(
            serialized["deliveryGroups"][0]["groupKey"],
            "delivery-group-1"
        );
        assert_eq!(
            serialized["deliveryGroups"][0]["videos"][0]["assetReference"]["assetId"],
            "asset-video-1"
        );
        assert_eq!(
            serialized["deliveryGroups"][0]["videos"][0]["screenshots"][0]["assetId"],
            "asset-shot-1"
        );

        let decoded: BusinessVideoCompletionAcceptanceData =
            serde_json::from_value(serialized).unwrap();
        assert_eq!(decoded, contract);
    }

    #[test]
    fn production_result_confirmation_contract_round_trips_with_camel_case_fields() {
        let contract = BusinessProductionResultConfirmationData {
            attachment_label: "附件一".to_owned(),
            contract_title: "年度制作服务合同".to_owned(),
            project_title: "白鹅潭制作项目".to_owned(),
            category: "视频制作".to_owned(),
            payment_amount_cents: 2_680_000,
            contract_deliverable_summary: "完成分镜、拍摄、剪辑及成片交付".to_owned(),
            supplier_legal_name: "广州示例文化有限公司".to_owned(),
            procurement_period: "2026-06-01 至 2026-07-20".to_owned(),
            delivery_items: vec![BusinessProductionResultConfirmationDeliveryItem {
                item_key: "delivery-item-1".to_owned(),
                title: "项目宣传片".to_owned(),
                deliverable_summary: "一条三分钟横版宣传片".to_owned(),
                evidence_images: vec![BusinessProductionResultConfirmationAssetReference {
                    asset_id: "asset-evidence-1".to_owned(),
                    sha256: "D".repeat(64),
                    group_key: "delivery-item-1/evidence-1".to_owned(),
                    file_name: "delivery-evidence.jpg".to_owned(),
                    caption: "交付成果主画面".to_owned(),
                }],
                storyboards: vec![BusinessProductionResultConfirmationStoryboard {
                    storyboard_number: "SB-01".to_owned(),
                    title: "项目开篇".to_owned(),
                    description: "建立项目区位与整体氛围".to_owned(),
                    shots: vec![BusinessProductionResultConfirmationShot {
                        shot_number: "SHOT-01".to_owned(),
                        shot_description: "航拍项目全景并切入主视觉".to_owned(),
                        images: vec![BusinessProductionResultConfirmationAssetReference {
                            asset_id: "asset-image-1".to_owned(),
                            sha256: "C".repeat(64),
                            group_key: "delivery-item-1/storyboard-SB-01/shot-SHOT-01".to_owned(),
                            file_name: "shot-01.jpg".to_owned(),
                            caption: "项目全景".to_owned(),
                        }],
                    }],
                }],
            }],
            acceptance_description: "制作成果与合同约定一致，验收通过".to_owned(),
            penalty_or_addition: "无扣罚或增补".to_owned(),
            completion_date: "2026-07-20".to_owned(),
            acceptance_date: "2026-07-29".to_owned(),
            clean_highlights_confirmed: true,
            manually_confirmed: true,
        };

        let serialized = serde_json::to_value(&contract).unwrap();
        assert_eq!(serialized["attachmentLabel"], "附件一");
        assert_eq!(serialized["paymentAmountCents"], 2_680_000);
        assert_eq!(
            serialized["deliveryItems"][0]["evidenceImages"][0]["groupKey"],
            "delivery-item-1/evidence-1"
        );
        assert_eq!(
            serialized["deliveryItems"][0]["storyboards"][0]["shots"][0]["images"][0]["assetId"],
            "asset-image-1"
        );
        assert_eq!(
            serialized["deliveryItems"][0]["storyboards"][0]["shots"][0]["images"][0]["groupKey"],
            "delivery-item-1/storyboard-SB-01/shot-SHOT-01"
        );
        assert_eq!(serialized["cleanHighlightsConfirmed"], true);
        assert!(serialized.get("attachment_label").is_none());
        assert!(serialized.to_string().find("bytes").is_none());
        assert!(serialized.to_string().find("path").is_none());

        let decoded: BusinessProductionResultConfirmationData =
            serde_json::from_value(serialized).unwrap();
        assert_eq!(decoded, contract);
    }

    #[test]
    fn legacy_business_profiles_default_new_quotation_fields() {
        let profile = BusinessProfile {
            tax_mode: BusinessTaxMode::TaxInclusive,
            project_discount_cents: 490_000,
            quotation_totals: Some(BusinessQuotationTotals {
                original_total_cents: 8_480_000,
                project_discount_cents: 490_000,
                tax_exclusive_total_cents: 7_537_736,
                tax_cents: 452_264,
                final_total_cents: 7_990_000,
            }),
            ..BusinessProfile::default()
        };
        let mut profile_json = serde_json::to_value(profile).unwrap();
        let profile_object = profile_json.as_object_mut().unwrap();
        profile_object.remove("taxMode");
        profile_object.remove("projectDiscountCents");
        profile_object.remove("quotationTotals");
        let decoded_profile: BusinessProfile = serde_json::from_value(profile_json).unwrap();
        assert_eq!(decoded_profile.tax_mode, BusinessTaxMode::TaxExclusive);
        assert_eq!(decoded_profile.project_discount_cents, 0);
        assert_eq!(decoded_profile.quotation_totals, None);

        let input = BusinessProfileInput {
            tax_mode: BusinessTaxMode::TaxInclusive,
            project_discount_cents: 490_000,
            ..BusinessProfileInput::default()
        };
        let mut input_json = serde_json::to_value(input).unwrap();
        let input_object = input_json.as_object_mut().unwrap();
        input_object.remove("taxMode");
        input_object.remove("projectDiscountCents");
        let decoded_input: BusinessProfileInput = serde_json::from_value(input_json).unwrap();
        assert_eq!(decoded_input.tax_mode, BusinessTaxMode::TaxExclusive);
        assert_eq!(decoded_input.project_discount_cents, 0);
    }

    #[test]
    fn legacy_surface_protocol_compatibility_is_bounded() {
        for supported in [
            LEGACY_PROTOCOL_VERSION,
            PROTOCOL_1_3_VERSION,
            PREVIOUS_PROTOCOL_VERSION,
            PROTOCOL_VERSION,
        ] {
            assert!(is_legacy_surface_protocol_supported(supported));
        }
        for unsupported in ["1.1", "1.6", "unsupported"] {
            assert!(!is_legacy_surface_protocol_supported(unsupported));
        }
    }

    #[test]
    fn backup_restore_json_contract_is_stable() {
        let command = BackupCommandEnvelope::Restore {
            command_id: "restore-command-1".to_string(),
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: OperationContext {
                actor_id: "operator-1".to_string(),
                account_id: Some("agency-1".to_string()),
                project_id: Some("project-1".to_string()),
                window_id: "business-workbench".to_string(),
                trace_id: "restore-trace-1".to_string(),
            },
            payload: RestoreAssetBackupPayload {
                asset_id: "asset-contract".to_string(),
                expected_sha256: "a".repeat(64),
            },
            idempotency_key: "restore-idempotency-1".to_string(),
            expected_revision: Some(7),
            deadline_at: Some(10_000),
        };

        let serialized = serde_json::to_value(&command).expect("serialize restore command");
        assert_eq!(serialized["commandType"], "backup.restore");
        assert_eq!(serialized["payload"]["assetId"], "asset-contract");
        assert_eq!(serialized["payload"]["expectedSha256"], "a".repeat(64));
        assert_eq!(serialized["expectedRevision"], 7);
        assert_eq!(
            serde_json::to_value(BackupEventType::Restored).expect("serialize restored event"),
            "backup.restored"
        );

        let round_trip: BackupCommandEnvelope =
            serde_json::from_value(serialized).expect("deserialize restore command");
        assert_eq!(round_trip, command);
    }

    fn business_template_operation_context() -> OperationContext {
        OperationContext {
            actor_id: "template-reviewer".to_string(),
            account_id: Some("agency-1".to_string()),
            project_id: Some("project-1".to_string()),
            window_id: "business-workbench".to_string(),
            trace_id: "template-trace-1".to_string(),
        }
    }

    #[test]
    fn business_template_version_json_contract_is_stable() {
        let record = BusinessTemplateVersionRecord {
            id: "template-version-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            source_asset_id: "source-asset-1".to_string(),
            source_sha256: "a".repeat(64),
            normalized_asset_id: "normalized-asset-1".to_string(),
            normalized_sha256: "b".repeat(64),
            template_key: "payment-application-v1".to_string(),
            mapping_version: "mapping-v1".to_string(),
            converter_engine: "MicrosoftWord".to_string(),
            converter_version: "16.0".to_string(),
            converter_policy_version: "word-only-v1".to_string(),
            status: BusinessTemplateVersionStatus::PendingReview,
            reviewed_by: None,
            reviewed_at: None,
            review_note: String::new(),
            revision: 1,
            created_at: 100,
            updated_at: 100,
        };

        let serialized = serde_json::to_value(&record).expect("serialize template version");
        assert_eq!(serialized["workspaceId"], "workspace-1");
        assert_eq!(serialized["sourceAssetId"], "source-asset-1");
        assert_eq!(serialized["normalizedAssetId"], "normalized-asset-1");
        assert_eq!(serialized["converterPolicyVersion"], "word-only-v1");
        assert_eq!(serialized["status"], "pendingReview");
        assert_eq!(serialized["reviewedBy"], serde_json::Value::Null);

        let round_trip: BusinessTemplateVersionRecord =
            serde_json::from_value(serialized).expect("deserialize template version");
        assert_eq!(round_trip, record);
    }

    #[test]
    fn business_template_command_json_contract_is_stable() {
        let commands = [
            (
                BusinessWorkspaceCommandEnvelope::NormalizeLegacyTemplate {
                    command_id: "normalize-template-1".to_string(),
                    protocol_version: PROTOCOL_VERSION.to_string(),
                    context: business_template_operation_context(),
                    payload: NormalizeBusinessLegacyTemplatePayload {
                        workspace_id: "workspace-1".to_string(),
                        source_asset_id: "source-asset-1".to_string(),
                        expected_source_sha256: "a".repeat(64),
                        template_key: "payment-application-v1".to_string(),
                        mapping_version: "mapping-v1".to_string(),
                    },
                    idempotency_key: "normalize-template-idempotency-1".to_string(),
                    expected_revision: Some(7),
                    deadline_at: Some(10_000),
                },
                "businessWorkspace.normalizeLegacyTemplate",
                "sourceAssetId",
                "source-asset-1",
            ),
            (
                BusinessWorkspaceCommandEnvelope::ApproveTemplateVersion {
                    command_id: "approve-template-1".to_string(),
                    protocol_version: PROTOCOL_VERSION.to_string(),
                    context: business_template_operation_context(),
                    payload: ApproveBusinessTemplateVersionPayload {
                        workspace_id: "workspace-1".to_string(),
                        template_version_id: "template-version-1".to_string(),
                        note: "approved".to_string(),
                    },
                    idempotency_key: "approve-template-idempotency-1".to_string(),
                    expected_revision: Some(8),
                    deadline_at: None,
                },
                "businessWorkspace.approveTemplateVersion",
                "templateVersionId",
                "template-version-1",
            ),
            (
                BusinessWorkspaceCommandEnvelope::RejectTemplateVersion {
                    command_id: "reject-template-1".to_string(),
                    protocol_version: PROTOCOL_VERSION.to_string(),
                    context: business_template_operation_context(),
                    payload: RejectBusinessTemplateVersionPayload {
                        workspace_id: "workspace-1".to_string(),
                        template_version_id: "template-version-2".to_string(),
                        note: "unsafe content".to_string(),
                    },
                    idempotency_key: "reject-template-idempotency-1".to_string(),
                    expected_revision: Some(9),
                    deadline_at: None,
                },
                "businessWorkspace.rejectTemplateVersion",
                "templateVersionId",
                "template-version-2",
            ),
        ];

        for (command, command_type, payload_field, payload_value) in commands {
            let serialized = serde_json::to_value(&command).expect("serialize template command");
            assert_eq!(serialized["commandType"], command_type);
            assert_eq!(serialized["payload"][payload_field], payload_value);
            let round_trip: BusinessWorkspaceCommandEnvelope =
                serde_json::from_value(serialized).expect("deserialize template command");
            assert_eq!(round_trip, command);
        }
    }

    #[test]
    fn business_template_event_json_contract_is_stable() {
        for (event_type, expected) in [
            (
                BusinessWorkspaceEventType::TemplateVersionNormalized,
                "businessWorkspace.templateVersionNormalized",
            ),
            (
                BusinessWorkspaceEventType::TemplateVersionApproved,
                "businessWorkspace.templateVersionApproved",
            ),
            (
                BusinessWorkspaceEventType::TemplateVersionRejected,
                "businessWorkspace.templateVersionRejected",
            ),
        ] {
            assert_eq!(
                serde_json::to_value(event_type).expect("serialize template event"),
                expected
            );
        }
    }

    #[test]
    fn protocol_1_3_surface_compatibility_is_bounded() {
        for supported in [
            PROTOCOL_1_3_VERSION,
            PREVIOUS_PROTOCOL_VERSION,
            PROTOCOL_VERSION,
        ] {
            assert!(is_protocol_1_3_surface_supported(supported));
        }
        for unsupported in [LEGACY_PROTOCOL_VERSION, "1.6", "unsupported"] {
            assert!(!is_protocol_1_3_surface_supported(unsupported));
        }
    }
}
