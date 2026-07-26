import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Cloud,
  CloudOff,
  Download,
  FileCheck2,
  FileSearch2,
  FileText,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  LocateFixed,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  Square,
  Sparkles,
  Upload,
  X,
} from "lucide-react";
import type { AssetActionCapabilities } from "../client-sdk";
import type { AssetBackupRecord } from "../generated/bsaigc/AssetBackupRecord";
import type { AssetSourceSelection } from "../generated/bsaigc/AssetSourceSelection";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import type { ContractReviewRecord } from "../generated/bsaigc/ContractReviewRecord";
import type { ContractReviewStage } from "../generated/bsaigc/ContractReviewStage";
import type { ContractReviewStatus } from "../generated/bsaigc/ContractReviewStatus";
import type { EvidenceContext } from "../generated/bsaigc/EvidenceContext";
import type { ReviewFindingDecision } from "../generated/bsaigc/ReviewFindingDecision";
import type { ReviewFindingRecord } from "../generated/bsaigc/ReviewFindingRecord";
import type { ReviewReportFormat } from "../generated/bsaigc/ReviewReportFormat";
import type { ReviewSeverity } from "../generated/bsaigc/ReviewSeverity";
import "./ContractReviewCenter.css";

export interface ContractReviewCenterProps {
  reviews: readonly ContractReviewRecord[];
  selectedReviewId: string | null;
  selectedReview: ContractReviewRecord | null;
  findings: readonly ReviewFindingRecord[];
  selectedFindingId: string | null;
  evidenceContext: EvidenceContext | null;
  backups: readonly AssetBackupRecord[];
  businessWorkspace: BusinessWorkspaceRecord | null;
  assetActionCapabilities: Readonly<Record<string, AssetActionCapabilities>>;
  selectedSource: AssetSourceSelection | null;
  hasSelectedProject: boolean;
  isDesktopRuntime: boolean;
  isLoading: boolean;
  busyAction: string | null;
  error: string | null;
  onChooseSource: () => void;
  onClearSource: () => void;
  onImportSource: () => void;
  onRefresh: () => void;
  onSelectReview: (reviewId: string) => void;
  onStartReview: (review: ContractReviewRecord) => void;
  onCancelReview: (review: ContractReviewRecord) => void;
  onRetryStage: (review: ContractReviewRecord) => void;
  onSelectFinding: (finding: ReviewFindingRecord) => void;
  onSelectEvidence: (evidenceId: string) => void;
  onDecideFinding: (
    finding: ReviewFindingRecord,
    decision: ReviewFindingDecision,
    comment: string,
  ) => void;
  onGenerateReport: (
    review: ContractReviewRecord,
    format: ReviewReportFormat,
  ) => void;
  onPromoteReviewedContract: (review: ContractReviewRecord) => void;
  onOpenAsset: (assetId: string) => void;
  onExportAsset: (assetId: string) => void;
  onRetryBackup: (backup: AssetBackupRecord) => void;
  onRestoreBackup: (backup: AssetBackupRecord) => void;
}

const STAGE_LABELS: Record<ContractReviewStage, string> = {
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

const STATUS_LABELS: Record<ContractReviewStatus, string> = {
  draft: "草稿",
  running: "处理中",
  awaitingConfirmation: "待确认",
  completed: "已完成",
  failed: "需处理",
  cancelled: "已取消",
};

const SEVERITY_LABELS: Record<ReviewSeverity, string> = {
  critical: "严重",
  high: "高风险",
  medium: "中风险",
  low: "低风险",
  info: "提示",
};

const DECISIONS: ReadonlyArray<{
  value: ReviewFindingDecision;
  label: string;
  hint: string;
}> = [
  { value: "confirmed", label: "确认风险", hint: "进入报告并要求处理" },
  { value: "needsRevision", label: "要求修改", hint: "形成合同修改意见" },
  { value: "acceptedRisk", label: "接受风险", hint: "记录原因后继续" },
  { value: "rejected", label: "驳回发现", hint: "认定该项不成立" },
];

function reviewFailureCopy(
  code: string,
  decidedCount = 0,
): { title: string; detail: string } {
  if (code.startsWith("CONTRACT_AGENT_")) {
    return {
      title: "智能审查暂不可用",
      detail:
        decidedCount > 0
          ? "规则检查结果已保留。已产生人工决策，为保护人工结论不再重跑智能审查，可继续逐条确认并生成报告。"
          : "规则检查结果已保留，可继续人工确认，或稍后重试智能审查。",
    };
  }
  if (code.includes("EXTRACTION") || code.includes("OCR")) {
    return {
      title: "合同解析未完成",
      detail: "本地原件已保留，请重试当前步骤。",
    };
  }
  return {
    title: "当前步骤未完成",
    detail: "本地资料已保留，可稍后重试。",
  };
}

export function ContractReviewCenter(props: ContractReviewCenterProps) {
  const [decision, setDecision] =
    useState<ReviewFindingDecision>("confirmed");
  const [comment, setComment] = useState("");
  const [reportFormat, setReportFormat] =
    useState<ReviewReportFormat>("html");

  const selectedFinding = useMemo(
    () =>
      props.findings.find((finding) => finding.id === props.selectedFindingId) ??
      null,
    [props.findings, props.selectedFindingId],
  );

  const savedDecision = useMemo(() => {
    if (!selectedFinding || !props.selectedReview) return null;
    return (
      [...props.selectedReview.decisions]
        .filter((item) => item.findingId === selectedFinding.id)
        .sort((left, right) => right.createdAt - left.createdAt)[0] ?? null
    );
  }, [props.selectedReview, selectedFinding]);

  useEffect(() => {
    setDecision(
      selectedFinding && selectedFinding.decision !== "unreviewed"
        ? selectedFinding.decision
        : "confirmed",
    );
    setComment(savedDecision?.comment ?? "");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedFinding?.id, selectedFinding?.revision]);

  const reviewBackups = useMemo(() => {
    if (!props.selectedReview) return [];
    const assetIds = new Set<string>([
      props.selectedReview.session.sourceAssetId,
      ...props.selectedReview.reports.map((report) => report.reportAssetId),
    ]);
    const snapshotAssetId = props.selectedReview.extraction?.snapshotAssetId;
    if (snapshotAssetId) assetIds.add(snapshotAssetId);
    return props.backups.filter((backup) => assetIds.has(backup.assetId));
  }, [props.backups, props.selectedReview]);

  const preferredReport = useMemo(() => {
    if (!props.selectedReview) return null;
    const newest = (format: ReviewReportFormat) =>
      [...props.selectedReview!.reports]
        .filter((report) => report.format === format)
        .sort((left, right) => right.generatedAt - left.generatedAt)[0] ?? null;
    return newest("docx") ?? newest("html") ?? newest("json");
  }, [props.selectedReview]);
  const promotedDocument = useMemo(
    () =>
      props.selectedReview
        ? props.businessWorkspace?.documents.find(
            (document) => document.reviewId === props.selectedReview!.session.id,
          ) ?? null
        : null,
    [props.businessWorkspace, props.selectedReview],
  );
  const preferredReportCapabilities = preferredReport
    ? props.assetActionCapabilities[preferredReport.reportAssetId]
    : undefined;

  const decidedCount = props.findings.filter(
    (finding) => finding.decision !== "unreviewed",
  ).length;
  const unresolvedFindingCount = props.findings.length - decidedCount;
  const selectedFailure = props.selectedReview?.session.failure ?? null;
  const failureCopy = selectedFailure
    ? reviewFailureCopy(selectedFailure.code, decidedCount)
    : null;
  const canRetrySelectedReview = Boolean(
    props.selectedReview &&
      selectedFailure?.retryable &&
      decidedCount === 0 &&
      (props.selectedReview.session.status === "failed" ||
        (props.selectedReview.session.status === "awaitingConfirmation" &&
          (selectedFailure.stage === "reviewingAgent" ||
            selectedFailure.stage === "mergingFindings"))),
  );
  const canCancelSelectedReview = Boolean(
    props.selectedReview &&
      !["completed", "cancelled"].includes(props.selectedReview.session.status),
  );
  const canDecideFinding = Boolean(
    props.selectedReview &&
      props.selectedReview.session.status === "awaitingConfirmation" &&
      props.selectedReview.session.stage === "awaitingConfirmation",
  );
  const isImporting = props.busyAction === "import";
  const isReviewBusy = Boolean(
    props.busyAction &&
      props.selectedReview &&
      props.busyAction.endsWith(props.selectedReview.session.id),
  );

  return (
    <section className="contract-review-center">
      <header className="contract-review-center__header">
        <div className="contract-review-center__heading">
          <FileSearch2 size={18} />
          <h1>合同审查</h1>
        </div>
      </header>

      {props.error && (
        <div className="contract-review-center__error" role="alert">
          <AlertTriangle size={15} />
          <span>{props.error}</span>
        </div>
      )}

      <div className="contract-review-center__grid">
        <aside className="contract-review-center__pane contract-review-center__rail">
          <div className="contract-review-center__pane-header">
            <strong>合同记录</strong>
            <div className="contract-review-center__pane-tools">
              <span className="contract-review-center__count">{props.reviews.length}</span>
              <button
                type="button"
                className="contract-review-center__refresh"
                onClick={props.onRefresh}
                disabled={props.isLoading}
                title="刷新合同审查"
                aria-label="刷新合同审查"
              >
                <RefreshCw size={14} className={props.isLoading ? "is-spin" : ""} />
              </button>
            </div>
          </div>

          <div className="contract-review-center__import-card">
            <div className="contract-review-center__import-icon">
              <Upload size={17} />
            </div>
            <div className="contract-review-center__import-copy">
              <strong>{props.selectedSource?.displayName ?? "导入合同"}</strong>
              <span>
                {props.selectedSource
                  ? `${formatBytes(props.selectedSource.sizeBytes)} · 待建档`
                  : "支持 PDF / DOCX"}
              </span>
            </div>
            <div className="contract-review-center__import-actions">
              {props.selectedSource && (
                <button type="button" onClick={props.onClearSource}>
                  <X size={13} /> 清除
                </button>
              )}
              <button
                type="button"
                onClick={props.onChooseSource}
                disabled={!props.isDesktopRuntime || isImporting}
              >
                选择文件
              </button>
              <button
                type="button"
                className="is-primary"
                onClick={props.onImportSource}
                disabled={
                  !props.selectedSource ||
                  !props.hasSelectedProject ||
                  !props.isDesktopRuntime ||
                  isImporting
                }
              >
                {isImporting ? <LoaderCircle size={13} className="is-spin" /> : <HardDrive size={13} />}
                {isImporting ? "保存中" : "保存并建档"}
              </button>
            </div>
          </div>

          <div className="contract-review-center__review-list">
            {props.reviews.length === 0 ? (
              <EmptyState
                icon={<FileText size={21} />}
                title="导入第一份合同"
                description="选择 PDF 或 DOCX 开始审查。"
              />
            ) : (
              props.reviews.map((review) => {
                const selected = review.session.id === props.selectedReviewId;
                return (
                  <button
                    type="button"
                    key={review.session.id}
                    className={`contract-review-center__review-row ${selected ? "is-selected" : ""}`}
                    onClick={() => props.onSelectReview(review.session.id)}
                  >
                    <span className="contract-review-center__file-icon">
                      <FileText size={16} />
                    </span>
                    <span className="contract-review-center__review-copy">
                      <strong title={review.session.sourceFileName}>
                        {review.session.sourceFileName}
                      </strong>
                      <span>{STAGE_LABELS[review.session.stage]}</span>
                    </span>
                    <span className={`contract-review-center__status is-${review.session.status}`}>
                      {STATUS_LABELS[review.session.status]}
                    </span>
                  </button>
                );
              })
            )}
          </div>

          {props.selectedReview && (
            <div className="contract-review-center__review-actions">
              {props.selectedReview.session.status === "draft" && (
                <button
                  type="button"
                  className="is-primary"
                  onClick={() => props.onStartReview(props.selectedReview!)}
                  disabled={isReviewBusy}
                >
                  <Sparkles size={14} /> 开始智能审查
                </button>
              )}
              {canCancelSelectedReview && (
                <button
                  type="button"
                  className="is-danger"
                  onClick={() => props.onCancelReview(props.selectedReview!)}
                  disabled={
                    props.busyAction === `cancel:${props.selectedReview.session.id}`
                  }
                  title="停止当前审查，已保存的本地资料不会删除"
                >
                  <Square size={13} /> 取消审查
                </button>
              )}
              {canRetrySelectedReview && (
                <button
                  type="button"
                  className="is-warning"
                  onClick={() => props.onRetryStage(props.selectedReview!)}
                  disabled={isReviewBusy}
                >
                  <RotateCcw size={14} />
                  {selectedFailure?.stage === "reviewingAgent" ||
                  selectedFailure?.stage === "mergingFindings"
                    ? "重试智能审查"
                    : "重试当前步骤"}
                </button>
              )}
              <span>
                更新于 {formatTime(props.selectedReview.session.updatedAt)}
              </span>
            </div>
          )}
        </aside>

        <main className="contract-review-center__pane contract-review-center__evidence-pane">
          <div className="contract-review-center__pane-header">
            <strong>原文与证据</strong>
            {props.evidenceContext && (
              <span className="contract-review-center__page-chip">
                第 {props.evidenceContext.page.pageIndex + 1} 页
              </span>
            )}
          </div>

          {!props.selectedReview ? (
            <EmptyState
              icon={<FileSearch2 size={23} />}
              title="选择合同"
              description="查看解析原文与证据。"
            />
          ) : !props.evidenceContext ? (
            <div className="contract-review-center__document-empty">
              <FileText size={28} />
              <strong>{props.selectedReview.session.sourceFileName}</strong>
              <span>
                {props.selectedReview.extraction
                  ? "选择右侧风险项定位原文。"
                  : "启动审查后在此查看解析内容。"}
              </span>
              <StageProgress stage={props.selectedReview.session.stage} />
            </div>
          ) : (
            <div className="contract-review-center__document">
              <div className="contract-review-center__document-toolbar">
                <span>
                  <LocateFixed size={14} /> 证据定位
                </span>
              </div>
              <article className="contract-review-center__paper">
                <div className="contract-review-center__paper-meta">
                  <span>{props.selectedReview.session.sourceFileName}</span>
                  <span>第 {props.evidenceContext.page.pageIndex + 1} 页</span>
                </div>
                <div className="contract-review-center__page-text">
                  {highlightQuote(
                    props.evidenceContext.page.text,
                    props.evidenceContext.evidence.quotedText,
                  )}
                </div>
                <blockquote>
                  <span>引用原文</span>
                  {props.evidenceContext.evidence.quotedText}
                </blockquote>
              </article>
            </div>
          )}
        </main>

        <aside className="contract-review-center__pane contract-review-center__review-pane">
          <div className="contract-review-center__pane-header">
            <strong>风险与人工决策</strong>
            <span className="contract-review-center__count">
              {decidedCount}/{props.findings.length}
            </span>
          </div>

          {failureCopy && (
            <div className="contract-review-center__failure">
              <ShieldAlert size={16} />
              <div>
                <strong>{failureCopy.title}</strong>
                <span>{failureCopy.detail}</span>
              </div>
            </div>
          )}

          <div className="contract-review-center__finding-list">
            {props.findings.length === 0 ? (
              <EmptyState
                icon={<ShieldAlert size={21} />}
                title="暂无风险项"
                description="启动审查后在此确认风险。"
              />
            ) : (
              props.findings.map((finding) => (
                <button
                  type="button"
                  key={finding.id}
                  className={`contract-review-center__finding-row is-${finding.severity} ${finding.id === props.selectedFindingId ? "is-selected" : ""}`}
                  onClick={() => props.onSelectFinding(finding)}
                >
                  <span className="contract-review-center__severity">
                    {SEVERITY_LABELS[finding.severity]}
                  </span>
                  <span className="contract-review-center__finding-copy">
                    <strong>{finding.title}</strong>
                    <span>{finding.category}</span>
                  </span>
                  {finding.decision !== "unreviewed" && <Check size={14} />}
                </button>
              ))
            )}
          </div>

          {selectedFinding && (
            <section className="contract-review-center__finding-detail">
              <div className="contract-review-center__finding-title">
                <span className={`contract-review-center__severity is-${selectedFinding.severity}`}>
                  {SEVERITY_LABELS[selectedFinding.severity]}
                </span>
                <strong>{selectedFinding.title}</strong>
              </div>
              <p>{selectedFinding.description}</p>
              <div className="contract-review-center__recommendation">
                <span>建议</span>
                <p>{selectedFinding.recommendation}</p>
              </div>
              {selectedFinding.evidenceIds.length > 0 && (
                <div className="contract-review-center__evidence-links">
                  {selectedFinding.evidenceIds.map((evidenceId, index) => (
                    <button
                      type="button"
                      key={evidenceId}
                      onClick={() => props.onSelectEvidence(evidenceId)}
                    >
                      <LocateFixed size={13} /> 原文证据 {index + 1}
                    </button>
                  ))}
                </div>
              )}
              {selectedFinding.evidenceIds.length === 0 &&
                selectedFinding.missingEvidenceReason && (
                  <div className="contract-review-center__missing-evidence" role="note">
                    <AlertTriangle size={14} />
                    <span>{selectedFinding.missingEvidenceReason}</span>
                  </div>
                )}
              <div className="contract-review-center__decision-grid">
                {DECISIONS.map((item) => (
                  <button
                    type="button"
                    key={item.value}
                    className={decision === item.value ? "is-selected" : ""}
                    onClick={() => setDecision(item.value)}
                    disabled={!canDecideFinding}
                  >
                    <strong>{item.label}</strong>
                    <span>{item.hint}</span>
                  </button>
                ))}
              </div>
              <textarea
                value={comment}
                onChange={(event) => setComment(event.currentTarget.value)}
                placeholder="记录判断依据、客户确认或修改要求…"
                rows={3}
                disabled={!canDecideFinding}
              />
              <button
                type="button"
                className="contract-review-center__decision-submit"
                onClick={() =>
                  props.onDecideFinding(selectedFinding, decision, comment.trim())
                }
                disabled={
                  !canDecideFinding ||
                  props.busyAction === `decision:${selectedFinding.id}`
                }
                title={
                  canDecideFinding
                    ? "保存人工判断"
                    : "当前审查阶段不可再修改人工判断"
                }
              >
                {props.busyAction === `decision:${selectedFinding.id}` ? (
                  <LoaderCircle size={14} className="is-spin" />
                ) : (
                  <CheckCircle2 size={14} />
                )}
                保存人工决策
              </button>
            </section>
          )}

          {props.selectedReview && (
            <section className="contract-review-center__delivery">
              <div className="contract-review-center__delivery-heading">
                <div>
                  <FileCheck2 size={15} />
                  <strong>报告与备份</strong>
                </div>
                <span>{props.selectedReview.reports.length} 份报告</span>
              </div>
              {preferredReport && (
                <div className="contract-review-center__report-card">
                  <div className="contract-review-center__report-card-copy">
                    <FileCheck2 size={15} />
                    <span>
                      <strong>{reportFormatLabel(preferredReport.format)}</strong>
                      <small>
                        {formatTime(preferredReport.generatedAt)} · 已保存到本地
                      </small>
                    </span>
                  </div>
                  <div className="contract-review-center__report-card-actions">
                    <button
                      type="button"
                      onClick={() => props.onOpenAsset(preferredReport.reportAssetId)}
                      disabled={!preferredReportCapabilities?.canOpen}
                      aria-label={`打开${reportFormatLabel(preferredReport.format)}`}
                      title={
                        preferredReportCapabilities?.canOpen
                          ? "打开报告"
                          : preferredReportCapabilities?.reason ?? "当前报告不可打开"
                      }
                    >
                      <FolderOpen size={13} /> 打开
                    </button>
                    <button
                      type="button"
                      onClick={() => props.onExportAsset(preferredReport.reportAssetId)}
                      disabled={!preferredReportCapabilities?.canExport}
                      aria-label={`导出${reportFormatLabel(preferredReport.format)}`}
                      title={
                        preferredReportCapabilities?.canExport
                          ? "导出报告"
                          : preferredReportCapabilities?.reason ?? "当前报告不可导出"
                      }
                    >
                      <Download size={13} /> 导出
                    </button>
                    <button
                      type="button"
                      className="is-primary"
                      onClick={() => props.onPromoteReviewedContract(props.selectedReview!)}
                      disabled={
                        Boolean(promotedDocument) ||
                        props.selectedReview.session.status !== "completed" ||
                        props.busyAction === `promote:${props.selectedReview.session.id}`
                      }
                      title={
                        promotedDocument
                          ? `已转为正式合同：${promotedDocument.documentNumber}`
                          : props.selectedReview.session.status !== "completed"
                            ? "完成审查后可转为正式合同"
                            : "优先使用 DOCX 报告，没有时使用 HTML 报告"
                      }
                    >
                      {props.busyAction === `promote:${props.selectedReview.session.id}` ? (
                        <LoaderCircle size={13} className="is-spin" />
                      ) : (
                        <CheckCircle2 size={13} />
                      )}
                      {promotedDocument ? "已转正式合同" : "转为正式合同"}
                    </button>
                  </div>
                </div>
              )}
              <div className="contract-review-center__report-actions">
                <select
                  value={reportFormat}
                  onChange={(event) =>
                    setReportFormat(event.currentTarget.value as ReviewReportFormat)
                  }
                >
                  <option value="docx">Word 审查报告</option>
                  <option value="html">网页审查报告</option>
                  <option value="json">结构化审查记录</option>
                </select>
                <button
                  type="button"
                  onClick={() =>
                    props.onGenerateReport(props.selectedReview!, reportFormat)
                  }
                  disabled={
                    !["awaitingConfirmation", "completed"].includes(
                      props.selectedReview.session.status,
                    ) ||
                    unresolvedFindingCount > 0 ||
                    props.busyAction === `report:${props.selectedReview.session.id}`
                  }
                  title={
                    unresolvedFindingCount > 0
                      ? `仍有 ${unresolvedFindingCount} 项风险未确认`
                      : "生成并保存本地审查报告"
                  }
                >
                  <FileCheck2 size={13} /> 生成审查报告
                </button>
              </div>
              {reviewBackups.length > 0 && (
                <div className="contract-review-center__backup-list">
                  {reviewBackups.map((backup) => (
                    <div
                      className={`contract-review-center__backup-row is-${backup.state}`}
                      key={backup.assetId}
                    >
                      {backup.state === "failed" ? <CloudOff size={14} /> : <Cloud size={14} />}
                      <span>
                        <strong>{backupLabel(backup)}</strong>
                      </span>
                      {backup.state === "failed" && (
                        <button
                          type="button"
                          onClick={() => props.onRetryBackup(backup)}
                          disabled={props.busyAction === `backup:${backup.assetId}`}
                        >
                          {props.busyAction === `backup:${backup.assetId}` ? (
                            <LoaderCircle size={13} className="is-spin" />
                          ) : (
                            <RefreshCw size={13} />
                          )}
                          重试
                        </button>
                      )}
                      {backup.state === "backedUp" && (
                        <button
                          type="button"
                          onClick={() => props.onRestoreBackup(backup)}
                          disabled={props.busyAction === `restore:${backup.assetId}`}
                        >
                          {props.busyAction === `restore:${backup.assetId}` ? (
                            <LoaderCircle size={13} className="is-spin" />
                          ) : (
                            <RotateCcw size={13} />
                          )}
                          恢复到本地
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </section>
          )}
        </aside>
      </div>
    </section>
  );
}

function EmptyState({
  icon,
  title,
  description,
}: {
  icon: ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className="contract-review-center__empty">
      <span>{icon}</span>
      <strong>{title}</strong>
      <p>{description}</p>
    </div>
  );
}

function StageProgress({ stage }: { stage: ContractReviewStage }) {
  const stages: ContractReviewStage[] = [
    "extracting",
    "reviewingRules",
    "reviewingAgent",
    "awaitingConfirmation",
    "completed",
  ];
  const stageStep: Record<ContractReviewStage, number> = {
    created: 0,
    extracting: 0,
    awaitingOcr: 0,
    reviewingRules: 1,
    reviewingAgent: 2,
    mergingFindings: 2,
    awaitingConfirmation: 3,
    generatingReport: 3,
    completed: 4,
  };
  const current = stageStep[stage] ?? stages.indexOf(stage);
  return (
    <ol className="contract-review-center__stage-progress">
      {stages.map((item, index) => (
        <li key={item} className={index <= current ? "is-active" : ""}>
          <span />
          {STAGE_LABELS[item]}
        </li>
      ))}
    </ol>
  );
}

function highlightQuote(text: string, quote: string) {
  const normalizedQuote = quote.trim();
  if (!normalizedQuote) return text;
  const index = text.indexOf(normalizedQuote);
  if (index < 0) return text;
  return (
    <>
      {text.slice(0, index)}
      <mark>{normalizedQuote}</mark>
      {text.slice(index + normalizedQuote.length)}
    </>
  );
}

function reportFormatLabel(format: ReviewReportFormat) {
  switch (format) {
    case "docx":
      return "Word 审查报告";
    case "html":
      return "网页审查报告";
    case "json":
      return "结构化审查记录";
  }
}

function backupLabel(backup: AssetBackupRecord) {
  switch (backup.state) {
    case "queued":
      return "等待云端备份";
    case "uploading":
      return "正在备份到云端";
    case "backedUp":
      return "云端备份完成";
    case "failed":
      return "云端备份失败，本地文件不受影响";
    case "cancelled":
      return "云端备份已取消";
    default:
      return "尚未安排云端备份";
  }
}


function formatTime(value: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}
