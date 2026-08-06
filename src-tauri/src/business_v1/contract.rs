use std::collections::HashSet;

use super::error::{require_non_empty, DomainError, DomainResult};
use super::QuantityMillis;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractKind {
    OneOff,
    AnnualFramework,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractStatus {
    Draft,
    Effective,
    Completed,
    Voided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementCadence {
    Monthly,
    Quarterly,
    PerOrder,
    OneOff,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    pub id: String,
    pub project_id: String,
    pub number: String,
    pub kind: ContractKind,
    pub status: ContractStatus,
    pub settlement_cadence: SettlementCadence,
    pub deliverables: Vec<Deliverable>,
}

impl Contract {
    pub fn new(
        id: impl Into<String>,
        project_id: impl Into<String>,
        number: impl Into<String>,
        kind: ContractKind,
        settlement_cadence: SettlementCadence,
        deliverables: Vec<Deliverable>,
    ) -> DomainResult<Self> {
        let contract = Self {
            id: id.into(),
            project_id: project_id.into(),
            number: number.into(),
            kind,
            status: ContractStatus::Draft,
            settlement_cadence,
            deliverables,
        };
        contract.validate()?;
        Ok(contract)
    }

    pub fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.id, "contract.id")?;
        require_non_empty(&self.project_id, "contract.project_id")?;
        require_non_empty(&self.number, "contract.number")?;
        if self.deliverables.is_empty() {
            return Err(DomainError::InvalidValue {
                field: "contract.deliverables",
                reason: "must contain at least one deliverable",
            });
        }
        let mut ids = HashSet::new();
        for deliverable in &self.deliverables {
            deliverable.validate()?;
            if !ids.insert(deliverable.id.as_str()) {
                return Err(DomainError::DuplicateReference {
                    entity: "deliverable",
                    id: deliverable.id.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn deliverable(&self, id: &str) -> Option<&Deliverable> {
        self.deliverables.iter().find(|item| item.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deliverable {
    pub id: String,
    pub name: String,
    pub contracted_quantity_millis: QuantityMillis,
    pub material_required: bool,
}

impl Deliverable {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        contracted_quantity_millis: QuantityMillis,
        material_required: bool,
    ) -> DomainResult<Self> {
        let deliverable = Self {
            id: id.into(),
            name: name.into(),
            contracted_quantity_millis,
            material_required,
        };
        deliverable.validate()?;
        Ok(deliverable)
    }

    pub fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.id, "deliverable.id")?;
        require_non_empty(&self.name, "deliverable.name")?;
        if self.contracted_quantity_millis <= 0 {
            return Err(DomainError::InvalidValue {
                field: "deliverable.contracted_quantity_millis",
                reason: "must be positive",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceOrderStatus {
    Draft,
    Confirmed,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceOrder {
    pub id: String,
    pub contract_id: String,
    pub sequence: u32,
    pub status: ServiceOrderStatus,
    pub deliverable_ids: Vec<String>,
}

impl ServiceOrder {
    pub fn new(
        id: impl Into<String>,
        contract: &Contract,
        sequence: u32,
        deliverable_ids: Vec<String>,
    ) -> DomainResult<Self> {
        if contract.kind != ContractKind::AnnualFramework {
            return Err(DomainError::InvalidValue {
                field: "service_order.contract_id",
                reason: "service orders require an annual framework contract",
            });
        }
        if sequence == 0 {
            return Err(DomainError::InvalidValue {
                field: "service_order.sequence",
                reason: "must be positive",
            });
        }
        validate_deliverable_references(contract, &deliverable_ids, "service order")?;
        let order = Self {
            id: id.into(),
            contract_id: contract.id.clone(),
            sequence,
            status: ServiceOrderStatus::Draft,
            deliverable_ids,
        };
        require_non_empty(&order.id, "service_order.id")?;
        Ok(order)
    }
}

pub(crate) fn validate_deliverable_references(
    contract: &Contract,
    deliverable_ids: &[String],
    entity: &'static str,
) -> DomainResult<()> {
    if deliverable_ids.is_empty() {
        return Err(DomainError::InvalidValue {
            field: "deliverable_ids",
            reason: "must contain at least one deliverable",
        });
    }
    let mut seen = HashSet::new();
    for id in deliverable_ids {
        require_non_empty(id, "deliverable_id")?;
        if !seen.insert(id.as_str()) {
            return Err(DomainError::DuplicateReference {
                entity,
                id: id.clone(),
            });
        }
        if contract.deliverable(id).is_none() {
            return Err(DomainError::MissingReference {
                entity: "contract deliverable",
                id: id.clone(),
            });
        }
    }
    Ok(())
}
