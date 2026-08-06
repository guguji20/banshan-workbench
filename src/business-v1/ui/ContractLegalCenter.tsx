import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  ExternalLink,
  FileCheck2,
  FilePlus2,
  FileText,
  LoaderCircle,
  Play,
  RefreshCw,
  RotateCcw,
  Scale,
  ShieldAlert,
  X,
} from "lucide-react";
import type { BsaigcClient } from "../../client-sdk/BsaigcClient";
import type { BusinessWorkspaceRecord } from "../../generated/bsaigc/BusinessWorkspaceRecord";
import type { ContractReviewRecord } from "../../generated/bsaigc/ContractReviewRecord";
import type { EvidenceContext } from "../../generated/bsaigc/EvidenceContext";
import type { ReviewFindingDecision } from "../../generated/bsaigc/ReviewFindingDecision";
import type { ReviewFindingRecord } from "../../generated/bsaigc/ReviewFindingRecord";
import {
  contractLegalDecisionLabel,
  projectContractLegalFinding,
  projectContractLegalReview,
} from "../application/contractLegalView";
import "./contract-legal.css";

export interface ContractLegalAttachmentCandidate {
  id: string;
  name: string;
  sourceLabel?: string;
  status?: "ready" | "processing" | "failed";
}

export interface ContractLegalCenterProps {
  client: BsaigcClient;
  projectId: string;
  workspace: BusinessWorkspaceRecord | null;
  attachmentCandidates: readonly ContractLegalAttachmentCandidate[];
  onClose: () => void;
  onOpenAsset: (assetId: string) => void | Promise<void>;
}

type CapabilityState = "checking" | "available" | "unavailable";
type HumanDecision = Exclude<ReviewFindingDecision, "unreviewed">;

const DECISIONS: ReadonlyArray<{ value: HumanDecision; label: string }> = [
  { value: "confirmed", label: "确认风险" },
  { value: "needsRevision", label: "要求修改" },
  { value: "acceptedRisk", label: "接受风险" },
  { value: "rejected", label: "驳回发现" },
];

function upsertReview(reviews: readonly ContractReviewRecord[], review: ContractReviewRecord): ContractReviewRecord[] {
  return [...reviews.filter((item) => item.session.id !== review.session.id), review].sort(
    (left, right) => right.session.updatedAt - left.session.updatedAt,
  );
}

function errorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) {
    const message = String(error.message).trim();
    if (message) return message;
  }
  return error instanceof Error && error.message.trim() ? error.message : "操作未完成，请刷新后重试。";
}

function errorCode(error: unknown): string | null {
  return error && typeof error === "object" && "code" in error ? String(error.code) : null;
}

function formatTime(value: number | null): string {
  if (!value) return "—";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(value);
}

function latestComment(review: ContractReviewRecord | null, findingId: string | null): string {
  if (!review || !findingId) return "";
  return [...review.decisions]
    .filter((item) => item.findingId === findingId)
    .sort((left, right) => right.createdAt - left.createdAt)[0]?.comment ?? "";
}

export function ContractLegalCenter({
  client,
  projectId,
  workspace,
  attachmentCandidates,
  onClose,
  onOpenAsset,
}: ContractLegalCenterProps) {
  const [capability, setCapability] = useState<CapabilityState>("checking");
  const [reviews, setReviews] = useState<ContractReviewRecord[]>([]);
  const [selectedReviewId, setSelectedReviewId] = useState<string | null>(null);
  const [selectedReview, setSelectedReview] = useState<ContractReviewRecord | null>(null);
  const [findings, setFindings] = useState<ReviewFindingRecord[]>([]);
  const [selectedFindingId, setSelectedFindingId] = useState<string | null>(null);
  const [evidence, setEvidence] = useState<EvidenceContext | null>(null);
  const [selectedEvidenceId, setSelectedEvidenceId] = useState<string | null>(null);
  const [selectedAssetId, setSelectedAssetId] = useState("");
  const [decision, setDecision] = useState<HumanDecision>("confirmed");
  const [comment, setComment] = useState("");
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const readyAttachments = useMemo(
    () => attachmentCandidates.filter((item) => item.status === undefined || item.status === "ready"),
    [attachmentCandidates],
  );
  const selectedFinding = useMemo(
    () => findings.find((item) => item.id === selectedFindingId) ?? null,
    [findings, selectedFindingId],
  );
  const selectedFindingView = selectedFinding ? projectContractLegalFinding(selectedFinding) : null;
  const reviewView = selectedReview
    ? projectContractLegalReview(selectedReview, { isDesktopRuntime: capability === "available" })
    : null;
  const latestDocxReport = useMemo(
    () => selectedReview?.reports
      .filter((report) => report.format === "docx")
      .sort((left, right) => right.generatedAt - left.generatedAt)[0] ?? null,
    [selectedReview],
  );

  const applyReview = useCallback((review: ContractReviewRecord, preferredFindingId?: string | null) => {
    const nextFinding = preferredFindingId
      ? review.findings.find((item) => item.id === preferredFindingId) ?? null
      : review.findings.find((item) => item.decision === "unreviewed") ?? review.findings[0] ?? null;
    const nextFindingId = nextFinding?.id ?? null;
    setReviews((current) => upsertReview(current, review));
    setSelectedReviewId(review.session.id);
    setSelectedReview(review);
    setFindings([...review.findings]);
    setSelectedFindingId(nextFindingId);
    setSelectedEvidenceId(nextFinding?.evidenceIds[0] ?? null);
    setComment(latestComment(review, nextFindingId));
  }, []);

  const refreshReview = useCallback(async (reviewId: string, preferredFindingId?: string | null) => {
    const [review, reviewFindings] = await Promise.all([
      client.getContractReview(reviewId),
      client.listReviewFindings({ reviewId }),
    ]);
    const refreshed = { ...review, findings: [...reviewFindings] };
    applyReview(refreshed, preferredFindingId);
    return refreshed;
  }, [applyReview, client]);

  const loadReviews = useCallback(async () => {
    if (!workspace) return;
    setBusyAction("refresh");
    setMessage(null);
    try {
      const items = [...await client.listContractReviews({ workspaceId: workspace.id, limit: 100 })];
      setCapability("available");
      setReviews(items);
      const nextReviewId = selectedReviewId && items.some((item) => item.session.id === selectedReviewId)
        ? selectedReviewId
        : items[0]?.session.id ?? null;
      if (nextReviewId) await refreshReview(nextReviewId, selectedFindingId);
      else {
        setSelectedReviewId(null);
        setSelectedReview(null);
        setFindings([]);
        setSelectedFindingId(null);
        setEvidence(null);
      }
    } catch (error) {
      setCapability(errorCode(error) === "NOT_CONFIGURED" ? "unavailable" : "checking");
      setMessage(errorMessage(error));
    } finally {
      setBusyAction(null);
    }
  }, [client, refreshReview, selectedFindingId, selectedReviewId, workspace]);

  useEffect(() => {
    if (!selectedAssetId && readyAttachments[0]) setSelectedAssetId(readyAttachments[0].id);
  }, [readyAttachments, selectedAssetId]);

  useEffect(() => {
    if (workspace) {
      void loadReviews();
      return;
    }
    setReviews([]);
    setSelectedReviewId(null);
    setSelectedReview(null);
    setFindings([]);
    setSelectedFindingId(null);
    setSelectedEvidenceId(null);
    setEvidence(null);
    setCapability("checking");
    let active = true;
    void client.listContractReviews({ limit: 1 })
      .then(() => {
        if (active) setCapability("available");
      })
      .catch((error: unknown) => {
        if (!active) return;
        setCapability(errorCode(error) === "NOT_CONFIGURED" ? "unavailable" : "checking");
        if (errorCode(error) !== "NOT_CONFIGURED") setMessage(errorMessage(error));
      });
    return () => {
      active = false;
    };
  }, [client, loadReviews, workspace]);

  useEffect(() => {
    if (!workspace || capability !== "available") return;
    let disposed = false;
    let unsubscribe: (() => void) | null = null;
    void client.subscribeContractReviewEvents((event) => {
      const review = event.contractReview;
      if (review.session.workspaceId !== workspace.id) return;
      setReviews((current) => upsertReview(current, review));
      if (review.session.id === selectedReviewId) applyReview(review, selectedFindingId);
    }).then((dispose) => {
      if (disposed) dispose();
      else unsubscribe = dispose;
    }).catch(() => undefined);
    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, [applyReview, capability, client, selectedFindingId, selectedReviewId, workspace]);

  useEffect(() => {
    const evidenceId = selectedEvidenceId ?? selectedFinding?.evidenceIds[0];
    if (!evidenceId || capability !== "available") {
      setEvidence(null);
      return;
    }
    let cancelled = false;
    setEvidence(null);
    void client.getEvidenceContext(evidenceId)
      .then((context) => {
        if (!cancelled) setEvidence(context);
      })
      .catch((error) => {
        if (!cancelled) setMessage(errorMessage(error));
      });
    return () => {
      cancelled = true;
    };
  }, [capability, client, selectedEvidenceId, selectedFinding]);

  useEffect(() => {
    setComment(latestComment(selectedReview, selectedFindingId));
    setDecision(selectedFinding?.decision !== "unreviewed" ? selectedFinding?.decision ?? "confirmed" : "confirmed");
  }, [selectedFinding, selectedFindingId, selectedReview]);

  const runReviewCommand = async (
    action: string,
    command: () => Promise<{ contractReview: ContractReviewRecord }>,
    successMessage: string,
    preferredFindingId?: string | null,
  ) => {
    if (busyAction) return;
    setBusyAction(action);
    setMessage(null);
    try {
      const response = await command();
      applyReview(response.contractReview, preferredFindingId);
      setMessage(successMessage);
    } catch (error) {
      if (errorCode(error) === "NOT_CONFIGURED") {
        setCapability("unavailable");
        setMessage("当前 WebHost 未配置合同审查能力，请在桌面版继续。");
      } else if (errorCode(error) === "REVISION_CONFLICT" && selectedReview) {
        await refreshReview(selectedReview.session.id, preferredFindingId).catch(() => undefined);
        setMessage("审查版本已更新，已刷新最新结果，请重新确认本次操作。");
      } else {
        setMessage(errorMessage(error));
      }
    } finally {
      setBusyAction(null);
    }
  };

  const createAndStartReview = async () => {
    if (!workspace || !selectedAssetId) return;
    await runReviewCommand("create", async () => {
      const created = await client.createContractReview(
        { workspaceId: workspace.id, sourceAssetId: selectedAssetId },
        {
          projectId,
          idempotencyKey: "contract-review:create:" + workspace.id + ":" + selectedAssetId + ":" + workspace.revision,
        },
      );
      return client.startContractReview(
        { reviewId: created.contractReview.session.id },
        created.contractReview.session.revision,
        {
          projectId,
          idempotencyKey: "contract-review:start:" + created.contractReview.session.id + ":" + created.contractReview.session.revision,
        },
      );
    }, "合同审查已启动。", null);
  };

  const startSelectedReview = async () => {
    if (!selectedReview) return;
    await runReviewCommand("start", () => client.startContractReview(
      { reviewId: selectedReview.session.id },
      selectedReview.session.revision,
      {
        projectId,
        idempotencyKey: "contract-review:start:" + selectedReview.session.id + ":" + selectedReview.session.revision,
      },
    ), "合同审查已启动。", selectedFindingId);
  };

  const retrySelectedReview = async () => {
    if (!selectedReview || !reviewView?.retry.canRetry || !reviewView.retry.stage) return;
    await runReviewCommand("retry", () => client.retryContractReviewStage(
      { reviewId: selectedReview.session.id, stage: reviewView.retry.stage! },
      selectedReview.session.revision,
      {
        projectId,
        idempotencyKey: "contract-review:retry:" + selectedReview.session.id + ":" + selectedReview.session.revision + ":" + reviewView.retry.stage,
      },
    ), "失败阶段已重新排队。", selectedFindingId);
  };

  const decideSelectedFinding = async () => {
    if (!selectedReview || !selectedFinding || !comment.trim()) {
      setMessage("请填写人工处理意见后再保存决策。");
      return;
    }
    await runReviewCommand("decision", () => client.decideReviewFinding(
      {
        reviewId: selectedReview.session.id,
        findingId: selectedFinding.id,
        decision,
        comment: comment.trim(),
      },
      selectedReview.session.revision,
      {
        projectId,
        idempotencyKey: "contract-review:decision:" + selectedFinding.id + ":" + selectedFinding.revision + ":" + decision,
      },
    ), "人工决策已保存，并保留确认人、意见和版本。", selectedFinding.id);
  };

  const generateDocxReport = async () => {
    if (!selectedReview || !reviewView?.reportGate.canGenerate) return;
    await runReviewCommand("report", () => client.generateReviewReport(
      { reviewId: selectedReview.session.id, format: "docx" },
      selectedReview.session.revision,
      {
        projectId,
        idempotencyKey: "contract-review:report:docx:" + selectedReview.session.id + ":" + selectedReview.session.revision,
      },
    ), "Word 审查报告已生成。", selectedFindingId);
  };

  const selectFinding = (finding: ReviewFindingRecord) => {
    setSelectedFindingId(finding.id);
    setSelectedEvidenceId(finding.evidenceIds[0] ?? null);
    setEvidence(null);
  };

  const selectEvidence = async (evidenceId: string) => {
    setSelectedEvidenceId(evidenceId);
    setEvidence(null);
    try {
      setEvidence(await client.getEvidenceContext(evidenceId));
    } catch (error) {
      if (errorCode(error) === "NOT_CONFIGURED") setCapability("unavailable");
      setMessage(errorMessage(error));
    }
  };

  const selectReview = async (reviewId: string) => {
    setSelectedReviewId(reviewId);
    setBusyAction("select");
    setMessage(null);
    try {
      await refreshReview(reviewId);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <div className="contract-legal-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget && !busyAction) onClose();
    }}>
      <section className="contract-legal-center" role="dialog" aria-modal="true" aria-labelledby="contract-legal-title" aria-busy={Boolean(busyAction)}>
        <header className="contract-legal-header">
          <div>
            <span className="contract-legal-eyebrow"><Scale size={14} /> 基础法务</span>
            <h2 id="contract-legal-title">合同审查中心</h2>
            <p>复用现有合同审查、OCR、Vault 与报告引擎；最终结论必须逐条人工确认。</p>
          </div>
          <div className="contract-legal-header__actions">
            <button type="button" onClick={() => void loadReviews()} disabled={!workspace || Boolean(busyAction)}>
              <RefreshCw size={15} className={busyAction === "refresh" ? "is-spin" : undefined} /> 刷新
            </button>
            <button type="button" className="is-icon" aria-label="关闭合同审查中心" onClick={onClose} disabled={Boolean(busyAction)}><X size={18} /></button>
          </div>
        </header>

        {message ? <div className="contract-legal-message" role="alert"><AlertTriangle size={16} />{message}</div> : null}

        {!workspace ? (
          <div className="contract-legal-empty is-prominent">
            <LoaderCircle size={28} className="is-spin" />
            <strong>正在准备当前项目工作区</strong>
            <p>桌面版会复用现有 SQLite 工作区；Web 预览不提供合同审查写入能力。</p>
          </div>
        ) : capability === "unavailable" ? (
          <div className="contract-legal-empty is-prominent">
            <ShieldAlert size={30} />
            <strong>当前运行环境不支持合同审查</strong>
            <p>请在 Windows 或 macOS 桌面版中使用；WebHost 不会伪造创建、决策或报告成功。</p>
          </div>
        ) : (
          <div className="contract-legal-layout">
            <aside className="contract-legal-sidebar">
              <div className="contract-legal-create">
                <label htmlFor="contract-legal-source">选择合同原件</label>
                <select id="contract-legal-source" value={selectedAssetId} onChange={(event) => setSelectedAssetId(event.currentTarget.value)} disabled={Boolean(busyAction)}>
                  <option value="">请选择已导入合同</option>
                  {readyAttachments.map((asset) => <option value={asset.id} key={asset.id}>{asset.name}</option>)}
                </select>
                <button type="button" className="is-primary" onClick={() => void createAndStartReview()} disabled={!selectedAssetId || Boolean(busyAction)}>
                  {busyAction === "create" ? <LoaderCircle size={15} className="is-spin" /> : <FilePlus2 size={15} />}新建并启动审查
                </button>
                {readyAttachments.length === 0 ? <small>先把 PDF、DOC 或 DOCX 合同添加到当前项目。</small> : null}
              </div>

              <div className="contract-legal-review-list" aria-label="合同审查列表">
                {reviews.map((review) => {
                  const view = projectContractLegalReview(review, { isDesktopRuntime: capability === "available" });
                  return (
                    <button type="button" className={review.session.id === selectedReviewId ? "is-active" : undefined} onClick={() => void selectReview(review.session.id)} key={review.session.id}>
                      <FileText size={16} />
                      <span><strong>{review.session.sourceFileName}</strong><small>{view.statusLabel} · {view.stageLabel}</small></span>
                      <em>{view.findingCounts.awaitingDecision}</em>
                    </button>
                  );
                })}
                {reviews.length === 0 && capability !== "checking" ? <div className="contract-legal-empty"><FileText size={24} /><span>当前项目还没有合同审查记录。</span></div> : null}
              </div>
            </aside>

            <main className="contract-legal-main">
              {!selectedReview || !reviewView ? (
                <div className="contract-legal-empty is-prominent">
                  {busyAction ? <LoaderCircle size={28} className="is-spin" /> : <Scale size={28} />}
                  <strong>{busyAction ? "正在读取合同审查" : "选择一份合同开始审查"}</strong>
                  <p>审查结果会保留来源、页码、人工意见与版本，不覆盖原合同。</p>
                </div>
              ) : (
                <>
                  <section className="contract-legal-summary">
                    <div>
                      <span className={"contract-legal-status is-" + selectedReview.session.status}>{reviewView.statusLabel}</span>
                      <h3>{selectedReview.session.sourceFileName}</h3>
                      <p>{reviewView.stageLabel} · revision {selectedReview.session.revision} · 更新于 {formatTime(selectedReview.session.updatedAt)}</p>
                    </div>
                    <div className="contract-legal-summary__actions">
                      {selectedReview.session.status === "draft" ? <button type="button" onClick={() => void startSelectedReview()} disabled={Boolean(busyAction)}><Play size={15} />启动</button> : null}
                      {reviewView.retry.canRetry ? <button type="button" onClick={() => void retrySelectedReview()} disabled={Boolean(busyAction)}><RotateCcw size={15} />重试阶段</button> : null}
                      {latestDocxReport ? <button type="button" onClick={() => void onOpenAsset(latestDocxReport.reportAssetId)}><ExternalLink size={15} />打开报告</button> : null}
                    </div>
                  </section>

                  <section className="contract-legal-metrics">
                    <div><span>风险总数</span><strong>{reviewView.findingCounts.total}</strong></div>
                    <div><span>待人工决策</span><strong>{reviewView.findingCounts.awaitingDecision}</strong></div>
                    <div><span>严重 / 高风险</span><strong>{reviewView.findingCounts.bySeverity.critical + reviewView.findingCounts.bySeverity.high}</strong></div>
                    <div><span>已保存决策</span><strong>{reviewView.findingCounts.decided}</strong></div>
                  </section>

                  {reviewView.failureMessage ? <div className="contract-legal-message"><ShieldAlert size={16} />{reviewView.failureMessage}</div> : null}

                  <div className="contract-legal-detail-grid">
                    <section className="contract-legal-findings">
                      <div className="contract-legal-section-title"><span>法务风险清单</span><em>{findings.length} 项</em></div>
                      <div className="contract-legal-finding-list">
                        {findings.map((finding) => {
                          const view = projectContractLegalFinding(finding);
                          return (
                            <button type="button" className={"is-" + finding.severity + (finding.id === selectedFindingId ? " is-active" : "")} onClick={() => selectFinding(finding)} key={finding.id}>
                              <span><strong>{view.severityLabel}</strong><em>{view.decisionLabel}</em></span>
                              <b>{finding.title}</b><small>{finding.category}</small>
                            </button>
                          );
                        })}
                        {findings.length === 0 ? <div className="contract-legal-empty"><CheckCircle2 size={24} /><span>尚未产生风险项。</span></div> : null}
                      </div>
                    </section>

                    <section className="contract-legal-finding-detail">
                      {!selectedFinding || !selectedFindingView ? (
                        <div className="contract-legal-empty"><ShieldAlert size={24} /><span>选择风险项查看建议条款和证据。</span></div>
                      ) : (
                        <>
                          <div className="contract-legal-section-title"><span>{selectedFinding.title}</span><em className={"is-" + selectedFinding.severity}>{selectedFindingView.severityLabel}</em></div>
                          <p className="contract-legal-description">{selectedFinding.description}</p>
                          <div className="contract-legal-recommendation"><strong>建议条款 / 建议动作</strong><p>{selectedFinding.recommendation || "暂无建议条款。"}</p></div>
                          <div className="contract-legal-evidence">
                            <strong>证据定位</strong>
                            {selectedFinding.evidenceIds.length > 1 ? (
                              <div className="contract-legal-evidence__tabs">
                                {selectedFinding.evidenceIds.map((evidenceId, index) => (
                                  <button type="button" className={selectedEvidenceId === evidenceId ? "is-active" : undefined} onClick={() => void selectEvidence(evidenceId)} key={evidenceId}>证据 {index + 1}</button>
                                ))}
                              </div>
                            ) : null}
                            {selectedFinding.missingEvidenceReason ? <p className="is-missing"><AlertTriangle size={15} />{selectedFinding.missingEvidenceReason}</p> : evidence ? (
                              <>
                                <blockquote><span>第 {evidence.evidence.pageIndex + 1} 页</span><p>{evidence.evidence.quotedText}</p><small>{evidence.evidence.contextBefore} {evidence.evidence.contextAfter}</small></blockquote>
                                <div className="contract-legal-evidence__actions">
                                  <button type="button" onClick={() => void onOpenAsset(evidence.evidence.sourceAssetId)}><ExternalLink size={14} />打开原文件</button>
                                  {evidence.page.previewAssetId ? <button type="button" onClick={() => void onOpenAsset(evidence.page.previewAssetId!)}><ExternalLink size={14} />查看页预览</button> : null}
                                </div>
                              </>
                            ) : <p>正在读取证据原文…</p>}
                          </div>
                          <div className="contract-legal-decision">
                            <strong>人工决策</strong>
                            {selectedFinding.decision === "unreviewed" ? (
                              <>
                                <div className="contract-legal-decision__options">
                                  {DECISIONS.map((option) => <label key={option.value}><input type="radio" name="contract-decision" value={option.value} checked={decision === option.value} onChange={() => setDecision(option.value)} />{option.label}</label>)}
                                </div>
                                <textarea value={comment} onChange={(event) => setComment(event.currentTarget.value)} placeholder="填写判断依据、修改要求或接受风险原因" rows={3} disabled={Boolean(busyAction)} />
                                <button type="button" className="is-primary" onClick={() => void decideSelectedFinding()} disabled={!comment.trim() || Boolean(busyAction)}>
                                  {busyAction === "decision" ? <LoaderCircle size={15} className="is-spin" /> : <CheckCircle2 size={15} />}保存人工决策
                                </button>
                              </>
                            ) : (
                              <div className="contract-legal-decision__saved"><CheckCircle2 size={16} /><span><strong>{contractLegalDecisionLabel(selectedFinding.decision)}</strong><p>{latestComment(selectedReview, selectedFinding.id) || "已保存人工决策。"}</p></span></div>
                            )}
                          </div>
                        </>
                      )}
                    </section>
                  </div>

                  <footer className="contract-legal-report">
                    <div><FileCheck2 size={20} /><span><strong>Word 审查报告</strong><small>{reviewView.reportGate.canGenerate ? "所有风险已完成人工决策，可以生成正式报告。" : reviewView.reportGate.blockers[0]?.message ?? "报告尚未解锁。"}</small></span></div>
                    {latestDocxReport ? <button type="button" onClick={() => void onOpenAsset(latestDocxReport.reportAssetId)}><ExternalLink size={15} />打开最新报告</button> : null}
                    <button type="button" className="is-primary" onClick={() => void generateDocxReport()} disabled={!reviewView.reportGate.canGenerate || Boolean(busyAction)}>
                      {busyAction === "report" ? <LoaderCircle size={15} className="is-spin" /> : <FileCheck2 size={15} />}生成 DOCX 报告
                    </button>
                  </footer>
                </>
              )}
          </main>
          </div>
        )}
      </section>
    </div>
  );
}

