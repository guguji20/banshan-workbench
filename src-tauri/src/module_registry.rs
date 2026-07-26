use crate::protocol::{ModuleAvailability, ModuleManifest, PROTOCOL_VERSION};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ModuleRegistry {
    manifests: Vec<ModuleManifest>,
}

impl ModuleRegistry {
    pub fn desktop() -> Self {
        let manifests = vec![
            manifest(
                "project.production",
                ModuleAvailability::Ready,
                &[
                    "project.create",
                    "project.updateBrief",
                    "project.changeStage",
                ],
                &[
                    "project.created",
                    "project.briefUpdated",
                    "project.stageChanged",
                ],
                &["project.read", "project.write"],
                &["project.get", "project.list", "brief.get"],
                &[
                    "sqlite.projects",
                    "sqlite.events",
                    "sqlite.command_receipts",
                ],
            ),
            manifest(
                "intake.requirementBrief",
                ModuleAvailability::Degraded,
                &[
                    "requirementBrief.create",
                    "requirementBrief.update",
                    "requirementBrief.changeStatus",
                ],
                &[
                    "requirementBrief.created",
                    "requirementBrief.updated",
                    "requirementBrief.statusChanged",
                ],
                &[
                    "requirementBrief.read",
                    "requirementBrief.write",
                    "requirementBrief.confirm",
                    "project.read",
                    "case.read",
                ],
                &["requirementBrief.list"],
                &[
                    "sqlite.requirement_briefs",
                    "sqlite.requirement_brief_events",
                    "sqlite.requirement_brief_command_receipts",
                ],
            ),
            manifest(
                "task.engine",
                ModuleAvailability::Degraded,
                &["task.create", "task.cancel", "task.retry"],
                &[
                    "task.created",
                    "task.canceled",
                    "task.retried",
                    "task.progressed",
                    "task.succeeded",
                    "task.failed",
                    "task.awaitingApproval",
                    "task.recovered",
                ],
                &[
                    "task.read",
                    "task.submit",
                    "task.cancel",
                    "task.retry",
                    "task.approve",
                ],
                &["task.list", "task.get", "task.cancel"],
                &[
                    "sqlite.tasks",
                    "sqlite.task_dependencies",
                    "sqlite.task_attempts",
                ],
            ),
            manifest(
                "asset.vault",
                ModuleAvailability::Degraded,
                &[
                    "asset.selectSource",
                    "asset.import",
                    "asset.open",
                    "asset.export",
                ],
                &["asset.imported"],
                &["asset.read", "asset.import", "asset.export"],
                &[
                    "asset.list",
                    "asset.get",
                    "asset.import",
                    "asset.open",
                    "asset.export",
                ],
                &[
                    "sqlite.assets",
                    "sqlite.asset_origins",
                    "vault.originals",
                    "vault.previews",
                ],
            ),
            manifest(
                "media.native",
                ModuleAvailability::Degraded,
                &["media.probe", "media.thumbnail", "media.extractAudio"],
                &[
                    "media.probed",
                    "media.thumbnailGenerated",
                    "media.audioExtracted",
                ],
                &["asset.read", "asset.import", "task.cancel"],
                &["media.probe", "media.thumbnail", "media.extractAudio"],
                &["vault.originals", "vault.previews", "sqlite.assets"],
            ),
            manifest(
                "creative.caseLibrary",
                ModuleAvailability::Degraded,
                &["case.create", "case.update"],
                &["case.created", "case.updated"],
                &["case.read", "case.write", "asset.read"],
                &["case.list"],
                &[
                    "sqlite.cases",
                    "sqlite.case_events",
                    "sqlite.case_command_receipts",
                ],
            ),
            manifest(
                "production.executionBrief",
                ModuleAvailability::Degraded,
                &[
                    "executionBrief.create",
                    "executionBrief.update",
                    "executionBrief.changeStatus",
                ],
                &[
                    "executionBrief.created",
                    "executionBrief.updated",
                    "executionBrief.statusChanged",
                ],
                &[
                    "executionBrief.read",
                    "executionBrief.write",
                    "project.read",
                ],
                &["executionBrief.list"],
                &[
                    "sqlite.execution_briefs",
                    "sqlite.execution_brief_events",
                    "sqlite.execution_brief_command_receipts",
                ],
            ),
            manifest(
                "business.documentCenter",
                ModuleAvailability::Ready,
                &[
                    "businessWorkspace.create",
                    "businessWorkspace.updateProfile",
                    "businessWorkspace.createDocument",
                    "businessWorkspace.promoteReviewedContract",
                    "businessWorkspace.changeDocumentStatus",
                    "businessWorkspace.generateDocument",
                    "businessWorkspace.upsertPayment",
                    "businessWorkspace.confirmQuote",
                    "businessWorkspace.recordReceipt",
                    "businessWorkspace.reverseReceipt",
                    "businessWorkspace.adoptLatestConfirmedRequirement",
                    "businessWorkspace.upsertCustomer",
                    "businessWorkspace.assignCustomer",
                    "businessWorkspace.upsertMilestone",
                    "businessWorkspace.registerDeliverableVersion",
                    "businessWorkspace.recordDeliverySent",
                    "businessWorkspace.recordDeliverySignoff",
                    "businessWorkspace.recordInvoiceIssued",
                    "businessWorkspace.recordInvoiceRedCorrection",
                    "businessWorkspace.attachInvoiceAsset",
                    "businessWorkspace.createArchiveSnapshot",
                    "businessWorkspace.changeStatus",
                ],
                &[
                    "businessWorkspace.created",
                    "businessWorkspace.profileUpdated",
                    "businessWorkspace.documentCreated",
                    "businessWorkspace.reviewedContractPromoted",
                    "businessWorkspace.documentStatusChanged",
                    "businessWorkspace.documentGenerated",
                    "businessWorkspace.paymentUpserted",
                    "businessWorkspace.quoteConfirmed",
                    "businessWorkspace.receiptRecorded",
                    "businessWorkspace.receiptReversed",
                    "businessWorkspace.requirementAdopted",
                    "businessWorkspace.customerUpserted",
                    "businessWorkspace.customerAssigned",
                    "businessWorkspace.milestoneUpserted",
                    "businessWorkspace.deliverableVersionRegistered",
                    "businessWorkspace.deliverySent",
                    "businessWorkspace.deliverySignoffRecorded",
                    "businessWorkspace.invoiceIssued",
                    "businessWorkspace.invoiceRedCorrected",
                    "businessWorkspace.invoiceAssetAttached",
                    "businessWorkspace.archiveSnapshotPrepared",
                    "businessWorkspace.statusChanged",
                ],
                &[
                    "businessWorkspace.read",
                    "businessWorkspace.write",
                    "businessWorkspace.approve",
                    "businessWorkspace.generate",
                    "asset.read",
                    "asset.import",
                    "project.read",
                    "requirementBrief.read",
                ],
                &[
                    "businessWorkspace.list",
                    "businessWorkspace.prefillCandidates",
                    "businessWorkspace.previewPrefill",
                ],
                &[
                    "sqlite.business_workspaces",
                    "sqlite.business_customers",
                    "sqlite.business_workspace_customers",
                    "sqlite.business_customer_conflicts",
                    "sqlite.business_customer_backfill",
                    "sqlite.business_documents",
                    "sqlite.business_payments",
                    "sqlite.business_quote_confirmations",
                    "sqlite.business_receipts",
                    "sqlite.business_delivery_milestones",
                    "sqlite.business_deliverable_versions",
                    "sqlite.business_delivery_submissions",
                    "sqlite.business_invoices",
                    "sqlite.business_archive_snapshots",
                    "sqlite.business_workspace_events",
                    "sqlite.business_workspace_command_receipts",
                    "sqlite.assets",
                    "sqlite.asset_origins",
                    "vault.originals",
                ],
            ),
            manifest(
                "document.intelligence",
                ModuleAvailability::Degraded,
                &[],
                &[
                    "contractReview.extractionCompleted",
                    "contractReview.ocrRequired",
                ],
                &[
                    "document.read",
                    "document.extract",
                    "document.ocr",
                    "asset.read",
                ],
                &["document.extract", "document.ocr", "document.getExtraction"],
                &[
                    "sqlite.assets",
                    "vault.originals",
                    "vault.previews",
                    "sqlite.document_extractions",
                    "sqlite.document_pages",
                    "sqlite.document_blocks",
                    "sqlite.document_tables",
                ],
            ),
            manifest(
                "business.contractReview",
                ModuleAvailability::Degraded,
                &[
                    "contractReview.create",
                    "contractReview.start",
                    "contractReview.cancel",
                    "contractReview.decideFinding",
                    "contractReview.retryStage",
                ],
                &[
                    "contractReview.created",
                    "contractReview.started",
                    "contractReview.stageChanged",
                    "contractReview.findingAdded",
                    "contractReview.findingUpdated",
                    "contractReview.findingDecided",
                    "contractReview.completed",
                    "contractReview.failed",
                    "contractReview.cancelled",
                ],
                &[
                    "contractReview.read",
                    "contractReview.write",
                    "contractReview.run",
                    "contractReview.decide",
                    "contractReview.cancel",
                    "asset.read",
                    "brain.turn",
                ],
                &[
                    "contractReview.get",
                    "contractReview.list",
                    "contractReview.start",
                    "contractReview.decideFinding",
                ],
                &[
                    "sqlite.assets",
                    "vault.originals",
                    "sqlite.contract_review_sessions",
                    "sqlite.contract_review_evidence",
                    "sqlite.contract_review_findings",
                    "sqlite.contract_review_rule_evaluations",
                    "sqlite.contract_review_decisions",
                    "sqlite.contract_review_events",
                    "sqlite.contract_review_command_receipts",
                ],
            ),
            manifest(
                "business.reviewReport",
                ModuleAvailability::Degraded,
                &["contractReview.generateReport"],
                &["contractReview.reportGenerated"],
                &[
                    "contractReview.read",
                    "reviewReport.generate",
                    "asset.read",
                    "asset.import",
                ],
                &["reviewReport.generate", "reviewReport.get"],
                &[
                    "sqlite.assets",
                    "sqlite.asset_origins",
                    "vault.originals",
                    "sqlite.contract_review_reports",
                ],
            ),
            // Local Vault and SQLite remain authoritative. R2 is only an asynchronous
            // disaster-recovery replica driven by the durable backup outbox.
            manifest(
                "vault.backup.r2",
                ModuleAvailability::Degraded,
                &[
                    "backup.queue",
                    "backup.retry",
                    "backup.cancel",
                    "backup.restore",
                ],
                &[
                    "backup.queued",
                    "backup.uploading",
                    "backup.backedUp",
                    "backup.failed",
                    "backup.cancelled",
                    "backup.restored",
                ],
                &[
                    "backup.read",
                    "backup.queue",
                    "backup.retry",
                    "backup.cancel",
                    "backup.restore",
                    "asset.read",
                ],
                &[
                    "backup.list",
                    "backup.get",
                    "backup.retry",
                    "backup.restore",
                ],
                &[
                    "vault.originals",
                    "sqlite.assets",
                    "authority.localVault",
                    "authority.sqlite",
                    "sqlite.asset_backups",
                    "sqlite.backup_events",
                    "sqlite.backup_command_receipts",
                    "backup.outbox",
                    "r2.asyncDisasterBackup",
                ],
            ),
            manifest(
                "brain.codex",
                ModuleAvailability::Degraded,
                &[
                    "brain.thread.start",
                    "brain.thread.resume",
                    "brain.turn.start",
                    "brain.turn.interrupt",
                ],
                &["brain.runtimeEvent"],
                &[
                    "brain.read",
                    "brain.turn",
                    "brain.tool.approve",
                    "brain.file.write",
                    "brain.command.execute",
                ],
                &[
                    "brain.thread.start",
                    "brain.turn.start",
                    "brain.turn.interrupt",
                ],
                &[
                    "codex.rollouts",
                    "sqlite.brain_threads",
                    "sqlite.brain_turns",
                ],
            ),
            manifest(
                "business.toolRegistry",
                ModuleAvailability::Degraded,
                &[],
                &[],
                &[
                    "business.project.read",
                    "business.artifact.read",
                    "business.document.extract",
                    "business.artifact.create",
                    "business.approval.request",
                    "business.artifact.compare",
                    "business.source.locate",
                    "business.template.read",
                    "business.calculation",
                    "business.ledger.read",
                    "business.project.write",
                    "business.task.plan",
                    "business.document.generate",
                    "business.document.validate",
                ],
                &[
                    "business.project.read",
                    "business.artifact.read",
                    "business.document.extract",
                    "business.artifact.create",
                    "business.approval.request",
                    "business.artifact.compare",
                    "business.source.locate",
                    "business.template.read",
                    "business.calculation",
                    "business.ledger.read",
                    "business.project.write",
                    "business.task.plan",
                    "business.document.generate",
                    "business.document.validate",
                ],
                &[
                    "sqlite.projects",
                    "sqlite.assets",
                    "sqlite.business_workspaces",
                    "sqlite.tasks",
                    "sqlite.business_artifact_lineage",
                    "sqlite.business_tool_generated_documents",
                    "vault.originals",
                ],
            ),
            manifest(
                "desktop.settings",
                ModuleAvailability::Degraded,
                &[
                    "settings.status",
                    "settings.openStorageLocation",
                    "settings.clearCache",
                    "settings.checkForUpdates",
                ],
                &[],
                &[
                    "settings.read",
                    "settings.openStorageLocation",
                    "settings.clearCache",
                    "settings.checkForUpdates",
                ],
                &[
                    "settings.status",
                    "settings.openStorageLocation",
                    "settings.clearCache",
                    "settings.checkForUpdates",
                ],
                &[
                    "sqlite.desktop_settings_state",
                    "sqlite.desktop_settings_command_receipts",
                    "storage.capabilities",
                ],
            ),
            manifest(
                "memory.local",
                ModuleAvailability::Planned,
                &["memory.put", "memory.delete"],
                &["memory.updated", "memory.deleted"],
                &["memory.read", "memory.write", "memory.delete"],
                &["memory.search", "memory.get"],
                &["sqlite.memory_records"],
            ),
            manifest(
                "diagnostic.outbox",
                ModuleAvailability::Planned,
                &["diagnostic.report", "diagnostic.retryUpload"],
                &["diagnostic.queued", "diagnostic.uploaded"],
                &["diagnostic.create", "diagnostic.upload"],
                &["diagnostic.list"],
                &["sqlite.diagnostic_outbox"],
            ),
            manifest(
                "security.approval",
                ModuleAvailability::Ready,
                &["approval.resolve"],
                &["approval.created", "approval.resolved"],
                &["approval.read", "approval.resolve"],
                &["approval.list", "approval.resolve"],
                &["sqlite.approvals"],
            ),
        ];
        let registry = Self { manifests };
        registry
            .validate()
            .expect("static module registry must be valid");
        registry
    }

    pub fn manifests(&self) -> Vec<ModuleManifest> {
        self.manifests.clone()
    }

    pub fn set_availability(&mut self, id: &str, availability: ModuleAvailability) {
        if let Some(manifest) = self.manifests.iter_mut().find(|item| item.id == id) {
            manifest.availability = availability;
        }
    }

    fn validate(&self) -> Result<(), String> {
        let mut ids = HashSet::new();
        let mut commands = HashSet::new();
        let mut events = HashSet::new();
        for manifest in &self.manifests {
            if manifest.id.trim().is_empty() || !ids.insert(manifest.id.as_str()) {
                return Err(format!("duplicate or empty module id: {}", manifest.id));
            }
            for command in &manifest.commands {
                if !commands.insert(command.as_str()) {
                    return Err(format!("command registered by multiple modules: {command}"));
                }
            }
            for event in &manifest.events {
                if !events.insert(event.as_str()) {
                    return Err(format!("event registered by multiple modules: {event}"));
                }
            }
        }
        Ok(())
    }
}

fn manifest(
    id: &str,
    availability: ModuleAvailability,
    commands: &[&str],
    events: &[&str],
    permissions: &[&str],
    tools: &[&str],
    storage: &[&str],
) -> ModuleManifest {
    ModuleManifest {
        id: id.to_string(),
        version: PROTOCOL_VERSION.to_string(),
        availability,
        commands: strings(commands),
        events: strings(events),
        permissions: strings(permissions),
        tools: strings(tools),
        storage: strings(storage),
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_registry_has_unique_contracts() {
        let registry = ModuleRegistry::desktop();
        registry.validate().unwrap();
        assert_eq!(registry.manifests.len(), 18);
        assert!(registry
            .manifests
            .iter()
            .any(|item| item.id == "project.production"));
        assert!(registry
            .manifests
            .iter()
            .any(|item| item.id == "media.native"));
        assert!(registry
            .manifests
            .iter()
            .any(|item| item.id == "intake.requirementBrief"));
        assert!(registry
            .manifests
            .iter()
            .any(|item| item.id == "business.documentCenter"));
        for module_id in [
            "document.intelligence",
            "business.contractReview",
            "business.reviewReport",
            "business.toolRegistry",
            "desktop.settings",
            "vault.backup.r2",
        ] {
            assert!(
                registry.manifests.iter().any(|item| item.id == module_id),
                "missing module manifest: {module_id}"
            );
        }
    }

    #[test]
    fn business_document_center_declares_protocol_1_6_contract() {
        let registry = ModuleRegistry::desktop();
        let module = registry
            .manifests
            .iter()
            .find(|item| item.id == "business.documentCenter")
            .unwrap();

        assert_eq!(module.availability, ModuleAvailability::Ready);
        assert_eq!(
            module.commands,
            strings(&[
                "businessWorkspace.create",
                "businessWorkspace.updateProfile",
                "businessWorkspace.createDocument",
                "businessWorkspace.promoteReviewedContract",
                "businessWorkspace.changeDocumentStatus",
                "businessWorkspace.generateDocument",
                "businessWorkspace.upsertPayment",
                "businessWorkspace.confirmQuote",
                "businessWorkspace.recordReceipt",
                "businessWorkspace.reverseReceipt",
                "businessWorkspace.adoptLatestConfirmedRequirement",
                "businessWorkspace.upsertCustomer",
                "businessWorkspace.assignCustomer",
                "businessWorkspace.upsertMilestone",
                "businessWorkspace.registerDeliverableVersion",
                "businessWorkspace.recordDeliverySent",
                "businessWorkspace.recordDeliverySignoff",
                "businessWorkspace.recordInvoiceIssued",
                "businessWorkspace.recordInvoiceRedCorrection",
                "businessWorkspace.attachInvoiceAsset",
                "businessWorkspace.createArchiveSnapshot",
                "businessWorkspace.changeStatus",
            ])
        );
        assert_eq!(
            module.events,
            strings(&[
                "businessWorkspace.created",
                "businessWorkspace.profileUpdated",
                "businessWorkspace.documentCreated",
                "businessWorkspace.reviewedContractPromoted",
                "businessWorkspace.documentStatusChanged",
                "businessWorkspace.documentGenerated",
                "businessWorkspace.paymentUpserted",
                "businessWorkspace.quoteConfirmed",
                "businessWorkspace.receiptRecorded",
                "businessWorkspace.receiptReversed",
                "businessWorkspace.requirementAdopted",
                "businessWorkspace.customerUpserted",
                "businessWorkspace.customerAssigned",
                "businessWorkspace.milestoneUpserted",
                "businessWorkspace.deliverableVersionRegistered",
                "businessWorkspace.deliverySent",
                "businessWorkspace.deliverySignoffRecorded",
                "businessWorkspace.invoiceIssued",
                "businessWorkspace.invoiceRedCorrected",
                "businessWorkspace.invoiceAssetAttached",
                "businessWorkspace.archiveSnapshotPrepared",
                "businessWorkspace.statusChanged",
            ])
        );
        assert_eq!(
            module.permissions,
            strings(&[
                "businessWorkspace.read",
                "businessWorkspace.write",
                "businessWorkspace.approve",
                "businessWorkspace.generate",
                "asset.read",
                "asset.import",
                "project.read",
                "requirementBrief.read",
            ])
        );
        assert_eq!(
            module.tools,
            strings(&[
                "businessWorkspace.list",
                "businessWorkspace.prefillCandidates",
                "businessWorkspace.previewPrefill",
            ])
        );
        assert_eq!(
            module.storage,
            strings(&[
                "sqlite.business_workspaces",
                "sqlite.business_customers",
                "sqlite.business_workspace_customers",
                "sqlite.business_customer_conflicts",
                "sqlite.business_customer_backfill",
                "sqlite.business_documents",
                "sqlite.business_payments",
                "sqlite.business_quote_confirmations",
                "sqlite.business_receipts",
                "sqlite.business_delivery_milestones",
                "sqlite.business_deliverable_versions",
                "sqlite.business_delivery_submissions",
                "sqlite.business_invoices",
                "sqlite.business_archive_snapshots",
                "sqlite.business_workspace_events",
                "sqlite.business_workspace_command_receipts",
                "sqlite.assets",
                "sqlite.asset_origins",
                "vault.originals",
            ])
        );
    }

    #[test]
    fn business_contract_modules_declare_frozen_protocol_contracts() {
        let registry = ModuleRegistry::desktop();
        let module = |id: &str| {
            registry
                .manifests
                .iter()
                .find(|item| item.id == id)
                .unwrap_or_else(|| panic!("missing module manifest: {id}"))
        };

        let document = module("document.intelligence");
        assert!(document.commands.is_empty());
        assert_eq!(
            document.events,
            strings(&[
                "contractReview.extractionCompleted",
                "contractReview.ocrRequired",
            ])
        );

        let review = module("business.contractReview");
        assert_eq!(
            review.commands,
            strings(&[
                "contractReview.create",
                "contractReview.start",
                "contractReview.cancel",
                "contractReview.decideFinding",
                "contractReview.retryStage",
            ])
        );
        assert_eq!(
            review.events,
            strings(&[
                "contractReview.created",
                "contractReview.started",
                "contractReview.stageChanged",
                "contractReview.findingAdded",
                "contractReview.findingUpdated",
                "contractReview.findingDecided",
                "contractReview.completed",
                "contractReview.failed",
                "contractReview.cancelled",
            ])
        );

        let report = module("business.reviewReport");
        assert_eq!(report.commands, strings(&["contractReview.generateReport"]));
        assert_eq!(report.events, strings(&["contractReview.reportGenerated"]));

        let backup = module("vault.backup.r2");
        assert_eq!(
            backup.commands,
            strings(&[
                "backup.queue",
                "backup.retry",
                "backup.cancel",
                "backup.restore",
            ])
        );
        assert_eq!(
            backup.events,
            strings(&[
                "backup.queued",
                "backup.uploading",
                "backup.backedUp",
                "backup.failed",
                "backup.cancelled",
                "backup.restored",
            ])
        );
    }

    #[test]
    fn r2_backup_contract_keeps_local_storage_authoritative() {
        let registry = ModuleRegistry::desktop();
        let backup = registry
            .manifests
            .iter()
            .find(|item| item.id == "vault.backup.r2")
            .unwrap();

        for required in [
            "vault.originals",
            "sqlite.assets",
            "authority.localVault",
            "authority.sqlite",
            "sqlite.asset_backups",
            "backup.outbox",
            "r2.asyncDisasterBackup",
        ] {
            assert!(
                backup.storage.iter().any(|item| item == required),
                "backup storage contract must declare {required}"
            );
        }

        let normalized = backup.storage.join(" ").to_ascii_lowercase();
        assert!(!normalized.contains("d1"));
        assert!(!normalized.contains("team"));
        assert!(!normalized.contains("primary"));
    }

    #[test]
    fn availability_can_be_promoted_after_runtime_startup() {
        let mut registry = ModuleRegistry::desktop();
        registry.set_availability("task.engine", ModuleAvailability::Ready);
        let task = registry
            .manifests
            .iter()
            .find(|item| item.id == "task.engine")
            .unwrap();
        assert_eq!(task.availability, ModuleAvailability::Ready);
    }
}
