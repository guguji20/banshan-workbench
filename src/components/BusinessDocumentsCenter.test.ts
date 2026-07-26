import { describe, expect, it } from "vitest";
import type { AssetRecord } from "../generated/bsaigc/AssetRecord";
import type { BusinessDocumentRecord } from "../generated/bsaigc/BusinessDocumentRecord";
import type { BusinessPaymentRecord } from "../generated/bsaigc/BusinessPaymentRecord";
import type { BusinessQuoteConfirmationRecord } from "../generated/bsaigc/BusinessQuoteConfirmationRecord";
import type { BusinessReceiptRecord } from "../generated/bsaigc/BusinessReceiptRecord";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import type { RequirementBriefRecord } from "../generated/bsaigc/RequirementBriefRecord";
import type { BusinessWorkspacePrefillChange } from "../generated/bsaigc/BusinessWorkspacePrefillChange";
import {
  applyHistoryLineItems,
  applyQuoteTemplateItems,
  archiveWorkspaceBlockReason,
  businessAssetDisplayName,
  documentCreationBlockReason,
  formatPrefillValue,
  outstandingPaymentCents,
  PREFILL_FIELD_LABELS,
  QUOTE_TEMPLATES,
  requirementAdoptionBlockReason,
  reversibleReceiptCents,
  summarizePrefillChanges,
} from "./BusinessDocumentsCenter";

function makeWorkspace(
  overrides: Partial<BusinessWorkspaceRecord> = {},
): BusinessWorkspaceRecord {
  return {
    id: "workspace-1",
    projectId: "project-1",
    customerId: "customer-1",
    customer: {
      id: "customer-1",
      displayName: "华邦",
      legalName: "华邦有限公司",
      taxId: "",
      billingAddress: "",
      primaryContactName: "",
      primaryPhone: "",
      primaryEmail: "",
      notes: "",
      status: "active",
      revision: 1,
      createdAt: 1,
      updatedAt: 1,
      archivedAt: null,
      archivedBy: null,
    },
    requirementBriefId: null,
    requirementBriefRevision: null,
    prefillSourceWorkspaceId: null,
    profile: {
      projectTitle: "华邦年度品牌视频",
      projectCode: "HB-2026",
      customerName: "华邦",
      customerLegalName: "华邦有限公司",
      customerTaxId: "",
      customerAddress: "",
      customerContact: "",
      customerPhone: "",
      customerEmail: "",
      supplierLegalName: "半山文化传媒有限公司",
      supplierTaxId: "",
      supplierAddress: "",
      supplierContact: "",
      supplierPhone: "",
      supplierBankName: "招商银行",
      supplierBankAccount: "1234567890",
      currency: "CNY",
      defaultTaxRateBps: 600,
      serviceStartAt: 1,
      serviceEndAt: 2,
      deliverySummary: "成片一条",
      paymentTerms: "验收后 30 天付款",
      acceptanceTerms: "客户确认成片",
      notes: "",
      lineItems: [
        {
          id: "line-1",
          name: "品牌视频制作",
          description: "",
          quantityMillis: 1_000,
          unit: "项",
          unitPriceCents: 100_000,
          taxRateBps: 600,
          amountCents: 100_000,
        },
      ],
    } as BusinessWorkspaceRecord["profile"],
    documents: [],
    payments: [],
    quoteConfirmations: [],
    receipts: [],
    milestones: [],
    deliverySubmissions: [],
    invoices: [],
    archiveSnapshots: [],
    archiveIntegrityStatus: "notCaptured",
    status: "active",
    archivedAt: null,
    archivedBy: null,
    lifecycleStage: "draft",
    financialSummary: {
      quotedCents: 0,
      contractCents: 0,
      scheduledCents: 0,
      requestedCents: 0,
      receivedCents: 0,
      outstandingCents: 0,
    },
    currentDocuments: {
      quoteDocumentId: null,
      contractDocumentId: null,
      paymentRequestDocumentId: null,
      acceptanceDocumentId: null,
    },
    revision: 1,
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

describe("BusinessDocumentsCenter lifecycle guards", () => {
  it("shows human filenames for Vault selections without exposing asset ids", () => {
    const assets = [
      { id: "asset-invoice", originalName: "华邦项目发票.pdf" },
    ] as AssetRecord[];
    expect(businessAssetDisplayName(assets, "asset-invoice")).toBe("华邦项目发票.pdf");
    expect(businessAssetDisplayName(assets, "asset-missing")).toBe("已选择文件");
  });

  it("requires confirmation of the exact current generated quote before contract creation", () => {
    const quote = {
      id: "quote-1",
      kind: "quote",
      status: "generated",
      revision: 3,
      outputAssetId: "asset-quote-1",
    } as BusinessDocumentRecord;
    const base = makeWorkspace({
      documents: [quote],
      currentDocuments: {
        quoteDocumentId: quote.id,
        contractDocumentId: null,
        paymentRequestDocumentId: null,
        acceptanceDocumentId: null,
      },
    });

    expect(documentCreationBlockReason(base, "contract")).toContain("确认凭证");

    const confirmation = {
      quoteDocumentId: quote.id,
      quoteDocumentRevision: quote.revision,
      quoteAssetId: quote.outputAssetId,
    } as BusinessQuoteConfirmationRecord;
    expect(
      documentCreationBlockReason(
        { ...base, quoteConfirmations: [confirmation] },
        "contract",
      ),
    ).toBeNull();
  });

  it("tracks partial receipts and remaining reversible amount from immutable ledger rows", () => {
    const payment = {
      id: "payment-1",
      amountCents: 100_000,
    } as BusinessPaymentRecord;
    const original = {
      id: "receipt-1",
      paymentId: payment.id,
      kind: "receipt",
      amountCents: 70_000,
    } as BusinessReceiptRecord;
    const reversal = {
      id: "reversal-1",
      paymentId: payment.id,
      kind: "reversal",
      amountCents: 20_000,
      reversesReceiptId: original.id,
    } as BusinessReceiptRecord;
    const workspace = makeWorkspace({
      payments: [payment],
      receipts: [original, reversal],
    });

    expect(outstandingPaymentCents(workspace, payment)).toBe(50_000);
    expect(reversibleReceiptCents(workspace, original)).toBe(50_000);
    expect(reversibleReceiptCents(workspace, reversal)).toBe(0);
  });

  it("requires preflight, then a fresh snapshot, before archive", () => {
    const contract = {
      id: "contract-1",
      kind: "contract",
      documentNumber: "C-001",
      status: "effective",
    } as BusinessDocumentRecord;
    const acceptance = {
      id: "acceptance-1",
      kind: "acceptance",
      documentNumber: "A-001",
      status: "effective",
    } as BusinessDocumentRecord;
    const readyForSnapshot = makeWorkspace({
      profile: { currency: "CNY" } as BusinessWorkspaceRecord["profile"],
      documents: [contract, acceptance],
      currentDocuments: {
        quoteDocumentId: null,
        contractDocumentId: contract.id,
        paymentRequestDocumentId: null,
        acceptanceDocumentId: acceptance.id,
      },
      milestones: [{
        id: "milestone-1",
        required: true,
        status: "accepted",
        deliverables: [{
          id: "deliverable-1",
          required: true,
          versions: [{ id: "version-1", status: "accepted" }],
        }],
      }] as BusinessWorkspaceRecord["milestones"],
      invoices: [{
        id: "invoice-1",
        kind: "issued",
        amountCents: 100_000,
        artifacts: [{ assetId: "invoice-asset-1" }],
      }] as BusinessWorkspaceRecord["invoices"],
      financialSummary: {
        quotedCents: 100_000,
        contractCents: 100_000,
        scheduledCents: 100_000,
        requestedCents: 100_000,
        receivedCents: 100_000,
        outstandingCents: 0,
      },
      lifecycleStage: "paid",
      revision: 12,
    });

    expect(archiveWorkspaceBlockReason(makeWorkspace())).toContain("生效合同");
    expect(archiveWorkspaceBlockReason(readyForSnapshot)).toBe("请先生成归档完整性快照");

    const staleSnapshot = {
      id: "snapshot-stale",
      capturedWorkspaceRevision: 9,
      capturedCustomerRevision: readyForSnapshot.customer.revision,
      manifestSha256: "stale",
      manifestAssetId: null,
      packageAssetId: null,
      entries: [],
      generatedBy: "tester",
      generatedAt: 1,
    };
    expect(
      archiveWorkspaceBlockReason({
        ...readyForSnapshot,
        archiveIntegrityStatus: "ready",
        archiveSnapshots: [staleSnapshot],
      }),
    ).toContain("版本不一致");

    expect(
      archiveWorkspaceBlockReason({
        ...readyForSnapshot,
        archiveIntegrityStatus: "ready",
        archiveSnapshots: [{
          ...staleSnapshot,
          id: "snapshot-fresh",
          capturedWorkspaceRevision: readyForSnapshot.revision - 1,
          generatedAt: 2,
        }],
      }),
    ).toBeNull();
  });

  it("allows adopting a newer confirmed requirement only before formal documents exist", () => {
    const latest = {
      id: "brief-2",
      revision: 4,
    } as RequirementBriefRecord;
    const workspace = makeWorkspace({
      requirementBriefId: "brief-1",
      requirementBriefRevision: 3,
    });

    expect(requirementAdoptionBlockReason(workspace, latest)).toBeNull();
    expect(
      requirementAdoptionBlockReason(
        {
          ...workspace,
          documents: [
            {
              id: "contract-1",
              documentNumber: "C-001",
              status: "approved",
            } as BusinessDocumentRecord,
          ],
        },
        latest,
      ),
    ).toContain("C-001");
  });
});

describe("workspace prefill helpers", () => {
  it("labels every prefill field in Chinese", () => {
    for (const label of Object.values(PREFILL_FIELD_LABELS)) {
      expect(label.length).toBeGreaterThan(0);
    }
    expect(PREFILL_FIELD_LABELS.supplierBankAccount).toBe("银行账号");
  });

  it("formats tax rate basis points as a percentage and blanks as a dash", () => {
    expect(formatPrefillValue("defaultTaxRateBps", "600")).toBe("6%");
    expect(formatPrefillValue("defaultTaxRateBps", "1350")).toBe("13.50%");
    expect(formatPrefillValue("defaultTaxRateBps", "")).toBe("—");
    expect(formatPrefillValue("customerLegalName", "华邦精密制造有限公司")).toBe(
      "华邦精密制造有限公司",
    );
  });

  it("summarizes filled versus kept prefill decisions", () => {
    const changes: BusinessWorkspacePrefillChange[] = [
      {
        field: "customerLegalName",
        targetValue: "",
        sourceValue: "甲",
        resultValue: "甲",
        decision: "filled",
      },
      {
        field: "currency",
        targetValue: "CNY",
        sourceValue: "CNY",
        resultValue: "CNY",
        decision: "unchanged",
      },
      {
        field: "supplierBankName",
        targetValue: "",
        sourceValue: "",
        resultValue: "",
        decision: "unchanged",
      },
    ];
    expect(summarizePrefillChanges(changes)).toEqual({ filled: 1, kept: 2 });
  });
});

describe("quote templates", () => {
  it("provides four starter skeletons with named items", () => {
    expect(QUOTE_TEMPLATES.map((template) => template.id)).toEqual([
      "single-video",
      "itemized",
      "annual-frame",
      "monthly-service",
    ]);
    for (const template of QUOTE_TEMPLATES) {
      expect(template.items.length).toBeGreaterThan(2);
      for (const item of template.items) {
        expect(item.name.length).toBeGreaterThan(0);
        expect(item.quantityMillis).toBeGreaterThan(0);
      }
    }
  });

  it("replaces blank rows and keeps filled rows when applying a skeleton", () => {
    const blank = { id: null, name: "", description: "", quantityMillis: 1000, unit: "项", unitPriceCents: 0, taxRateBps: 600 };
    const filled = { ...blank, name: "已填写的服务", unitPriceCents: 5000 };
    const next = applyQuoteTemplateItems([blank, filled], QUOTE_TEMPLATES[0].items, 600);
    expect(next[0]).toEqual(filled);
    expect(next).toHaveLength(1 + QUOTE_TEMPLATES[0].items.length);
    expect(next[1].name).toBe(QUOTE_TEMPLATES[0].items[0].name);
    expect(next[1].unitPriceCents).toBe(0);
    expect(next[1].taxRateBps).toBe(600);
    expect(next[1].id).toBeNull();
  });

  it("copies history line items with their prices but without ids", () => {
    const history = [
      { id: "line-9", name: "品牌视频制作", description: "含拍摄", quantityMillis: 2000, unit: "条", unitPriceCents: 3_000_000, taxRateBps: 600, amountCents: 6_360_000 },
    ];
    const next = applyHistoryLineItems([], history);
    expect(next).toHaveLength(1);
    expect(next[0]).toEqual({
      id: null,
      name: "品牌视频制作",
      description: "含拍摄",
      quantityMillis: 2000,
      unit: "条",
      unitPriceCents: 3_000_000,
      taxRateBps: 600,
    });
  });
});
