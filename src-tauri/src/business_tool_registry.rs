//! Explicit, model-safe registry for the complete Business Skill Bundle tools.
//!
//! Current host inventory:
//! - BackendHost::list_projects and BackendHost::authorize_operation are the
//!   stable project and approval entry points.
//! - asset_service::get_asset returns safe metadata. Vault path resolution is
//!   backend-only and must remain inside the adapter.
//! - DocumentIntelligence::extract requires a backend-only source Path, so the
//!   model-facing contract accepts only assetId.
//! - contract_review_service::get_review can expose persisted extraction data
//!   to the host adapter for existing reviews.
//! - generic generated Artifact creation has no public service API yet, so it
//!   remains an explicit adapter capability and never reports fake success.
//!
//! This module owns only the wire allowlist, serializable contracts, permission
//! metadata, typed dispatch, validation, and the final no-path/no-URL boundary.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

pub const BUSINESS_TOOL_NAMESPACE: &str = "business";
pub const MAX_TOOL_ARGUMENT_BYTES: usize = 256 * 1024;
pub const MAX_ARTIFACT_CONTENT_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_MAX_ARTIFACT_CHARS: u32 = 48_000;
pub const MAX_ARTIFACT_CHARS: u32 = 120_000;
pub const DEFAULT_MAX_DOCUMENT_CHARS: u32 = 80_000;
pub const MAX_DOCUMENT_CHARS: u32 = 200_000;
pub const DEFAULT_MAX_DOCUMENT_PAGES: u32 = 40;
pub const MAX_DOCUMENT_PAGES: u32 = 200;
pub const MAX_TOOL_ITEMS: usize = 128;
pub const MAX_COMPARE_DIFFERENCES: u32 = 200;
pub const MAX_COMPARE_CHARS: u32 = 120_000;
pub const MAX_QUERY_CHARS: usize = 500;
pub const MAX_EXCERPT_CHARS: u32 = 2_000;
pub const MAX_TEMPLATE_CHARS: u32 = 120_000;
pub const MAX_CALCULATION_LINES: usize = 200;
pub const MAX_TASK_STEPS: usize = 100;
pub const MAX_DOCUMENT_FIELDS: usize = 200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BusinessTool {
    ProjectRead,
    ArtifactRead,
    DocumentExtract,
    ArtifactCreate,
    ApprovalRequest,
    ArtifactCompare,
    SourceLocate,
    TemplateRead,
    Calculation,
    LedgerRead,
    ProjectWrite,
    TaskPlan,
    DocumentGenerate,
    DocumentValidate,
}

impl BusinessTool {
    pub const ALL: [Self; 14] = [
        Self::ProjectRead,
        Self::ArtifactRead,
        Self::DocumentExtract,
        Self::ArtifactCreate,
        Self::ApprovalRequest,
        Self::ArtifactCompare,
        Self::SourceLocate,
        Self::TemplateRead,
        Self::Calculation,
        Self::LedgerRead,
        Self::ProjectWrite,
        Self::TaskPlan,
        Self::DocumentGenerate,
        Self::DocumentValidate,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::ProjectRead => "project_read",
            Self::ArtifactRead => "artifact_read",
            Self::DocumentExtract => "document_extract",
            Self::ArtifactCreate => "artifact_create",
            Self::ApprovalRequest => "approval_request",
            Self::ArtifactCompare => "artifact_compare",
            Self::SourceLocate => "source_locate",
            Self::TemplateRead => "template_read",
            Self::Calculation => "calculation",
            Self::LedgerRead => "ledger_read",
            Self::ProjectWrite => "project_write",
            Self::TaskPlan => "task_plan",
            Self::DocumentGenerate => "document_generate",
            Self::DocumentValidate => "document_validate",
        }
    }

    pub fn from_wire(namespace: &str, tool: &str) -> Result<Self, BusinessToolError> {
        if namespace != BUSINESS_TOOL_NAMESPACE {
            return Err(BusinessToolError::new(
                "BUSINESS_TOOL_NAMESPACE_DENIED",
                "dynamic tool namespace is not allowlisted",
                false,
            ));
        }
        match tool {
            "project_read" => Ok(Self::ProjectRead),
            "artifact_read" => Ok(Self::ArtifactRead),
            "document_extract" => Ok(Self::DocumentExtract),
            "artifact_create" => Ok(Self::ArtifactCreate),
            "approval_request" => Ok(Self::ApprovalRequest),
            "artifact_compare" => Ok(Self::ArtifactCompare),
            "source_locate" => Ok(Self::SourceLocate),
            "template_read" => Ok(Self::TemplateRead),
            "calculation" => Ok(Self::Calculation),
            "ledger_read" => Ok(Self::LedgerRead),
            "project_write" => Ok(Self::ProjectWrite),
            "task_plan" => Ok(Self::TaskPlan),
            "document_generate" => Ok(Self::DocumentGenerate),
            "document_validate" => Ok(Self::DocumentValidate),
            _ => Err(BusinessToolError::new(
                "BUSINESS_TOOL_NOT_ALLOWLISTED",
                "dynamic tool is not allowlisted",
                false,
            )),
        }
    }

    pub fn permission(self) -> BusinessToolPermission {
        let (permission, effect, resource_scope, approval_policy) = match self {
            Self::ProjectRead => (
                "business.project.read",
                BusinessToolEffect::Read,
                BusinessResourceScope::Project,
                BusinessApprovalPolicy::None,
            ),
            Self::ArtifactRead => (
                "business.artifact.read",
                BusinessToolEffect::Read,
                BusinessResourceScope::Artifact,
                BusinessApprovalPolicy::None,
            ),
            Self::DocumentExtract => (
                "business.document.extract",
                BusinessToolEffect::ReversibleWrite,
                BusinessResourceScope::Artifact,
                BusinessApprovalPolicy::None,
            ),
            Self::ArtifactCreate => (
                "business.artifact.create",
                BusinessToolEffect::ReversibleWrite,
                BusinessResourceScope::Project,
                BusinessApprovalPolicy::None,
            ),
            Self::ApprovalRequest => (
                "business.approval.request",
                BusinessToolEffect::ApprovalLedgerWrite,
                BusinessResourceScope::Approval,
                BusinessApprovalPolicy::CreatesPendingApproval,
            ),
            Self::ArtifactCompare => (
                "business.artifact.compare",
                BusinessToolEffect::Read,
                BusinessResourceScope::Artifact,
                BusinessApprovalPolicy::None,
            ),
            Self::SourceLocate => (
                "business.source.locate",
                BusinessToolEffect::Read,
                BusinessResourceScope::Source,
                BusinessApprovalPolicy::None,
            ),
            Self::TemplateRead => (
                "business.template.read",
                BusinessToolEffect::Read,
                BusinessResourceScope::Template,
                BusinessApprovalPolicy::None,
            ),
            Self::Calculation => (
                "business.calculation",
                BusinessToolEffect::Compute,
                BusinessResourceScope::Calculation,
                BusinessApprovalPolicy::None,
            ),
            Self::LedgerRead => (
                "business.ledger.read",
                BusinessToolEffect::Read,
                BusinessResourceScope::Ledger,
                BusinessApprovalPolicy::None,
            ),
            Self::ProjectWrite => (
                "business.project.write",
                BusinessToolEffect::ReversibleWrite,
                BusinessResourceScope::Project,
                BusinessApprovalPolicy::None,
            ),
            Self::TaskPlan => (
                "business.task.plan",
                BusinessToolEffect::ReversibleWrite,
                BusinessResourceScope::Task,
                BusinessApprovalPolicy::None,
            ),
            Self::DocumentGenerate => (
                "business.document.generate",
                BusinessToolEffect::ReversibleWrite,
                BusinessResourceScope::Document,
                BusinessApprovalPolicy::None,
            ),
            Self::DocumentValidate => (
                "business.document.validate",
                BusinessToolEffect::Read,
                BusinessResourceScope::Document,
                BusinessApprovalPolicy::None,
            ),
        };
        BusinessToolPermission {
            permission: permission.to_string(),
            effect,
            resource_scope,
            approval_policy,
        }
    }

    pub const fn binding(self) -> BusinessToolBackendBinding {
        match self {
            Self::ProjectRead => BusinessToolBackendBinding::ProjectRepository,
            Self::ArtifactRead => BusinessToolBackendBinding::AssetService,
            Self::DocumentExtract => BusinessToolBackendBinding::DocumentIntelligence,
            Self::ArtifactCreate => BusinessToolBackendBinding::ArtifactService,
            Self::ApprovalRequest => BusinessToolBackendBinding::ApprovalLedger,
            Self::ArtifactCompare => BusinessToolBackendBinding::ArtifactComparisonService,
            Self::SourceLocate => BusinessToolBackendBinding::SourceIndex,
            Self::TemplateRead => BusinessToolBackendBinding::TemplateRepository,
            Self::Calculation => BusinessToolBackendBinding::CalculationEngine,
            Self::LedgerRead => BusinessToolBackendBinding::LedgerRepository,
            Self::ProjectWrite => BusinessToolBackendBinding::ProjectRepository,
            Self::TaskPlan => BusinessToolBackendBinding::TaskEngine,
            Self::DocumentGenerate => BusinessToolBackendBinding::DocumentGeneration,
            Self::DocumentValidate => BusinessToolBackendBinding::DocumentValidation,
        }
    }

    pub const fn parameter_budget(self) -> BusinessToolParameterBudget {
        let max_argument_bytes = match self {
            Self::ArtifactCreate | Self::DocumentGenerate => {
                MAX_TOOL_ARGUMENT_BYTES + MAX_ARTIFACT_CONTENT_BYTES
            }
            Self::DocumentExtract => MAX_TOOL_ARGUMENT_BYTES + MAX_DOCUMENT_CHARS as usize,
            _ => MAX_TOOL_ARGUMENT_BYTES,
        };
        let max_output_bytes = match self {
            Self::DocumentExtract | Self::ArtifactCompare => 2 * 1024 * 1024,
            Self::SourceLocate | Self::LedgerRead | Self::TaskPlan => 512 * 1024,
            _ => MAX_ARTIFACT_CONTENT_BYTES,
        };
        let max_collection_items = match self {
            Self::Calculation => MAX_CALCULATION_LINES,
            Self::TaskPlan => MAX_TASK_STEPS,
            Self::DocumentGenerate => MAX_DOCUMENT_FIELDS,
            Self::LedgerRead | Self::SourceLocate => MAX_TOOL_ITEMS,
            Self::ArtifactCompare => MAX_COMPARE_DIFFERENCES as usize,
            _ => MAX_TOOL_ITEMS,
        };
        BusinessToolParameterBudget {
            max_argument_bytes,
            max_output_bytes,
            max_collection_items,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BusinessToolEffect {
    Read,
    Compute,
    ReversibleWrite,
    ApprovalLedgerWrite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BusinessResourceScope {
    Project,
    Artifact,
    Approval,
    Source,
    Template,
    Calculation,
    Ledger,
    Task,
    Document,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BusinessApprovalPolicy {
    None,
    CreatesPendingApproval,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BusinessToolBackendBinding {
    ProjectRepository,
    AssetService,
    DocumentIntelligence,
    ArtifactService,
    ApprovalLedger,
    ArtifactComparisonService,
    SourceIndex,
    TemplateRepository,
    CalculationEngine,
    LedgerRepository,
    TaskEngine,
    DocumentGeneration,
    DocumentValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessToolPermission {
    pub permission: String,
    pub effect: BusinessToolEffect,
    pub resource_scope: BusinessResourceScope,
    pub approval_policy: BusinessApprovalPolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessToolParameterBudget {
    pub max_argument_bytes: usize,
    pub max_output_bytes: usize,
    pub max_collection_items: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessToolDefinition {
    pub namespace: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub permission: BusinessToolPermission,
    pub backend_binding: BusinessToolBackendBinding,
    pub parameter_budget: BusinessToolParameterBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessToolContext {
    pub call_id: String,
    pub actor_id: String,
    pub account_id: Option<String>,
    pub project_id: Option<String>,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessToolCall {
    pub namespace: String,
    pub tool: String,
    #[serde(default = "empty_json_object")]
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessToolDispatchResult {
    pub namespace: String,
    pub tool: String,
    pub permission: BusinessToolPermission,
    pub output: BusinessToolOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "tool",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum BusinessToolOutput {
    ProjectRead(ProjectReadOutput),
    ArtifactRead(ArtifactReadOutput),
    DocumentExtract(DocumentExtractOutput),
    ArtifactCreate(ArtifactCreateOutput),
    ApprovalRequest(ApprovalRequestOutput),
    ArtifactCompare(ArtifactCompareOutput),
    SourceLocate(SourceLocateOutput),
    TemplateRead(TemplateReadOutput),
    Calculation(CalculationOutput),
    LedgerRead(LedgerReadOutput),
    ProjectWrite(ProjectWriteOutput),
    TaskPlan(TaskPlanOutput),
    DocumentGenerate(DocumentGenerateOutput),
    DocumentValidate(DocumentValidateOutput),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReadInput {
    pub project_id: String,
    #[serde(default = "default_true")]
    pub include_business_workspace: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactContentMode {
    #[default]
    MetadataOnly,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactReadInput {
    pub asset_id: String,
    #[serde(default)]
    pub content_mode: ArtifactContentMode,
    #[serde(default = "default_max_artifact_chars")]
    pub max_chars: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DocumentPurpose {
    ContractReview,
    ContractCompare,
    TenderReview,
    BusinessSearch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentExtractInput {
    pub asset_id: String,
    pub purpose: DocumentPurpose,
    pub review_id: Option<String>,
    #[serde(default)]
    pub start_page: u32,
    #[serde(default = "default_max_document_pages")]
    pub max_pages: u32,
    #[serde(default = "default_max_document_chars")]
    pub max_chars: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactCreateFormat {
    Markdown,
    PlainText,
    Json,
}

impl ArtifactCreateFormat {
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Markdown => "text/markdown",
            Self::PlainText => "text/plain",
            Self::Json => "application/json",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactCreateInput {
    pub project_id: String,
    pub display_name: String,
    pub format: ArtifactCreateFormat,
    pub content: String,
    #[serde(default)]
    pub source_artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BusinessApprovalAction {
    ContractFindingDecision,
    ContractPromotion,
    FinancialCommitment,
    ExternalDispatch,
    ArtifactDeletion,
}

impl BusinessApprovalAction {
    pub const fn operation(self) -> &'static str {
        match self {
            Self::ContractFindingDecision => "contractReview.decideFinding",
            Self::ContractPromotion => "businessWorkspace.promoteReviewedContract",
            Self::FinancialCommitment => "business.financialCommitment",
            Self::ExternalDispatch => "business.externalDispatch",
            Self::ArtifactDeletion => "asset.delete",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BusinessApprovalResource {
    Project,
    BusinessWorkspace,
    ContractReview,
    ReviewFinding,
    Artifact,
    BusinessDocument,
    Payment,
}

impl BusinessApprovalResource {
    pub const fn resource_type(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::BusinessWorkspace => "businessWorkspace",
            Self::ContractReview => "contractReview",
            Self::ReviewFinding => "reviewFinding",
            Self::Artifact => "asset",
            Self::BusinessDocument => "businessDocument",
            Self::Payment => "payment",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalRequestInput {
    pub action: BusinessApprovalAction,
    pub resource: BusinessApprovalResource,
    pub resource_id: String,
    pub summary: String,
    #[serde(default)]
    pub related_artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReadOutput {
    pub project: BusinessProjectView,
    pub business_workspace: Option<BusinessWorkspaceView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessProjectView {
    pub id: String,
    pub name: String,
    pub client_name: String,
    pub stage: String,
    pub revision: i64,
    pub updated_at: i64,
    pub brief: BusinessProjectBriefView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessProjectBriefView {
    pub objective: String,
    pub audience: String,
    pub deliverables: Vec<String>,
    pub mandatory_items: Vec<String>,
    pub constraints: Vec<String>,
    pub risks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessWorkspaceView {
    pub id: String,
    pub project_id: String,
    pub status: String,
    pub lifecycle_stage: String,
    pub revision: i64,
    pub current_document_ids: Vec<String>,
    pub outstanding_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactReadOutput {
    pub artifact: BusinessArtifactView,
    pub content: Option<BusinessArtifactContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessArtifactView {
    pub asset_id: String,
    pub project_id: Option<String>,
    pub display_name: String,
    pub kind: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub revision: i64,
    pub preview_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessArtifactContent {
    pub format: String,
    pub text: String,
    pub content_sha256: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentExtractOutput {
    pub extraction_id: String,
    pub source_asset_id: String,
    pub source_asset_sha256: String,
    pub status: String,
    pub parser_name: String,
    pub parser_version: String,
    pub page_count: i64,
    pub content_sha256: Option<String>,
    pub snapshot_asset_id: Option<String>,
    pub pages: Vec<DocumentPageView>,
    pub tables: Vec<DocumentTableView>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentPageView {
    pub page_index: i64,
    pub text: String,
    pub text_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentTableView {
    pub page_index: i64,
    pub order_index: i64,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactCreateOutput {
    pub artifact: BusinessArtifactView,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalRequestStatus {
    Pending,
    AlreadyApproved,
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalRequestOutput {
    pub approval_id: String,
    pub status: ApprovalRequestStatus,
    pub operation: String,
    pub resource_type: String,
    pub resource_id: String,
    pub expires_at: Option<i64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactCompareMode {
    Text,
    Structure,
    Semantic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactCompareInput {
    pub left_asset_id: String,
    pub right_asset_id: String,
    pub mode: ArtifactCompareMode,
    #[serde(default = "default_max_compare_differences")]
    pub max_differences: u32,
    #[serde(default = "default_max_compare_chars")]
    pub max_chars: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactDifference {
    pub kind: String,
    pub location: String,
    pub left_text: Option<String>,
    pub right_text: Option<String>,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactCompareOutput {
    pub comparison_id: String,
    pub left_asset_id: String,
    pub right_asset_id: String,
    pub mode: ArtifactCompareMode,
    pub status: String,
    pub summary: String,
    pub differences: Vec<ArtifactDifference>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BusinessSourceKind {
    Artifact,
    Project,
    Template,
    ContractReview,
    Quote,
    Acceptance,
    Payment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceLocateInput {
    pub query: String,
    pub project_id: Option<String>,
    #[serde(default)]
    pub kinds: Vec<BusinessSourceKind>,
    #[serde(default = "default_max_source_results")]
    pub max_results: u32,
    #[serde(default = "default_true")]
    pub include_excerpt: bool,
    #[serde(default = "default_max_excerpt_chars")]
    pub max_excerpt_chars: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceMatch {
    pub source_id: String,
    pub project_id: Option<String>,
    pub display_name: String,
    pub kind: BusinessSourceKind,
    pub relevance: f32,
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceLocateOutput {
    pub query: String,
    pub matches: Vec<SourceMatch>,
    pub total_matches: u32,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateReadInput {
    pub template_id: String,
    #[serde(default = "default_max_template_chars")]
    pub max_chars: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateFieldDefinition {
    pub key: String,
    pub label: String,
    pub required: bool,
    pub value_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateReadOutput {
    pub template_id: String,
    pub display_name: String,
    pub version: String,
    pub format: String,
    pub content: String,
    pub fields: Vec<TemplateFieldDefinition>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CalculationMode {
    Quote,
    Payment,
    Acceptance,
    Receivable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalculationLineInput {
    pub key: String,
    pub description: String,
    pub quantity_milli: i64,
    pub unit_price_cents: i64,
    #[serde(default)]
    pub discount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalculationInput {
    pub calculation_id: String,
    pub mode: CalculationMode,
    pub currency: String,
    pub lines: Vec<CalculationLineInput>,
    #[serde(default)]
    pub discount_cents: i64,
    #[serde(default)]
    pub tax_rate_basis_points: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalculationLineOutput {
    pub key: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalculationOutput {
    pub calculation_id: String,
    pub mode: CalculationMode,
    pub currency: String,
    pub lines: Vec<CalculationLineOutput>,
    pub subtotal_cents: i64,
    pub discount_cents: i64,
    pub taxable_cents: i64,
    pub tax_cents: i64,
    pub total_cents: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LedgerEntryKind {
    Quote,
    Contract,
    PaymentRequest,
    Acceptance,
    Receipt,
    Adjustment,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LedgerEntryStatus {
    Draft,
    Submitted,
    Approved,
    Paid,
    Rejected,
    Voided,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgerReadInput {
    pub project_id: String,
    #[serde(default)]
    pub kinds: Vec<LedgerEntryKind>,
    #[serde(default)]
    pub statuses: Vec<LedgerEntryStatus>,
    #[serde(default = "default_max_ledger_entries")]
    pub max_entries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgerEntryView {
    pub entry_id: String,
    pub project_id: String,
    pub kind: LedgerEntryKind,
    pub status: LedgerEntryStatus,
    pub document_id: Option<String>,
    pub amount_cents: i64,
    pub currency: String,
    pub due_at: Option<i64>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgerReadOutput {
    pub project_id: String,
    pub entries: Vec<LedgerEntryView>,
    pub total_amount_cents: i64,
    pub outstanding_amount_cents: i64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectWritePatch {
    pub name: Option<String>,
    pub client_name: Option<String>,
    pub stage: Option<String>,
    pub brief: Option<BusinessProjectBriefView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectWriteInput {
    pub project_id: String,
    pub expected_revision: i64,
    pub patch: ProjectWritePatch,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectWriteOutput {
    pub project: BusinessProjectView,
    pub changed_fields: Vec<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TaskPlanPriority {
    Low,
    Normal,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPlanStepInput {
    pub key: String,
    pub title: String,
    pub instructions: String,
    pub owner_role: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPlanInput {
    pub project_id: String,
    pub title: String,
    pub objective: String,
    pub priority: TaskPlanPriority,
    pub steps: Vec<TaskPlanStepInput>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPlanItem {
    pub task_id: String,
    pub key: String,
    pub title: String,
    pub status: String,
    pub position: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPlanOutput {
    pub plan_id: String,
    pub project_id: String,
    pub title: String,
    pub status: String,
    pub revision: i64,
    pub tasks: Vec<TaskPlanItem>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BusinessDocumentType {
    Quote,
    Contract,
    PaymentRequest,
    Acceptance,
    TenderChecklist,
    Brief,
    ReviewReport,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BusinessDocumentFormat {
    Markdown,
    PlainText,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum DocumentFieldValue {
    Text(String),
    Number(i64),
    MoneyCents(i64),
    Boolean(bool),
    Date(String),
    TextList(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentFieldInput {
    pub key: String,
    pub value: DocumentFieldValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentGenerateInput {
    pub project_id: String,
    pub document_type: BusinessDocumentType,
    pub format: BusinessDocumentFormat,
    pub template_id: Option<String>,
    pub fields: Vec<DocumentFieldInput>,
    #[serde(default)]
    pub source_artifact_ids: Vec<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentGenerateOutput {
    pub document_id: String,
    pub project_id: String,
    pub document_type: BusinessDocumentType,
    pub artifact: BusinessArtifactView,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DocumentValidationCheck {
    RequiredFields,
    ProjectBinding,
    Amounts,
    Dates,
    SourceEvidence,
    Formatting,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentValidateInput {
    pub artifact_id: String,
    pub document_type: BusinessDocumentType,
    #[serde(default)]
    pub checks: Vec<DocumentValidationCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentValidationIssue {
    pub code: String,
    pub severity: String,
    pub field: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentValidateOutput {
    pub artifact_id: String,
    pub document_type: BusinessDocumentType,
    pub valid: bool,
    pub issues: Vec<DocumentValidationIssue>,
    pub checked_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessToolError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl BusinessToolError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }

    #[allow(dead_code)]
    pub fn adapter_unavailable(capability: &str) -> Self {
        Self::new(
            "BUSINESS_TOOL_ADAPTER_UNAVAILABLE",
            format!("business tool adapter is unavailable for {capability}"),
            true,
        )
    }

    fn invalid_arguments(message: impl Into<String>) -> Self {
        Self::new("BUSINESS_TOOL_ARGUMENTS_INVALID", message, false)
    }
}

impl std::fmt::Display for BusinessToolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for BusinessToolError {}

/// Host-owned adapter. Implementations must call real SQLite/Vault/service APIs
/// and return an error when unavailable. The registry never synthesizes success.
pub trait BusinessToolDispatchAdapter: Send + Sync {
    fn project_read(
        &self,
        context: &BusinessToolContext,
        input: ProjectReadInput,
    ) -> Result<ProjectReadOutput, BusinessToolError>;

    fn artifact_read(
        &self,
        context: &BusinessToolContext,
        input: ArtifactReadInput,
    ) -> Result<ArtifactReadOutput, BusinessToolError>;

    fn document_extract(
        &self,
        context: &BusinessToolContext,
        input: DocumentExtractInput,
    ) -> Result<DocumentExtractOutput, BusinessToolError>;

    fn artifact_create(
        &self,
        context: &BusinessToolContext,
        input: ArtifactCreateInput,
    ) -> Result<ArtifactCreateOutput, BusinessToolError>;

    fn approval_request(
        &self,
        context: &BusinessToolContext,
        input: ApprovalRequestInput,
    ) -> Result<ApprovalRequestOutput, BusinessToolError>;

    /// Default methods keep the existing host adapter source-compatible while
    /// each new backend binding is implemented in the main line.
    fn artifact_compare(
        &self,
        _context: &BusinessToolContext,
        _input: ArtifactCompareInput,
    ) -> Result<ArtifactCompareOutput, BusinessToolError> {
        Err(unimplemented_tool(BusinessTool::ArtifactCompare))
    }

    fn source_locate(
        &self,
        _context: &BusinessToolContext,
        _input: SourceLocateInput,
    ) -> Result<SourceLocateOutput, BusinessToolError> {
        Err(unimplemented_tool(BusinessTool::SourceLocate))
    }

    fn template_read(
        &self,
        _context: &BusinessToolContext,
        _input: TemplateReadInput,
    ) -> Result<TemplateReadOutput, BusinessToolError> {
        Err(unimplemented_tool(BusinessTool::TemplateRead))
    }

    fn calculation(
        &self,
        _context: &BusinessToolContext,
        _input: CalculationInput,
    ) -> Result<CalculationOutput, BusinessToolError> {
        Err(unimplemented_tool(BusinessTool::Calculation))
    }

    fn ledger_read(
        &self,
        _context: &BusinessToolContext,
        _input: LedgerReadInput,
    ) -> Result<LedgerReadOutput, BusinessToolError> {
        Err(unimplemented_tool(BusinessTool::LedgerRead))
    }

    fn project_write(
        &self,
        _context: &BusinessToolContext,
        _input: ProjectWriteInput,
    ) -> Result<ProjectWriteOutput, BusinessToolError> {
        Err(unimplemented_tool(BusinessTool::ProjectWrite))
    }

    fn task_plan(
        &self,
        _context: &BusinessToolContext,
        _input: TaskPlanInput,
    ) -> Result<TaskPlanOutput, BusinessToolError> {
        Err(unimplemented_tool(BusinessTool::TaskPlan))
    }

    fn document_generate(
        &self,
        _context: &BusinessToolContext,
        _input: DocumentGenerateInput,
    ) -> Result<DocumentGenerateOutput, BusinessToolError> {
        Err(unimplemented_tool(BusinessTool::DocumentGenerate))
    }

    fn document_validate(
        &self,
        _context: &BusinessToolContext,
        _input: DocumentValidateInput,
    ) -> Result<DocumentValidateOutput, BusinessToolError> {
        Err(unimplemented_tool(BusinessTool::DocumentValidate))
    }
}

#[derive(Clone)]
pub struct BusinessToolRegistry {
    adapter: Arc<dyn BusinessToolDispatchAdapter>,
}

impl BusinessToolRegistry {
    pub fn new<A>(adapter: A) -> Self
    where
        A: BusinessToolDispatchAdapter + 'static,
    {
        Self {
            adapter: Arc::new(adapter),
        }
    }

    #[allow(dead_code)]
    pub fn from_shared(adapter: Arc<dyn BusinessToolDispatchAdapter>) -> Self {
        Self { adapter }
    }

    pub fn definitions() -> Vec<BusinessToolDefinition> {
        BusinessTool::ALL.into_iter().map(tool_definition).collect()
    }

    pub fn resolve(namespace: &str, tool: &str) -> Result<BusinessTool, BusinessToolError> {
        BusinessTool::from_wire(namespace, tool)
    }

    pub fn dispatch(
        &self,
        context: &BusinessToolContext,
        call: BusinessToolCall,
    ) -> Result<BusinessToolDispatchResult, BusinessToolError> {
        validate_context(context)?;
        let tool = Self::resolve(&call.namespace, &call.tool)?;
        validate_argument_budget(tool, &call.arguments)?;

        let output = match tool {
            BusinessTool::ProjectRead => {
                let input: ProjectReadInput = parse_arguments(call.arguments)?;
                validate_project_read(&input)?;
                let expected_project_id = input.project_id.clone();
                let output = self
                    .adapter
                    .project_read(context, input)
                    .map_err(redact_adapter_error)?;
                if output.project.id != expected_project_id {
                    return Err(binding_mismatch("projectId"));
                }
                if let Some(workspace) = &output.business_workspace {
                    if workspace.project_id != expected_project_id {
                        return Err(binding_mismatch("workspace.projectId"));
                    }
                }
                BusinessToolOutput::ProjectRead(output)
            }
            BusinessTool::ArtifactRead => {
                let input: ArtifactReadInput = parse_arguments(call.arguments)?;
                validate_artifact_read(&input)?;
                let expected_asset_id = input.asset_id.clone();
                let output = self
                    .adapter
                    .artifact_read(context, input)
                    .map_err(redact_adapter_error)?;
                if output.artifact.asset_id != expected_asset_id {
                    return Err(binding_mismatch("assetId"));
                }
                BusinessToolOutput::ArtifactRead(output)
            }
            BusinessTool::DocumentExtract => {
                let input: DocumentExtractInput = parse_arguments(call.arguments)?;
                validate_document_extract(&input)?;
                let expected_asset_id = input.asset_id.clone();
                let output = self
                    .adapter
                    .document_extract(context, input)
                    .map_err(redact_adapter_error)?;
                if output.source_asset_id != expected_asset_id {
                    return Err(binding_mismatch("sourceAssetId"));
                }
                BusinessToolOutput::DocumentExtract(output)
            }
            BusinessTool::ArtifactCreate => {
                let input: ArtifactCreateInput = parse_arguments(call.arguments)?;
                validate_artifact_create(&input)?;
                let expected_project_id = input.project_id.clone();
                let output = self
                    .adapter
                    .artifact_create(context, input)
                    .map_err(redact_adapter_error)?;
                if output.artifact.project_id.as_deref() != Some(expected_project_id.as_str()) {
                    return Err(binding_mismatch("artifact.projectId"));
                }
                if output.idempotency_key != context.call_id {
                    return Err(binding_mismatch("idempotencyKey"));
                }
                BusinessToolOutput::ArtifactCreate(output)
            }
            BusinessTool::ApprovalRequest => {
                let input: ApprovalRequestInput = parse_arguments(call.arguments)?;
                validate_approval_request(&input)?;
                let expected_operation = input.action.operation().to_string();
                let expected_resource_type = input.resource.resource_type().to_string();
                let expected_resource_id = input.resource_id.clone();
                let output = self
                    .adapter
                    .approval_request(context, input)
                    .map_err(redact_adapter_error)?;
                if output.operation != expected_operation
                    || output.resource_type != expected_resource_type
                    || output.resource_id != expected_resource_id
                {
                    return Err(binding_mismatch("approval target"));
                }
                BusinessToolOutput::ApprovalRequest(output)
            }
            BusinessTool::ArtifactCompare => {
                let input: ArtifactCompareInput = parse_arguments(call.arguments)?;
                validate_artifact_compare(&input)?;
                let left_asset_id = input.left_asset_id.clone();
                let right_asset_id = input.right_asset_id.clone();
                let output = self
                    .adapter
                    .artifact_compare(context, input)
                    .map_err(redact_adapter_error)?;
                if output.left_asset_id != left_asset_id || output.right_asset_id != right_asset_id
                {
                    return Err(binding_mismatch("comparison asset ids"));
                }
                BusinessToolOutput::ArtifactCompare(output)
            }
            BusinessTool::SourceLocate => {
                let input: SourceLocateInput = parse_arguments(call.arguments)?;
                validate_source_locate(&input)?;
                let project_id = input.project_id.clone();
                let query = input.query.clone();
                let output = self
                    .adapter
                    .source_locate(context, input)
                    .map_err(redact_adapter_error)?;
                if output.query != query
                    || output.matches.iter().any(|item| {
                        project_id
                            .as_deref()
                            .is_some_and(|expected| item.project_id.as_deref() != Some(expected))
                    })
                {
                    return Err(binding_mismatch("source query or projectId"));
                }
                BusinessToolOutput::SourceLocate(output)
            }
            BusinessTool::TemplateRead => {
                let input: TemplateReadInput = parse_arguments(call.arguments)?;
                validate_template_read(&input)?;
                let expected_template_id = input.template_id.clone();
                let output = self
                    .adapter
                    .template_read(context, input)
                    .map_err(redact_adapter_error)?;
                if output.template_id != expected_template_id {
                    return Err(binding_mismatch("templateId"));
                }
                BusinessToolOutput::TemplateRead(output)
            }
            BusinessTool::Calculation => {
                let input: CalculationInput = parse_arguments(call.arguments)?;
                validate_calculation(&input)?;
                let expected_calculation_id = input.calculation_id.clone();
                let expected_mode = input.mode;
                let expected_currency = input.currency.clone();
                let expected_line_count = input.lines.len();
                let output = self
                    .adapter
                    .calculation(context, input)
                    .map_err(redact_adapter_error)?;
                if output.calculation_id != expected_calculation_id
                    || output.mode != expected_mode
                    || output.currency != expected_currency
                    || output.lines.len() != expected_line_count
                {
                    return Err(binding_mismatch("calculation identity"));
                }
                BusinessToolOutput::Calculation(output)
            }
            BusinessTool::LedgerRead => {
                let input: LedgerReadInput = parse_arguments(call.arguments)?;
                validate_ledger_read(&input)?;
                let expected_project_id = input.project_id.clone();
                let output = self
                    .adapter
                    .ledger_read(context, input)
                    .map_err(redact_adapter_error)?;
                if output.project_id != expected_project_id
                    || output
                        .entries
                        .iter()
                        .any(|entry| entry.project_id != expected_project_id)
                {
                    return Err(binding_mismatch("ledger projectId"));
                }
                BusinessToolOutput::LedgerRead(output)
            }
            BusinessTool::ProjectWrite => {
                let input: ProjectWriteInput = parse_arguments(call.arguments)?;
                validate_project_write(&input)?;
                let expected_project_id = input.project_id.clone();
                let output = self
                    .adapter
                    .project_write(context, input)
                    .map_err(redact_adapter_error)?;
                if output.project.id != expected_project_id
                    || output.idempotency_key != context.call_id
                {
                    return Err(binding_mismatch("project write identity"));
                }
                BusinessToolOutput::ProjectWrite(output)
            }
            BusinessTool::TaskPlan => {
                let input: TaskPlanInput = parse_arguments(call.arguments)?;
                validate_task_plan(&input)?;
                let expected_project_id = input.project_id.clone();
                let expected_idempotency_key = input.idempotency_key.clone();
                let output = self
                    .adapter
                    .task_plan(context, input)
                    .map_err(redact_adapter_error)?;
                if output.project_id != expected_project_id
                    || output.idempotency_key != expected_idempotency_key
                {
                    return Err(binding_mismatch("task plan identity"));
                }
                BusinessToolOutput::TaskPlan(output)
            }
            BusinessTool::DocumentGenerate => {
                let input: DocumentGenerateInput = parse_arguments(call.arguments)?;
                validate_document_generate(&input)?;
                let expected_project_id = input.project_id.clone();
                let expected_document_type = input.document_type;
                let expected_idempotency_key = input.idempotency_key.clone();
                let output = self
                    .adapter
                    .document_generate(context, input)
                    .map_err(redact_adapter_error)?;
                if output.project_id != expected_project_id
                    || output.document_type != expected_document_type
                    || output.idempotency_key != expected_idempotency_key
                    || output.artifact.project_id.as_deref() != Some(expected_project_id.as_str())
                {
                    return Err(binding_mismatch("generated document identity"));
                }
                BusinessToolOutput::DocumentGenerate(output)
            }
            BusinessTool::DocumentValidate => {
                let input: DocumentValidateInput = parse_arguments(call.arguments)?;
                validate_document_validate(&input)?;
                let expected_artifact_id = input.artifact_id.clone();
                let expected_document_type = input.document_type;
                let output = self
                    .adapter
                    .document_validate(context, input)
                    .map_err(redact_adapter_error)?;
                if output.artifact_id != expected_artifact_id
                    || output.document_type != expected_document_type
                {
                    return Err(binding_mismatch("validated document identity"));
                }
                BusinessToolOutput::DocumentValidate(output)
            }
        };

        validate_typed_output(&output)?;
        ensure_model_safe_output(&output)?;
        validate_output_budget(tool, &output)?;
        Ok(BusinessToolDispatchResult {
            namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
            tool: tool.wire_name().to_string(),
            permission: tool.permission(),
            output,
        })
    }
}

pub fn ensure_model_safe_output<T: Serialize>(value: &T) -> Result<(), BusinessToolError> {
    let value = serde_json::to_value(value).map_err(|_| {
        BusinessToolError::new(
            "BUSINESS_TOOL_OUTPUT_SERIALIZATION_FAILED",
            "business tool output could not be serialized",
            false,
        )
    })?;
    inspect_model_output(&value)
}

fn validate_typed_output(output: &BusinessToolOutput) -> Result<(), BusinessToolError> {
    let invalid = |message: &str| {
        BusinessToolError::new("BUSINESS_TOOL_OUTPUT_INVALID", message.to_string(), false)
    };
    match output {
        BusinessToolOutput::ProjectRead(output) => {
            validate_opaque_id("project.id", &output.project.id).map_err(|_| {
                invalid("business tool backend returned an invalid project identifier")
            })?;
        }
        BusinessToolOutput::ArtifactRead(output) => {
            validate_opaque_id("artifact.assetId", &output.artifact.asset_id).map_err(|_| {
                invalid("business tool backend returned an invalid asset identifier")
            })?;
        }
        BusinessToolOutput::DocumentExtract(output) => {
            if output.pages.len() > MAX_DOCUMENT_PAGES as usize
                || output.tables.len() > MAX_TOOL_ITEMS
            {
                return Err(invalid(
                    "business tool backend exceeded document extraction collection limits",
                ));
            }
        }
        BusinessToolOutput::ArtifactCreate(output) => {
            validate_opaque_id("artifact.assetId", &output.artifact.asset_id).map_err(|_| {
                invalid("business tool backend returned an invalid asset identifier")
            })?;
        }
        BusinessToolOutput::ApprovalRequest(output) => {
            if output.status == ApprovalRequestStatus::Pending {
                validate_opaque_id("approvalId", &output.approval_id).map_err(|_| {
                    invalid("business tool backend returned an invalid approval identifier")
                })?;
            }
        }
        BusinessToolOutput::ArtifactCompare(output) => {
            if output.differences.len() > MAX_COMPARE_DIFFERENCES as usize {
                return Err(invalid(
                    "business tool backend exceeded comparison collection limits",
                ));
            }
        }
        BusinessToolOutput::SourceLocate(output) => {
            if output.matches.len() > MAX_TOOL_ITEMS
                || output.matches.iter().any(|item| {
                    !item.relevance.is_finite() || !(0.0..=1.0).contains(&item.relevance)
                })
            {
                return Err(invalid(
                    "business tool backend returned invalid source search results",
                ));
            }
        }
        BusinessToolOutput::TemplateRead(output) => {
            if output.fields.len() > MAX_TOOL_ITEMS
                || output.content.chars().count() > MAX_TEMPLATE_CHARS as usize
            {
                return Err(invalid(
                    "business tool backend exceeded template output limits",
                ));
            }
        }
        BusinessToolOutput::Calculation(output) => {
            validate_currency(&output.currency)
                .map_err(|_| invalid("business tool backend returned an invalid currency"))?;
            if output.lines.len() > MAX_CALCULATION_LINES {
                return Err(invalid(
                    "business tool backend exceeded calculation collection limits",
                ));
            }
        }
        BusinessToolOutput::LedgerRead(output) => {
            if output.entries.len() > MAX_TOOL_ITEMS
                || output
                    .entries
                    .iter()
                    .any(|entry| validate_currency(&entry.currency).is_err())
            {
                return Err(invalid(
                    "business tool backend returned invalid ledger entries",
                ));
            }
        }
        BusinessToolOutput::ProjectWrite(output) => {
            if output.changed_fields.is_empty() || output.changed_fields.len() > 4 {
                return Err(invalid(
                    "business tool backend returned invalid changed project fields",
                ));
            }
        }
        BusinessToolOutput::TaskPlan(output) => {
            if output.tasks.is_empty() || output.tasks.len() > MAX_TASK_STEPS {
                return Err(invalid(
                    "business tool backend returned invalid task plan items",
                ));
            }
        }
        BusinessToolOutput::DocumentGenerate(output) => {
            validate_opaque_id("documentId", &output.document_id).map_err(|_| {
                invalid("business tool backend returned an invalid document identifier")
            })?;
            validate_opaque_id("artifact.assetId", &output.artifact.asset_id).map_err(|_| {
                invalid("business tool backend returned an invalid asset identifier")
            })?;
        }
        BusinessToolOutput::DocumentValidate(output) => {
            if output.issues.len() > MAX_TOOL_ITEMS {
                return Err(invalid(
                    "business tool backend exceeded validation issue limits",
                ));
            }
        }
    }
    Ok(())
}

fn inspect_model_output(value: &Value) -> Result<(), BusinessToolError> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if forbidden_output_key(key) {
                    return Err(unsafe_output());
                }
                inspect_model_output(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                inspect_model_output(value)?;
            }
        }
        Value::String(value) if contains_forbidden_locator(value) => {
            return Err(unsafe_output());
        }
        _ => {}
    }
    Ok(())
}

fn forbidden_output_key(key: &str) -> bool {
    let compact = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    [
        "absolutepath",
        "localpath",
        "filesystempath",
        "sourcepath",
        "vaultpath",
        "rawurl",
        "remoteurl",
        "downloadurl",
        "signedurl",
        "credential",
        "apikey",
        "authorizationheader",
    ]
    .contains(&compact.as_str())
}

fn contains_forbidden_locator(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("://")
        || lower.contains("file:")
        || lower.contains("data:")
        || lower.contains("blob:")
    {
        return true;
    }

    let bytes = value.as_bytes();
    if bytes.windows(3).any(|window| {
        window[0].is_ascii_alphabetic()
            && window[1] == b':'
            && (window[2] == b'\\' || window[2] == b'/')
    }) {
        return true;
    }

    value.split_whitespace().any(|token| {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
        });
        token.starts_with("\\\\") || (token.len() > 1 && token.starts_with('/'))
    })
}

fn validate_context(context: &BusinessToolContext) -> Result<(), BusinessToolError> {
    validate_opaque_id("callId", &context.call_id)?;
    validate_opaque_id("actorId", &context.actor_id)?;
    validate_opaque_id("traceId", &context.trace_id)?;
    if let Some(account_id) = &context.account_id {
        validate_opaque_id("accountId", account_id)?;
    }
    if let Some(project_id) = &context.project_id {
        validate_opaque_id("context.projectId", project_id)?;
    }
    Ok(())
}

fn validate_argument_budget(
    tool: BusinessTool,
    arguments: &Value,
) -> Result<(), BusinessToolError> {
    let bytes = serde_json::to_vec(arguments)
        .map_err(|_| BusinessToolError::invalid_arguments("arguments must be valid JSON"))?;
    if bytes.len() > tool.parameter_budget().max_argument_bytes {
        return Err(BusinessToolError::invalid_arguments(
            "arguments exceed the business tool payload limit",
        ));
    }
    Ok(())
}

fn parse_arguments<T>(arguments: Value) -> Result<T, BusinessToolError>
where
    T: for<'de> Deserialize<'de>,
{
    if !arguments.is_object() {
        return Err(BusinessToolError::invalid_arguments(
            "arguments must be a JSON object",
        ));
    }
    serde_json::from_value(arguments).map_err(|_| {
        BusinessToolError::invalid_arguments(
            "arguments do not match the selected business tool schema",
        )
    })
}

fn validate_project_read(input: &ProjectReadInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("projectId", &input.project_id)
}

fn validate_artifact_read(input: &ArtifactReadInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("assetId", &input.asset_id)?;
    if input.max_chars == 0 || input.max_chars > MAX_ARTIFACT_CHARS {
        return Err(BusinessToolError::invalid_arguments(format!(
            "maxChars must be between 1 and {MAX_ARTIFACT_CHARS}"
        )));
    }
    Ok(())
}

fn validate_document_extract(input: &DocumentExtractInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("assetId", &input.asset_id)?;
    if let Some(review_id) = &input.review_id {
        validate_opaque_id("reviewId", review_id)?;
    }
    if input.max_pages == 0 || input.max_pages > MAX_DOCUMENT_PAGES {
        return Err(BusinessToolError::invalid_arguments(format!(
            "maxPages must be between 1 and {MAX_DOCUMENT_PAGES}"
        )));
    }
    if input.max_chars == 0 || input.max_chars > MAX_DOCUMENT_CHARS {
        return Err(BusinessToolError::invalid_arguments(format!(
            "maxChars must be between 1 and {MAX_DOCUMENT_CHARS}"
        )));
    }
    Ok(())
}

fn validate_artifact_create(input: &ArtifactCreateInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("projectId", &input.project_id)?;
    validate_display_name(&input.display_name)?;
    if input.content.len() > MAX_ARTIFACT_CONTENT_BYTES {
        return Err(BusinessToolError::invalid_arguments(
            "artifact content exceeds the inline content limit",
        ));
    }
    if input.content.trim().is_empty() {
        return Err(BusinessToolError::invalid_arguments(
            "artifact content is required",
        ));
    }
    if input.source_artifact_ids.len() > 64 {
        return Err(BusinessToolError::invalid_arguments(
            "sourceArtifactIds exceeds the maximum of 64",
        ));
    }
    for asset_id in &input.source_artifact_ids {
        validate_opaque_id("sourceArtifactId", asset_id)?;
    }
    if input.format == ArtifactCreateFormat::Json {
        serde_json::from_str::<Value>(&input.content).map_err(|_| {
            BusinessToolError::invalid_arguments("JSON artifact content must be valid JSON")
        })?;
    }
    Ok(())
}

fn validate_approval_request(input: &ApprovalRequestInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("resourceId", &input.resource_id)?;
    let summary = input.summary.trim();
    if summary.is_empty() || summary.chars().count() > 2_000 {
        return Err(BusinessToolError::invalid_arguments(
            "summary must contain between 1 and 2000 characters",
        ));
    }
    if input.related_artifact_ids.len() > 32 {
        return Err(BusinessToolError::invalid_arguments(
            "relatedArtifactIds exceeds the maximum of 32",
        ));
    }
    for asset_id in &input.related_artifact_ids {
        validate_opaque_id("relatedArtifactId", asset_id)?;
    }
    validate_approval_pair(input.action, input.resource)
}

fn validate_artifact_compare(input: &ArtifactCompareInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("leftAssetId", &input.left_asset_id)?;
    validate_opaque_id("rightAssetId", &input.right_asset_id)?;
    if input.left_asset_id == input.right_asset_id {
        return Err(BusinessToolError::invalid_arguments(
            "leftAssetId and rightAssetId must identify different artifacts",
        ));
    }
    if input.max_differences == 0 || input.max_differences > MAX_COMPARE_DIFFERENCES {
        return Err(BusinessToolError::invalid_arguments(format!(
            "maxDifferences must be between 1 and {MAX_COMPARE_DIFFERENCES}"
        )));
    }
    if input.max_chars == 0 || input.max_chars > MAX_COMPARE_CHARS {
        return Err(BusinessToolError::invalid_arguments(format!(
            "maxChars must be between 1 and {MAX_COMPARE_CHARS}"
        )));
    }
    Ok(())
}

fn validate_source_locate(input: &SourceLocateInput) -> Result<(), BusinessToolError> {
    validate_safe_text("query", &input.query, MAX_QUERY_CHARS)?;
    if let Some(project_id) = &input.project_id {
        validate_opaque_id("projectId", project_id)?;
    }
    if input.kinds.len() > MAX_TOOL_ITEMS {
        return Err(BusinessToolError::invalid_arguments(
            "kinds exceeds the maximum collection size",
        ));
    }
    if input.max_results == 0 || input.max_results as usize > MAX_TOOL_ITEMS {
        return Err(BusinessToolError::invalid_arguments(
            "maxResults must be between 1 and 128",
        ));
    }
    if input.max_excerpt_chars == 0 || input.max_excerpt_chars > MAX_EXCERPT_CHARS {
        return Err(BusinessToolError::invalid_arguments(format!(
            "maxExcerptChars must be between 1 and {MAX_EXCERPT_CHARS}"
        )));
    }
    Ok(())
}

fn validate_template_read(input: &TemplateReadInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("templateId", &input.template_id)?;
    if input.max_chars == 0 || input.max_chars > MAX_TEMPLATE_CHARS {
        return Err(BusinessToolError::invalid_arguments(format!(
            "maxChars must be between 1 and {MAX_TEMPLATE_CHARS}"
        )));
    }
    Ok(())
}

fn validate_calculation(input: &CalculationInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("calculationId", &input.calculation_id)?;
    validate_currency(&input.currency)?;
    if input.lines.is_empty() || input.lines.len() > MAX_CALCULATION_LINES {
        return Err(BusinessToolError::invalid_arguments(
            "lines must contain between 1 and 200 items",
        ));
    }
    if input.discount_cents < 0 || input.tax_rate_basis_points > 10_000 {
        return Err(BusinessToolError::invalid_arguments(
            "discountCents must be non-negative and taxRateBasisPoints must be at most 10000",
        ));
    }
    let mut subtotal = 0_i64;
    for line in &input.lines {
        validate_opaque_id("line.key", &line.key)?;
        validate_safe_text("line.description", &line.description, 500)?;
        if line.quantity_milli <= 0 || line.unit_price_cents < 0 || line.discount_cents < 0 {
            return Err(BusinessToolError::invalid_arguments(
                "calculation line quantity and prices are out of range",
            ));
        }
        let amount = line
            .quantity_milli
            .checked_mul(line.unit_price_cents)
            .and_then(|value| value.checked_div(1_000))
            .and_then(|value| value.checked_sub(line.discount_cents))
            .filter(|value| *value >= 0)
            .ok_or_else(|| BusinessToolError::invalid_arguments("calculation line overflows"))?;
        subtotal = subtotal
            .checked_add(amount)
            .ok_or_else(|| BusinessToolError::invalid_arguments("calculation total overflows"))?;
    }
    let taxable = subtotal
        .checked_sub(input.discount_cents)
        .filter(|value| *value >= 0)
        .ok_or_else(|| BusinessToolError::invalid_arguments("discount exceeds subtotal"))?;
    taxable
        .checked_mul(i64::from(input.tax_rate_basis_points))
        .and_then(|value| value.checked_div(10_000))
        .ok_or_else(|| BusinessToolError::invalid_arguments("tax calculation overflows"))?;
    Ok(())
}

fn validate_ledger_read(input: &LedgerReadInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("projectId", &input.project_id)?;
    if input.kinds.len() > MAX_TOOL_ITEMS || input.statuses.len() > MAX_TOOL_ITEMS {
        return Err(BusinessToolError::invalid_arguments(
            "ledger filters exceed the maximum collection size",
        ));
    }
    if input.max_entries == 0 || input.max_entries as usize > MAX_TOOL_ITEMS {
        return Err(BusinessToolError::invalid_arguments(
            "maxEntries must be between 1 and 128",
        ));
    }
    Ok(())
}

fn validate_project_write(input: &ProjectWriteInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("projectId", &input.project_id)?;
    if input.expected_revision < 0 {
        return Err(BusinessToolError::invalid_arguments(
            "expectedRevision must be non-negative",
        ));
    }
    validate_safe_text("reason", &input.reason, 2_000)?;
    if input.patch.name.is_none()
        && input.patch.client_name.is_none()
        && input.patch.stage.is_none()
        && input.patch.brief.is_none()
    {
        return Err(BusinessToolError::invalid_arguments(
            "project patch must contain at least one field",
        ));
    }
    if let Some(name) = &input.patch.name {
        validate_safe_text("patch.name", name, 180)?;
    }
    if let Some(client_name) = &input.patch.client_name {
        validate_safe_text("patch.clientName", client_name, 180)?;
    }
    if let Some(stage) = &input.patch.stage {
        validate_opaque_id("patch.stage", stage)?;
    }
    if let Some(brief) = &input.patch.brief {
        validate_project_brief(brief)?;
    }
    Ok(())
}

fn validate_project_brief(brief: &BusinessProjectBriefView) -> Result<(), BusinessToolError> {
    validate_safe_text("brief.objective", &brief.objective, 4_000)?;
    validate_safe_text("brief.audience", &brief.audience, 1_000)?;
    validate_text_list("brief.deliverables", &brief.deliverables, 64, 500)?;
    validate_text_list("brief.mandatoryItems", &brief.mandatory_items, 64, 500)?;
    validate_text_list("brief.constraints", &brief.constraints, 64, 500)?;
    validate_text_list("brief.risks", &brief.risks, 64, 500)
}

fn validate_task_plan(input: &TaskPlanInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("projectId", &input.project_id)?;
    validate_safe_text("title", &input.title, 180)?;
    validate_safe_text("objective", &input.objective, 4_000)?;
    validate_opaque_id("idempotencyKey", &input.idempotency_key)?;
    if input.steps.is_empty() || input.steps.len() > MAX_TASK_STEPS {
        return Err(BusinessToolError::invalid_arguments(
            "steps must contain between 1 and 100 items",
        ));
    }
    for step in &input.steps {
        validate_opaque_id("step.key", &step.key)?;
        validate_safe_text("step.title", &step.title, 180)?;
        validate_safe_text("step.instructions", &step.instructions, 4_000)?;
        validate_opaque_id("step.ownerRole", &step.owner_role)?;
        if step.depends_on.len() > MAX_TASK_STEPS {
            return Err(BusinessToolError::invalid_arguments(
                "step.dependsOn exceeds the maximum collection size",
            ));
        }
        for dependency in &step.depends_on {
            validate_opaque_id("step.dependsOn", dependency)?;
        }
    }
    Ok(())
}

fn validate_document_generate(input: &DocumentGenerateInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("projectId", &input.project_id)?;
    if let Some(template_id) = &input.template_id {
        validate_opaque_id("templateId", template_id)?;
    }
    validate_opaque_id("idempotencyKey", &input.idempotency_key)?;
    if input.fields.is_empty() || input.fields.len() > MAX_DOCUMENT_FIELDS {
        return Err(BusinessToolError::invalid_arguments(
            "fields must contain between 1 and 200 items",
        ));
    }
    for field in &input.fields {
        validate_opaque_id("field.key", &field.key)?;
        validate_document_field_value(&field.value)?;
    }
    if input.source_artifact_ids.len() > MAX_TOOL_ITEMS {
        return Err(BusinessToolError::invalid_arguments(
            "sourceArtifactIds exceeds the maximum collection size",
        ));
    }
    for asset_id in &input.source_artifact_ids {
        validate_opaque_id("sourceArtifactId", asset_id)?;
    }
    Ok(())
}

fn validate_document_field_value(value: &DocumentFieldValue) -> Result<(), BusinessToolError> {
    match value {
        DocumentFieldValue::Text(value) => validate_safe_text("field.value", value, 20_000),
        DocumentFieldValue::Number(_) | DocumentFieldValue::MoneyCents(_) => Ok(()),
        DocumentFieldValue::Boolean(_) => Ok(()),
        DocumentFieldValue::Date(value) => validate_safe_text("field.date", value, 64),
        DocumentFieldValue::TextList(values) => {
            validate_text_list("field.textList", values, 64, 500)
        }
    }
}

fn validate_document_validate(input: &DocumentValidateInput) -> Result<(), BusinessToolError> {
    validate_opaque_id("artifactId", &input.artifact_id)?;
    if input.checks.len() > 16 {
        return Err(BusinessToolError::invalid_arguments(
            "checks exceeds the maximum collection size",
        ));
    }
    Ok(())
}

fn validate_currency(value: &str) -> Result<(), BusinessToolError> {
    if value.len() != 3
        || !value
            .chars()
            .all(|character| character.is_ascii_uppercase())
    {
        return Err(BusinessToolError::invalid_arguments(
            "currency must be a three-letter uppercase code",
        ));
    }
    Ok(())
}

fn validate_text_list(
    field: &str,
    values: &[String],
    max_items: usize,
    max_chars: usize,
) -> Result<(), BusinessToolError> {
    if values.len() > max_items {
        return Err(BusinessToolError::invalid_arguments(format!(
            "{field} exceeds the maximum collection size"
        )));
    }
    for value in values {
        validate_safe_text(field, value, max_chars)?;
    }
    Ok(())
}

fn validate_safe_text(field: &str, value: &str, max_chars: usize) -> Result<(), BusinessToolError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed != value
        || trimmed.chars().count() > max_chars
        || contains_forbidden_locator(trimmed)
    {
        return Err(BusinessToolError::invalid_arguments(format!(
            "{field} must be bounded text without a path or URL"
        )));
    }
    Ok(())
}

fn validate_approval_pair(
    action: BusinessApprovalAction,
    resource: BusinessApprovalResource,
) -> Result<(), BusinessToolError> {
    let allowed = matches!(
        (action, resource),
        (
            BusinessApprovalAction::ContractFindingDecision,
            BusinessApprovalResource::ReviewFinding
        ) | (
            BusinessApprovalAction::ContractPromotion,
            BusinessApprovalResource::ContractReview
        ) | (
            BusinessApprovalAction::FinancialCommitment,
            BusinessApprovalResource::BusinessDocument
        ) | (
            BusinessApprovalAction::FinancialCommitment,
            BusinessApprovalResource::Payment
        ) | (
            BusinessApprovalAction::ExternalDispatch,
            BusinessApprovalResource::BusinessDocument
        ) | (
            BusinessApprovalAction::ExternalDispatch,
            BusinessApprovalResource::Artifact
        ) | (
            BusinessApprovalAction::ArtifactDeletion,
            BusinessApprovalResource::Artifact
        )
    );
    if !allowed {
        return Err(BusinessToolError::invalid_arguments(
            "approval action is not valid for the selected resource type",
        ));
    }
    Ok(())
}

fn validate_opaque_id(field: &str, value: &str) -> Result<(), BusinessToolError> {
    let trimmed = value.trim();
    let valid = !trimmed.is_empty()
        && trimmed.len() <= 160
        && trimmed == value
        && trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
        })
        && !contains_forbidden_locator(trimmed)
        && !trimmed.contains("..");
    if !valid {
        return Err(BusinessToolError::invalid_arguments(format!(
            "{field} must be an opaque identifier, not a path or URL"
        )));
    }
    Ok(())
}

fn validate_display_name(value: &str) -> Result<(), BusinessToolError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > 180
        || trimmed != value
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || contains_forbidden_locator(trimmed)
    {
        return Err(BusinessToolError::invalid_arguments(
            "displayName must be a plain file name without a path or URL",
        ));
    }
    Ok(())
}

fn redact_adapter_error(error: BusinessToolError) -> BusinessToolError {
    let safe_code = !error.code.is_empty()
        && error.code.len() <= 96
        && error.code.chars().all(|character| {
            character.is_ascii_uppercase() || character == '_' || character.is_ascii_digit()
        });
    if !safe_code || contains_forbidden_locator(&error.message) {
        return BusinessToolError::new(
            "BUSINESS_TOOL_BACKEND_ERROR_REDACTED",
            "business tool backend failed; sensitive locator details were removed",
            error.retryable,
        );
    }
    error
}

fn binding_mismatch(field: &str) -> BusinessToolError {
    BusinessToolError::new(
        "BUSINESS_TOOL_BINDING_MISMATCH",
        format!("business tool backend returned a mismatched {field}"),
        false,
    )
}

fn unsafe_output() -> BusinessToolError {
    BusinessToolError::new(
        "BUSINESS_TOOL_OUTPUT_UNSAFE",
        "business tool output contained a forbidden URL or absolute path",
        false,
    )
}

fn unimplemented_tool(tool: BusinessTool) -> BusinessToolError {
    BusinessToolError::new(
        "BUSINESS_TOOL_BACKEND_UNIMPLEMENTED",
        format!(
            "business tool backend binding is not implemented for {}",
            tool.wire_name()
        ),
        false,
    )
}

fn validate_output_budget<T: Serialize>(
    tool: BusinessTool,
    output: &T,
) -> Result<(), BusinessToolError> {
    let bytes = serde_json::to_vec(output).map_err(|_| {
        BusinessToolError::new(
            "BUSINESS_TOOL_OUTPUT_SERIALIZATION_FAILED",
            "business tool output could not be serialized",
            false,
        )
    })?;
    if bytes.len() > tool.parameter_budget().max_output_bytes {
        return Err(BusinessToolError::new(
            "BUSINESS_TOOL_OUTPUT_BUDGET_EXCEEDED",
            "business tool output exceeds the model-safe payload limit",
            false,
        ));
    }
    Ok(())
}

fn empty_json_object() -> Value {
    json!({})
}

const fn default_true() -> bool {
    true
}

const fn default_max_artifact_chars() -> u32 {
    DEFAULT_MAX_ARTIFACT_CHARS
}

const fn default_max_document_pages() -> u32 {
    DEFAULT_MAX_DOCUMENT_PAGES
}

const fn default_max_document_chars() -> u32 {
    DEFAULT_MAX_DOCUMENT_CHARS
}

const fn default_max_compare_differences() -> u32 {
    MAX_COMPARE_DIFFERENCES
}

const fn default_max_compare_chars() -> u32 {
    MAX_COMPARE_CHARS
}

const fn default_max_source_results() -> u32 {
    MAX_TOOL_ITEMS as u32
}

const fn default_max_excerpt_chars() -> u32 {
    MAX_EXCERPT_CHARS
}

const fn default_max_template_chars() -> u32 {
    MAX_TEMPLATE_CHARS
}

const fn default_max_ledger_entries() -> u32 {
    MAX_TOOL_ITEMS as u32
}

fn tool_definition(tool: BusinessTool) -> BusinessToolDefinition {
    let (description, input_schema, output_schema) = match tool {
        BusinessTool::ProjectRead => (
            "Read one local project and its business workspace by stable projectId.",
            object_schema(&["projectId"], json!({
                "projectId": opaque_id_schema("Stable local project identifier."),
                "includeBusinessWorkspace": {"type": "boolean", "default": true}
            })),
            object_schema(&["project", "businessWorkspace"], json!({
                "project": {"type": "object", "description": "Model-safe project view without storage locators."},
                "businessWorkspace": {"type": ["object", "null"]}
            })),
        ),
        BusinessTool::ArtifactRead => (
            "Read local Artifact metadata and optional bounded text by assetId; never accepts a path or URL.",
            object_schema(&["assetId"], json!({
                "assetId": opaque_id_schema("Stable local asset identifier."),
                "contentMode": {"type": "string", "enum": ["metadataOnly", "text"], "default": "metadataOnly"},
                "maxChars": {"type": "integer", "minimum": 1, "maximum": MAX_ARTIFACT_CHARS, "default": DEFAULT_MAX_ARTIFACT_CHARS}
            })),
            object_schema(&["artifact", "content"], json!({
                "artifact": {"type": "object"},
                "content": {"type": ["object", "null"]}
            })),
        ),
        BusinessTool::DocumentExtract => (
            "Extract bounded text from a local document asset; Vault storage stays private.",
            object_schema(&["assetId", "purpose"], json!({
                "assetId": opaque_id_schema("Stable local source asset identifier."),
                "purpose": {"type": "string", "enum": ["contractReview", "contractCompare", "tenderReview", "businessSearch"]},
                "reviewId": {"type": ["string", "null"], "maxLength": 160},
                "startPage": {"type": "integer", "minimum": 0, "default": 0},
                "maxPages": {"type": "integer", "minimum": 1, "maximum": MAX_DOCUMENT_PAGES, "default": DEFAULT_MAX_DOCUMENT_PAGES},
                "maxChars": {"type": "integer", "minimum": 1, "maximum": MAX_DOCUMENT_CHARS, "default": DEFAULT_MAX_DOCUMENT_CHARS}
            })),
            object_schema(&["extractionId", "sourceAssetId", "status", "pages", "tables", "truncated"], json!({
                "extractionId": {"type": "string"},
                "sourceAssetId": {"type": "string"},
                "status": {"type": "string"},
                "pages": {"type": "array"},
                "tables": {"type": "array"},
                "truncated": {"type": "boolean"}
            })),
        ),
        BusinessTool::ArtifactCreate => (
            "Create an immutable local business Artifact; success requires a committed Vault asset.",
            object_schema(&["projectId", "displayName", "format", "content"], json!({
                "projectId": opaque_id_schema("Owning local project identifier."),
                "displayName": {"type": "string", "minLength": 1, "maxLength": 180},
                "format": {"type": "string", "enum": ["markdown", "plainText", "json"]},
                "content": {"type": "string", "minLength": 1, "maxLength": MAX_ARTIFACT_CONTENT_BYTES},
                "sourceArtifactIds": {"type": "array", "maxItems": 64, "items": opaque_id_schema("Source asset identifier.")}
            })),
            object_schema(&["artifact", "idempotencyKey"], json!({
                "artifact": {"type": "object"},
                "idempotencyKey": {"type": "string"}
            })),
        ),
        BusinessTool::ApprovalRequest => (
            "Create or reuse a human approval for an allowlisted action; never executes that action.",
            object_schema(&["action", "resource", "resourceId", "summary"], json!({
                "action": {"type": "string", "enum": ["contractFindingDecision", "contractPromotion", "financialCommitment", "externalDispatch", "artifactDeletion"]},
                "resource": {"type": "string", "enum": ["project", "businessWorkspace", "contractReview", "reviewFinding", "artifact", "businessDocument", "payment"]},
                "resourceId": opaque_id_schema("Stable local resource identifier."),
                "summary": {"type": "string", "minLength": 1, "maxLength": 2000},
                "relatedArtifactIds": {"type": "array", "maxItems": 32, "items": opaque_id_schema("Related asset identifier.")}
            })),
            object_schema(&["approvalId", "status", "operation", "resourceType", "resourceId"], json!({
                "approvalId": {"type": "string"},
                "status": {"type": "string", "enum": ["pending", "alreadyApproved", "denied"]},
                "operation": {"type": "string"},
                "resourceType": {"type": "string"},
                "resourceId": {"type": "string"}
            })),
        ),
        BusinessTool::ArtifactCompare => (
            "Compare two local Artifacts by stable assetId and return bounded typed differences.",
            object_schema(&["leftAssetId", "rightAssetId", "mode"], json!({
                "leftAssetId": opaque_id_schema("Left local asset identifier."),
                "rightAssetId": opaque_id_schema("Right local asset identifier."),
                "mode": {"type": "string", "enum": ["text", "structure", "semantic"]},
                "maxDifferences": {"type": "integer", "minimum": 1, "maximum": MAX_COMPARE_DIFFERENCES, "default": MAX_COMPARE_DIFFERENCES},
                "maxChars": {"type": "integer", "minimum": 1, "maximum": MAX_COMPARE_CHARS, "default": MAX_COMPARE_CHARS}
            })),
            object_schema(&["comparisonId", "leftAssetId", "rightAssetId", "mode", "status", "summary", "differences", "truncated"], json!({
                "comparisonId": opaque_id_schema("Stable comparison identifier."),
                "leftAssetId": opaque_id_schema("Left asset identifier."),
                "rightAssetId": opaque_id_schema("Right asset identifier."),
                "mode": {"type": "string", "enum": ["text", "structure", "semantic"]},
                "status": {"type": "string"},
                "summary": {"type": "string"},
                "differences": {"type": "array", "maxItems": MAX_COMPARE_DIFFERENCES},
                "truncated": {"type": "boolean"}
            })),
        ),
        BusinessTool::SourceLocate => (
            "Locate bounded business sources in local indexes; results contain stable IDs, never paths or URLs.",
            object_schema(&["query"], json!({
                "query": {"type": "string", "minLength": 1, "maxLength": MAX_QUERY_CHARS},
                "projectId": nullable_opaque_id_schema("Optional project scope."),
                "kinds": {"type": "array", "maxItems": MAX_TOOL_ITEMS, "items": source_kind_schema()},
                "maxResults": {"type": "integer", "minimum": 1, "maximum": MAX_TOOL_ITEMS, "default": MAX_TOOL_ITEMS},
                "includeExcerpt": {"type": "boolean", "default": true},
                "maxExcerptChars": {"type": "integer", "minimum": 1, "maximum": MAX_EXCERPT_CHARS, "default": MAX_EXCERPT_CHARS}
            })),
            object_schema(&["query", "matches", "totalMatches", "truncated"], json!({
                "query": {"type": "string"},
                "matches": {"type": "array", "maxItems": MAX_TOOL_ITEMS},
                "totalMatches": {"type": "integer", "minimum": 0},
                "truncated": {"type": "boolean"}
            })),
        ),
        BusinessTool::TemplateRead => (
            "Read one allowlisted local business template by templateId with bounded content.",
            object_schema(&["templateId"], json!({
                "templateId": opaque_id_schema("Stable template identifier."),
                "maxChars": {"type": "integer", "minimum": 1, "maximum": MAX_TEMPLATE_CHARS, "default": MAX_TEMPLATE_CHARS}
            })),
            object_schema(&["templateId", "displayName", "version", "format", "content", "fields", "truncated"], json!({
                "templateId": opaque_id_schema("Stable template identifier."),
                "displayName": {"type": "string"},
                "version": {"type": "string"},
                "format": {"type": "string"},
                "content": {"type": "string", "maxLength": MAX_TEMPLATE_CHARS},
                "fields": {"type": "array", "maxItems": MAX_TOOL_ITEMS},
                "truncated": {"type": "boolean"}
            })),
        ),
        BusinessTool::Calculation => (
            "Calculate business money totals from typed integer-cent line items without evaluating expressions.",
            object_schema(&["calculationId", "mode", "currency", "lines"], json!({
                "calculationId": opaque_id_schema("Caller-stable calculation identifier."),
                "mode": {"type": "string", "enum": ["quote", "payment", "acceptance", "receivable"]},
                "currency": {"type": "string", "pattern": "^[A-Z]{3}$"},
                "lines": {"type": "array", "minItems": 1, "maxItems": MAX_CALCULATION_LINES, "items": calculation_line_input_schema()},
                "discountCents": {"type": "integer", "minimum": 0, "default": 0},
                "taxRateBasisPoints": {"type": "integer", "minimum": 0, "maximum": 10000, "default": 0}
            })),
            object_schema(&["calculationId", "mode", "currency", "lines", "subtotalCents", "discountCents", "taxableCents", "taxCents", "totalCents"], json!({
                "calculationId": opaque_id_schema("Calculation identifier."),
                "mode": {"type": "string"},
                "currency": {"type": "string"},
                "lines": {"type": "array", "maxItems": MAX_CALCULATION_LINES},
                "subtotalCents": {"type": "integer"},
                "discountCents": {"type": "integer"},
                "taxableCents": {"type": "integer"},
                "taxCents": {"type": "integer"},
                "totalCents": {"type": "integer"}
            })),
        ),
        BusinessTool::LedgerRead => (
            "Read bounded local business ledger entries for one project.",
            object_schema(&["projectId"], json!({
                "projectId": opaque_id_schema("Owning project identifier."),
                "kinds": {"type": "array", "maxItems": MAX_TOOL_ITEMS, "items": ledger_kind_schema()},
                "statuses": {"type": "array", "maxItems": MAX_TOOL_ITEMS, "items": ledger_status_schema()},
                "maxEntries": {"type": "integer", "minimum": 1, "maximum": MAX_TOOL_ITEMS, "default": MAX_TOOL_ITEMS}
            })),
            object_schema(&["projectId", "entries", "totalAmountCents", "outstandingAmountCents", "truncated"], json!({
                "projectId": opaque_id_schema("Owning project identifier."),
                "entries": {"type": "array", "maxItems": MAX_TOOL_ITEMS},
                "totalAmountCents": {"type": "integer"},
                "outstandingAmountCents": {"type": "integer"},
                "truncated": {"type": "boolean"}
            })),
        ),
        BusinessTool::ProjectWrite => (
            "Apply a revision-checked typed patch to local project master data.",
            object_schema(&["projectId", "expectedRevision", "patch", "reason"], json!({
                "projectId": opaque_id_schema("Owning project identifier."),
                "expectedRevision": {"type": "integer", "minimum": 0},
                "patch": project_write_patch_schema(),
                "reason": {"type": "string", "minLength": 1, "maxLength": 2000}
            })),
            object_schema(&["project", "changedFields", "idempotencyKey"], json!({
                "project": {"type": "object"},
                "changedFields": {"type": "array", "maxItems": 4, "items": {"type": "string"}},
                "idempotencyKey": opaque_id_schema("Committed command idempotency key.")
            })),
        ),
        BusinessTool::TaskPlan => (
            "Create a durable typed task plan for one local business project.",
            object_schema(&["projectId", "title", "objective", "priority", "steps", "idempotencyKey"], json!({
                "projectId": opaque_id_schema("Owning project identifier."),
                "title": {"type": "string", "minLength": 1, "maxLength": 180},
                "objective": {"type": "string", "minLength": 1, "maxLength": 4000},
                "priority": {"type": "string", "enum": ["low", "normal", "high", "urgent"]},
                "steps": {"type": "array", "minItems": 1, "maxItems": MAX_TASK_STEPS, "items": task_plan_step_schema()},
                "idempotencyKey": opaque_id_schema("Caller-stable idempotency key.")
            })),
            object_schema(&["planId", "projectId", "title", "status", "revision", "tasks", "idempotencyKey"], json!({
                "planId": opaque_id_schema("Task plan identifier."),
                "projectId": opaque_id_schema("Owning project identifier."),
                "title": {"type": "string"},
                "status": {"type": "string"},
                "revision": {"type": "integer"},
                "tasks": {"type": "array", "maxItems": MAX_TASK_STEPS},
                "idempotencyKey": opaque_id_schema("Committed idempotency key.")
            })),
        ),
        BusinessTool::DocumentGenerate => (
            "Generate a local business document from typed fields and commit it as an Artifact.",
            object_schema(&["projectId", "documentType", "format", "fields", "idempotencyKey"], json!({
                "projectId": opaque_id_schema("Owning project identifier."),
                "documentType": document_type_schema(),
                "format": {"type": "string", "enum": ["markdown", "plainText", "json"]},
                "templateId": nullable_opaque_id_schema("Optional local template identifier."),
                "fields": {"type": "array", "minItems": 1, "maxItems": MAX_DOCUMENT_FIELDS, "items": document_field_schema()},
                "sourceArtifactIds": {"type": "array", "maxItems": MAX_TOOL_ITEMS, "items": opaque_id_schema("Source asset identifier.")},
                "idempotencyKey": opaque_id_schema("Caller-stable idempotency key.")
            })),
            object_schema(&["documentId", "projectId", "documentType", "artifact", "idempotencyKey"], json!({
                "documentId": opaque_id_schema("Generated document identifier."),
                "projectId": opaque_id_schema("Owning project identifier."),
                "documentType": document_type_schema(),
                "artifact": {"type": "object"},
                "idempotencyKey": opaque_id_schema("Committed idempotency key.")
            })),
        ),
        BusinessTool::DocumentValidate => (
            "Validate one local business document Artifact with an allowlisted typed checklist.",
            object_schema(&["artifactId", "documentType"], json!({
                "artifactId": opaque_id_schema("Local document asset identifier."),
                "documentType": document_type_schema(),
                "checks": {"type": "array", "maxItems": 16, "items": {"type": "string", "enum": ["requiredFields", "projectBinding", "amounts", "dates", "sourceEvidence", "formatting"]}}
            })),
            object_schema(&["artifactId", "documentType", "valid", "issues", "checkedAt"], json!({
                "artifactId": opaque_id_schema("Validated asset identifier."),
                "documentType": document_type_schema(),
                "valid": {"type": "boolean"},
                "issues": {"type": "array", "maxItems": MAX_TOOL_ITEMS},
                "checkedAt": {"type": "integer"}
            })),
        ),
    };

    BusinessToolDefinition {
        namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
        name: tool.wire_name().to_string(),
        description: description.to_string(),
        input_schema,
        output_schema,
        permission: tool.permission(),
        backend_binding: tool.binding(),
        parameter_budget: tool.parameter_budget(),
    }
}

fn object_schema(required: &[&str], properties: Value) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

fn opaque_id_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "maxLength": 160,
        "pattern": "^[A-Za-z0-9_.:-]+$",
        "description": description
    })
}

fn nullable_opaque_id_schema(description: &str) -> Value {
    json!({
        "type": ["string", "null"],
        "minLength": 1,
        "maxLength": 160,
        "pattern": "^[A-Za-z0-9_.:-]+$",
        "description": description
    })
}

fn source_kind_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["artifact", "project", "template", "contractReview", "quote", "acceptance", "payment"]
    })
}

fn ledger_kind_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["quote", "contract", "paymentRequest", "acceptance", "receipt", "adjustment"]
    })
}

fn ledger_status_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["draft", "submitted", "approved", "paid", "rejected", "voided"]
    })
}

fn document_type_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["quote", "contract", "paymentRequest", "acceptance", "tenderChecklist", "brief", "reviewReport"]
    })
}

fn calculation_line_input_schema() -> Value {
    object_schema(
        &["key", "description", "quantityMilli", "unitPriceCents"],
        json!({
            "key": opaque_id_schema("Stable line key."),
            "description": {"type": "string", "minLength": 1, "maxLength": 500},
            "quantityMilli": {"type": "integer", "minimum": 1},
            "unitPriceCents": {"type": "integer", "minimum": 0},
            "discountCents": {"type": "integer", "minimum": 0, "default": 0}
        }),
    )
}

fn project_write_patch_schema() -> Value {
    object_schema(
        &[],
        json!({
            "name": {"type": ["string", "null"], "minLength": 1, "maxLength": 180},
            "clientName": {"type": ["string", "null"], "minLength": 1, "maxLength": 180},
            "stage": nullable_opaque_id_schema("Project stage."),
            "brief": {"type": ["object", "null"], "additionalProperties": false}
        }),
    )
}

fn task_plan_step_schema() -> Value {
    object_schema(
        &["key", "title", "instructions", "ownerRole"],
        json!({
            "key": opaque_id_schema("Stable step key."),
            "title": {"type": "string", "minLength": 1, "maxLength": 180},
            "instructions": {"type": "string", "minLength": 1, "maxLength": 4000},
            "ownerRole": opaque_id_schema("Owner role identifier."),
            "dependsOn": {"type": "array", "maxItems": MAX_TASK_STEPS, "items": opaque_id_schema("Dependency step key.")}
        }),
    )
}

fn document_field_schema() -> Value {
    object_schema(
        &["key", "value"],
        json!({
            "key": opaque_id_schema("Document field key."),
            "value": {
                "oneOf": [
                    {"type": "object", "additionalProperties": false, "required": ["kind", "value"], "properties": {"kind": {"const": "text"}, "value": {"type": "string", "maxLength": 20000}}},
                    {"type": "object", "additionalProperties": false, "required": ["kind", "value"], "properties": {"kind": {"enum": ["number", "moneyCents"]}, "value": {"type": "integer"}}},
                    {"type": "object", "additionalProperties": false, "required": ["kind", "value"], "properties": {"kind": {"const": "boolean"}, "value": {"type": "boolean"}}},
                    {"type": "object", "additionalProperties": false, "required": ["kind", "value"], "properties": {"kind": {"const": "date"}, "value": {"type": "string", "maxLength": 64}}},
                    {"type": "object", "additionalProperties": false, "required": ["kind", "value"], "properties": {"kind": {"const": "textList"}, "value": {"type": "array", "maxItems": 64, "items": {"type": "string", "maxLength": 500}}}}
                ]
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct StubAdapter {
        unsafe_text: bool,
        mismatch_project: bool,
    }

    impl StubAdapter {
        fn valid() -> Self {
            Self {
                unsafe_text: false,
                mismatch_project: false,
            }
        }
    }

    impl BusinessToolDispatchAdapter for StubAdapter {
        fn project_read(
            &self,
            _context: &BusinessToolContext,
            input: ProjectReadInput,
        ) -> Result<ProjectReadOutput, BusinessToolError> {
            Ok(ProjectReadOutput {
                project: BusinessProjectView {
                    id: if self.mismatch_project {
                        "project-other".to_string()
                    } else {
                        input.project_id.clone()
                    },
                    name: "示例项目".to_string(),
                    client_name: "示例客户".to_string(),
                    stage: "briefing".to_string(),
                    revision: 3,
                    updated_at: 10,
                    brief: BusinessProjectBriefView::default(),
                },
                business_workspace: input.include_business_workspace.then(|| {
                    BusinessWorkspaceView {
                        id: "workspace-1".to_string(),
                        project_id: input.project_id,
                        status: "active".to_string(),
                        lifecycle_stage: "draft".to_string(),
                        revision: 2,
                        current_document_ids: Vec::new(),
                        outstanding_cents: 0,
                    }
                }),
            })
        }

        fn artifact_read(
            &self,
            _context: &BusinessToolContext,
            input: ArtifactReadInput,
        ) -> Result<ArtifactReadOutput, BusinessToolError> {
            Ok(ArtifactReadOutput {
                artifact: artifact(&input.asset_id, Some("project-1")),
                content: Some(BusinessArtifactContent {
                    format: "text/plain".to_string(),
                    text: if self.unsafe_text {
                        "C:\\private\\contract.txt".to_string()
                    } else {
                        "合同正文".to_string()
                    },
                    content_sha256: Some("a".repeat(64)),
                    truncated: false,
                }),
            })
        }

        fn document_extract(
            &self,
            _context: &BusinessToolContext,
            input: DocumentExtractInput,
        ) -> Result<DocumentExtractOutput, BusinessToolError> {
            Ok(DocumentExtractOutput {
                extraction_id: "extraction-1".to_string(),
                source_asset_id: input.asset_id,
                source_asset_sha256: "a".repeat(64),
                status: "completed".to_string(),
                parser_name: "test".to_string(),
                parser_version: "1".to_string(),
                page_count: 1,
                content_sha256: Some("b".repeat(64)),
                snapshot_asset_id: None,
                pages: Vec::new(),
                tables: Vec::new(),
                truncated: false,
            })
        }

        fn artifact_create(
            &self,
            context: &BusinessToolContext,
            input: ArtifactCreateInput,
        ) -> Result<ArtifactCreateOutput, BusinessToolError> {
            Ok(ArtifactCreateOutput {
                artifact: artifact("asset-created", Some(&input.project_id)),
                idempotency_key: context.call_id.clone(),
            })
        }

        fn approval_request(
            &self,
            _context: &BusinessToolContext,
            input: ApprovalRequestInput,
        ) -> Result<ApprovalRequestOutput, BusinessToolError> {
            Ok(ApprovalRequestOutput {
                approval_id: "approval-1".to_string(),
                status: ApprovalRequestStatus::Pending,
                operation: input.action.operation().to_string(),
                resource_type: input.resource.resource_type().to_string(),
                resource_id: input.resource_id,
                expires_at: Some(100),
                reason: None,
            })
        }

        fn artifact_compare(
            &self,
            _context: &BusinessToolContext,
            input: ArtifactCompareInput,
        ) -> Result<ArtifactCompareOutput, BusinessToolError> {
            Ok(ArtifactCompareOutput {
                comparison_id: "comparison-1".to_string(),
                left_asset_id: input.left_asset_id,
                right_asset_id: input.right_asset_id,
                mode: input.mode,
                status: "completed".to_string(),
                summary: "one material change".to_string(),
                differences: vec![ArtifactDifference {
                    kind: "changed".to_string(),
                    location: "clause-1".to_string(),
                    left_text: Some("old".to_string()),
                    right_text: Some("new".to_string()),
                    severity: "high".to_string(),
                }],
                truncated: false,
            })
        }

        fn source_locate(
            &self,
            _context: &BusinessToolContext,
            input: SourceLocateInput,
        ) -> Result<SourceLocateOutput, BusinessToolError> {
            Ok(SourceLocateOutput {
                query: input.query,
                matches: vec![SourceMatch {
                    source_id: "asset-1".to_string(),
                    project_id: input.project_id,
                    display_name: "contract".to_string(),
                    kind: BusinessSourceKind::Artifact,
                    relevance: 0.9,
                    excerpt: input.include_excerpt.then(|| "payment clause".to_string()),
                }],
                total_matches: 1,
                truncated: false,
            })
        }

        fn template_read(
            &self,
            _context: &BusinessToolContext,
            input: TemplateReadInput,
        ) -> Result<TemplateReadOutput, BusinessToolError> {
            Ok(TemplateReadOutput {
                template_id: input.template_id,
                display_name: "quote template".to_string(),
                version: "1.0.0".to_string(),
                format: "markdown".to_string(),
                content: "template body".to_string(),
                fields: vec![TemplateFieldDefinition {
                    key: "client_name".to_string(),
                    label: "client".to_string(),
                    required: true,
                    value_kind: "text".to_string(),
                }],
                truncated: false,
            })
        }

        fn calculation(
            &self,
            _context: &BusinessToolContext,
            input: CalculationInput,
        ) -> Result<CalculationOutput, BusinessToolError> {
            let lines = input
                .lines
                .iter()
                .map(|line| CalculationLineOutput {
                    key: line.key.clone(),
                    amount_cents: line.quantity_milli * line.unit_price_cents / 1_000
                        - line.discount_cents,
                })
                .collect::<Vec<_>>();
            let subtotal_cents = lines.iter().map(|line| line.amount_cents).sum::<i64>();
            let taxable_cents = subtotal_cents - input.discount_cents;
            let tax_cents = taxable_cents * i64::from(input.tax_rate_basis_points) / 10_000;
            Ok(CalculationOutput {
                calculation_id: input.calculation_id,
                mode: input.mode,
                currency: input.currency,
                lines,
                subtotal_cents,
                discount_cents: input.discount_cents,
                taxable_cents,
                tax_cents,
                total_cents: taxable_cents + tax_cents,
            })
        }

        fn ledger_read(
            &self,
            _context: &BusinessToolContext,
            input: LedgerReadInput,
        ) -> Result<LedgerReadOutput, BusinessToolError> {
            Ok(LedgerReadOutput {
                project_id: input.project_id.clone(),
                entries: vec![LedgerEntryView {
                    entry_id: "ledger-1".to_string(),
                    project_id: input.project_id,
                    kind: LedgerEntryKind::Receipt,
                    status: LedgerEntryStatus::Paid,
                    document_id: Some("document-1".to_string()),
                    amount_cents: 10_000,
                    currency: "CNY".to_string(),
                    due_at: None,
                    updated_at: 100,
                }],
                total_amount_cents: 10_000,
                outstanding_amount_cents: 0,
                truncated: false,
            })
        }

        fn project_write(
            &self,
            context: &BusinessToolContext,
            input: ProjectWriteInput,
        ) -> Result<ProjectWriteOutput, BusinessToolError> {
            Ok(ProjectWriteOutput {
                project: BusinessProjectView {
                    id: if self.mismatch_project {
                        "project-other".to_string()
                    } else {
                        input.project_id
                    },
                    name: input.patch.name.unwrap_or_else(|| "project".to_string()),
                    client_name: input
                        .patch
                        .client_name
                        .unwrap_or_else(|| "client".to_string()),
                    stage: input.patch.stage.unwrap_or_else(|| "briefing".to_string()),
                    revision: input.expected_revision + 1,
                    updated_at: 100,
                    brief: input.patch.brief.unwrap_or_default(),
                },
                changed_fields: vec!["name".to_string()],
                idempotency_key: context.call_id.clone(),
            })
        }

        fn task_plan(
            &self,
            _context: &BusinessToolContext,
            input: TaskPlanInput,
        ) -> Result<TaskPlanOutput, BusinessToolError> {
            let tasks = input
                .steps
                .iter()
                .enumerate()
                .map(|(position, step)| TaskPlanItem {
                    task_id: format!("task-{position}"),
                    key: step.key.clone(),
                    title: step.title.clone(),
                    status: "pending".to_string(),
                    position: position as u32,
                })
                .collect();
            Ok(TaskPlanOutput {
                plan_id: "plan-1".to_string(),
                project_id: input.project_id,
                title: input.title,
                status: "active".to_string(),
                revision: 1,
                tasks,
                idempotency_key: input.idempotency_key,
            })
        }

        fn document_generate(
            &self,
            _context: &BusinessToolContext,
            input: DocumentGenerateInput,
        ) -> Result<DocumentGenerateOutput, BusinessToolError> {
            Ok(DocumentGenerateOutput {
                document_id: "document-1".to_string(),
                project_id: input.project_id.clone(),
                document_type: input.document_type,
                artifact: artifact("asset-generated", Some(&input.project_id)),
                idempotency_key: input.idempotency_key,
            })
        }

        fn document_validate(
            &self,
            _context: &BusinessToolContext,
            input: DocumentValidateInput,
        ) -> Result<DocumentValidateOutput, BusinessToolError> {
            Ok(DocumentValidateOutput {
                artifact_id: input.artifact_id,
                document_type: input.document_type,
                valid: true,
                issues: Vec::new(),
                checked_at: 100,
            })
        }
    }

    fn artifact(asset_id: &str, project_id: Option<&str>) -> BusinessArtifactView {
        BusinessArtifactView {
            asset_id: asset_id.to_string(),
            project_id: project_id.map(str::to_string),
            display_name: "report.md".to_string(),
            kind: "document".to_string(),
            mime_type: "text/plain".to_string(),
            size_bytes: 10,
            sha256: "a".repeat(64),
            revision: 1,
            preview_available: false,
        }
    }

    fn context() -> BusinessToolContext {
        BusinessToolContext {
            call_id: "call-1".to_string(),
            actor_id: "codex-runtime".to_string(),
            account_id: None,
            project_id: Some("project-1".to_string()),
            trace_id: "trace-1".to_string(),
        }
    }

    fn valid_calls() -> Vec<(&'static str, Value)> {
        vec![
            ("project_read", json!({"projectId": "project-1"})),
            (
                "artifact_read",
                json!({"assetId": "asset-1", "contentMode": "metadataOnly"}),
            ),
            (
                "document_extract",
                json!({"assetId": "asset-1", "purpose": "contractReview"}),
            ),
            (
                "artifact_create",
                json!({
                    "projectId": "project-1",
                    "displayName": "report.md",
                    "format": "markdown",
                    "content": "report"
                }),
            ),
            (
                "approval_request",
                json!({
                    "action": "externalDispatch",
                    "resource": "artifact",
                    "resourceId": "asset-1",
                    "summary": "send approved artifact"
                }),
            ),
            (
                "artifact_compare",
                json!({
                    "leftAssetId": "asset-1",
                    "rightAssetId": "asset-2",
                    "mode": "text"
                }),
            ),
            (
                "source_locate",
                json!({"query": "payment clause", "projectId": "project-1"}),
            ),
            ("template_read", json!({"templateId": "template-quote"})),
            (
                "calculation",
                json!({
                    "calculationId": "calculation-1",
                    "mode": "quote",
                    "currency": "CNY",
                    "lines": [{
                        "key": "line-1",
                        "description": "production",
                        "quantityMilli": 1000,
                        "unitPriceCents": 10000
                    }]
                }),
            ),
            ("ledger_read", json!({"projectId": "project-1"})),
            (
                "project_write",
                json!({
                    "projectId": "project-1",
                    "expectedRevision": 1,
                    "patch": {"name": "updated project"},
                    "reason": "confirmed by account manager"
                }),
            ),
            (
                "task_plan",
                json!({
                    "projectId": "project-1",
                    "title": "contract review",
                    "objective": "finish review",
                    "priority": "high",
                    "steps": [{
                        "key": "extract",
                        "title": "extract contract",
                        "instructions": "extract bounded text",
                        "ownerRole": "account_manager"
                    }],
                    "idempotencyKey": "plan-command-1"
                }),
            ),
            (
                "document_generate",
                json!({
                    "projectId": "project-1",
                    "documentType": "quote",
                    "format": "markdown",
                    "fields": [{
                        "key": "client_name",
                        "value": {"kind": "text", "value": "client"}
                    }],
                    "idempotencyKey": "document-command-1"
                }),
            ),
            (
                "document_validate",
                json!({
                    "artifactId": "asset-1",
                    "documentType": "contract",
                    "checks": ["requiredFields", "projectBinding"]
                }),
            ),
        ]
    }

    #[test]
    fn definitions_publish_only_the_explicit_business_namespace_allowlist() {
        let definitions = BusinessToolRegistry::definitions();
        assert_eq!(definitions.len(), 14);
        assert!(definitions
            .iter()
            .all(|definition| definition.namespace == BUSINESS_TOOL_NAMESPACE));
        assert!(definitions.iter().all(|definition| {
            definition.input_schema["additionalProperties"] == false
                && definition.output_schema["additionalProperties"] == false
                && definition.parameter_budget.max_argument_bytes > 0
                && definition.parameter_budget.max_output_bytes > 0
                && !definition.permission.permission.is_empty()
        }));
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "project_read",
                "artifact_read",
                "document_extract",
                "artifact_create",
                "approval_request",
                "artifact_compare",
                "source_locate",
                "template_read",
                "calculation",
                "ledger_read",
                "project_write",
                "task_plan",
                "document_generate",
                "document_validate"
            ]
        );
    }

    #[test]
    fn dispatch_rejects_unknown_namespace_and_tool() {
        assert_eq!(
            BusinessToolRegistry::resolve("other", "project_read")
                .unwrap_err()
                .code,
            "BUSINESS_TOOL_NAMESPACE_DENIED"
        );
        assert_eq!(
            BusinessToolRegistry::resolve(BUSINESS_TOOL_NAMESPACE, "arbitrary")
                .unwrap_err()
                .code,
            "BUSINESS_TOOL_NOT_ALLOWLISTED"
        );
    }

    #[test]
    fn project_read_dispatches_typed_arguments_and_output() {
        let result = BusinessToolRegistry::new(StubAdapter::valid())
            .dispatch(
                &context(),
                BusinessToolCall {
                    namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                    tool: "project_read".to_string(),
                    arguments: json!({ "projectId": "project-1" }),
                },
            )
            .unwrap();
        assert!(matches!(result.output, BusinessToolOutput::ProjectRead(_)));
        assert_eq!(result.permission.permission, "business.project.read");
    }

    #[test]
    fn all_fourteen_tools_dispatch_typed_inputs_and_outputs() {
        let registry = BusinessToolRegistry::new(StubAdapter::valid());
        for (tool, arguments) in valid_calls() {
            let result = registry
                .dispatch(
                    &context(),
                    BusinessToolCall {
                        namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                        tool: tool.to_string(),
                        arguments,
                    },
                )
                .unwrap_or_else(|error| panic!("{tool} failed: {error}"));
            assert_eq!(result.tool, tool);
            assert_eq!(
                result.permission.permission,
                format!("business.{}", tool.replace('_', "."))
            );
            ensure_model_safe_output(&result.output).unwrap();
        }
    }

    #[test]
    fn definitions_publish_expected_backend_bindings_and_effects() {
        let definitions = BusinessToolRegistry::definitions();
        let definition = |name: &str| {
            definitions
                .iter()
                .find(|definition| definition.name == name)
                .unwrap()
        };
        assert_eq!(
            definition("artifact_compare").backend_binding,
            BusinessToolBackendBinding::ArtifactComparisonService
        );
        assert_eq!(
            definition("calculation").permission.effect,
            BusinessToolEffect::Compute
        );
        assert_eq!(
            definition("ledger_read").backend_binding,
            BusinessToolBackendBinding::LedgerRepository
        );
        assert_eq!(
            definition("project_write").permission.effect,
            BusinessToolEffect::ReversibleWrite
        );
        assert_eq!(
            definition("document_generate").backend_binding,
            BusinessToolBackendBinding::DocumentGeneration
        );
    }

    #[test]
    fn adapter_binding_mismatch_never_reaches_the_model() {
        let error = BusinessToolRegistry::new(StubAdapter {
            mismatch_project: true,
            ..StubAdapter::valid()
        })
        .dispatch(
            &context(),
            BusinessToolCall {
                namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                tool: "project_read".to_string(),
                arguments: json!({ "projectId": "project-1" }),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_TOOL_BINDING_MISMATCH");
    }

    #[test]
    fn unsafe_paths_are_blocked_at_the_model_boundary() {
        let error = BusinessToolRegistry::new(StubAdapter {
            unsafe_text: true,
            ..StubAdapter::valid()
        })
        .dispatch(
            &context(),
            BusinessToolCall {
                namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                tool: "artifact_read".to_string(),
                arguments: json!({
                    "assetId": "asset-1",
                    "contentMode": "text",
                    "maxChars": 100
                }),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_TOOL_OUTPUT_UNSAFE");
    }

    #[test]
    fn invalid_approval_action_resource_pair_is_rejected_before_dispatch() {
        let error = BusinessToolRegistry::new(StubAdapter::valid())
            .dispatch(
                &context(),
                BusinessToolCall {
                    namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                    tool: "approval_request".to_string(),
                    arguments: json!({
                        "action": "artifactDeletion",
                        "resource": "payment",
                        "resourceId": "payment-1",
                        "summary": "删除"
                    }),
                },
            )
            .unwrap_err();
        assert_eq!(error.code, "BUSINESS_TOOL_ARGUMENTS_INVALID");
    }

    #[test]
    fn strict_serde_rejects_unknown_fields_and_non_object_arguments() {
        let registry = BusinessToolRegistry::new(StubAdapter::valid());
        for arguments in [
            json!({"templateId": "template-1", "rawPath": "secret"}),
            json!(["template-1"]),
        ] {
            let error = registry
                .dispatch(
                    &context(),
                    BusinessToolCall {
                        namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                        tool: "template_read".to_string(),
                        arguments,
                    },
                )
                .unwrap_err();
            assert_eq!(error.code, "BUSINESS_TOOL_ARGUMENTS_INVALID");
        }
    }

    #[test]
    fn tool_specific_argument_and_collection_budgets_are_enforced() {
        let registry = BusinessToolRegistry::new(StubAdapter::valid());
        let oversized = "x".repeat(MAX_TOOL_ARGUMENT_BYTES + 1);
        let error = registry
            .dispatch(
                &context(),
                BusinessToolCall {
                    namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                    tool: "source_locate".to_string(),
                    arguments: json!({"query": oversized}),
                },
            )
            .unwrap_err();
        assert_eq!(error.code, "BUSINESS_TOOL_ARGUMENTS_INVALID");

        let lines = (0..=MAX_CALCULATION_LINES)
            .map(|index| {
                json!({
                    "key": format!("line-{index}"),
                    "description": "line",
                    "quantityMilli": 1000,
                    "unitPriceCents": 1
                })
            })
            .collect::<Vec<_>>();
        let error = registry
            .dispatch(
                &context(),
                BusinessToolCall {
                    namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                    tool: "calculation".to_string(),
                    arguments: json!({
                        "calculationId": "calc-1",
                        "mode": "quote",
                        "currency": "CNY",
                        "lines": lines
                    }),
                },
            )
            .unwrap_err();
        assert_eq!(error.code, "BUSINESS_TOOL_ARGUMENTS_INVALID");
    }

    #[test]
    fn invalid_business_parameters_are_rejected_before_adapter_dispatch() {
        let registry = BusinessToolRegistry::new(StubAdapter::valid());
        let cases = [
            (
                "artifact_compare",
                json!({
                    "leftAssetId": "asset-1",
                    "rightAssetId": "asset-1",
                    "mode": "text"
                }),
            ),
            (
                "calculation",
                json!({
                    "calculationId": "calc-1",
                    "mode": "quote",
                    "currency": "cny",
                    "lines": [{
                        "key": "line-1",
                        "description": "line",
                        "quantityMilli": 1000,
                        "unitPriceCents": 1
                    }]
                }),
            ),
            (
                "project_write",
                json!({
                    "projectId": "project-1",
                    "expectedRevision": 1,
                    "patch": {},
                    "reason": "update"
                }),
            ),
            (
                "task_plan",
                json!({
                    "projectId": "project-1",
                    "title": "plan",
                    "objective": "objective",
                    "priority": "normal",
                    "steps": [],
                    "idempotencyKey": "plan-1"
                }),
            ),
        ];
        for (tool, arguments) in cases {
            let error = registry
                .dispatch(
                    &context(),
                    BusinessToolCall {
                        namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                        tool: tool.to_string(),
                        arguments,
                    },
                )
                .unwrap_err();
            assert_eq!(error.code, "BUSINESS_TOOL_ARGUMENTS_INVALID", "{tool}");
        }
    }

    #[test]
    fn input_and_output_security_boundaries_block_locators_and_credentials() {
        let registry = BusinessToolRegistry::new(StubAdapter::valid());
        for (tool, arguments) in [
            (
                "source_locate",
                json!({"query": "https://private.example/contract"}),
            ),
            (
                "template_read",
                json!({"templateId": "C:\\private\\template"}),
            ),
            (
                "document_generate",
                json!({
                    "projectId": "project-1",
                    "documentType": "quote",
                    "format": "markdown",
                    "fields": [{
                        "key": "source",
                        "value": {"kind": "text", "value": "file:///private/quote"}
                    }],
                    "idempotencyKey": "document-1"
                }),
            ),
        ] {
            let error = registry
                .dispatch(
                    &context(),
                    BusinessToolCall {
                        namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                        tool: tool.to_string(),
                        arguments,
                    },
                )
                .unwrap_err();
            assert_eq!(error.code, "BUSINESS_TOOL_ARGUMENTS_INVALID", "{tool}");
        }

        for unsafe_output in [
            json!({"rawUrl": "redacted"}),
            json!({"apiKey": "redacted"}),
            json!({"text": "C:\\private\\contract.docx"}),
        ] {
            assert_eq!(
                ensure_model_safe_output(&unsafe_output).unwrap_err().code,
                "BUSINESS_TOOL_OUTPUT_UNSAFE"
            );
        }
    }

    #[test]
    fn new_write_binding_mismatch_is_rejected() {
        let error = BusinessToolRegistry::new(StubAdapter {
            mismatch_project: true,
            ..StubAdapter::valid()
        })
        .dispatch(
            &context(),
            BusinessToolCall {
                namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
                tool: "project_write".to_string(),
                arguments: json!({
                    "projectId": "project-1",
                    "expectedRevision": 1,
                    "patch": {"name": "updated"},
                    "reason": "confirmed"
                }),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_TOOL_BINDING_MISMATCH");
    }
}
