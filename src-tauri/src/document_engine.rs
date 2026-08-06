use crate::protocol::{
    BusinessDocumentFormat, BusinessDocumentKind, BusinessDocumentRecord, BusinessLineItem,
    BusinessProfile, HostError,
};
use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const STAGING_DIRECTORY: &str = ".business-workspace-staging";
const RECONCILE_LOCK_FILE: &str = ".reconcile.lock";
const GENERATION_LEASE_FILE: &str = ".generation.lease";
pub(crate) const QUOTE_TEMPLATE_KEY: &str = "builtin.quote.standard.v1";
pub(crate) const CONTRACT_TEMPLATE_KEY: &str = "builtin.contract.service.v1";
pub(crate) const PAYMENT_REQUEST_TEMPLATE_KEY: &str = "builtin.payment-request.standard.v1";
pub(crate) const ACCEPTANCE_TEMPLATE_KEY: &str = "builtin.acceptance.standard.v1";
pub(crate) const BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY: &str =
    "project.baietan.acceptance.video-completion-acceptance.v1";
pub(crate) const BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY: &str =
    "project.baietan.acceptance.production-result-confirmation.v1";
pub(crate) const BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY: &str =
    "project.baietan.acceptance.service-settlement-list.v1";
pub(crate) const BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY: &str =
    "project.baietan.acceptance.contract-settlement.v1";
pub(crate) const BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY: &str =
    "project.baietan.acceptance.payment-application-settlement-calculation.v1";
const BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_SHA256: &str =
    "CF9E21CEC8C5458F709410A17350B58D066EA98F3E6F15194598EFCFAA38B5FB";
const BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_SHA256: &str =
    "7F25AB4C3F1F6F92208F44CD7360717486051A351EE0202DAFCB621DA275C7BF";
const BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_MAPPING_VERSION: &str =
    "baietan.video-completion-acceptance.v1";
const BAIETAN_PRODUCTION_RESULT_CONFIRMATION_MAPPING_VERSION: &str =
    "baietan.production-result-confirmation.v1";
const BAIETAN_SERVICE_SETTLEMENT_LIST_MAPPING_VERSION: &str = "baietan.service-settlement-list.v1";
const BAIETAN_CONTRACT_SETTLEMENT_MAPPING_VERSION: &str = "baietan.contract-settlement.v1";
const BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_MAPPING_VERSION: &str =
    "baietan.payment-application-settlement-calculation.v1";

struct TemplateRegistration {
    kind: BusinessDocumentKind,
    allowed_format: Option<BusinessDocumentFormat>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DocumentGenerationResources<'a> {
    pub(crate) video_completion_acceptance: Option<
        &'a crate::business_v1::video_completion_acceptance_template::VideoCompletionAcceptanceTemplateData,
    >,
    pub(crate) production_result_confirmation: Option<
        &'a crate::business_v1::production_result_confirmation_template::ProductionResultConfirmationTemplateData,
    >,
}

pub(crate) fn template_requires_source_asset(template_key: &str) -> bool {
    matches!(
        template_key,
        BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY
            | BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY
            | BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY
            | BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY
            | BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY
    )
}

pub(crate) fn expected_template_source_sha256(template_key: &str) -> Option<&'static str> {
    match template_key {
        BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY => {
            Some(BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_SHA256)
        }
        BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY => {
            Some(BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_SHA256)
        }
        BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY => {
            Some(crate::business_v1::acceptance_docx_template::SERVICE_SETTLEMENT_TEMPLATE_SHA256)
        }
        BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY => {
            Some(crate::business_v1::acceptance_xlsx_template::CONTRACT_SETTLEMENT_TEMPLATE_SHA256)
        }
        _ => None,
    }
}

pub(crate) fn expected_template_mapping_version(template_key: &str) -> Option<&'static str> {
    match template_key {
        BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY => {
            Some(BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_MAPPING_VERSION)
        }
        BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY => {
            Some(BAIETAN_PRODUCTION_RESULT_CONFIRMATION_MAPPING_VERSION)
        }
        BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY => {
            Some(BAIETAN_SERVICE_SETTLEMENT_LIST_MAPPING_VERSION)
        }
        BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY => {
            Some(BAIETAN_CONTRACT_SETTLEMENT_MAPPING_VERSION)
        }
        BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY => {
            Some(BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_MAPPING_VERSION)
        }
        _ => None,
    }
}

pub(crate) fn preflight_normalized_template(
    template_key: &str,
    source: &[u8],
    expected_sha256: &str,
) -> Result<(), HostError> {
    if template_key == BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY {
        return crate::business_v1::payment_application_template::validate_payment_application_template_source_from_bytes(
            source,
            expected_sha256,
        );
    }
    Ok(())
}

/// A generated OOXML package whose native path is backend-only. Dropping the
/// value removes the complete staging directory, including partial output.
#[derive(Debug)]
pub struct StagedDocument {
    path: PathBuf,
    staging_root: PathBuf,
    staging_directory: PathBuf,
    lease: Option<File>,
}

impl StagedDocument {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) fn stage_normalized_template(
    vault_root: &Path,
    template_version_id: &str,
) -> Result<StagedDocument, HostError> {
    let template_version_id = Uuid::parse_str(template_version_id)
        .map(|value| value.to_string())
        .map_err(|_| HostError::validation("templateVersionId must be a UUID"))?;
    fs::create_dir_all(vault_root).map_err(document_io_error("create Vault root"))?;
    let vault_root =
        fs::canonicalize(vault_root).map_err(document_io_error("resolve Vault root"))?;
    if !vault_root.is_dir() {
        return Err(HostError::new(
            "VAULT_INVALID",
            "Vault root is not a directory",
            false,
        ));
    }

    let staging_root = prepare_staging_root(&vault_root)?;
    let reconcile_lock = open_lock_file(&staging_root.join(RECONCILE_LOCK_FILE))?;
    reconcile_lock
        .lock_exclusive()
        .map_err(document_io_error("lock template staging creation"))?;
    let staging_directory = staging_root.join(Uuid::new_v4().to_string());
    fs::create_dir(&staging_directory).map_err(document_io_error(
        "create normalized template staging directory",
    ))?;
    let staging_directory = validate_staging_subdirectory(&staging_root, &staging_directory)?;
    let lease = open_lock_file(&staging_directory.join(GENERATION_LEASE_FILE))?;
    lease
        .lock_exclusive()
        .map_err(document_io_error("lock active template normalization"))?;
    FileExt::unlock(&reconcile_lock)
        .map_err(document_io_error("unlock template staging creation"))?;

    Ok(StagedDocument {
        path: staging_directory.join(format!(
            "bsaigc-normalized-template-{template_version_id}.docx"
        )),
        staging_root,
        staging_directory,
        lease: Some(lease),
    })
}

impl Drop for StagedDocument {
    fn drop(&mut self) {
        if let Some(lease) = self.lease.take() {
            let _ = FileExt::unlock(&lease);
            drop(lease);
        }
        let _ = remove_staging_directory_if_safe(
            &self.staging_root,
            &self.staging_directory,
            "remove staged document directory",
        );
    }
}

pub(crate) fn reconcile_staging(vault_root: &Path) -> Result<(), HostError> {
    fs::create_dir_all(vault_root).map_err(document_io_error("create Vault root"))?;
    let vault_root =
        fs::canonicalize(vault_root).map_err(document_io_error("resolve Vault root"))?;
    let staging_root = prepare_staging_root(&vault_root)?;
    let reconcile_lock = open_lock_file(&staging_root.join(RECONCILE_LOCK_FILE))?;
    reconcile_lock
        .lock_exclusive()
        .map_err(document_io_error("lock document staging reconciliation"))?;
    for entry in
        fs::read_dir(&staging_root).map_err(document_io_error("read document staging root"))?
    {
        let entry = entry.map_err(document_io_error("read document staging entry"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(document_io_error("inspect document staging entry"))?;
        if is_link_or_reparse(&metadata) {
            return Err(vault_path_error(
                "document staging entry cannot be a symlink or reparse point",
            ));
        }
        if !metadata.is_dir() {
            continue;
        }
        let staging_directory = validate_staging_subdirectory(&staging_root, &entry.path())?;
        let lease_path = staging_directory.join(GENERATION_LEASE_FILE);
        let lease = match open_existing_lock_file(&lease_path)? {
            Some(lease) => lease,
            None => {
                remove_staging_directory_if_safe(
                    &staging_root,
                    &staging_directory,
                    "remove interrupted staging directory",
                )?;
                continue;
            }
        };
        match lease.try_lock_exclusive() {
            Ok(()) => {
                FileExt::unlock(&lease)
                    .map_err(document_io_error("unlock interrupted generation lease"))?;
                drop(lease);
                remove_staging_directory_if_safe(
                    &staging_root,
                    &staging_directory,
                    "remove interrupted staging directory",
                )?;
            }
            Err(error) if is_lock_contended(&error) => {}
            Err(error) => return Err(document_io_error("inspect generation lease")(error)),
        }
    }
    FileExt::unlock(&reconcile_lock)
        .map_err(document_io_error("unlock document staging reconciliation"))?;
    Ok(())
}

pub(crate) fn generation_is_active(
    vault_root: &Path,
    generated_name: &str,
) -> Result<bool, HostError> {
    if Path::new(generated_name)
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Ok(false);
    }
    fs::create_dir_all(vault_root).map_err(document_io_error("create Vault root"))?;
    let vault_root =
        fs::canonicalize(vault_root).map_err(document_io_error("resolve Vault root"))?;
    let staging_root = prepare_staging_root(&vault_root)?;
    let reconcile_lock = open_lock_file(&staging_root.join(RECONCILE_LOCK_FILE))?;
    reconcile_lock
        .lock_exclusive()
        .map_err(document_io_error("lock active generation lookup"))?;
    let mut active = false;
    for entry in
        fs::read_dir(&staging_root).map_err(document_io_error("read document staging root"))?
    {
        let entry = entry.map_err(document_io_error("read document staging entry"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(document_io_error("inspect document staging entry"))?;
        if is_link_or_reparse(&metadata) {
            return Err(vault_path_error(
                "document staging entry cannot be a symlink or reparse point",
            ));
        }
        if !metadata.is_dir() {
            continue;
        }
        let staging_directory = validate_staging_subdirectory(&staging_root, &entry.path())?;
        if !staging_directory.join(generated_name).is_file() {
            continue;
        }
        let Some(lease) = open_existing_lock_file(&staging_directory.join(GENERATION_LEASE_FILE))?
        else {
            continue;
        };
        match lease.try_lock_exclusive() {
            Ok(()) => {
                FileExt::unlock(&lease)
                    .map_err(document_io_error("unlock inactive generation lease"))?;
            }
            Err(error) if is_lock_contended(&error) => {
                active = true;
                break;
            }
            Err(error) => return Err(document_io_error("inspect generation lease")(error)),
        }
    }
    FileExt::unlock(&reconcile_lock)
        .map_err(document_io_error("unlock active generation lookup"))?;
    Ok(active)
}

#[cfg(test)]
pub(crate) fn generate_document(
    vault_root: &Path,
    document: &BusinessDocumentRecord,
    format: &BusinessDocumentFormat,
) -> Result<StagedDocument, HostError> {
    generate_document_with_template(vault_root, document, format, None)
}

#[allow(dead_code)]
pub(crate) fn generate_document_with_template(
    vault_root: &Path,
    document: &BusinessDocumentRecord,
    format: &BusinessDocumentFormat,
    template_source: Option<&[u8]>,
) -> Result<StagedDocument, HostError> {
    generate_document_with_template_and_resources(
        vault_root,
        document,
        format,
        template_source,
        DocumentGenerationResources::default(),
    )
}

pub(crate) fn generate_document_with_template_and_resources(
    vault_root: &Path,
    document: &BusinessDocumentRecord,
    format: &BusinessDocumentFormat,
    template_source: Option<&[u8]>,
    resources: DocumentGenerationResources<'_>,
) -> Result<StagedDocument, HostError> {
    validate_template_for_format(&document.kind, &document.template_key, format)?;
    if template_requires_source_asset(&document.template_key) && template_source.is_none() {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_RENDERING_NOT_READY",
            "registered customer templates require the dedicated template renderer",
            false,
        ));
    }
    if document.template_key == BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY
        && resources.video_completion_acceptance.is_none()
    {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_RENDERING_NOT_READY",
            "video completion acceptance rendering resources are required",
            false,
        ));
    }
    if document.template_key == BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY
        && resources.production_result_confirmation.is_none()
    {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_RENDERING_NOT_READY",
            "production result confirmation rendering resources are required",
            false,
        ));
    }
    if document.template_key != BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY
        && resources.video_completion_acceptance.is_some()
    {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_DATA_UNEXPECTED",
            "video completion acceptance resources are only valid for their registered template",
            false,
        ));
    }
    if document.template_key != BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY
        && resources.production_result_confirmation.is_some()
    {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_DATA_UNEXPECTED",
            "production result confirmation resources are only valid for their registered template",
            false,
        ));
    }
    fs::create_dir_all(vault_root).map_err(document_io_error("create Vault root"))?;
    let vault_root =
        fs::canonicalize(vault_root).map_err(document_io_error("resolve Vault root"))?;
    if !vault_root.is_dir() {
        return Err(HostError::new(
            "VAULT_INVALID",
            "Vault root is not a directory",
            false,
        ));
    }

    let staging_root = prepare_staging_root(&vault_root)?;
    let reconcile_lock = open_lock_file(&staging_root.join(RECONCILE_LOCK_FILE))?;
    reconcile_lock
        .lock_exclusive()
        .map_err(document_io_error("lock document staging creation"))?;
    let staging_directory = staging_root.join(Uuid::new_v4().to_string());
    fs::create_dir(&staging_directory).map_err(document_io_error(
        "create business document staging directory",
    ))?;
    let staging_directory = validate_staging_subdirectory(&staging_root, &staging_directory)?;
    let lease = open_lock_file(&staging_directory.join(GENERATION_LEASE_FILE))?;
    lease
        .lock_exclusive()
        .map_err(document_io_error("lock active document generation"))?;
    FileExt::unlock(&reconcile_lock)
        .map_err(document_io_error("unlock document staging creation"))?;

    let extension = match format {
        BusinessDocumentFormat::Docx => "docx",
        BusinessDocumentFormat::Xlsx => "xlsx",
    };
    // The marker makes an interrupted import reconcilable after restart. It is
    // an asset display name, never a local path.
    let path = staging_directory.join(format!(
        "bsaigc-business-document-{}.{}",
        document.id, extension
    ));
    let staged = StagedDocument {
        path,
        staging_root,
        staging_directory,
        lease: Some(lease),
    };
    let result = match document.template_key.as_str() {
        BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY => {
            render_video_completion_acceptance_template(
                template_source.expect("source template checked above"),
                staged.path(),
                document,
                resources,
            )
        }
        BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY => {
            render_production_result_confirmation_template(
                template_source.expect("source template checked above"),
                staged.path(),
                document,
                resources,
            )
        }
        BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY => render_contract_settlement_template(
            template_source.expect("source template checked above"),
            staged.path(),
            document,
        ),
        BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY => render_service_settlement_template(
            template_source.expect("source template checked above"),
            staged.path(),
            document,
        ),
        BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY => {
            render_payment_application_template(
                template_source.expect("source template checked above"),
                staged.path(),
                document,
            )
        }
        template_key if template_requires_source_asset(template_key) => Err(HostError::new(
            "BUSINESS_TEMPLATE_RENDERING_NOT_READY",
            "registered customer template renderer is not implemented",
            false,
        )),
        _ => match format {
            BusinessDocumentFormat::Docx => write_docx(staged.path(), document),
            BusinessDocumentFormat::Xlsx => write_xlsx(staged.path(), document),
        },
    };
    result?;
    Ok(staged)
}

fn render_video_completion_acceptance_template(
    source: &[u8],
    destination: &Path,
    document: &BusinessDocumentRecord,
    resources: DocumentGenerationResources<'_>,
) -> Result<(), HostError> {
    let data = resources.video_completion_acceptance.ok_or_else(|| {
        HostError::new(
            "BUSINESS_TEMPLATE_RENDERING_NOT_READY",
            "video completion acceptance rendering resources are required",
            false,
        )
    })?;
    let expected_sha256 = document
        .snapshot
        .template_source_sha256
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_TEMPLATE_SOURCE_REQUIRED",
                "video completion acceptance document snapshot is missing template SHA-256",
                false,
            )
        })?;
    crate::business_v1::video_completion_acceptance_template::render_video_completion_acceptance_template_from_bytes(
        source,
        expected_sha256,
        destination,
        data,
    )
}

fn render_production_result_confirmation_template(
    source: &[u8],
    destination: &Path,
    document: &BusinessDocumentRecord,
    resources: DocumentGenerationResources<'_>,
) -> Result<(), HostError> {
    let data = resources.production_result_confirmation.ok_or_else(|| {
        HostError::new(
            "BUSINESS_TEMPLATE_RENDERING_NOT_READY",
            "production result confirmation rendering resources are required",
            false,
        )
    })?;
    let expected_sha256 = document
        .snapshot
        .template_source_sha256
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_TEMPLATE_SOURCE_REQUIRED",
                "production result confirmation document snapshot is missing template SHA-256",
                false,
            )
        })?;
    crate::business_v1::production_result_confirmation_template::render_production_result_confirmation_template_from_bytes(
        source,
        expected_sha256,
        destination,
        data,
    )
}

fn render_contract_settlement_template(
    source: &[u8],
    destination: &Path,
    document: &BusinessDocumentRecord,
) -> Result<(), HostError> {
    let settlement = document
        .snapshot
        .contract_settlement
        .as_ref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_CONTRACT_SETTLEMENT_DATA_REQUIRED",
                "contract settlement document snapshot is missing frozen settlement data",
                false,
            )
        })?;
    let expected_sha256 = document
        .snapshot
        .template_source_sha256
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_TEMPLATE_SOURCE_REQUIRED",
                "contract settlement document snapshot is missing template SHA-256",
                false,
            )
        })?;
    let data = crate::business_v1::acceptance_xlsx_template::ContractSettlementTemplateData {
        project_title: document.snapshot.profile.project_title.clone(),
        contract_title: settlement.contract_title.clone(),
        contract_number: settlement.contract_number.clone(),
        customer_legal_name: document.snapshot.profile.customer_legal_name.clone(),
        supplier_legal_name: document.snapshot.profile.supplier_legal_name.clone(),
        original_contract_amount_cents: settlement.original_contract_amount_cents,
        contract_adjustment_cents: settlement.contract_adjustment_cents,
        retention_rate_bps: settlement.retention_rate_bps,
        final_settlement_amount_cents: settlement.final_settlement_amount_cents,
        final_settlement_amount_uppercase_cny:
            crate::business_v1::acceptance_xlsx_template::uppercase_cny(
                settlement.final_settlement_amount_cents,
            )?,
    };
    crate::business_v1::acceptance_xlsx_template::clone_contract_settlement_template_from_bytes(
        source,
        expected_sha256,
        destination,
        &data,
    )
}

fn render_service_settlement_template(
    source: &[u8],
    destination: &Path,
    document: &BusinessDocumentRecord,
) -> Result<(), HostError> {
    if document.snapshot.service_settlement_items.is_empty() {
        return Err(HostError::new(
            "BUSINESS_SERVICE_SETTLEMENT_DATA_REQUIRED",
            "service settlement document snapshot is missing frozen service rows",
            false,
        ));
    }
    let expected_sha256 = document
        .snapshot
        .template_source_sha256
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_TEMPLATE_SOURCE_REQUIRED",
                "service settlement document snapshot is missing template SHA-256",
                false,
            )
        })?;
    let data = crate::business_v1::acceptance_docx_template::ServiceSettlementTemplateData {
        items: document
            .snapshot
            .service_settlement_items
            .iter()
            .map(
                |item| crate::business_v1::acceptance_docx_template::ServiceSettlementItem {
                    service_name: item.service_name.clone(),
                    period: item.period.clone(),
                    description: item.description.clone(),
                    provided_as_required: item.provided_as_required,
                    evidence_label: item.evidence_label.clone(),
                    remarks: item.remarks.clone(),
                },
            )
            .collect(),
    };
    crate::business_v1::acceptance_docx_template::clone_service_settlement_template_from_bytes(
        source,
        expected_sha256,
        destination,
        &data,
    )
}

fn render_payment_application_template(
    source: &[u8],
    destination: &Path,
    document: &BusinessDocumentRecord,
) -> Result<(), HostError> {
    let frozen = document
        .snapshot
        .payment_application
        .as_ref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_PAYMENT_APPLICATION_DATA_REQUIRED",
                "payment application document snapshot is missing frozen data",
                false,
            )
        })?;
    let expected_sha256 = document
        .snapshot
        .template_source_sha256
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_TEMPLATE_SOURCE_REQUIRED",
                "payment application document snapshot is missing template SHA-256",
                false,
            )
        })?;
    let profile = &document.snapshot.profile;
    let data = crate::business_v1::payment_application_template::PaymentApplicationTemplateData {
        customer_legal_name: profile.customer_legal_name.clone(),
        project_title: profile.project_title.clone(),
        contract_title: frozen.contract_title.clone(),
        contract_number: frozen.contract_number.clone(),
        supplier_legal_name: profile.supplier_legal_name.clone(),
        work_summary: frozen.work_summary.clone(),
        payment_period_start: frozen.payment_period_start.clone(),
        payment_period_end: frozen.payment_period_end.clone(),
        settlement_period: frozen.settlement_period.clone(),
        payment_sequence: frozen.payment_sequence,
        invoice_amount_cents: frozen.invoice_amount_cents,
        cumulative_recognized_amount_cents: frozen.cumulative_recognized_amount_cents,
        payable_amount_cents: frozen.remaining_payable_cents,
        withheld_amount_cents: frozen.withheld_amount_cents,
        cumulative_paid_cents: frozen.cumulative_paid_cents,
        application_date: frozen.application_date.clone(),
        bank_account: crate::business_v1::payment_application_template::PaymentBankAccount {
            recipient_name: profile.supplier_legal_name.clone(),
            bank_name: profile.supplier_bank_name.clone(),
            account_number: profile.supplier_bank_account.clone(),
            routing_number: frozen.supplier_bank_routing_number.clone(),
        },
        settlement_items: frozen
            .settlement_items
            .iter()
            .map(
                |item| crate::business_v1::payment_application_template::PaymentSettlementItem {
                    name: item.name.clone(),
                    unit: item.unit.clone(),
                    contract_unit_price_cents: item.contract_unit_price_cents,
                    original_quantity_millis: item.original_quantity_millis,
                    settlement_quantity_millis: item.settlement_quantity_millis,
                    remarks: item.remarks.clone(),
                },
            )
            .collect(),
    };
    if data.settlement_total_cents()? != frozen.settlement_total_cents
        || data.remaining_payable_cents()? != frozen.remaining_payable_cents
    {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_REMAINING_PAYABLE_MISMATCH",
            "renderer input does not match the frozen payment totals",
            false,
        ));
    }
    crate::business_v1::payment_application_template::render_payment_application_template_from_bytes(
        source,
        expected_sha256,
        destination,
        &data,
    )
}

pub(crate) fn validate_template(
    kind: &BusinessDocumentKind,
    template_key: &str,
) -> Result<(), HostError> {
    validate_registered_template(kind, template_key, None)
}

pub(crate) fn validate_template_for_format(
    kind: &BusinessDocumentKind,
    template_key: &str,
    format: &BusinessDocumentFormat,
) -> Result<(), HostError> {
    validate_format(kind, format)?;
    validate_registered_template(kind, template_key, Some(format))
}

fn validate_registered_template(
    kind: &BusinessDocumentKind,
    template_key: &str,
    format: Option<&BusinessDocumentFormat>,
) -> Result<(), HostError> {
    let registration = match template_key {
        QUOTE_TEMPLATE_KEY => TemplateRegistration {
            kind: BusinessDocumentKind::Quote,
            allowed_format: Some(BusinessDocumentFormat::Xlsx),
        },
        CONTRACT_TEMPLATE_KEY => TemplateRegistration {
            kind: BusinessDocumentKind::Contract,
            allowed_format: Some(BusinessDocumentFormat::Docx),
        },
        PAYMENT_REQUEST_TEMPLATE_KEY => TemplateRegistration {
            kind: BusinessDocumentKind::PaymentRequest,
            allowed_format: Some(BusinessDocumentFormat::Docx),
        },
        ACCEPTANCE_TEMPLATE_KEY => TemplateRegistration {
            kind: BusinessDocumentKind::Acceptance,
            allowed_format: None,
        },
        BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY
        | BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY
        | BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY
        | BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY => TemplateRegistration {
            kind: BusinessDocumentKind::Acceptance,
            allowed_format: Some(BusinessDocumentFormat::Docx),
        },
        BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY => TemplateRegistration {
            kind: BusinessDocumentKind::Acceptance,
            allowed_format: Some(BusinessDocumentFormat::Xlsx),
        },
        _ => {
            return Err(HostError::new(
                "BUSINESS_TEMPLATE_UNKNOWN",
                format!("business document template is not registered: {template_key}"),
                false,
            ));
        }
    };
    if &registration.kind != kind {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_KIND_MISMATCH",
            "business document template is registered for a different document kind",
            false,
        ));
    }
    if let (Some(actual), Some(allowed)) = (format, registration.allowed_format.as_ref()) {
        if actual != allowed {
            return Err(HostError::new(
                "BUSINESS_TEMPLATE_FORMAT_MISMATCH",
                "business document template does not allow the requested output format",
                false,
            ));
        }
    }
    Ok(())
}

fn open_lock_file(path: &Path) -> Result<File, HostError> {
    open_lock_file_with_mode(path, true)?.ok_or_else(|| {
        HostError::new(
            "DOCUMENT_IO_ERROR",
            "document staging lock was not created",
            true,
        )
    })
}

fn open_existing_lock_file(path: &Path) -> Result<Option<File>, HostError> {
    open_lock_file_with_mode(path, false)
}

fn open_lock_file_with_mode(
    path: &Path,
    create_if_missing: bool,
) -> Result<Option<File>, HostError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => validate_lock_metadata(&metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create_if_missing => {
            return Ok(None);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(document_io_error("inspect document staging lock")(error)),
    }

    let file = match OpenOptions::new()
        .create(create_if_missing)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create_if_missing => {
            return Ok(None);
        }
        Err(error) => return Err(document_io_error("open document staging lock")(error)),
    };

    validate_lock_metadata(
        &file
            .metadata()
            .map_err(document_io_error("inspect opened document staging lock"))?,
    )?;
    let path_metadata = fs::symlink_metadata(path)
        .map_err(document_io_error("inspect document staging lock path"))?;
    validate_lock_metadata(&path_metadata)?;
    Ok(Some(file))
}

fn validate_lock_metadata(metadata: &fs::Metadata) -> Result<(), HostError> {
    if is_link_or_reparse(metadata) || !metadata.is_file() {
        Err(vault_path_error(
            "document staging lock must be a regular file",
        ))
    } else {
        Ok(())
    }
}

fn prepare_staging_root(vault_root: &Path) -> Result<PathBuf, HostError> {
    let candidate = vault_root.join(STAGING_DIRECTORY);
    if let Ok(metadata) = fs::symlink_metadata(&candidate) {
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(vault_path_error(
                "document staging root must be a regular directory",
            ));
        }
    } else {
        fs::create_dir(&candidate)
            .map_err(document_io_error("create business document staging root"))?;
    }
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(document_io_error("inspect business document staging root"))?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(vault_path_error(
            "document staging root must be a regular directory",
        ));
    }
    let resolved = fs::canonicalize(&candidate)
        .map_err(document_io_error("resolve business document staging root"))?;
    if resolved.parent() != Some(vault_root) || !resolved.starts_with(vault_root) {
        return Err(vault_path_error("document staging root escapes Vault"));
    }
    Ok(resolved)
}

fn validate_staging_subdirectory(
    staging_root: &Path,
    candidate: &Path,
) -> Result<PathBuf, HostError> {
    let metadata = fs::symlink_metadata(candidate)
        .map_err(document_io_error("inspect document staging directory"))?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(vault_path_error(
            "document staging directory must be a regular directory",
        ));
    }
    let resolved = fs::canonicalize(candidate)
        .map_err(document_io_error("resolve document staging directory"))?;
    if resolved.parent() != Some(staging_root) || !resolved.starts_with(staging_root) {
        return Err(vault_path_error("document staging directory escapes Vault"));
    }
    Ok(resolved)
}

fn remove_staging_directory_if_safe(
    staging_root: &Path,
    candidate: &Path,
    action: &'static str,
) -> Result<(), HostError> {
    match fs::symlink_metadata(candidate) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(document_io_error(
                "inspect staging directory before removal",
            )(error))
        }
    }
    let resolved = validate_staging_subdirectory(staging_root, candidate)?;
    fs::remove_dir_all(resolved).map_err(document_io_error(action))
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn vault_path_error(message: impl Into<String>) -> HostError {
    HostError::new("VAULT_PATH_INVALID", message, false)
}

fn is_lock_contended(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
        || cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33))
}

fn validate_format(
    kind: &BusinessDocumentKind,
    format: &BusinessDocumentFormat,
) -> Result<(), HostError> {
    let valid = matches!(
        (kind, format),
        (BusinessDocumentKind::Quote, BusinessDocumentFormat::Xlsx)
            | (
                BusinessDocumentKind::Contract | BusinessDocumentKind::PaymentRequest,
                BusinessDocumentFormat::Docx
            )
            | (BusinessDocumentKind::Acceptance, _)
    );
    if valid {
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_DOCUMENT_FORMAT_INVALID",
            "quote documents require XLSX; contract and payment request documents require DOCX",
            false,
        ))
    }
}

fn write_docx(path: &Path, document: &BusinessDocumentRecord) -> Result<(), HostError> {
    let file = File::create(path).map_err(document_io_error("create DOCX"))?;
    let mut archive = ZipWriter::new(file);
    write_entry(&mut archive, "[Content_Types].xml", DOCX_CONTENT_TYPES)?;
    write_entry(&mut archive, "_rels/.rels", DOCX_ROOT_RELS)?;
    write_entry(&mut archive, "docProps/app.xml", DOCX_APP)?;
    write_entry(&mut archive, "docProps/core.xml", &docx_core(document))?;
    write_entry(&mut archive, "word/styles.xml", DOCX_STYLES)?;
    write_entry(&mut archive, "word/document.xml", &docx_document(document)?)?;
    archive
        .finish()
        .map_err(document_zip_error("finish DOCX"))?
        .sync_all()
        .map_err(document_io_error("sync DOCX"))
}

fn write_xlsx(path: &Path, document: &BusinessDocumentRecord) -> Result<(), HostError> {
    let file = File::create(path).map_err(document_io_error("create XLSX"))?;
    let mut archive = ZipWriter::new(file);
    write_entry(&mut archive, "[Content_Types].xml", XLSX_CONTENT_TYPES)?;
    write_entry(&mut archive, "_rels/.rels", XLSX_ROOT_RELS)?;
    write_entry(&mut archive, "docProps/app.xml", XLSX_APP)?;
    write_entry(&mut archive, "docProps/core.xml", &xlsx_core(document))?;
    write_entry(&mut archive, "xl/workbook.xml", XLSX_WORKBOOK)?;
    write_entry(
        &mut archive,
        "xl/_rels/workbook.xml.rels",
        XLSX_WORKBOOK_RELS,
    )?;
    write_entry(&mut archive, "xl/styles.xml", XLSX_STYLES)?;
    write_entry(
        &mut archive,
        "xl/worksheets/sheet1.xml",
        &xlsx_sheet(document)?,
    )?;
    archive
        .finish()
        .map_err(document_zip_error("finish XLSX"))?
        .sync_all()
        .map_err(document_io_error("sync XLSX"))
}

fn write_entry(archive: &mut ZipWriter<File>, name: &str, contents: &str) -> Result<(), HostError> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o644);
    archive
        .start_file(name, options)
        .map_err(document_zip_error("start OOXML entry"))?;
    archive
        .write_all(contents.as_bytes())
        .map_err(document_io_error("write OOXML entry"))
}

fn docx_core(document: &BusinessDocumentRecord) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{}</dc:title><dc:creator>华邦互娱商务系统</dc:creator><cp:lastModifiedBy>华邦互娱商务系统</cp:lastModifiedBy><dcterms:created xsi:type="dcterms:W3CDTF">2026-01-01T00:00:00Z</dcterms:created><dcterms:modified xsi:type="dcterms:W3CDTF">2026-01-01T00:00:00Z</dcterms:modified></cp:coreProperties>"#,
        xml(&document.title)
    )
}

fn xlsx_core(document: &BusinessDocumentRecord) -> String {
    docx_core(document)
}

fn docx_document(document: &BusinessDocumentRecord) -> Result<String, HostError> {
    let profile = &document.snapshot.profile;
    let mut body = String::new();
    body.push_str(&paragraph(&document.title, Some("Title")));
    body.push_str(&paragraph(
        &format!("Document No.: {}", document.document_number),
        None,
    ));
    body.push_str(&paragraph(
        &format!("Project: {}", profile.project_title),
        None,
    ));
    body.push_str(&party_details(profile));
    match document.kind {
        BusinessDocumentKind::Contract => {
            body.push_str(&paragraph("Service Period", Some("Heading1")));
            body.push_str(&paragraph(
                &format!(
                    "Service Start (Unix ms): {}",
                    optional_timestamp(profile.service_start_at)
                ),
                None,
            ));
            body.push_str(&paragraph(
                &format!(
                    "Service End (Unix ms): {}",
                    optional_timestamp(profile.service_end_at)
                ),
                None,
            ));
            body.push_str(&paragraph("Payment Terms", Some("Heading1")));
            body.push_str(&paragraph(&profile.payment_terms, None));
            body.push_str(&paragraph("Acceptance Terms", Some("Heading1")));
            body.push_str(&paragraph(&profile.acceptance_terms, None));
        }
        BusinessDocumentKind::PaymentRequest => {
            let payment = document.snapshot.payment.as_ref().ok_or_else(|| {
                HostError::new(
                    "BUSINESS_PAYMENT_REQUIRED",
                    "paymentRequest document snapshot is missing payment",
                    false,
                )
            })?;
            body.push_str(&paragraph("Payment Request", Some("Heading1")));
            body.push_str(&paragraph(
                &format!("Payment Label: {}", payment.label),
                None,
            ));
            body.push_str(&paragraph(
                &format!(
                    "Payment Due (Unix ms): {}",
                    optional_timestamp(payment.due_at)
                ),
                None,
            ));
            body.push_str(&paragraph(
                &format!("Payment Reference: {}", payment.reference),
                None,
            ));
            body.push_str(&paragraph(
                &format!("Receiving Bank: {}", profile.supplier_bank_name),
                None,
            ));
            body.push_str(&paragraph(
                &format!("Receiving Account: {}", profile.supplier_bank_account),
                None,
            ));
            body.push_str(&paragraph(
                &format!(
                    "Amount Due: {} {}",
                    profile.currency,
                    format_cents(payment.amount_cents)
                ),
                None,
            ));
            body.push_str(&paragraph(
                &format!(
                    "Amount Due in words: {}",
                    total_in_words(&profile.currency, payment.amount_cents)
                ),
                None,
            ));
            body.push_str(&paragraph("Payment Terms", Some("Heading1")));
            body.push_str(&paragraph(&profile.payment_terms, None));
        }
        BusinessDocumentKind::Acceptance => {
            body.push_str(&paragraph("Delivery Summary", Some("Heading1")));
            body.push_str(&paragraph(&profile.delivery_summary, None));
            body.push_str(&paragraph("Acceptance Terms", Some("Heading1")));
            body.push_str(&paragraph(&profile.acceptance_terms, None));
        }
        BusinessDocumentKind::Quote => unreachable!("quote output is XLSX"),
    }
    if document.kind != BusinessDocumentKind::PaymentRequest {
        body.push_str(&line_item_table(
            &profile.line_items,
            &profile.currency,
            document_totals(profile)?,
        ));
    }
    if !profile.notes.is_empty() {
        body.push_str(&paragraph("Notes", Some("Heading1")));
        body.push_str(&paragraph(&profile.notes, None));
    }
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    ))
}

fn party_details(profile: &crate::protocol::BusinessProfile) -> String {
    let mut body = paragraph("Parties", Some("Heading1"));
    for value in [
        format!("Customer: {}", display_customer(profile)),
        format!("Customer Tax ID: {}", profile.customer_tax_id),
        format!("Customer Address: {}", profile.customer_address),
        format!(
            "Customer Contact: {} / {} / {}",
            profile.customer_contact, profile.customer_phone, profile.customer_email
        ),
        format!("Supplier: {}", profile.supplier_legal_name),
        format!("Supplier Tax ID: {}", profile.supplier_tax_id),
        format!("Supplier Address: {}", profile.supplier_address),
        format!(
            "Supplier Contact: {} / {}",
            profile.supplier_contact, profile.supplier_phone
        ),
    ] {
        body.push_str(&paragraph(&value, None));
    }
    body
}

fn paragraph(text: &str, style: Option<&str>) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    let style = style
        .map(|value| format!(r#"<w:pPr><w:pStyle w:val="{}"/></w:pPr>"#, xml(value)))
        .unwrap_or_default();
    format!(
        r#"<w:p>{style}<w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
        xml(text)
    )
}

fn line_item_table(items: &[BusinessLineItem], currency: &str, totals: Totals) -> String {
    let mut rows = String::new();
    rows.push_str(&word_row(&[
        "Item",
        "Description",
        "Quantity",
        "Unit",
        "Unit Price",
        "Tax Rate",
        "Amount",
        "Tax Amount",
    ]));
    for item in items {
        rows.push_str(&word_row(&[
            &item.name,
            &item.description,
            &format_millis(item.quantity_millis),
            &item.unit,
            &format!("{} {}", currency, format_cents(item.unit_price_cents)),
            &format!("{}%", scaled_decimal(item.tax_rate_bps, 2)),
            &format!("{} {}", currency, format_cents(item.amount_cents)),
            &format!("{} {}", currency, format_cents(line_tax_cents(item))),
        ]));
    }
    rows.push_str(&word_row(&[
        "Original Total",
        "",
        "",
        "",
        "",
        "",
        "",
        &format!("{} {}", currency, format_cents(totals.original_total_cents)),
    ]));
    rows.push_str(&word_row(&[
        "Project Discount",
        "",
        "",
        "",
        "",
        "",
        "",
        &format!(
            "{} {}",
            currency,
            format_cents(totals.project_discount_cents)
        ),
    ]));
    rows.push_str(&word_row(&[
        "Subtotal",
        "",
        "",
        "",
        "",
        "",
        "",
        &format!("{} {}", currency, format_cents(totals.subtotal_cents)),
    ]));
    rows.push_str(&word_row(&[
        "Tax",
        "",
        "",
        "",
        "",
        "",
        "",
        &format!("{} {}", currency, format_cents(totals.tax_cents)),
    ]));
    rows.push_str(&word_row(&[
        "Total",
        "",
        "",
        "",
        "",
        "",
        "",
        &format!("{} {}", currency, format_cents(totals.total_cents)),
    ]));
    rows.push_str(&word_row(&[
        "Total in words",
        "",
        "",
        "",
        "",
        "",
        "",
        &total_in_words(currency, totals.total_cents),
    ]));
    format!(
        r#"<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:color="999999"/><w:left w:val="single" w:sz="4" w:color="999999"/><w:bottom w:val="single" w:sz="4" w:color="999999"/><w:right w:val="single" w:sz="4" w:color="999999"/><w:insideH w:val="single" w:sz="4" w:color="D9D9D9"/><w:insideV w:val="single" w:sz="4" w:color="D9D9D9"/></w:tblBorders></w:tblPr>{rows}</w:tbl>"#
    )
}

fn word_row(values: &[&str]) -> String {
    let cells = values
        .iter()
        .map(|value| {
            format!(
                r#"<w:tc><w:tcPr><w:tcW w:w="0" w:type="auto"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p></w:tc>"#,
                xml(value)
            )
        })
        .collect::<String>();
    format!("<w:tr>{cells}</w:tr>")
}

fn xlsx_sheet(document: &BusinessDocumentRecord) -> Result<String, HostError> {
    let profile = &document.snapshot.profile;
    let totals = document_totals(profile)?;
    let mut rows = String::new();
    rows.push_str(&sheet_row(1, &[inline_cell("A1", &document.title, 2)]));
    rows.push_str(&sheet_row(
        2,
        &[
            inline_cell("A2", "Document No.", 1),
            inline_cell("B2", &document.document_number, 0),
        ],
    ));
    rows.push_str(&sheet_row(
        3,
        &[
            inline_cell("A3", "Project", 1),
            inline_cell("B3", &profile.project_title, 0),
            inline_cell("D3", "Customer", 1),
            inline_cell("E3", display_customer(profile), 0),
        ],
    ));
    rows.push_str(&sheet_row(
        4,
        &[
            inline_cell("A4", "Supplier", 1),
            inline_cell("B4", &profile.supplier_legal_name, 0),
            inline_cell("D4", "Currency", 1),
            inline_cell("E4", &profile.currency, 0),
        ],
    ));
    rows.push_str(&sheet_row(
        5,
        &[
            "Item",
            "Description",
            "Quantity",
            "Unit",
            "Unit Price",
            "Tax Rate",
            "Amount",
            "Tax Amount",
        ]
        .iter()
        .enumerate()
        .map(|(index, value)| inline_cell(&format!("{}5", column(index)), value, 1))
        .collect::<Vec<_>>(),
    ));
    for (offset, item) in profile.line_items.iter().enumerate() {
        let row = offset + 6;
        rows.push_str(&sheet_row(
            row,
            &[
                inline_cell(&format!("A{row}"), &item.name, 0),
                inline_cell(&format!("B{row}"), &item.description, 0),
                number_cell(
                    &format!("C{row}"),
                    &scaled_decimal(item.quantity_millis, 3),
                    3,
                ),
                inline_cell(&format!("D{row}"), &item.unit, 0),
                number_cell(
                    &format!("E{row}"),
                    &scaled_decimal(item.unit_price_cents, 2),
                    3,
                ),
                number_cell(&format!("F{row}"), &scaled_decimal(item.tax_rate_bps, 4), 4),
                number_cell(&format!("G{row}"), &scaled_decimal(item.amount_cents, 2), 3),
                number_cell(
                    &format!("H{row}"),
                    &scaled_decimal(line_tax_cents(item), 2),
                    3,
                ),
            ],
        ));
    }
    let total_row = profile.line_items.len() + 6;
    rows.push_str(&sheet_row(
        total_row,
        &[
            inline_cell(&format!("F{total_row}"), "Original Total", 1),
            number_cell(
                &format!("G{total_row}"),
                &scaled_decimal(totals.original_total_cents, 2),
                3,
            ),
        ],
    ));
    rows.push_str(&sheet_row(
        total_row + 1,
        &[
            inline_cell(&format!("F{}", total_row + 1), "Project Discount", 1),
            number_cell(
                &format!("G{}", total_row + 1),
                &scaled_decimal(totals.project_discount_cents, 2),
                3,
            ),
        ],
    ));
    rows.push_str(&sheet_row(
        total_row + 2,
        &[
            inline_cell(&format!("F{}", total_row + 2), "Subtotal", 1),
            number_cell(
                &format!("G{}", total_row + 2),
                &scaled_decimal(totals.subtotal_cents, 2),
                3,
            ),
        ],
    ));
    rows.push_str(&sheet_row(
        total_row + 3,
        &[
            inline_cell(&format!("F{}", total_row + 3), "Tax", 1),
            number_cell(
                &format!("G{}", total_row + 3),
                &scaled_decimal(totals.tax_cents, 2),
                3,
            ),
        ],
    ));
    rows.push_str(&sheet_row(
        total_row + 4,
        &[
            inline_cell(&format!("F{}", total_row + 4), "Total", 1),
            number_cell(
                &format!("G{}", total_row + 4),
                &scaled_decimal(totals.total_cents, 2),
                3,
            ),
        ],
    ));
    rows.push_str(&sheet_row(
        total_row + 5,
        &[
            inline_cell(&format!("A{}", total_row + 5), "Total in words", 1),
            inline_cell(
                &format!("B{}", total_row + 5),
                &total_in_words(&profile.currency, totals.total_cents),
                0,
            ),
        ],
    ));
    let notes_row = total_row + 7;
    rows.push_str(&sheet_row(
        notes_row,
        &[
            inline_cell(&format!("A{notes_row}"), "Currency", 1),
            inline_cell(&format!("B{notes_row}"), &profile.currency, 0),
            inline_cell(&format!("D{notes_row}"), "Payment Terms", 1),
            inline_cell(&format!("E{notes_row}"), &profile.payment_terms, 0),
        ],
    ));
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cols><col min="1" max="1" width="22" customWidth="1"/><col min="2" max="2" width="42" customWidth="1"/><col min="3" max="8" width="16" customWidth="1"/></cols><sheetData>{rows}</sheetData><mergeCells count="1"><mergeCell ref="A1:H1"/></mergeCells><pageMargins left="0.3" right="0.3" top="0.5" bottom="0.5" header="0.2" footer="0.2"/></worksheet>"#
    ))
}

fn sheet_row(row: usize, cells: &[String]) -> String {
    format!(r#"<row r="{row}">{}</row>"#, cells.concat())
}

fn inline_cell(reference: &str, value: &str, style: usize) -> String {
    format!(
        r#"<c r="{}" s="{style}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
        xml(reference),
        xml(value)
    )
}

fn number_cell(reference: &str, value: &str, style: usize) -> String {
    format!(
        r#"<c r="{}" s="{style}"><v>{value}</v></c>"#,
        xml(reference)
    )
}

#[derive(Clone, Copy)]
struct Totals {
    original_total_cents: i64,
    project_discount_cents: i64,
    subtotal_cents: i64,
    tax_cents: i64,
    total_cents: i64,
}

fn document_totals(profile: &BusinessProfile) -> Result<Totals, HostError> {
    if let Some(totals) = &profile.quotation_totals {
        return Ok(Totals {
            original_total_cents: totals.original_total_cents,
            project_discount_cents: totals.project_discount_cents,
            subtotal_cents: totals.tax_exclusive_total_cents,
            tax_cents: totals.tax_cents,
            total_cents: totals.final_total_cents,
        });
    }

    let mut original_total = 0_i128;
    let mut tax = 0_i128;
    for item in &profile.line_items {
        original_total += i128::from(item.amount_cents);
        tax += i128::from(line_tax_cents(item));
    }
    let discount = i128::from(profile.project_discount_cents);
    let total = original_total - discount;
    let subtotal = original_total - tax;
    if original_total > i128::from(i64::MAX)
        || subtotal < 0
        || tax > i128::from(i64::MAX)
        || total < 0
        || total > i128::from(i64::MAX)
    {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_AMOUNT_OVERFLOW",
            "business document totals exceed the supported integer range",
            false,
        ));
    }
    Ok(Totals {
        original_total_cents: original_total as i64,
        project_discount_cents: profile.project_discount_cents,
        subtotal_cents: subtotal as i64,
        tax_cents: tax as i64,
        total_cents: total as i64,
    })
}

fn line_tax_cents(item: &BusinessLineItem) -> i64 {
    let denominator = 10_000_i128 + i128::from(item.tax_rate_bps);
    ((i128::from(item.amount_cents) * i128::from(item.tax_rate_bps) + denominator / 2)
        / denominator) as i64
}

fn scaled_decimal(value: i64, fractional_digits: u32) -> String {
    let scale = 10_i64.pow(fractional_digits);
    format!(
        "{}.{:0width$}",
        value / scale,
        value.abs() % scale,
        width = fractional_digits as usize
    )
}

fn optional_timestamp(value: Option<i64>) -> String {
    value
        .map(|timestamp| timestamp.to_string())
        .unwrap_or_else(|| "Not specified".to_string())
}

fn total_in_words(currency: &str, total_cents: i64) -> String {
    format!(
        "{} {} AND {:02}/100",
        currency,
        integer_in_words((total_cents / 100) as u64),
        total_cents % 100
    )
}

fn integer_in_words(mut value: u64) -> String {
    if value == 0 {
        return "ZERO".to_string();
    }
    let scales = [
        "",
        "THOUSAND",
        "MILLION",
        "BILLION",
        "TRILLION",
        "QUADRILLION",
    ];
    let mut groups = Vec::new();
    let mut scale = 0;
    while value > 0 {
        let group = (value % 1_000) as u16;
        if group != 0 {
            let suffix = scales.get(scale).copied().unwrap_or("QUINTILLION");
            groups.push(if suffix.is_empty() {
                under_thousand_in_words(group)
            } else {
                format!("{} {suffix}", under_thousand_in_words(group))
            });
        }
        value /= 1_000;
        scale += 1;
    }
    groups.reverse();
    groups.join(" ")
}

fn under_thousand_in_words(value: u16) -> String {
    const SMALL: [&str; 20] = [
        "ZERO",
        "ONE",
        "TWO",
        "THREE",
        "FOUR",
        "FIVE",
        "SIX",
        "SEVEN",
        "EIGHT",
        "NINE",
        "TEN",
        "ELEVEN",
        "TWELVE",
        "THIRTEEN",
        "FOURTEEN",
        "FIFTEEN",
        "SIXTEEN",
        "SEVENTEEN",
        "EIGHTEEN",
        "NINETEEN",
    ];
    const TENS: [&str; 10] = [
        "", "", "TWENTY", "THIRTY", "FORTY", "FIFTY", "SIXTY", "SEVENTY", "EIGHTY", "NINETY",
    ];
    let mut parts = Vec::new();
    let hundreds = value / 100;
    let remainder = value % 100;
    if hundreds > 0 {
        parts.push(format!("{} HUNDRED", SMALL[hundreds as usize]));
    }
    if remainder > 0 && remainder < 20 {
        parts.push(SMALL[remainder as usize].to_string());
    } else if remainder >= 20 {
        let tens = TENS[(remainder / 10) as usize];
        let ones = remainder % 10;
        parts.push(if ones == 0 {
            tens.to_string()
        } else {
            format!("{tens}-{}", SMALL[ones as usize])
        });
    }
    parts.join(" ")
}

fn column(index: usize) -> char {
    char::from_u32('A' as u32 + index as u32).expect("small fixed column index")
}

fn display_customer(profile: &crate::protocol::BusinessProfile) -> &str {
    if profile.customer_legal_name.is_empty() {
        &profile.customer_name
    } else {
        &profile.customer_legal_name
    }
}

fn format_millis(value: i64) -> String {
    format!("{}.{:03}", value / 1000, value.abs() % 1000)
}

fn format_cents(value: i64) -> String {
    format!("{}.{:02}", value / 100, value.abs() % 100)
}

fn xml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if !is_xml_character(character) {
            continue;
        }
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn is_xml_character(value: char) -> bool {
    matches!(value, '\u{9}' | '\u{A}' | '\u{D}')
        || ('\u{20}'..='\u{D7FF}').contains(&value)
        || ('\u{E000}'..='\u{FFFD}').contains(&value)
        || ('\u{10000}'..='\u{10FFFF}').contains(&value)
}

fn document_io_error(action: &'static str) -> impl FnOnce(std::io::Error) -> HostError {
    move |error| {
        HostError::new(
            "BUSINESS_DOCUMENT_IO",
            format!("{action} failed: {error}"),
            true,
        )
    }
}

fn document_zip_error(action: &'static str) -> impl FnOnce(zip::result::ZipError) -> HostError {
    move |error| {
        HostError::new(
            "BUSINESS_DOCUMENT_PACKAGE_FAILED",
            format!("{action} failed: {error}"),
            true,
        )
    }
}

const DOCX_CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#;
const DOCX_ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;
const DOCX_APP: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>华邦互娱商务系统</Application><AppVersion>1.0</AppVersion></Properties>"#;
const DOCX_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:rPr><w:sz w:val="20"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:rPr><w:b/><w:sz w:val="36"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:rPr><w:b/><w:sz w:val="26"/></w:rPr></w:style></w:styles>"#;

const XLSX_CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#;
const XLSX_ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;
const XLSX_APP: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>华邦互娱商务系统</Application><AppVersion>1.0</AppVersion></Properties>"#;
const XLSX_WORKBOOK: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Quote" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
const XLSX_WORKBOOK_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
const XLSX_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="2"><numFmt numFmtId="164" formatCode="0.00"/><numFmt numFmtId="165" formatCode="0.00%"/></numFmts><fonts count="2"><font><sz val="11"/><name val="Arial"/></font><font><b/><sz val="11"/><name val="Arial"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="5"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"><alignment horizontal="center"/></xf><xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="165" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        BusinessDocumentSnapshot, BusinessDocumentStatus, BusinessPaymentRecord,
        BusinessPaymentStatus, BusinessProfile, BusinessQuotationTotals, BusinessTaxMode,
    };
    use std::io::Read;
    use zip::ZipArchive;

    fn document(kind: BusinessDocumentKind) -> BusinessDocumentRecord {
        let template_key = match &kind {
            BusinessDocumentKind::Quote => QUOTE_TEMPLATE_KEY,
            BusinessDocumentKind::Contract => CONTRACT_TEMPLATE_KEY,
            BusinessDocumentKind::PaymentRequest => PAYMENT_REQUEST_TEMPLATE_KEY,
            BusinessDocumentKind::Acceptance => ACCEPTANCE_TEMPLATE_KEY,
        };
        let payment =
            (kind == BusinessDocumentKind::PaymentRequest).then(|| BusinessPaymentRecord {
                id: Uuid::new_v4().to_string(),
                label: "Deposit".to_string(),
                amount_cents: 10_000,
                due_at: Some(1_750_000_000_000),
                occurred_at: None,
                status: BusinessPaymentStatus::Requested,
                reference: "PO-TEST".to_string(),
                notes: String::new(),
                revision: 1,
                created_at: 1,
                updated_at: 1,
            });
        BusinessDocumentRecord {
            id: Uuid::new_v4().to_string(),
            kind,
            sequence_number: 1,
            document_number: "DOC<&>001".to_string(),
            title: "Title <quoted> & valid\u{1}".to_string(),
            template_key: template_key.to_string(),
            status: BusinessDocumentStatus::Approved,
            snapshot: BusinessDocumentSnapshot {
                workspace_revision: 2,
                acceptance_batch_id: None,
                acceptance_output_spec_id: None,
                acceptance_batch_revision: None,
                material_bindings: Vec::new(),
                template_asset_id: None,
                template_source_sha256: None,
                template_mapping_version: String::new(),
                contract_settlement: None,
                service_settlement_items: Vec::new(),
                payment_application: None,
                video_completion_acceptance: None,
                production_result_confirmation: None,
                customer_id: String::new(),
                customer: Default::default(),
                profile: BusinessProfile {
                    project_title: "Project & One".to_string(),
                    customer_name: "Customer".to_string(),
                    customer_legal_name: "Customer Legal Ltd.".to_string(),
                    customer_tax_id: "CUSTOMER-TAX-001".to_string(),
                    customer_address: "Customer Street 1".to_string(),
                    customer_contact: "Alice".to_string(),
                    customer_phone: "10086".to_string(),
                    customer_email: "alice@example.test".to_string(),
                    supplier_legal_name: "Supplier".to_string(),
                    supplier_tax_id: "SUPPLIER-TAX-001".to_string(),
                    supplier_address: "Supplier Street 2".to_string(),
                    supplier_contact: "Bob".to_string(),
                    supplier_phone: "10010".to_string(),
                    supplier_bank_name: "Test Bank".to_string(),
                    supplier_bank_account: "622200000001".to_string(),
                    currency: "CNY".to_string(),
                    tax_mode: BusinessTaxMode::TaxExclusive,
                    project_discount_cents: 0,
                    quotation_totals: Some(BusinessQuotationTotals {
                        original_total_cents: 15_900,
                        project_discount_cents: 0,
                        tax_exclusive_total_cents: 15_000,
                        tax_cents: 900,
                        final_total_cents: 15_900,
                    }),
                    service_start_at: Some(1_700_000_000_000),
                    service_end_at: Some(1_800_000_000_000),
                    delivery_summary: "Final film and cutdowns".to_string(),
                    payment_terms: "Pay within 10 days".to_string(),
                    acceptance_terms: "Written sign-off".to_string(),
                    line_items: vec![BusinessLineItem {
                        id: Uuid::new_v4().to_string(),
                        name: "Production".to_string(),
                        description: "Main delivery".to_string(),
                        quantity_millis: 1_500,
                        unit: "item".to_string(),
                        unit_price_cents: 10_000,
                        tax_rate_bps: 600,
                        amount_cents: 15_900,
                    }],
                    ..BusinessProfile::default()
                },
                payment,
            },
            output_asset_id: None,
            output_format: None,
            source_asset_id: None,
            review_id: None,
            report_asset_id: None,
            evidence: None,
            manual_waiver: None,
            voided_at: None,
            voided_by: None,
            void_reason: String::new(),
            approved_at: Some(1),
            approved_by: Some("operator".to_string()),
            generated_at: None,
            revision: 2,
            created_at: 1,
            updated_at: 1,
        }
    }

    fn video_completion_acceptance_data(
    ) -> crate::business_v1::video_completion_acceptance_template::VideoCompletionAcceptanceTemplateData
    {
        use crate::business_v1::video_completion_acceptance_template::{
            VideoAssetReference, VideoBlock, VideoCompletionAcceptanceTemplateData,
            VideoDeliveryGroup, VideoScreenshot,
        };
        use sha2::{Digest, Sha256};

        let image_bytes = vec![
            0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0,
            1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 0, b'I', b'E', b'N', b'D',
            174, 66, 96, 130,
        ];
        let image_sha256 = format!("{:X}", Sha256::digest(&image_bytes));
        VideoCompletionAcceptanceTemplateData {
            contract_title: "Test contract".to_string(),
            project_title: "Test project".to_string(),
            completion_date: "2026-07-29".to_string(),
            delivery_groups: vec![VideoDeliveryGroup {
                name: "Delivery group".to_string(),
                service_description: "Completed video delivery".to_string(),
                videos: vec![VideoBlock {
                    title: "Main video".to_string(),
                    video_type: "Final cut".to_string(),
                    content: "Approved final video".to_string(),
                    duration: "30s".to_string(),
                    asset_reference: VideoAssetReference {
                        asset_id: "asset-video-1".to_string(),
                        file_name: "main.mp4".to_string(),
                        sha256: "A".repeat(64),
                        external_link: None,
                    },
                    screenshots: vec![VideoScreenshot {
                        asset_id: "asset-shot-1".to_string(),
                        sha256: image_sha256,
                        caption: "Representative frame".to_string(),
                        mime_type: "image/png".to_string(),
                        image_bytes,
                        width_px: 1,
                        height_px: 1,
                    }],
                }],
            }],
            acceptance_conclusion: "Accepted".to_string(),
            manually_confirmed: true,
        }
    }

    fn production_result_confirmation_data(
    ) -> crate::business_v1::production_result_confirmation_template::ProductionResultConfirmationTemplateData
    {
        use crate::business_v1::production_result_confirmation_template::{
            ProductionResultConfirmationDeliveryItem, ProductionResultConfirmationImage,
            ProductionResultConfirmationShot, ProductionResultConfirmationStoryboard,
            ProductionResultConfirmationTemplateData,
        };
        use sha2::{Digest, Sha256};

        let image_bytes = vec![
            0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0,
            1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 0, b'I', b'E', b'N', b'D',
            174, 66, 96, 130,
        ];
        let image_sha256 = format!("{:X}", Sha256::digest(&image_bytes));
        let mut next_shot = 1usize;
        let storyboards = [14usize, 14, 13, 13]
            .into_iter()
            .enumerate()
            .map(|(storyboard_index, shot_count)| {
                let shots = (0..shot_count)
                    .map(|_| {
                        let shot_number = format!("SHOT-{next_shot:02}");
                        next_shot += 1;
                        ProductionResultConfirmationShot {
                            shot_number: shot_number.clone(),
                            scene: "Test scene".to_string(),
                            description: "Approved storyboard frame".to_string(),
                            on_screen_copy: "Test copy".to_string(),
                            remarks: String::new(),
                            source_highlighted: true,
                            images: vec![ProductionResultConfirmationImage {
                                asset_id: format!("asset-{shot_number}"),
                                sha256: image_sha256.clone(),
                                mime_type: "image/png".to_string(),
                                width_px: 1,
                                height_px: 1,
                                alt_text: shot_number,
                                image_bytes: image_bytes.clone(),
                            }],
                        }
                    })
                    .collect();
                ProductionResultConfirmationStoryboard {
                    title: format!("Storyboard {}", storyboard_index + 1),
                    specification: "Approved specification".to_string(),
                    production_format: "Video".to_string(),
                    duration: "30s".to_string(),
                    shots,
                }
            })
            .collect();

        ProductionResultConfirmationTemplateData {
            attachment_label: "Attachment 1".to_string(),
            document_title: "Production Result Confirmation".to_string(),
            category: "Video production".to_string(),
            project_name: "Test project".to_string(),
            contract_title: "Test contract".to_string(),
            payment_amount_cents: 100_000,
            contract_deliverable_summary: "Approved production deliverables".to_string(),
            supplier_legal_name: "Test supplier".to_string(),
            procurement_period: "2026-07-01 to 2026-07-29".to_string(),
            acceptance_description: "Production results accepted".to_string(),
            penalty_or_additions: "None".to_string(),
            delivery_items: vec![ProductionResultConfirmationDeliveryItem {
                item_id: "delivery-item-1".to_string(),
                name: "Final video".to_string(),
                specification: "Approved master".to_string(),
                required_quantity: "1".to_string(),
                unit: "item".to_string(),
                received_quantity: "1".to_string(),
                acceptance_note: "Accepted".to_string(),
                images: Vec::new(),
            }],
            execution_completed_date: "2026-07-28".to_string(),
            acceptance_date: "2026-07-29".to_string(),
            handler_signoff: "Handler".to_string(),
            professional_lead_signoff: "Lead".to_string(),
            other_department_signoff: String::new(),
            supplier_handler_signoff: "Supplier".to_string(),
            storyboards,
            clean_highlights: Some(true),
        }
    }

    fn assert_archive(staged: &StagedDocument, required_entries: &[&str], content_entry: &str) {
        let bytes = fs::read(staged.path()).unwrap();
        assert!(bytes.starts_with(b"PK\x03\x04"));
        let mut archive = ZipArchive::new(File::open(staged.path()).unwrap()).unwrap();
        for entry in required_entries {
            archive.by_name(entry).unwrap();
        }
        let mut content = String::new();
        archive
            .by_name(content_entry)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains("Project &amp; One"));
        assert!(!content.contains('\u{1}'));
    }

    fn read_entry(staged: &StagedDocument, entry: &str) -> String {
        let mut archive = ZipArchive::new(File::open(staged.path()).unwrap()).unwrap();
        let mut content = String::new();
        archive
            .by_name(entry)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        content
    }

    fn staging_fixture(vault_root: &Path, generated_name: &str) -> PathBuf {
        fs::create_dir_all(vault_root).unwrap();
        let vault_root = fs::canonicalize(vault_root).unwrap();
        let staging_root = prepare_staging_root(&vault_root).unwrap();
        let staging_directory = staging_root.join(Uuid::new_v4().to_string());
        fs::create_dir(&staging_directory).unwrap();
        fs::write(staging_directory.join(generated_name), b"generated").unwrap();
        staging_directory
    }

    #[test]
    fn quote_is_a_real_xlsx_and_staging_is_scoped_to_vault() {
        let temporary = tempfile::tempdir().unwrap();
        let staged = generate_document(
            temporary.path(),
            &document(BusinessDocumentKind::Quote),
            &BusinessDocumentFormat::Xlsx,
        )
        .unwrap();
        let staged_path = staged.path().to_path_buf();
        assert!(staged_path.starts_with(fs::canonicalize(temporary.path()).unwrap()));
        assert_archive(
            &staged,
            &[
                "[Content_Types].xml",
                "_rels/.rels",
                "xl/workbook.xml",
                "xl/_rels/workbook.xml.rels",
                "xl/worksheets/sheet1.xml",
                "xl/styles.xml",
            ],
            "xl/worksheets/sheet1.xml",
        );
        let sheet = read_entry(&staged, "xl/worksheets/sheet1.xml");
        assert!(sheet.contains("Subtotal"));
        assert!(sheet.contains("Tax"));
        assert!(sheet.contains("Total"));
        assert!(sheet.contains("Supplier"));
        assert!(sheet.contains("CNY"));
        assert!(sheet.contains(">150.00</v>"));
        assert!(sheet.contains(">9.00</v>"));
        assert!(sheet.contains(">159.00</v>"));
        assert!(sheet.contains("CNY ONE HUNDRED FIFTY-NINE AND 00/100"));
        drop(staged);
        assert!(!staged_path.exists());
    }

    #[test]
    fn acceptance_can_be_a_real_xlsx_package() {
        let temporary = tempfile::tempdir().unwrap();
        let staged = generate_document(
            temporary.path(),
            &document(BusinessDocumentKind::Acceptance),
            &BusinessDocumentFormat::Xlsx,
        )
        .unwrap();
        assert_archive(
            &staged,
            &[
                "[Content_Types].xml",
                "_rels/.rels",
                "xl/workbook.xml",
                "xl/_rels/workbook.xml.rels",
                "xl/worksheets/sheet1.xml",
                "xl/styles.xml",
            ],
            "xl/worksheets/sheet1.xml",
        );
    }

    #[test]
    fn baietan_tax_inclusive_quote_keeps_unit_price_and_applies_project_discount_once() {
        let mut document = document(BusinessDocumentKind::Quote);
        let profile = &mut document.snapshot.profile;
        profile.tax_mode = BusinessTaxMode::TaxInclusive;
        profile.project_discount_cents = 490_000;
        profile.line_items = vec![BusinessLineItem {
            id: Uuid::new_v4().to_string(),
            name: "Video production".to_string(),
            description: "Baietan delivery".to_string(),
            quantity_millis: 4_000,
            unit: "item".to_string(),
            unit_price_cents: 2_120_000,
            tax_rate_bps: 600,
            amount_cents: 8_480_000,
        }];
        profile.quotation_totals = Some(BusinessQuotationTotals {
            original_total_cents: 8_480_000,
            project_discount_cents: 490_000,
            tax_exclusive_total_cents: 7_537_736,
            tax_cents: 452_264,
            final_total_cents: 7_990_000,
        });

        let temporary = tempfile::tempdir().unwrap();
        let staged =
            generate_document(temporary.path(), &document, &BusinessDocumentFormat::Xlsx).unwrap();
        let sheet = read_entry(&staged, "xl/worksheets/sheet1.xml");
        assert!(sheet.contains(">21200.00</v>"));
        assert!(sheet.contains(">84800.00</v>"));
        assert!(sheet.contains(">4900.00</v>"));
        assert!(sheet.contains(">79900.00</v>"));
        assert!(!sheet.contains(">89888.00</v>"));
        assert!(!sheet.contains(">95281.28</v>"));
    }

    #[test]
    fn all_word_document_kinds_are_real_docx_packages() {
        for kind in [
            BusinessDocumentKind::Contract,
            BusinessDocumentKind::PaymentRequest,
            BusinessDocumentKind::Acceptance,
        ] {
            let temporary = tempfile::tempdir().unwrap();
            let staged = generate_document(
                temporary.path(),
                &document(kind.clone()),
                &BusinessDocumentFormat::Docx,
            )
            .unwrap();
            assert_archive(
                &staged,
                &[
                    "[Content_Types].xml",
                    "_rels/.rels",
                    "word/document.xml",
                    "word/styles.xml",
                    "docProps/core.xml",
                ],
                "word/document.xml",
            );
            let content = read_entry(&staged, "word/document.xml");
            assert!(content.contains("Customer Tax ID: CUSTOMER-TAX-001"));
            assert!(content.contains("Customer Address: Customer Street 1"));
            assert!(content.contains("Supplier Tax ID: SUPPLIER-TAX-001"));
            assert!(content.contains("Supplier Address: Supplier Street 2"));
            match kind {
                BusinessDocumentKind::Contract => {
                    assert!(content.contains("Service Start (Unix ms): 1700000000000"));
                    assert!(content.contains("Service End (Unix ms): 1800000000000"));
                    assert!(content.contains("Pay within 10 days"));
                    assert!(content.contains("Subtotal"));
                    assert!(content.contains("CNY 150.00"));
                    assert!(content.contains("CNY 9.00"));
                    assert!(content.contains("CNY 159.00"));
                }
                BusinessDocumentKind::PaymentRequest => {
                    assert!(content.contains("Receiving Bank: Test Bank"));
                    assert!(content.contains("Receiving Account: 622200000001"));
                    assert!(content.contains("Amount Due: CNY 100.00"));
                    assert!(content.contains("Amount Due in words: CNY ONE HUNDRED AND 00/100"));
                    assert!(content.contains("Payment Label: Deposit"));
                    assert!(content.contains("Payment Reference: PO-TEST"));
                    assert!(!content.contains("Subtotal"));
                    assert!(!content.contains("CNY 159.00"));
                }
                BusinessDocumentKind::Acceptance => {
                    assert!(content.contains("Final film and cutdowns"));
                    assert!(content.contains("Written sign-off"));
                    assert!(content.contains("Subtotal"));
                    assert!(content.contains("CNY 150.00"));
                    assert!(content.contains("CNY 9.00"));
                    assert!(content.contains("CNY 159.00"));
                }
                BusinessDocumentKind::Quote => unreachable!(),
            }
        }
    }

    #[test]
    fn payment_request_uses_only_the_frozen_payment_total() {
        let content = docx_document(&document(BusinessDocumentKind::PaymentRequest)).unwrap();

        assert!(content.contains("Amount Due: CNY 100.00"));
        assert!(content.contains("Amount Due in words: CNY ONE HUNDRED AND 00/100"));
        assert!(!content.contains("Subtotal"));
        assert!(!content.contains(">Total</w:t>"));
        assert!(!content.contains("CNY 150.00"));
        assert!(!content.contains("CNY 9.00"));
        assert!(!content.contains("CNY 159.00"));
        assert!(!content.contains("CNY ONE HUNDRED FIFTY-NINE AND 00/100"));
    }

    #[test]
    fn baietan_acceptance_template_keys_are_registered_with_stable_names_and_formats() {
        let registrations = [
            (
                BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY,
                "project.baietan.acceptance.video-completion-acceptance.v1",
                BusinessDocumentFormat::Docx,
            ),
            (
                BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY,
                "project.baietan.acceptance.production-result-confirmation.v1",
                BusinessDocumentFormat::Docx,
            ),
            (
                BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY,
                "project.baietan.acceptance.service-settlement-list.v1",
                BusinessDocumentFormat::Docx,
            ),
            (
                BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY,
                "project.baietan.acceptance.contract-settlement.v1",
                BusinessDocumentFormat::Xlsx,
            ),
            (
                BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY,
                "project.baietan.acceptance.payment-application-settlement-calculation.v1",
                BusinessDocumentFormat::Docx,
            ),
        ];

        for (template_key, expected_key, format) in registrations {
            assert_eq!(template_key, expected_key);
            validate_template(&BusinessDocumentKind::Acceptance, template_key).unwrap();
            validate_template_for_format(&BusinessDocumentKind::Acceptance, template_key, &format)
                .unwrap();
        }
    }

    #[test]
    fn baietan_acceptance_template_keys_reject_format_mismatches() {
        let mismatches = [
            (
                BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY,
                BusinessDocumentFormat::Xlsx,
            ),
            (
                BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY,
                BusinessDocumentFormat::Xlsx,
            ),
            (
                BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY,
                BusinessDocumentFormat::Xlsx,
            ),
            (
                BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY,
                BusinessDocumentFormat::Docx,
            ),
            (
                BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY,
                BusinessDocumentFormat::Xlsx,
            ),
        ];

        for (template_key, format) in mismatches {
            let temporary = tempfile::tempdir().unwrap();
            let mut acceptance = document(BusinessDocumentKind::Acceptance);
            acceptance.template_key = template_key.to_string();
            let error = generate_document(temporary.path(), &acceptance, &format).unwrap_err();
            assert_eq!(error.code, "BUSINESS_TEMPLATE_FORMAT_MISMATCH");
        }
    }

    #[test]
    fn baietan_acceptance_template_keys_never_fall_back_to_builtin_renderers() {
        let registrations = [
            (
                BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY,
                BusinessDocumentFormat::Docx,
            ),
            (
                BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY,
                BusinessDocumentFormat::Docx,
            ),
            (
                BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY,
                BusinessDocumentFormat::Docx,
            ),
            (
                BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY,
                BusinessDocumentFormat::Xlsx,
            ),
            (
                BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY,
                BusinessDocumentFormat::Docx,
            ),
        ];

        for (template_key, format) in registrations {
            let temporary = tempfile::tempdir().unwrap();
            let mut acceptance = document(BusinessDocumentKind::Acceptance);
            acceptance.template_key = template_key.to_string();
            let error = generate_document(temporary.path(), &acceptance, &format).unwrap_err();
            assert_eq!(error.code, "BUSINESS_TEMPLATE_RENDERING_NOT_READY");
            assert!(fs::read_dir(temporary.path()).unwrap().next().is_none());
        }
    }

    #[test]
    fn legacy_template_entry_delegates_with_empty_video_resources() {
        let temporary = tempfile::tempdir().unwrap();
        let mut acceptance = document(BusinessDocumentKind::Acceptance);
        acceptance.template_key = BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY.to_string();
        acceptance.snapshot.template_source_sha256 =
            Some(BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_SHA256.to_string());

        let error = generate_document_with_template(
            temporary.path(),
            &acceptance,
            &BusinessDocumentFormat::Docx,
            Some(b"not-a-template"),
        )
        .unwrap_err();

        assert_eq!(error.code, "BUSINESS_TEMPLATE_RENDERING_NOT_READY");
        assert!(fs::read_dir(temporary.path()).unwrap().next().is_none());
    }

    #[test]
    fn resource_entry_dispatches_video_completion_renderer() {
        let temporary = tempfile::tempdir().unwrap();
        let mut acceptance = document(BusinessDocumentKind::Acceptance);
        acceptance.template_key = BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY.to_string();
        acceptance.snapshot.template_source_sha256 =
            Some(BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_SHA256.to_string());
        let data = video_completion_acceptance_data();

        let error = generate_document_with_template_and_resources(
            temporary.path(),
            &acceptance,
            &BusinessDocumentFormat::Docx,
            Some(b"not-a-template"),
            DocumentGenerationResources {
                video_completion_acceptance: Some(&data),
                production_result_confirmation: None,
            },
        )
        .unwrap_err();

        assert_eq!(error.code, "VALIDATION_FAILED");
        assert!(error.message.contains("SHA-256"));
        let staging_root = temporary.path().join(STAGING_DIRECTORY);
        assert!(staging_root.is_dir());
        assert!(fs::read_dir(staging_root)
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry.file_type().unwrap().is_dir()));
    }

    #[test]
    fn resource_entry_dispatches_production_result_confirmation_renderer() {
        let temporary = tempfile::tempdir().unwrap();
        let mut acceptance = document(BusinessDocumentKind::Acceptance);
        acceptance.template_key = BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY.to_string();
        acceptance.snapshot.template_source_sha256 =
            Some(BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_SHA256.to_string());
        let data = production_result_confirmation_data();

        let error = generate_document_with_template_and_resources(
            temporary.path(),
            &acceptance,
            &BusinessDocumentFormat::Docx,
            Some(b"not-a-template"),
            DocumentGenerationResources {
                video_completion_acceptance: None,
                production_result_confirmation: Some(&data),
            },
        )
        .unwrap_err();

        assert_eq!(error.code, "VALIDATION_FAILED");
        assert!(error.message.contains("SHA-256"));
        let staging_root = temporary.path().join(STAGING_DIRECTORY);
        assert!(staging_root.is_dir());
        assert!(fs::read_dir(staging_root)
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry.file_type().unwrap().is_dir()));
    }

    #[test]
    fn production_result_confirmation_resources_are_rejected_for_other_templates() {
        let temporary = tempfile::tempdir().unwrap();
        let acceptance = document(BusinessDocumentKind::Acceptance);
        let data = production_result_confirmation_data();

        let error = generate_document_with_template_and_resources(
            temporary.path(),
            &acceptance,
            &BusinessDocumentFormat::Docx,
            None,
            DocumentGenerationResources {
                video_completion_acceptance: None,
                production_result_confirmation: Some(&data),
            },
        )
        .unwrap_err();

        assert_eq!(error.code, "BUSINESS_TEMPLATE_DATA_UNEXPECTED");
        assert!(error.message.contains("production result confirmation"));
        assert!(fs::read_dir(temporary.path()).unwrap().next().is_none());
    }

    #[test]
    fn builtin_acceptance_template_remains_compatible_with_both_formats() {
        validate_template(&BusinessDocumentKind::Acceptance, ACCEPTANCE_TEMPLATE_KEY).unwrap();
        for format in [BusinessDocumentFormat::Docx, BusinessDocumentFormat::Xlsx] {
            validate_template_for_format(
                &BusinessDocumentKind::Acceptance,
                ACCEPTANCE_TEMPLATE_KEY,
                &format,
            )
            .unwrap();
        }
    }

    #[test]
    fn unknown_template_key_is_rejected() {
        let error = validate_template(
            &BusinessDocumentKind::Acceptance,
            "project.baietan.acceptance.unknown.v1",
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_TEMPLATE_UNKNOWN");
    }

    #[test]
    fn kind_format_matrix_is_strict() {
        let temporary = tempfile::tempdir().unwrap();
        let error = generate_document(
            temporary.path(),
            &document(BusinessDocumentKind::Quote),
            &BusinessDocumentFormat::Docx,
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_DOCUMENT_FORMAT_INVALID");

        let mut unknown = document(BusinessDocumentKind::Contract);
        unknown.template_key = "builtin.unknown.v1".to_string();
        assert_eq!(
            generate_document(temporary.path(), &unknown, &BusinessDocumentFormat::Docx,)
                .unwrap_err()
                .code,
            "BUSINESS_TEMPLATE_UNKNOWN"
        );
        let mut mismatch = document(BusinessDocumentKind::Contract);
        mismatch.template_key = QUOTE_TEMPLATE_KEY.to_string();
        assert_eq!(
            generate_document(temporary.path(), &mismatch, &BusinessDocumentFormat::Docx,)
                .unwrap_err()
                .code,
            "BUSINESS_TEMPLATE_KIND_MISMATCH"
        );
    }

    #[test]
    fn exact_decimal_output_preserves_large_integer_cents() {
        let mut document = document(BusinessDocumentKind::Quote);
        document.snapshot.profile.quotation_totals = None;
        let item = &mut document.snapshot.profile.line_items[0];
        item.quantity_millis = 1_000;
        item.unit_price_cents = 9_000_000_000_000_000;
        item.amount_cents = 9_000_000_000_000_000;
        item.tax_rate_bps = 0;
        let sheet = xlsx_sheet(&document).unwrap();
        assert!(sheet.contains("90000000000000.00"));
        assert!(!sheet.to_ascii_lowercase().contains("e+"));
        assert!(!sheet.contains("89999999999999"));
    }

    #[test]
    fn reconciliation_skips_active_lease_and_collects_released_lease() {
        let temporary = tempfile::tempdir().unwrap();
        let staged = generate_document(
            temporary.path(),
            &document(BusinessDocumentKind::Quote),
            &BusinessDocumentFormat::Xlsx,
        )
        .unwrap();
        let staged_path = staged.path().to_path_buf();
        let generated_name = staged_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        reconcile_staging(temporary.path()).unwrap();
        assert!(staged_path.exists());
        assert!(generation_is_active(temporary.path(), &generated_name).unwrap());

        let mut abandoned = std::mem::ManuallyDrop::new(staged);
        let lease = abandoned.lease.take().unwrap();
        FileExt::unlock(&lease).unwrap();
        drop(lease);
        reconcile_staging(temporary.path()).unwrap();
        assert!(!staged_path.exists());
        assert!(!generation_is_active(temporary.path(), &generated_name).unwrap());
    }

    #[test]
    fn normalized_template_staging_uses_the_shared_recovery_lease() {
        let temporary = tempfile::tempdir().unwrap();
        let template_version_id = Uuid::new_v4().to_string();
        let staged = stage_normalized_template(temporary.path(), &template_version_id).unwrap();
        fs::write(staged.path(), b"normalized template").unwrap();
        let staged_path = staged.path().to_path_buf();
        let generated_name = staged_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        reconcile_staging(temporary.path()).unwrap();
        assert!(staged_path.exists());
        assert!(generation_is_active(temporary.path(), &generated_name).unwrap());
        drop(staged);
        assert!(!staged_path.exists());
    }

    #[test]
    fn generation_lease_rejects_non_file_entries() {
        let temporary = tempfile::tempdir().unwrap();
        let generated_name = "generated.docx";
        let staging_directory = staging_fixture(temporary.path(), generated_name);
        fs::create_dir(staging_directory.join(GENERATION_LEASE_FILE)).unwrap();

        let reconcile_error = reconcile_staging(temporary.path()).unwrap_err();
        assert_eq!(reconcile_error.code, "VAULT_PATH_INVALID");
        let active_error = generation_is_active(temporary.path(), generated_name).unwrap_err();
        assert_eq!(active_error.code, "VAULT_PATH_INVALID");
    }

    #[cfg(unix)]
    #[test]
    fn generation_lease_rejects_unix_symlinks() {
        let temporary = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let generated_name = "generated.docx";
        let staging_directory = staging_fixture(temporary.path(), generated_name);
        let outside_lease = outside.path().join("outside.lease");
        fs::write(&outside_lease, b"outside").unwrap();
        std::os::unix::fs::symlink(
            &outside_lease,
            staging_directory.join(GENERATION_LEASE_FILE),
        )
        .unwrap();

        let reconcile_error = reconcile_staging(temporary.path()).unwrap_err();
        assert_eq!(reconcile_error.code, "VAULT_PATH_INVALID");
        let active_error = generation_is_active(temporary.path(), generated_name).unwrap_err();
        assert_eq!(active_error.code, "VAULT_PATH_INVALID");
    }

    #[cfg(windows)]
    #[test]
    fn generation_lease_rejects_windows_file_symlinks_when_supported() {
        let temporary = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let generated_name = "generated.docx";
        let staging_directory = staging_fixture(temporary.path(), generated_name);
        let outside_lease = outside.path().join("outside.lease");
        fs::write(&outside_lease, b"outside").unwrap();
        if std::os::windows::fs::symlink_file(
            &outside_lease,
            staging_directory.join(GENERATION_LEASE_FILE),
        )
        .is_err()
        {
            return;
        }

        let reconcile_error = reconcile_staging(temporary.path()).unwrap_err();
        assert_eq!(reconcile_error.code, "VAULT_PATH_INVALID");
        let active_error = generation_is_active(temporary.path(), generated_name).unwrap_err();
        assert_eq!(active_error.code, "VAULT_PATH_INVALID");
    }

    #[cfg(unix)]
    #[test]
    fn staged_drop_does_not_follow_replaced_unix_directory_symlink() {
        let temporary = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, b"keep").unwrap();
        let mut staged = generate_document(
            temporary.path(),
            &document(BusinessDocumentKind::Quote),
            &BusinessDocumentFormat::Xlsx,
        )
        .unwrap();
        let staging_directory = staged.staging_directory.clone();
        let lease = staged.lease.take().unwrap();
        FileExt::unlock(&lease).unwrap();
        drop(lease);
        fs::remove_dir_all(&staging_directory).unwrap();
        std::os::unix::fs::symlink(outside.path(), &staging_directory).unwrap();

        drop(staged);

        assert_eq!(fs::read(&sentinel).unwrap(), b"keep");
        assert!(fs::symlink_metadata(&staging_directory)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(windows)]
    #[test]
    fn staged_drop_does_not_follow_replaced_windows_directory_symlink_when_supported() {
        let temporary = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, b"keep").unwrap();
        let mut staged = generate_document(
            temporary.path(),
            &document(BusinessDocumentKind::Quote),
            &BusinessDocumentFormat::Xlsx,
        )
        .unwrap();
        let staging_directory = staged.staging_directory.clone();
        let lease = staged.lease.take().unwrap();
        FileExt::unlock(&lease).unwrap();
        drop(lease);
        fs::remove_dir_all(&staging_directory).unwrap();
        if std::os::windows::fs::symlink_dir(outside.path(), &staging_directory).is_err() {
            return;
        }

        drop(staged);

        assert_eq!(fs::read(&sentinel).unwrap(), b"keep");
        assert!(is_link_or_reparse(
            &fs::symlink_metadata(&staging_directory).unwrap()
        ));
    }

    #[test]
    fn staging_root_rejects_non_directories_and_links() {
        let file_vault = tempfile::tempdir().unwrap();
        fs::write(
            file_vault.path().join(STAGING_DIRECTORY),
            b"not a directory",
        )
        .unwrap();
        let error = generate_document(
            file_vault.path(),
            &document(BusinessDocumentKind::Quote),
            &BusinessDocumentFormat::Xlsx,
        )
        .unwrap_err();
        assert_eq!(error.code, "VAULT_PATH_INVALID");

        let link_vault = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path(), link_vault.path().join(STAGING_DIRECTORY))
                .unwrap();
            let error = generate_document(
                link_vault.path(),
                &document(BusinessDocumentKind::Quote),
                &BusinessDocumentFormat::Xlsx,
            )
            .unwrap_err();
            assert_eq!(error.code, "VAULT_PATH_INVALID");
        }
        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_dir(
                outside.path(),
                link_vault.path().join(STAGING_DIRECTORY),
            )
            .is_ok()
            {
                let error = generate_document(
                    link_vault.path(),
                    &document(BusinessDocumentKind::Quote),
                    &BusinessDocumentFormat::Xlsx,
                )
                .unwrap_err();
                assert_eq!(error.code, "VAULT_PATH_INVALID");
            }
        }
    }
}
