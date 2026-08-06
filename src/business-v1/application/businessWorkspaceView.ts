import type { BusinessDocumentRecord } from "../../generated/bsaigc/BusinessDocumentRecord";
import type { BusinessWorkspaceRecord } from "../../generated/bsaigc/BusinessWorkspaceRecord";
import { BUSINESS_ENGINE_NAME } from "../../brand";
import type {
  BusinessTaskKind,
  ChatMessage,
  DocumentVersion,
  WorkspaceContext,
  WorkspaceTask,
} from "../ui";

const DOCUMENT_LABELS: Record<BusinessDocumentRecord["kind"], string> = {
  quote: "报价",
  contract: "合同",
  paymentRequest: "请款",
  acceptance: "验收",
};

interface BusinessWorkspaceContextOptions {
  preparingAcceptanceBatchId?: string | null;
  isBusy?: boolean;
}

export function findBusinessWorkspace(
  workspaces: readonly BusinessWorkspaceRecord[],
  projectId: string | null,
): BusinessWorkspaceRecord | null {
  if (!projectId) return null;
  return workspaces.find((workspace) => workspace.projectId === projectId) ?? null;
}

export function buildBusinessWorkspaceContext(
  workspace: BusinessWorkspaceRecord | null,
  hasProject: boolean,
  options: BusinessWorkspaceContextOptions = {},
): WorkspaceContext {
  if (!hasProject) return emptyContext();
  if (!workspace) {
    return {
      ...emptyContext(),
      missingMaterials: [{
        id: "business-workspace",
        title: "项目业务资料库尚未初始化",
        detail: "启动报价、验收或结算任务后会自动建立本地业务资料库。",
        severity: "blocking",
      }],
    };
  }

  const missingMaterials: WorkspaceContext["missingMaterials"] = [];
  const legalRisks: WorkspaceContext["legalRisks"] = [];
  const profile = workspace.profile;
  if (!profile.lineItems.length) {
    missingMaterials.push({
      id: "quotation-line-items",
      title: "缺少报价服务项",
      detail: "至少需要服务名称、数量、单位、含税单价和税率。",
      severity: "blocking",
    });
  }
  if (!profile.customerLegalName.trim()) {
    missingMaterials.push({
      id: "customer-legal-name",
      title: "缺少甲方主体",
      detail: "正式报价和合同输出前必须确认客户法定主体。",
      severity: "warning",
    });
  }
  if (!profile.supplierLegalName.trim()) {
    missingMaterials.push({
      id: "supplier-legal-name",
      title: "缺少我方开票主体",
      detail: "正式输出前必须从公司资料库选择供应方主体。",
      severity: "warning",
    });
  }
  if (!profile.paymentTerms.trim()) {
    legalRisks.push({
      id: "payment-terms",
      title: "付款条款未确认",
      detail: "合同、请款和结算任务不能静默补写付款节点。",
      level: "medium",
      sourceLabel: "项目业务资料",
    });
  }
  if (!profile.acceptanceTerms.trim()) {
    legalRisks.push({
      id: "acceptance-terms",
      title: "验收口径未确认",
      detail: "验收输出前需要明确交付内容、证据和签收要求。",
      level: "medium",
      sourceLabel: "项目业务资料",
    });
  }

  const acceptanceBatches = workspace.acceptanceBatches ?? [];
  for (const batch of acceptanceBatches) {
    for (const blocker of batch.readiness.blockers) {
      missingMaterials.push({
        id: `acceptance:${batch.id}:${blocker.requirementId}`,
        title: `${batch.label} · ${blocker.requirementLabel}未齐`,
        detail: acceptanceBlockerDetail(blocker),
        severity: "blocking",
      });
    }
  }

  const documents = [...workspace.documents].sort(compareDocuments);
  const acceptanceBatchesById = new Map(acceptanceBatches.map((batch) => [batch.id, batch]));
  const acceptanceBatchSummaries = acceptanceBatches.map((batch) => {
    const validOutputSpecIds = new Set(batch.outputSpecs.map((spec) => spec.id));
    const preparedOutputSpecIds = new Set(
      documents
        .filter((document) => (
          document.kind === "acceptance"
          && document.snapshot.acceptanceBatchId === batch.id
          && document.snapshot.acceptanceOutputSpecId !== null
          && validOutputSpecIds.has(document.snapshot.acceptanceOutputSpecId)
        ))
        .map((document) => document.snapshot.acceptanceOutputSpecId as string),
    );
    const totalCount = validOutputSpecIds.size;
    const preparedCount = preparedOutputSpecIds.size;
    const isPreparing = options.preparingAcceptanceBatchId === batch.id;
    let prepareDisabledReason: string | undefined;
    if (totalCount === 0) prepareDisabledReason = "当前批次没有输出规格";
    else if (preparedCount === totalCount) prepareDisabledReason = "验收文件已准备";
    else if (isPreparing) prepareDisabledReason = "正在准备验收文件";
    else if (options.isBusy) prepareDisabledReason = "正在处理其他任务";

    return {
      id: batch.id,
      label: batch.label,
      status: batch.status,
      preparedCount,
      totalCount,
      isReady: batch.readiness.isReady,
      blockerText: batch.readiness.isReady
        ? undefined
        : batch.readiness.blockers.map(acceptanceBlockerDetail).join("；") || "验收材料未齐",
      isPreparing,
      prepareDisabledReason,
    };
  });
  const currentDocumentIds = currentDocumentIdsByKind(documents);
  return {
    acceptanceBatches: acceptanceBatchSummaries,
    missingMaterials,
    conflicts: [],
    templates: documents.map((document) => ({
      id: `template:${document.id}`,
      name: document.templateKey,
      versionLabel: `文档 #${document.sequenceNumber}`,
      sourceLabel: DOCUMENT_LABELS[document.kind],
      confidence: 1,
      isSelected: currentDocumentIds.has(document.id),
    })),
    legalRisks,
    previews: documents
      .filter((document) => document.outputAssetId && document.outputFormat)
      .map((document) => ({
        id: document.outputAssetId as string,
        name: document.title,
        format: document.outputFormat as "docx" | "xlsx",
        pageLabel: `${document.documentNumber} · 文档 #${document.sequenceNumber}`,
        status: "ready" as const,
      })),
    approvals: documents
      .filter((document) => ["draft", "inReview", "approved"].includes(document.status))
      .map((document) => ({
        id: document.id,
        title: approvalTitle(document),
        detail: approvalDetail(document),
        requestedBy: "本地业务引擎",
        requestedAt: formatTimestamp(document.updatedAt),
        status: "pending" as const,
        blocked: acceptanceApprovalBlockReason(document, acceptanceBatchesById),
      })),
    versions: documents.map((document): DocumentVersion => ({
      id: document.id,
      label: `${DOCUMENT_LABELS[document.kind]} #${document.sequenceNumber}`,
      authorName: document.approvedBy ?? "本地用户",
      createdAt: formatTimestamp(document.updatedAt),
      note: `${document.documentNumber} · ${documentStatusLabel(document.status)}`,
      isCurrent: currentDocumentIds.has(document.id),
    })),
  };
}

export function buildBusinessWorkspaceTaskMessages(
  workspace: BusinessWorkspaceRecord | null,
): ChatMessage[] {
  if (!workspace) return [];
  const acceptanceBatchesById = new Map(
    workspace.acceptanceBatches.map((batch) => [batch.id, batch]),
  );
  return [...workspace.documents]
    .sort(compareDocuments)
    .map((document) => ({
      id: `business-document:${document.id}`,
      role: "assistant" as const,
      authorName: BUSINESS_ENGINE_NAME,
      createdAt: formatTimestamp(document.updatedAt),
      content: `${DOCUMENT_LABELS[document.kind]}任务已写入本地业务账本。`,
      task: documentTask(document, acceptanceBatchesById),
    }));
}

function documentTask(
  document: BusinessDocumentRecord,
  acceptanceBatchesById: ReadonlyMap<
    string,
    BusinessWorkspaceRecord["acceptanceBatches"][number]
  >,
): WorkspaceTask {
  const status = documentTaskStatus(document);
  const output = document.outputAssetId && document.outputFormat ? [{
    id: document.outputAssetId,
    name: document.title,
    format: document.outputFormat,
    versionLabel: `文档 #${document.sequenceNumber}`,
    status: "ready" as const,
    detail: document.documentNumber,
  }] : [];
  return {
    id: document.id,
    kind: documentTaskKind(document.kind),
    title: document.title,
    status: status.status,
    stageLabel: status.stageLabel,
    progress: status.progress,
    detail: `${document.documentNumber} · ${documentStatusLabel(document.status)}`,
    startedAt: formatTimestamp(document.createdAt),
    requiresConfirmation: ["draft", "inReview", "approved"].includes(document.status),
    confirmationBlockedReason: acceptanceApprovalBlockReason(
      document,
      acceptanceBatchesById,
    ),
    outputs: output,
  };
}

function documentTaskStatus(document: BusinessDocumentRecord): Pick<WorkspaceTask, "status" | "stageLabel" | "progress"> {
  switch (document.status) {
    case "draft":
      return { status: "waiting-confirmation", stageLabel: "草稿待提交", progress: 30 };
    case "inReview":
      return { status: "waiting-confirmation", stageLabel: "内容待批准", progress: 55 };
    case "approved":
      return { status: "waiting-confirmation", stageLabel: "批准后待生成", progress: 80 };
    case "generated":
    case "effective":
      return { status: "completed", stageLabel: documentStatusLabel(document.status), progress: 100 };
    case "voided":
      return { status: "failed", stageLabel: "已作废", progress: 100 };
  }
}

function documentTaskKind(kind: BusinessDocumentRecord["kind"]): BusinessTaskKind {
  if (kind === "quote") return "quotation";
  if (kind === "acceptance") return "acceptance";
  if (kind === "paymentRequest") return "settlement";
  return "contract-review";
}

function approvalTitle(document: BusinessDocumentRecord): string {
  const label = DOCUMENT_LABELS[document.kind];
  if (document.status === "draft") return `提交${label}复核`;
  if (document.status === "inReview") return `批准${label}内容`;
  return `生成${label}可编辑文件`;
}

function approvalDetail(document: BusinessDocumentRecord): string {
  if (document.status === "draft") return "提交后进入人工复核，不会直接生成正式文件。";
  if (document.status === "inReview") return "批准动作会记录操作者、时间、文档版本和工作区 revision。";
  return document.kind === "quote"
    ? "将生成可编辑 XLSX 并导入本地 Vault。"
    : "将生成可编辑 DOCX 并导入本地 Vault。";
}

function acceptanceApprovalBlockReason(
  document: BusinessDocumentRecord,
  batchesById: ReadonlyMap<string, BusinessWorkspaceRecord["acceptanceBatches"][number]>,
): string | undefined {
  const batchId = document.snapshot.acceptanceBatchId;
  if (
    document.kind !== "acceptance"
    || document.status === "draft"
    || !batchId
  ) return undefined;

  const batch = batchesById.get(batchId);
  if (!batch) return "验收批次不存在，暂不能确认。";
  if (batch.readiness.isReady) return undefined;

  const blockers = batch.readiness.blockers.map(acceptanceBlockerDetail).join("；");
  return blockers ? `验收材料未齐：${blockers}` : "验收材料未齐，暂不能确认。";
}

function acceptanceBlockerDetail(blocker: {
  requiredGroupCount: number;
  providedGroupCount: number;
  missingGroupCount: number;
}): string {
  return `要求${blocker.requiredGroupCount}组，当前${blocker.providedGroupCount}组，缺${blocker.missingGroupCount}组`;
}

function documentStatusLabel(status: BusinessDocumentRecord["status"]): string {
  const labels: Record<BusinessDocumentRecord["status"], string> = {
    draft: "草稿",
    inReview: "复核中",
    approved: "已批准",
    generated: "已生成",
    effective: "已生效",
    voided: "已作废",
  };
  return labels[status];
}

function currentDocumentIdsByKind(documents: readonly BusinessDocumentRecord[]): Set<string> {
  const current = new Map<BusinessDocumentRecord["kind"], BusinessDocumentRecord>();
  for (const document of documents) {
    const candidate = current.get(document.kind);
    if (!candidate || compareDocuments(candidate, document) < 0) current.set(document.kind, document);
  }
  return new Set([...current.values()].map((document) => document.id));
}

function compareDocuments(left: BusinessDocumentRecord, right: BusinessDocumentRecord): number {
  return left.updatedAt - right.updatedAt || left.sequenceNumber - right.sequenceNumber || left.id.localeCompare(right.id);
}

function formatTimestamp(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(value);
}

function emptyContext(): WorkspaceContext {
  return {
    acceptanceBatches: [],
    missingMaterials: [],
    conflicts: [],
    templates: [],
    legalRisks: [],
    previews: [],
    approvals: [],
    versions: [],
  };
}
