#[path = "../src/business_tool_registry.rs"]
mod business_tool_registry;

use business_tool_registry::*;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdapterMode {
    Normal,
    ProjectMismatch,
    ArtifactMismatch,
    DocumentMismatch,
    ArtifactProjectMismatch,
    ArtifactIdempotencyMismatch,
    ApprovalMismatch,
    UnsafeUrl,
    UnsafePath,
    ErrorPath,
    Unavailable,
}

#[derive(Clone)]
struct RecordingAdapter {
    calls: Arc<Mutex<Vec<&'static str>>>,
    mode: AdapterMode,
}

impl RecordingAdapter {
    fn new(mode: AdapterMode) -> (Self, Arc<Mutex<Vec<&'static str>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: Arc::clone(&calls),
                mode,
            },
            calls,
        )
    }

    fn record(&self, name: &'static str) {
        self.calls.lock().expect("call log poisoned").push(name);
    }

    fn maybe_fail(&self, capability: &str) -> Result<(), BusinessToolError> {
        match self.mode {
            AdapterMode::ErrorPath => Err(BusinessToolError::new(
                "BACKEND_FAILURE",
                r"failed to open C:\Vault\private\contract.pdf",
                true,
            )),
            AdapterMode::Unavailable => Err(BusinessToolError::adapter_unavailable(capability)),
            _ => Ok(()),
        }
    }
}

impl BusinessToolDispatchAdapter for RecordingAdapter {
    fn project_read(
        &self,
        _context: &BusinessToolContext,
        input: ProjectReadInput,
    ) -> Result<ProjectReadOutput, BusinessToolError> {
        self.record("project_read");
        self.maybe_fail("project_read")?;
        let project_id = if self.mode == AdapterMode::ProjectMismatch {
            "project-other".to_string()
        } else {
            input.project_id
        };
        Ok(ProjectReadOutput {
            project: BusinessProjectView {
                id: project_id.clone(),
                name: "商务项目".to_string(),
                client_name: "示例客户".to_string(),
                stage: "contractReview".to_string(),
                revision: 3,
                updated_at: 1_750_000_000,
                brief: BusinessProjectBriefView::default(),
            },
            business_workspace: input
                .include_business_workspace
                .then(|| BusinessWorkspaceView {
                    id: "workspace-1".to_string(),
                    project_id,
                    status: "active".to_string(),
                    lifecycle_stage: "contractReview".to_string(),
                    revision: 2,
                    current_document_ids: vec!["asset-1".to_string()],
                    outstanding_cents: 10_000,
                }),
        })
    }

    fn artifact_read(
        &self,
        _context: &BusinessToolContext,
        input: ArtifactReadInput,
    ) -> Result<ArtifactReadOutput, BusinessToolError> {
        self.record("artifact_read");
        self.maybe_fail("artifact_read")?;
        let asset_id = if self.mode == AdapterMode::ArtifactMismatch {
            "asset-other".to_string()
        } else {
            input.asset_id
        };
        let display_name = if self.mode == AdapterMode::UnsafeUrl {
            "https://private.example/contract.pdf".to_string()
        } else {
            "contract.pdf".to_string()
        };
        let content = match input.content_mode {
            ArtifactContentMode::MetadataOnly => None,
            ArtifactContentMode::Text => Some(BusinessArtifactContent {
                format: "plainText".to_string(),
                text: if self.mode == AdapterMode::UnsafePath {
                    r"source: C:\Vault\private\contract.pdf".to_string()
                } else {
                    "合同正文".to_string()
                },
                content_sha256: Some("content-sha".to_string()),
                truncated: false,
            }),
        };
        Ok(ArtifactReadOutput {
            artifact: artifact_view(asset_id, Some("project-1"), display_name),
            content,
        })
    }

    fn document_extract(
        &self,
        _context: &BusinessToolContext,
        input: DocumentExtractInput,
    ) -> Result<DocumentExtractOutput, BusinessToolError> {
        self.record("document_extract");
        self.maybe_fail("document_extract")?;
        Ok(DocumentExtractOutput {
            extraction_id: "extract-1".to_string(),
            source_asset_id: if self.mode == AdapterMode::DocumentMismatch {
                "asset-other".to_string()
            } else {
                input.asset_id
            },
            source_asset_sha256: "source-sha".to_string(),
            status: "completed".to_string(),
            parser_name: "document-intelligence".to_string(),
            parser_version: "1.0.0".to_string(),
            page_count: 1,
            content_sha256: Some("content-sha".to_string()),
            snapshot_asset_id: Some("snapshot-1".to_string()),
            pages: vec![DocumentPageView {
                page_index: 0,
                text: "合同正文".to_string(),
                text_sha256: "page-sha".to_string(),
            }],
            tables: vec![],
            truncated: false,
        })
    }

    fn artifact_create(
        &self,
        context: &BusinessToolContext,
        input: ArtifactCreateInput,
    ) -> Result<ArtifactCreateOutput, BusinessToolError> {
        self.record("artifact_create");
        self.maybe_fail("artifact_create")?;
        let project_id = if self.mode == AdapterMode::ArtifactProjectMismatch {
            "project-other"
        } else {
            input.project_id.as_str()
        };
        Ok(ArtifactCreateOutput {
            artifact: artifact_view(
                "asset-created".to_string(),
                Some(project_id),
                input.display_name,
            ),
            idempotency_key: if self.mode == AdapterMode::ArtifactIdempotencyMismatch {
                "call-other".to_string()
            } else {
                context.call_id.clone()
            },
        })
    }

    fn approval_request(
        &self,
        _context: &BusinessToolContext,
        input: ApprovalRequestInput,
    ) -> Result<ApprovalRequestOutput, BusinessToolError> {
        self.record("approval_request");
        self.maybe_fail("approval_request")?;
        Ok(ApprovalRequestOutput {
            approval_id: "approval-1".to_string(),
            status: ApprovalRequestStatus::Pending,
            operation: if self.mode == AdapterMode::ApprovalMismatch {
                "asset.delete".to_string()
            } else {
                input.action.operation().to_string()
            },
            resource_type: input.resource.resource_type().to_string(),
            resource_id: input.resource_id,
            expires_at: None,
            reason: None,
        })
    }
}

fn artifact_view(
    asset_id: impl Into<String>,
    project_id: Option<&str>,
    display_name: impl Into<String>,
) -> BusinessArtifactView {
    BusinessArtifactView {
        asset_id: asset_id.into(),
        project_id: project_id.map(str::to_string),
        display_name: display_name.into(),
        kind: "document".to_string(),
        mime_type: "text/plain".to_string(),
        size_bytes: 128,
        sha256: "artifact-sha".to_string(),
        revision: 1,
        preview_available: true,
    }
}

fn context() -> BusinessToolContext {
    BusinessToolContext {
        call_id: "call-1".to_string(),
        actor_id: "actor-1".to_string(),
        account_id: Some("account-1".to_string()),
        project_id: Some("project-1".to_string()),
        trace_id: "trace-1".to_string(),
    }
}

fn call(tool: &str, arguments: Value) -> BusinessToolCall {
    BusinessToolCall {
        namespace: BUSINESS_TOOL_NAMESPACE.to_string(),
        tool: tool.to_string(),
        arguments,
    }
}

fn registry(mode: AdapterMode) -> (BusinessToolRegistry, Arc<Mutex<Vec<&'static str>>>) {
    let (adapter, calls) = RecordingAdapter::new(mode);
    (BusinessToolRegistry::new(adapter), calls)
}

fn error_code(result: Result<BusinessToolDispatchResult, BusinessToolError>) -> String {
    result.expect_err("dispatch should fail").code
}

#[test]
fn exposes_only_the_complete_business_allowlisted_tools() {
    let definitions = BusinessToolRegistry::definitions();
    let names = definitions
        .iter()
        .map(|definition| definition.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
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
            "document_validate",
        ]
    );
    assert!(definitions
        .iter()
        .all(|definition| definition.namespace == BUSINESS_TOOL_NAMESPACE));
    assert!(definitions
        .iter()
        .all(|definition| !definition.name.contains('.')));
}

#[test]
fn rejects_unknown_namespace_and_tool() {
    assert_eq!(
        BusinessToolRegistry::resolve("system", "project_read")
            .expect_err("namespace must be denied")
            .code,
        "BUSINESS_TOOL_NAMESPACE_DENIED"
    );
    assert_eq!(
        BusinessToolRegistry::resolve(BUSINESS_TOOL_NAMESPACE, "shell_exec")
            .expect_err("tool must be denied")
            .code,
        "BUSINESS_TOOL_NOT_ALLOWLISTED"
    );
}

#[test]
fn dispatches_all_five_typed_adapter_methods() {
    let (registry, calls) = registry(AdapterMode::Normal);
    let context = context();

    registry
        .dispatch(
            &context,
            call(
                "project_read",
                json!({"projectId": "project-1", "includeBusinessWorkspace": true}),
            ),
        )
        .expect("project read");
    registry
        .dispatch(
            &context,
            call(
                "artifact_read",
                json!({"assetId": "asset-1", "contentMode": "metadataOnly"}),
            ),
        )
        .expect("artifact read");
    registry
        .dispatch(
            &context,
            call(
                "document_extract",
                json!({"assetId": "asset-1", "purpose": "contractReview"}),
            ),
        )
        .expect("document extract");
    registry
        .dispatch(
            &context,
            call(
                "artifact_create",
                json!({
                    "projectId": "project-1",
                    "displayName": "review.md",
                    "format": "markdown",
                    "content": "# 审查结论"
                }),
            ),
        )
        .expect("artifact create");
    registry
        .dispatch(
            &context,
            call(
                "approval_request",
                json!({
                    "action": "contractFindingDecision",
                    "resource": "reviewFinding",
                    "resourceId": "finding-1",
                    "summary": "确认风险项处置"
                }),
            ),
        )
        .expect("approval request");

    assert_eq!(
        *calls.lock().expect("call log poisoned"),
        vec![
            "project_read",
            "artifact_read",
            "document_extract",
            "artifact_create",
            "approval_request",
        ]
    );
}

#[test]
fn rejects_urls_absolute_paths_and_unknown_fields_before_adapter_dispatch() {
    let (registry, calls) = registry(AdapterMode::Normal);
    let context = context();

    for invalid_call in [
        call("artifact_read", json!({"assetId": "https://example.com/a"})),
        call(
            "document_extract",
            json!({
                "assetId": r"C:\Vault\contract.pdf",
                "purpose": "contractReview"
            }),
        ),
        call(
            "project_read",
            json!({
                "projectId": "project-1",
                "absolutePath": r"C:\Vault\project.json"
            }),
        ),
    ] {
        assert_eq!(
            error_code(registry.dispatch(&context, invalid_call)),
            "BUSINESS_TOOL_ARGUMENTS_INVALID"
        );
    }

    assert!(calls.lock().expect("call log poisoned").is_empty());
}

#[test]
fn blocks_adapter_outputs_containing_urls_or_absolute_paths() {
    let context = context();

    let (url_registry, _) = registry(AdapterMode::UnsafeUrl);
    assert_eq!(
        error_code(url_registry.dispatch(
            &context,
            call("artifact_read", json!({"assetId": "asset-1"})),
        )),
        "BUSINESS_TOOL_OUTPUT_UNSAFE"
    );

    let (path_registry, _) = registry(AdapterMode::UnsafePath);
    assert_eq!(
        error_code(path_registry.dispatch(
            &context,
            call(
                "artifact_read",
                json!({"assetId": "asset-1", "contentMode": "text"}),
            ),
        )),
        "BUSINESS_TOOL_OUTPUT_UNSAFE"
    );
}

#[test]
fn redacts_sensitive_adapter_errors() {
    let (registry, _) = registry(AdapterMode::ErrorPath);
    let error = registry
        .dispatch(
            &context(),
            call("artifact_read", json!({"assetId": "asset-1"})),
        )
        .expect_err("backend error must be returned");

    assert_eq!(error.code, "BUSINESS_TOOL_BACKEND_ERROR_REDACTED");
    assert!(!error.message.contains("Vault"));
    assert!(!error.message.contains(r"C:\"));
    assert!(error.retryable);
}

#[test]
fn rejects_non_allowlisted_approval_action_resource_pairs() {
    let (registry, calls) = registry(AdapterMode::Normal);
    let result = registry.dispatch(
        &context(),
        call(
            "approval_request",
            json!({
                "action": "financialCommitment",
                "resource": "artifact",
                "resourceId": "asset-1",
                "summary": "非法组合"
            }),
        ),
    );

    assert_eq!(error_code(result), "BUSINESS_TOOL_ARGUMENTS_INVALID");
    assert!(calls.lock().expect("call log poisoned").is_empty());
}

#[test]
fn rejects_backend_resource_binding_mismatches() {
    let context = context();
    let cases = [
        (
            AdapterMode::ProjectMismatch,
            call("project_read", json!({"projectId": "project-1"})),
        ),
        (
            AdapterMode::ArtifactMismatch,
            call("artifact_read", json!({"assetId": "asset-1"})),
        ),
        (
            AdapterMode::DocumentMismatch,
            call(
                "document_extract",
                json!({"assetId": "asset-1", "purpose": "contractReview"}),
            ),
        ),
        (
            AdapterMode::ArtifactProjectMismatch,
            call(
                "artifact_create",
                json!({
                    "projectId": "project-1",
                    "displayName": "review.md",
                    "format": "markdown",
                    "content": "ok"
                }),
            ),
        ),
        (
            AdapterMode::ArtifactIdempotencyMismatch,
            call(
                "artifact_create",
                json!({
                    "projectId": "project-1",
                    "displayName": "review.md",
                    "format": "markdown",
                    "content": "ok"
                }),
            ),
        ),
        (
            AdapterMode::ApprovalMismatch,
            call(
                "approval_request",
                json!({
                    "action": "contractFindingDecision",
                    "resource": "reviewFinding",
                    "resourceId": "finding-1",
                    "summary": "确认风险项"
                }),
            ),
        ),
    ];

    for (mode, business_call) in cases {
        let (registry, _) = registry(mode);
        assert_eq!(
            error_code(registry.dispatch(&context, business_call)),
            "BUSINESS_TOOL_BINDING_MISMATCH",
            "mode {mode:?} should fail closed"
        );
    }
}

#[test]
fn definitions_permissions_and_schemas_are_serializable() {
    let (adapter, _) = RecordingAdapter::new(AdapterMode::Normal);
    let _shared_registry = BusinessToolRegistry::from_shared(Arc::new(adapter));
    assert_eq!(ArtifactCreateFormat::Markdown.mime_type(), "text/markdown");
    let value = serde_json::to_value(BusinessToolRegistry::definitions())
        .expect("definitions must serialize");
    let definitions = value.as_array().expect("definitions array");

    assert_eq!(definitions.len(), 14);
    for definition in definitions {
        assert_eq!(definition["namespace"], BUSINESS_TOOL_NAMESPACE);
        assert_eq!(definition["inputSchema"]["type"], "object");
        assert_eq!(definition["inputSchema"]["additionalProperties"], false);
        assert_eq!(definition["outputSchema"]["type"], "object");
        assert_eq!(
            definition["permission"]["permission"].as_str().unwrap()[..9],
            *"business."
        );
    }
}

#[test]
fn artifact_create_requires_call_id_as_idempotency_key() {
    let (registry, _) = registry(AdapterMode::Normal);
    let result = registry
        .dispatch(
            &context(),
            call(
                "artifact_create",
                json!({
                    "projectId": "project-1",
                    "displayName": "review.md",
                    "format": "markdown",
                    "content": "结论"
                }),
            ),
        )
        .expect("artifact create should succeed");

    match result.output {
        BusinessToolOutput::ArtifactCreate(output) => {
            assert_eq!(output.idempotency_key, "call-1");
            assert_eq!(output.artifact.project_id.as_deref(), Some("project-1"));
        }
        output => panic!("unexpected output: {output:?}"),
    }
}

#[test]
fn unavailable_adapter_returns_explicit_typed_error() {
    let (registry, _) = registry(AdapterMode::Unavailable);
    let error = registry
        .dispatch(
            &context(),
            call("project_read", json!({"projectId": "project-1"})),
        )
        .expect_err("unavailable adapter must fail");

    assert_eq!(error.code, "BUSINESS_TOOL_ADAPTER_UNAVAILABLE");
    assert!(error.retryable);
}
