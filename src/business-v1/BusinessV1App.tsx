import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { X } from "lucide-react";
import { BsaigcClient, DesktopHostAdapter, WebHostAdapter, isTauriRuntime } from "../client-sdk";
import { PRODUCT_AGENT_NAME, PRODUCT_NAME } from "../brand";
import { AuthGate } from "../components/AuthGate";
import { SettingsCenter } from "../components/SettingsCenter";
import { localizeAuthError } from "../components/authText";
import type { AssetSourceSelection } from "../generated/bsaigc/AssetSourceSelection";
import type { AuthCredentials } from "../generated/bsaigc/AuthCredentials";
import type { AuthStatus } from "../generated/bsaigc/AuthStatus";
import type { BrainThreadRecord } from "../generated/bsaigc/BrainThreadRecord";
import type { BrainTurnContext } from "../generated/bsaigc/BrainTurnContext";
import type { BusinessDocumentStatus } from "../generated/bsaigc/BusinessDocumentStatus";
import type { BusinessDocumentFormat } from "../generated/bsaigc/BusinessDocumentFormat";
import type { BusinessDocumentRecord } from "../generated/bsaigc/BusinessDocumentRecord";
import type { BusinessProfileInput } from "../generated/bsaigc/BusinessProfileInput";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import type { BusinessAcceptanceMaterialInput } from "../generated/bsaigc/BusinessAcceptanceMaterialInput";
import type { CreateBusinessAcceptanceBatchPayload } from "../generated/bsaigc/CreateBusinessAcceptanceBatchPayload";
import type { UpsertBusinessSettlementBatchPayload } from "../generated/bsaigc/UpsertBusinessSettlementBatchPayload";
import type { VoidBusinessSettlementBatchPayload } from "../generated/bsaigc/VoidBusinessSettlementBatchPayload";
import {
  buildBusinessWorkspaceContext,
  buildBusinessWorkspaceTaskMessages,
  findBusinessWorkspace,
} from "./application/businessWorkspaceView";
import {
  buildBusinessTurnInput,
  businessTaskThreadTitle,
  canReuseThreadForTask,
  routeBusinessTask,
} from "./application/taskRouting";
import { toWorkbenchMessages, type WorkbenchMessage } from "./application/workbenchModel";
import { useBusinessSettingsController } from "./useBusinessSettingsController";
import {
  AnnualSettlementCenter,
  type AnnualSettlementBatch,
  type AnnualSettlementBatchInput as AnnualSettlementViewInput,
  type AnnualSettlementWorkspace,
} from "./ui/AnnualSettlementCenter";
import { AcceptanceCenter } from "./ui/AcceptanceCenter";
import { SharedCaseCenter } from "./ui/SharedCaseCenter";
import {
  ContractLegalCenter,
  type ContractLegalAttachmentCandidate,
} from "./ui/ContractLegalCenter";
import {
  BusinessWorkspaceShell,
  HistoricalDataCenter,
  QuotationCenter,
  type BusinessTaskKind,
  type BusinessWorkspaceActions,
  type ChatMessage,
  type NetworkScope,
  type QuotationCenterInput,
  type SourceScope,
  type WorkspaceAttachment,
  type WorkspaceContext,
} from "./ui";

const desktopRuntime = isTauriRuntime();
const hostAdapter = desktopRuntime ? new DesktopHostAdapter() : new WebHostAdapter();
const client = new BsaigcClient(hostAdapter, {
  actorId: "local-operator",
  windowId: "business-v1",
});

const QUICK_PROMPTS: Record<BusinessTaskKind, string> = {
  quotation: "为当前项目生成一份报价草稿，先核对服务行项、数量、税率和项目优惠。",
  "contract-review": "审查当前合同，重点检查付款、验收、版权、违约和终止条款。",
  acceptance: "根据当前合同和素材启动本次验收，先列出缺失、冲突和重复材料。",
  settlement: "整理当前项目的结算、请款、发票和回款状态。",
  archive: "检查当前项目是否满足归档条件，并生成归档清单。",
  search: "检索当前项目和已授权案例中的相关资料。",
};

function formatTime(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(value);
}

export function toBusinessChatMessage(message: WorkbenchMessage): ChatMessage {
  return {
    id: message.id,
    role: message.role,
    authorName: message.role === "user" ? "我" : message.role === "assistant" ? PRODUCT_AGENT_NAME : "系统",
    createdAt: formatTime(message.createdAt),
    content: message.text,
    sources: message.sources,
  };
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function attachmentKind(source: AssetSourceSelection): WorkspaceAttachment["kind"] {
  if (source.detectedKind === "image") return "image";
  const extension = source.displayName.split(".").pop()?.toLocaleLowerCase();
  if (extension === "pdf") return "pdf";
  if (extension === "xlsx" || extension === "xls") return "spreadsheet";
  if (source.detectedKind === "document") return "document";
  return "file";
}

const CONTRACT_FILE_EXTENSIONS = new Set(["pdf", "doc", "docx"]);

function isContractAttachmentName(name: string): boolean {
  const extension = name.split(".").pop()?.toLocaleLowerCase();
  return extension ? CONTRACT_FILE_EXTENSIONS.has(extension) : false;
}

const QUANTITY_MILLIS = 1_000;

function quantityFromMillis(value: number): number {
  return Math.max(0, value) / QUANTITY_MILLIS;
}

function quantityToMillis(value: number): number {
  return Math.round(Math.max(0, value) * QUANTITY_MILLIS);
}

function normalizedLineItemName(value: string): string {
  return value.trim().toLocaleLowerCase();
}

function matchingLineItem(
  workspace: BusinessWorkspaceRecord,
  deliverableId: string,
  deliverableName: string,
) {
  const normalizedName = normalizedLineItemName(deliverableName);
  return workspace.profile.lineItems.find((lineItem) =>
    lineItem.id === deliverableId || normalizedLineItemName(lineItem.name) === normalizedName
  ) ?? null;
}

function latestSettlementLines(workspace: BusinessWorkspaceRecord) {
  const latest = new Map<string, { updatedAt: number; line: BusinessWorkspaceRecord["settlementBatches"][number]["lines"][number] }>();
  for (const batch of workspace.settlementBatches) {
    for (const line of batch.lines) {
      const current = latest.get(line.deliverableId);
      if (!current || batch.updatedAt >= current.updatedAt) {
        latest.set(line.deliverableId, { updatedAt: batch.updatedAt, line });
      }
    }
  }
  return new Map([...latest].map(([deliverableId, entry]) => [deliverableId, entry.line]));
}

export function buildAnnualSettlementWorkspace(
  workspace: BusinessWorkspaceRecord,
): AnnualSettlementWorkspace {
  const latestLines = latestSettlementLines(workspace);
  const activeSettledMillis = new Map<string, number>();
  for (const batch of workspace.settlementBatches) {
    if (batch.status === "voided") continue;
    for (const line of batch.lines) {
      activeSettledMillis.set(
        line.deliverableId,
        Math.max(activeSettledMillis.get(line.deliverableId) ?? 0, line.cumulativeSettledMillis),
      );
    }
  }

  return {
    id: workspace.id,
    projectTitle: workspace.profile.projectTitle.trim() || "未命名项目",
    projectCode: workspace.profile.projectCode.trim() || undefined,
    customerName: workspace.profile.customerName.trim() || workspace.customer.displayName.trim() || undefined,
    deliverables: workspace.milestones.flatMap((milestone) => milestone.deliverables.map((deliverable) => {
      const latestLine = latestLines.get(deliverable.id);
      const lineItem = matchingLineItem(workspace, deliverable.id, deliverable.name);
      const contractQuantityMillis = latestLine?.contractQuantityMillis ?? lineItem?.quantityMillis ?? QUANTITY_MILLIS;
      const hasAcceptedVersion = deliverable.versions.some((version) => version.status === "accepted");
      const hasExecutedVersion = deliverable.versions.some((version) => version.status === "sent" || version.status === "accepted");
      const inferredExecutedMillis = milestone.status === "delivered" || milestone.status === "accepted" || hasExecutedVersion
        ? contractQuantityMillis
        : 0;
      const inferredAcceptedMillis = milestone.status === "accepted" || hasAcceptedVersion
        ? contractQuantityMillis
        : 0;
      return {
        id: deliverable.id,
        milestoneId: milestone.id,
        milestoneTitle: milestone.title,
        name: deliverable.name,
        unit: latestLine?.unit || lineItem?.unit || "项",
        contractQuantity: quantityFromMillis(contractQuantityMillis),
        executedQuantity: quantityFromMillis(latestLine?.cumulativeExecutedMillis ?? inferredExecutedMillis),
        acceptedQuantity: quantityFromMillis(latestLine?.cumulativeAcceptedMillis ?? inferredAcceptedMillis),
        settledQuantity: quantityFromMillis(activeSettledMillis.get(deliverable.id) ?? 0),
      };
    })),
  };
}

export function buildAnnualSettlementBatches(
  workspace: BusinessWorkspaceRecord,
): AnnualSettlementBatch[] {
  const milestoneTitles = new Map(workspace.milestones.map((milestone) => [milestone.id, milestone.title]));
  return [...workspace.settlementBatches]
    .sort((left, right) => right.updatedAt - left.updatedAt)
    .map((batch) => ({
      id: batch.id,
      workspaceId: batch.workspaceId,
      period: batch.settlementPeriod,
      cadence: batch.cadence,
      status: batch.status,
      lines: batch.lines.map((line) => ({
        deliverableId: line.deliverableId,
        deliverableName: line.deliverableName,
        milestoneTitle: milestoneTitles.get(line.milestoneId) ?? "未分组",
        unit: line.unit,
        quantity: quantityFromMillis(line.currentSettlementMillis),
      })),
      note: batch.notes,
      createdAt: batch.createdAt,
      updatedAt: batch.updatedAt,
      voidedAt: batch.voidedAt,
    }));
}

function settlementContractNumber(
  workspace: BusinessWorkspaceRecord,
  batchId: string | null,
): string {
  const existingBatch = batchId
    ? workspace.settlementBatches.find((batch) => batch.id === batchId)
    : null;
  if (existingBatch?.contractNumber.trim()) return existingBatch.contractNumber.trim();
  const contractDocument = workspace.documents.find((document) =>
    document.id === workspace.currentDocuments.contractDocumentId && document.status !== "voided"
  );
  return contractDocument?.documentNumber.trim()
    || workspace.settlementBatches.find((batch) => batch.status !== "voided")?.contractNumber.trim()
    || workspace.profile.projectCode.trim()
    || workspace.id;
}

export function buildUpsertAnnualSettlementPayload(
  workspace: BusinessWorkspaceRecord,
  input: AnnualSettlementViewInput,
): UpsertBusinessSettlementBatchPayload {
  const viewWorkspace = buildAnnualSettlementWorkspace(workspace);
  const deliverables = new Map(viewWorkspace.deliverables.map((deliverable) => [deliverable.id, deliverable]));
  const existingBatch = input.id
    ? workspace.settlementBatches.find((batch) => batch.id === input.id)
    : null;

  return {
    workspaceId: workspace.id,
    batch: {
      id: input.id,
      contractNumber: settlementContractNumber(workspace, input.id),
      settlementPeriod: input.period.trim(),
      cadence: input.cadence,
      status: existingBatch?.status ?? "draft",
      lines: input.lines.map((inputLine) => {
        const deliverable = deliverables.get(inputLine.deliverableId);
        if (!deliverable) throw new Error(`交付项“${inputLine.deliverableName}”已不在当前工作区。`);
        const existingLine = existingBatch?.lines.find((line) => line.deliverableId === inputLine.deliverableId);
        const cumulativeExecutedMillis = quantityToMillis(deliverable.executedQuantity);
        const cumulativeAcceptedMillis = quantityToMillis(deliverable.acceptedQuantity);
        return {
          deliverableId: deliverable.id,
          contractQuantityMillis: quantityToMillis(deliverable.contractQuantity),
          cumulativeExecutedMillis,
          currentExecutedMillis: existingLine?.currentExecutedMillis ?? cumulativeExecutedMillis,
          cumulativeAcceptedMillis,
          currentAcceptedMillis: existingLine?.currentAcceptedMillis ?? cumulativeAcceptedMillis,
          currentSettlementMillis: quantityToMillis(inputLine.quantity),
          unit: deliverable.unit,
          notes: existingLine?.notes ?? "",
        };
      }),
      notes: input.note.trim(),
    },
  };
}

export function buildVoidAnnualSettlementPayload(
  workspace: BusinessWorkspaceRecord,
  batch: AnnualSettlementBatch,
  reason: string,
): VoidBusinessSettlementBatchPayload {
  return {
    workspaceId: workspace.id,
    batchId: batch.id,
    reason: reason.trim(),
  };
}

export type AnnualSettlementClient = Pick<
  BsaigcClient,
  "upsertBusinessSettlementBatch" | "voidBusinessSettlementBatch"
>;

export function buildBusinessBrainTurnContext(
  networkScope: NetworkScope | null | undefined,
  workspaceToken: string | null,
  attachmentAssetIds: readonly string[],
): BrainTurnContext {
  return {
    workspaceToken,
    accessMode: "requestApproval",
    webEnabled: networkScope === "web-enabled",
    attachmentAssetIds: [...attachmentAssetIds],
  };
}

export async function upsertAnnualSettlementBatch(
  settlementClient: AnnualSettlementClient,
  workspace: BusinessWorkspaceRecord,
  input: AnnualSettlementViewInput,
) {
  return settlementClient.upsertBusinessSettlementBatch(
    buildUpsertAnnualSettlementPayload(workspace, input),
    workspace.revision,
  );
}

export async function voidAnnualSettlementBatch(
  settlementClient: AnnualSettlementClient,
  workspace: BusinessWorkspaceRecord,
  batch: AnnualSettlementBatch,
  reason: string,
) {
  const payload = buildVoidAnnualSettlementPayload(workspace, batch, reason);
  if (!payload.reason) throw new Error("请输入作废原因。");
  return settlementClient.voidBusinessSettlementBatch(payload, workspace.revision);
}

export function businessDocumentOutputFormat(
  workspace: BusinessWorkspaceRecord,
  document: BusinessDocumentRecord,
): BusinessDocumentFormat {
  if (document.kind === "quote") return "xlsx";
  if (document.kind !== "acceptance") return "docx";

  const batchId = document.snapshot.acceptanceBatchId;
  const outputSpecId = document.snapshot.acceptanceOutputSpecId;
  if (!batchId || !outputSpecId) return "docx";

  const outputSpec = workspace.acceptanceBatches
    .find((batch) => batch.id === batchId)
    ?.outputSpecs.find((candidate) => candidate.id === outputSpecId);
  if (!outputSpec) {
    throw new Error("验收文件缺少对应的输出规格，不能生成正式文件。");
  }
  return outputSpec.format;
}

export type QuotationWorkflowClient = Pick<
  BsaigcClient,
  "updateBusinessProfile" | "createBusinessDocument" | "changeBusinessDocumentStatus" | "generateBusinessDocument"
>;

export function buildQuotationProfileInput(
  workspace: BusinessWorkspaceRecord,
  input: QuotationCenterInput,
): BusinessProfileInput {
  const { quotationTotals: _quotationTotals, lineItems: _lineItems, ...profile } = workspace.profile;
  return {
    ...profile,
    projectDiscountCents: input.projectDiscountCents,
    defaultTaxRateBps: input.defaultTaxRateBps,
    taxMode: input.taxMode,
    lineItems: input.lineItems.map((line) => ({
      id: line.id,
      name: line.name.trim(),
      description: line.description.trim(),
      quantityMillis: line.quantityMillis,
      unit: line.unit.trim() || "项",
      unitPriceCents: line.unitPriceCents,
      taxRateBps: line.taxRateBps,
    })),
  };
}

export async function saveBusinessQuotationProfile(
  quotationClient: QuotationWorkflowClient,
  workspace: BusinessWorkspaceRecord,
  input: QuotationCenterInput,
): Promise<BusinessWorkspaceRecord> {
  const response = await quotationClient.updateBusinessProfile({
    workspaceId: workspace.id,
    profile: buildQuotationProfileInput(workspace, input),
  }, workspace.revision);
  return response.businessWorkspace;
}

function nextQuotationDocumentNumber(workspace: BusinessWorkspaceRecord): string {
  const nextSequence = workspace.documents
    .filter((document) => document.kind === "quote")
    .reduce((maximum, document) => Math.max(maximum, document.sequenceNumber), 0) + 1;
  const projectCode = workspace.profile.projectCode.trim() || workspace.projectId;
  return `${projectCode}-BJ-V${String(nextSequence).padStart(2, "0")}`;
}

export async function advanceBusinessQuotationApproval(
  quotationClient: QuotationWorkflowClient,
  workspace: BusinessWorkspaceRecord,
  documentId: string | null,
): Promise<BusinessWorkspaceRecord> {
  let currentWorkspace = workspace;
  let document = documentId
    ? currentWorkspace.documents.find((candidate) => candidate.id === documentId) ?? null
    : null;
  if (!document || document.kind !== "quote" || ["generated", "effective", "voided"].includes(document.status)) {
    const created = await quotationClient.createBusinessDocument({
      workspaceId: currentWorkspace.id,
      kind: "quote",
      documentNumber: nextQuotationDocumentNumber(currentWorkspace),
      title: `${currentWorkspace.profile.projectTitle || "项目"}报价单`,
      templateKey: "builtin.business.quote.v1",
      paymentId: null,
      acceptanceBatchId: null,
    }, currentWorkspace.revision);
    currentWorkspace = created.businessWorkspace;
    document = currentWorkspace.documents.find((candidate) =>
      candidate.id === currentWorkspace.currentDocuments.quoteDocumentId
    ) ?? null;
  }
  if (!document) throw new Error("报价文档创建后未进入当前工作区。" );
  if (document.status === "draft") {
    const submitted = await quotationClient.changeBusinessDocumentStatus({
      workspaceId: currentWorkspace.id,
      documentId: document.id,
      status: "inReview",
      evidence: null,
      manualWaiver: null,
      reason: "提交人工确认",
    }, currentWorkspace.revision);
    return submitted.businessWorkspace;
  }
  if (document.status === "inReview") {
    const approved = await quotationClient.changeBusinessDocumentStatus({
      workspaceId: currentWorkspace.id,
      documentId: document.id,
      status: "approved",
      evidence: null,
      manualWaiver: null,
      reason: "报价金额、优惠、税率和版本已人工确认",
    }, currentWorkspace.revision);
    return approved.businessWorkspace;
  }
  return currentWorkspace;
}

export async function generateBusinessQuotationXlsx(
  quotationClient: QuotationWorkflowClient,
  workspace: BusinessWorkspaceRecord,
  documentId: string,
): Promise<BusinessWorkspaceRecord> {
  const document = workspace.documents.find((candidate) => candidate.id === documentId && candidate.kind === "quote");
  if (!document) throw new Error("找不到要生成的报价文档。" );
  if (document.status !== "approved") throw new Error("报价尚未完成人工确认，不能生成正式 XLSX。" );
  const response = await quotationClient.generateBusinessDocument({
    workspaceId: workspace.id,
    documentId,
    format: "xlsx",
  }, workspace.revision);
  return response.businessWorkspace;
}

const ACCEPTANCE_REQUIREMENTS = [
  { label: "视频成片", kind: "video", requiredGroupCount: 4 },
  { label: "脚本", kind: "script", requiredGroupCount: 1 },
  { label: "视频截图", kind: "screenshot", requiredGroupCount: 1 },
  { label: "拍摄花絮", kind: "behindTheScenes", requiredGroupCount: 1 },
  { label: "发布数据", kind: "publishingData", requiredGroupCount: 1 },
  { label: "验收证明", kind: "proof", requiredGroupCount: 1 },
] as const;

const ACCEPTANCE_OUTPUTS = [
  { outputCode: "contract-settlement", title: "合同费用结算明细", format: "xlsx" },
  { outputCode: "service-settlement-list", title: "服务项目结算清单", format: "docx" },
  { outputCode: "payment-application", title: "付款申请与结算计算", format: "docx" },
  { outputCode: "video-completion-acceptance", title: "视频制作完成验收单", format: "docx" },
  { outputCode: "production-result-confirmation", title: "制作成果确认单", format: "docx" },
] as const;

function acceptanceRecordId(prefix: string): string {
  const randomUuid = globalThis.crypto?.randomUUID?.();
  return `${prefix}-${randomUuid ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`}`;
}

export function buildDefaultAcceptanceBatchPayload(
  workspaceId: string,
  label: string,
  createId: (prefix: string) => string = acceptanceRecordId,
): CreateBusinessAcceptanceBatchPayload {
  const requirements = ACCEPTANCE_REQUIREMENTS.map((requirement) => ({
    id: createId(`acceptance-requirement-${requirement.kind}`),
    label: requirement.label,
    kind: requirement.kind,
    requiredGroupCount: requirement.requiredGroupCount,
  }));
  const requirementIds = requirements.map((requirement) => requirement.id);
  const normalizedLabel = label.trim() || "本次验收";

  return {
    workspaceId,
    label: normalizedLabel,
    requirements,
    outputSpecs: ACCEPTANCE_OUTPUTS.map((output, index) => ({
      id: createId(`acceptance-output-${index + 1}`),
      outputCode: output.outputCode,
      documentNumber: `ACC-${String(index + 1).padStart(2, "0")}`,
      title: `${normalizedLabel}-${output.title}`,
      templateKey: "builtin.acceptance.standard.v1",
      templateAssetId: null,
      templateSourceSha256: null,
      templateMappingVersion: "",
      contractSettlement: null,
      serviceSettlementItems: [],
      paymentApplication: null,
      videoCompletionAcceptance: undefined,
      productionResultConfirmation: undefined,
      format: output.format,
      requirementIds,
    })),
  };
}

export function BusinessV1App() {
  const snapshot = useSyncExternalStore(client.subscribe, client.getSnapshot, client.getSnapshot);
  const settings = useBusinessSettingsController(client, desktopRuntime);
  const [authStatus, setAuthStatus] = useState<AuthStatus | null>(null);
  const [authChecked, setAuthChecked] = useState(!desktopRuntime);
  const [authBusy, setAuthBusy] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [authLoadError, setAuthLoadError] = useState<string | null>(null);
  const [rememberedAuth, setRememberedAuth] = useState<AuthCredentials | null>(null);
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null);
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [attachments, setAttachments] = useState<WorkspaceAttachment[]>([]);
  const [attachmentAssetIds, setAttachmentAssetIds] = useState<string[]>([]);
  const [workspaceByProject, setWorkspaceByProject] = useState<
    Record<string, { token: string; label: string; revision: number }>
  >({});
  const [sourceScope, setSourceScope] = useState<SourceScope>("workspace");
  const [networkScope, setNetworkScope] = useState<NetworkScope>("local-only");
  const [modelId, setModelId] = useState("default");
  const [busy, setBusy] = useState(false);
  const [localMessage, setLocalMessage] = useState<string | null>(null);
  const [projectDraft, setProjectDraft] = useState<{ name: string; customerName: string } | null>(null);
  const [preparingAcceptanceBatchId, setPreparingAcceptanceBatchId] = useState<string | null>(null);
  const [settlementCenterOpen, setSettlementCenterOpen] = useState(false);
  const [contractLegalCenterOpen, setContractLegalCenterOpen] = useState(false);
  const [historicalDataCenterOpen, setHistoricalDataCenterOpen] = useState(false);
  const [quotationCenterOpen, setQuotationCenterOpen] = useState(false);
  const [acceptanceCenterOpen, setAcceptanceCenterOpen] = useState(false);

  useEffect(() => {
    document.title = `${PRODUCT_NAME} 1.0`;
    if (!desktopRuntime) return;
    let cancelled = false;
    Promise.allSettled([client.authStatus(), client.authRememberedCredentials()])
      .then(async ([status, remembered]) => {
        if (cancelled) return;
        const rememberedCredentials =
          remembered.status === "fulfilled" ? remembered.value : null;
        if (remembered.status === "fulfilled") {
          setRememberedAuth(rememberedCredentials);
        }
        if (status.status === "fulfilled") {
          let resolvedStatus = status.value;
          if (!resolvedStatus.currentUser && rememberedCredentials) {
            try {
              resolvedStatus = await client.authLoginRemembered();
            } catch (error) {
              setAuthError(localizeAuthError(error));
            }
          }
          if (cancelled) return;
          setAuthStatus(resolvedStatus);
          setAuthLoadError(null);
          if (resolvedStatus.currentUser) {
            await client.start();
            const bindings = await client.listBrainProjectWorkspaces();
            if (cancelled) return;
            setWorkspaceByProject(Object.fromEntries(bindings.map((binding) => [
              binding.projectId,
              {
                token: binding.workspaceToken,
                label: binding.displayName,
                revision: binding.revision,
              },
            ])));
          }
        } else {
          setAuthLoadError(localizeAuthError(status.reason));
        }
      })
      .catch((error) => {
        if (!cancelled) setLocalMessage(localizeAuthError(error));
      })
      .finally(() => {
        if (!cancelled) setAuthChecked(true);
      });
    return () => {
      cancelled = true;
      client.stop();
    };
  }, []);

  useEffect(() => {
    if (!activeProjectId && snapshot.projects[0]) setActiveProjectId(snapshot.projects[0].id);
  }, [activeProjectId, snapshot.projects]);

  useEffect(() => {
    const selected = snapshot.brainThreads.find((thread) => thread.id === activeConversationId);
    if (selected && (activeProjectId === null || selected.projectId === activeProjectId)) return;
    const first = snapshot.brainThreads.find((thread) => thread.projectId === activeProjectId && thread.status !== "archived");
    setActiveConversationId(first?.id ?? null);
  }, [activeConversationId, activeProjectId, snapshot.brainThreads]);

  useEffect(() => {
    if (activeConversationId) void client.refreshBrainTurns(activeConversationId);
  }, [activeConversationId]);

  const projects = useMemo(
    () => snapshot.projects.map((project) => ({
      id: project.id,
      name: project.name,
      customerName: project.clientName,
      updatedAt: formatTime(project.updatedAt),
      localPath: workspaceByProject[project.id]?.label,
    })),
    [snapshot.projects, workspaceByProject],
  );
  const conversations = useMemo(
    () => snapshot.brainThreads
      .filter((thread) => thread.status !== "archived" && thread.projectId === activeProjectId)
      .map((thread) => ({
        id: thread.id,
        projectId: thread.projectId ?? "unscoped",
        title: thread.title?.trim() || "新商务任务",
        preview: thread.status === "running" ? "正在执行" : "本地会话",
        updatedAt: formatTime(thread.updatedAt),
      })),
    [activeProjectId, snapshot.brainThreads],
  );
  const activeBusinessWorkspace = useMemo(
    () => findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId),
    [activeProjectId, snapshot.businessWorkspaces],
  );
  const annualSettlementWorkspace = useMemo(
    () => activeBusinessWorkspace ? buildAnnualSettlementWorkspace(activeBusinessWorkspace) : null,
    [activeBusinessWorkspace],
  );
  const annualSettlementBatches = useMemo(
    () => activeBusinessWorkspace ? buildAnnualSettlementBatches(activeBusinessWorkspace) : [],
    [activeBusinessWorkspace],
  );
  const contractAttachmentCandidates = useMemo<ContractLegalAttachmentCandidate[]>(() => {
    if (!activeProjectId) return [];
    const candidates = new Map<string, ContractLegalAttachmentCandidate>();
    for (const asset of snapshot.assets) {
      if (asset.projectId !== activeProjectId || !isContractAttachmentName(asset.originalName)) continue;
      candidates.set(asset.id, {
        id: asset.id,
        name: asset.originalName,
        sourceLabel: "项目资产",
        status: asset.status,
      });
    }
    for (const attachment of attachments) {
      if (!attachmentAssetIds.includes(attachment.id) || !isContractAttachmentName(attachment.name)) continue;
      candidates.set(attachment.id, {
        id: attachment.id,
        name: attachment.name,
        sourceLabel: attachment.sourceLabel ?? "本地文件",
        status: attachment.status === "reading" ? "processing" : attachment.status,
      });
    }
    return [...candidates.values()].sort((left, right) => left.name.localeCompare(right.name, "zh-CN"));
  }, [activeProjectId, attachmentAssetIds, attachments, snapshot.assets]);
  const acceptanceAssetCandidates = useMemo(() => activeProjectId
    ? snapshot.assets
      .filter((asset) => asset.projectId === activeProjectId)
      .map((asset) => ({ id: asset.id, name: asset.originalName, kind: asset.kind }))
    : [], [activeProjectId, snapshot.assets]);
  const workspaceContext = useMemo<WorkspaceContext>(
    () => buildBusinessWorkspaceContext(activeBusinessWorkspace, activeProjectId !== null, {
      preparingAcceptanceBatchId,
      isBusy: busy,
    }),
    [activeBusinessWorkspace, activeProjectId, busy, preparingAcceptanceBatchId],
  );
  const messages = useMemo<ChatMessage[]>(() => {
    const mapped = toWorkbenchMessages(
      snapshot.brainTurns.filter((turn) => turn.threadId === activeConversationId),
    ).map(toBusinessChatMessage);
    mapped.push(...buildBusinessWorkspaceTaskMessages(activeBusinessWorkspace));
    if (localMessage) mapped.push({
      id: "local-message",
      role: "system",
      authorName: "系统",
      createdAt: formatTime(Date.now()),
      content: localMessage,
    });
    return mapped;
  }, [activeBusinessWorkspace, activeConversationId, localMessage, snapshot.brainTurns]);

  const syncRememberedAuth = async (username: string, password: string, remember: boolean) => {
    if (remember) {
      const credentials = { username, password };
      await client.authRememberCredentials(credentials);
      setRememberedAuth(credentials);
    } else {
      await client.authForgetCredentials();
      setRememberedAuth(null);
    }
  };
  const handleAuthChangePassword = async (oldPassword: string, newPassword: string) => {
    const status = await client.authChangePassword({ oldPassword, newPassword });
    setAuthStatus(status);
    if (rememberedAuth && status.currentUser?.username === rememberedAuth.username) {
      try {
        await client.authRememberCredentials({
          username: rememberedAuth.username,
          password: newPassword,
        });
        setRememberedAuth({ username: rememberedAuth.username, password: newPassword });
      } catch {
        await client.authForgetCredentials().catch(() => undefined);
        setRememberedAuth(null);
      }
    }
    return status;
  };

  const runAuth = async (
    operation: () => Promise<AuthStatus>,
    username: string,
    password: string,
    remember: boolean,
  ) => {
    setAuthBusy(true);
    setAuthError(null);
    try {
      const status = await operation();
      try {
        await syncRememberedAuth(username, password, remember);
      } catch (error) {
        await client.authLogout().catch(() => undefined);
        throw error;
      }
      setAuthStatus(status);
      await client.start();
      const bindings = await client.listBrainProjectWorkspaces();
      setWorkspaceByProject(Object.fromEntries(bindings.map((binding) => [
        binding.projectId,
        {
          token: binding.workspaceToken,
          label: binding.displayName,
          revision: binding.revision,
        },
      ])));
    } catch (error) {
      setAuthError(localizeAuthError(error));
    } finally {
      setAuthBusy(false);
    }
  };

  const createConversation = async (title: string | null = null): Promise<BrainThreadRecord | null> => {
    if (busy) return null;
    if (!activeProjectId) {
      setLocalMessage("请先创建或选择一个客户项目。" );
      return null;
    }
    setBusy(true);
    setLocalMessage(null);
    try {
      const thread = await client.startBrainThread({
        projectId: activeProjectId,
        title,
        model: modelId === "default" ? null : modelId,
      });
      setActiveConversationId(thread.id);
      return thread;
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      return null;
    } finally {
      setBusy(false);
    }
  };

  const importSources = async (sources: readonly AssetSourceSelection[]) => {
    for (const source of sources) {
        const imported = await client.importAsset(source.sourceToken, activeProjectId);
        setAttachmentAssetIds((current) => [...new Set([...current, imported.asset.id])]);
        setAttachments((current) => current.some((item) => item.id === imported.asset.id) ? current : [...current, {
          id: imported.asset.id,
          name: imported.asset.originalName,
          kind: attachmentKind(source),
          sizeLabel: formatSize(imported.asset.sizeBytes),
          sourceLabel: "本地文件",
          status: "ready",
        }]);
    }
  };

  const addFiles = async () => {
    setLocalMessage(null);
    if (!activeProjectId) {
      setLocalMessage("请先创建或选择一个客户项目。" );
      return;
    }
    try {
      const sources = await client.selectAssetSources();
      await importSources(sources);
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
    }
  };

  const stageImages = async (images: readonly File[]) => {
    setLocalMessage(null);
    if (!activeProjectId) {
      setLocalMessage("请先创建或选择一个客户项目。" );
      return;
    }
    try {
      const sources: AssetSourceSelection[] = [];
      for (const [index, image] of images.entries()) {
        const bytes = Array.from(new Uint8Array(await image.arrayBuffer()));
        sources.push(await client.stageClipboardImage({
          fileName: image.name || `clipboard-image-${index + 1}.png`,
          mimeType: image.type || "image/png",
          bytes,
        }));
      }
      await importSources(sources);
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
    }
  };

  const importDroppedPaths = async (paths: readonly string[]) => {
    setLocalMessage(null);
    if (!activeProjectId) {
      setLocalMessage("请先创建或选择一个客户项目。" );
      return;
    }
    try {
      const dropped = await client.registerBrainDroppedPaths([...paths]);
      if (dropped.workspace && activeProjectId) {
        const binding = await client.bindBrainProjectWorkspace(
          activeProjectId,
          dropped.workspace.workspaceToken,
          workspaceByProject[activeProjectId]?.revision ?? null,
        );
        setWorkspaceByProject((current) => ({
          ...current,
          [activeProjectId]: {
            token: binding.workspaceToken,
            label: binding.displayName,
            revision: binding.revision,
          },
        }));
      }
      await importSources(dropped.files);
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
    }
  };

  const addFolder = async () => {
    setLocalMessage(null);
    if (!activeProjectId) {
      setLocalMessage("请先创建或选择一个客户项目。" );
      return;
    }
    try {
      const workspace = await client.selectBrainWorkspace();
      if (!workspace || !activeProjectId) return;
      const binding = await client.bindBrainProjectWorkspace(
        activeProjectId,
        workspace.workspaceToken,
        workspaceByProject[activeProjectId]?.revision ?? null,
      );
      setWorkspaceByProject((current) => ({
        ...current,
        [activeProjectId]: {
          token: binding.workspaceToken,
          label: binding.displayName,
          revision: binding.revision,
        },
      }));
      setAttachments((current) => [
        ...current.filter((item) => item.kind !== "folder"),
        { id: `workspace:${workspace.workspaceToken}`, name: workspace.displayName, kind: "folder", sourceLabel: "项目工作区", status: "ready" },
      ]);
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
    }
  };

  const sendMessage = async () => {
    const prompt = draft.trim() || (attachments.length ? "请处理我附上的资料。" : "");
    if (!prompt || busy) return;
    if (!activeProjectId) {
      setLocalMessage("请先创建或选择一个客户项目。" );
      return;
    }
    setBusy(true);
    setLocalMessage(null);
    try {
      const routed = routeBusinessTask({
        prompt,
        projectId: activeProjectId,
        knowledgeScope: networkScope === "web-enabled" ? "web" : sourceScope === "workspace-shared" ? "shared" : "local",
      });
      let thread = snapshot.brainThreads.find((candidate) => candidate.id === activeConversationId) ?? null;
      if (!canReuseThreadForTask(thread, routed)) {
        thread = await client.startBrainThread({
          projectId: activeProjectId,
          title: businessTaskThreadTitle(routed),
          model: modelId === "default" ? null : modelId,
        });
        setActiveConversationId(thread.id);
      }
      if (!thread) throw new Error("无法建立商务任务会话");
      await client.startBrainTurn({
        threadId: thread.id,
        inputText: buildBusinessTurnInput(routed),
        model: modelId === "default" ? null : modelId,
        effort: "high",
      }, {
        workspaceToken: workspaceByProject[activeProjectId]?.token ?? null,
        accessMode: "requestApproval",
        webEnabled: networkScope === "web-enabled",
        attachmentAssetIds,
      });
      setDraft("");
      setAttachments([]);
      setAttachmentAssetIds([]);
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
    } finally {
      setBusy(false);
    }
  };

  const ensureBusinessWorkspace = async () => {
    if (!activeProjectId || activeBusinessWorkspace) return;
    try {
      await client.createBusinessWorkspace({ projectId: activeProjectId });
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
    }
  };

  const handleAnnualSettlementUpsert = async (input: AnnualSettlementViewInput) => {
    const workspace = findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId);
    if (!workspace) throw new Error("当前项目工作区尚未准备完成。");
    if (busy) throw new Error("已有操作正在执行，请稍候。");
    setBusy(true);
    setLocalMessage(null);
    try {
      await upsertAnnualSettlementBatch(client, workspace, input);
      setLocalMessage(input.id ? "结算批次已更新。" : "结算批次已创建。");
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      throw error;
    } finally {
      setBusy(false);
    }
  };

  const handleAnnualSettlementVoid = async (batch: AnnualSettlementBatch) => {
    const reason = window.prompt("请输入作废原因", "业务调整");
    if (reason === null) return;
    const workspace = findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId);
    if (!workspace) throw new Error("当前项目工作区尚未准备完成。");
    if (busy) throw new Error("已有操作正在执行，请稍候。");
    setBusy(true);
    setLocalMessage(null);
    try {
      await voidAnnualSettlementBatch(client, workspace, batch, reason);
      setLocalMessage("结算批次已作废，相关交付项可重新结算。");
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      throw error;
    } finally {
      setBusy(false);
    }
  };

  const handleQuotationSave = async (input: QuotationCenterInput) => {
    const workspace = findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId);
    if (!workspace) throw new Error("当前项目工作区尚未准备完成。" );
    if (busy) throw new Error("已有操作正在执行，请稍候。" );
    setBusy(true);
    setLocalMessage(null);
    try {
      await saveBusinessQuotationProfile(client, workspace, input);
      setLocalMessage("报价参数已保存，正式金额已由业务服务重新计算。" );
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      throw error;
    } finally {
      setBusy(false);
    }
  };

  const handleQuotationApproval = async (documentId: string | null) => {
    const workspace = findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId);
    if (!workspace) throw new Error("当前项目工作区尚未准备完成。" );
    if (busy) throw new Error("已有操作正在执行，请稍候。" );
    setBusy(true);
    setLocalMessage(null);
    try {
      const result = await advanceBusinessQuotationApproval(client, workspace, documentId);
      const currentQuote = result.documents.find((document) => document.id === result.currentDocuments.quoteDocumentId);
      setLocalMessage(currentQuote?.status === "approved" ? "报价已完成人工确认，可以生成正式 XLSX。" : "报价已提交人工确认。" );
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      throw error;
    } finally {
      setBusy(false);
    }
  };

  const handleQuotationGenerate = async (documentId: string) => {
    const workspace = findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId);
    if (!workspace) throw new Error("当前项目工作区尚未准备完成。" );
    if (busy) throw new Error("已有操作正在执行，请稍候。" );
    setBusy(true);
    setLocalMessage(null);
    try {
      await generateBusinessQuotationXlsx(client, workspace, documentId);
      setLocalMessage("正式报价 XLSX 已生成并进入项目成果。" );
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      throw error;
    } finally {
      setBusy(false);
    }
  };

  const changeDocumentStatus = async (
    documentId: string,
    status: BusinessDocumentStatus,
    reason = "",
    propagateError = false,
  ) => {
    const workspace = findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId);
    if (!workspace || busy) return;
    setBusy(true);
    setLocalMessage(null);
    try {
      await client.changeBusinessDocumentStatus({
        workspaceId: workspace.id,
        documentId,
        status,
        evidence: null,
        manualWaiver: null,
        reason,
      }, workspace.revision);
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      if (propagateError) throw error;
    } finally {
      setBusy(false);
    }
  };

  const advanceBusinessDocument = async (documentId: string, propagateError = false) => {
    const workspace = findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId);
    const document = workspace?.documents.find((candidate) => candidate.id === documentId);
    if (!workspace || !document || busy) return;
    if (document.status === "draft") {
      await changeDocumentStatus(document.id, "inReview", "", propagateError);
      return;
    }
    if (document.status === "inReview") {
      await changeDocumentStatus(document.id, "approved", "", propagateError);
      return;
    }
    if (document.status !== "approved") {
      setLocalMessage("当前文档不需要继续确认。" );
      return;
    }
    setBusy(true);
    setLocalMessage(null);
    try {
      await client.generateBusinessDocument({
        workspaceId: workspace.id,
        documentId: document.id,
        format: businessDocumentOutputFormat(workspace, document),
      }, workspace.revision);
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      if (propagateError) throw error;
    } finally {
      setBusy(false);
    }
  };

  const prepareAcceptanceDocuments = async (batchId: string) => {
    const workspace = findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId);
    if (!workspace || busy || preparingAcceptanceBatchId) return;
    setBusy(true);
    setPreparingAcceptanceBatchId(batchId);
    setLocalMessage(null);
    try {
      await client.prepareBusinessAcceptanceDocuments({
        workspaceId: workspace.id,
        batchId,
      }, workspace.revision);
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      throw error;
    } finally {
      setPreparingAcceptanceBatchId(null);
      setBusy(false);
    }
  };

  const createAcceptanceBatch = async (label: string) => {
    const workspace = findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId);
    if (!workspace || busy) return;
    setBusy(true);
    setLocalMessage(null);
    try {
      await client.createBusinessAcceptanceBatch(
        buildDefaultAcceptanceBatchPayload(workspace.id, label),
        workspace.revision,
      );
      setLocalMessage("验收批次已创建，请绑定本次合同素材。" );
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      throw error;
    } finally {
      setBusy(false);
    }
  };

  const addAcceptanceMaterial = async (
    batchId: string,
    material: BusinessAcceptanceMaterialInput,
  ) => {
    const workspace = findBusinessWorkspace(snapshot.businessWorkspaces, activeProjectId);
    if (!workspace || busy) return;
    setBusy(true);
    setLocalMessage(null);
    try {
      await client.upsertBusinessAcceptanceMaterial({
        workspaceId: workspace.id,
        batchId,
        material,
      }, workspace.revision);
      setLocalMessage("验收素材已绑定，缺失检查已重新计算。" );
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
      throw error;
    } finally {
      setBusy(false);
    }
  };

  const createProject = async () => {
    const draftProject = projectDraft;
    const name = draftProject?.name.trim();
    if (!draftProject || !name || busy) return;
    setBusy(true);
    setLocalMessage(null);
    try {
      await client.createProject({
        name,
        clientName: draftProject.customerName.trim(),
      });
      setProjectDraft(null);
    } catch (error) {
      setLocalMessage(localizeAuthError(error));
    } finally {
      setBusy(false);
    }
  };

  const refreshHistoricalData = async () => {
    await Promise.all([
      client.refreshTasks(),
      client.refreshCases(),
      client.refreshRequirementBriefs(),
      client.refreshExecutionBriefs(),
      client.refreshAssets(),
      client.refreshBrainThreads(null),
    ]);
  };

  const restoreArchivedThread = async (threadId: string) => {
    await client.brainThreadArchive(threadId, false);
  };

  const actions: BusinessWorkspaceActions = {
    onCreateProject: () => setProjectDraft({ name: "", customerName: "" }),
    onSelectProject: (projectId) => {
      setQuotationCenterOpen(false);
      setAcceptanceCenterOpen(false);
      setSettlementCenterOpen(false);
      setContractLegalCenterOpen(false);
      setActiveConversationId(null);
      setAttachments([]);
      setAttachmentAssetIds([]);
      setDraft("");
      setLocalMessage(null);
      setActiveProjectId(projectId);
    },
    onProjectAction: () => setLocalMessage("项目置顶、重命名和归档将在项目资料库接入后开放。"),
    onCreateConversation: () => void createConversation(),
    onSelectConversation: setActiveConversationId,
    onConversationAction: (id, action) => {
      if (action === "archive") void client.brainThreadArchive(id, true);
      else if (action === "delete") void client.brainThreadDelete(id);
      else if (action === "rename") {
        const title = window.prompt("会话名称");
        if (title?.trim()) void client.brainThreadRename(id, title.trim());
      } else setLocalMessage("会话置顶功能暂未开放。" );
    },
    onStartTask: (kind) => {
      setDraft(QUICK_PROMPTS[kind]);
      setQuotationCenterOpen(kind === "quotation");
      setAcceptanceCenterOpen(kind === "acceptance");
      setSettlementCenterOpen(kind === "settlement");
      setContractLegalCenterOpen(kind === "contract-review");
      if (["quotation", "contract-review", "acceptance", "settlement", "archive"].includes(kind)) {
        void ensureBusinessWorkspace();
      }
    },
    onOpenWorkspaceFolder: () => void addFolder(),
    onOpenArtifact: (id) => void client.openAsset(id),
    onRetryTask: () => setLocalMessage("请在对应会话中重试任务。"),
    onConfirmTask: (id) => void advanceBusinessDocument(id),
    onComposerChange: setDraft,
    onSendMessage: () => void sendMessage(),
    onAddFiles: () => void addFiles(),
    onAddFolder: () => void addFolder(),
    onPasteScreenshot: (images) => {
      if (images?.length) void stageImages(images);
      else setLocalMessage("请直接在输入框粘贴截图。" );
    },
    onDropFiles: (files) => {
      const images = files.filter((file) => file.type.startsWith("image/"));
      if (images.length) void stageImages(images);
      if (images.length !== files.length) setLocalMessage("普通文件请使用附件按钮或桌面拖放导入。" );
    },
    onDropPaths: (paths) => void importDroppedPaths(paths),
    onRemoveAttachment: (id) => {
      setAttachments((current) => current.filter((item) => item.id !== id));
      setAttachmentAssetIds((current) => current.filter((assetId) => assetId !== id));
      if (id.startsWith("workspace:") && activeProjectId) {
        const projectId = activeProjectId;
        const binding = workspaceByProject[projectId];
        if (binding) {
          void client.unbindBrainProjectWorkspace(projectId, binding.revision)
            .then(() => setWorkspaceByProject((current) => {
              const next = { ...current };
              delete next[projectId];
              return next;
            }))
            .catch((error) => setLocalMessage(localizeAuthError(error)));
        }
      }
    },
    onSourceScopeChange: setSourceScope,
    onNetworkScopeChange: setNetworkScope,
    onModelChange: setModelId,
    onResolveMissingMaterial: () => setLocalMessage("请添加缺失资料后重新运行检查。"),
    onResolveConflict: () => setLocalMessage("字段冲突需要选择来源并留下确认记录。"),
    onSelectTemplate: () => setLocalMessage("模板匹配将在真实 Office PoC 完成后接入。"),
    onReviewLegalRisk: () => {
      if (!activeProjectId) {
        setLocalMessage("请先创建或选择一个客户项目。" );
        return;
      }
      setContractLegalCenterOpen(true);
      void ensureBusinessWorkspace();
    },
    onPrepareAcceptanceDocuments: (batchId) => void prepareAcceptanceDocuments(batchId),
    onOpenPreview: (id) => void client.openAsset(id),
    onApprovalDecision: (id, decision) => {
      if (decision === "approve") void advanceBusinessDocument(id);
      else {
        const document = activeBusinessWorkspace?.documents.find((candidate) => candidate.id === id);
        if (document?.status === "inReview") void changeDocumentStatus(id, "draft", "人工退回修改");
        else setLocalMessage("当前审批阶段不能直接退回。" );
      }
    },
    onRestoreVersion: () => setLocalMessage("恢复时会创建新的修订记录，不会覆盖历史版本。"),
    onOpenHistory: () => setHistoricalDataCenterOpen(true),
    onOpenSettings: () => settings.openSettings("ai"),
    onCheckForUpdates: () => settings.openSettings("updates"),
    onSignOut: () => void client.authLogout().then(setAuthStatus),
  };

  if (!authChecked) return <div className="app-auth-loading" aria-busy="true" />;
  if (desktopRuntime && authLoadError && !authStatus) {
    return (
      <div className="business-v1-blocking-error" role="alert">
        <strong>登录状态读取失败</strong>
        <p>{authLoadError}</p>
        <button type="button" onClick={() => window.location.reload()}>重新加载</button>
      </div>
    );
  }
  if (authStatus && !authStatus.currentUser) {
    return <AuthGate
      status={authStatus}
      busy={authBusy}
      error={authError}
      initialCredentials={rememberedAuth}
      onInitialize={(username, password, remember) => void runAuth(() => client.authInitializeAdmin({ username, password }), username, password, remember)}
      onLogin={(username, password, remember) => void runAuth(() => client.authLogin({ username, password }), username, password, remember)}
      onForgetSaved={() => void client.authForgetCredentials().then(() => setRememberedAuth(null))}
    />;
  }

  return (
    <>
      <BusinessWorkspaceShell
        productName={PRODUCT_NAME}
        projects={projects}
        conversations={conversations}
        activeProjectId={activeProjectId}
        activeConversationId={activeConversationId}
        messages={messages}
        context={workspaceContext}
        user={{
          id: authStatus?.currentUser?.username ?? "local-user",
          name: authStatus?.currentUser?.username ?? "本地用户",
          roleLabel: authStatus?.currentUser?.role === "admin" ? "管理员" : "商务",
          initials: (authStatus?.currentUser?.username ?? "半").slice(0, 1),
        }}
        composer={{
          value: draft,
          attachments,
          sourceScope,
          networkScope,
          modelId,
          isSubmitting: busy || !activeProjectId,
          placeholder: activeProjectId ? "输入任务，或直接添加合同、素材和文件夹" : "选择项目后开始任务",
        }}
        modelOptions={[{ value: "default", label: "默认模型" }]}
        actions={actions}
        isLoading={snapshot.synchronizing}
      />
      <SettingsCenter
        open={settings.open}
        initialCategory={settings.category}
        onClose={settings.closeSettings}
        authStatus={authStatus}
        onLogout={async () => {
          settings.closeSettings();
          setAuthStatus(await client.authLogout());
        }}
        onAuthChangePassword={handleAuthChangePassword}
        onAuthListUsers={() => client.authListUsers()}
        onAuthCreateUser={(username, password, role) =>
          client.authCreateUser({ username, password, role })
        }
        onAuthResetPassword={(username, newPassword) =>
          client.authResetPassword({ username, newPassword })
        }
        onAuthDeleteUser={(username) => client.authDeleteUser({ username })}
        onAuthRefreshRegistry={() =>
          client.authRefreshRegistry().then((status) => {
            setAuthStatus(status);
            return status;
          })
        }
        {...settings.centerProps}
      />
      {quotationCenterOpen && activeBusinessWorkspace ? (
        <QuotationCenter
          workspace={activeBusinessWorkspace}
          onSave={handleQuotationSave}
          onAdvanceApproval={handleQuotationApproval}
          onGenerate={handleQuotationGenerate}
          onOpenAsset={(assetId) => client.openAsset(assetId)}
          onClose={() => setQuotationCenterOpen(false)}
          disabled={busy}
        />
      ) : null}
      {acceptanceCenterOpen && activeBusinessWorkspace ? (
        <AcceptanceCenter
          workspace={activeBusinessWorkspace}
          assets={acceptanceAssetCandidates}
          onCreateBatch={createAcceptanceBatch}
          onAddMaterial={addAcceptanceMaterial}
          onPrepare={prepareAcceptanceDocuments}
          onAdvanceDocument={(documentId) => advanceBusinessDocument(documentId, true)}
          onOpenAsset={(assetId) => client.openAsset(assetId)}
          onClose={() => setAcceptanceCenterOpen(false)}
          disabled={busy}
        />
      ) : null}
      {settlementCenterOpen && annualSettlementWorkspace && activeBusinessWorkspace ? (
        <div
          className="bw-annual-settlement-backdrop"
          role="dialog"
          aria-modal="true"
          aria-label="年框结算中心"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget && !busy) setSettlementCenterOpen(false);
          }}
        >
          <AnnualSettlementCenter
            workspace={annualSettlementWorkspace}
            settlementBatches={annualSettlementBatches}
            onUpsert={handleAnnualSettlementUpsert}
            onVoid={handleAnnualSettlementVoid}
            onClose={() => setSettlementCenterOpen(false)}
            disabled={busy}
          />
        </div>
      ) : null}
      {contractLegalCenterOpen && activeProjectId ? (
        <ContractLegalCenter
          client={client}
          projectId={activeProjectId}
          workspace={activeBusinessWorkspace}
          attachmentCandidates={contractAttachmentCandidates}
          onClose={() => setContractLegalCenterOpen(false)}
          onOpenAsset={(assetId) => client.openAsset(assetId)}
        />
      ) : null}
      <SharedCaseCenter
        client={client}
        currentUser={authStatus?.currentUser ?? null}
        activeProjectId={activeProjectId}
      />
      <HistoricalDataCenter
        open={historicalDataCenterOpen}
        snapshot={snapshot}
        activeProjectId={activeProjectId}
        onClose={() => setHistoricalDataCenterOpen(false)}
        onRefresh={refreshHistoricalData}
        onOpenAsset={(assetId) => client.openAsset(assetId)}
        onRestoreThread={restoreArchivedThread}
      />
      {projectDraft ? (
        <div className="bw-dialog-backdrop" role="presentation">
          <form
            className="bw-project-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="bw-project-dialog-title"
            onSubmit={(event) => {
              event.preventDefault();
              void createProject();
            }}
          >
            <header>
              <div>
                <strong id="bw-project-dialog-title">新建项目</strong>
                <span>建立独立客户工作区</span>
              </div>
              <button
                className="bw-icon-button"
                type="button"
                title="关闭"
                aria-label="关闭"
                onClick={() => setProjectDraft(null)}
                disabled={busy}
              >
                <X size={17} />
              </button>
            </header>
            <div className="bw-project-dialog__fields">
              <label>
                <span>项目名称</span>
                <input
                  autoFocus
                  value={projectDraft.name}
                  onChange={(event) => setProjectDraft({ ...projectDraft, name: event.target.value })}
                  placeholder="例如：白鹅潭年度品牌项目"
                  disabled={busy}
                />
              </label>
              <label>
                <span>客户名称</span>
                <input
                  value={projectDraft.customerName}
                  onChange={(event) => setProjectDraft({ ...projectDraft, customerName: event.target.value })}
                  placeholder="例如：客户集团或甲方主体"
                  disabled={busy}
                />
              </label>
            </div>
            <footer>
              <button className="bw-secondary-button" type="button" onClick={() => setProjectDraft(null)} disabled={busy}>取消</button>
              <button className="bw-primary-button" type="submit" disabled={busy || !projectDraft.name.trim()}>创建项目</button>
            </footer>
          </form>
        </div>
      ) : null}
    </>
  );
}
