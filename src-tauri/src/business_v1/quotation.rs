use std::cmp::Ordering;

use super::error::{require_non_empty, DomainError, DomainResult};
use super::{BasisPoints, MinorUnits, QuantityMillis, BASIS_POINT_SCALE, QUANTITY_SCALE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotationStatus {
    Draft,
    InReview,
    Approved,
    Voided,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quotation {
    pub id: String,
    pub project_id: String,
    pub version: u32,
    pub currency: String,
    pub status: QuotationStatus,
    pub items: Vec<QuotationItem>,
    pub project_discount_cents: MinorUnits,
}

impl Quotation {
    pub fn calculate(&self) -> DomainResult<CalculatedQuotation> {
        require_non_empty(&self.id, "quotation.id")?;
        require_non_empty(&self.project_id, "quotation.project_id")?;
        require_non_empty(&self.currency, "quotation.currency")?;
        if self.version == 0 {
            return Err(DomainError::InvalidValue {
                field: "quotation.version",
                reason: "must be positive",
            });
        }
        if self.items.is_empty() {
            return Err(DomainError::InvalidValue {
                field: "quotation.items",
                reason: "must contain at least one item",
            });
        }
        if self.project_discount_cents < 0 {
            return Err(DomainError::InvalidValue {
                field: "quotation.project_discount_cents",
                reason: "must not be negative",
            });
        }

        let mut calculated = Vec::with_capacity(self.items.len());
        let mut original_total = 0_i64;
        for item in &self.items {
            let line_total = item.original_total_cents()?;
            original_total = original_total
                .checked_add(line_total)
                .ok_or(DomainError::ArithmeticOverflow)?;
            calculated.push(CalculatedQuotationItem {
                item_id: item.id.clone(),
                description: item.description.clone(),
                quantity_millis: item.quantity_millis,
                tax_inclusive_unit_price_cents: item.tax_inclusive_unit_price_cents,
                tax_rate_basis_points: item.tax_rate_basis_points,
                original_total_cents: line_total,
                allocated_project_discount_cents: 0,
                final_tax_inclusive_cents: 0,
                tax_cents: 0,
                tax_exclusive_cents: 0,
            });
        }
        if self.project_discount_cents > original_total {
            return Err(DomainError::InvalidValue {
                field: "quotation.project_discount_cents",
                reason: "must not exceed original total",
            });
        }

        let line_totals = calculated
            .iter()
            .map(|item| item.original_total_cents)
            .collect::<Vec<_>>();
        let allocations =
            allocate_discount(self.project_discount_cents, &line_totals, original_total)?;
        let mut tax_exclusive_total = 0_i64;
        let mut tax_total = 0_i64;
        for ((item, source), discount) in calculated.iter_mut().zip(&self.items).zip(allocations) {
            item.allocated_project_discount_cents = discount;
            item.final_tax_inclusive_cents = item
                .original_total_cents
                .checked_sub(discount)
                .ok_or(DomainError::ArithmeticOverflow)?;
            item.tax_exclusive_cents = divide_round_half_up(
                i128::from(item.final_tax_inclusive_cents) * i128::from(BASIS_POINT_SCALE),
                i128::from(BASIS_POINT_SCALE) + i128::from(source.tax_rate_basis_points),
            )?;
            item.tax_cents = item
                .final_tax_inclusive_cents
                .checked_sub(item.tax_exclusive_cents)
                .ok_or(DomainError::ArithmeticOverflow)?;
            tax_exclusive_total = tax_exclusive_total
                .checked_add(item.tax_exclusive_cents)
                .ok_or(DomainError::ArithmeticOverflow)?;
            tax_total = tax_total
                .checked_add(item.tax_cents)
                .ok_or(DomainError::ArithmeticOverflow)?;
        }

        let result = CalculatedQuotation {
            quotation_id: self.id.clone(),
            currency: self.currency.clone(),
            items: calculated,
            original_total_cents: original_total,
            project_discount_cents: self.project_discount_cents,
            final_tax_inclusive_total_cents: original_total
                .checked_sub(self.project_discount_cents)
                .ok_or(DomainError::ArithmeticOverflow)?,
            tax_exclusive_total_cents: tax_exclusive_total,
            tax_total_cents: tax_total,
        };
        result.verify()?;
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotationItem {
    pub id: String,
    pub description: String,
    pub quantity_millis: QuantityMillis,
    pub tax_inclusive_unit_price_cents: MinorUnits,
    pub tax_rate_basis_points: BasisPoints,
}

impl QuotationItem {
    pub fn original_total_cents(&self) -> DomainResult<MinorUnits> {
        require_non_empty(&self.id, "quotation_item.id")?;
        require_non_empty(&self.description, "quotation_item.description")?;
        if self.quantity_millis <= 0 {
            return Err(DomainError::InvalidValue {
                field: "quotation_item.quantity_millis",
                reason: "must be positive",
            });
        }
        if self.tax_inclusive_unit_price_cents < 0 {
            return Err(DomainError::InvalidValue {
                field: "quotation_item.tax_inclusive_unit_price_cents",
                reason: "must not be negative",
            });
        }
        if self.tax_rate_basis_points > 10_000 {
            return Err(DomainError::InvalidValue {
                field: "quotation_item.tax_rate_basis_points",
                reason: "must be at most 10000",
            });
        }
        divide_round_half_up(
            i128::from(self.quantity_millis) * i128::from(self.tax_inclusive_unit_price_cents),
            i128::from(QUANTITY_SCALE),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculatedQuotationItem {
    pub item_id: String,
    pub description: String,
    pub quantity_millis: QuantityMillis,
    pub tax_inclusive_unit_price_cents: MinorUnits,
    pub tax_rate_basis_points: BasisPoints,
    pub original_total_cents: MinorUnits,
    pub allocated_project_discount_cents: MinorUnits,
    pub final_tax_inclusive_cents: MinorUnits,
    pub tax_cents: MinorUnits,
    pub tax_exclusive_cents: MinorUnits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculatedQuotation {
    pub quotation_id: String,
    pub currency: String,
    pub items: Vec<CalculatedQuotationItem>,
    pub original_total_cents: MinorUnits,
    pub project_discount_cents: MinorUnits,
    pub final_tax_inclusive_total_cents: MinorUnits,
    pub tax_exclusive_total_cents: MinorUnits,
    pub tax_total_cents: MinorUnits,
}

impl CalculatedQuotation {
    pub fn verify(&self) -> DomainResult<()> {
        let original_total = checked_sum(self.items.iter().map(|item| item.original_total_cents))?;
        let allocated_discount = checked_sum(
            self.items
                .iter()
                .map(|item| item.allocated_project_discount_cents),
        )?;
        let final_total =
            checked_sum(self.items.iter().map(|item| item.final_tax_inclusive_cents))?;
        let tax_total = checked_sum(self.items.iter().map(|item| item.tax_cents))?;
        let tax_exclusive_total =
            checked_sum(self.items.iter().map(|item| item.tax_exclusive_cents))?;
        let valid = original_total == self.original_total_cents
            && allocated_discount == self.project_discount_cents
            && final_total == self.final_tax_inclusive_total_cents
            && tax_total == self.tax_total_cents
            && tax_exclusive_total == self.tax_exclusive_total_cents
            && self
                .original_total_cents
                .checked_sub(self.project_discount_cents)
                == Some(self.final_tax_inclusive_total_cents)
            && self
                .tax_exclusive_total_cents
                .checked_add(self.tax_total_cents)
                == Some(self.final_tax_inclusive_total_cents)
            && self.items.iter().all(|item| {
                item.original_total_cents
                    .checked_sub(item.allocated_project_discount_cents)
                    == Some(item.final_tax_inclusive_cents)
                    && item.tax_exclusive_cents.checked_add(item.tax_cents)
                        == Some(item.final_tax_inclusive_cents)
            });
        if !valid {
            return Err(DomainError::InvalidValue {
                field: "calculated_quotation",
                reason: "totals or deterministic discount allocation do not reconcile",
            });
        }
        Ok(())
    }
}

fn allocate_discount(
    discount: MinorUnits,
    line_totals: &[MinorUnits],
    original_total: MinorUnits,
) -> DomainResult<Vec<MinorUnits>> {
    if discount == 0 {
        return Ok(vec![0; line_totals.len()]);
    }
    if original_total <= 0 {
        return Err(DomainError::InvalidValue {
            field: "quotation.original_total",
            reason: "must be positive when a project discount exists",
        });
    }
    let denominator = i128::from(original_total);
    let mut allocations = Vec::with_capacity(line_totals.len());
    let mut remainders = Vec::with_capacity(line_totals.len());
    let mut allocated = 0_i64;
    for (index, line_total) in line_totals.iter().copied().enumerate() {
        let numerator = i128::from(discount) * i128::from(line_total);
        let base =
            i64::try_from(numerator / denominator).map_err(|_| DomainError::ArithmeticOverflow)?;
        allocations.push(base);
        allocated = allocated
            .checked_add(base)
            .ok_or(DomainError::ArithmeticOverflow)?;
        remainders.push((index, numerator % denominator));
    }
    remainders.sort_by(|left, right| match right.1.cmp(&left.1) {
        Ordering::Equal => left.0.cmp(&right.0),
        ordering => ordering,
    });
    let remaining = discount
        .checked_sub(allocated)
        .ok_or(DomainError::ArithmeticOverflow)?;
    for (index, _) in remainders.into_iter().take(remaining as usize) {
        allocations[index] = allocations[index]
            .checked_add(1)
            .ok_or(DomainError::ArithmeticOverflow)?;
    }
    Ok(allocations)
}

fn checked_sum(mut values: impl Iterator<Item = MinorUnits>) -> DomainResult<MinorUnits> {
    values.try_fold(0_i64, |sum, value| {
        sum.checked_add(value)
            .ok_or(DomainError::ArithmeticOverflow)
    })
}

fn divide_round_half_up(numerator: i128, denominator: i128) -> DomainResult<MinorUnits> {
    if numerator < 0 || denominator <= 0 {
        return Err(DomainError::InvalidValue {
            field: "money_calculation",
            reason: "requires a non-negative numerator and positive denominator",
        });
    }
    let rounded = numerator
        .checked_add(denominator / 2)
        .ok_or(DomainError::ArithmeticOverflow)?
        / denominator;
    i64::try_from(rounded).map_err(|_| DomainError::ArithmeticOverflow)
}
