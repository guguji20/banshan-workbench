import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { BusinessWorkspaceRecord } from "../../generated/bsaigc/BusinessWorkspaceRecord";
import {
  QuotationCenter,
  currentQuotationDocument,
  quotationCenterInput,
  validateQuotationCenterInput,
} from "./QuotationCenter";

function workspaceFixture(status: BusinessWorkspaceRecord["documents"][number]["status"] = "inReview"): BusinessWorkspaceRecord {
  return {
    id: "workspace-baietan",
    projectId: "project-baietan",
    revision: 18,
    profile: {
      projectTitle: "白鹅潭瑞玺系列视频",
      projectCode: "BET-2026-001",
      projectDiscountCents: 490_000,
      defaultTaxRateBps: 0,
      taxMode: "taxInclusive",
      lineItems: [{
        id: "video-series",
        name: "系列视频制作",
        description: "四条品牌视频",
        quantityMillis: 4_000,
        unit: "条",
        unitPriceCents: 2_120_000,
        taxRateBps: 0,
        amountCents: 8_480_000,
      }],
    },
    currentDocuments: { quoteDocumentId: "quote-v2" },
    documents: [{
      id: "quote-v2",
      kind: "quote",
      sequenceNumber: 2,
      status,
      outputAssetId: status === "generated" ? "asset-quote-v2" : null,
      outputFormat: status === "generated" ? "xlsx" : null,
      updatedAt: 200,
    }],
  } as BusinessWorkspaceRecord;
}

describe("quotation center rules", () => {
  it("preserves the approved unit price and Baietan totals in the editable input", () => {
    const input = quotationCenterInput(workspaceFixture());
    expect(input.lineItems[0]).toMatchObject({ quantityMillis: 4_000, unitPriceCents: 2_120_000 });
    expect(input.projectDiscountCents).toBe(490_000);
    expect(validateQuotationCenterInput(input)).toEqual([]);
  });

  it("fails closed for invalid quantities, discounts, and tax rates", () => {
    const input = quotationCenterInput(workspaceFixture());
    input.lineItems[0].quantityMillis = 0;
    input.lineItems[0].taxRateBps = 10_001;
    input.projectDiscountCents = -1;
    expect(validateQuotationCenterInput(input)).toEqual([
      "“系列视频制作”数量必须大于 0",
      "“系列视频制作”税率必须在 0% 到 100% 之间",
      "项目优惠不能为负数",
    ]);
  });

  it("normalizes legacy profiles that predate discount and tax mode fields", () => {
    const workspace = workspaceFixture();
    const legacyProfile = workspace.profile as Partial<BusinessWorkspaceRecord["profile"]>;
    delete legacyProfile.projectDiscountCents;
    delete legacyProfile.taxMode;

    const input = quotationCenterInput(workspace);

    expect(input.projectDiscountCents).toBe(0);
    expect(input.taxMode).toBe("taxInclusive");
    expect(validateQuotationCenterInput(input)).toEqual([]);
  });

  it("selects the current quote and ignores a voided current pointer", () => {
    const workspace = workspaceFixture();
    workspace.documents.push({ id: "quote-v1", kind: "quote", sequenceNumber: 1, status: "approved", updatedAt: 100 } as BusinessWorkspaceRecord["documents"][number]);
    expect(currentQuotationDocument(workspace)?.id).toBe("quote-v2");
    workspace.documents[0].status = "voided";
    expect(currentQuotationDocument(workspace)?.id).toBe("quote-v1");
  });
});

describe("QuotationCenter", () => {
  it("renders editable pricing, version, status, exact totals, and blocks XLSX generation before approval", () => {
    const html = renderToStaticMarkup(<QuotationCenter
      workspace={workspaceFixture("inReview")}
      onSave={vi.fn()}
      onAdvanceApproval={vi.fn()}
      onGenerate={vi.fn()}
      onOpenAsset={vi.fn()}
      onClose={vi.fn()}
    />);

    expect(html).toContain("报价中心");
    expect(html).toContain("V2");
    expect(html).toContain("待人工确认");
    expect(html).toContain("¥84,800.00");
    expect(html).toContain("¥4,900.00");
    expect(html).toContain("¥79,900.00");
    expect(html).toContain("确认报价");
    expect(html).toMatch(/生成 XLSX<\/button>/);
    expect(html).toMatch(/disabled=""[^>]*>生成 XLSX/);
  });

  it("enables generation only after human approval and exposes the generated asset", () => {
    const approved = renderToStaticMarkup(<QuotationCenter
      workspace={workspaceFixture("approved")}
      onSave={vi.fn()}
      onAdvanceApproval={vi.fn()}
      onGenerate={vi.fn()}
      onOpenAsset={vi.fn()}
      onClose={vi.fn()}
    />);
    expect(approved).toContain("已人工确认");
    expect(approved).not.toMatch(/disabled=""[^>]*>生成 XLSX/);

    const generated = renderToStaticMarkup(<QuotationCenter
      workspace={workspaceFixture("generated")}
      onSave={vi.fn()}
      onAdvanceApproval={vi.fn()}
      onGenerate={vi.fn()}
      onOpenAsset={vi.fn()}
      onClose={vi.fn()}
    />);
    expect(generated).toContain("已生成");
    expect(generated).toContain("打开成果");
  });
});
