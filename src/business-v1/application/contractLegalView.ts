import type { ContractReviewRecord } from "../../generated/bsaigc/ContractReviewRecord";
import type { ContractReviewStage } from "../../generated/bsaigc/ContractReviewStage";
import type { ContractReviewStatus } from "../../generated/bsaigc/ContractReviewStatus";
import type { EvidenceAnchor } from "../../generated/bsaigc/EvidenceAnchor";
import type { EvidenceContext } from "../../generated/bsaigc/EvidenceContext";
import type { ReviewFindingDecision } from "../../generated/bsaigc/ReviewFindingDecision";
import type { ReviewFindingRecord } from "../../generated/bsaigc/ReviewFindingRecord";
import type { ReviewReportRecord } from "../../generated/bsaigc/ReviewReportRecord";
import type { ReviewSeverity } from "../../generated/bsaigc/ReviewSeverity";

export const CONTRACT_REVIEW_STAGE_LABELS: Readonly<Record<ContractReviewStage, string>> = {
  created: "等待开始",
  extracting: "解析合同",
  awaitingOcr: "等待 OCR",
  reviewingRules: "规则审查",
  reviewingAgent: "智能审查",
  mergingFindings: "合并风险",
  awaitingConfirmation: "人工确认",
  generatingReport: "生成报告",
  completed: "本地完成",
};

export const CONTRACT_REVIEW_STATUS_LABELS: Readonly<Record<ContractReviewStatus, string>> = {
  draft: "草稿",
  running: "处理中",
  awaitingConfirmation: "待确认",
  completed: "已完成",
  failed: "需处理",
  cancelled: "已取消",
};

export const CONTRACT_LEGAL_SEVERITY_LABELS: Readonly<Record<ReviewSeverity, string>> = {
  critical: "严重",
  high: "高风险",
  medium: "中风险",
  low: "低风险",
  info: "提示",
};

export const CONTRACT_LEGAL_DECISION_LABELS: Readonly<Record<ReviewFindingDecision, string>> = {
  unreviewed: "未决策",
  confirmed: "已确认风险",
  rejected: "已驳回",
  acceptedRisk: "接受风险",
  needsRevision: "要求修改",
};

export type ContractLegalCapabilityId =
  | "reviewCommands"
  | "evidenceRead"
  | "reportGeneration"
  | "stageRetry";

export interface ContractLegalCapabilityInput {
  isDesktopRuntime: boolean;
  reviewCommands?: boolean;
  evidenceRead?: boolean;
  reportGeneration?: boolean;
  stageRetry?: boolean;
}

export interface ContractLegalCapabilityProjection {
  reviewCommands: boolean;
  evidenceRead: boolean;
  reportGeneration: boolean;
  stageRetry: boolean;
  isDegraded: boolean;
  unavailable: ContractLegalCapabilityId[];
  message: string | null;
}

export type ContractReportBlockerCode =
  | "capabilityUnavailable"
  | "extractionMissing"
  | "reviewState"
  | "findingsAwaitingDecision";

export interface ContractReportBlocker {
  code: ContractReportBlockerCode;
  message: string;
}

export interface ContractReportGate {
  canGenerate: boolean;
  openFindingCount: number;
  blockers: ContractReportBlocker[];
}

export type ContractReviewRetryMode = "failedStage" | "degradedAgent";

export type ContractReviewRetryBlockerCode =
  | "capabilityUnavailable"
  | "failureMissing"
  | "failureNotRetryable"
  | "humanDecisionExists"
  | "reviewState"
  | "completedStage";

export interface ContractReviewRetryProjection {
  canRetry: boolean;
  stage: ContractReviewStage | null;
  mode: ContractReviewRetryMode | null;
  blockerCode: ContractReviewRetryBlockerCode | null;
  message: string;
}

export interface ContractLegalFindingProjection {
  id: string;
  reviewId: string;
  title: string;
  category: string;
  description: string;
  recommendation: string;
  severity: ReviewSeverity;
  severityLabel: string;
  decision: ReviewFindingDecision;
  decisionLabel: string;
  needsDecision: boolean;
  isSuperseded: boolean;
  evidenceIds: string[];
  evidenceCount: number;
  hasEvidence: boolean;
  missingEvidenceReason: string | null;
  revision: number;
}

export interface ContractLegalEvidenceProjection {
  id: string;
  pageIndex: number;
  pageNumber: number;
  quote: string;
  contextBefore: string;
  contextAfter: string;
  contextText: string;
  blockId: string | null;
  blockText: string | null;
  previewAssetId: string | null;
  hasPreciseLocation: boolean;
}

export interface ContractLegalFindingCounts {
  total: number;
  awaitingDecision: number;
  decided: number;
  superseded: number;
  bySeverity: Record<ReviewSeverity, number>;
}

export interface ContractLegalReviewProjection {
  id: string;
  workspaceId: string;
  sourceAssetId: string;
  sourceFileName: string;
  status: ContractReviewStatus;
  statusLabel: string;
  stage: ContractReviewStage;
  stageLabel: string;
  revision: number;
  failureMessage: string | null;
  capabilities: ContractLegalCapabilityProjection;
  findings: ContractLegalFindingProjection[];
  evidence: ContractLegalEvidenceProjection[];
  findingCounts: ContractLegalFindingCounts;
  reportGate: ContractReportGate;
  retry: ContractReviewRetryProjection;
  preferredReport: ReviewReportRecord | null;
}

const FULL_CAPABILITIES: ContractLegalCapabilityInput = {
  isDesktopRuntime: true,
};

export function contractReviewStageLabel(stage: ContractReviewStage): string {
  return CONTRACT_REVIEW_STAGE_LABELS[stage];
}

export function contractLegalSeverityLabel(severity: ReviewSeverity): string {
  return CONTRACT_LEGAL_SEVERITY_LABELS[severity];
}

export function contractLegalDecisionLabel(decision: ReviewFindingDecision): string {
  return CONTRACT_LEGAL_DECISION_LABELS[decision];
}

export function projectContractLegalCapabilities(
  input: ContractLegalCapabilityInput,
): ContractLegalCapabilityProjection {
  const enabled = (value: boolean | undefined) => input.isDesktopRuntime && value !== false;
  const projection = {
    reviewCommands: enabled(input.reviewCommands),
    evidenceRead: enabled(input.evidenceRead),
    reportGeneration: enabled(input.reportGeneration),
    stageRetry: enabled(input.stageRetry),
  };
  const unavailable = (Object.entries(projection) as Array<[ContractLegalCapabilityId, boolean]>)
    .filter(([, available]) => !available)
    .map(([capability]) => capability);

  return {
    ...projection,
    isDegraded: unavailable.length > 0,
    unavailable,
    message: unavailable.length === 0
      ? null
      : input.isDesktopRuntime
        ? "部分法务能力当前不可用，已降级为可用能力范围。"
        : "当前运行环境不支持本地合同审查，仅可展示已加载的法务信息。",
  };
}

export function projectContractLegalFinding(
  finding: ReviewFindingRecord,
): ContractLegalFindingProjection {
  const isSuperseded = finding.status === "superseded";
  const needsDecision = finding.status === "open" && finding.decision === "unreviewed";

  return {
    id: finding.id,
    reviewId: finding.reviewId,
    title: finding.title,
    category: finding.category,
    description: finding.description,
    recommendation: finding.recommendation,
    severity: finding.severity,
    severityLabel: contractLegalSeverityLabel(finding.severity),
    decision: finding.decision,
    decisionLabel: contractLegalDecisionLabel(finding.decision),
    needsDecision,
    isSuperseded,
    evidenceIds: [...finding.evidenceIds],
    evidenceCount: finding.evidenceIds.length,
    hasEvidence: finding.evidenceIds.length > 0,
    missingEvidenceReason: finding.evidenceIds.length === 0
      ? finding.missingEvidenceReason
      : null,
    revision: finding.revision,
  };
}

export function projectContractLegalEvidence(
  evidence: EvidenceAnchor,
  context: EvidenceContext | null = null,
): ContractLegalEvidenceProjection {
  const matchingContext = context?.evidence.id === evidence.id ? context : null;
  const contextText = [evidence.contextBefore, evidence.quotedText, evidence.contextAfter]
    .map((value) => value.trim())
    .filter(Boolean)
    .join("\n");

  return {
    id: evidence.id,
    pageIndex: evidence.pageIndex,
    pageNumber: evidence.pageIndex + 1,
    quote: evidence.quotedText.trim(),
    contextBefore: evidence.contextBefore.trim(),
    contextAfter: evidence.contextAfter.trim(),
    contextText,
    blockId: evidence.blockId,
    blockText: matchingContext?.block?.text.trim() || null,
    previewAssetId: matchingContext?.page.previewAssetId ?? null,
    hasPreciseLocation: Boolean(
      evidence.blockId ||
        evidence.bbox ||
        (evidence.charStart !== null && evidence.charEnd !== null),
    ),
  };
}

export function evaluateContractReportGate(
  review: ContractReviewRecord,
  capabilities: ContractLegalCapabilityProjection = projectContractLegalCapabilities(FULL_CAPABILITIES),
): ContractReportGate {
  const openFindingCount = review.findings.filter((finding) => finding.status === "open").length;
  const blockers: ContractReportBlocker[] = [];

  if (!capabilities.reportGeneration) {
    blockers.push({ code: "capabilityUnavailable", message: "当前运行环境不支持生成审查报告。" });
  }
  if (!review.session.extractionId) {
    blockers.push({ code: "extractionMissing", message: "合同解析结果尚未保存，不能生成报告。" });
  }
  if (!["awaitingConfirmation", "completed"].includes(review.session.status)) {
    blockers.push({ code: "reviewState", message: "合同审查尚未进入人工确认或完成状态。" });
  }
  if (openFindingCount > 0) {
    blockers.push({
      code: "findingsAwaitingDecision",
      message: `仍有 ${openFindingCount} 项风险等待人工决策。`,
    });
  }

  return { canGenerate: blockers.length === 0, openFindingCount, blockers };
}

export function evaluateContractReviewRetry(
  review: ContractReviewRecord,
  capabilities: ContractLegalCapabilityProjection = projectContractLegalCapabilities(FULL_CAPABILITIES),
): ContractReviewRetryProjection {
  if (!capabilities.stageRetry) {
    return blockedRetry("capabilityUnavailable", "当前运行环境不支持重试合同审查阶段。");
  }
  const failure = review.session.failure;
  if (!failure) return blockedRetry("failureMissing", "当前审查没有可重试的失败记录。");
  if (!failure.retryable) {
    return blockedRetry("failureNotRetryable", "当前失败不可重试，需要人工处理。");
  }
  if (review.decisions.length > 0) {
    return blockedRetry("humanDecisionExists", "已有人工决策，为保护结论不能重跑审查阶段。");
  }
  if (failure.stage === "completed") {
    return blockedRetry("completedStage", "完成阶段不能重试。");
  }

  const degradedAgent = review.session.status === "awaitingConfirmation" &&
    review.session.stage === "awaitingConfirmation" &&
    (failure.stage === "reviewingAgent" || failure.stage === "mergingFindings");
  const failedStage = review.session.status === "failed";
  if (!degradedAgent && !failedStage) {
    return blockedRetry("reviewState", "只有失败阶段或降级后的智能审查可以重试。");
  }

  const stage = failure.stage === "reviewingAgent" || failure.stage === "mergingFindings"
    ? "reviewingAgent"
    : failure.stage;
  return {
    canRetry: true,
    stage,
    mode: degradedAgent ? "degradedAgent" : "failedStage",
    blockerCode: null,
    message: degradedAgent ? "可重新运行智能审查。" : `可重试“${contractReviewStageLabel(stage)}”阶段。`,
  };
}

export function projectContractLegalReview(
  review: ContractReviewRecord,
  capabilityInput: ContractLegalCapabilityInput = FULL_CAPABILITIES,
): ContractLegalReviewProjection {
  const capabilities = projectContractLegalCapabilities(capabilityInput);
  const findings = review.findings.map(projectContractLegalFinding);
  const bySeverity = createSeverityCounts();
  for (const finding of review.findings) bySeverity[finding.severity] += 1;
  const awaitingDecision = findings.filter((finding) => finding.needsDecision).length;
  const superseded = findings.filter((finding) => finding.isSuperseded).length;

  return {
    id: review.session.id,
    workspaceId: review.session.workspaceId,
    sourceAssetId: review.session.sourceAssetId,
    sourceFileName: review.session.sourceFileName,
    status: review.session.status,
    statusLabel: CONTRACT_REVIEW_STATUS_LABELS[review.session.status],
    stage: review.session.stage,
    stageLabel: contractReviewStageLabel(review.session.stage),
    revision: review.session.revision,
    failureMessage: review.session.failure?.message ?? null,
    capabilities,
    findings,
    evidence: review.evidence.map((item) => projectContractLegalEvidence(item)),
    findingCounts: {
      total: findings.length,
      awaitingDecision,
      decided: findings.length - awaitingDecision - superseded,
      superseded,
      bySeverity,
    },
    reportGate: evaluateContractReportGate(review, capabilities),
    retry: evaluateContractReviewRetry(review, capabilities),
    preferredReport: preferredContractReviewReport(review.reports),
  };
}

export function preferredContractReviewReport(
  reports: readonly ReviewReportRecord[],
): ReviewReportRecord | null {
  const newest = (format: ReviewReportRecord["format"]) =>
    reports
      .filter((report) => report.format === format)
      .reduce<ReviewReportRecord | null>(
        (selected, report) => !selected || report.generatedAt > selected.generatedAt ? report : selected,
        null,
      );
  return newest("docx") ?? newest("html") ?? newest("json");
}

function createSeverityCounts(): Record<ReviewSeverity, number> {
  return { info: 0, low: 0, medium: 0, high: 0, critical: 0 };
}

function blockedRetry(
  blockerCode: ContractReviewRetryBlockerCode,
  message: string,
): ContractReviewRetryProjection {
  return { canRetry: false, stage: null, mode: null, blockerCode, message };
}
