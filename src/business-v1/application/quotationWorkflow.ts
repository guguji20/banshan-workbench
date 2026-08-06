import {
  calculateQuotation,
  createAuditedField,
  money,
  type FieldAudit,
  type FieldSourceKind,
  type Quotation,
  type QuotationTotals,
} from "../domain";

export interface QuotationLineInput {
  id: string;
  description: string;
  quantity: number;
  unitPriceCents: number;
}

export interface CreateQuotationDraftInput {
  id: string;
  companyId: string;
  customerProjectId: string;
  title: string;
  lines: readonly QuotationLineInput[];
  projectDiscountCents: number;
  taxBasisPoints: number;
  taxMode: "taxExclusive" | "taxInclusive";
  actorId: string;
  sourceKind: FieldSourceKind;
  sourceId: string;
  sourceLabel: string;
  createdAt: string;
}

export interface QuotationDraftResult {
  quotation: Quotation;
  totals: QuotationTotals;
  summary: {
    subtotal: string;
    projectDiscount: string;
    tax: string;
    finalTotal: string;
  };
}

export function createQuotationDraft(input: CreateQuotationDraftInput): QuotationDraftResult {
  const audit: FieldAudit = {
    version: 1,
    sources: [{
      kind: input.sourceKind,
      referenceId: input.sourceId,
      label: input.sourceLabel,
      capturedAt: input.createdAt,
    }],
  };
  const field = <T,>(value: T) => createAuditedField(value, audit);
  const quotation: Quotation = {
    id: input.id,
    companyId: input.companyId,
    customerProjectId: input.customerProjectId,
    title: field(input.title),
    status: "draft",
    version: { number: 1, createdBy: input.actorId, createdAt: input.createdAt },
    items: input.lines.map((line) => ({
      id: line.id,
      description: field(line.description),
      quantity: field(line.quantity),
      unitPrice: field(money(line.unitPriceCents)),
    })),
    discounts: input.projectDiscountCents > 0 ? [{
      id: `${input.id}:project-discount`,
      label: field("项目优惠"),
      amount: field(money(input.projectDiscountCents)),
    }] : [],
    tax: { basisPoints: field(input.taxBasisPoints), mode: input.taxMode },
    versionAudit: [{
      version: 1,
      action: "created",
      actorId: input.actorId,
      occurredAt: input.createdAt,
    }],
  };
  const totals = calculateQuotation(quotation);
  return {
    quotation,
    totals,
    summary: {
      subtotal: formatCny(totals.subtotal.cents),
      projectDiscount: formatCny(totals.discountTotal.cents),
      tax: formatCny(totals.taxAmount.cents),
      finalTotal: formatCny(totals.finalTotal.cents),
    },
  };
}

export function formatCny(cents: number): string {
  return new Intl.NumberFormat("zh-CN", {
    style: "currency",
    currency: "CNY",
    minimumFractionDigits: 2,
  }).format(cents / 100);
}
