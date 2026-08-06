use std::collections::{HashMap, HashSet};

use super::contract::{validate_deliverable_references, Contract};
use super::error::{require_non_empty, DomainError, DomainResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialKind {
    Script,
    Video,
    Screenshot,
    BehindTheScenes,
    PublishingData,
    Invoice,
    Proof,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialEvidence {
    pub id: String,
    pub deliverable_id: String,
    pub kind: MaterialKind,
    pub source_name: String,
    pub group_key: String,
    pub confirmed: bool,
    pub duplicate: bool,
}

impl MaterialEvidence {
    pub fn new(
        id: impl Into<String>,
        deliverable_id: impl Into<String>,
        kind: MaterialKind,
        source_name: impl Into<String>,
    ) -> DomainResult<Self> {
        let id = id.into();
        let material = Self {
            group_key: id.clone(),
            id,
            deliverable_id: deliverable_id.into(),
            kind,
            source_name: source_name.into(),
            confirmed: true,
            duplicate: false,
        };
        material.validate()?;
        Ok(material)
    }

    pub fn new_grouped(
        id: impl Into<String>,
        deliverable_id: impl Into<String>,
        kind: MaterialKind,
        source_name: impl Into<String>,
        group_key: impl Into<String>,
        confirmed: bool,
        duplicate: bool,
    ) -> DomainResult<Self> {
        let material = Self {
            id: id.into(),
            deliverable_id: deliverable_id.into(),
            kind,
            source_name: source_name.into(),
            group_key: group_key.into(),
            confirmed,
            duplicate,
        };
        material.validate()?;
        Ok(material)
    }

    fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.id, "material_evidence.id")?;
        require_non_empty(&self.deliverable_id, "material_evidence.deliverable_id")?;
        require_non_empty(&self.source_name, "material_evidence.source_name")?;
        require_non_empty(&self.group_key, "material_evidence.group_key")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceBatch {
    pub id: String,
    pub contract_id: String,
    pub service_order_id: Option<String>,
    pub deliverable_ids: Vec<String>,
    pub materials: Vec<MaterialEvidence>,
    required_group_counts: HashMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceReadiness {
    pub missing_deliverable_ids: Vec<String>,
    required_group_counts: HashMap<String, u32>,
    provided_group_counts: HashMap<String, u32>,
    missing_group_counts: HashMap<String, u32>,
}

impl AcceptanceReadiness {
    pub fn is_ready_for_official(&self) -> bool {
        self.missing_deliverable_ids.is_empty()
    }

    pub fn required_group_count(&self, deliverable_id: &str) -> Option<u32> {
        self.required_group_counts.get(deliverable_id).copied()
    }

    pub fn provided_group_count(&self, deliverable_id: &str) -> Option<u32> {
        self.provided_group_counts.get(deliverable_id).copied()
    }

    pub fn missing_group_count(&self, deliverable_id: &str) -> Option<u32> {
        self.missing_group_counts.get(deliverable_id).copied()
    }
}

impl AcceptanceBatch {
    pub fn new(
        id: impl Into<String>,
        contract: &Contract,
        service_order_id: Option<String>,
        deliverable_ids: Vec<String>,
        materials: Vec<MaterialEvidence>,
    ) -> DomainResult<Self> {
        validate_deliverable_references(contract, &deliverable_ids, "acceptance batch")?;
        let required_group_counts = deliverable_ids
            .iter()
            .map(|deliverable_id| (deliverable_id.clone(), 1))
            .collect();
        let batch = Self {
            id: id.into(),
            contract_id: contract.id.clone(),
            service_order_id,
            deliverable_ids,
            materials,
            required_group_counts,
        };
        batch.validate(contract)?;
        Ok(batch)
    }

    pub fn set_required_group_count(
        &mut self,
        deliverable_id: &str,
        required_group_count: u32,
    ) -> DomainResult<()> {
        require_non_empty(deliverable_id, "acceptance_requirement.deliverable_id")?;
        if !self
            .deliverable_ids
            .iter()
            .any(|accepted_id| accepted_id == deliverable_id)
        {
            return Err(DomainError::MissingReference {
                entity: "acceptance deliverable",
                id: deliverable_id.to_owned(),
            });
        }
        if required_group_count == 0 {
            return Err(DomainError::InvalidValue {
                field: "acceptance_requirement.required_group_count",
                reason: "must be positive",
            });
        }
        self.required_group_counts
            .insert(deliverable_id.to_owned(), required_group_count);
        Ok(())
    }

    pub fn required_group_count(&self, deliverable_id: &str) -> Option<u32> {
        self.required_group_counts.get(deliverable_id).copied()
    }

    pub fn readiness(&self, contract: &Contract) -> DomainResult<AcceptanceReadiness> {
        self.validate(contract)?;
        let mut provided_groups: HashMap<&str, HashSet<&str>> = HashMap::new();
        for material in self
            .materials
            .iter()
            .filter(|material| material.confirmed && !material.duplicate)
        {
            provided_groups
                .entry(material.deliverable_id.as_str())
                .or_default()
                .insert(material.group_key.as_str());
        }

        let mut missing_deliverable_ids = Vec::new();
        let mut provided_group_counts = HashMap::new();
        let mut missing_group_counts = HashMap::new();
        for deliverable_id in &self.deliverable_ids {
            let required_group_count = self.required_group_counts[deliverable_id];
            let provided_group_count = provided_groups
                .get(deliverable_id.as_str())
                .map(HashSet::len)
                .unwrap_or_default()
                .try_into()
                .map_err(|_| DomainError::ArithmeticOverflow)?;
            let material_required = contract
                .deliverable(deliverable_id)
                .is_some_and(|deliverable| deliverable.material_required);
            let missing_group_count = if material_required {
                required_group_count.saturating_sub(provided_group_count)
            } else {
                0
            };
            provided_group_counts.insert(deliverable_id.clone(), provided_group_count);
            missing_group_counts.insert(deliverable_id.clone(), missing_group_count);
            if missing_group_count > 0 {
                missing_deliverable_ids.push(deliverable_id.clone());
            }
        }
        Ok(AcceptanceReadiness {
            missing_deliverable_ids,
            required_group_counts: self.required_group_counts.clone(),
            provided_group_counts,
            missing_group_counts,
        })
    }

    pub fn authorize_official(&self, contract: &Contract) -> DomainResult<()> {
        let readiness = self.readiness(contract)?;
        if !readiness.is_ready_for_official() {
            return Err(DomainError::MissingMaterials {
                acceptance_batch_id: self.id.clone(),
                deliverable_ids: readiness.missing_deliverable_ids,
            });
        }
        Ok(())
    }

    fn validate(&self, contract: &Contract) -> DomainResult<()> {
        require_non_empty(&self.id, "acceptance_batch.id")?;
        if self.contract_id != contract.id {
            return Err(DomainError::MismatchedReference {
                field: "acceptance_batch.contract_id",
                expected: contract.id.clone(),
                actual: self.contract_id.clone(),
            });
        }
        validate_deliverable_references(contract, &self.deliverable_ids, "acceptance batch")?;
        let accepted_ids: HashSet<&str> = self.deliverable_ids.iter().map(String::as_str).collect();
        if self.required_group_counts.len() != accepted_ids.len() {
            return Err(DomainError::InvalidValue {
                field: "acceptance_batch.required_group_counts",
                reason: "must contain exactly one count per deliverable",
            });
        }
        for (deliverable_id, required_group_count) in &self.required_group_counts {
            if !accepted_ids.contains(deliverable_id.as_str()) {
                return Err(DomainError::MissingReference {
                    entity: "acceptance deliverable",
                    id: deliverable_id.clone(),
                });
            }
            if *required_group_count == 0 {
                return Err(DomainError::InvalidValue {
                    field: "acceptance_requirement.required_group_count",
                    reason: "must be positive",
                });
            }
        }
        let mut material_ids = HashSet::new();
        for material in &self.materials {
            material.validate()?;
            if !material_ids.insert(material.id.as_str()) {
                return Err(DomainError::DuplicateReference {
                    entity: "material evidence",
                    id: material.id.clone(),
                });
            }
            if !accepted_ids.contains(material.deliverable_id.as_str()) {
                return Err(DomainError::MissingReference {
                    entity: "acceptance deliverable",
                    id: material.deliverable_id.clone(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::business_v1::{ContractKind, Deliverable, SettlementCadence, QUANTITY_SCALE};

    fn contract_with_required_material() -> Contract {
        Contract::new(
            "contract-1",
            "project-1",
            "HT-001",
            ContractKind::OneOff,
            SettlementCadence::OneOff,
            vec![Deliverable::new("delivery-1", "video delivery", QUANTITY_SCALE, true).unwrap()],
        )
        .unwrap()
    }

    fn grouped_material(
        id: &str,
        group_key: &str,
        confirmed: bool,
        duplicate: bool,
    ) -> MaterialEvidence {
        MaterialEvidence::new_grouped(
            id,
            "delivery-1",
            MaterialKind::Video,
            format!("{id}.mp4"),
            group_key,
            confirmed,
            duplicate,
        )
        .unwrap()
    }

    #[test]
    fn readiness_reports_required_four_provided_three_missing_one() {
        let contract = contract_with_required_material();
        let materials = vec![
            grouped_material("material-1", "group-1", true, false),
            grouped_material("material-2", "group-2", true, false),
            grouped_material("material-3", "group-3", true, false),
        ];
        let mut batch = AcceptanceBatch::new(
            "acceptance-1",
            &contract,
            None,
            vec!["delivery-1".to_owned()],
            materials,
        )
        .unwrap();
        batch.set_required_group_count("delivery-1", 4).unwrap();

        let readiness = batch.readiness(&contract).unwrap();

        assert_eq!(readiness.required_group_count("delivery-1"), Some(4));
        assert_eq!(readiness.provided_group_count("delivery-1"), Some(3));
        assert_eq!(readiness.missing_group_count("delivery-1"), Some(1));
        assert_eq!(
            readiness.missing_deliverable_ids,
            vec!["delivery-1".to_owned()]
        );
        assert!(matches!(
            batch.authorize_official(&contract),
            Err(DomainError::MissingMaterials { deliverable_ids, .. })
                if deliverable_ids == vec!["delivery-1".to_owned()]
        ));
    }

    #[test]
    fn readiness_counts_distinct_confirmed_non_duplicate_groups_only() {
        let contract = contract_with_required_material();
        let materials = vec![
            grouped_material("material-1", "group-1", true, false),
            grouped_material("material-1-copy", "group-1", true, false),
            grouped_material("material-2", "group-2", true, false),
            grouped_material("material-duplicate", "group-3", true, true),
            grouped_material("material-unconfirmed", "group-4", false, false),
        ];
        let mut batch = AcceptanceBatch::new(
            "acceptance-2",
            &contract,
            None,
            vec!["delivery-1".to_owned()],
            materials,
        )
        .unwrap();
        batch.set_required_group_count("delivery-1", 4).unwrap();

        let readiness = batch.readiness(&contract).unwrap();

        assert_eq!(readiness.provided_group_count("delivery-1"), Some(2));
        assert_eq!(readiness.missing_group_count("delivery-1"), Some(2));
    }

    #[test]
    fn legacy_constructor_defaults_to_confirmed_unique_groups() {
        let material =
            MaterialEvidence::new("material-1", "delivery-1", MaterialKind::Video, "video.mp4")
                .unwrap();

        assert_eq!(material.group_key, "material-1");
        assert!(material.confirmed);
        assert!(!material.duplicate);
    }

    #[test]
    fn required_group_count_must_be_positive() {
        let contract = contract_with_required_material();
        let mut batch = AcceptanceBatch::new(
            "acceptance-3",
            &contract,
            None,
            vec!["delivery-1".to_owned()],
            Vec::new(),
        )
        .unwrap();

        assert!(matches!(
            batch.set_required_group_count("delivery-1", 0),
            Err(DomainError::InvalidValue {
                field: "acceptance_requirement.required_group_count",
                ..
            })
        ));
    }
}
