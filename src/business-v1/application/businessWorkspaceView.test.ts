import { describe, expect, it } from "vitest";
import type { BusinessWorkspaceRecord } from "../../generated/bsaigc/BusinessWorkspaceRecord";
import { buildBusinessWorkspaceContext, buildBusinessWorkspaceTaskMessages } from "./businessWorkspaceView";

describe("business workspace view", () => {
  it("surfaces durable quote output, approval, and versions", () => {
    const workspace = fixture();
    const context = buildBusinessWorkspaceContext(workspace, true);
    const messages = buildBusinessWorkspaceTaskMessages(workspace);

    expect(context.missingMaterials).toEqual([]);
    expect(context.previews).toEqual([expect.objectContaining({ id: "asset-quote", format: "xlsx" })]);
    expect(context.approvals).toEqual([]);
    expect(context.versions[0]).toEqual(expect.objectContaining({ label: "报价 #1", isCurrent: true }));
    expect(messages[0].task).toEqual(expect.objectContaining({ status: "completed", progress: 100 }));
    expect(messages[0].task?.outputs?.[0]).toEqual(expect.objectContaining({ id: "asset-quote", status: "ready" }));
  });

  it("keeps old workspaces with empty acceptance batches unchanged", () => {
    const workspace = fixture();
    workspace.documents[0].status = "inReview";

    const context = buildBusinessWorkspaceContext(workspace, true);

    expect(workspace.acceptanceBatches).toEqual([]);
    expect(context.missingMaterials).toEqual([]);
    expect(context.approvals[0]).toEqual(expect.objectContaining({
      id: "quote-1",
      blocked: undefined,
    }));
  });

  it("blocks incomplete profiles and exposes the approval stage", () => {
    const workspace = fixture();
    workspace.profile.lineItems = [];
    workspace.profile.customerLegalName = "";
    workspace.documents[0].status = "inReview";
    workspace.documents[0].outputAssetId = null;
    workspace.documents[0].outputFormat = null;

    const context = buildBusinessWorkspaceContext(workspace, true);
    const messages = buildBusinessWorkspaceTaskMessages(workspace);

    expect(context.missingMaterials.map((item) => item.id)).toEqual([
      "quotation-line-items",
      "customer-legal-name",
    ]);
    expect(context.approvals[0]).toEqual(expect.objectContaining({ id: "quote-1", status: "pending" }));
    expect(messages[0].task).toEqual(expect.objectContaining({
      status: "waiting-confirmation",
      requiresConfirmation: true,
    }));
  });

  it("projects acceptance readiness 4/3/1 into missing materials and approval blocking", () => {
    const workspace = acceptanceFixture();

    const context = buildBusinessWorkspaceContext(workspace, true);
    const messages = buildBusinessWorkspaceTaskMessages(workspace);

    expect(context.missingMaterials).toContainEqual({
      id: "acceptance:acceptance-batch-1:video-series",
      title: "白鹅潭验收 · 系列视频未齐",
      detail: "要求4组，当前3组，缺1组",
      severity: "blocking",
    });
    expect(context.approvals[0]).toEqual(expect.objectContaining({
      id: "acceptance-1",
      blocked: "验收材料未齐：要求4组，当前3组，缺1组",
    }));
    expect(messages[0].task?.confirmationBlockedReason).toBe(
      "验收材料未齐：要求4组，当前3组，缺1组",
    );
  });

  it("counts 0/5 prepared acceptance outputs while allowing draft preparation", () => {
    const context = buildBusinessWorkspaceContext(acceptanceFixture(), true);

    expect(context.acceptanceBatches[0]).toEqual(expect.objectContaining({
      preparedCount: 0,
      totalCount: 5,
      isReady: false,
      isPreparing: false,
      prepareDisabledReason: undefined,
    }));
  });

  it("counts 3/5 unique current-batch output specs", () => {
    const workspace = acceptanceFixture();
    addPreparedAcceptanceDocument(workspace, "acceptance-2", "acceptance-batch-1", "output-1");
    addPreparedAcceptanceDocument(workspace, "acceptance-3", "acceptance-batch-1", "output-2");
    addPreparedAcceptanceDocument(workspace, "acceptance-4", "acceptance-batch-1", "output-3");
    addPreparedAcceptanceDocument(workspace, "acceptance-duplicate", "acceptance-batch-1", "output-3");
    addPreparedAcceptanceDocument(workspace, "acceptance-other-batch", "acceptance-batch-2", "output-4");
    addPreparedAcceptanceDocument(workspace, "acceptance-invalid-spec", "acceptance-batch-1", "removed-output");

    const context = buildBusinessWorkspaceContext(workspace, true);

    expect(context.acceptanceBatches[0]).toEqual(expect.objectContaining({
      preparedCount: 3,
      totalCount: 5,
      prepareDisabledReason: undefined,
    }));
  });

  it("disables preparation after all 5 outputs exist", () => {
    const workspace = acceptanceFixture();
    for (let index = 1; index <= 5; index += 1) {
      addPreparedAcceptanceDocument(
        workspace,
        `acceptance-${index + 1}`,
        "acceptance-batch-1",
        `output-${index}`,
      );
    }

    const context = buildBusinessWorkspaceContext(workspace, true);

    expect(context.acceptanceBatches[0]).toEqual(expect.objectContaining({
      preparedCount: 5,
      totalCount: 5,
      prepareDisabledReason: "验收文件已准备",
    }));
  });

  it("allows an incomplete acceptance draft to enter review", () => {
    const workspace = acceptanceFixture();
    workspace.documents[0].status = "draft";

    const context = buildBusinessWorkspaceContext(workspace, true);
    const messages = buildBusinessWorkspaceTaskMessages(workspace);

    expect(context.missingMaterials).toHaveLength(1);
    expect(context.approvals[0]).toEqual(expect.objectContaining({
      id: "acceptance-1",
      blocked: undefined,
    }));
    expect(messages[0].task?.confirmationBlockedReason).toBeUndefined();
  });

  it("blocks both in-review approval and approved generation while materials are incomplete", () => {
    const workspace = acceptanceFixture();
    workspace.documents[0].status = "approved";

    const context = buildBusinessWorkspaceContext(workspace, true);
    const messages = buildBusinessWorkspaceTaskMessages(workspace);

    expect(context.approvals[0].blocked).toContain("验收材料未齐");
    expect(messages[0].task?.confirmationBlockedReason).toContain("验收材料未齐");
  });

  it("unblocks the acceptance approval after readiness is complete", () => {
    const workspace = acceptanceFixture();
    workspace.acceptanceBatches[0].readiness = { isReady: true, blockers: [] };

    const context = buildBusinessWorkspaceContext(workspace, true);
    const messages = buildBusinessWorkspaceTaskMessages(workspace);

    expect(context.missingMaterials).toEqual([]);
    expect(context.approvals[0]).toEqual(expect.objectContaining({
      id: "acceptance-1",
      blocked: undefined,
    }));
    expect(messages[0].task?.confirmationBlockedReason).toBeUndefined();
  });
});

function acceptanceFixture(): BusinessWorkspaceRecord {
  const workspace = fixture();
  workspace.documents[0] = {
    ...workspace.documents[0],
    id: "acceptance-1",
    kind: "acceptance",
    documentNumber: "YS-001",
    title: "白鹅潭验收文件",
    templateKey: "builtin.business.acceptance.v1",
    status: "inReview",
    snapshot: {
      ...workspace.documents[0].snapshot,
      acceptanceBatchId: "acceptance-batch-1",
    },
    outputAssetId: null,
    outputFormat: null,
  };
  workspace.acceptanceBatches = [{
    id: "acceptance-batch-1",
    workspaceId: workspace.id,
    label: "白鹅潭验收",
    requirements: [{
      id: "video-series",
      label: "系列视频",
      kind: "video",
      requiredGroupCount: 4,
    }],
    outputSpecs: Array.from({ length: 5 }, (_, index) => ({
      id: `output-${index + 1}`,
      outputCode: `acceptance-output-${index + 1}`,
      documentNumber: `YS-${index + 1}`,
      title: `验收文件 ${index + 1}`,
      templateKey: `builtin.acceptance.output-${index + 1}.v1`,
      templateAssetId: null,
      templateSourceSha256: null,
      templateMappingVersion: "",
      contractSettlement: null,
      serviceSettlementItems: [],
      paymentApplication: null,
      format: index === 3 ? "xlsx" : "docx",
      requirementIds: ["video-series"],
    })),
    materials: [],
    readiness: {
      isReady: false,
      blockers: [{
        code: "missingMaterialGroups",
        requirementId: "video-series",
        requirementLabel: "系列视频",
        requiredGroupCount: 4,
        providedGroupCount: 3,
        missingGroupCount: 1,
      }],
    },
    documentIds: ["acceptance-1"],
    status: "collecting",
    revision: 1,
    createdAt: 1,
    updatedAt: 2,
  }];
  workspace.currentDocuments.acceptanceDocumentId = "acceptance-1";
  return workspace;
}

function fixture(): BusinessWorkspaceRecord {
  return {
    id: "workspace-1",
    projectId: "project-1",
    customerId: "customer-1",
    customer: {
      id: "customer-1",
      displayName: "白鹅潭客户",
      legalName: "白鹅潭客户有限公司",
      taxId: "tax-customer",
      billingAddress: "广州",
      primaryContactName: "客户代表",
      primaryPhone: "13800000000",
      primaryEmail: "customer@example.com",
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
      projectTitle: "白鹅潭项目",
      projectCode: "BRT-001",
      customerName: "白鹅潭客户",
      customerLegalName: "白鹅潭客户有限公司",
      customerTaxId: "tax-customer",
      customerAddress: "广州",
      customerContact: "客户代表",
      customerPhone: "13800000000",
      customerEmail: "customer@example.com",
      supplierLegalName: "半山有限公司",
      supplierTaxId: "tax-supplier",
      supplierAddress: "广州",
      supplierContact: "商务",
      supplierPhone: "13900000000",
      supplierBankName: "测试银行",
      supplierBankAccount: "0000",
      currency: "CNY",
      defaultTaxRateBps: 600,
      taxMode: "taxInclusive",
      projectDiscountCents: 490_000,
      quotationTotals: {
        originalTotalCents: 8_480_000,
        projectDiscountCents: 490_000,
        taxExclusiveTotalCents: 7_537_736,
        taxCents: 452_264,
        finalTotalCents: 7_990_000,
      },
      serviceStartAt: null,
      serviceEndAt: null,
      deliverySummary: "四项服务",
      paymentTerms: "验收后付款",
      acceptanceTerms: "按合同验收",
      notes: "",
      lineItems: [{
        id: "line-1",
        name: "创意服务",
        description: "创意服务",
        quantityMillis: 4_000,
        unit: "项",
        unitPriceCents: 2_120_000,
        taxRateBps: 600,
        amountCents: 8_480_000,
      }],
    },
    documents: [{
      id: "quote-1",
      kind: "quote",
      sequenceNumber: 1,
      documentNumber: "BJ-001",
      title: "白鹅潭报价单",
      templateKey: "builtin.business.quote.v1",
      status: "generated",
      snapshot: {
        workspaceRevision: 1,
        acceptanceBatchId: null,
        acceptanceOutputSpecId: null,
        acceptanceBatchRevision: null,
        materialBindings: [],
        templateAssetId: null,
        templateSourceSha256: null,
        templateMappingVersion: "",
      contractSettlement: null,
      serviceSettlementItems: [],
      paymentApplication: null,
        customerId: "customer-1",
        customer: {
          id: "customer-1",
          displayName: "白鹅潭客户",
          legalName: "白鹅潭客户有限公司",
          taxId: "tax-customer",
          billingAddress: "广州",
          primaryContactName: "客户代表",
          primaryPhone: "13800000000",
          primaryEmail: "customer@example.com",
          notes: "",
          status: "active",
          revision: 1,
          createdAt: 1,
          updatedAt: 1,
          archivedAt: null,
          archivedBy: null,
        },
        profile: {} as BusinessWorkspaceRecord["profile"],
        payment: null,
      },
      outputAssetId: "asset-quote",
      outputFormat: "xlsx",
      sourceAssetId: null,
      reviewId: null,
      reportAssetId: null,
      evidence: null,
      manualWaiver: null,
      voidedAt: null,
      voidedBy: null,
      voidReason: "",
      approvedAt: 2,
      approvedBy: "operator",
      generatedAt: 3,
      revision: 4,
      createdAt: 1,
      updatedAt: 3,
    }],
    payments: [],
    quoteConfirmations: [],
    receipts: [],
    milestones: [],
    settlementBatches: [],
    acceptanceBatches: [],
    templateVersions: [],
    deliverySubmissions: [],
    invoices: [],
    archiveSnapshots: [],
    archiveIntegrityStatus: "notCaptured",
    status: "active",
    archivedAt: null,
    archivedBy: null,
    lifecycleStage: "quoted",
    financialSummary: {
      quotedCents: 8_480_000,
      contractCents: 0,
      scheduledCents: 0,
      requestedCents: 0,
      receivedCents: 0,
      outstandingCents: 0,
    },
    currentDocuments: {
      quoteDocumentId: "quote-1",
      contractDocumentId: null,
      paymentRequestDocumentId: null,
      acceptanceDocumentId: null,
    },
    revision: 4,
    createdAt: 1,
    updatedAt: 3,
  };
}

function addPreparedAcceptanceDocument(
  workspace: BusinessWorkspaceRecord,
  id: string,
  batchId: string,
  outputSpecId: string,
) {
  const source = workspace.documents[0];
  workspace.documents.push({
    ...source,
    id,
    sequenceNumber: workspace.documents.length + 1,
    documentNumber: id,
    snapshot: {
      ...source.snapshot,
      acceptanceBatchId: batchId,
      acceptanceOutputSpecId: outputSpecId,
    },
  });
}
