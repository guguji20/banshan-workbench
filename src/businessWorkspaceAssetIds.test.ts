import { describe, expect, it } from "vitest";
import type { BusinessWorkspaceRecord } from "./generated/bsaigc/BusinessWorkspaceRecord";
import { businessWorkspaceAssetIds } from "./businessWorkspaceAssetIds";

describe("businessWorkspaceAssetIds", () => {
  it("collects every document, delivery, signoff, quote confirmation, receipt, invoice, and archive asset once", () => {
    const workspace = {
      documents: [{ outputAssetId: "document", reportAssetId: "report" }],
      milestones: [{
        deliverables: [{ versions: [{ artifact: { assetId: "delivery" } }] }],
      }],
      deliverySubmissions: [{
        signoffs: [{ evidence: { assetId: "signoff" } }, { evidence: null }],
      }],
      quoteConfirmations: [{
        quoteAssetId: "quote",
        evidence: { assetId: "confirmation" },
      }],
      receipts: [
        { evidence: { assetId: "receipt" } },
        { evidence: null },
      ],
      invoices: [{
        artifacts: [{ assetId: "invoice" }, { assetId: "document" }],
      }],
      archiveSnapshots: [{
        manifestAssetId: "manifest",
        packageAssetId: "package",
        entries: [{ artifact: { assetId: "delivery" } }],
      }],
    } as unknown as BusinessWorkspaceRecord;

    expect(businessWorkspaceAssetIds(workspace)).toEqual([
      "confirmation",
      "delivery",
      "document",
      "invoice",
      "manifest",
      "package",
      "quote",
      "receipt",
      "report",
      "signoff",
    ]);
    expect(businessWorkspaceAssetIds(null)).toEqual([]);
  });
});
