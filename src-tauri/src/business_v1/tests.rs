use super::*;

fn annual_contract() -> Contract {
    Contract::new(
        "contract-annual-1",
        "project-1",
        "FRAME-2026-001",
        ContractKind::AnnualFramework,
        SettlementCadence::Quarterly,
        (1..=4)
            .map(|index| {
                Deliverable::new(
                    format!("delivery-{index}"),
                    format!("video {index}"),
                    1_000,
                    true,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn company_and_project_reject_invalid_identity_data() {
    assert!(matches!(
        Company::new("company-1", "Banshan", "Banshan", "", 600),
        Err(DomainError::EmptyField(
            "company.unified_social_credit_code"
        ))
    ));
    assert!(matches!(
        CustomerProject::new("project-1", "", "Baietan", "Customer Ltd", "company-1"),
        Err(DomainError::EmptyField(
            "customer_project.customer_group_id"
        ))
    ));
}

#[test]
fn service_orders_only_belong_to_annual_framework_contracts() {
    let one_off = Contract::new(
        "contract-1",
        "project-1",
        "ONE-001",
        ContractKind::OneOff,
        SettlementCadence::OneOff,
        vec![Deliverable::new("delivery-1", "video", 1_000, true).unwrap()],
    )
    .unwrap();
    let result = ServiceOrder::new("order-1", &one_off, 1, vec!["delivery-1".to_owned()]);
    assert!(matches!(
        result,
        Err(DomainError::InvalidValue {
            field: "service_order.contract_id",
            ..
        })
    ));
}

#[test]
fn baietan_quote_preserves_unit_price_and_applies_project_discount() {
    let quotation = Quotation {
        id: "quote-1".to_owned(),
        project_id: "baietan".to_owned(),
        version: 1,
        currency: "CNY".to_owned(),
        status: QuotationStatus::Draft,
        items: vec![QuotationItem {
            id: "item-1".to_owned(),
            description: "video production".to_owned(),
            quantity_millis: 4_000,
            tax_inclusive_unit_price_cents: 2_120_000,
            tax_rate_basis_points: 600,
        }],
        project_discount_cents: 490_000,
    };
    let calculated = quotation.calculate().unwrap();
    assert_eq!(calculated.original_total_cents, 8_480_000);
    assert_eq!(calculated.project_discount_cents, 490_000);
    assert_eq!(calculated.final_tax_inclusive_total_cents, 7_990_000);
    assert_eq!(
        calculated.items[0].tax_inclusive_unit_price_cents,
        2_120_000
    );
    assert_eq!(
        calculated.items[0].allocated_project_discount_cents,
        490_000
    );
    assert_eq!(calculated.tax_exclusive_total_cents, 7_537_736);
    assert_eq!(calculated.tax_total_cents, 452_264);
    calculated.verify().unwrap();
}

#[test]
fn discount_allocation_is_deterministic_and_reconciles_to_the_cent() {
    let quotation = Quotation {
        id: "quote-2".to_owned(),
        project_id: "project-1".to_owned(),
        version: 1,
        currency: "CNY".to_owned(),
        status: QuotationStatus::Draft,
        items: (1..=3)
            .map(|index| QuotationItem {
                id: format!("item-{index}"),
                description: format!("line {index}"),
                quantity_millis: 1_000,
                tax_inclusive_unit_price_cents: 100,
                tax_rate_basis_points: 600,
            })
            .collect(),
        project_discount_cents: 100,
    };
    let first = quotation.calculate().unwrap();
    let second = quotation.calculate().unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item.allocated_project_discount_cents)
            .collect::<Vec<_>>(),
        vec![34, 33, 33]
    );
    assert_eq!(first.final_tax_inclusive_total_cents, 200);
    first.verify().unwrap();
}

#[test]
fn project_discount_cannot_exceed_original_quote_total() {
    let quotation = Quotation {
        id: "quote-3".to_owned(),
        project_id: "project-1".to_owned(),
        version: 1,
        currency: "CNY".to_owned(),
        status: QuotationStatus::Draft,
        items: vec![QuotationItem {
            id: "item-1".to_owned(),
            description: "service".to_owned(),
            quantity_millis: 1_000,
            tax_inclusive_unit_price_cents: 100,
            tax_rate_basis_points: 0,
        }],
        project_discount_cents: 101,
    };
    assert!(matches!(
        quotation.calculate(),
        Err(DomainError::InvalidValue {
            field: "quotation.project_discount_cents",
            ..
        })
    ));
}

#[test]
fn acceptance_blocks_official_output_when_required_material_is_missing() {
    let contract = annual_contract();
    let materials = (1..=3)
        .map(|index| {
            MaterialEvidence::new(
                format!("material-{index}"),
                format!("delivery-{index}"),
                MaterialKind::Video,
                format!("video-{index}.mp4"),
            )
            .unwrap()
        })
        .collect();
    let batch = AcceptanceBatch::new(
        "acceptance-1",
        &contract,
        Some("order-1".to_owned()),
        (1..=4).map(|index| format!("delivery-{index}")).collect(),
        materials,
    )
    .unwrap();
    let readiness = batch.readiness(&contract).unwrap();
    assert_eq!(
        readiness.missing_deliverable_ids,
        vec!["delivery-4".to_owned()]
    );
    assert!(matches!(batch.authorize_official(&contract),
        Err(DomainError::MissingMaterials { deliverable_ids, .. })
        if deliverable_ids == vec!["delivery-4".to_owned()]
    ));
}

#[test]
fn acceptance_allows_official_output_after_material_is_complete() {
    let contract = annual_contract();
    let materials = (1..=4)
        .map(|index| {
            MaterialEvidence::new(
                format!("material-{index}"),
                format!("delivery-{index}"),
                MaterialKind::Video,
                format!("video-{index}.mp4"),
            )
            .unwrap()
        })
        .collect();
    let batch = AcceptanceBatch::new(
        "acceptance-2",
        &contract,
        None,
        (1..=4).map(|index| format!("delivery-{index}")).collect(),
        materials,
    )
    .unwrap();
    batch.authorize_official(&contract).unwrap();
}

#[test]
fn annual_deliverable_cannot_appear_in_two_settlement_batches() {
    let contract = annual_contract();
    let first = SettlementBatch::new(
        "settlement-q1",
        &contract,
        vec!["acceptance-q1".to_owned()],
        vec!["delivery-1".to_owned(), "delivery-2".to_owned()],
    )
    .unwrap();
    let second = SettlementBatch::new(
        "settlement-q2",
        &contract,
        vec!["acceptance-q2".to_owned()],
        vec!["delivery-2".to_owned(), "delivery-3".to_owned()],
    )
    .unwrap();
    let mut ledger = SettlementLedger::default();
    ledger.reserve(&contract, &first).unwrap();
    let result = ledger.reserve(&contract, &second);
    assert_eq!(
        result,
        Err(DomainError::AlreadySettled {
            deliverable_id: "delivery-2".to_owned(),
            settlement_batch_id: "settlement-q1".to_owned(),
        })
    );
    assert_eq!(ledger.settlement_batch_for("delivery-3"), None);
}

#[test]
fn settlement_reservation_is_idempotent_and_release_is_scoped() {
    let contract = annual_contract();
    let batch = SettlementBatch::new(
        "settlement-q1",
        &contract,
        vec![],
        vec!["delivery-1".to_owned()],
    )
    .unwrap();
    let mut ledger = SettlementLedger::default();
    ledger.reserve(&contract, &batch).unwrap();
    ledger.reserve(&contract, &batch).unwrap();
    assert_eq!(
        ledger.settlement_batch_for("delivery-1"),
        Some("settlement-q1")
    );
    ledger.release(&batch);
    assert_eq!(ledger.settlement_batch_for("delivery-1"), None);
}
