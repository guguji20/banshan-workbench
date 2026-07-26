import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import {
  Archive,
  Banknote,
  Check,
  CheckCircle2,
  CircleAlert,
  CircleDollarSign,
  Clock3,
  Boxes,
  ChevronRight,
  ClipboardCheck,
  Download,
  FileArchive,
  FileCheck2,
  FileClock,
  FileOutput,
  FilePenLine,
  FilePlus2,
  FileText,
  FolderKanban,
  FolderOpen,
  Hash,
  Landmark,
  LoaderCircle,
  PackageCheck,
  Paperclip,
  Plus,
  ReceiptText,
  RefreshCw,
  RotateCcw,
  Save,
  Send,
  ShieldCheck,
  Trash2,
  UserRound,
  WalletCards,
  X,
} from "lucide-react";
import type { AssetActionCapabilities } from "../client-sdk";
import type { AssetRecord } from "../generated/bsaigc/AssetRecord";
import type { AssignBusinessCustomerPayload } from "../generated/bsaigc/AssignBusinessCustomerPayload";
import type { AttachBusinessInvoiceAssetPayload } from "../generated/bsaigc/AttachBusinessInvoiceAssetPayload";
import type { BusinessArchiveSnapshotRecord } from "../generated/bsaigc/BusinessArchiveSnapshotRecord";
import type { BusinessCustomerInput } from "../generated/bsaigc/BusinessCustomerInput";
import type { BusinessCustomerReceivableSummary } from "../generated/bsaigc/BusinessCustomerReceivableSummary";
import type { BusinessDeliverableRecord } from "../generated/bsaigc/BusinessDeliverableRecord";
import type { BusinessDeliverableVersionRecord } from "../generated/bsaigc/BusinessDeliverableVersionRecord";
import type { BusinessDeliverySubmissionRecord } from "../generated/bsaigc/BusinessDeliverySubmissionRecord";
import type { BusinessDocumentFormat } from "../generated/bsaigc/BusinessDocumentFormat";
import type { BusinessDocumentKind } from "../generated/bsaigc/BusinessDocumentKind";
import type { BusinessDocumentRecord } from "../generated/bsaigc/BusinessDocumentRecord";
import type { BusinessDocumentStatus } from "../generated/bsaigc/BusinessDocumentStatus";
import type { BusinessInvoiceRecord } from "../generated/bsaigc/BusinessInvoiceRecord";
import type { BusinessLifecycleStage } from "../generated/bsaigc/BusinessLifecycleStage";
import type { BusinessLineItem } from "../generated/bsaigc/BusinessLineItem";
import type { BusinessLineItemInput } from "../generated/bsaigc/BusinessLineItemInput";
import type { BusinessMilestoneInput } from "../generated/bsaigc/BusinessMilestoneInput";
import type { BusinessMilestoneRecord } from "../generated/bsaigc/BusinessMilestoneRecord";
import type { BusinessPaymentInput } from "../generated/bsaigc/BusinessPaymentInput";
import type { BusinessPaymentRecord } from "../generated/bsaigc/BusinessPaymentRecord";
import type { BusinessPaymentStatus } from "../generated/bsaigc/BusinessPaymentStatus";
import type { BusinessProfile } from "../generated/bsaigc/BusinessProfile";
import type { BusinessProfileInput } from "../generated/bsaigc/BusinessProfileInput";
import type { BusinessReceiptRecord } from "../generated/bsaigc/BusinessReceiptRecord";
import type { BusinessWorkspaceDomainEvent } from "../generated/bsaigc/BusinessWorkspaceDomainEvent";
import type { BusinessWorkspacePrefillCandidate } from "../generated/bsaigc/BusinessWorkspacePrefillCandidate";
import type { BusinessWorkspacePrefillChange } from "../generated/bsaigc/BusinessWorkspacePrefillChange";
import type { BusinessWorkspacePrefillDecision } from "../generated/bsaigc/BusinessWorkspacePrefillDecision";
import type { BusinessWorkspacePrefillField } from "../generated/bsaigc/BusinessWorkspacePrefillField";
import type { BusinessWorkspacePrefillMatchKind } from "../generated/bsaigc/BusinessWorkspacePrefillMatchKind";
import type { BusinessWorkspacePrefillPreview } from "../generated/bsaigc/BusinessWorkspacePrefillPreview";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import type { BusinessWorkspaceStatus } from "../generated/bsaigc/BusinessWorkspaceStatus";
import type { ProjectRecord } from "../generated/bsaigc/ProjectRecord";
import type { RecordBusinessDeliverySentPayload } from "../generated/bsaigc/RecordBusinessDeliverySentPayload";
import type { RecordBusinessDeliverySignoffPayload } from "../generated/bsaigc/RecordBusinessDeliverySignoffPayload";
import type { RecordBusinessInvoiceIssuedPayload } from "../generated/bsaigc/RecordBusinessInvoiceIssuedPayload";
import type { RecordBusinessInvoiceRedCorrectionPayload } from "../generated/bsaigc/RecordBusinessInvoiceRedCorrectionPayload";
import type { RegisterBusinessDeliverableVersionPayload } from "../generated/bsaigc/RegisterBusinessDeliverableVersionPayload";
import type { RequirementBriefRecord } from "../generated/bsaigc/RequirementBriefRecord";
import type { UpsertBusinessCustomerPayload } from "../generated/bsaigc/UpsertBusinessCustomerPayload";
import type { UpsertBusinessMilestonePayload } from "../generated/bsaigc/UpsertBusinessMilestonePayload";
import "./BusinessDocumentsCenter.css";

type BusinessView = "overview" | "customer" | "documents" | "delivery" | "finance" | "archive";

interface DocumentDefinition {
  kind: BusinessDocumentKind;
  label: string;
  shortLabel: string;
  description: string;
  prefix: string;
  templateKey: string;
  format: BusinessDocumentFormat;
  icon: typeof FileText;
}

interface DocumentDraft {
  kind: BusinessDocumentKind;
  documentNumber: string;
  title: string;
  paymentId: string;
}

interface PaymentDraft {
  id: string | null;
  label: string;
  amount: string;
  dueDate: string;
  occurredDate: string;
  status: BusinessPaymentStatus;
  reference: string;
  notes: string;
}

interface QuoteConfirmationDraft {
  quoteDocumentId: string;
  confirmationVersion: string;
  customerRepresentative: string;
  occurredDate: string;
  notes: string;
}

interface ReceiptDraft {
  paymentId: string;
  amount: string;
  occurredDate: string;
  reference: string;
  notes: string;
  includeEvidence: boolean;
}

interface ReceiptReversalDraft {
  receiptId: string;
  amount: string;
  occurredDate: string;
  reference: string;
  reason: string;
}

interface DocumentTransitionDraft {
  documentId: string;
  status: BusinessDocumentStatus;
  mode: "evidence" | "waiver";
  occurredDate: string;
  evidenceNote: string;
  reason: string;
}

interface DeliverableDraft {
  milestoneId: string;
  deliverableId: string | null;
  name: string;
  required: boolean;
  assetId: string;
  notes: string;
}

interface DeliverySentDraft {
  milestoneId: string;
  versionIds: string[];
  recipient: string;
  channel: string;
  sentDate: string;
  note: string;
}

interface DeliverySignoffDraft {
  submissionId: string;
  decisions: Record<string, "accepted" | "rejected" | "pending">;
  customerRepresentative: string;
  evidenceAssetId: string;
  evidenceNote: string;
  occurredDate: string;
  note: string;
}

interface InvoiceDraft {
  paymentId: string;
  invoiceCode: string;
  invoiceNumber: string;
  amount: string;
  tax: string;
  issuedDate: string;
  assetIds: string;
}

interface InvoiceReversalDraft {
  originalInvoiceId: string;
  invoiceCode: string;
  invoiceNumber: string;
  amount: string;
  tax: string;
  issuedDate: string;
  reason: string;
  assetIds: string;
}

interface InvoiceAttachmentDraft {
  invoiceId: string;
  assetId: string;
  role: string;
}

interface ArchivePreflightItem {
  id: string;
  label: string;
  detail: string;
  passed: boolean;
}

export interface BusinessDocumentsCenterActions {
  onCreateBusinessWorkspace: (
    projectId: string,
    prefillSourceWorkspaceId?: string,
  ) => Promise<boolean>;
  onListBusinessWorkspacePrefillCandidates: (
    projectId: string,
  ) => Promise<readonly BusinessWorkspacePrefillCandidate[]>;
  onPreviewBusinessWorkspacePrefill: (
    projectId: string,
    sourceWorkspaceId: string,
  ) => Promise<BusinessWorkspacePrefillPreview>;
  onRefreshBusinessWorkspaces: () => Promise<boolean>;
  onUpdateBusinessProfile: (
    workspaceId: string,
    profile: BusinessProfileInput,
  ) => Promise<boolean>;
  onCreateBusinessDocument: (
    workspaceId: string,
    draft: {
      kind: BusinessDocumentKind;
      documentNumber: string;
      title: string;
      templateKey: string;
      paymentId: string | null;
    },
  ) => Promise<boolean>;
  onChangeBusinessDocumentStatus: (
    workspaceId: string,
    documentId: string,
    status: BusinessDocumentStatus,
    input: {
      reason: string;
      attachEvidence: boolean;
      evidenceOccurredAt: number | null;
      evidenceNote: string;
      manualWaiverReason: string | null;
    },
  ) => Promise<boolean>;
  onGenerateBusinessDocument: (
    workspaceId: string,
    documentId: string,
    format: BusinessDocumentFormat,
  ) => Promise<boolean>;
  onUpsertBusinessPayment: (
    workspaceId: string,
    payment: BusinessPaymentInput,
  ) => Promise<boolean>;
  onConfirmBusinessQuote: (
    workspaceId: string,
    confirmation: {
      quoteDocumentId: string;
      confirmationVersion: string;
      customerRepresentative: string;
      occurredAt: number;
      notes: string;
    },
  ) => Promise<boolean>;
  onRecordBusinessReceipt: (
    workspaceId: string,
    receipt: {
      paymentId: string;
      amountCents: number;
      occurredAt: number;
      reference: string;
      notes: string;
      includeEvidence: boolean;
    },
  ) => Promise<boolean>;
  onReverseBusinessReceipt: (
    workspaceId: string,
    reversal: {
      receiptId: string;
      amountCents: number;
      occurredAt: number;
      reference: string;
      reason: string;
    },
  ) => Promise<boolean>;
  onAdoptLatestConfirmedRequirement: (workspaceId: string) => Promise<boolean>;
  onChangeBusinessWorkspaceStatus: (
    workspaceId: string,
    status: BusinessWorkspaceStatus,
  ) => Promise<boolean>;
  onUpsertBusinessCustomer?: (payload: UpsertBusinessCustomerPayload) => Promise<boolean>;
  onAssignBusinessCustomer?: (payload: AssignBusinessCustomerPayload) => Promise<boolean>;
  onUpsertBusinessMilestone?: (payload: UpsertBusinessMilestonePayload) => Promise<boolean>;
  onRegisterBusinessDeliverableVersion?: (
    payload: RegisterBusinessDeliverableVersionPayload,
  ) => Promise<boolean>;
  onRecordBusinessDeliverySent?: (payload: RecordBusinessDeliverySentPayload) => Promise<boolean>;
  onRecordBusinessDeliverySignoff?: (
    payload: RecordBusinessDeliverySignoffPayload,
  ) => Promise<boolean>;
  onRecordBusinessInvoiceIssued?: (
    payload: RecordBusinessInvoiceIssuedPayload,
  ) => Promise<boolean>;
  onRecordBusinessInvoiceRedCorrection?: (
    payload: RecordBusinessInvoiceRedCorrectionPayload,
  ) => Promise<boolean>;
  onAttachBusinessInvoiceAsset?: (
    payload: AttachBusinessInvoiceAssetPayload,
  ) => Promise<boolean>;
  onCreateBusinessArchiveSnapshot?: (workspaceId: string) => Promise<boolean>;
  onImportBusinessAsset?: (workspaceId: string) => Promise<AssetRecord | null>;
}

export interface QuoteHistorySource {
  workspaceId: string;
  projectTitle: string;
  customerName: string;
  updatedAt: number;
  lineItems: readonly BusinessLineItem[];
}

export interface BusinessDocumentsCenterProps
  extends BusinessDocumentsCenterActions {
  projects: readonly ProjectRecord[];
  selectedProjectId: string | null;
  workspace: BusinessWorkspaceRecord | null;
  quoteHistorySources: readonly QuoteHistorySource[];
  latestConfirmedRequirement: RequirementBriefRecord | null;
  workspaceEvents: readonly BusinessWorkspaceDomainEvent[];
  assets: readonly AssetRecord[];
  businessCustomers: readonly BusinessCustomerReceivableSummary[];
  busyAction: string | null;
  error: string | null;
  isDesktopRuntime: boolean;
  assetActionCapabilities: Readonly<Record<string, AssetActionCapabilities>>;
  onSelectProject: (projectId: string) => void;
  onOpenAsset: (assetId: string) => void;
  onExportAsset: (assetId: string) => void;
  onDismissError?: () => void;
}

export interface BusinessWorkspaceSummary {
  quotedCents: number;
  contractCents: number;
  plannedCents: number;
  requestedCents: number;
  receivedCents: number;
  outstandingCents: number;
  generatedDocuments: number;
}

const DOCUMENT_DEFINITIONS: readonly DocumentDefinition[] = [
  { kind: "quote", label: "报价单", shortLabel: "报价", description: "服务明细、数量、税率与项目总价", prefix: "Q", templateKey: "builtin.quote.standard.v1", format: "xlsx", icon: ReceiptText },
  { kind: "contract", label: "服务合同", shortLabel: "合同", description: "交付、付款、验收与服务周期", prefix: "C", templateKey: "builtin.contract.service.v1", format: "docx", icon: FileText },
  { kind: "paymentRequest", label: "请款单", shortLabel: "请款", description: "关联付款节点、收款账户与请款金额", prefix: "PR", templateKey: "builtin.payment-request.standard.v1", format: "docx", icon: Banknote },
  { kind: "acceptance", label: "验收单", shortLabel: "验收", description: "交付内容、验收标准与确认记录", prefix: "A", templateKey: "builtin.acceptance.standard.v1", format: "docx", icon: FileCheck2 },
] as const;

const BUSINESS_VIEWS: ReadonlyArray<{ id: BusinessView; label: string; icon: typeof FileText }> = [
  { id: "overview", label: "总览", icon: WalletCards },
  { id: "customer", label: "客户", icon: UserRound },
  { id: "documents", label: "单据", icon: ReceiptText },
  { id: "delivery", label: "交付", icon: PackageCheck },
  { id: "finance", label: "财务", icon: CircleDollarSign },
  { id: "archive", label: "归档", icon: FileArchive },
] as const;

const LIFECYCLE_STAGES: ReadonlyArray<{ id: BusinessLifecycleStage; label: string }> = [
  { id: "draft", label: "资料" },
  { id: "quoted", label: "报价" },
  { id: "contracted", label: "合同" },
  { id: "paymentRequested", label: "请款" },
  { id: "accepted", label: "验收" },
  { id: "paid", label: "到账" },
  { id: "archived", label: "归档" },
] as const;

const DOCUMENT_STATUS_LABELS: Record<BusinessDocumentStatus, string> = {
  draft: "草稿", inReview: "待审批", approved: "已批准", generated: "已生成", effective: "已生效", voided: "已作废",
};

const PAYMENT_STATUS_LABELS: Record<BusinessPaymentStatus, string> = {
  planned: "付款计划",
  requested: "已请款",
  partiallyReceived: "部分到账",
  received: "已到账",
  canceled: "已取消",
};

const MILESTONE_STATUS_OPTIONS: ReadonlyArray<{
  value: BusinessMilestoneInput["status"];
  label: string;
}> = [
  { value: "planned", label: "计划中" },
  { value: "inProgress", label: "进行中" },
  { value: "canceled", label: "已取消" },
];

const MILESTONE_STATUS_LABELS: Record<BusinessMilestoneRecord["status"], string> = {
  planned: "计划中",
  inProgress: "进行中",
  delivered: "已交付",
  accepted: "已签收",
  canceled: "已取消",
};

const DELIVERABLE_STATUS_LABELS: Record<BusinessDeliverableVersionRecord["status"], string> = {
  draft: "待发送",
  sent: "已发送",
  accepted: "已接受",
  rejected: "已拒绝",
  superseded: "已被替代",
};

const DELIVERY_STATUS_LABELS: Record<BusinessDeliverySubmissionRecord["status"], string> = {
  sent: "待签收",
  partiallySigned: "部分签收",
  accepted: "已签收",
  rejected: "已拒绝",
};

const INVOICE_STATUS_LABELS: Record<BusinessInvoiceRecord["status"], string> = {
  issued: "已开票",
  partiallyReversed: "部分红冲",
  fullyReversed: "已全额红冲",
};

export const PREFILL_FIELD_LABELS: Record<BusinessWorkspacePrefillField, string> = {
  customerLegalName: "客户公司全称",
  customerTaxId: "客户税号",
  customerAddress: "客户地址",
  customerContact: "客户联系人",
  customerPhone: "客户电话",
  customerEmail: "客户邮箱",
  supplierLegalName: "我方公司全称",
  supplierTaxId: "我方税号",
  supplierAddress: "我方地址",
  supplierContact: "我方联系人",
  supplierPhone: "我方电话",
  supplierBankName: "收款银行",
  supplierBankAccount: "银行账号",
  currency: "币种",
  defaultTaxRateBps: "默认税率",
};

export const PREFILL_MATCH_KIND_LABELS: Record<BusinessWorkspacePrefillMatchKind, string> = {
  customerName: "客户名相同",
  customerLegalName: "公司全称相同",
  both: "客户名与公司全称相同",
};

const PREFILL_DECISION_LABELS: Record<BusinessWorkspacePrefillDecision, string> = {
  filled: "带入",
  unchanged: "保持",
  replaced: "覆盖",
  cleared: "清空",
};

export function formatPrefillValue(
  field: BusinessWorkspacePrefillField,
  value: string,
): string {
  if (!value) return "—";
  if (field === "defaultTaxRateBps") {
    const bps = Number(value);
    if (Number.isFinite(bps) && bps > 0) {
      const percent = bps / 100;
      return Number.isInteger(percent) ? `${percent}%` : `${percent.toFixed(2)}%`;
    }
  }
  return value;
}

export interface QuoteTemplateItem {
  name: string;
  description: string;
  quantityMillis: number;
  unit: string;
}

export interface QuoteTemplate {
  id: string;
  label: string;
  items: readonly QuoteTemplateItem[];
}

export const QUOTE_TEMPLATES: readonly QuoteTemplate[] = [
  {
    id: "single-video",
    label: "单条视频",
    items: [
      { name: "视频策划与脚本", description: "创意方向、脚本与分镜", quantityMillis: 1000, unit: "项" },
      { name: "拍摄执行", description: "含摄影摄像、灯光与现场执行", quantityMillis: 1000, unit: "天" },
      { name: "后期剪辑与调色", description: "粗剪、精剪与调色", quantityMillis: 1000, unit: "项" },
      { name: "成片交付", description: "含 2 轮修改与多平台规格输出", quantityMillis: 1000, unit: "项" },
    ],
  },
  {
    id: "itemized",
    label: "细分拆项",
    items: [
      { name: "前期策划", description: "脚本、分镜、勘景", quantityMillis: 1000, unit: "项" },
      { name: "导演/编导", description: "", quantityMillis: 1000, unit: "天" },
      { name: "摄影摄像", description: "含机位与灯光", quantityMillis: 1000, unit: "天" },
      { name: "演员/模特", description: "", quantityMillis: 1000, unit: "人" },
      { name: "场地与美术道具", description: "", quantityMillis: 1000, unit: "项" },
      { name: "后期剪辑", description: "", quantityMillis: 1000, unit: "项" },
      { name: "调色", description: "", quantityMillis: 1000, unit: "项" },
      { name: "动画包装/字幕", description: "", quantityMillis: 1000, unit: "项" },
      { name: "配音配乐与版权", description: "", quantityMillis: 1000, unit: "项" },
      { name: "成片输出与交付", description: "多平台规格", quantityMillis: 1000, unit: "项" },
    ],
  },
  {
    id: "annual-frame",
    label: "年框/框架服务",
    items: [
      { name: "年度框架服务费", description: "全年商务与制作统筹", quantityMillis: 1000, unit: "年" },
      { name: "单条视频制作（框架单价）", description: "按框架单价×预估条数", quantityMillis: 12000, unit: "条" },
      { name: "平面拍摄（框架单价）", description: "", quantityMillis: 4000, unit: "次" },
      { name: "加急与额外修改预留", description: "超出约定轮次时按此计费", quantityMillis: 1000, unit: "项" },
    ],
  },
  {
    id: "monthly-service",
    label: "月度服务",
    items: [
      { name: "月度内容策划", description: "选题与脚本", quantityMillis: 1000, unit: "月" },
      { name: "月度拍摄", description: "约定每月拍摄次数", quantityMillis: 4000, unit: "次" },
      { name: "月度剪辑产出", description: "约定每月成片条数", quantityMillis: 8000, unit: "条" },
      { name: "月度运营对接", description: "复盘与排期沟通", quantityMillis: 1000, unit: "月" },
    ],
  },
];

export function isBlankLineItem(item: BusinessLineItemInput): boolean {
  return !item.name.trim() && !item.description.trim() && item.unitPriceCents === 0;
}

export function applyQuoteTemplateItems(
  current: readonly BusinessLineItemInput[],
  additions: readonly QuoteTemplateItem[],
  taxRateBps: number,
): BusinessLineItemInput[] {
  const kept = current.filter((item) => !isBlankLineItem(item));
  const created = additions.map<BusinessLineItemInput>((item) => ({
    id: null,
    name: item.name,
    description: item.description,
    quantityMillis: item.quantityMillis,
    unit: item.unit,
    unitPriceCents: 0,
    taxRateBps,
  }));
  return [...kept, ...created];
}

export function applyHistoryLineItems(
  current: readonly BusinessLineItemInput[],
  history: readonly BusinessLineItem[],
): BusinessLineItemInput[] {
  const kept = current.filter((item) => !isBlankLineItem(item));
  const copied = history.map<BusinessLineItemInput>((item) => ({
    id: null,
    name: item.name,
    description: item.description,
    quantityMillis: item.quantityMillis,
    unit: item.unit,
    unitPriceCents: item.unitPriceCents,
    taxRateBps: item.taxRateBps,
  }));
  return [...kept, ...copied];
}

export function summarizePrefillChanges(
  changes: readonly BusinessWorkspacePrefillChange[],
): { filled: number; kept: number } {
  let filled = 0;
  let kept = 0;
  for (const change of changes) {
    if (change.decision === "filled" || change.decision === "replaced") {
      filled += 1;
    } else {
      kept += 1;
    }
  }
  return { filled, kept };
}

export function BusinessDocumentsCenter({
  projects,
  selectedProjectId,
  workspace,
  quoteHistorySources,
  latestConfirmedRequirement,
  workspaceEvents,
  assets,
  businessCustomers,
  busyAction,
  error,
  isDesktopRuntime,
  onCreateBusinessWorkspace,
  onListBusinessWorkspacePrefillCandidates,
  onPreviewBusinessWorkspacePrefill,
  onRefreshBusinessWorkspaces,
  onUpdateBusinessProfile,
  onCreateBusinessDocument,
  onChangeBusinessDocumentStatus,
  onGenerateBusinessDocument,
  onUpsertBusinessPayment,
  onConfirmBusinessQuote,
  onRecordBusinessReceipt,
  onReverseBusinessReceipt,
  onAdoptLatestConfirmedRequirement,
  onChangeBusinessWorkspaceStatus,
  onUpsertBusinessCustomer,
  onAssignBusinessCustomer,
  onUpsertBusinessMilestone,
  onRegisterBusinessDeliverableVersion,
  onRecordBusinessDeliverySent,
  onRecordBusinessDeliverySignoff,
  onRecordBusinessInvoiceIssued,
  onRecordBusinessInvoiceRedCorrection,
  onAttachBusinessInvoiceAsset,
  onCreateBusinessArchiveSnapshot,
  onImportBusinessAsset,
  onDismissError,
  assetActionCapabilities,
  onOpenAsset,
  onExportAsset,
}: BusinessDocumentsCenterProps) {
  const [view, setView] = useState<BusinessView>("overview");
  const [profileDraft, setProfileDraft] = useState<BusinessProfileInput | null>(
    workspace ? businessProfileToInput(workspace.profile) : null,
  );
  const [customerDraft, setCustomerDraft] = useState<BusinessCustomerInput | null>(
    workspace ? businessCustomerToInput(workspace) : null,
  );
  const [documentDraft, setDocumentDraft] = useState<DocumentDraft | null>(null);
  const [paymentDraft, setPaymentDraft] = useState<PaymentDraft | null>(null);
  const [quoteConfirmationDraft, setQuoteConfirmationDraft] = useState<QuoteConfirmationDraft | null>(null);
  const [receiptDraft, setReceiptDraft] = useState<ReceiptDraft | null>(null);
  const [receiptReversalDraft, setReceiptReversalDraft] = useState<ReceiptReversalDraft | null>(null);
  const [documentTransitionDraft, setDocumentTransitionDraft] = useState<DocumentTransitionDraft | null>(null);
  const [milestoneDraft, setMilestoneDraft] = useState<BusinessMilestoneInput | null>(null);
  const [deliverableDraft, setDeliverableDraft] = useState<DeliverableDraft | null>(null);
  const [deliverySentDraft, setDeliverySentDraft] = useState<DeliverySentDraft | null>(null);
  const [deliverySignoffDraft, setDeliverySignoffDraft] = useState<DeliverySignoffDraft | null>(null);
  const [invoiceDraft, setInvoiceDraft] = useState<InvoiceDraft | null>(null);
  const [invoiceReversalDraft, setInvoiceReversalDraft] = useState<InvoiceReversalDraft | null>(null);
  const [invoiceAttachmentDraft, setInvoiceAttachmentDraft] = useState<InvoiceAttachmentDraft | null>(null);

  const selectedProject = projects.find((project) => project.id === selectedProjectId) ?? null;
  const busy = busyAction !== null;
  const readOnly = workspace?.status === "archived";

  const draftBaselineRef = useRef<{
    workspaceId: string | null;
    profile: string;
    customer: string;
  }>({ workspaceId: null, profile: "", customer: "" });

  useEffect(() => {
    // Resync drafts whenever the authoritative workspace revision moves
    // (profile adopt/sync, customer rebind, remote refresh), not only on
    // workspace switch — otherwise stale drafts overwrite adopted data.
    // In-progress edits are preserved: a draft is only reseeded when it is
    // still pristine relative to the last seeded baseline.
    if (!workspace) {
      draftBaselineRef.current = { workspaceId: null, profile: "", customer: "" };
      setProfileDraft(null);
      setCustomerDraft(null);
      return;
    }
    const nextProfile = businessProfileToInput(workspace.profile);
    const nextCustomer = businessCustomerToInput(workspace);
    const nextProfileJson = JSON.stringify(nextProfile);
    const nextCustomerJson = JSON.stringify(nextCustomer);
    const workspaceChanged = draftBaselineRef.current.workspaceId !== workspace.id;
    setProfileDraft((current) => {
      const pristine =
        current === null ||
        JSON.stringify(current) === draftBaselineRef.current.profile;
      return workspaceChanged || pristine ? nextProfile : current;
    });
    setCustomerDraft((current) => {
      const pristine =
        current === null ||
        JSON.stringify(current) === draftBaselineRef.current.customer;
      return workspaceChanged || pristine ? nextCustomer : current;
    });
    draftBaselineRef.current = {
      workspaceId: workspace.id,
      profile: nextProfileJson,
      customer: nextCustomerJson,
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspace?.id, workspace?.revision]);

  useEffect(() => {
    setDocumentDraft(null);
    setPaymentDraft(null);
    setQuoteConfirmationDraft(null);
    setReceiptDraft(null);
    setReceiptReversalDraft(null);
    setDocumentTransitionDraft(null);
    setMilestoneDraft(null);
    setDeliverableDraft(null);
    setDeliverySentDraft(null);
    setDeliverySignoffDraft(null);
    setInvoiceDraft(null);
    setInvoiceReversalDraft(null);
    setInvoiceAttachmentDraft(null);
    setView("overview");
  }, [selectedProjectId]);

  const summary = useMemo(
    () => (workspace ? summarizeBusinessWorkspace(workspace) : null),
    [workspace],
  );
  const visibleEvents = useMemo(
    () => workspace
      ? workspaceEvents
          .filter((event) => event.aggregateId === workspace.id)
          .sort((left, right) => right.sequence - left.sequence)
          .slice(0, 10)
      : [],
    [workspace, workspaceEvents],
  );
  const archiveChecks = useMemo(
    () => (workspace ? buildArchivePreflight(workspace) : []),
    [workspace],
  );
  const archiveBlockReason = workspace ? archiveWorkspaceBlockReason(workspace) : "工作区尚未加载";

  const startDocument = (kind: BusinessDocumentKind) => {
    if (!workspace || !selectedProject || readOnly || documentCreationBlockReason(workspace, kind)) return;
    setView("documents");
    setQuoteConfirmationDraft(null);
    setDocumentDraft((current) => {
      const next = defaultDocumentDraft(kind, workspace, selectedProject);
      if (!current || current.kind === kind) return next;
      // 切换单据类型时保留用户已修改的标题；编号按新类型前缀重新生成。
      const previousDefault = defaultDocumentDraft(
        current.kind,
        workspace,
        selectedProject,
      );
      return {
        ...next,
        title:
          current.title.trim() && current.title !== previousDefault.title
            ? current.title
            : next.title,
      };
    });
  };

  const submitProfile = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !profileDraft || busy || readOnly) return;
    await onUpdateBusinessProfile(workspace.id, profileDraft);
  };

  const submitCustomer = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !customerDraft || !onUpsertBusinessCustomer || busy || readOnly) return;
    await onUpsertBusinessCustomer({
      workspaceId: workspace.id,
      customerId: workspace.customerId || null,
      customer: customerDraft,
    });
  };

  const submitDocument = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !documentDraft || busy || readOnly || documentCreationBlockReason(workspace, documentDraft.kind)) return;
    const definition = documentDefinition(documentDraft.kind);
    const succeeded = await onCreateBusinessDocument(workspace.id, {
      kind: documentDraft.kind,
      documentNumber: documentDraft.documentNumber.trim(),
      title: documentDraft.title.trim(),
      templateKey: definition.templateKey,
      paymentId: documentDraft.kind === "paymentRequest" ? documentDraft.paymentId || null : null,
    });
    if (succeeded) setDocumentDraft(null);
  };

  const submitPayment = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !paymentDraft || busy || readOnly) return;
    if (paymentDraft.status === "received" && (!paymentDraft.occurredDate || !paymentDraft.reference.trim())) return;
    const succeeded = await onUpsertBusinessPayment(workspace.id, paymentDraftToInput(paymentDraft));
    if (succeeded) setPaymentDraft(null);
  };

  const submitQuoteConfirmation = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !quoteConfirmationDraft || busy || readOnly) return;
    const occurredAt = dateInputToTimestamp(quoteConfirmationDraft.occurredDate);
    if (!occurredAt || !quoteConfirmationDraft.confirmationVersion.trim() || !quoteConfirmationDraft.customerRepresentative.trim()) return;
    const succeeded = await onConfirmBusinessQuote(workspace.id, {
      quoteDocumentId: quoteConfirmationDraft.quoteDocumentId,
      confirmationVersion: quoteConfirmationDraft.confirmationVersion.trim(),
      customerRepresentative: quoteConfirmationDraft.customerRepresentative.trim(),
      occurredAt,
      notes: quoteConfirmationDraft.notes.trim(),
    });
    if (succeeded) setQuoteConfirmationDraft(null);
  };

  const submitReceipt = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !receiptDraft || busy || readOnly) return;
    const occurredAt = dateInputToTimestamp(receiptDraft.occurredDate);
    const amountCents = decimalToCents(receiptDraft.amount);
    if (!occurredAt || amountCents <= 0 || !receiptDraft.reference.trim()) return;
    const succeeded = await onRecordBusinessReceipt(workspace.id, {
      paymentId: receiptDraft.paymentId,
      amountCents,
      occurredAt,
      reference: receiptDraft.reference.trim(),
      notes: receiptDraft.notes.trim(),
      includeEvidence: receiptDraft.includeEvidence,
    });
    if (succeeded) setReceiptDraft(null);
  };

  const submitReceiptReversal = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !receiptReversalDraft || busy || readOnly) return;
    const occurredAt = dateInputToTimestamp(receiptReversalDraft.occurredDate);
    const amountCents = decimalToCents(receiptReversalDraft.amount);
    if (!occurredAt || amountCents <= 0 || !receiptReversalDraft.reference.trim() || !receiptReversalDraft.reason.trim()) return;
    if (!window.confirm("确认冲销这笔到账记录？系统会保留原始流水并新增冲销记录。")) return;
    const succeeded = await onReverseBusinessReceipt(workspace.id, {
      receiptId: receiptReversalDraft.receiptId,
      amountCents,
      occurredAt,
      reference: receiptReversalDraft.reference.trim(),
      reason: receiptReversalDraft.reason.trim(),
    });
    if (succeeded) setReceiptReversalDraft(null);
  };

  const changeDocumentStatus = async (document: BusinessDocumentRecord, status: BusinessDocumentStatus) => {
    if (!workspace || busy || readOnly || documentTransitionBlockReason(workspace, document, status)) return;
    if (status === "effective" || status === "voided") {
      setDocumentTransitionDraft(defaultDocumentTransitionDraft(document, status));
      return;
    }
    await onChangeBusinessDocumentStatus(workspace.id, document.id, status, emptyDocumentTransitionInput());
  };

  const submitDocumentTransition = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !documentTransitionDraft || busy || readOnly) return;
    const document = workspace.documents.find((candidate) => candidate.id === documentTransitionDraft.documentId);
    if (!document) return;
    const effective = documentTransitionDraft.status === "effective";
    const attachEvidence = effective && documentTransitionDraft.mode === "evidence";
    const occurredAt = attachEvidence ? dateInputToTimestamp(documentTransitionDraft.occurredDate) : null;
    const manualWaiverReason = effective && documentTransitionDraft.mode === "waiver"
      ? documentTransitionDraft.reason.trim()
      : null;
    if (
      (documentTransitionDraft.status === "voided" && !documentTransitionDraft.reason.trim()) ||
      (attachEvidence && !occurredAt) ||
      (documentTransitionDraft.mode === "waiver" && !manualWaiverReason)
    ) return;
    if (
      documentTransitionDraft.status === "voided" &&
      !window.confirm("确认作废“" + document.title + "”？原记录会保留且不能恢复。")
    ) return;
    const succeeded = await onChangeBusinessDocumentStatus(
      workspace.id,
      document.id,
      documentTransitionDraft.status,
      {
        reason: documentTransitionDraft.reason.trim(),
        attachEvidence,
        evidenceOccurredAt: occurredAt,
        evidenceNote: documentTransitionDraft.evidenceNote.trim(),
        manualWaiverReason,
      },
    );
    if (succeeded) setDocumentTransitionDraft(null);
  };

  const advancePayment = async (payment: BusinessPaymentRecord, status: BusinessPaymentStatus) => {
    if (!workspace || busy || readOnly) return;
    if (status === "canceled" && !window.confirm("确认取消“" + payment.label + "”？取消后不能恢复。")) return;
    await onUpsertBusinessPayment(workspace.id, {
      id: payment.id,
      label: payment.label,
      amountCents: payment.amountCents,
      dueAt: payment.dueAt,
      occurredAt: null,
      status,
      reference: payment.reference,
      notes: payment.notes,
    });
  };

  const adoptLatestRequirement = async () => {
    if (!workspace || busy || readOnly) return;
    const reason = requirementAdoptionBlockReason(workspace, latestConfirmedRequirement);
    if (reason) return;
    if (!window.confirm("采用最新已确认需求并更新项目资料？现有草稿单据不会自动重写。")) return;
    await onAdoptLatestConfirmedRequirement(workspace.id);
  };

  const submitMilestone = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !milestoneDraft || !onUpsertBusinessMilestone || busy || readOnly) return;
    const succeeded = await onUpsertBusinessMilestone({
      workspaceId: workspace.id,
      milestone: {
        ...milestoneDraft,
        title: milestoneDraft.title.trim(),
        description: milestoneDraft.description.trim(),
        acceptanceCriteria: milestoneDraft.acceptanceCriteria.trim(),
      },
    });
    if (succeeded) setMilestoneDraft(null);
  };

  const submitDeliverable = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !deliverableDraft || !onRegisterBusinessDeliverableVersion || busy || readOnly) return;
    if (!deliverableDraft.milestoneId || !deliverableDraft.name.trim() || !deliverableDraft.assetId.trim()) return;
    const succeeded = await onRegisterBusinessDeliverableVersion({
      workspaceId: workspace.id,
      milestoneId: deliverableDraft.milestoneId,
      deliverableId: deliverableDraft.deliverableId,
      name: deliverableDraft.name.trim(),
      required: deliverableDraft.required,
      assetId: deliverableDraft.assetId.trim(),
      notes: deliverableDraft.notes.trim(),
    });
    if (succeeded) setDeliverableDraft(null);
  };

  const submitDeliverySent = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !deliverySentDraft || !onRecordBusinessDeliverySent || busy || readOnly) return;
    const sentAt = dateInputToTimestamp(deliverySentDraft.sentDate);
    if (!sentAt || deliverySentDraft.versionIds.length === 0 || !deliverySentDraft.recipient.trim() || !deliverySentDraft.channel.trim()) return;
    const succeeded = await onRecordBusinessDeliverySent({
      workspaceId: workspace.id,
      milestoneId: deliverySentDraft.milestoneId,
      versionIds: deliverySentDraft.versionIds,
      recipient: deliverySentDraft.recipient.trim(),
      channel: deliverySentDraft.channel.trim(),
      sentAt,
      note: deliverySentDraft.note.trim(),
    });
    if (succeeded) setDeliverySentDraft(null);
  };

  const submitDeliverySignoff = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !deliverySignoffDraft || !onRecordBusinessDeliverySignoff || busy || readOnly) return;
    const occurredAt = dateInputToTimestamp(deliverySignoffDraft.occurredDate);
    const acceptedVersionIds = Object.entries(deliverySignoffDraft.decisions)
      .filter(([, decision]) => decision === "accepted")
      .map(([versionId]) => versionId);
    const rejectedVersionIds = Object.entries(deliverySignoffDraft.decisions)
      .filter(([, decision]) => decision === "rejected")
      .map(([versionId]) => versionId);
    if (!occurredAt || acceptedVersionIds.length + rejectedVersionIds.length === 0 || !deliverySignoffDraft.customerRepresentative.trim()) return;
    const succeeded = await onRecordBusinessDeliverySignoff({
      workspaceId: workspace.id,
      submissionId: deliverySignoffDraft.submissionId,
      acceptedVersionIds,
      rejectedVersionIds,
      customerRepresentative: deliverySignoffDraft.customerRepresentative.trim(),
      evidence: deliverySignoffDraft.evidenceAssetId.trim()
        ? {
            assetId: deliverySignoffDraft.evidenceAssetId.trim(),
            occurredAt,
            note: deliverySignoffDraft.evidenceNote.trim(),
          }
        : null,
      note: deliverySignoffDraft.note.trim(),
      occurredAt,
    });
    if (succeeded) setDeliverySignoffDraft(null);
  };

  const submitInvoice = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !invoiceDraft || !onRecordBusinessInvoiceIssued || busy || readOnly) return;
    const issuedAt = dateInputToTimestamp(invoiceDraft.issuedDate);
    const assetIds = splitAssetIds(invoiceDraft.assetIds);
    const amountCents = decimalToCents(invoiceDraft.amount);
    const taxCents = decimalToCents(invoiceDraft.tax);
    if (!issuedAt || amountCents <= 0 || !invoiceDraft.invoiceNumber.trim() || assetIds.length === 0) return;
    const succeeded = await onRecordBusinessInvoiceIssued({
      workspaceId: workspace.id,
      paymentId: invoiceDraft.paymentId || null,
      invoiceCode: invoiceDraft.invoiceCode.trim(),
      invoiceNumber: invoiceDraft.invoiceNumber.trim(),
      amountCents,
      taxCents,
      issuedAt,
      assetIds,
    });
    if (succeeded) setInvoiceDraft(null);
  };

  const submitInvoiceReversal = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !invoiceReversalDraft || !onRecordBusinessInvoiceRedCorrection || busy || readOnly) return;
    const issuedAt = dateInputToTimestamp(invoiceReversalDraft.issuedDate);
    const assetIds = splitAssetIds(invoiceReversalDraft.assetIds);
    const amountCents = decimalToCents(invoiceReversalDraft.amount);
    const taxCents = decimalToCents(invoiceReversalDraft.tax);
    if (!issuedAt || amountCents <= 0 || !invoiceReversalDraft.invoiceNumber.trim() || !invoiceReversalDraft.reason.trim() || assetIds.length === 0) return;
    if (!window.confirm("确认登记红冲？原发票不会修改，系统将新增一条反向记录。")) return;
    const succeeded = await onRecordBusinessInvoiceRedCorrection({
      workspaceId: workspace.id,
      originalInvoiceId: invoiceReversalDraft.originalInvoiceId,
      invoiceCode: invoiceReversalDraft.invoiceCode.trim(),
      invoiceNumber: invoiceReversalDraft.invoiceNumber.trim(),
      amountCents,
      taxCents,
      issuedAt,
      reason: invoiceReversalDraft.reason.trim(),
      assetIds,
    });
    if (succeeded) setInvoiceReversalDraft(null);
  };

  const submitInvoiceAttachment = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!workspace || !invoiceAttachmentDraft || !onAttachBusinessInvoiceAsset || busy || readOnly) return;
    if (!invoiceAttachmentDraft.assetId.trim() || !invoiceAttachmentDraft.role.trim()) return;
    const succeeded = await onAttachBusinessInvoiceAsset({
      workspaceId: workspace.id,
      invoiceId: invoiceAttachmentDraft.invoiceId,
      assetId: invoiceAttachmentDraft.assetId.trim(),
      role: invoiceAttachmentDraft.role.trim(),
    });
    if (succeeded) setInvoiceAttachmentDraft(null);
  };

  const createArchiveSnapshot = async () => {
    if (!workspace || !onCreateBusinessArchiveSnapshot || busy || readOnly) return;
    if (archiveChecks.some((item) => !item.passed)) return;
    await onCreateBusinessArchiveSnapshot(workspace.id);
  };

  const changeWorkspaceStatus = async (status: BusinessWorkspaceStatus) => {
    if (!workspace || busy) return;
    if (status === "archived") {
      if (archiveBlockReason) return;
      if (!window.confirm("确认归档该商务工作区？归档后只读，原始快照和流水不会修改。")) return;
    }
    const succeeded = await onChangeBusinessWorkspaceStatus(workspace.id, status);
    if (succeeded) {
      setDocumentDraft(null);
      setPaymentDraft(null);
      setView(status === "archived" ? "archive" : "overview");
    }
  };

  return (
    <section className="business-documents-center business-documents-center--six">
      <header className="bdc-topbar">
        <div className="bdc-topbar__identity">
          <span className="bdc-topbar__mark"><WalletCards size={16} /></span>
          <div>
            <strong>{selectedProject?.name ?? "商务工作台"}</strong>
            <small>
              {workspace
                ? (workspace.customer.displayName || workspace.profile.customerName || "未命名客户")
                : (selectedProject?.clientName || "选择项目后开始")}
            </small>
          </div>
        </div>
        <div className="bdc-topbar__actions">
          {workspace && <span className={workspace.status === "archived" ? "is-archived" : "is-active"}>{workspace.status === "archived" ? "已归档" : "进行中"}</span>}
          <button
            type="button"
            className="bdc-icon-button"
            onClick={() => void onRefreshBusinessWorkspaces()}
            disabled={!isDesktopRuntime || busy}
            title="刷新"
            aria-label="刷新商务资料"
          >
            {busyAction === "business:refresh"
              ? <LoaderCircle className="business-documents-center__spin" size={14} />
              : <RefreshCw size={14} />}
          </button>
        </div>
      </header>

      {error && (
        <div className="business-documents-center__error bdc-error" role="alert">
          <CircleAlert size={15} />
          <span>{error}</span>
          {onDismissError && <button type="button" onClick={onDismissError} aria-label="关闭错误"><X size={14} /></button>}
        </div>
      )}

      {workspace && (
        <nav className="business-documents-center__tabs bdc-tabs" aria-label="商务工作台视图">
          {BUSINESS_VIEWS.map((item) => {
            const Icon = item.icon;
            return (
              <button
                type="button"
                key={item.id}
                className={view === item.id ? "is-active" : ""}
                onClick={() => setView(item.id)}
                aria-current={view === item.id ? "page" : undefined}
              >
                <Icon size={14} />
                {item.label}
              </button>
            );
          })}
        </nav>
      )}

      <div className="bdc-body">
        {!selectedProject ? (
          <CenterEmpty icon={<FolderKanban size={28} />}>
            <span>从左侧选择一个项目</span>
          </CenterEmpty>
        ) : !workspace ? (
          <WorkspaceBootstrap
            projectId={selectedProject.id}
            projectTitle={selectedProject.name}
            busy={busy}
            busyAction={busyAction}
            isDesktopRuntime={isDesktopRuntime}
            onCreate={(prefillSourceWorkspaceId) =>
              onCreateBusinessWorkspace(
                selectedProject.id,
                prefillSourceWorkspaceId ?? undefined,
              )
            }
            onListCandidates={onListBusinessWorkspacePrefillCandidates}
            onPreview={onPreviewBusinessWorkspacePrefill}
          />
        ) : (
          <>
            {readOnly && (
              <div className="bdc-readonly" role="status">
                <Archive size={14} />
                已归档，只读查看
              </div>
            )}

            {view === "overview" && summary && (
              <OverviewView
                workspace={workspace}
                workspaceEvents={visibleEvents}
                summary={summary}
                busy={busy}
                readOnly={readOnly}
                onOpenCustomer={() => setView("customer")}
                onStartDocument={startDocument}
                onOpenDelivery={() => setView("delivery")}
                onOpenFinance={() => setView("finance")}
                onOpenArchive={() => setView("archive")}
              />
            )}

            {view === "customer" && profileDraft && customerDraft && (
              <CustomerView
                workspace={workspace}
                latestConfirmedRequirement={latestConfirmedRequirement}
                businessCustomers={businessCustomers}
                customerDraft={customerDraft}
                profileDraft={profileDraft}
                quoteHistorySources={quoteHistorySources}
                busy={busy || readOnly}
                customerSaveBusy={busyAction === "business:customer:upsert"}
                profileSaveBusy={busyAction === "business:update-profile"}
                adoptBusy={busyAction === "business:requirement:adopt"}
                customerActionAvailable={Boolean(onUpsertBusinessCustomer)}
                customerAssignAvailable={Boolean(onAssignBusinessCustomer)}
                customerAssignBusy={busyAction === "business:customer:assign"}
                onCustomerChange={setCustomerDraft}
                onCustomerReset={() => setCustomerDraft(businessCustomerToInput(workspace))}
                onCustomerSubmit={submitCustomer}
                onAssignCustomer={(customerId) =>
                  onAssignBusinessCustomer?.({ workspaceId: workspace.id, customerId }) ??
                  Promise.resolve(false)
                }
                onAdoptRequirement={() => void adoptLatestRequirement()}
                onProfileChange={setProfileDraft}
                onProfileReset={() => setProfileDraft(businessProfileToInput(workspace.profile))}
                onProfileSubmit={submitProfile}
              />
            )}

            {view === "documents" && (
              <DocumentsView
                workspace={workspace}
                draft={documentDraft}
                quoteConfirmationDraft={quoteConfirmationDraft}
                documentTransitionDraft={documentTransitionDraft}
                busyAction={busyAction}
                readOnly={readOnly}
                assetActionCapabilities={assetActionCapabilities}
                onDraftChange={setDocumentDraft}
                onQuoteConfirmationDraftChange={setQuoteConfirmationDraft}
                onDocumentTransitionDraftChange={setDocumentTransitionDraft}
                onStartDocument={startDocument}
                onStartQuoteConfirmation={(document) => {
                  setDocumentDraft(null);
                  setDocumentTransitionDraft(null);
                  setQuoteConfirmationDraft(defaultQuoteConfirmationDraft(document, workspace));
                }}
                onCancelDraft={() => setDocumentDraft(null)}
                onCancelQuoteConfirmation={() => setQuoteConfirmationDraft(null)}
                onCancelDocumentTransition={() => setDocumentTransitionDraft(null)}
                onSubmitDraft={submitDocument}
                onSubmitQuoteConfirmation={submitQuoteConfirmation}
                onSubmitDocumentTransition={submitDocumentTransition}
                onChangeStatus={(document, status) => void changeDocumentStatus(document, status)}
                onGenerate={(document) => {
                  if (documentGenerateBlockReason(workspace, document)) return;
                  const definition = documentDefinition(document.kind);
                  void onGenerateBusinessDocument(workspace.id, document.id, definition.format);
                }}
                onOpenAsset={onOpenAsset}
                onExportAsset={onExportAsset}
              />
            )}

            {view === "delivery" && (
              <DeliveryView
                workspace={workspace}
                assets={assets}
                busyAction={busyAction}
                readOnly={readOnly}
                milestoneDraft={milestoneDraft}
                deliverableDraft={deliverableDraft}
                sentDraft={deliverySentDraft}
                signoffDraft={deliverySignoffDraft}
                milestoneActionAvailable={Boolean(onUpsertBusinessMilestone)}
                deliverableActionAvailable={Boolean(onRegisterBusinessDeliverableVersion)}
                sentActionAvailable={Boolean(onRecordBusinessDeliverySent)}
                signoffActionAvailable={Boolean(onRecordBusinessDeliverySignoff)}
                assetActionCapabilities={assetActionCapabilities}
                assetImportAvailable={Boolean(onImportBusinessAsset)}
                onImportAsset={() =>
                  onImportBusinessAsset?.(workspace.id) ?? Promise.resolve(null)
                }
                onMilestoneDraftChange={setMilestoneDraft}
                onDeliverableDraftChange={setDeliverableDraft}
                onSentDraftChange={setDeliverySentDraft}
                onSignoffDraftChange={setDeliverySignoffDraft}
                onCreateMilestone={() => setMilestoneDraft(emptyMilestoneDraft())}
                onEditMilestone={(milestone) => setMilestoneDraft(milestoneToInput(milestone))}
                onCreateDeliverable={(milestone) => setDeliverableDraft(emptyDeliverableDraft(milestone))}
                onCreateVersion={(milestone, deliverable) => setDeliverableDraft(deliverableVersionDraft(milestone, deliverable))}
                onStartSending={(milestone) => setDeliverySentDraft(defaultDeliverySentDraft(milestone, workspace))}
                onStartSignoff={(submission) => setDeliverySignoffDraft(defaultDeliverySignoffDraft(submission))}
                onSubmitMilestone={submitMilestone}
                onSubmitDeliverable={submitDeliverable}
                onSubmitSent={submitDeliverySent}
                onSubmitSignoff={submitDeliverySignoff}
                onOpenAsset={onOpenAsset}
                onExportAsset={onExportAsset}
              />
            )}

            {view === "finance" && summary && (
              <FinanceView
                workspace={workspace}
                assets={assets}
                summary={summary}
                paymentDraft={paymentDraft}
                receiptDraft={receiptDraft}
                receiptReversalDraft={receiptReversalDraft}
                invoiceDraft={invoiceDraft}
                invoiceReversalDraft={invoiceReversalDraft}
                invoiceAttachmentDraft={invoiceAttachmentDraft}
                busyAction={busyAction}
                readOnly={readOnly}
                invoiceActionAvailable={Boolean(onRecordBusinessInvoiceIssued)}
                invoiceReversalActionAvailable={Boolean(onRecordBusinessInvoiceRedCorrection)}
                invoiceAttachmentActionAvailable={Boolean(onAttachBusinessInvoiceAsset)}
                assetActionCapabilities={assetActionCapabilities}
                assetImportAvailable={Boolean(onImportBusinessAsset)}
                onImportAsset={() =>
                  onImportBusinessAsset?.(workspace.id) ?? Promise.resolve(null)
                }
                onCreatePayment={() => {
                  setReceiptDraft(null);
                  setReceiptReversalDraft(null);
                  setPaymentDraft(emptyPaymentDraft());
                }}
                onEditPayment={(payment) => {
                  setReceiptDraft(null);
                  setReceiptReversalDraft(null);
                  setPaymentDraft(paymentRecordToDraft(payment));
                }}
                onPaymentDraftChange={setPaymentDraft}
                onReceiptDraftChange={setReceiptDraft}
                onReceiptReversalDraftChange={setReceiptReversalDraft}
                onCancelPayment={() => setPaymentDraft(null)}
                onCancelReceipt={() => setReceiptDraft(null)}
                onCancelReceiptReversal={() => setReceiptReversalDraft(null)}
                onSubmitPayment={submitPayment}
                onSubmitReceipt={submitReceipt}
                onSubmitReceiptReversal={submitReceiptReversal}
                onAdvancePayment={(payment, status) => void advancePayment(payment, status)}
                onStartReceipt={(payment) => {
                  setPaymentDraft(null);
                  setReceiptReversalDraft(null);
                  setReceiptDraft(defaultReceiptDraft(payment, workspace));
                }}
                onStartReceiptReversal={(receipt) => {
                  setPaymentDraft(null);
                  setReceiptDraft(null);
                  setReceiptReversalDraft(defaultReceiptReversalDraft(receipt, workspace));
                }}
                onCreateRequest={(paymentId) => {
                  const next = defaultDocumentDraft("paymentRequest", workspace, selectedProject);
                  setDocumentDraft({ ...next, paymentId });
                  setView("documents");
                }}
                onInvoiceDraftChange={setInvoiceDraft}
                onInvoiceReversalDraftChange={setInvoiceReversalDraft}
                onInvoiceAttachmentDraftChange={setInvoiceAttachmentDraft}
                onStartInvoice={() => setInvoiceDraft(emptyInvoiceDraft(workspace))}
                onStartInvoiceReversal={(invoice) => workspace && setInvoiceReversalDraft(defaultInvoiceReversalDraft(workspace, invoice))}
                onStartInvoiceAttachment={(invoice) => setInvoiceAttachmentDraft({ invoiceId: invoice.id, assetId: "", role: "supplement" })}
                onSubmitInvoice={submitInvoice}
                onSubmitInvoiceReversal={submitInvoiceReversal}
                onSubmitInvoiceAttachment={submitInvoiceAttachment}
                onOpenAsset={onOpenAsset}
                onExportAsset={onExportAsset}
              />
            )}

            {view === "archive" && (
              <ArchiveView
                workspace={workspace}
                checks={archiveChecks}
                busyAction={busyAction}
                snapshotActionAvailable={Boolean(onCreateBusinessArchiveSnapshot)}
                archiveBlockReason={archiveBlockReason}
                onCreateSnapshot={() => void createArchiveSnapshot()}
                onArchive={() => void changeWorkspaceStatus("archived")}
                onReopen={() => void changeWorkspaceStatus("active")}
                onOpenAsset={onOpenAsset}
                onExportAsset={onExportAsset}
              />
            )}
          </>
        )}
      </div>
    </section>
  );
}

function OverviewView({
  workspace,
  workspaceEvents,
  summary,
  busy,
  readOnly,
  onOpenCustomer,
  onStartDocument,
  onOpenDelivery,
  onOpenFinance,
  onOpenArchive,
}: {
  workspace: BusinessWorkspaceRecord;
  workspaceEvents: readonly BusinessWorkspaceDomainEvent[];
  summary: BusinessWorkspaceSummary;
  busy: boolean;
  readOnly: boolean;
  onOpenCustomer: () => void;
  onStartDocument: (kind: BusinessDocumentKind) => void;
  onOpenDelivery: () => void;
  onOpenFinance: () => void;
  onOpenArchive: () => void;
}) {
  const requiredMilestones = workspace.milestones.filter((milestone) => milestone.required);
  const acceptedMilestones = requiredMilestones.filter((milestone) => milestone.status === "accepted");
  const invoiceNet = invoiceNetCents(workspace.invoices);
  const currency = workspace.profile.currency;
  const archiveReady = workspace.archiveIntegrityStatus === "ready";

  return (
    <div className="business-documents-center__view bdc-overview">
      <section className="bdc-metrics">
        <Metric label="合同" value={formatMoney(summary.contractCents, currency)} detail={workspace.currentDocuments.contractDocumentId ? "已建立" : "待建立"} />
        <Metric label="已到账" value={formatMoney(summary.receivedCents, currency)} detail={summary.outstandingCents > 0 ? "待收 " + formatMoney(summary.outstandingCents, currency) : "已结清"} tone="success" />
        <Metric label="开票净额" value={formatMoney(invoiceNet, currency)} detail={workspace.invoices.length + " 条票据记录"} tone={invoiceNet === summary.contractCents && invoiceNet > 0 ? "success" : "default"} />
        <Metric label="交付" value={acceptedMilestones.length + "/" + requiredMilestones.length} detail="必需里程碑已签收" tone={requiredMilestones.length > 0 && acceptedMilestones.length === requiredMilestones.length ? "success" : "warning"} />
      </section>

      <section className="bdc-flow-card">
        <header className="bdc-section-head">
          <div><span>FLOW</span><strong>当前闭环</strong></div>
        </header>
        <div className="bdc-flow-grid">
          <button type="button" onClick={onOpenCustomer}>
            <span className={workspace.customerId ? "is-done" : ""}>{workspace.customerId ? <Check size={13} /> : "1"}</span>
            <div><strong>客户</strong><small>{workspace.customer.legalName || "补齐主体资料"}</small></div>
            <ChevronRight size={14} />
          </button>
          <button type="button" onClick={() => onStartDocument("quote")} disabled={busy || readOnly || Boolean(documentCreationBlockReason(workspace, "quote"))}>
            <span className={workspace.documents.length > 0 ? "is-done" : ""}>{workspace.documents.length > 0 ? <Check size={13} /> : "2"}</span>
            <div><strong>单据</strong><small>{summary.generatedDocuments} 份已生成</small></div>
            <ChevronRight size={14} />
          </button>
          <button type="button" onClick={onOpenDelivery}>
            <span className={acceptedMilestones.length > 0 && acceptedMilestones.length === requiredMilestones.length ? "is-done" : ""}>3</span>
            <div><strong>交付</strong><small>{workspace.deliverySubmissions.length} 次发送</small></div>
            <ChevronRight size={14} />
          </button>
          <button type="button" onClick={onOpenFinance}>
            <span className={summary.outstandingCents === 0 && summary.contractCents > 0 ? "is-done" : ""}>4</span>
            <div><strong>财务</strong><small>{workspace.payments.length} 个付款节点</small></div>
            <ChevronRight size={14} />
          </button>
          <button type="button" onClick={onOpenArchive}>
            <span className={archiveReady ? "is-done" : ""}>{archiveReady ? <Check size={13} /> : "5"}</span>
            <div><strong>归档</strong><small>{archiveIntegrityLabel(workspace.archiveIntegrityStatus)}</small></div>
            <ChevronRight size={14} />
          </button>
        </div>
      </section>

      <section className="bdc-overview-grid">
        <div className="bdc-panel">
          <header className="bdc-section-head"><div><span>DOCUMENTS</span><strong>当前单据</strong></div></header>
          <div className="bdc-compact-list">
            {DOCUMENT_DEFINITIONS.map((definition) => {
              const document = currentDocumentForKind(workspace, definition.kind);
              const Icon = definition.icon;
              return (
                <button type="button" key={definition.kind} onClick={() => onStartDocument(definition.kind)} disabled={busy || readOnly || Boolean(documentCreationBlockReason(workspace, definition.kind))}>
                  <span className="bdc-list-icon"><Icon size={15} /></span>
                  <span><strong>{definition.shortLabel}</strong><small>{document?.documentNumber ?? "未建立"}</small></span>
                  <em>{document ? DOCUMENT_STATUS_LABELS[document.status] : "新建"}</em>
                </button>
              );
            })}
          </div>
        </div>

        <div className="bdc-panel">
          <header className="bdc-section-head"><div><span>ACTIVITY</span><strong>最近动态</strong></div></header>
          <div className="bdc-activity-list">
            {workspaceEvents.length === 0 ? (
              <InlineEmpty icon={<FileClock size={18} />} text="暂无动态" />
            ) : workspaceEvents.slice(0, 6).map((event) => (
              <article key={event.eventId}>
                <span />
                <div><strong>{eventTypeLabel(event.eventType)}</strong><small>{event.reason || eventReason(event.eventType)}</small></div>
                <time>{formatDateTime(event.occurredAt)}</time>
              </article>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}

function CustomerView({
  workspace,
  latestConfirmedRequirement,
  businessCustomers,
  customerDraft,
  profileDraft,
  quoteHistorySources,
  busy,
  customerSaveBusy,
  profileSaveBusy,
  adoptBusy,
  customerActionAvailable,
  customerAssignAvailable,
  customerAssignBusy,
  onCustomerChange,
  onCustomerReset,
  onCustomerSubmit,
  onAssignCustomer,
  onAdoptRequirement,
  onProfileChange,
  onProfileReset,
  onProfileSubmit,
}: {
  workspace: BusinessWorkspaceRecord;
  latestConfirmedRequirement: RequirementBriefRecord | null;
  businessCustomers: readonly BusinessCustomerReceivableSummary[];
  customerDraft: BusinessCustomerInput;
  profileDraft: BusinessProfileInput;
  quoteHistorySources: readonly QuoteHistorySource[];
  busy: boolean;
  customerSaveBusy: boolean;
  profileSaveBusy: boolean;
  adoptBusy: boolean;
  customerActionAvailable: boolean;
  customerAssignAvailable: boolean;
  customerAssignBusy: boolean;
  onCustomerChange: (draft: BusinessCustomerInput) => void;
  onCustomerReset: () => void;
  onCustomerSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onAssignCustomer: (customerId: string) => Promise<boolean>;
  onAdoptRequirement: () => void;
  onProfileChange: (draft: BusinessProfileInput) => void;
  onProfileReset: () => void;
  onProfileSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const [selectedCustomerId, setSelectedCustomerId] = useState("");
  const assignableCustomers = businessCustomers.filter(
    (customer) =>
      customer.customerStatus === "active" && customer.customerId !== workspace.customerId,
  );
  useEffect(() => setSelectedCustomerId(""), [workspace.customerId]);
  const adoptionReason = requirementAdoptionBlockReason(workspace, latestConfirmedRequirement);
  const requirementCurrent = latestConfirmedRequirement !== null &&
    workspace.requirementBriefId === latestConfirmedRequirement.id &&
    workspace.requirementBriefRevision === latestConfirmedRequirement.revision;

  const setCustomerField = <Key extends keyof BusinessCustomerInput>(
    key: Key,
    value: BusinessCustomerInput[Key],
  ) => {
    const next = { ...customerDraft, [key]: value };
    onCustomerChange(next);
    const profilePatch: Partial<BusinessProfileInput> = {};
    if (key === "displayName") profilePatch.customerName = String(value);
    if (key === "legalName") profilePatch.customerLegalName = String(value);
    if (key === "taxId") profilePatch.customerTaxId = String(value);
    if (key === "billingAddress") profilePatch.customerAddress = String(value);
    if (key === "primaryContactName") profilePatch.customerContact = String(value);
    if (key === "primaryPhone") profilePatch.customerPhone = String(value);
    if (key === "primaryEmail") profilePatch.customerEmail = String(value);
    if (Object.keys(profilePatch).length > 0) onProfileChange({ ...profileDraft, ...profilePatch });
  };

  const setProfileField = <Key extends keyof BusinessProfileInput>(
    key: Key,
    value: BusinessProfileInput[Key],
  ) => onProfileChange({ ...profileDraft, [key]: value });

  const updateLineItem = (index: number, update: Partial<BusinessLineItemInput>) => {
    onProfileChange({
      ...profileDraft,
      lineItems: profileDraft.lineItems.map((item, itemIndex) =>
        itemIndex === index ? { ...item, ...update } : item,
      ),
    });
  };

  return (
    <div className="business-documents-center__view bdc-customer-view">
      <section className="bdc-panel bdc-customer-link">
        <header className="bdc-section-head">
          <div><span>DIRECTORY</span><strong>绑定已有客户</strong></div>
          <small>{assignableCustomers.length} 个可选客户</small>
        </header>
        <div className="bdc-customer-link__controls">
          <select
            value={selectedCustomerId}
            onChange={(event) => setSelectedCustomerId(event.currentTarget.value)}
            disabled={busy || assignableCustomers.length === 0}
            aria-label="选择已有客户"
          >
            <option value="">{assignableCustomers.length === 0 ? "没有其他可绑定客户" : "选择客户"}</option>
            {assignableCustomers.map((customer) => (
              <option key={customer.customerId} value={customer.customerId}>
                {customer.customerLegalName || customer.customerName}
                {customer.customerTaxId ? ` · ${customer.customerTaxId}` : ""}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="is-primary"
            disabled={busy || !customerAssignAvailable || !selectedCustomerId}
            onClick={() => {
              if (!selectedCustomerId) return;
              void onAssignCustomer(selectedCustomerId).then((assigned) => {
                if (assigned) setSelectedCustomerId("");
              });
            }}
          >
            {customerAssignBusy ? <LoaderCircle className="business-documents-center__spin" size={13} /> : <UserRound size={13} />}
            绑定客户
          </button>
        </div>
      </section>

      <form className="bdc-panel bdc-customer-master" onSubmit={onCustomerSubmit}>
        <header className="bdc-section-head">
          <div><span>CUSTOMER</span><strong>客户主数据</strong></div>
          <span className="bdc-revision">版本 {workspace.customer.revision}</span>
        </header>
        <div className="bdc-customer-id">
          <span><Hash size={13} />客户 ID</span>
          <code title={workspace.customerId}>{workspace.customerId}</code>
        </div>
        <div className="bdc-form-grid bdc-form-grid--two">
          <Field label="客户简称"><input value={customerDraft.displayName} onChange={(event) => setCustomerField("displayName", event.currentTarget.value)} required /></Field>
          <Field label="法定名称"><input value={customerDraft.legalName} onChange={(event) => setCustomerField("legalName", event.currentTarget.value)} required /></Field>
          <Field label="统一社会信用代码"><input value={customerDraft.taxId} onChange={(event) => setCustomerField("taxId", event.currentTarget.value)} /></Field>
          <Field label="开票地址"><input value={customerDraft.billingAddress} onChange={(event) => setCustomerField("billingAddress", event.currentTarget.value)} /></Field>
          <Field label="联系人"><input value={customerDraft.primaryContactName} onChange={(event) => setCustomerField("primaryContactName", event.currentTarget.value)} /></Field>
          <Field label="电话"><input value={customerDraft.primaryPhone} onChange={(event) => setCustomerField("primaryPhone", event.currentTarget.value)} /></Field>
          <Field label="邮箱" wide><input type="email" value={customerDraft.primaryEmail} onChange={(event) => setCustomerField("primaryEmail", event.currentTarget.value)} /></Field>
          <Field label="客户备注" wide><textarea rows={3} value={customerDraft.notes} onChange={(event) => setCustomerField("notes", event.currentTarget.value)} /></Field>
        </div>
        <footer className="bdc-form-actions">
          <button type="button" onClick={onCustomerReset} disabled={busy}><RotateCcw size={13} />恢复</button>
          <button type="submit" className="is-primary" disabled={busy || !customerActionAvailable} title={customerActionAvailable ? "保存客户主数据" : "保存能力不可用"}>
            {customerSaveBusy ? <LoaderCircle className="business-documents-center__spin" size={13} /> : <Save size={13} />}
            保存客户
          </button>
        </footer>
      </form>

      <form className="bdc-panel bdc-project-profile" onSubmit={onProfileSubmit}>
        <header className="bdc-section-head">
          <div><span>PROJECT DEFAULTS</span><strong>本项目单据资料</strong></div>
          <button type="button" onClick={onAdoptRequirement} disabled={busy || Boolean(adoptionReason)} title={adoptionReason ?? "采用最新已确认需求"}>
            {adoptBusy ? <LoaderCircle className="business-documents-center__spin" size={13} /> : requirementCurrent ? <Check size={13} /> : <RefreshCw size={13} />}
            {requirementCurrent ? "需求已同步" : "同步需求"}
          </button>
        </header>

        <details open>
          <summary>项目与履约 <span>{profileDraft.projectCode || "未编号"}</span></summary>
          <div className="bdc-form-grid bdc-form-grid--two">
            <Field label="项目名称"><input value={profileDraft.projectTitle} onChange={(event) => setProfileField("projectTitle", event.currentTarget.value)} required /></Field>
            <Field label="项目编号"><input value={profileDraft.projectCode} onChange={(event) => setProfileField("projectCode", event.currentTarget.value)} required /></Field>
            <Field label="服务开始"><input type="date" value={timestampToDateInput(profileDraft.serviceStartAt)} onChange={(event) => setProfileField("serviceStartAt", dateInputToTimestamp(event.currentTarget.value))} /></Field>
            <Field label="服务结束"><input type="date" value={timestampToDateInput(profileDraft.serviceEndAt)} onChange={(event) => setProfileField("serviceEndAt", dateInputToTimestamp(event.currentTarget.value))} /></Field>
            <Field label="交付摘要" wide><textarea rows={2} value={profileDraft.deliverySummary} onChange={(event) => setProfileField("deliverySummary", event.currentTarget.value)} /></Field>
            <Field label="付款条款" wide><textarea rows={2} value={profileDraft.paymentTerms} onChange={(event) => setProfileField("paymentTerms", event.currentTarget.value)} /></Field>
            <Field label="验收条款" wide><textarea rows={2} value={profileDraft.acceptanceTerms} onChange={(event) => setProfileField("acceptanceTerms", event.currentTarget.value)} /></Field>
          </div>
        </details>

        <details>
          <summary>我方开票与收款 <span>{profileDraft.supplierLegalName || "未填写"}</span></summary>
          <div className="bdc-form-grid bdc-form-grid--two">
            <Field label="公司名称"><input value={profileDraft.supplierLegalName} onChange={(event) => setProfileField("supplierLegalName", event.currentTarget.value)} /></Field>
            <Field label="税号"><input value={profileDraft.supplierTaxId} onChange={(event) => setProfileField("supplierTaxId", event.currentTarget.value)} /></Field>
            <Field label="地址"><input value={profileDraft.supplierAddress} onChange={(event) => setProfileField("supplierAddress", event.currentTarget.value)} /></Field>
            <Field label="联系人"><input value={profileDraft.supplierContact} onChange={(event) => setProfileField("supplierContact", event.currentTarget.value)} /></Field>
            <Field label="电话"><input value={profileDraft.supplierPhone} onChange={(event) => setProfileField("supplierPhone", event.currentTarget.value)} /></Field>
            <Field label="开户行"><input value={profileDraft.supplierBankName} onChange={(event) => setProfileField("supplierBankName", event.currentTarget.value)} /></Field>
            <Field label="银行账号" wide><input value={profileDraft.supplierBankAccount} onChange={(event) => setProfileField("supplierBankAccount", event.currentTarget.value)} /></Field>
            <Field label="币种"><input value={profileDraft.currency} onChange={(event) => setProfileField("currency", event.currentTarget.value.toUpperCase())} /></Field>
            <Field label="默认税率 (%)"><input type="number" min="0" max="100" step="0.01" value={basisPointsToPercent(profileDraft.defaultTaxRateBps)} onChange={(event) => setProfileField("defaultTaxRateBps", percentToBasisPoints(event.currentTarget.value))} /></Field>
          </div>
        </details>

        <details open>
          <summary>服务明细 <span>{formatMoney(profileLineItemsTotal(profileDraft as BusinessProfile), profileDraft.currency)}</span></summary>
          <div className="bdc-line-items">
            <div className="bdc-line-toolbar">
              <select
                value=""
                onChange={(event) => {
                  const template = QUOTE_TEMPLATES.find(
                    (candidate) => candidate.id === event.currentTarget.value,
                  );
                  if (!template) return;
                  onProfileChange({
                    ...profileDraft,
                    lineItems: applyQuoteTemplateItems(
                      profileDraft.lineItems,
                      template.items,
                      profileDraft.defaultTaxRateBps,
                    ),
                  });
                }}
                disabled={busy}
                aria-label="套用报价模板"
              >
                <option value="">套用报价模板…</option>
                {QUOTE_TEMPLATES.map((template) => (
                  <option key={template.id} value={template.id}>
                    {template.label}（{template.items.length} 项）
                  </option>
                ))}
              </select>
              {quoteHistorySources.length > 0 && (
                <select
                  value=""
                  onChange={(event) => {
                    const source = quoteHistorySources.find(
                      (candidate) => candidate.workspaceId === event.currentTarget.value,
                    );
                    if (!source) return;
                    onProfileChange({
                      ...profileDraft,
                      lineItems: applyHistoryLineItems(
                        profileDraft.lineItems,
                        source.lineItems,
                      ),
                    });
                  }}
                  disabled={busy}
                  aria-label="套用历史报价"
                >
                  <option value="">套用历史报价…</option>
                  {quoteHistorySources.map((source) => (
                    <option key={source.workspaceId} value={source.workspaceId}>
                      {source.projectTitle} · {source.customerName}（{source.lineItems.length} 项）
                    </option>
                  ))}
                </select>
              )}
              <small>模板不带价格，历史报价带原价；都会加进下面的列表，随时可以改。</small>
            </div>
            {profileDraft.lineItems.map((item, index) => (
              <div className="bdc-line-item" key={item.id ?? "line-" + index}>
                <input className="is-name" value={item.name} onChange={(event) => updateLineItem(index, { name: event.currentTarget.value })} placeholder="服务项" required />
                <input type="number" min="0.001" step="0.001" value={millisToDecimal(item.quantityMillis)} onChange={(event) => updateLineItem(index, { quantityMillis: decimalToMillis(event.currentTarget.value) })} aria-label="数量" />
                <input value={item.unit} onChange={(event) => updateLineItem(index, { unit: event.currentTarget.value })} aria-label="单位" />
                <input type="number" min="0" step="0.01" value={centsToDecimal(item.unitPriceCents)} onChange={(event) => updateLineItem(index, { unitPriceCents: decimalToCents(event.currentTarget.value) })} aria-label="单价" />
                <strong>{formatMoney(lineItemAmount(item), profileDraft.currency)}</strong>
                <button type="button" onClick={() => onProfileChange({ ...profileDraft, lineItems: profileDraft.lineItems.filter((_, itemIndex) => itemIndex !== index) })} disabled={busy} aria-label="删除服务项"><Trash2 size={13} /></button>
              </div>
            ))}
            <button type="button" className="bdc-add-row" onClick={() => onProfileChange({ ...profileDraft, lineItems: [...profileDraft.lineItems, emptyLineItem(profileDraft.defaultTaxRateBps)] })} disabled={busy}><Plus size={13} />添加服务项</button>
          </div>
        </details>

        <details>
          <summary>内部备注</summary>
          <textarea className="bdc-notes" rows={3} value={profileDraft.notes} onChange={(event) => setProfileField("notes", event.currentTarget.value)} />
        </details>

        <footer className="bdc-form-actions">
          <small>草稿只在切换项目时重置</small>
          <button type="button" onClick={onProfileReset} disabled={busy}><RotateCcw size={13} />恢复</button>
          <button type="submit" className="is-primary" disabled={busy}>
            {profileSaveBusy ? <LoaderCircle className="business-documents-center__spin" size={13} /> : <Save size={13} />}
            保存项目资料
          </button>
        </footer>
      </form>
    </div>
  );
}

function DocumentsView({
  workspace,
  draft,
  quoteConfirmationDraft,
  documentTransitionDraft,
  busyAction,
  readOnly,
  assetActionCapabilities,
  onDraftChange,
  onQuoteConfirmationDraftChange,
  onDocumentTransitionDraftChange,
  onStartDocument,
  onStartQuoteConfirmation,
  onCancelDraft,
  onCancelQuoteConfirmation,
  onCancelDocumentTransition,
  onSubmitDraft,
  onSubmitQuoteConfirmation,
  onSubmitDocumentTransition,
  onChangeStatus,
  onGenerate,
  onOpenAsset,
  onExportAsset,
}: {
  workspace: BusinessWorkspaceRecord;
  draft: DocumentDraft | null;
  quoteConfirmationDraft: QuoteConfirmationDraft | null;
  documentTransitionDraft: DocumentTransitionDraft | null;
  busyAction: string | null;
  readOnly: boolean;
  assetActionCapabilities: Readonly<Record<string, AssetActionCapabilities>>;
  onDraftChange: (draft: DocumentDraft) => void;
  onQuoteConfirmationDraftChange: (draft: QuoteConfirmationDraft) => void;
  onDocumentTransitionDraftChange: (draft: DocumentTransitionDraft) => void;
  onStartDocument: (kind: BusinessDocumentKind) => void;
  onStartQuoteConfirmation: (document: BusinessDocumentRecord) => void;
  onCancelDraft: () => void;
  onCancelQuoteConfirmation: () => void;
  onCancelDocumentTransition: () => void;
  onSubmitDraft: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitQuoteConfirmation: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitDocumentTransition: (event: FormEvent<HTMLFormElement>) => void;
  onChangeStatus: (document: BusinessDocumentRecord, status: BusinessDocumentStatus) => void;
  onGenerate: (document: BusinessDocumentRecord) => void;
  onOpenAsset: (assetId: string) => void;
  onExportAsset: (assetId: string) => void;
}) {
  const busy = busyAction !== null;
  const orderedDocuments = [...workspace.documents].sort(
    (left, right) => right.updatedAt - left.updatedAt,
  );
  const requestablePayments = availablePaymentPlans(workspace);
  const draftBlockReason = draft
    ? documentCreationBlockReason(workspace, draft.kind)
    : null;
  const quoteForConfirmation = quoteConfirmationDraft
    ? workspace.documents.find(
        (document) => document.id === quoteConfirmationDraft.quoteDocumentId,
      ) ?? null
    : null;
  const confirmationBlockReason = quoteForConfirmation
    ? quoteConfirmationBlockReason(workspace, quoteForConfirmation)
    : "找不到需要确认的报价单";
  const transitionDocument = documentTransitionDraft
    ? workspace.documents.find(
        (document) => document.id === documentTransitionDraft.documentId,
      ) ?? null
    : null;

  return (
    <div className="business-documents-center__view business-documents-center__documents-view">
      {readOnly && <InlineNotice text="工作区已归档，单据仅供查看。重新打开后才能创建、审批、生成或作废。" />}

      <section className="business-documents-center__document-kinds is-picker">
        {DOCUMENT_DEFINITIONS.map((definition) => {
          const Icon = definition.icon;
          const count = workspace.documents.filter((document) => document.kind === definition.kind).length;
          const current = currentDocumentForKind(workspace, definition.kind);
          const blockReason = documentCreationBlockReason(workspace, definition.kind);
          return (
            <button
              type="button"
              key={definition.kind}
              className={current ? "is-current" : ""}
              onClick={() => onStartDocument(definition.kind)}
              disabled={busy || readOnly || Boolean(blockReason)}
              title={blockReason ?? `新建${definition.label}`}
            >
              <span><Icon size={17} /></span>
              <strong>{definition.label}</strong>
              <small>{current ? `当前有效 · ${current.documentNumber}` : `${count} 个版本`}</small>
              <Plus size={14} />
            </button>
          );
        })}
      </section>

      {!readOnly && (
        <div className="business-documents-center__gate-note">
          <ShieldCheck size={15} />
          <span>合同需基于当前已生成报价；请款与验收需基于当前已生效合同；请款单仅能关联尚未请款的付款计划。</span>
        </div>
      )}

      {draft && (
        <form className="business-documents-center__composer" onSubmit={onSubmitDraft}>
          <header className="business-documents-center__section-title">
            <div>
              <span>NEW DOCUMENT</span>
              <strong>新建{documentDefinition(draft.kind).label}</strong>
            </div>
            <button type="button" onClick={onCancelDraft} title="关闭" aria-label="关闭新建单据">
              <X size={15} />
            </button>
          </header>
          {draftBlockReason && <InlineNotice tone="warning" text={draftBlockReason} />}
          <div className="business-documents-center__composer-fields">
            <Field label="单据类型">
              <select
                value={draft.kind}
                onChange={(event) => onStartDocument(event.currentTarget.value as BusinessDocumentKind)}
                disabled={readOnly}
              >
                {DOCUMENT_DEFINITIONS.map((definition) => (
                  <option
                    key={definition.kind}
                    value={definition.kind}
                    disabled={Boolean(documentCreationBlockReason(workspace, definition.kind))}
                  >
                    {definition.label}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="单据编号">
              <input
                value={draft.documentNumber}
                onChange={(event) => onDraftChange({ ...draft, documentNumber: event.currentTarget.value })}
                required
                disabled={readOnly}
              />
            </Field>
            <Field label="标题" wide>
              <input
                value={draft.title}
                onChange={(event) => onDraftChange({ ...draft, title: event.currentTarget.value })}
                required
                disabled={readOnly}
              />
            </Field>
            {draft.kind === "paymentRequest" && (
              <Field label="付款节点" wide>
                <select
                  value={draft.paymentId}
                  onChange={(event) => onDraftChange({ ...draft, paymentId: event.currentTarget.value })}
                  required
                  disabled={readOnly}
                >
                  <option value="">选择尚未请款的付款计划</option>
                  {requestablePayments.map((payment) => (
                    <option key={payment.id} value={payment.id}>
                      {payment.label} · {formatMoney(payment.amountCents, workspace.profile.currency)}
                    </option>
                  ))}
                </select>
              </Field>
            )}
          </div>
          <footer>
            <span>保存后可继续审核并生成正式文件</span>
            <button
              type="submit"
              className="business-documents-center__primary-button"
              disabled={
                busy ||
                readOnly ||
                Boolean(draftBlockReason) ||
                !draft.documentNumber.trim() ||
                !draft.title.trim() ||
                (draft.kind === "paymentRequest" && !draft.paymentId)
              }
              title={draftBlockReason ?? "建立单据草稿"}
            >
              {busyAction === "business:create-document" ? <LoaderCircle className="business-documents-center__spin" size={14} /> : <FilePlus2 size={14} />}
              建立单据草稿
            </button>
          </footer>
        </form>
      )}

      {quoteConfirmationDraft && quoteForConfirmation && (
        <form className="business-documents-center__composer is-confirmation" onSubmit={onSubmitQuoteConfirmation}>
          <header className="business-documents-center__section-title">
            <div>
              <span>QUOTE CONFIRMATION</span>
              <strong>登记客户报价确认</strong>
            </div>
            <button type="button" onClick={onCancelQuoteConfirmation} title="关闭" aria-label="关闭报价确认">
              <X size={15} />
            </button>
          </header>
          {confirmationBlockReason && <InlineNotice tone="warning" text={confirmationBlockReason} />}
          <div className="business-documents-center__composer-fields">
            <Field label="确认报价">
              <div className="business-documents-center__readonly-field">{quoteForConfirmation.documentNumber}</div>
            </Field>
            <Field label="确认版本">
              <input
                value={quoteConfirmationDraft.confirmationVersion}
                onChange={(event) => onQuoteConfirmationDraftChange({ ...quoteConfirmationDraft, confirmationVersion: event.currentTarget.value })}
                required
              />
            </Field>
            <Field label="客户确认人">
              <input
                value={quoteConfirmationDraft.customerRepresentative}
                onChange={(event) => onQuoteConfirmationDraftChange({ ...quoteConfirmationDraft, customerRepresentative: event.currentTarget.value })}
                required
              />
            </Field>
            <Field label="确认日期">
              <input
                type="date"
                value={quoteConfirmationDraft.occurredDate}
                onChange={(event) => onQuoteConfirmationDraftChange({ ...quoteConfirmationDraft, occurredDate: event.currentTarget.value })}
                required
              />
            </Field>
            <Field label="备注" wide>
              <textarea
                value={quoteConfirmationDraft.notes}
                onChange={(event) => onQuoteConfirmationDraftChange({ ...quoteConfirmationDraft, notes: event.currentTarget.value })}
                rows={3}
                placeholder="确认渠道、范围或补充约定"
              />
            </Field>
          </div>
          <footer>
            <span>提交时选择客户邮件、聊天截图或盖章回传件作为确认凭证</span>
            <button
              type="submit"
              className="business-documents-center__primary-button"
              disabled={
                busy ||
                readOnly ||
                Boolean(confirmationBlockReason) ||
                !quoteConfirmationDraft.confirmationVersion.trim() ||
                !quoteConfirmationDraft.customerRepresentative.trim() ||
                !quoteConfirmationDraft.occurredDate
              }
              title={confirmationBlockReason ?? "选择确认凭证并登记"}
            >
              {busyAction?.startsWith("business:quote:") ? <LoaderCircle className="business-documents-center__spin" size={14} /> : <CheckCircle2 size={14} />}
              选择凭证并确认
            </button>
          </footer>
        </form>
      )}

      {documentTransitionDraft && transitionDocument && (
        <form className="business-documents-center__composer is-transition" onSubmit={onSubmitDocumentTransition}>
          <header className="business-documents-center__section-title">
            <div>
              <span>{documentTransitionDraft.status === "effective" ? "EFFECTIVE" : "VOID"}</span>
              <strong>{documentTransitionDraft.status === "effective" ? `确认${documentDefinition(transitionDocument.kind).label}生效` : `作废${documentDefinition(transitionDocument.kind).label}`}</strong>
            </div>
            <button type="button" onClick={onCancelDocumentTransition} title="关闭" aria-label="关闭状态确认"><X size={15} /></button>
          </header>
          {documentTransitionDraft.status === "effective" ? (
            <>
              <div className="business-documents-center__segmented" role="group" aria-label="生效依据">
                <button type="button" className={documentTransitionDraft.mode === "evidence" ? "is-active" : ""} onClick={() => onDocumentTransitionDraftChange({ ...documentTransitionDraft, mode: "evidence", reason: "" })}><FileCheck2 size={13} />签署凭证</button>
                <button type="button" className={documentTransitionDraft.mode === "waiver" ? "is-active" : ""} onClick={() => onDocumentTransitionDraftChange({ ...documentTransitionDraft, mode: "waiver" })}><ShieldCheck size={13} />人工豁免</button>
              </div>
              {documentTransitionDraft.mode === "evidence" ? (
                <div className="business-documents-center__composer-fields">
                  <Field label="签署 / 验收日期">
                    <input type="date" value={documentTransitionDraft.occurredDate} onChange={(event) => onDocumentTransitionDraftChange({ ...documentTransitionDraft, occurredDate: event.currentTarget.value })} required />
                  </Field>
                  <Field label="凭证说明">
                    <input value={documentTransitionDraft.evidenceNote} onChange={(event) => onDocumentTransitionDraftChange({ ...documentTransitionDraft, evidenceNote: event.currentTarget.value })} placeholder={transitionDocument.kind === "contract" ? "双方签署合同" : "客户验收确认"} />
                  </Field>
                </div>
              ) : (
                <Field label="豁免原因" wide>
                  <textarea value={documentTransitionDraft.reason} onChange={(event) => onDocumentTransitionDraftChange({ ...documentTransitionDraft, reason: event.currentTarget.value })} rows={3} required placeholder="说明没有正式凭证仍允许生效的业务依据" />
                </Field>
              )}
            </>
          ) : (
            <Field label="作废原因" wide>
              <textarea value={documentTransitionDraft.reason} onChange={(event) => onDocumentTransitionDraftChange({ ...documentTransitionDraft, reason: event.currentTarget.value })} rows={3} required placeholder="作废原因会进入审计记录" />
            </Field>
          )}
          <footer>
            <span>{documentTransitionDraft.status === "effective" && documentTransitionDraft.mode === "evidence" ? "提交时选择签署或验收凭证" : "该操作会写入本地审计记录"}</span>
            <button
              type="submit"
              className={documentTransitionDraft.status === "voided" ? "is-danger" : "business-documents-center__primary-button"}
              disabled={busy || readOnly || (documentTransitionDraft.status === "voided" && !documentTransitionDraft.reason.trim()) || (documentTransitionDraft.status === "effective" && documentTransitionDraft.mode === "evidence" && !documentTransitionDraft.occurredDate) || (documentTransitionDraft.status === "effective" && documentTransitionDraft.mode === "waiver" && !documentTransitionDraft.reason.trim())}
            >
              {busyAction?.includes(transitionDocument.id) ? <LoaderCircle className="business-documents-center__spin" size={14} /> : documentTransitionDraft.status === "effective" ? <ShieldCheck size={14} /> : <X size={14} />}
              {documentTransitionDraft.status === "effective" ? (documentTransitionDraft.mode === "evidence" ? "选择凭证并生效" : "确认豁免并生效") : "确认作废"}
            </button>
          </footer>
        </form>
      )}

      <section className="business-documents-center__document-ledger">
        <header className="business-documents-center__section-title">
          <div>
            <span>单据与版本</span>
            <strong>报价、合同、请款与验收</strong>
          </div>
          <small>{workspace.documents.length} 份</small>
        </header>
        {orderedDocuments.length === 0 ? (
          <InlineEmpty icon={<FileClock size={20} />} text="还没有商务单据" />
        ) : (
          <div className="business-documents-center__document-list">
            {orderedDocuments.map((document) => {
              const definition = documentDefinition(document.kind);
              const Icon = definition.icon;
              const transitions = allowedDocumentTransitions(document);
              const actionBusy = busyAction?.includes(document.id) ?? false;
              const generateReason = documentGenerateBlockReason(workspace, document);
              const isCurrent = currentDocumentIdForKind(workspace, document.kind) === document.id;
              const quoteConfirmation = document.kind === "quote"
                ? quoteConfirmationForDocument(workspace, document)
                : null;
              return (
                <article key={document.id} className={isCurrent ? "is-current" : ""}>
                  <div className="business-documents-center__document-main">
                    <span className="business-documents-center__document-icon"><Icon size={17} /></span>
                    <div>
                      <span>{definition.label} · 第 {document.sequenceNumber} 版</span>
                      <strong>{document.title}</strong>
                      <small>{document.documentNumber} · {formatDateTime(document.updatedAt)}</small>
                    </div>
                    <div className="business-documents-center__document-statuses">
                      {isCurrent && <em className="is-current">当前有效</em>}
                      {quoteConfirmation && <em className="is-confirmed">客户已确认</em>}
                      <em className={`is-${document.status}`}>{DOCUMENT_STATUS_LABELS[document.status]}</em>
                    </div>
                  </div>
                  {document.outputAssetId && (() => {
                    const assetId = document.outputAssetId;
                    const capabilities = assetActionCapabilities[assetId];
                    return (
                      <div className="business-documents-center__asset-row">
                        <Archive size={13} />
                        <span>正式文件</span>
                        <div className="business-documents-center__asset-actions">
                          <button
                            type="button"
                            onClick={() => onOpenAsset(assetId)}
                            disabled={!capabilities?.canOpen}
                            title={capabilities?.canOpen ? "打开" : capabilities?.reason ?? "当前文件暂时无法打开"}
                            aria-label={`打开${document.title}`}
                          >
                            <FolderOpen size={13} />打开
                          </button>
                          <button
                            type="button"
                            onClick={() => onExportAsset(assetId)}
                            disabled={!capabilities?.canExport}
                            title={capabilities?.canExport ? "导出" : capabilities?.reason ?? "当前文件暂时无法导出"}
                            aria-label={`导出${document.title}`}
                          >
                            <Download size={13} />导出
                          </button>
                        </div>
                      </div>
                    );
                  })()}
                  <div className="business-documents-center__document-actions">
                    {transitions.map((status) => {
                      const blockReason = documentTransitionBlockReason(workspace, document, status);
                      return (
                        <button
                          type="button"
                          key={status}
                          className={status === "approved" || status === "effective" ? "is-primary" : status === "voided" ? "is-danger" : ""}
                          onClick={() => onChangeStatus(document, status)}
                          disabled={busy || readOnly || Boolean(blockReason)}
                          title={blockReason ?? documentTransitionLabel(status)}
                        >
                          {actionBusy ? <LoaderCircle className="business-documents-center__spin" size={13} /> : documentTransitionIcon(status)}
                          {documentTransitionLabel(status)}
                        </button>
                      );
                    })}
                    {document.status === "approved" && (
                      <button
                        type="button"
                        className="is-primary"
                        onClick={() => onGenerate(document)}
                        disabled={busy || readOnly || Boolean(generateReason)}
                        title={generateReason ?? `生成 ${definition.format.toUpperCase()}`}
                      >
                        {actionBusy ? <LoaderCircle className="business-documents-center__spin" size={13} /> : <FileOutput size={13} />}
                        生成 {definition.format.toUpperCase()}
                      </button>
                    )}
                    {document.kind === "quote" && document.status === "generated" && !quoteConfirmation && (
                      <button
                        type="button"
                        className="is-primary"
                        onClick={() => onStartQuoteConfirmation(document)}
                        disabled={busy || readOnly || !isCurrent}
                        title={isCurrent ? "登记客户对当前报价的确认凭证" : "只能确认当前有效报价"}
                      >
                        <CheckCircle2 size={13} />客户确认
                      </button>
                    )}
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </section>

      <section className="business-documents-center__confirmation-ledger">
        <header className="business-documents-center__section-title">
          <div><span>QUOTE CONFIRMATIONS</span><strong>客户报价确认记录</strong></div>
          <small>{workspace.quoteConfirmations.length} 条</small>
        </header>
        {workspace.quoteConfirmations.length === 0 ? (
          <InlineEmpty icon={<ShieldCheck size={20} />} text="当前报价尚未登记客户确认" />
        ) : (
          <div className="business-documents-center__confirmation-list">
            {[...workspace.quoteConfirmations].sort((left, right) => right.confirmedAt - left.confirmedAt).map((confirmation) => {
              const quote = workspace.documents.find((document) => document.id === confirmation.quoteDocumentId);
              const capabilities = assetActionCapabilities[confirmation.evidence.assetId];
              return (
                <article key={confirmation.id}>
                  <span className="business-documents-center__ledger-icon"><CheckCircle2 size={16} /></span>
                  <div>
                    <strong>{confirmation.confirmationVersion}</strong>
                    <small>{quote?.documentNumber ?? "历史报价"} · {confirmation.customerRepresentative} · {formatDate(confirmation.evidence.occurredAt)}</small>
                  </div>
                  <div className="business-documents-center__asset-actions">
                    <button type="button" onClick={() => onOpenAsset(confirmation.evidence.assetId)} disabled={!capabilities?.canOpen} title={capabilities?.canOpen ? "打开确认凭证" : capabilities?.reason ?? "凭证暂时无法打开"}><FolderOpen size={13} />打开</button>
                    <button type="button" onClick={() => onExportAsset(confirmation.evidence.assetId)} disabled={!capabilities?.canExport} title={capabilities?.canExport ? "导出确认凭证" : capabilities?.reason ?? "凭证暂时无法导出"}><Download size={13} />导出</button>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}
function DeliveryView({
  workspace,
  assets,
  busyAction,
  readOnly,
  milestoneDraft,
  deliverableDraft,
  sentDraft,
  signoffDraft,
  milestoneActionAvailable,
  deliverableActionAvailable,
  sentActionAvailable,
  signoffActionAvailable,
  assetActionCapabilities,
  assetImportAvailable,
  onImportAsset,
  onMilestoneDraftChange,
  onDeliverableDraftChange,
  onSentDraftChange,
  onSignoffDraftChange,
  onCreateMilestone,
  onEditMilestone,
  onCreateDeliverable,
  onCreateVersion,
  onStartSending,
  onStartSignoff,
  onSubmitMilestone,
  onSubmitDeliverable,
  onSubmitSent,
  onSubmitSignoff,
  onOpenAsset,
  onExportAsset,
}: {
  workspace: BusinessWorkspaceRecord;
  assets: readonly AssetRecord[];
  busyAction: string | null;
  readOnly: boolean;
  milestoneDraft: BusinessMilestoneInput | null;
  deliverableDraft: DeliverableDraft | null;
  sentDraft: DeliverySentDraft | null;
  signoffDraft: DeliverySignoffDraft | null;
  milestoneActionAvailable: boolean;
  deliverableActionAvailable: boolean;
  sentActionAvailable: boolean;
  signoffActionAvailable: boolean;
  assetActionCapabilities: Readonly<Record<string, AssetActionCapabilities>>;
  assetImportAvailable: boolean;
  onImportAsset: () => Promise<AssetRecord | null>;
  onMilestoneDraftChange: (draft: BusinessMilestoneInput | null) => void;
  onDeliverableDraftChange: (draft: DeliverableDraft | null) => void;
  onSentDraftChange: (draft: DeliverySentDraft | null) => void;
  onSignoffDraftChange: (draft: DeliverySignoffDraft | null) => void;
  onCreateMilestone: () => void;
  onEditMilestone: (milestone: BusinessMilestoneRecord) => void;
  onCreateDeliverable: (milestone: BusinessMilestoneRecord) => void;
  onCreateVersion: (milestone: BusinessMilestoneRecord, deliverable: BusinessDeliverableRecord) => void;
  onStartSending: (milestone: BusinessMilestoneRecord) => void;
  onStartSignoff: (submission: BusinessDeliverySubmissionRecord) => void;
  onSubmitMilestone: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitDeliverable: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitSent: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitSignoff: (event: FormEvent<HTMLFormElement>) => void;
  onOpenAsset: (assetId: string) => void;
  onExportAsset: (assetId: string) => void;
}) {
  const busy = busyAction !== null;
  const submissionsByMilestone = new Map<string, BusinessDeliverySubmissionRecord[]>();
  workspace.deliverySubmissions.forEach((submission) => {
    const current = submissionsByMilestone.get(submission.milestoneId) ?? [];
    current.push(submission);
    submissionsByMilestone.set(submission.milestoneId, current);
  });

  return (
    <div className="business-documents-center__view bdc-delivery-view">
      <header className="bdc-page-head">
        <div><span>DELIVERY</span><h2>交付闭环</h2><small>{workspace.milestones.length} 个里程碑 · {workspace.deliverySubmissions.length} 次发送</small></div>
        <button type="button" className="is-primary" onClick={onCreateMilestone} disabled={busy || readOnly || !milestoneActionAvailable} title={milestoneActionAvailable ? "新建里程碑" : "当前操作不可用"}><Plus size={13} />里程碑</button>
      </header>

      {(milestoneDraft || deliverableDraft || sentDraft || signoffDraft) && (
        <section className="bdc-editor-strip">
          {milestoneDraft && (
            <form onSubmit={onSubmitMilestone}>
              <header><strong>{milestoneDraft.id ? "编辑里程碑" : "新建里程碑"}</strong><button type="button" onClick={() => onMilestoneDraftChange(null)} aria-label="关闭"><X size={14} /></button></header>
              <div className="bdc-form-grid bdc-form-grid--four">
                <Field label="名称"><input value={milestoneDraft.title} onChange={(event) => onMilestoneDraftChange({ ...milestoneDraft, title: event.currentTarget.value })} required /></Field>
                <Field label="计划日期"><input type="date" value={timestampToDateInput(milestoneDraft.dueAt)} onChange={(event) => onMilestoneDraftChange({ ...milestoneDraft, dueAt: dateInputToTimestamp(event.currentTarget.value) })} /></Field>
                <Field label="状态"><select value={milestoneDraft.status} onChange={(event) => onMilestoneDraftChange({ ...milestoneDraft, status: event.currentTarget.value as BusinessMilestoneInput["status"] })}>{MILESTONE_STATUS_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></Field>
                <label className="bdc-check"><input type="checkbox" checked={milestoneDraft.required} onChange={(event) => onMilestoneDraftChange({ ...milestoneDraft, required: event.currentTarget.checked })} />必需</label>
                <Field label="说明" wide><input value={milestoneDraft.description} onChange={(event) => onMilestoneDraftChange({ ...milestoneDraft, description: event.currentTarget.value })} /></Field>
                <Field label="验收标准（必填）" wide><input value={milestoneDraft.acceptanceCriteria} onChange={(event) => onMilestoneDraftChange({ ...milestoneDraft, acceptanceCriteria: event.currentTarget.value })} placeholder="例如：客户确认成片并出具验收单" required /></Field>
              </div>
              <footer><button type="button" onClick={() => onMilestoneDraftChange(null)}>取消</button><button type="submit" className="is-primary" disabled={busy || !milestoneActionAvailable}>{busyAction === "business:milestone:upsert" ? <LoaderCircle className="business-documents-center__spin" size={13} /> : <Save size={13} />}保存</button></footer>
            </form>
          )}

          {deliverableDraft && (
            <form onSubmit={onSubmitDeliverable}>
              <header><strong>{deliverableDraft.deliverableId ? "登记新版本" : "新增交付物"}</strong><button type="button" onClick={() => onDeliverableDraftChange(null)} aria-label="关闭"><X size={14} /></button></header>
              <div className="bdc-form-grid bdc-form-grid--four">
                <Field label="交付物名称"><input value={deliverableDraft.name} onChange={(event) => onDeliverableDraftChange({ ...deliverableDraft, name: event.currentTarget.value })} required /></Field>
                <Field label="交付文件" wide>
                  <VaultFilePicker
                    assets={assets}
                    assetIds={deliverableDraft.assetId ? [deliverableDraft.assetId] : []}
                    disabled={busy || !assetImportAvailable}
                    onChoose={() => {
                      void onImportAsset().then((asset) => {
                        if (asset) onDeliverableDraftChange({ ...deliverableDraft, assetId: asset.id });
                      });
                    }}
                    onClear={() => onDeliverableDraftChange({ ...deliverableDraft, assetId: "" })}
                  />
                </Field>
                <label className="bdc-check"><input type="checkbox" checked={deliverableDraft.required} onChange={(event) => onDeliverableDraftChange({ ...deliverableDraft, required: event.currentTarget.checked })} />必需</label>
                <Field label="版本备注" wide><input value={deliverableDraft.notes} onChange={(event) => onDeliverableDraftChange({ ...deliverableDraft, notes: event.currentTarget.value })} /></Field>
              </div>
              <footer><button type="button" onClick={() => onDeliverableDraftChange(null)}>取消</button><button type="submit" className="is-primary" disabled={busy || !deliverableActionAvailable}>{busyAction === "business:deliverable:register" ? <LoaderCircle className="business-documents-center__spin" size={13} /> : <Save size={13} />}登记版本</button></footer>
            </form>
          )}

          {sentDraft && (
            <form onSubmit={onSubmitSent}>
              <header><strong>登记发送</strong><button type="button" onClick={() => onSentDraftChange(null)} aria-label="关闭"><X size={14} /></button></header>
              <div className="bdc-version-picker">
                {versionsForMilestone(workspace, sentDraft.milestoneId).map((version) => (
                  <label key={version.id}>
                    <input type="checkbox" checked={sentDraft.versionIds.includes(version.id)} onChange={(event) => onSentDraftChange({ ...sentDraft, versionIds: event.currentTarget.checked ? [...sentDraft.versionIds, version.id] : sentDraft.versionIds.filter((id) => id !== version.id) })} />
                    <span>{version.name} · v{version.versionNumber}</span><em>{DELIVERABLE_STATUS_LABELS[version.status]}</em>
                  </label>
                ))}
              </div>
              <div className="bdc-form-grid bdc-form-grid--four">
                <Field label="收件人"><input value={sentDraft.recipient} onChange={(event) => onSentDraftChange({ ...sentDraft, recipient: event.currentTarget.value })} required /></Field>
                <Field label="渠道"><input value={sentDraft.channel} onChange={(event) => onSentDraftChange({ ...sentDraft, channel: event.currentTarget.value })} placeholder="微信 / 邮件 / 线下" required /></Field>
                <Field label="发送日期"><input type="date" value={sentDraft.sentDate} onChange={(event) => onSentDraftChange({ ...sentDraft, sentDate: event.currentTarget.value })} required /></Field>
                <Field label="备注"><input value={sentDraft.note} onChange={(event) => onSentDraftChange({ ...sentDraft, note: event.currentTarget.value })} /></Field>
              </div>
              <footer><button type="button" onClick={() => onSentDraftChange(null)}>取消</button><button type="submit" className="is-primary" disabled={busy || !sentActionAvailable || sentDraft.versionIds.length === 0}><Send size={13} />登记发送</button></footer>
            </form>
          )}

          {signoffDraft && (
            <form onSubmit={onSubmitSignoff}>
              <header><strong>登记客户签收</strong><button type="button" onClick={() => onSignoffDraftChange(null)} aria-label="关闭"><X size={14} /></button></header>
              <div className="bdc-signoff-picker">
                {Object.entries(signoffDraft.decisions).map(([versionId, decision]) => {
                  const version = findDeliverableVersion(workspace, versionId);
                  return (
                    <label key={versionId}>
                      <span>{version ? version.name + " · v" + version.versionNumber : shortId(versionId)}</span>
                      <select value={decision} onChange={(event) => onSignoffDraftChange({ ...signoffDraft, decisions: { ...signoffDraft.decisions, [versionId]: event.currentTarget.value as "accepted" | "rejected" | "pending" } })}>
                        <option value="pending">待确认</option><option value="accepted">接受</option><option value="rejected">拒绝</option>
                      </select>
                    </label>
                  );
                })}
              </div>
              <div className="bdc-form-grid bdc-form-grid--four">
                <Field label="客户代表"><input value={signoffDraft.customerRepresentative} onChange={(event) => onSignoffDraftChange({ ...signoffDraft, customerRepresentative: event.currentTarget.value })} required /></Field>
                <Field label="签收日期"><input type="date" value={signoffDraft.occurredDate} onChange={(event) => onSignoffDraftChange({ ...signoffDraft, occurredDate: event.currentTarget.value })} required /></Field>
                <Field label="签收凭证">
                  <VaultFilePicker
                    assets={assets}
                    assetIds={signoffDraft.evidenceAssetId ? [signoffDraft.evidenceAssetId] : []}
                    disabled={busy || !assetImportAvailable}
                    optional
                    onChoose={() => {
                      void onImportAsset().then((asset) => {
                        if (asset) onSignoffDraftChange({ ...signoffDraft, evidenceAssetId: asset.id });
                      });
                    }}
                    onClear={() => onSignoffDraftChange({ ...signoffDraft, evidenceAssetId: "" })}
                  />
                </Field>
                <Field label="凭证备注"><input value={signoffDraft.evidenceNote} onChange={(event) => onSignoffDraftChange({ ...signoffDraft, evidenceNote: event.currentTarget.value })} /></Field>
                <Field label="签收备注" wide><input value={signoffDraft.note} onChange={(event) => onSignoffDraftChange({ ...signoffDraft, note: event.currentTarget.value })} /></Field>
              </div>
              <footer><button type="button" onClick={() => onSignoffDraftChange(null)}>取消</button><button type="submit" className="is-primary" disabled={busy || !signoffActionAvailable || !Object.values(signoffDraft.decisions).some((decision) => decision !== "pending")} title={Object.values(signoffDraft.decisions).some((decision) => decision !== "pending") ? "保存签收" : "请先对至少一个版本给出接受或拒绝结论"}><ClipboardCheck size={13} />保存签收</button></footer>
            </form>
          )}
        </section>
      )}

      {workspace.milestones.length === 0 ? (
        <CenterEmpty icon={<Boxes size={26} />}><span>先建立第一个交付里程碑</span></CenterEmpty>
      ) : (
        <div className="bdc-milestone-list">
          {[...workspace.milestones].sort((left, right) => left.sequenceNumber - right.sequenceNumber).map((milestone) => {
            const submissions = (submissionsByMilestone.get(milestone.id) ?? []).sort((left, right) => right.submissionNumber - left.submissionNumber);
            return (
              <article className="bdc-milestone" key={milestone.id}>
                <header>
                  <span className={milestone.status === "accepted" ? "is-done" : ""}>{milestone.status === "accepted" ? <Check size={13} /> : milestone.sequenceNumber}</span>
                  <div><strong>{milestone.title}</strong><small>{milestone.required ? "必需" : "可选"} · {formatDate(milestone.dueAt)} · {MILESTONE_STATUS_LABELS[milestone.status]}</small></div>
                  <div className="bdc-row-actions">
                    <button
                      type="button"
                      onClick={() => onEditMilestone(milestone)}
                      disabled={busy || readOnly || !milestoneActionAvailable || milestone.status === "delivered" || milestone.status === "accepted"}
                      title={
                        milestone.status === "delivered" || milestone.status === "accepted"
                          ? "已交付/已签收的里程碑由系统维护，不可再编辑"
                          : "编辑里程碑"
                      }
                    ><FilePenLine size={13} />编辑</button>
                    <button type="button" onClick={() => onCreateDeliverable(milestone)} disabled={busy || readOnly || !deliverableActionAvailable}><Plus size={13} />交付物</button>
                    <button type="button" onClick={() => onStartSending(milestone)} disabled={busy || readOnly || !sentActionAvailable || versionsForMilestone(workspace, milestone.id).length === 0}><Send size={13} />发送</button>
                  </div>
                </header>
                {(milestone.description || milestone.acceptanceCriteria) && <p>{milestone.description}{milestone.acceptanceCriteria ? " · 验收：" + milestone.acceptanceCriteria : ""}</p>}
                <div className="bdc-deliverables">
                  {milestone.deliverables.length === 0 ? <InlineEmpty icon={<PackageCheck size={17} />} text="暂无交付物" /> : milestone.deliverables.map((deliverable) => {
                    const latest = [...deliverable.versions].sort((left, right) => right.versionNumber - left.versionNumber)[0];
                    return (
                      <div className="bdc-deliverable" key={deliverable.id}>
                        <span className="bdc-list-icon"><FileOutput size={14} /></span>
                        <div><strong>{deliverable.name}</strong><small>{deliverable.required ? "必需" : "可选"} · {deliverable.versions.length} 个版本</small></div>
                        <div className="bdc-version-stack">
                          {latest ? <><em className={"is-" + latest.status}>v{latest.versionNumber} · {DELIVERABLE_STATUS_LABELS[latest.status]}</em><code>{shortId(latest.artifact.assetId)}</code></> : <em>无版本</em>}
                        </div>
                        {latest && <AssetMiniActions assetId={latest.artifact.assetId} capabilities={assetActionCapabilities[latest.artifact.assetId]} onOpen={onOpenAsset} onExport={onExportAsset} />}
                        <button type="button" onClick={() => onCreateVersion(milestone, deliverable)} disabled={busy || readOnly || !deliverableActionAvailable}>新版本</button>
                      </div>
                    );
                  })}
                </div>
                {submissions.length > 0 && (
                  <div className="bdc-submission-list">
                    {submissions.map((submission) => (
                      <div key={submission.id}>
                        <span><Send size={13} /></span>
                        <div><strong>第 {submission.submissionNumber} 次发送 · {submission.recipient}</strong><small>{submission.channel} · {formatDateTime(submission.sentAt)} · {submission.versionIds.length} 个版本</small></div>
                        <em className={"is-" + submission.status}>{DELIVERY_STATUS_LABELS[submission.status]}</em>
                        <button
                          type="button"
                          onClick={() => onStartSignoff(submission)}
                          disabled={busy || readOnly || !signoffActionAvailable || undecidedSubmissionVersionIds(submission).length === 0}
                          title={
                            undecidedSubmissionVersionIds(submission).length === 0
                              ? "该批次所有版本已有签收结论；如需继续交付请发送新批次"
                              : "登记客户签收"
                          }
                        >签收</button>
                      </div>
                    ))}
                  </div>
                )}
              </article>
            );
          })}
        </div>
      )}
    </div>
  );
}

function FinanceView({
  workspace,
  assets,
  summary,
  paymentDraft,
  receiptDraft,
  receiptReversalDraft,
  invoiceDraft,
  invoiceReversalDraft,
  invoiceAttachmentDraft,
  busyAction,
  readOnly,
  invoiceActionAvailable,
  invoiceReversalActionAvailable,
  invoiceAttachmentActionAvailable,
  assetActionCapabilities,
  assetImportAvailable,
  onImportAsset,
  onCreatePayment,
  onEditPayment,
  onPaymentDraftChange,
  onReceiptDraftChange,
  onReceiptReversalDraftChange,
  onCancelPayment,
  onCancelReceipt,
  onCancelReceiptReversal,
  onSubmitPayment,
  onSubmitReceipt,
  onSubmitReceiptReversal,
  onAdvancePayment,
  onStartReceipt,
  onStartReceiptReversal,
  onCreateRequest,
  onInvoiceDraftChange,
  onInvoiceReversalDraftChange,
  onInvoiceAttachmentDraftChange,
  onStartInvoice,
  onStartInvoiceReversal,
  onStartInvoiceAttachment,
  onSubmitInvoice,
  onSubmitInvoiceReversal,
  onSubmitInvoiceAttachment,
  onOpenAsset,
  onExportAsset,
}: {
  workspace: BusinessWorkspaceRecord;
  assets: readonly AssetRecord[];
  summary: BusinessWorkspaceSummary;
  paymentDraft: PaymentDraft | null;
  receiptDraft: ReceiptDraft | null;
  receiptReversalDraft: ReceiptReversalDraft | null;
  invoiceDraft: InvoiceDraft | null;
  invoiceReversalDraft: InvoiceReversalDraft | null;
  invoiceAttachmentDraft: InvoiceAttachmentDraft | null;
  busyAction: string | null;
  readOnly: boolean;
  invoiceActionAvailable: boolean;
  invoiceReversalActionAvailable: boolean;
  invoiceAttachmentActionAvailable: boolean;
  assetActionCapabilities: Readonly<Record<string, AssetActionCapabilities>>;
  assetImportAvailable: boolean;
  onImportAsset: () => Promise<AssetRecord | null>;
  onCreatePayment: () => void;
  onEditPayment: (payment: BusinessPaymentRecord) => void;
  onPaymentDraftChange: (draft: PaymentDraft) => void;
  onReceiptDraftChange: (draft: ReceiptDraft) => void;
  onReceiptReversalDraftChange: (draft: ReceiptReversalDraft) => void;
  onCancelPayment: () => void;
  onCancelReceipt: () => void;
  onCancelReceiptReversal: () => void;
  onSubmitPayment: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitReceipt: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitReceiptReversal: (event: FormEvent<HTMLFormElement>) => void;
  onAdvancePayment: (payment: BusinessPaymentRecord, status: BusinessPaymentStatus) => void;
  onStartReceipt: (payment: BusinessPaymentRecord) => void;
  onStartReceiptReversal: (receipt: BusinessReceiptRecord) => void;
  onCreateRequest: (paymentId: string) => void;
  onInvoiceDraftChange: (draft: InvoiceDraft | null) => void;
  onInvoiceReversalDraftChange: (draft: InvoiceReversalDraft | null) => void;
  onInvoiceAttachmentDraftChange: (draft: InvoiceAttachmentDraft | null) => void;
  onStartInvoice: () => void;
  onStartInvoiceReversal: (invoice: BusinessInvoiceRecord) => void;
  onStartInvoiceAttachment: (invoice: BusinessInvoiceRecord) => void;
  onSubmitInvoice: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitInvoiceReversal: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitInvoiceAttachment: (event: FormEvent<HTMLFormElement>) => void;
  onOpenAsset: (assetId: string) => void;
  onExportAsset: (assetId: string) => void;
}) {
  const currency = workspace.profile.currency;
  const invoiceNet = invoiceNetCents(workspace.invoices);
  const busy = busyAction !== null;

  return (
    <div className="business-documents-center__view bdc-finance-view">
      <section className="bdc-metrics bdc-metrics--finance">
        <Metric label="付款计划" value={formatMoney(summary.plannedCents, currency)} detail={workspace.payments.length + " 个节点"} />
        <Metric label="已开票净额" value={formatMoney(invoiceNet, currency)} detail={workspace.invoices.length + " 条记录"} />
        <Metric label="已到账" value={formatMoney(summary.receivedCents, currency)} detail={workspace.receipts.length + " 条流水"} tone="success" />
        <Metric label="待收" value={formatMoney(summary.outstandingCents, currency)} detail={summary.outstandingCents === 0 ? "已结清" : "继续跟进"} tone={summary.outstandingCents > 0 ? "warning" : "success"} />
      </section>

      <section className="bdc-panel bdc-invoice-panel">
        <header className="bdc-section-head">
          <div><span>INVOICES</span><strong>发票</strong></div>
          <button type="button" className="is-primary" onClick={onStartInvoice} disabled={busy || readOnly || !invoiceActionAvailable || workspace.financialSummary.contractCents <= 0} title={workspace.financialSummary.contractCents <= 0 ? "请先将合同确认生效后再登记发票（开票净额不能超过生效合同金额）" : invoiceActionAvailable ? "登记发票" : "当前操作不可用"}><Plus size={13} />登记发票</button>
        </header>

        {(invoiceDraft || invoiceReversalDraft || invoiceAttachmentDraft) && (
          <div className="bdc-editor-strip bdc-editor-strip--inside">
            {invoiceDraft && (
              <form onSubmit={onSubmitInvoice}>
                <header><strong>登记已开票</strong><button type="button" onClick={() => onInvoiceDraftChange(null)} aria-label="关闭"><X size={14} /></button></header>
                <div className="bdc-form-grid bdc-form-grid--four">
                  <Field label="付款节点"><select value={invoiceDraft.paymentId} onChange={(event) => onInvoiceDraftChange({ ...invoiceDraft, paymentId: event.currentTarget.value })}><option value="">不关联</option>{workspace.payments.map((payment) => <option key={payment.id} value={payment.id}>{payment.label}</option>)}</select></Field>
                  <Field label="发票代码"><input value={invoiceDraft.invoiceCode} onChange={(event) => onInvoiceDraftChange({ ...invoiceDraft, invoiceCode: event.currentTarget.value })} required /></Field>
                  <Field label="发票号码"><input value={invoiceDraft.invoiceNumber} onChange={(event) => onInvoiceDraftChange({ ...invoiceDraft, invoiceNumber: event.currentTarget.value })} required /></Field>
                  <Field label="开票日期"><input type="date" value={invoiceDraft.issuedDate} onChange={(event) => onInvoiceDraftChange({ ...invoiceDraft, issuedDate: event.currentTarget.value })} required /></Field>
                  <Field label="价税合计"><input type="number" min="0.01" step="0.01" value={invoiceDraft.amount} onChange={(event) => onInvoiceDraftChange({ ...invoiceDraft, amount: event.currentTarget.value })} required /></Field>
                  <Field label="税额"><input type="number" min="0" step="0.01" value={invoiceDraft.tax} onChange={(event) => onInvoiceDraftChange({ ...invoiceDraft, tax: event.currentTarget.value })} /></Field>
                  <Field label="发票附件" wide>
                    <VaultFilePicker
                      assets={assets}
                      assetIds={splitAssetIds(invoiceDraft.assetIds)}
                      disabled={busy || !assetImportAvailable}
                      multiple
                      onChoose={() => {
                        void onImportAsset().then((asset) => {
                          if (!asset) return;
                          const ids = new Set(splitAssetIds(invoiceDraft.assetIds));
                          ids.add(asset.id);
                          onInvoiceDraftChange({ ...invoiceDraft, assetIds: [...ids].join(",") });
                        });
                      }}
                      onClear={() => onInvoiceDraftChange({ ...invoiceDraft, assetIds: "" })}
                    />
                  </Field>
                </div>
                <footer><button type="button" onClick={() => onInvoiceDraftChange(null)}>取消</button><button type="submit" className="is-primary" disabled={busy || !invoiceActionAvailable || splitAssetIds(invoiceDraft.assetIds).length === 0} title={splitAssetIds(invoiceDraft.assetIds).length === 0 ? "请先选择发票附件（发票必须挂接 Vault 文件）" : "登记发票"}><ReceiptText size={13} />登记</button></footer>
              </form>
            )}
            {invoiceReversalDraft && (
              <form onSubmit={onSubmitInvoiceReversal}>
                <header><strong>登记红冲</strong><button type="button" onClick={() => onInvoiceReversalDraftChange(null)} aria-label="关闭"><X size={14} /></button></header>
                <div className="bdc-form-grid bdc-form-grid--four">
                  <Field label="红票代码"><input value={invoiceReversalDraft.invoiceCode} onChange={(event) => onInvoiceReversalDraftChange({ ...invoiceReversalDraft, invoiceCode: event.currentTarget.value })} required /></Field>
                  <Field label="红票号码"><input value={invoiceReversalDraft.invoiceNumber} onChange={(event) => onInvoiceReversalDraftChange({ ...invoiceReversalDraft, invoiceNumber: event.currentTarget.value })} required /></Field>
                  <Field label="红冲金额">{(() => {
                    const original = workspace.invoices.find((invoice) => invoice.id === invoiceReversalDraft.originalInvoiceId);
                    const remaining = original ? invoiceReversibleCents(workspace, original) : 0;
                    return <input type="number" min="0.01" step="0.01" max={remaining > 0 ? centsToDecimal(remaining) : undefined} title={remaining > 0 ? `可红冲余额 ${centsToDecimal(remaining)}` : undefined} value={invoiceReversalDraft.amount} onChange={(event) => onInvoiceReversalDraftChange({ ...invoiceReversalDraft, amount: event.currentTarget.value })} required />;
                  })()}</Field>
                  <Field label="红冲税额"><input type="number" min="0" step="0.01" value={invoiceReversalDraft.tax} onChange={(event) => onInvoiceReversalDraftChange({ ...invoiceReversalDraft, tax: event.currentTarget.value })} /></Field>
                  <Field label="开票日期"><input type="date" value={invoiceReversalDraft.issuedDate} onChange={(event) => onInvoiceReversalDraftChange({ ...invoiceReversalDraft, issuedDate: event.currentTarget.value })} required /></Field>
                  <Field label="红冲原因"><input value={invoiceReversalDraft.reason} onChange={(event) => onInvoiceReversalDraftChange({ ...invoiceReversalDraft, reason: event.currentTarget.value })} required /></Field>
                  <Field label="红票附件" wide>
                    <VaultFilePicker
                      assets={assets}
                      assetIds={splitAssetIds(invoiceReversalDraft.assetIds)}
                      disabled={busy || !assetImportAvailable}
                      multiple
                      onChoose={() => {
                        void onImportAsset().then((asset) => {
                          if (!asset) return;
                          const ids = new Set(splitAssetIds(invoiceReversalDraft.assetIds));
                          ids.add(asset.id);
                          onInvoiceReversalDraftChange({ ...invoiceReversalDraft, assetIds: [...ids].join(",") });
                        });
                      }}
                      onClear={() => onInvoiceReversalDraftChange({ ...invoiceReversalDraft, assetIds: "" })}
                    />
                  </Field>
                </div>
                <footer><button type="button" onClick={() => onInvoiceReversalDraftChange(null)}>取消</button><button type="submit" className="is-danger" disabled={busy || !invoiceReversalActionAvailable || splitAssetIds(invoiceReversalDraft.assetIds).length === 0} title={splitAssetIds(invoiceReversalDraft.assetIds).length === 0 ? "请先选择红票附件（红冲必须挂接 Vault 文件）" : "确认红冲"}><RotateCcw size={13} />确认红冲</button></footer>
              </form>
            )}
            {invoiceAttachmentDraft && (
              <form onSubmit={onSubmitInvoiceAttachment}>
                <header><strong>补充附件</strong><button type="button" onClick={() => onInvoiceAttachmentDraftChange(null)} aria-label="关闭"><X size={14} /></button></header>
                <div className="bdc-form-grid bdc-form-grid--four">
                  <Field label="附件文件" wide>
                    <VaultFilePicker
                      assets={assets}
                      assetIds={invoiceAttachmentDraft.assetId ? [invoiceAttachmentDraft.assetId] : []}
                      disabled={busy || !assetImportAvailable}
                      onChoose={() => {
                        void onImportAsset().then((asset) => {
                          if (asset) onInvoiceAttachmentDraftChange({ ...invoiceAttachmentDraft, assetId: asset.id });
                        });
                      }}
                      onClear={() => onInvoiceAttachmentDraftChange({ ...invoiceAttachmentDraft, assetId: "" })}
                    />
                  </Field>
                  <Field label="附件角色"><input value={invoiceAttachmentDraft.role} onChange={(event) => onInvoiceAttachmentDraftChange({ ...invoiceAttachmentDraft, role: event.currentTarget.value })} required /></Field>
                </div>
                <footer><button type="button" onClick={() => onInvoiceAttachmentDraftChange(null)}>取消</button><button type="submit" className="is-primary" disabled={busy || !invoiceAttachmentActionAvailable}><Paperclip size={13} />补充</button></footer>
              </form>
            )}
          </div>
        )}

        <div className="bdc-invoice-list">
          {workspace.invoices.length === 0 ? <InlineEmpty icon={<ReceiptText size={18} />} text="暂无发票" /> : [...workspace.invoices].sort((left, right) => right.issuedAt - left.issuedAt).map((invoice) => (
            <article key={invoice.id} className={invoice.kind === "reversal" ? "is-reversal" : ""}>
              <span className="bdc-list-icon">{invoice.kind === "reversal" ? <RotateCcw size={14} /> : <ReceiptText size={14} />}</span>
              <div><strong>{invoice.invoiceNumber}</strong><small>{formatDate(invoice.issuedAt)} · {invoice.kind === "reversal" ? "红冲" : INVOICE_STATUS_LABELS[invoice.status]} · {invoice.artifacts.length} 个附件</small></div>
              <strong className={invoice.kind === "reversal" ? "is-negative" : ""}>{invoice.kind === "reversal" ? "−" : ""}{formatMoney(invoice.amountCents, invoice.currency)}</strong>
              <div className="bdc-row-actions">
                {invoice.artifacts[0] && <AssetMiniActions assetId={invoice.artifacts[0].assetId} capabilities={assetActionCapabilities[invoice.artifacts[0].assetId]} onOpen={onOpenAsset} onExport={onExportAsset} />}
                <button type="button" onClick={() => onStartInvoiceAttachment(invoice)} disabled={busy || readOnly || !invoiceAttachmentActionAvailable}><Paperclip size={12} />附件</button>
                {invoice.kind === "issued" && invoice.status !== "fullyReversed" && <button type="button" onClick={() => onStartInvoiceReversal(invoice)} disabled={busy || readOnly || !invoiceReversalActionAvailable}>红冲</button>}
              </div>
            </article>
          ))}
        </div>
      </section>

      <PaymentsView
        workspace={workspace}
        summary={summary}
        draft={paymentDraft}
        receiptDraft={receiptDraft}
        receiptReversalDraft={receiptReversalDraft}
        busyAction={busyAction}
        readOnly={readOnly}
        assetActionCapabilities={assetActionCapabilities}
        onCreate={onCreatePayment}
        onEdit={onEditPayment}
        onDraftChange={onPaymentDraftChange}
        onReceiptDraftChange={onReceiptDraftChange}
        onReceiptReversalDraftChange={onReceiptReversalDraftChange}
        onCancelDraft={onCancelPayment}
        onCancelReceipt={onCancelReceipt}
        onCancelReceiptReversal={onCancelReceiptReversal}
        onSubmitDraft={onSubmitPayment}
        onSubmitReceipt={onSubmitReceipt}
        onSubmitReceiptReversal={onSubmitReceiptReversal}
        onAdvance={onAdvancePayment}
        onStartReceipt={onStartReceipt}
        onStartReceiptReversal={onStartReceiptReversal}
        onCreateRequest={onCreateRequest}
        onOpenAsset={onOpenAsset}
        onExportAsset={onExportAsset}
      />
    </div>
  );
}

function ArchiveView({
  workspace,
  checks,
  busyAction,
  snapshotActionAvailable,
  archiveBlockReason,
  onCreateSnapshot,
  onArchive,
  onReopen,
  onOpenAsset,
  onExportAsset,
}: {
  workspace: BusinessWorkspaceRecord;
  checks: readonly ArchivePreflightItem[];
  busyAction: string | null;
  snapshotActionAvailable: boolean;
  archiveBlockReason: string | null;
  onCreateSnapshot: () => void;
  onArchive: () => void;
  onReopen: () => void;
  onOpenAsset: (assetId: string) => void;
  onExportAsset: (assetId: string) => void;
}) {
  const latestSnapshot = latestArchiveSnapshot(workspace.archiveSnapshots);
  const preflightPassed = checks.every((item) => item.passed);
  const snapshotFresh = workspace.archiveIntegrityStatus === "ready"
    && latestSnapshot !== null
    && latestSnapshot.capturedWorkspaceRevision + 1 === workspace.revision
    && latestSnapshot.capturedCustomerRevision === workspace.customer.revision;
  const busy = busyAction !== null;

  return (
    <div className="business-documents-center__view bdc-archive-view">
      <header className="bdc-page-head">
        <div><span>ARCHIVE</span><h2>归档</h2><small>预检 → 快照 → 归档</small></div>
        <span className={"bdc-integrity is-" + workspace.archiveIntegrityStatus}>{archiveIntegrityLabel(workspace.archiveIntegrityStatus)}</span>
      </header>

      <section className="bdc-archive-steps" aria-label="归档步骤">
        <div className={preflightPassed ? "is-done" : "is-current"}><span>{preflightPassed ? <Check size={13} /> : "1"}</span><strong>预检</strong></div>
        <ChevronRight size={14} />
        <div className={snapshotFresh ? "is-done" : preflightPassed ? "is-current" : ""}><span>{snapshotFresh ? <Check size={13} /> : "2"}</span><strong>生成快照</strong></div>
        <ChevronRight size={14} />
        <div className={workspace.status === "archived" ? "is-done" : snapshotFresh ? "is-current" : ""}><span>{workspace.status === "archived" ? <Check size={13} /> : "3"}</span><strong>归档</strong></div>
      </section>

      <div className="bdc-archive-grid">
        <section className="bdc-panel">
          <header className="bdc-section-head"><div><span>PREFLIGHT</span><strong>归档预检</strong></div><small>{checks.filter((item) => item.passed).length}/{checks.length}</small></header>
          <div className="bdc-check-list">
            {checks.map((item) => (
              <article key={item.id} className={item.passed ? "is-passed" : "is-blocked"}>
                <span>{item.passed ? <Check size={13} /> : <CircleAlert size={13} />}</span>
                <div><strong>{item.label}</strong><small>{item.detail}</small></div>
              </article>
            ))}
          </div>
        </section>

        <section className="bdc-panel bdc-snapshot-panel">
          <header className="bdc-section-head"><div><span>SNAPSHOT</span><strong>完整性快照</strong></div></header>
          {latestSnapshot ? (
            <div className="bdc-snapshot-card">
              <span className="bdc-snapshot-icon"><ShieldCheck size={20} /></span>
              <div><strong>{latestSnapshot.entries.length} 个文件</strong><small>资料版本 {latestSnapshot.capturedWorkspaceRevision} · 客户版本 {latestSnapshot.capturedCustomerRevision}</small></div>
              <code title={latestSnapshot.manifestSha256}>{latestSnapshot.manifestSha256}</code>
              <time>{formatDateTime(latestSnapshot.generatedAt)}</time>
              <div className="bdc-row-actions">
                {latestSnapshot.manifestAssetId && <><button type="button" onClick={() => onOpenAsset(latestSnapshot.manifestAssetId!)}><FolderOpen size={12} />清单</button><button type="button" onClick={() => onExportAsset(latestSnapshot.manifestAssetId!)}><Download size={12} />导出</button></>}
                {latestSnapshot.packageAssetId && <><button type="button" onClick={() => onOpenAsset(latestSnapshot.packageAssetId!)}><FolderOpen size={12} />归档包</button><button type="button" onClick={() => onExportAsset(latestSnapshot.packageAssetId!)}><Download size={12} />导出</button></>}
              </div>
            </div>
          ) : (
            <InlineEmpty icon={<FileArchive size={20} />} text="尚未生成快照" />
          )}

          <div className="bdc-archive-actions">
            {workspace.status === "archived" ? (
              <button type="button" className="is-primary" onClick={onReopen} disabled={busy}><RotateCcw size={13} />重新打开</button>
            ) : !snapshotFresh ? (
              <button type="button" className="is-primary" onClick={onCreateSnapshot} disabled={busy || !preflightPassed || !snapshotActionAvailable} title={!snapshotActionAvailable ? "快照能力不可用" : preflightPassed ? "读取 Vault 并生成归档快照" : "先完成预检"}>
                {busyAction === "business:archive:snapshot" ? <LoaderCircle className="business-documents-center__spin" size={13} /> : <ShieldCheck size={13} />}
                生成快照
              </button>
            ) : (
              <>
                <button type="button" onClick={onCreateSnapshot} disabled={busy || !snapshotActionAvailable} title={snapshotActionAvailable ? "重新读取 Vault 并生成快照" : "快照能力不可用"}><ShieldCheck size={13} />重新生成</button>
                <button type="button" className="is-primary" onClick={onArchive} disabled={busy || Boolean(archiveBlockReason)} title={archiveBlockReason ?? "归档并转为只读"}><Archive size={13} />归档</button>
              </>
            )}
          </div>
          {archiveBlockReason && workspace.status !== "archived" && <p className="bdc-block-reason"><CircleAlert size={13} />{archiveBlockReason}</p>}
        </section>
      </div>

      {workspace.archiveSnapshots.length > 0 && (
        <section className="bdc-panel">
          <header className="bdc-section-head"><div><span>HISTORY</span><strong>快照历史</strong></div></header>
          <div className="bdc-snapshot-history">
            {[...workspace.archiveSnapshots].sort((left, right) => right.generatedAt - left.generatedAt).map((snapshot) => (
              <article key={snapshot.id}><FileArchive size={14} /><div><strong>{formatDateTime(snapshot.generatedAt)}</strong><small>{snapshot.entries.length} 项 · 版本 {snapshot.capturedWorkspaceRevision}</small></div><code>{shortId(snapshot.manifestSha256, 12)}</code></article>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

function AssetMiniActions({ assetId, capabilities, onOpen, onExport }: {
  assetId: string;
  capabilities: AssetActionCapabilities | undefined;
  onOpen: (assetId: string) => void;
  onExport: (assetId: string) => void;
}) {
  return (
    <div className="bdc-asset-actions">
      <button type="button" onClick={() => onOpen(assetId)} disabled={!capabilities?.canOpen} title={capabilities?.canOpen ? "打开" : capabilities?.reason ?? "不可打开"}><FolderOpen size={12} /></button>
      <button type="button" onClick={() => onExport(assetId)} disabled={!capabilities?.canExport} title={capabilities?.canExport ? "导出" : capabilities?.reason ?? "不可导出"}><Download size={12} /></button>
    </div>
  );
}

function PaymentsView({
  workspace,
  summary,
  draft,
  receiptDraft,
  receiptReversalDraft,
  busyAction,
  readOnly,
  assetActionCapabilities,
  onCreate,
  onEdit,
  onDraftChange,
  onReceiptDraftChange,
  onReceiptReversalDraftChange,
  onCancelDraft,
  onCancelReceipt,
  onCancelReceiptReversal,
  onSubmitDraft,
  onSubmitReceipt,
  onSubmitReceiptReversal,
  onAdvance,
  onStartReceipt,
  onStartReceiptReversal,
  onCreateRequest,
  onOpenAsset,
  onExportAsset,
}: {
  workspace: BusinessWorkspaceRecord;
  summary: BusinessWorkspaceSummary;
  draft: PaymentDraft | null;
  receiptDraft: ReceiptDraft | null;
  receiptReversalDraft: ReceiptReversalDraft | null;
  busyAction: string | null;
  readOnly: boolean;
  assetActionCapabilities: Readonly<Record<string, AssetActionCapabilities>>;
  onCreate: () => void;
  onEdit: (payment: BusinessPaymentRecord) => void;
  onDraftChange: (draft: PaymentDraft) => void;
  onReceiptDraftChange: (draft: ReceiptDraft) => void;
  onReceiptReversalDraftChange: (draft: ReceiptReversalDraft) => void;
  onCancelDraft: () => void;
  onCancelReceipt: () => void;
  onCancelReceiptReversal: () => void;
  onSubmitDraft: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitReceipt: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitReceiptReversal: (event: FormEvent<HTMLFormElement>) => void;
  onAdvance: (payment: BusinessPaymentRecord, status: BusinessPaymentStatus) => void;
  onStartReceipt: (payment: BusinessPaymentRecord) => void;
  onStartReceiptReversal: (receipt: BusinessReceiptRecord) => void;
  onCreateRequest: (paymentId: string) => void;
  onOpenAsset: (assetId: string) => void;
  onExportAsset: (assetId: string) => void;
}) {
  const busy = busyAction !== null;
  const payments = [...workspace.payments].sort(
    (left, right) => (left.dueAt ?? Infinity) - (right.dueAt ?? Infinity),
  );
  const receiptPayment = receiptDraft
    ? workspace.payments.find((payment) => payment.id === receiptDraft.paymentId) ?? null
    : null;
  const receiptDraftReason = receiptDraft
    ? receiptBlockReason(workspace, receiptPayment)
    : null;
  const reversalReceipt = receiptReversalDraft
    ? workspace.receipts.find((receipt) => receipt.id === receiptReversalDraft.receiptId) ?? null
    : null;
  const reversalRemaining = reversalReceipt
    ? reversibleReceiptCents(workspace, reversalReceipt)
    : 0;

  return (
    <div className="business-documents-center__view business-documents-center__payments-view">
      {readOnly && <InlineNotice text="工作区已归档，付款计划、请款状态与到账凭证均为只读。" />}

      <section className="business-documents-center__payment-summary">
        <Metric label="计划金额" value={formatMoney(summary.plannedCents, workspace.profile.currency)} detail={`${workspace.payments.length} 个节点`} />
        <Metric label="已请款" value={formatMoney(summary.requestedCents, workspace.profile.currency)} detail="由正式请款单推进" />
        <Metric label="已到账" value={formatMoney(summary.receivedCents, workspace.profile.currency)} detail="累计到账" tone="success" />
        <Metric label="待回款" value={formatMoney(summary.outstandingCents, workspace.profile.currency)} detail="以生效合同为准" tone={summary.outstandingCents > 0 ? "warning" : "default"} />
        <button type="button" onClick={onCreate} disabled={busy || readOnly} title={readOnly ? "归档工作区不可新增付款计划" : "新增付款计划"}>
          <Plus size={15} />新增付款计划
        </button>
      </section>

      <div className="business-documents-center__flow-note">
        <FileCheck2 size={16} />
        <div>
          <strong>正式请款路径</strong>
          <span>付款计划 → 请款单草稿 → 审批 → 生成正式请款单 → 系统推进为已请款 → 登记到账。</span>
        </div>
      </div>

      {draft && (
        <form className="business-documents-center__composer" onSubmit={onSubmitDraft}>
          <header className="business-documents-center__section-title">
            <div>
              <span>PAYMENT</span>
              <strong>{draft.id ? "编辑付款计划" : "新增付款计划"}</strong>
            </div>
            <button type="button" onClick={onCancelDraft} title="关闭" aria-label="关闭付款编辑"><X size={15} /></button>
          </header>
          <div className="business-documents-center__composer-fields">
            <Field label="节点名称">
              <input
                value={draft.label}
                onChange={(event) => onDraftChange({ ...draft, label: event.currentTarget.value })}
                required
                disabled={readOnly}
              />
            </Field>
            <Field label={`金额（${workspace.profile.currency}）`}>
              <input
                type="number"
                min="0.01"
                step="0.01"
                value={draft.amount}
                onChange={(event) => onDraftChange({ ...draft, amount: event.currentTarget.value })}
                required
                disabled={readOnly}
              />
            </Field>
            <Field label="计划日期">
              <input
                type="date"
                value={draft.dueDate}
                onChange={(event) => onDraftChange({ ...draft, dueDate: event.currentTarget.value })}
                disabled={readOnly}
              />
            </Field>
            <Field label="业务状态">
              <div className="business-documents-center__readonly-field">
                付款计划（正式请款后由系统自动推进）
              </div>
            </Field>
            <Field label="客户 PO / 参考号">
              <input
                value={draft.reference}
                onChange={(event) => onDraftChange({ ...draft, reference: event.currentTarget.value })}
                disabled={readOnly}
                placeholder="可选"
              />
            </Field>
            <Field label="备注" wide>
              <textarea
                value={draft.notes}
                onChange={(event) => onDraftChange({ ...draft, notes: event.currentTarget.value })}
                rows={3}
                disabled={readOnly}
              />
            </Field>
          </div>
          <footer>
            <span>{draft.id ? "正在编辑付款记录" : "保存后可继续请款与回款跟进"}</span>
            <button
              type="submit"
              className="business-documents-center__primary-button"
              disabled={
                busy ||
                readOnly ||
                !draft.label.trim() ||
                decimalToCents(draft.amount) <= 0
              }
              title="保存付款计划"
            >
              {busyAction?.startsWith("business:payment") ? <LoaderCircle className="business-documents-center__spin" size={14} /> : <Save size={14} />}
              保存付款计划
            </button>
          </footer>
        </form>
      )}

      {receiptDraft && receiptPayment && (
        <form className="business-documents-center__composer is-receipt" onSubmit={onSubmitReceipt}>
          <header className="business-documents-center__section-title">
            <div><span>RECEIPT</span><strong>登记到账流水</strong></div>
            <button type="button" onClick={onCancelReceipt} title="关闭" aria-label="关闭到账登记"><X size={15} /></button>
          </header>
          {receiptDraftReason && <InlineNotice tone="warning" text={receiptDraftReason} />}
          <div className="business-documents-center__composer-fields">
            <Field label="付款节点">
              <div className="business-documents-center__readonly-field">{receiptPayment.label}</div>
            </Field>
            <Field label={`本次到账（${workspace.profile.currency}）`}>
              <input type="number" min="0.01" max={centsToDecimal(outstandingPaymentCents(workspace, receiptPayment))} step="0.01" value={receiptDraft.amount} onChange={(event) => onReceiptDraftChange({ ...receiptDraft, amount: event.currentTarget.value })} required />
            </Field>
            <Field label="到账日期">
              <input type="date" value={receiptDraft.occurredDate} onChange={(event) => onReceiptDraftChange({ ...receiptDraft, occurredDate: event.currentTarget.value })} required />
            </Field>
            <Field label="银行流水 / 凭证号">
              <input value={receiptDraft.reference} onChange={(event) => onReceiptDraftChange({ ...receiptDraft, reference: event.currentTarget.value })} required placeholder="全局不可重复" />
            </Field>
            <Field label="备注" wide>
              <textarea value={receiptDraft.notes} onChange={(event) => onReceiptDraftChange({ ...receiptDraft, notes: event.currentTarget.value })} rows={3} />
            </Field>
            <label className="business-documents-center__checkbox is-wide">
              <input type="checkbox" checked={receiptDraft.includeEvidence} onChange={(event) => onReceiptDraftChange({ ...receiptDraft, includeEvidence: event.currentTarget.checked })} />
              <span><strong>附到账凭证</strong><small>提交时选择银行回单或客户付款截图，建议保留。</small></span>
            </label>
          </div>
          <footer>
            <span>允许分次到账；累计金额不能超过付款节点金额</span>
            <button type="submit" className="business-documents-center__primary-button" disabled={busy || readOnly || Boolean(receiptDraftReason) || decimalToCents(receiptDraft.amount) <= 0 || decimalToCents(receiptDraft.amount) > outstandingPaymentCents(workspace, receiptPayment) || !receiptDraft.occurredDate || !receiptDraft.reference.trim()} title={receiptDraftReason ?? "登记到账流水"}>
              {busyAction?.includes(receiptPayment.id) ? <LoaderCircle className="business-documents-center__spin" size={14} /> : <CheckCircle2 size={14} />}
              {receiptDraft.includeEvidence ? "选择凭证并登记" : "登记到账"}
            </button>
          </footer>
        </form>
      )}

      {receiptReversalDraft && reversalReceipt && (
        <form className="business-documents-center__composer is-reversal" onSubmit={onSubmitReceiptReversal}>
          <header className="business-documents-center__section-title">
            <div><span>REVERSAL</span><strong>冲销到账流水</strong></div>
            <button type="button" onClick={onCancelReceiptReversal} title="关闭" aria-label="关闭到账冲销"><X size={15} /></button>
          </header>
          <InlineNotice tone="warning" text="冲销不会删除原始流水；系统会新增一条反向记录并重算付款节点状态。" />
          <div className="business-documents-center__composer-fields">
            <Field label="原始流水">
              <div className="business-documents-center__readonly-field">{reversalReceipt.reference}</div>
            </Field>
            <Field label={`冲销金额（${workspace.profile.currency}）`}>
              <input type="number" min="0.01" max={centsToDecimal(reversalRemaining)} step="0.01" value={receiptReversalDraft.amount} onChange={(event) => onReceiptReversalDraftChange({ ...receiptReversalDraft, amount: event.currentTarget.value })} required />
            </Field>
            <Field label="冲销日期">
              <input type="date" min={timestampToDateInput(reversalReceipt.occurredAt)} value={receiptReversalDraft.occurredDate} onChange={(event) => onReceiptReversalDraftChange({ ...receiptReversalDraft, occurredDate: event.currentTarget.value })} required />
            </Field>
            <Field label="冲销凭证号">
              <input value={receiptReversalDraft.reference} onChange={(event) => onReceiptReversalDraftChange({ ...receiptReversalDraft, reference: event.currentTarget.value })} required placeholder="全局不可重复" />
            </Field>
            <Field label="冲销原因" wide>
              <textarea value={receiptReversalDraft.reason} onChange={(event) => onReceiptReversalDraftChange({ ...receiptReversalDraft, reason: event.currentTarget.value })} rows={3} required />
            </Field>
          </div>
          <footer>
            <span>剩余可冲销 {formatMoney(reversalRemaining, workspace.profile.currency)}</span>
            <button type="submit" className="is-danger" disabled={busy || readOnly || decimalToCents(receiptReversalDraft.amount) <= 0 || decimalToCents(receiptReversalDraft.amount) > reversalRemaining || !receiptReversalDraft.occurredDate || !receiptReversalDraft.reference.trim() || !receiptReversalDraft.reason.trim()}>
              {busyAction?.includes(reversalReceipt.id) ? <LoaderCircle className="business-documents-center__spin" size={14} /> : <RotateCcw size={14} />}
              确认冲销
            </button>
          </footer>
        </form>
      )}

      <section className="business-documents-center__payment-ledger">
        <header className="business-documents-center__section-title">
          <div><span>PAYMENT LEDGER</span><strong>付款计划、正式请款与到账</strong></div>
          <small>{workspace.payments.length} 笔记录</small>
        </header>
        {payments.length === 0 ? (
          <InlineEmpty icon={<Landmark size={20} />} text="还没有付款或回款记录" />
        ) : (
          <div className="business-documents-center__payment-list">
            {payments.map((payment) => {
              const actionBusy = busyAction?.includes(payment.id) ?? false;
              const requestReason = paymentRequestBlockReason(workspace, payment);
              const receiveReason = receiptBlockReason(workspace, payment);
              const requestDocument = generatedPaymentRequestForPayment(workspace, payment.id);
              return (
                <article key={payment.id}>
                  <div className="business-documents-center__payment-state">
                    {paymentStatusIcon(payment.status)}
                    <span className={`is-${payment.status}`}>{PAYMENT_STATUS_LABELS[payment.status]}</span>
                  </div>
                  <div className="business-documents-center__payment-copy">
                    <strong>{payment.label}</strong>
                    <small>计划 {formatDate(payment.dueAt)}{payment.occurredAt ? ` · 到账 ${formatDate(payment.occurredAt)}` : ""}</small>
                    {requestDocument && (
                      <span className="business-documents-center__request-proof">
                        <FileCheck2 size={12} />正式请款单 {requestDocument.documentNumber}
                      </span>
                    )}
                    {payment.reference && <code>{payment.reference}</code>}
                  </div>
                  <strong className="business-documents-center__payment-amount">{formatMoney(payment.amountCents, workspace.profile.currency)}</strong>
                  <div className="business-documents-center__payment-actions">
                    {(() => {
                      const capturedBy = workspace.documents.find(
                        (document) =>
                          document.kind === "paymentRequest" &&
                          document.status !== "voided" &&
                          document.snapshot.payment?.id === payment.id,
                      );
                      const editBlocked = readOnly
                        ? "归档工作区不可编辑"
                        : capturedBy
                          ? `已被请款单 ${capturedBy.documentNumber} 捕获，需先作废该请款单才能编辑`
                          : null;
                      const cancelBlocked = readOnly
                        ? "归档工作区不可取消"
                        : payment.status === "requested" && capturedBy
                          ? `已被请款单 ${capturedBy.documentNumber} 捕获，需先作废该请款单才能取消`
                          : null;
                      return (
                        <>
                          {payment.status === "planned" && (
                            <button type="button" onClick={() => onEdit(payment)} disabled={busy || readOnly || Boolean(capturedBy)} title={editBlocked ?? "编辑付款计划"}>
                              <FilePenLine size={13} />编辑
                            </button>
                          )}
                          {payment.status === "planned" && (
                            <button
                              type="button"
                              className="is-primary"
                              onClick={() => onCreateRequest(payment.id)}
                              disabled={busy || readOnly || Boolean(requestReason)}
                              title={requestReason ?? "创建正式请款单"}
                            >
                              <FilePlus2 size={13} />请款单
                            </button>
                          )}
                          {(payment.status === "requested" || payment.status === "partiallyReceived") && (
                            <button
                              type="button"
                              className="is-primary"
                              onClick={() => onStartReceipt(payment)}
                              disabled={busy || readOnly || Boolean(receiveReason)}
                              title={receiveReason ?? "登记到账"}
                            >
                              {actionBusy ? <LoaderCircle className="business-documents-center__spin" size={13} /> : <CheckCircle2 size={13} />}
                              登记到账
                            </button>
                          )}
                          {(payment.status === "planned" || payment.status === "requested") && (
                            <button
                              type="button"
                              className="is-danger"
                              onClick={() => onAdvance(payment, "canceled")}
                              disabled={busy || readOnly || Boolean(cancelBlocked)}
                              title={cancelBlocked ?? "取消付款节点"}
                            >
                              <X size={13} />取消
                            </button>
                          )}
                        </>
                      );
                    })()}
                  </div>
                  {(payment.status === "planned" ? requestReason : payment.status === "requested" ? receiveReason : null) && (
                    <div className="business-documents-center__blocked-reason">
                      <CircleAlert size={13} />
                      {payment.status === "planned" ? requestReason : receiveReason}
                    </div>
                  )}
                </article>
              );
            })}
          </div>
        )}
      </section>

      <section className="business-documents-center__receipt-ledger">
        <header className="business-documents-center__section-title">
          <div><span>RECEIPT LEDGER</span><strong>到账与冲销流水</strong></div>
          <small>{workspace.receipts.length} 条</small>
        </header>
        {workspace.receipts.length === 0 ? (
          <InlineEmpty icon={<Landmark size={20} />} text="还没有到账流水" />
        ) : (
          <div className="business-documents-center__receipt-list">
            {[...workspace.receipts].sort((left, right) => right.createdAt - left.createdAt).map((receipt) => {
              const payment = workspace.payments.find((candidate) => candidate.id === receipt.paymentId);
              const evidenceId = receipt.evidence?.assetId ?? null;
              const capabilities = evidenceId ? assetActionCapabilities[evidenceId] : undefined;
              const reversibleCents = reversibleReceiptCents(workspace, receipt);
              const actionBusy = busyAction?.includes(receipt.id) ?? false;
              return (
                <article key={receipt.id} className={receipt.kind === "reversal" ? "is-reversal" : ""}>
                  <span className="business-documents-center__ledger-icon">{receipt.kind === "receipt" ? <CheckCircle2 size={16} /> : <RotateCcw size={16} />}</span>
                  <div>
                    <strong>{payment?.label ?? "历史付款节点"}</strong>
                    <small>{formatDate(receipt.occurredAt)} · {receipt.reference}{receipt.kind === "reversal" ? " · 冲销" : ""}</small>
                    {receipt.notes && <span>{receipt.notes}</span>}
                  </div>
                  <strong className={receipt.kind === "reversal" ? "is-negative" : ""}>{receipt.kind === "reversal" ? "-" : "+"}{formatMoney(receipt.amountCents, workspace.profile.currency)}</strong>
                  <div className="business-documents-center__asset-actions">
                    {evidenceId && <button type="button" onClick={() => onOpenAsset(evidenceId)} disabled={!capabilities?.canOpen} title={capabilities?.canOpen ? "打开到账凭证" : capabilities?.reason ?? "凭证暂时无法打开"}><FolderOpen size={13} />凭证</button>}
                    {evidenceId && <button type="button" onClick={() => onExportAsset(evidenceId)} disabled={!capabilities?.canExport} title={capabilities?.canExport ? "导出到账凭证" : capabilities?.reason ?? "凭证暂时无法导出"}><Download size={13} />导出</button>}
                    {receipt.kind === "receipt" && reversibleCents > 0 && <button type="button" className="is-danger" onClick={() => onStartReceiptReversal(receipt)} disabled={busy || readOnly} title="保留原流水并新增冲销记录">{actionBusy ? <LoaderCircle className="business-documents-center__spin" size={13} /> : <RotateCcw size={13} />}冲销</button>}
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}

export function businessAssetDisplayName(
  assets: readonly AssetRecord[],
  assetId: string,
): string {
  return assets.find((asset) => asset.id === assetId)?.originalName || "已选择文件";
}

function VaultFilePicker({
  assets,
  assetIds,
  disabled,
  multiple = false,
  optional = false,
  onChoose,
  onClear,
}: {
  assets: readonly AssetRecord[];
  assetIds: readonly string[];
  disabled: boolean;
  multiple?: boolean;
  optional?: boolean;
  onChoose: () => void;
  onClear: () => void;
}) {
  return (
    <div className="bdc-vault-picker">
      <div className="bdc-vault-picker__selection">
        {assetIds.length === 0 ? (
          <span>{optional ? "未添加凭证" : "尚未选择文件"}</span>
        ) : (
          assetIds.map((assetId) => (
            <span key={assetId} title={businessAssetDisplayName(assets, assetId)}>
              <Paperclip size={12} />
              {businessAssetDisplayName(assets, assetId)}
            </span>
          ))
        )}
      </div>
      <div className="bdc-vault-picker__actions">
        <button type="button" onClick={onChoose} disabled={disabled}>
          <FilePlus2 size={13} />
          {multiple && assetIds.length > 0 ? "继续添加" : "选择文件"}
        </button>
        {assetIds.length > 0 && (
          <button type="button" onClick={onClear} disabled={disabled} title="清空已选文件" aria-label="清空已选文件">
            <X size={13} />
          </button>
        )}
      </div>
    </div>
  );
}

function Field({ label, wide = false, children }: { label: string; wide?: boolean; children: ReactNode }) {
  return <label className={wide ? "is-wide" : ""}><span>{label}</span>{children}</label>;
}

function Metric({ label, value, detail, tone = "default" }: { label: string; value: string; detail: string; tone?: "default" | "success" | "warning" }) {
  return <article className={`business-documents-center__metric is-${tone}`}><span>{label}</span><strong>{value}</strong><small>{detail}</small></article>;
}

interface WorkspaceBootstrapProps {
  projectId: string;
  projectTitle: string;
  busy: boolean;
  busyAction: string | null;
  isDesktopRuntime: boolean;
  onCreate: (prefillSourceWorkspaceId: string | null) => Promise<boolean>;
  onListCandidates: (
    projectId: string,
  ) => Promise<readonly BusinessWorkspacePrefillCandidate[]>;
  onPreview: (
    projectId: string,
    sourceWorkspaceId: string,
  ) => Promise<BusinessWorkspacePrefillPreview>;
}

function WorkspaceBootstrap({
  projectId,
  projectTitle,
  busy,
  busyAction,
  isDesktopRuntime,
  onCreate,
  onListCandidates,
  onPreview,
}: WorkspaceBootstrapProps) {
  const [candidates, setCandidates] = useState<
    readonly BusinessWorkspacePrefillCandidate[] | null
  >(null);
  const [candidatesLoading, setCandidatesLoading] = useState(false);
  const [selectedSourceId, setSelectedSourceId] = useState<string | null>(null);
  const [preview, setPreview] = useState<BusinessWorkspacePrefillPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [prefillError, setPrefillError] = useState<string | null>(null);
  const listCandidatesRef = useRef(onListCandidates);
  listCandidatesRef.current = onListCandidates;
  const previewRef = useRef(onPreview);
  previewRef.current = onPreview;

  useEffect(() => {
    setCandidates(null);
    setSelectedSourceId(null);
    setPreview(null);
    setPrefillError(null);
    if (!isDesktopRuntime) return;
    let cancelled = false;
    setCandidatesLoading(true);
    listCandidatesRef
      .current(projectId)
      .then((result) => {
        if (!cancelled) setCandidates(result);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setCandidates([]);
          setPrefillError(error instanceof Error ? error.message : String(error));
        }
      })
      .finally(() => {
        if (!cancelled) setCandidatesLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [projectId, isDesktopRuntime]);

  const selectCandidate = (candidate: BusinessWorkspacePrefillCandidate) => {
    if (selectedSourceId === candidate.sourceWorkspaceId) {
      setSelectedSourceId(null);
      setPreview(null);
      return;
    }
    setSelectedSourceId(candidate.sourceWorkspaceId);
    setPreview(null);
    setPrefillError(null);
    setPreviewLoading(true);
    previewRef
      .current(projectId, candidate.sourceWorkspaceId)
      .then((result) => {
        setPreview((current) => result.sourceWorkspaceId === candidate.sourceWorkspaceId ? result : current);
      })
      .catch((error: unknown) => {
        setPrefillError(error instanceof Error ? error.message : String(error));
      })
      .finally(() => setPreviewLoading(false));
  };

  const creating = busyAction === "business:create-workspace";
  const previewSummary = preview ? summarizePrefillChanges(preview.changes) : null;
  const sortedChanges = preview
    ? [...preview.changes].sort((a, b) => {
        const rank = (change: BusinessWorkspacePrefillChange) =>
          change.decision === "filled" || change.decision === "replaced" ? 0 : 1;
        return rank(a) - rank(b);
      })
    : [];

  return (
    <div className="bdc-bootstrap">
      <div className="bdc-bootstrap__intro">
        <WalletCards size={26} />
        <strong>「{projectTitle}」还没有商务档案</strong>
        <span>建立后即可维护客户资料、报价、合同、请款、验收与回款闭环。</span>
      </div>
      <button
        type="button"
        className="business-documents-center__empty-action"
        onClick={() => void onCreate(null)}
        disabled={!isDesktopRuntime || busy}
      >
        {creating && !selectedSourceId ? "正在准备…" : "建立空白商务档案"}
      </button>

      {candidatesLoading && (
        <div className="bdc-bootstrap__hint">
          <LoaderCircle size={13} className="business-documents-center__spin" />
          正在查找可复用的老客户资料…
        </div>
      )}

      {candidates !== null && candidates.length > 0 && (
        <section className="bdc-bootstrap__prefill" aria-label="老客户资料复用">
          <header>
            <strong>老客户资料复用</strong>
            <small>
              找到 {candidates.length} 个同客户历史项目，可带入公司、税号、银行等资料（只填空缺项，不覆盖）
            </small>
          </header>
          <ul>
            {candidates.map((candidate) => (
              <li key={candidate.sourceWorkspaceId}>
                <button
                  type="button"
                  className={
                    selectedSourceId === candidate.sourceWorkspaceId ? "is-active" : ""
                  }
                  onClick={() => selectCandidate(candidate)}
                  disabled={busy}
                >
                  <UserRound size={15} />
                  <span className="bdc-bootstrap__candidate-main">
                    <strong>{candidate.sourceProjectTitle}</strong>
                    <small>
                      {candidate.customerName}
                      {candidate.customerLegalName
                        ? ` · ${candidate.customerLegalName}`
                        : ""}
                    </small>
                  </span>
                  <span className="bdc-bootstrap__candidate-meta">
                    <small>{PREFILL_MATCH_KIND_LABELS[candidate.matchKind]}</small>
                    <small>
                      {candidate.populatedFields.length} 项资料 ·{" "}
                      {formatDate(candidate.sourceUpdatedAt)}
                    </small>
                  </span>
                  <ChevronRight size={14} />
                </button>
              </li>
            ))}
          </ul>

          {previewLoading && (
            <div className="bdc-bootstrap__hint">
              <LoaderCircle size={13} className="business-documents-center__spin" />
              正在生成带入预览…
            </div>
          )}

          {preview && previewSummary && (
            <div className="bdc-bootstrap__preview">
              <div className="bdc-bootstrap__preview-summary">
                来自「{preview.sourceProjectTitle}」：将带入{" "}
                <strong>{previewSummary.filled}</strong> 项，保持{" "}
                {previewSummary.kept} 项不变
              </div>
              <table>
                <tbody>
                  {sortedChanges.map((change) => (
                    <tr
                      key={change.field}
                      className={
                        change.decision === "filled" || change.decision === "replaced"
                          ? "is-filled"
                          : ""
                      }
                    >
                      <th scope="row">{PREFILL_FIELD_LABELS[change.field]}</th>
                      <td>{formatPrefillValue(change.field, change.resultValue)}</td>
                      <td className="bdc-bootstrap__decision">
                        {PREFILL_DECISION_LABELS[change.decision]}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <button
                type="button"
                className="business-documents-center__empty-action"
                onClick={() => void onCreate(preview.sourceWorkspaceId)}
                disabled={!isDesktopRuntime || busy}
              >
                <Check size={14} />
                {creating && selectedSourceId ? "正在建立…" : "复用并建立商务档案"}
              </button>
            </div>
          )}
        </section>
      )}

      {prefillError && (
        <div className="bdc-bootstrap__error" role="status">
          <CircleAlert size={13} />
          老客户资料查询未完成：{prefillError}
        </div>
      )}
    </div>
  );
}

function CenterEmpty({ icon, children }: { icon: ReactNode; children: ReactNode }) {
  return <div className="business-documents-center__empty">{icon}<div className="business-documents-center__empty-prompt">{children}</div></div>;
}

function InlineEmpty({ icon, text }: { icon: ReactNode; text: string }) {
  return <div className="business-documents-center__inline-empty">{icon}<span>{text}</span></div>;
}

function InlineNotice({ text, tone = "default" }: { text: string; tone?: "default" | "warning" }) {
  return <div className={`business-documents-center__flow-note is-${tone}`}><CircleAlert size={15} /><span>{text}</span></div>;
}

function shortId(value: string, length = 8): string {
  const normalized = value.trim();
  if (!normalized) return "—";
  return normalized.length <= length ? normalized : `${normalized.slice(0, length)}…`;
}

function businessCustomerToInput(
  workspace: BusinessWorkspaceRecord,
): BusinessCustomerInput {
  return {
    displayName: workspace.customer.displayName,
    legalName: workspace.customer.legalName,
    taxId: workspace.customer.taxId,
    billingAddress: workspace.customer.billingAddress,
    primaryContactName: workspace.customer.primaryContactName,
    primaryPhone: workspace.customer.primaryPhone,
    primaryEmail: workspace.customer.primaryEmail,
    notes: workspace.customer.notes,
  };
}

function emptyMilestoneDraft(): BusinessMilestoneInput {
  return {
    id: null,
    title: "",
    description: "",
    dueAt: null,
    acceptanceCriteria: "",
    required: true,
    status: "planned",
  };
}

function milestoneToInput(
  milestone: BusinessMilestoneRecord,
): BusinessMilestoneInput {
  return {
    id: milestone.id,
    title: milestone.title,
    description: milestone.description,
    dueAt: milestone.dueAt,
    acceptanceCriteria: milestone.acceptanceCriteria,
    required: milestone.required,
    status: milestone.status,
  };
}

function emptyDeliverableDraft(
  milestone: BusinessMilestoneRecord,
): DeliverableDraft {
  return {
    milestoneId: milestone.id,
    deliverableId: null,
    name: "",
    required: true,
    assetId: "",
    notes: "",
  };
}

function deliverableVersionDraft(
  milestone: BusinessMilestoneRecord,
  deliverable: BusinessDeliverableRecord,
): DeliverableDraft {
  return {
    milestoneId: milestone.id,
    deliverableId: deliverable.id,
    name: deliverable.name,
    required: deliverable.required,
    assetId: "",
    notes: "",
  };
}

function versionsForMilestone(
  workspace: BusinessWorkspaceRecord,
  milestoneId: string,
): BusinessDeliverableVersionRecord[] {
  const milestone = workspace.milestones.find((candidate) => candidate.id === milestoneId);
  if (!milestone) return [];
  return milestone.deliverables
    .flatMap((deliverable) => deliverable.versions)
    .sort((left, right) => right.createdAt - left.createdAt || right.versionNumber - left.versionNumber);
}

function findDeliverableVersion(
  workspace: BusinessWorkspaceRecord,
  versionId: string,
): BusinessDeliverableVersionRecord | null {
  for (const milestone of workspace.milestones) {
    for (const deliverable of milestone.deliverables) {
      const version = deliverable.versions.find((candidate) => candidate.id === versionId);
      if (version) return version;
    }
  }
  return null;
}

function defaultDeliverySentDraft(
  milestone: BusinessMilestoneRecord,
  workspace: BusinessWorkspaceRecord,
): DeliverySentDraft {
  const versionIds = milestone.deliverables
    .map((deliverable) => [...deliverable.versions]
      .filter((version) => version.status !== "rejected" && version.status !== "superseded")
      .sort((left, right) => right.versionNumber - left.versionNumber)[0]?.id)
    .filter((versionId): versionId is string => Boolean(versionId));
  return {
    milestoneId: milestone.id,
    versionIds,
    recipient: workspace.customer.primaryContactName || workspace.profile.customerContact,
    channel: "微信",
    sentDate: todayDateInput(),
    note: "",
  };
}

export function undecidedSubmissionVersionIds(
  submission: BusinessDeliverySubmissionRecord,
): string[] {
  const decided = new Set<string>();
  for (const signoff of submission.signoffs) {
    for (const versionId of signoff.acceptedVersionIds) decided.add(versionId);
    for (const versionId of signoff.rejectedVersionIds) decided.add(versionId);
  }
  return submission.versionIds.filter((versionId) => !decided.has(versionId));
}

function defaultDeliverySignoffDraft(
  submission: BusinessDeliverySubmissionRecord,
): DeliverySignoffDraft {
  const decisions = undecidedSubmissionVersionIds(submission).reduce<
    DeliverySignoffDraft["decisions"]
  >((current, versionId) => {
    current[versionId] = "pending";
    return current;
  }, {});
  const latestSignoff =
    [...submission.signoffs].sort(
      (left, right) => right.recordedAt - left.recordedAt,
    )[0] ?? null;
  return {
    submissionId: submission.id,
    decisions,
    customerRepresentative: latestSignoff?.customerRepresentative ?? "",
    evidenceAssetId: "",
    evidenceNote: "",
    occurredDate: todayDateInput(),
    note: "",
  };
}

function emptyInvoiceDraft(workspace: BusinessWorkspaceRecord): InvoiceDraft {
  const payment = workspace.payments.find((candidate) => candidate.status === "requested")
    ?? workspace.payments.find((candidate) => candidate.status === "planned" || candidate.status === "partiallyReceived")
    ?? null;
  const remainingCents = Math.max(
    0,
    workspace.financialSummary.contractCents - invoiceNetCents(workspace.invoices),
  );
  const amountCents = payment ? Math.min(payment.amountCents, remainingCents) : remainingCents;
  const taxCents = workspace.profile.defaultTaxRateBps > 0
    ? Math.round((amountCents * workspace.profile.defaultTaxRateBps) / (10000 + workspace.profile.defaultTaxRateBps))
    : 0;
  return {
    paymentId: payment?.id ?? "",
    invoiceCode: "",
    invoiceNumber: "",
    amount: amountCents > 0 ? centsToDecimal(amountCents) : "",
    tax: taxCents > 0 ? centsToDecimal(taxCents) : "0.00",
    issuedDate: todayDateInput(),
    assetIds: "",
  };
}

export function invoiceReversibleCents(
  workspace: BusinessWorkspaceRecord,
  invoice: BusinessInvoiceRecord,
): number {
  if (invoice.kind !== "issued") return 0;
  const reversed = workspace.invoices
    .filter(
      (candidate) =>
        candidate.kind === "reversal" &&
        candidate.originalInvoiceId === invoice.id,
    )
    .reduce((total, candidate) => total + candidate.amountCents, 0);
  return Math.max(0, invoice.amountCents - reversed);
}

function defaultInvoiceReversalDraft(
  workspace: BusinessWorkspaceRecord,
  invoice: BusinessInvoiceRecord,
): InvoiceReversalDraft {
  const remainingCents = invoiceReversibleCents(workspace, invoice);
  const taxCents =
    invoice.amountCents > 0
      ? Math.min(
          invoice.taxCents,
          Math.round((invoice.taxCents * remainingCents) / invoice.amountCents),
        )
      : 0;
  return {
    originalInvoiceId: invoice.id,
    invoiceCode: "",
    invoiceNumber: "",
    amount: remainingCents > 0 ? centsToDecimal(remainingCents) : "",
    tax: centsToDecimal(taxCents),
    issuedDate: todayDateInput(),
    reason: "",
    assetIds: "",
  };
}

function splitAssetIds(value: string): string[] {
  return [...new Set(
    value
      .split(/[\s,，;；]+/)
      .map((assetId) => assetId.trim())
      .filter(Boolean),
  )];
}

function invoiceNetCents(
  invoices: readonly BusinessInvoiceRecord[],
): number {
  return invoices.reduce(
    (total, invoice) => total + (invoice.kind === "reversal" ? -invoice.amountCents : invoice.amountCents),
    0,
  );
}

function latestArchiveSnapshot(
  snapshots: readonly BusinessArchiveSnapshotRecord[],
): BusinessArchiveSnapshotRecord | null {
  return snapshots.reduce<BusinessArchiveSnapshotRecord | null>(
    (latest, snapshot) => !latest || snapshot.generatedAt > latest.generatedAt ? snapshot : latest,
    null,
  );
}

function archiveIntegrityLabel(
  status: BusinessWorkspaceRecord["archiveIntegrityStatus"],
): string {
  const labels: Record<BusinessWorkspaceRecord["archiveIntegrityStatus"], string> = {
    notCaptured: "未生成快照",
    ready: "快照有效",
    stale: "快照已过期",
    failed: "快照失败",
  };
  return labels[status];
}

function buildArchivePreflight(
  workspace: BusinessWorkspaceRecord,
): ArchivePreflightItem[] {
  const contract = currentDocumentForKind(workspace, "contract");
  const acceptance = currentDocumentForKind(workspace, "acceptance");
  const openDocuments = workspace.documents.filter((document) =>
    document.status === "draft" || document.status === "inReview" || document.status === "approved",
  );
  const openPayments = workspace.payments.filter((payment) =>
    payment.status === "planned" ||
    payment.status === "requested" ||
    payment.status === "partiallyReceived",
  );
  const requiredMilestones = workspace.milestones.filter((milestone) => milestone.required);
  const acceptedMilestones = requiredMilestones.filter((milestone) => milestone.status === "accepted");
  const requiredDeliverables = requiredMilestones.flatMap((milestone) =>
    milestone.deliverables.filter((deliverable) => deliverable.required),
  );
  const acceptedDeliverables = requiredDeliverables.filter((deliverable) =>
    deliverable.versions.some((version) => version.status === "accepted"),
  );
  const unresolvedSubmissions = workspace.deliverySubmissions.filter(
    (submission) => undecidedSubmissionVersionIds(submission).length > 0,
  );
  const netInvoiceCents = invoiceNetCents(workspace.invoices);
  const invoicesWithoutAssets = workspace.invoices.filter((invoice) => invoice.artifacts.length === 0);
  const contractAmountReady = workspace.financialSummary.contractCents > 0;
  const receiptsBalanced = contractAmountReady
    && workspace.financialSummary.receivedCents === workspace.financialSummary.contractCents;

  return [
    {
      id: "contract",
      label: "生效合同",
      detail: contract?.status === "effective" ? contract.documentNumber : "当前合同尚未生效",
      passed: contract?.status === "effective",
    },
    {
      id: "acceptance",
      label: "生效验收单",
      detail: acceptance?.status === "effective" ? acceptance.documentNumber : "当前验收单尚未生效",
      passed: acceptance?.status === "effective",
    },
    {
      id: "receipts",
      label: "合同款到账",
      detail: contractAmountReady
        ? `${formatMoney(workspace.financialSummary.receivedCents, workspace.profile.currency)} / ${formatMoney(workspace.financialSummary.contractCents, workspace.profile.currency)}`
        : "生效合同金额必须大于 0",
      passed: receiptsBalanced,
    },
    {
      id: "documents",
      label: "单据已完结",
      detail: openDocuments.length === 0 ? "没有草稿或待复核单据" : `${openDocuments.length} 份单据未完结`,
      passed: openDocuments.length === 0,
    },
    {
      id: "payments",
      label: "付款节点已完结",
      detail: openPayments.length === 0 ? "没有待请款节点" : `${openPayments.length} 个付款节点未完结`,
      passed: openPayments.length === 0,
    },
    {
      id: "milestones",
      label: "必需里程碑已签收",
      detail: requiredMilestones.length === 0
        ? "至少需要一个必需里程碑"
        : `${acceptedMilestones.length}/${requiredMilestones.length} 已签收`,
      passed: requiredMilestones.length > 0 && acceptedMilestones.length === requiredMilestones.length,
    },
    {
      id: "deliverables",
      label: "必需交付物有接受版本",
      detail: requiredDeliverables.length === 0
        ? "当前没有必需交付物"
        : `${acceptedDeliverables.length}/${requiredDeliverables.length} 已接受`,
      passed: acceptedDeliverables.length === requiredDeliverables.length,
    },
    {
      id: "submissions",
      label: "发送批次已结束",
      detail: unresolvedSubmissions.length === 0 ? "所有发送批次的版本都已有签收结论" : `${unresolvedSubmissions.length} 个发送批次仍有版本待签收`,
      passed: unresolvedSubmissions.length === 0,
    },
    {
      id: "invoices",
      label: "开票净额匹配合同",
      detail: `${formatMoney(netInvoiceCents, workspace.profile.currency)} / ${formatMoney(workspace.financialSummary.contractCents, workspace.profile.currency)}`,
      passed: workspace.invoices.length > 0
        && netInvoiceCents === workspace.financialSummary.contractCents,
    },
    {
      id: "invoice-assets",
      label: "发票附件完整",
      detail: invoicesWithoutAssets.length === 0 ? "每张发票和红票均有 Vault 附件" : `${invoicesWithoutAssets.length} 张票据缺少附件`,
      passed: workspace.invoices.length > 0 && invoicesWithoutAssets.length === 0,
    },
    {
      id: "lifecycle",
      label: "生命周期已到账",
      detail: `当前：${lifecycleLabel(workspace.lifecycleStage)}`,
      passed: workspace.lifecycleStage === "paid",
    },
  ];
}

export function businessProfileToInput(profile: BusinessProfile): BusinessProfileInput {
  return {
    ...profile,
    lineItems: profile.lineItems.map((item) => ({
      id: item.id,
      name: item.name,
      description: item.description,
      quantityMillis: item.quantityMillis,
      unit: item.unit,
      unitPriceCents: item.unitPriceCents,
      taxRateBps: item.taxRateBps,
    })),
  };
}

export function allowedDocumentTransitions(document: BusinessDocumentRecord): BusinessDocumentStatus[] {
  switch (document.status) {
    case "draft":
      return ["inReview", "voided"];
    case "inReview":
      return ["approved", "draft", "voided"];
    case "approved":
      return ["inReview", "voided"];
    case "generated":
      return document.kind === "contract" || document.kind === "acceptance"
        ? ["effective", "voided"]
        : ["voided"];
    case "effective":
      return ["voided"];
    case "voided":
      return [];
    default:
      return [];
  }
}

export function summarizeBusinessWorkspace(workspace: BusinessWorkspaceRecord): BusinessWorkspaceSummary {
  return {
    quotedCents: workspace.financialSummary.quotedCents,
    contractCents: workspace.financialSummary.contractCents,
    plannedCents: workspace.financialSummary.scheduledCents,
    requestedCents: workspace.financialSummary.requestedCents,
    receivedCents: workspace.financialSummary.receivedCents,
    outstandingCents: workspace.financialSummary.outstandingCents,
    generatedDocuments: workspace.documents.filter((document) => document.status === "generated" || document.status === "effective").length,
  };
}

export function decimalToCents(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? Math.round(parsed * 100) : 0;
}

function documentDefinition(kind: BusinessDocumentKind): DocumentDefinition {
  return DOCUMENT_DEFINITIONS.find((item) => item.kind === kind)!;
}

function currentDocumentIdForKind(workspace: BusinessWorkspaceRecord, kind: BusinessDocumentKind): string | null {
  switch (kind) {
    case "quote":
      return workspace.currentDocuments.quoteDocumentId;
    case "contract":
      return workspace.currentDocuments.contractDocumentId;
    case "paymentRequest":
      return workspace.currentDocuments.paymentRequestDocumentId;
    case "acceptance":
      return workspace.currentDocuments.acceptanceDocumentId;
    default:
      return null;
  }
}

function currentDocumentForKind(workspace: BusinessWorkspaceRecord, kind: BusinessDocumentKind): BusinessDocumentRecord | null {
  const documentId = currentDocumentIdForKind(workspace, kind);
  return documentId
    ? workspace.documents.find((document) => document.id === documentId) ?? null
    : null;
}

function availablePaymentPlans(workspace: BusinessWorkspaceRecord): BusinessPaymentRecord[] {
  return workspace.payments.filter((payment) =>
    payment.status === "planned" &&
    !workspace.documents.some((document) =>
      document.kind === "paymentRequest" &&
      document.status !== "voided" &&
      document.snapshot.payment?.id === payment.id,
    ),
  );
}

function defaultDocumentDraft(kind: BusinessDocumentKind, workspace: BusinessWorkspaceRecord, project: ProjectRecord): DocumentDraft {
  const definition = documentDefinition(kind);
  const sequence = workspace.documents.filter((document) => document.kind === kind).length + 1;
  const projectCode = workspace.profile.projectCode.trim() || project.id.slice(0, 8).toUpperCase();
  return {
    kind,
    documentNumber: `${definition.prefix}-${projectCode}-${String(sequence).padStart(2, "0")}`,
    title: `${project.name}${definition.label}`,
    paymentId: kind === "paymentRequest" ? availablePaymentPlans(workspace)[0]?.id ?? "" : "",
  };
}

function emptyPaymentDraft(): PaymentDraft {
  return {
    id: null,
    label: "",
    amount: "",
    dueDate: "",
    occurredDate: "",
    status: "planned",
    reference: "",
    notes: "",
  };
}

function paymentRecordToDraft(payment: BusinessPaymentRecord, status: BusinessPaymentStatus = payment.status): PaymentDraft {
  return {
    id: payment.id,
    label: payment.label,
    amount: centsToDecimal(payment.amountCents),
    dueDate: timestampToDateInput(payment.dueAt),
    occurredDate: status === "received" ? todayDateInput() : "",
    status,
    reference: status === "received" ? "" : payment.reference,
    notes: payment.notes,
  };
}

function paymentDraftToInput(draft: PaymentDraft): BusinessPaymentInput {
  return {
    id: draft.id,
    label: draft.label.trim(),
    amountCents: decimalToCents(draft.amount),
    dueAt: dateInputToTimestamp(draft.dueDate),
    occurredAt: null,
    status: "planned",
    reference: draft.reference.trim(),
    notes: draft.notes.trim(),
  };
}

function defaultQuoteConfirmationDraft(
  document: BusinessDocumentRecord,
  workspace: BusinessWorkspaceRecord,
): QuoteConfirmationDraft {
  return {
    quoteDocumentId: document.id,
    confirmationVersion: `${document.documentNumber}-R${document.revision}`,
    customerRepresentative: workspace.profile.customerContact,
    occurredDate: todayDateInput(),
    notes: "",
  };
}

function defaultReceiptDraft(
  payment: BusinessPaymentRecord,
  workspace: BusinessWorkspaceRecord,
): ReceiptDraft {
  return {
    paymentId: payment.id,
    amount: centsToDecimal(outstandingPaymentCents(workspace, payment)),
    occurredDate: todayDateInput(),
    reference: "",
    notes: "",
    includeEvidence: true,
  };
}

function defaultReceiptReversalDraft(
  receipt: BusinessReceiptRecord,
  workspace: BusinessWorkspaceRecord,
): ReceiptReversalDraft {
  return {
    receiptId: receipt.id,
    amount: centsToDecimal(reversibleReceiptCents(workspace, receipt)),
    occurredDate: todayDateInput(),
    reference: "",
    reason: "",
  };
}

function defaultDocumentTransitionDraft(
  document: BusinessDocumentRecord,
  status: BusinessDocumentStatus,
): DocumentTransitionDraft {
  return {
    documentId: document.id,
    status,
    mode: "evidence",
    occurredDate: todayDateInput(),
    evidenceNote:
      document.kind === "contract" ? "双方签署合同" : "客户验收确认",
    reason: "",
  };
}

function emptyDocumentTransitionInput() {
  return {
    reason: "",
    attachEvidence: false,
    evidenceOccurredAt: null,
    evidenceNote: "",
    manualWaiverReason: null,
  };
}

function emptyLineItem(taxRateBps: number): BusinessLineItemInput {
  return { id: null, name: "", description: "", quantityMillis: 1000, unit: "项", unitPriceCents: 0, taxRateBps };
}

function lineItemAmount(item: Pick<BusinessLineItemInput, "quantityMillis" | "unitPriceCents" | "taxRateBps">): number {
  return Math.round(
    (item.quantityMillis * item.unitPriceCents * (10000 + item.taxRateBps)) /
      (1000 * 10000),
  );
}

function basisPointsToPercent(value: number): string {
  return (value / 100).toFixed(2).replace(/\.00$/, "").replace(/(\.\d)0$/, "$1");
}

function percentToBasisPoints(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? Math.max(0, Math.round(parsed * 100)) : 0;
}

function millisToDecimal(value: number): string {
  return (value / 1000).toFixed(3).replace(/\.000$/, "").replace(/(\.\d*?[1-9])0+$/, "$1");
}

function decimalToMillis(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? Math.max(0, Math.round(parsed * 1000)) : 0;
}
function profileLineItemsTotal(profile: BusinessProfile): number {
  return profile.lineItems.reduce((total, item) => total + lineItemAmount(item), 0);
}

const PROFILE_FIELD_LABELS: Record<string, string> = {
  projectTitle: "项目名称",
  customerName: "客户名称",
  supplierLegalName: "供应商主体名称",
  currency: "币种",
  lineItems: "服务明细",
  serviceStartAt: "服务开始日期",
  serviceEndAt: "服务结束日期",
  paymentTerms: "付款条款",
  acceptanceTerms: "验收条款",
  supplierBankName: "收款银行",
  supplierBankAccount: "收款账号",
  deliverySummary: "交付摘要",
  payment: "关联付款计划",
  positiveAmount: "大于 0 的付款金额",
};

export function documentProfileMissingFields(
  profile: BusinessProfile,
  kind: BusinessDocumentKind,
  payment: BusinessPaymentRecord | null = null,
  includePaymentChecks = true,
): string[] {
  const text = (value: string | undefined | null) => (value ?? "").trim();
  const missing: string[] = [];
  if (!text(profile.projectTitle)) missing.push("projectTitle");
  if (!text(profile.customerName) && !text(profile.customerLegalName)) {
    missing.push("customerName");
  }
  if (!text(profile.supplierLegalName)) missing.push("supplierLegalName");
  if (!text(profile.currency)) missing.push("currency");
  if ((profile.lineItems ?? []).length === 0) missing.push("lineItems");
  if (kind === "contract") {
    if (profile.serviceStartAt == null) missing.push("serviceStartAt");
    if (profile.serviceEndAt == null) missing.push("serviceEndAt");
    if (!text(profile.paymentTerms)) missing.push("paymentTerms");
    if (!text(profile.acceptanceTerms)) missing.push("acceptanceTerms");
  }
  if (kind === "paymentRequest") {
    if (!text(profile.supplierBankName)) missing.push("supplierBankName");
    if (!text(profile.supplierBankAccount)) missing.push("supplierBankAccount");
    if (!text(profile.paymentTerms)) missing.push("paymentTerms");
    if (includePaymentChecks) {
      if (!payment) missing.push("payment");
      else if (payment.amountCents <= 0) missing.push("positiveAmount");
    }
  }
  if (kind === "acceptance") {
    if (!text(profile.deliverySummary)) missing.push("deliverySummary");
    if (!text(profile.acceptanceTerms)) missing.push("acceptanceTerms");
  }
  return missing;
}

function profileFieldLabels(fields: string[]): string {
  return fields.map((field) => PROFILE_FIELD_LABELS[field] ?? field).join("、");
}

export function documentCreationBlockReason(workspace: BusinessWorkspaceRecord, kind: BusinessDocumentKind): string | null {
  if (workspace.status === "archived") return "工作区已归档，请重新打开后再创建单据";
  if (kind === "contract") {
    const quote = currentDocumentForKind(workspace, "quote");
    if (!quote || quote.status !== "generated") return "请先生成报价单，并确保其为当前有效报价";
    if (!quoteConfirmationForDocument(workspace, quote)) return "请先登记客户对当前报价版本的确认凭证";
  }
  if (kind === "paymentRequest" || kind === "acceptance") {
    const contract = currentDocumentForKind(workspace, "contract");
    if (!contract || contract.status !== "effective") return "请先将当前合同确认生效";
  }
  if (kind === "paymentRequest" && availablePaymentPlans(workspace).length === 0) {
    return "没有可关联的付款计划；请先新增付款计划或处理已有请款单";
  }
  // 单据创建时会冻结当前项目资料快照；缺字段的快照之后无法通过批准，
  // 所以在创建入口就提示补全，避免生成只能作废的单据。
  const missing = documentProfileMissingFields(
    workspace.profile,
    kind,
    null,
    false,
  );
  if (missing.length > 0) {
    return `请先在「资料」页补全：${profileFieldLabels(missing)}（单据创建时会冻结资料快照）`;
  }
  return null;
}

function documentTransitionBlockReason(
  workspace: BusinessWorkspaceRecord,
  document: BusinessDocumentRecord,
  status: BusinessDocumentStatus,
): string | null {
  if (workspace.status === "archived") return "工作区已归档，单据状态不可修改";
  if (!allowedDocumentTransitions(document).includes(status)) return "该单据不允许执行此状态转换";
  if (status === "approved") {
    const missing = documentProfileMissingFields(
      document.snapshot.profile,
      document.kind,
      document.snapshot.payment,
    );
    if (missing.length > 0) {
      return `单据快照缺少：${profileFieldLabels(missing)}。快照在创建时已冻结，请作废本单据，补全项目资料后重新创建`;
    }
  }
  if (
    status === "approved" &&
    (document.kind === "quote" || document.kind === "contract") &&
    profileLineItemsTotal(document.snapshot.profile) <= 0
  ) {
    return `${documentDefinition(document.kind).label}的服务明细含税合计必须大于 0`;
  }
  if (
    status === "effective" &&
    document.kind !== "contract" &&
    document.kind !== "acceptance"
  ) {
    return "只有已生成的合同和验收单可以确认生效";
  }
  return null;
}

function documentGenerateBlockReason(workspace: BusinessWorkspaceRecord, document: BusinessDocumentRecord): string | null {
  if (workspace.status === "archived") return "工作区已归档，不能生成单据";
  if (document.status !== "approved") return "只有已批准单据可以生成正式文件";
  if (
    (document.kind === "quote" || document.kind === "contract") &&
    profileLineItemsTotal(document.snapshot.profile) <= 0
  ) {
    return `${documentDefinition(document.kind).label}的服务明细含税合计必须大于 0`;
  }
  return null;
}
function paymentRequestBlockReason(workspace: BusinessWorkspaceRecord, payment: BusinessPaymentRecord): string | null {
  if (workspace.status === "archived") return "工作区已归档，不能发起请款";
  if (payment.status !== "planned") return "只有付款计划可以创建正式请款单";
  const contract = currentDocumentForKind(workspace, "contract");
  if (!contract || contract.status !== "effective") return "请先将当前合同确认生效";
  const existing = workspace.documents.find((document) =>
    document.kind === "paymentRequest" &&
    document.status !== "voided" &&
    document.snapshot.payment?.id === payment.id,
  );
  if (existing) return `已有请款单 ${existing.documentNumber}，请继续审批生成或先作废`;
  return null;
}

function generatedPaymentRequestForPayment(workspace: BusinessWorkspaceRecord, paymentId: string): BusinessDocumentRecord | null {
  return [...workspace.documents]
    .sort((left, right) => right.sequenceNumber - left.sequenceNumber)
    .find((document) =>
      document.kind === "paymentRequest" &&
      document.status === "generated" &&
      document.snapshot.payment?.id === paymentId,
    ) ?? null;
}

function receiptBlockReason(workspace: BusinessWorkspaceRecord, payment: BusinessPaymentRecord | null): string | null {
  if (!payment) return "找不到关联付款节点";
  if (workspace.status === "archived") return "工作区已归档，不能登记到账";
  if (payment.status !== "requested" && payment.status !== "partiallyReceived") return "只有已请款或部分到账的付款节点才能继续登记到账";
  if (!generatedPaymentRequestForPayment(workspace, payment.id)) {
    return "缺少关联该付款节点的 Generated PaymentRequest，不能登记到账";
  }
  if (outstandingPaymentCents(workspace, payment) <= 0) return "该付款节点已足额到账";
  return null;
}

export function quoteConfirmationForDocument(
  workspace: BusinessWorkspaceRecord,
  document: BusinessDocumentRecord,
) {
  if (!document.outputAssetId) return null;
  return workspace.quoteConfirmations.find(
    (confirmation) =>
      confirmation.quoteDocumentId === document.id &&
      confirmation.quoteDocumentRevision === document.revision &&
      confirmation.quoteAssetId === document.outputAssetId,
  ) ?? null;
}

function quoteConfirmationBlockReason(
  workspace: BusinessWorkspaceRecord,
  document: BusinessDocumentRecord,
): string | null {
  if (workspace.status === "archived") return "工作区已归档，不能登记报价确认";
  if (document.kind !== "quote" || document.status !== "generated" || !document.outputAssetId) {
    return "只有已生成正式文件的报价单可以登记客户确认";
  }
  if (currentDocumentIdForKind(workspace, "quote") !== document.id) return "只能确认当前有效报价";
  if (quoteConfirmationForDocument(workspace, document)) return "该报价版本已经登记客户确认";
  return null;
}

export function requirementAdoptionBlockReason(
  workspace: BusinessWorkspaceRecord,
  latestConfirmedRequirement: RequirementBriefRecord | null,
): string | null {
  if (workspace.status === "archived") return "工作区已归档，不能同步需求";
  if (!latestConfirmedRequirement) return "当前项目还没有已确认需求";
  if (
    workspace.requirementBriefId === latestConfirmedRequirement.id &&
    workspace.requirementBriefRevision === latestConfirmedRequirement.revision
  ) return "当前项目资料已经是最新已确认需求";
  const formalDocument = workspace.documents.find((document) =>
    document.status === "approved" ||
    document.status === "generated" ||
    document.status === "effective" ||
    document.status === "voided",
  );
  return formalDocument
    ? `已有正式或作废单据 ${formalDocument.documentNumber}，不能自动改写项目资料`
    : null;
}

function receiptNetCentsForPayment(
  workspace: BusinessWorkspaceRecord,
  paymentId: string,
): number {
  return workspace.receipts
    .filter((receipt) => receipt.paymentId === paymentId)
    .reduce(
      (total, receipt) =>
        total + (receipt.kind === "receipt" ? receipt.amountCents : -receipt.amountCents),
      0,
    );
}

export function outstandingPaymentCents(
  workspace: BusinessWorkspaceRecord,
  payment: BusinessPaymentRecord,
): number {
  return Math.max(0, payment.amountCents - receiptNetCentsForPayment(workspace, payment.id));
}

export function reversibleReceiptCents(
  workspace: BusinessWorkspaceRecord,
  receipt: BusinessReceiptRecord,
): number {
  if (receipt.kind !== "receipt") return 0;
  const reversed = workspace.receipts
    .filter(
      (candidate) =>
        candidate.kind === "reversal" && candidate.reversesReceiptId === receipt.id,
    )
    .reduce((total, candidate) => total + candidate.amountCents, 0);
  return Math.max(0, receipt.amountCents - reversed);
}

export function archiveWorkspaceBlockReason(workspace: BusinessWorkspaceRecord): string | null {
  if (workspace.status === "archived") return null;
  const failedCheck = buildArchivePreflight(workspace).find((item) => !item.passed);
  if (failedCheck) return `${failedCheck.label}：${failedCheck.detail}`;
  if (workspace.archiveIntegrityStatus !== "ready") {
    return workspace.archiveIntegrityStatus === "stale"
      ? "归档快照已过期，请重新生成"
      : workspace.archiveIntegrityStatus === "failed"
        ? "归档快照生成失败，请重新生成"
        : "请先生成归档完整性快照";
  }
  const snapshot = latestArchiveSnapshot(workspace.archiveSnapshots);
  if (!snapshot) return "请先生成归档完整性快照";
  if (
    snapshot.capturedWorkspaceRevision + 1 !== workspace.revision
    || snapshot.capturedCustomerRevision !== workspace.customer.revision
  ) {
    return "归档快照与当前资料版本不一致，请重新生成";
  }
  return null;
}

function documentTransitionLabel(status: BusinessDocumentStatus): string {
  const labels: Record<BusinessDocumentStatus, string> = {
    draft: "退回草稿",
    inReview: "提交复核",
    approved: "批准",
    generated: "已生成",
    effective: "确认生效",
    voided: "作废",
  };
  return labels[status];
}

function documentTransitionIcon(status: BusinessDocumentStatus): ReactNode {
  if (status === "approved") return <CheckCircle2 size={13} />;
  if (status === "effective") return <ShieldCheck size={13} />;
  if (status === "inReview") return <Send size={13} />;
  if (status === "draft") return <RotateCcw size={13} />;
  if (status === "generated") return <FileOutput size={13} />;
  return <X size={13} />;
}

function paymentStatusIcon(status: BusinessPaymentStatus): ReactNode {
  if (status === "received") return <CheckCircle2 size={16} />;
  if (status === "requested") return <Banknote size={16} />;
  if (status === "canceled") return <X size={16} />;
  return <Clock3 size={16} />;
}

function lifecycleLabel(stage: BusinessLifecycleStage): string {
  return LIFECYCLE_STAGES.find((item) => item.id === stage)?.label ?? stage;
}

function eventTypeLabel(eventType: BusinessWorkspaceDomainEvent["eventType"]): string {
  switch (eventType) {
    case "businessWorkspace.created":
      return "工作区创建";
    case "businessWorkspace.profileUpdated":
      return "项目资料更新";
    case "businessWorkspace.documentCreated":
      return "单据创建";
    case "businessWorkspace.documentStatusChanged":
      return "单据状态变更";
    case "businessWorkspace.documentGenerated":
      return "正式单据生成";
    case "businessWorkspace.paymentUpserted":
      return "付款记录更新";
    case "businessWorkspace.quoteConfirmed":
      return "客户报价确认";
    case "businessWorkspace.receiptRecorded":
      return "到账流水登记";
    case "businessWorkspace.receiptReversed":
      return "到账流水冲销";
    case "businessWorkspace.requirementAdopted":
      return "已确认需求同步";
    case "businessWorkspace.customerUpserted":
      return "客户资料更新";
    case "businessWorkspace.customerAssigned":
      return "客户关联更新";
    case "businessWorkspace.milestoneUpserted":
      return "交付里程碑更新";
    case "businessWorkspace.deliverableVersionRegistered":
      return "交付版本登记";
    case "businessWorkspace.deliverySent":
      return "交付发送登记";
    case "businessWorkspace.deliverySignoffRecorded":
      return "客户签收登记";
    case "businessWorkspace.invoiceIssued":
      return "发票登记";
    case "businessWorkspace.invoiceRedCorrected":
      return "发票红冲登记";
    case "businessWorkspace.invoiceAssetAttached":
      return "发票附件补充";
    case "businessWorkspace.archiveSnapshotPrepared":
      return "归档快照生成";
    case "businessWorkspace.reviewedContractPromoted":
      return "审查合同转正式合同";
    case "businessWorkspace.statusChanged":
      return "工作区状态变更";
    default:
      return eventType;
  }
}

function eventReason(eventType: BusinessWorkspaceDomainEvent["eventType"]): string {
  switch (eventType) {
    case "businessWorkspace.created":
      return "建立商务工作区";
    case "businessWorkspace.profileUpdated":
      return "更新商务项目资料";
    case "businessWorkspace.documentCreated":
      return "创建商务单据";
    case "businessWorkspace.documentStatusChanged":
      return "推进商务单据状态";
    case "businessWorkspace.documentGenerated":
      return "生成正式商务单据";
    case "businessWorkspace.paymentUpserted":
      return "更新付款与到账记录";
    case "businessWorkspace.quoteConfirmed":
      return "登记客户报价确认凭证";
    case "businessWorkspace.receiptRecorded":
      return "登记到账流水";
    case "businessWorkspace.receiptReversed":
      return "冲销到账流水";
    case "businessWorkspace.requirementAdopted":
      return "采用最新已确认需求";
    case "businessWorkspace.customerUpserted":
      return "更新稳定客户主数据";
    case "businessWorkspace.customerAssigned":
      return "关联商务工作区与客户";
    case "businessWorkspace.milestoneUpserted":
      return "更新交付里程碑";
    case "businessWorkspace.deliverableVersionRegistered":
      return "登记 Vault 交付版本";
    case "businessWorkspace.deliverySent":
      return "登记交付发送批次";
    case "businessWorkspace.deliverySignoffRecorded":
      return "登记客户版本签收结果";
    case "businessWorkspace.invoiceIssued":
      return "登记发票与附件";
    case "businessWorkspace.invoiceRedCorrected":
      return "登记发票红冲记录";
    case "businessWorkspace.invoiceAssetAttached":
      return "补充发票 Vault 附件";
    case "businessWorkspace.archiveSnapshotPrepared":
      return "生成归档完整性快照";
    case "businessWorkspace.reviewedContractPromoted":
      return "将审查通过合同纳入商务流程";
    case "businessWorkspace.statusChanged":
      return "调整商务工作区状态";
    default:
      return "商务工作区操作";
  }
}

function formatMoney(cents: number, currency: string): string {
  return new Intl.NumberFormat("zh-CN", {
    style: "currency",
    currency: currency || "CNY",
    minimumFractionDigits: 2,
  }).format(cents / 100);
}

function formatDate(value: number | null): string {
  if (!value) return "未设日期";
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(value);
}

function formatDateTime(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(value);
}

function timestampToDateInput(value: number | null): string {
  if (!value) return "";
  const date = new Date(value);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function dateInputToTimestamp(value: string): number | null {
  if (!value) return null;
  const [year, month, day] = value.split("-").map(Number);
  return new Date(year, month - 1, day, 12, 0, 0, 0).getTime();
}

function todayDateInput(): string {
  return timestampToDateInput(Date.now());
}

function centsToDecimal(cents: number): string {
  return (cents / 100).toFixed(2);
}
