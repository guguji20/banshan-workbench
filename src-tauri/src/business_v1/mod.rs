mod acceptance;
pub(crate) mod acceptance_docx_template;
pub(crate) mod acceptance_xlsx_template;
mod contract;
mod error;
mod identity;
pub(crate) mod legacy_doc_normalizer;
pub(crate) mod payment_application_template;
pub(crate) mod production_result_confirmation_template;
mod quotation;
mod settlement;
mod template_version;
pub(crate) mod video_completion_acceptance_template;

pub use acceptance::{AcceptanceBatch, AcceptanceReadiness, MaterialEvidence, MaterialKind};
pub use contract::{
    Contract, ContractKind, ContractStatus, Deliverable, ServiceOrder, ServiceOrderStatus,
    SettlementCadence,
};
pub use error::{DomainError, DomainResult};
pub use identity::{Company, CustomerProject, ProjectStatus};
pub use quotation::{
    CalculatedQuotation, CalculatedQuotationItem, Quotation, QuotationItem, QuotationStatus,
};
pub use settlement::{SettlementBatch, SettlementLedger, SettlementStatus};
pub use template_version::{
    TemplateArtifact, TemplateConverter, TemplateVersion, TemplateVersionDecision,
    TemplateVersionStatus,
};

pub type MinorUnits = i64;
pub type QuantityMillis = i64;
pub type BasisPoints = u32;

pub const QUANTITY_SCALE: i64 = 1_000;
pub const BASIS_POINT_SCALE: i64 = 10_000;

#[cfg(test)]
mod tests;
