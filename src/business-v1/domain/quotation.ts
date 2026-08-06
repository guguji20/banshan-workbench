import type {
  AuditedField,
  VersionAuditEntry,
} from "./audit";
import { validateFieldAudit, validateVersionAudit } from "./audit";
import {
  addMoney,
  money,
  multiplyMoney,
  percentageOf,
  subtractMoney,
  type Money,
} from "./money";
import {
  assertSafeArithmetic,
  assertValid,
  nonNegativeInteger,
  positiveInteger,
  requiredText,
  type ValidationIssue,
} from "./validation";

export type QuotationStatus =
  | "draft"
  | "readyForReview"
  | "confirmed"
  | "withdrawn";

export interface QuotationItem {
  id: string;
  description: AuditedField<string>;
  quantity: AuditedField<number>;
  unitPrice: AuditedField<Money>;
}

export interface Discount {
  id: string;
  label: AuditedField<string>;
  amount: AuditedField<Money>;
}

export interface Tax {
  basisPoints: AuditedField<number>;
  mode: "taxExclusive" | "taxInclusive";
}

export interface QuotationVersion {
  number: number;
  parentVersion?: number;
  createdBy: string;
  createdAt: string;
  note?: string;
}

export interface Quotation {
  id: string;
  companyId: string;
  customerProjectId: string;
  title: AuditedField<string>;
  status: QuotationStatus;
  version: QuotationVersion;
  items: readonly QuotationItem[];
  discounts: readonly Discount[];
  tax: Tax;
  versionAudit: readonly VersionAuditEntry[];
}

export interface QuotationItemCalculation {
  itemId: string;
  quantity: number;
  unitPrice: Money;
  lineTotal: Money;
}

export interface QuotationTotals {
  items: readonly QuotationItemCalculation[];
  subtotal: Money;
  discountTotal: Money;
  taxableAmount: Money;
  taxAmount: Money;
  finalTotal: Money;
}

export type QuotationTransition = "submitForReview" | "confirm" | "withdraw" | "revise";

export interface TransitionContext {
  actorId: string;
  occurredAt: string;
  note?: string;
}

const transitions: Record<
  QuotationStatus,
  Partial<Record<QuotationTransition, QuotationStatus>>
> = {
  draft: { submitForReview: "readyForReview", withdraw: "withdrawn" },
  readyForReview: { confirm: "confirmed", revise: "draft", withdraw: "withdrawn" },
  confirmed: { revise: "draft", withdraw: "withdrawn" },
  withdrawn: { revise: "draft" },
};

export function calculateQuotation(quotation: Quotation): QuotationTotals {
  validateQuotation(quotation);
  const items = quotation.items.map((item) => ({
    itemId: item.id,
    quantity: item.quantity.value,
    unitPrice: money(item.unitPrice.value.cents, item.unitPrice.value.currency),
    lineTotal: multiplyMoney(item.unitPrice.value, item.quantity.value),
  }));
  const subtotal = addMoney(...items.map((item) => item.lineTotal));
  const discountTotal = addMoney(
    ...quotation.discounts.map((discount) => discount.amount.value),
  );
  const taxableAmount = subtractMoney(subtotal, discountTotal);
  const taxAmount = percentageOf(taxableAmount, quotation.tax.basisPoints.value);
  const finalTotal =
    quotation.tax.mode === "taxExclusive"
      ? addMoney(taxableAmount, taxAmount)
      : taxableAmount;
  return { items, subtotal, discountTotal, taxableAmount, taxAmount, finalTotal };
}

export function transitionQuotation(
  quotation: Quotation,
  transition: QuotationTransition,
  context: TransitionContext,
): Quotation {
  validateQuotation(quotation);
  const nextStatus = transitions[quotation.status][transition];
  if (!nextStatus) {
    throw new Error(`Cannot ${transition} quotation from ${quotation.status}`);
  }
  requiredTransitionContext(context);

  if (transition === "confirm") {
    calculateQuotation(quotation);
    return {
      ...quotation,
      status: nextStatus,
      versionAudit: [
        ...quotation.versionAudit,
        auditEntry(quotation.version.number, "confirmed", context),
      ],
    };
  }

  if (transition === "revise") {
    const nextVersion = assertSafeArithmetic(
      quotation.version.number + 1,
      "quotation.version.number",
    );
    return {
      ...quotation,
      status: nextStatus,
      version: {
        number: nextVersion,
        parentVersion: quotation.version.number,
        createdBy: context.actorId,
        createdAt: context.occurredAt,
        note: context.note,
      },
      versionAudit: [
        ...quotation.versionAudit,
        auditEntry(nextVersion, "revised", context),
      ],
    };
  }

  return {
    ...quotation,
    status: nextStatus,
    versionAudit: [
      ...quotation.versionAudit,
      auditEntry(
        quotation.version.number,
        transition === "submitForReview" ? "submitted" : "withdrawn",
        context,
      ),
    ],
  };
}

export function validateQuotation(quotation: Quotation): void {
  const issues: ValidationIssue[] = [];
  requiredText(quotation.id, "quotation.id", issues);
  requiredText(quotation.companyId, "quotation.companyId", issues);
  requiredText(quotation.customerProjectId, "quotation.customerProjectId", issues);
  requiredText(quotation.title.value, "quotation.title", issues);
  positiveInteger(quotation.version.number, "quotation.version.number", issues);
  requiredText(quotation.version.createdBy, "quotation.version.createdBy", issues);
  requiredText(quotation.version.createdAt, "quotation.version.createdAt", issues);
  if (quotation.items.length === 0) {
    issues.push({ field: "quotation.items", code: "item_required", message: "must not be empty" });
  }
  const itemIds = new Set<string>();
  quotation.items.forEach((item, index) => {
    requiredText(item.id, `quotation.items[${index}].id`, issues);
    requiredText(item.description.value, `quotation.items[${index}].description`, issues);
    positiveInteger(item.quantity.value, `quotation.items[${index}].quantity`, issues);
    nonNegativeInteger(item.unitPrice.value.cents, `quotation.items[${index}].unitPrice`, issues);
    if (itemIds.has(item.id)) {
      issues.push({ field: `quotation.items[${index}].id`, code: "duplicate_id", message: "must be unique" });
    }
    itemIds.add(item.id);
  });
  let discountCents = 0;
  const discountIds = new Set<string>();
  quotation.discounts.forEach((discount, index) => {
    requiredText(discount.id, `quotation.discounts[${index}].id`, issues);
    requiredText(discount.label.value, `quotation.discounts[${index}].label`, issues);
    nonNegativeInteger(discount.amount.value.cents, `quotation.discounts[${index}].amount`, issues);
    discountCents = assertSafeArithmetic(
      discountCents + discount.amount.value.cents,
      "quotation.discountTotal",
    );
    if (discountIds.has(discount.id)) {
      issues.push({ field: `quotation.discounts[${index}].id`, code: "duplicate_id", message: "must be unique" });
    }
    discountIds.add(discount.id);
  });
  nonNegativeInteger(quotation.tax.basisPoints.value, "quotation.tax.basisPoints", issues);
  if (quotation.tax.basisPoints.value > 10_000) {
    issues.push({ field: "quotation.tax.basisPoints", code: "tax_rate_range", message: "must not exceed 10000 basis points" });
  }
  assertValid(issues);

  validateFieldAudit(quotation.title.audit);
  quotation.items.forEach((item) => {
    validateFieldAudit(item.description.audit);
    validateFieldAudit(item.quantity.audit);
    validateFieldAudit(item.unitPrice.audit);
  });
  quotation.discounts.forEach((discount) => {
    validateFieldAudit(discount.label.audit);
    validateFieldAudit(discount.amount.audit);
  });
  validateFieldAudit(quotation.tax.basisPoints.audit);
  validateVersionAudit(quotation.versionAudit, quotation.version.number);

  const subtotalCents = quotation.items.reduce(
    (total, item) =>
      assertSafeArithmetic(
        total + multiplyMoney(item.unitPrice.value, item.quantity.value).cents,
        "quotation.subtotal",
      ),
    0,
  );
  if (discountCents > subtotalCents) {
    assertValid([
      { field: "quotation.discounts", code: "discount_exceeds_subtotal", message: "must not exceed subtotal" },
    ]);
  }
}

function requiredTransitionContext(context: TransitionContext): void {
  const issues: ValidationIssue[] = [];
  requiredText(context.actorId, "transition.actorId", issues);
  requiredText(context.occurredAt, "transition.occurredAt", issues);
  assertValid(issues);
}

function auditEntry(
  version: number,
  action: VersionAuditEntry["action"],
  context: TransitionContext,
): VersionAuditEntry {
  return {
    version,
    action,
    actorId: context.actorId,
    occurredAt: context.occurredAt,
    note: context.note,
  };
}
