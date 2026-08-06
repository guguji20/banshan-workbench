import { describe, expect, it, vi } from "vitest";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import {
  buildAnnualSettlementBatches,
  buildDefaultAcceptanceBatchPayload,
  buildAnnualSettlementWorkspace,
  buildBusinessBrainTurnContext,
  buildQuotationProfileInput,
  buildUpsertAnnualSettlementPayload,
  buildVoidAnnualSettlementPayload,
  advanceBusinessQuotationApproval,
  businessDocumentOutputFormat,
  generateBusinessQuotationXlsx,
  saveBusinessQuotationProfile,
  toBusinessChatMessage,
  upsertAnnualSettlementBatch,
  voidAnnualSettlementBatch,
  type AnnualSettlementClient,
  type QuotationWorkflowClient,
} from "./BusinessV1App";
import type { AnnualSettlementBatch, AnnualSettlementBatchInput } from "./ui/AnnualSettlementCenter";
import type { QuotationCenterInput } from "./ui/QuotationCenter";

function workspaceFixture(): BusinessWorkspaceRecord {
  return {
    id: "workspace-annual",
    projectId: "project-annual",
    customer: { displayName: "示例客户" },
    profile: {
      projectTitle: "2026 品牌服务年框",
      projectCode: "AF-2026-001",
      customerName: "示例客户",
      lineItems: [{ id: "deliverable-video", name: "品牌短片", quantityMillis: 4_000, unit: "条" }],
    },
    documents: [{ id: "contract-1", documentNumber: "HT-2026-001", status: "approved" }],
    currentDocuments: { contractDocumentId: "contract-1" },
    milestones: [{
      id: "milestone-q1",
      title: "第一季度",
      status: "accepted",
      deliverables: [{ id: "deliverable-video", milestoneId: "milestone-q1", name: "品牌短片", versions: [] }],
    }],
    settlementBatches: [{
      id: "batch-q1",
      workspaceId: "workspace-annual",
      contractNumber: "HT-2026-001",
      settlementPeriod: "2026 年第一季度",
      cadence: "quarterly",
      status: "confirmed",
      lines: [{
        deliverableId: "deliverable-video",
        milestoneId: "milestone-q1",
        deliverableName: "品牌短片",
        contractQuantityMillis: 4_000,
        cumulativeExecutedMillis: 4_000,
        currentExecutedMillis: 4_000,
        cumulativeAcceptedMillis: 4_000,
        currentAcceptedMillis: 4_000,
        cumulativeSettledMillis: 1_500,
        currentSettlementMillis: 1_500,
        remainingQuantityMillis: 2_500,
        unit: "条",
        notes: "首批",
      }],
      notes: "第一季度结算",
      revision: 2,
      createdAt: 100,
      updatedAt: 200,
      voidedAt: null,
      voidedBy: null,
      voidReason: "",
    }],
    revision: 12,
  } as unknown as BusinessWorkspaceRecord;
}

const NEW_BATCH_INPUT: AnnualSettlementBatchInput = {
  id: null,
  workspaceId: "workspace-annual",
  period: " 2026 年第二季度 ",
  cadence: "quarterly",
  lines: [{
    deliverableId: "deliverable-video",
    deliverableName: "品牌短片",
    milestoneTitle: "第一季度",
    unit: "条",
    quantity: 1.25,
  }],
  note: " 二季度结算 ",
};

const BATCH_VIEW: AnnualSettlementBatch = {
  id: "batch-q1",
  workspaceId: "workspace-annual",
  period: "2026 年第一季度",
  cadence: "quarterly",
  status: "confirmed",
  lines: [],
  note: "",
  createdAt: 100,
  updatedAt: 200,
};

describe("BusinessV1App brain turn integration", () => {
  it("preserves persisted web source dates when mapping historical assistant messages", () => {
    const sources = [{
      id: "web-source:https://example.com/report",
      url: "https://example.com/report",
      title: "历史报告",
      domain: "example.com",
      accessedAt: Date.UTC(2026, 6, 29, 3, 0),
      accessedDate: "2026-07-29",
      verificationLabel: "外部未确认" as const,
    }];

    const message = toBusinessChatMessage({
      id: "turn-1:assistant",
      role: "assistant",
      text: "历史联网结果",
      status: "complete",
      createdAt: Date.UTC(2026, 6, 29, 3, 0),
      sources,
    });

    expect(message.sources).toBe(sources);
    expect(message.sources?.[0].accessedDate).toBe("2026-07-29");
  });

  it("fails closed unless web access is explicitly enabled", () => {
    expect(buildBusinessBrainTurnContext("local-only", null, [])).toMatchObject({
      accessMode: "requestApproval",
      webEnabled: false,
    });
    expect(buildBusinessBrainTurnContext("web-enabled", "workspace-token", ["asset-1"])).toEqual({
      workspaceToken: "workspace-token",
      accessMode: "requestApproval",
      webEnabled: true,
      attachmentAssetIds: ["asset-1"],
    });
    expect(buildBusinessBrainTurnContext(undefined, null, [])).toMatchObject({ webEnabled: false });
  });
});

describe("BusinessV1App annual settlement integration", () => {
  it("maps workspace milestones and formal settlement batches into the center view model", () => {
    const workspace = workspaceFixture();

    expect(buildAnnualSettlementWorkspace(workspace)).toMatchObject({
      id: "workspace-annual",
      projectTitle: "2026 品牌服务年框",
      projectCode: "AF-2026-001",
      deliverables: [{
        id: "deliverable-video",
        milestoneTitle: "第一季度",
        contractQuantity: 4,
        executedQuantity: 4,
        acceptedQuantity: 4,
        settledQuantity: 1.5,
        unit: "条",
      }],
    });
    expect(buildAnnualSettlementBatches(workspace)[0]).toMatchObject({
      id: "batch-q1",
      period: "2026 年第一季度",
      cadence: "quarterly",
      status: "confirmed",
      lines: [{ deliverableId: "deliverable-video", milestoneTitle: "第一季度", quantity: 1.5 }],
    });
  });

  it("adapts center quantities and contract context to the generated upsert protocol", () => {
    const payload = buildUpsertAnnualSettlementPayload(workspaceFixture(), NEW_BATCH_INPUT);

    expect(payload).toEqual({
      workspaceId: "workspace-annual",
      batch: {
        id: null,
        contractNumber: "HT-2026-001",
        settlementPeriod: "2026 年第二季度",
        cadence: "quarterly",
        status: "draft",
        lines: [{
          deliverableId: "deliverable-video",
          contractQuantityMillis: 4_000,
          cumulativeExecutedMillis: 4_000,
          currentExecutedMillis: 4_000,
          cumulativeAcceptedMillis: 4_000,
          currentAcceptedMillis: 4_000,
          currentSettlementMillis: 1_250,
          unit: "条",
          notes: "",
        }],
        notes: "二季度结算",
      },
    });
  });

  it("calls the existing client methods with workspace revision and a trimmed void reason", async () => {
    const workspace = workspaceFixture();
    const upsert = vi.fn().mockResolvedValue({});
    const voidBatch = vi.fn().mockResolvedValue({});
    const settlementClient = {
      upsertBusinessSettlementBatch: upsert,
      voidBusinessSettlementBatch: voidBatch,
    } as unknown as AnnualSettlementClient;

    await upsertAnnualSettlementBatch(settlementClient, workspace, NEW_BATCH_INPUT);
    await voidAnnualSettlementBatch(settlementClient, workspace, BATCH_VIEW, " 录入错误 ");

    expect(upsert).toHaveBeenCalledWith(buildUpsertAnnualSettlementPayload(workspace, NEW_BATCH_INPUT), 12);
    expect(voidBatch).toHaveBeenCalledWith({
      workspaceId: "workspace-annual",
      batchId: "batch-q1",
      reason: "录入错误",
    }, 12);
    expect(buildVoidAnnualSettlementPayload(workspace, BATCH_VIEW, " 作废 ").reason).toBe("作废");
  });
});

describe("BusinessV1App acceptance command chain", () => {
  it("builds a six-category batch with four video groups and five editable outputs", () => {
    let sequence = 0;
    const payload = buildDefaultAcceptanceBatchPayload(
      "workspace-baietan",
      " 白鹅潭本次验收 ",
      (prefix) => `${prefix}-${++sequence}`,
    );

    expect(payload.workspaceId).toBe("workspace-baietan");
    expect(payload.label).toBe("白鹅潭本次验收");
    expect(payload.requirements).toHaveLength(6);
    expect(payload.requirements[0]).toMatchObject({
      label: "视频成片",
      kind: "video",
      requiredGroupCount: 4,
    });
    expect(payload.outputSpecs).toHaveLength(5);
    expect(payload.outputSpecs.map((output) => output.format)).toEqual([
      "xlsx",
      "docx",
      "docx",
      "docx",
      "docx",
    ]);
    expect(payload.outputSpecs.every((output) =>
      output.requirementIds.length === 6
      && output.templateKey === "builtin.acceptance.standard.v1"
    )).toBe(true);
  });
});

describe("BusinessV1App document generation integration", () => {
  it("uses the acceptance output spec format instead of forcing DOCX", () => {
    const workspace = workspaceFixture();
    workspace.acceptanceBatches = [{
      id: "acceptance-batch-1",
      outputSpecs: [{ id: "settlement-xlsx", format: "xlsx" }],
    }] as BusinessWorkspaceRecord["acceptanceBatches"];
    const document = {
      id: "acceptance-settlement-1",
      kind: "acceptance",
      snapshot: {
        acceptanceBatchId: "acceptance-batch-1",
        acceptanceOutputSpecId: "settlement-xlsx",
      },
    } as BusinessWorkspaceRecord["documents"][number];

    expect(businessDocumentOutputFormat(workspace, document)).toBe("xlsx");
  });

  it("fails closed when a linked acceptance output spec is missing", () => {
    const workspace = workspaceFixture();
    workspace.acceptanceBatches = [];
    const document = {
      id: "acceptance-missing-spec",
      kind: "acceptance",
      snapshot: {
        acceptanceBatchId: "acceptance-batch-1",
        acceptanceOutputSpecId: "missing-output",
      },
    } as BusinessWorkspaceRecord["documents"][number];

    expect(() => businessDocumentOutputFormat(workspace, document)).toThrow(
      "验收文件缺少对应的输出规格",
    );
  });

  it("keeps quote XLSX and ordinary documents DOCX", () => {
    const workspace = workspaceFixture();
    const quote = { kind: "quote" } as BusinessWorkspaceRecord["documents"][number];
    const contract = { kind: "contract" } as BusinessWorkspaceRecord["documents"][number];

    expect(businessDocumentOutputFormat(workspace, quote)).toBe("xlsx");
    expect(businessDocumentOutputFormat(workspace, contract)).toBe("docx");
  });
});

describe("BusinessV1App quotation command chain", () => {
  const QUOTATION_INPUT: QuotationCenterInput = {
    lineItems: [{
      id: "deliverable-video",
      name: " 系列视频制作 ",
      description: " 四条品牌视频 ",
      quantityMillis: 4_000,
      unit: " 条 ",
      unitPriceCents: 2_120_000,
      taxRateBps: 0,
    }],
    projectDiscountCents: 490_000,
    defaultTaxRateBps: 0,
    taxMode: "taxInclusive",
  };

  function quotationWorkspace(): BusinessWorkspaceRecord {
    const workspace = workspaceFixture();
    workspace.id = "workspace-baietan";
    workspace.projectId = "project-baietan";
    workspace.revision = 12;
    workspace.profile = {
      ...workspace.profile,
      projectTitle: "白鹅潭瑞玺系列视频",
      projectCode: "BET-2026-001",
      projectDiscountCents: 0,
      defaultTaxRateBps: 600,
      taxMode: "taxExclusive",
      quotationTotals: null,
      lineItems: [{
        id: "deliverable-video",
        name: "旧服务",
        description: "旧说明",
        quantityMillis: 1_000,
        unit: "项",
        unitPriceCents: 100,
        taxRateBps: 600,
        amountCents: 100,
      }],
    } as BusinessWorkspaceRecord["profile"];
    workspace.documents = [];
    workspace.currentDocuments = { ...workspace.currentDocuments, quoteDocumentId: null };
    return workspace;
  }

  it("saves editable pricing through the existing profile command without client totals", async () => {
    const workspace = quotationWorkspace();
    const updated = { ...workspace, revision: 13 } as BusinessWorkspaceRecord;
    const updateBusinessProfile = vi.fn().mockResolvedValue({ businessWorkspace: updated });
    const quotationClient = { updateBusinessProfile } as unknown as QuotationWorkflowClient;

    expect(buildQuotationProfileInput(workspace, QUOTATION_INPUT)).toMatchObject({
      projectDiscountCents: 490_000,
      defaultTaxRateBps: 0,
      taxMode: "taxInclusive",
      lineItems: [{
        id: "deliverable-video",
        name: "系列视频制作",
        description: "四条品牌视频",
        quantityMillis: 4_000,
        unit: "条",
        unitPriceCents: 2_120_000,
      }],
    });

    await expect(saveBusinessQuotationProfile(quotationClient, workspace, QUOTATION_INPUT)).resolves.toBe(updated);
    expect(updateBusinessProfile).toHaveBeenCalledWith({
      workspaceId: "workspace-baietan",
      profile: expect.not.objectContaining({ quotationTotals: expect.anything() }),
    }, 12);
  });

  it("creates a quote and uses each returned revision for draft to review to approval", async () => {
    const workspace = quotationWorkspace();
    const draftQuote = {
      id: "quote-v1",
      kind: "quote",
      sequenceNumber: 1,
      status: "draft",
    } as BusinessWorkspaceRecord["documents"][number];
    const createdWorkspace = {
      ...workspace,
      revision: 13,
      documents: [draftQuote],
      currentDocuments: { ...workspace.currentDocuments, quoteDocumentId: draftQuote.id },
    } as BusinessWorkspaceRecord;
    const reviewWorkspace = {
      ...createdWorkspace,
      revision: 14,
      documents: [{ ...draftQuote, status: "inReview" }],
    } as BusinessWorkspaceRecord;
    const approvedWorkspace = {
      ...reviewWorkspace,
      revision: 15,
      documents: [{ ...draftQuote, status: "approved" }],
    } as BusinessWorkspaceRecord;
    const createBusinessDocument = vi.fn().mockResolvedValue({ businessWorkspace: createdWorkspace });
    const changeBusinessDocumentStatus = vi.fn()
      .mockResolvedValueOnce({ businessWorkspace: reviewWorkspace })
      .mockResolvedValueOnce({ businessWorkspace: approvedWorkspace });
    const quotationClient = { createBusinessDocument, changeBusinessDocumentStatus } as unknown as QuotationWorkflowClient;

    await expect(advanceBusinessQuotationApproval(quotationClient, workspace, null)).resolves.toBe(reviewWorkspace);
    expect(createBusinessDocument).toHaveBeenCalledWith(expect.objectContaining({
      workspaceId: "workspace-baietan",
      kind: "quote",
      templateKey: "builtin.business.quote.v1",
    }), 12);
    expect(changeBusinessDocumentStatus).toHaveBeenNthCalledWith(1, expect.objectContaining({
      documentId: "quote-v1",
      status: "inReview",
    }), 13);

    await expect(advanceBusinessQuotationApproval(quotationClient, reviewWorkspace, "quote-v1")).resolves.toBe(approvedWorkspace);
    expect(changeBusinessDocumentStatus).toHaveBeenNthCalledWith(2, expect.objectContaining({
      documentId: "quote-v1",
      status: "approved",
    }), 14);
  });

  it("blocks formal XLSX before approval and generates with the current revision after approval", async () => {
    const workspace = quotationWorkspace();
    const quote = { id: "quote-v1", kind: "quote", status: "inReview" } as BusinessWorkspaceRecord["documents"][number];
    workspace.documents = [quote];
    const generateBusinessDocument = vi.fn().mockResolvedValue({ businessWorkspace: { ...workspace, revision: 13 } });
    const quotationClient = { generateBusinessDocument } as unknown as QuotationWorkflowClient;

    await expect(generateBusinessQuotationXlsx(quotationClient, workspace, quote.id)).rejects.toThrow("尚未完成人工确认");
    expect(generateBusinessDocument).not.toHaveBeenCalled();

    quote.status = "approved";
    await generateBusinessQuotationXlsx(quotationClient, workspace, quote.id);
    expect(generateBusinessDocument).toHaveBeenCalledWith({
      workspaceId: "workspace-baietan",
      documentId: "quote-v1",
      format: "xlsx",
    }, 12);
  });
});
