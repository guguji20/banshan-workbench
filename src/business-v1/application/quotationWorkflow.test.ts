import { describe, expect, test } from "vitest";
import { createQuotationDraft } from "./quotationWorkflow";

describe("business v1 quotation workflow", () => {
  test("builds the Baietan quote without changing its approved unit price", () => {
    const result = createQuotationDraft({
      id: "quote-baietan-1",
      companyId: "company-huabang",
      customerProjectId: "project-baietan",
      title: "白鹅潭瑞玺系列视频报价",
      lines: [{
        id: "video-series",
        description: "系列视频制作",
        quantity: 4,
        unitPriceCents: 2_120_000,
      }],
      projectDiscountCents: 490_000,
      taxBasisPoints: 0,
      taxMode: "taxInclusive",
      actorId: "operator-1",
      sourceKind: "historicalQuotation",
      sourceId: "baietan-baseline",
      sourceLabel: "白鹅潭基准案例",
      createdAt: "2026-07-28T16:00:00.000Z",
    });

    expect(result.quotation.items[0].unitPrice.value.cents).toBe(2_120_000);
    expect(result.totals.items[0].lineTotal.cents).toBe(8_480_000);
    expect(result.totals.discountTotal.cents).toBe(490_000);
    expect(result.totals.finalTotal.cents).toBe(7_990_000);
    expect(result.summary).toEqual({
      subtotal: "¥84,800.00",
      projectDiscount: "¥4,900.00",
      tax: "¥0.00",
      finalTotal: "¥79,900.00",
    });
  });
});
