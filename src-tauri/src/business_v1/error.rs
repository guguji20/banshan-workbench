use std::error::Error;
use std::fmt;

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    EmptyField(&'static str),
    InvalidValue {
        field: &'static str,
        reason: &'static str,
    },
    MismatchedReference {
        field: &'static str,
        expected: String,
        actual: String,
    },
    DuplicateReference {
        entity: &'static str,
        id: String,
    },
    MissingReference {
        entity: &'static str,
        id: String,
    },
    AlreadySettled {
        deliverable_id: String,
        settlement_batch_id: String,
    },
    MissingMaterials {
        acceptance_batch_id: String,
        deliverable_ids: Vec<String>,
    },
    ArithmeticOverflow,
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(formatter, "{field} must not be empty"),
            Self::InvalidValue { field, reason } => write!(formatter, "invalid {field}: {reason}"),
            Self::MismatchedReference { field, expected, actual } => write!(
                formatter,
                "{field} reference mismatch: expected {expected}, got {actual}"
            ),
            Self::DuplicateReference { entity, id } => {
                write!(formatter, "duplicate {entity} reference: {id}")
            }
            Self::MissingReference { entity, id } => {
                write!(formatter, "missing {entity} reference: {id}")
            }
            Self::AlreadySettled { deliverable_id, settlement_batch_id } => write!(
                formatter,
                "deliverable {deliverable_id} already belongs to settlement batch {settlement_batch_id}"
            ),
            Self::MissingMaterials { acceptance_batch_id, deliverable_ids } => write!(
                formatter,
                "acceptance batch {acceptance_batch_id} cannot become official; missing material for: {}",
                deliverable_ids.join(", ")
            ),
            Self::ArithmeticOverflow => write!(formatter, "domain arithmetic overflow"),
        }
    }
}

impl Error for DomainError {}

pub(crate) fn require_non_empty(value: &str, field: &'static str) -> DomainResult<()> {
    if value.trim().is_empty() {
        return Err(DomainError::EmptyField(field));
    }
    Ok(())
}
