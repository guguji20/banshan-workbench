use std::collections::{HashMap, HashSet};

use super::contract::{validate_deliverable_references, Contract};
use super::error::{require_non_empty, DomainError, DomainResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementStatus {
    Draft,
    Confirmed,
    Invoiced,
    Paid,
    Voided,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementBatch {
    pub id: String,
    pub contract_id: String,
    pub acceptance_batch_ids: Vec<String>,
    pub deliverable_ids: Vec<String>,
    pub status: SettlementStatus,
}

impl SettlementBatch {
    pub fn new(
        id: impl Into<String>,
        contract: &Contract,
        acceptance_batch_ids: Vec<String>,
        deliverable_ids: Vec<String>,
    ) -> DomainResult<Self> {
        validate_deliverable_references(contract, &deliverable_ids, "settlement batch")?;
        let batch = Self {
            id: id.into(),
            contract_id: contract.id.clone(),
            acceptance_batch_ids,
            deliverable_ids,
            status: SettlementStatus::Draft,
        };
        batch.validate(contract)?;
        Ok(batch)
    }

    fn validate(&self, contract: &Contract) -> DomainResult<()> {
        require_non_empty(&self.id, "settlement_batch.id")?;
        if self.contract_id != contract.id {
            return Err(DomainError::MismatchedReference {
                field: "settlement_batch.contract_id",
                expected: contract.id.clone(),
                actual: self.contract_id.clone(),
            });
        }
        validate_deliverable_references(contract, &self.deliverable_ids, "settlement batch")?;
        let mut acceptance_ids = HashSet::new();
        for acceptance_id in &self.acceptance_batch_ids {
            require_non_empty(acceptance_id, "settlement_batch.acceptance_batch_id")?;
            if !acceptance_ids.insert(acceptance_id.as_str()) {
                return Err(DomainError::DuplicateReference {
                    entity: "acceptance batch",
                    id: acceptance_id.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettlementLedger {
    deliverable_assignments: HashMap<String, String>,
}

impl SettlementLedger {
    pub fn from_batches<'a>(
        contract: &Contract,
        batches: impl IntoIterator<Item = &'a SettlementBatch>,
    ) -> DomainResult<Self> {
        let mut ledger = Self::default();
        for batch in batches {
            ledger.reserve(contract, batch)?;
        }
        Ok(ledger)
    }

    pub fn reserve(&mut self, contract: &Contract, batch: &SettlementBatch) -> DomainResult<()> {
        batch.validate(contract)?;
        for deliverable_id in &batch.deliverable_ids {
            if let Some(existing_batch_id) = self.deliverable_assignments.get(deliverable_id) {
                if existing_batch_id != &batch.id {
                    return Err(DomainError::AlreadySettled {
                        deliverable_id: deliverable_id.clone(),
                        settlement_batch_id: existing_batch_id.clone(),
                    });
                }
            }
        }
        for deliverable_id in &batch.deliverable_ids {
            self.deliverable_assignments
                .insert(deliverable_id.clone(), batch.id.clone());
        }
        Ok(())
    }

    pub fn release(&mut self, batch: &SettlementBatch) {
        self.deliverable_assignments
            .retain(|_, batch_id| batch_id != &batch.id);
    }

    pub fn settlement_batch_for(&self, deliverable_id: &str) -> Option<&str> {
        self.deliverable_assignments
            .get(deliverable_id)
            .map(String::as_str)
    }
}
