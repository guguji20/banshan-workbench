import { useEffect, useMemo, useRef, useState } from "react";
import { ClipboardCheck, FileCheck2, Plus, X } from "lucide-react";
import type { BusinessAcceptanceBatchRecord } from "../../generated/bsaigc/BusinessAcceptanceBatchRecord";
import type { BusinessAcceptanceMaterialInput } from "../../generated/bsaigc/BusinessAcceptanceMaterialInput";
import type { BusinessAcceptanceRequirementRecord } from "../../generated/bsaigc/BusinessAcceptanceRequirementRecord";
import type { BusinessDocumentRecord } from "../../generated/bsaigc/BusinessDocumentRecord";
import type { BusinessWorkspaceRecord } from "../../generated/bsaigc/BusinessWorkspaceRecord";
import "./acceptance-center.css";

export interface AcceptanceCenterAsset {
  id: string;
  name: string;
  kind: string;
}

export interface AcceptanceCenterProps {
  workspace: BusinessWorkspaceRecord;
  assets: readonly AcceptanceCenterAsset[];
  disabled?: boolean;
  onCreateBatch: (label: string) => void | Promise<void>;
  onAddMaterial: (batchId: string, input: BusinessAcceptanceMaterialInput) => void | Promise<void>;
  onPrepare: (batchId: string) => void | Promise<void>;
  onAdvanceDocument: (documentId: string) => void | Promise<void>;
  onOpenAsset: (assetId: string) => void | Promise<void>;
  onClose: () => void;
}

export interface AcceptanceRequirementProgress {
  required: number;
  provided: number;
  missing: number;
}

export interface AcceptanceDocumentAction {
  kind: "advance" | "open" | "none";
  label: string;
  disabled: boolean;
  assetId: string | null;
}

interface MaterialDraft {
  assetId: string;
  groupKey: string;
  confirmed: boolean;
}

type AcceptanceActionErrors = Record<string, string>;

const BATCH_STATUS_LABELS: Record<BusinessAcceptanceBatchRecord["status"], string> = {
  collecting: "素材收集中",
  documentsPrepared: "草稿已准备",
  approved: "验收已批准",
  generated: "成果已生成",
};

const DOCUMENT_STATUS_LABELS: Record<BusinessDocumentRecord["status"], string> = {
  draft: "草稿",
  inReview: "复核中",
  approved: "已批准",
  generated: "已生成",
  effective: "已生效",
  voided: "已作废",
};

export function acceptanceRequirementProgress(
  batch: BusinessAcceptanceBatchRecord,
  requirement: BusinessAcceptanceRequirementRecord,
): AcceptanceRequirementProgress {
  const blocker = batch.readiness.blockers.find((item) => item.requirementId === requirement.id);
  if (blocker) {
    return {
      required: blocker.requiredGroupCount,
      provided: blocker.providedGroupCount,
      missing: blocker.missingGroupCount,
    };
  }

  const providedGroups = new Set(
    batch.materials
      .filter((material) => (
        material.requirementId === requirement.id
        && material.confirmed
        && material.duplicateOfMaterialId === null
        && material.groupKey.trim().length > 0
      ))
      .map((material) => material.groupKey.trim()),
  );
  const required = Math.max(0, requirement.requiredGroupCount);
  const provided = providedGroups.size;
  return { required, provided, missing: Math.max(0, required - provided) };
}

export function acceptanceDocumentsForBatch(
  workspace: BusinessWorkspaceRecord,
  batch: BusinessAcceptanceBatchRecord,
): BusinessDocumentRecord[] {
  const documentIds = new Set(batch.documentIds);
  return workspace.documents
    .filter((document) => (
      documentIds.has(document.id)
      || document.snapshot?.acceptanceBatchId === batch.id
    ))
    .sort((left, right) => left.sequenceNumber - right.sequenceNumber || left.updatedAt - right.updatedAt);
}

export function acceptanceDocumentAction(
  document: BusinessDocumentRecord,
  isReady: boolean,
): AcceptanceDocumentAction {
  if (document.status === "draft") {
    return { kind: "advance", label: "提交复核", disabled: !isReady, assetId: null };
  }
  if (document.status === "inReview") {
    return { kind: "advance", label: "批准验收", disabled: !isReady, assetId: null };
  }
  if (document.status === "approved") {
    return { kind: "advance", label: "生成正式文件", disabled: !isReady, assetId: null };
  }
  if ((document.status === "generated" || document.status === "effective") && document.outputAssetId) {
    return { kind: "open", label: "打开成果", disabled: false, assetId: document.outputAssetId };
  }
  return {
    kind: "none",
    label: document.status === "voided" ? "文档已作废" : "成果尚未就绪",
    disabled: true,
    assetId: null,
  };
}

function defaultMaterialDraft(): MaterialDraft {
  return { assetId: "", groupKey: "", confirmed: false };
}

const ACCEPTANCE_ASSET_KIND_ALIASES: Record<BusinessAcceptanceRequirementRecord["kind"], readonly string[]> = {
  script: ["script", "document"],
  video: ["video"],
  screenshot: ["screenshot", "image"],
  behindTheScenes: ["behindTheScenes", "image", "video"],
  publishingData: ["publishingData", "document", "image"],
  invoice: ["invoice", "document"],
  proof: ["proof", "document", "image"],
  other: ["other"],
};

function normalizeAssetKind(kind: string): string {
  return kind.trim().toLowerCase().replace(/[\s_-]+/g, "");
}

export function acceptanceAssetMatchesRequirement(
  asset: AcceptanceCenterAsset,
  requirementKind: BusinessAcceptanceRequirementRecord["kind"],
): boolean {
  const normalizedAssetKind = normalizeAssetKind(asset.kind);
  return ACCEPTANCE_ASSET_KIND_ALIASES[requirementKind]
    .some((kind) => normalizeAssetKind(kind) === normalizedAssetKind);
}

function actionErrorMessage(label: string, error: unknown): string {
  const rawDetail = error instanceof Error
    ? error.message
    : typeof error === "object" && error !== null && "message" in error && typeof error.message === "string"
      ? error.message
      : "";
  return `${label}失败：${rawDetail.trim() || "请稍后重试"}`;
}

export function AcceptanceCenter({
  workspace,
  assets,
  disabled = false,
  onCreateBatch,
  onAddMaterial,
  onPrepare,
  onAdvanceDocument,
  onOpenAsset,
  onClose,
}: AcceptanceCenterProps) {
  const [selectedBatchId, setSelectedBatchId] = useState(workspace.acceptanceBatches[0]?.id ?? "");
  const [newBatchLabel, setNewBatchLabel] = useState("");
  const [materialDrafts, setMaterialDrafts] = useState<Record<string, MaterialDraft>>({});
  const [pendingActions, setPendingActions] = useState<ReadonlySet<string>>(() => new Set());
  const [actionErrors, setActionErrors] = useState<AcceptanceActionErrors>({});
  const pendingActionsRef = useRef(new Set<string>());

  useEffect(() => {
    if (!workspace.acceptanceBatches.some((batch) => batch.id === selectedBatchId)) {
      setSelectedBatchId(workspace.acceptanceBatches[0]?.id ?? "");
    }
  }, [selectedBatchId, workspace.acceptanceBatches]);

  const selectedBatch = useMemo(
    () => workspace.acceptanceBatches.find((batch) => batch.id === selectedBatchId) ?? workspace.acceptanceBatches[0] ?? null,
    [selectedBatchId, workspace.acceptanceBatches],
  );
  const documents = useMemo(
    () => selectedBatch ? acceptanceDocumentsForBatch(workspace, selectedBatch) : [],
    [selectedBatch, workspace],
  );

  const runExclusive = async (
    actionKey: string,
    actionLabel: string,
    operation: () => void | Promise<void>,
    onSuccess?: () => void,
  ) => {
    if (disabled || pendingActionsRef.current.has(actionKey)) return;

    pendingActionsRef.current.add(actionKey);
    setPendingActions(new Set(pendingActionsRef.current));
    setActionErrors((current) => {
      if (!(actionKey in current)) return current;
      const next = { ...current };
      delete next[actionKey];
      return next;
    });

    try {
      await operation();
      onSuccess?.();
    } catch (error) {
      setActionErrors((current) => ({
        ...current,
        [actionKey]: actionErrorMessage(actionLabel, error),
      }));
    } finally {
      pendingActionsRef.current.delete(actionKey);
      setPendingActions(new Set(pendingActionsRef.current));
    }
  };

  const createBatch = () => {
    const label = newBatchLabel.trim();
    if (!label || disabled) return;
    void runExclusive(
      "create-batch",
      "创建验收批次",
      () => onCreateBatch(label),
      () => setNewBatchLabel(""),
    );
  };

  const updateMaterialDraft = (requirementId: string, patch: Partial<MaterialDraft>) => {
    setMaterialDrafts((current) => ({
      ...current,
      [requirementId]: { ...(current[requirementId] ?? defaultMaterialDraft()), ...patch },
    }));
  };

  const addMaterial = (requirement: BusinessAcceptanceRequirementRecord) => {
    if (!selectedBatch || disabled) return;
    const draft = materialDrafts[requirement.id] ?? defaultMaterialDraft();
    const groupKey = draft.groupKey.trim();
    if (!draft.assetId || !groupKey || !draft.confirmed) return;
    const batchId = selectedBatch.id;
    void runExclusive(
      `add-material:${batchId}:${requirement.id}`,
      "绑定素材",
      () => onAddMaterial(batchId, {
        id: null,
        requirementId: requirement.id,
        assetId: draft.assetId,
        kind: requirement.kind,
        groupKey,
        confirmed: true,
        duplicateOfMaterialId: null,
        notes: "",
      }),
      () => setMaterialDrafts((current) => ({ ...current, [requirement.id]: defaultMaterialDraft() })),
    );
  };

  const createPending = pendingActions.has("create-batch");

  return (
    <div className="bw-acceptance-backdrop" role="presentation">
      <section className="bw-acceptance-center" role="dialog" aria-modal="true" aria-labelledby="bw-acceptance-title">
        <header className="bw-acceptance-header">
          <div>
            <span className="bw-acceptance-eyebrow">华邦互娱商务系统 1.0</span>
            <h2 id="bw-acceptance-title">独立验收中心</h2>
            <p>按需求组收齐素材，再推进验收文档复核、批准和生成。</p>
          </div>
          <button type="button" className="bw-acceptance-icon-button" aria-label="关闭验收中心" disabled={disabled} onClick={onClose}>
            <X size={18} />
          </button>
        </header>

        {workspace.acceptanceBatches.length === 0 ? (
          <div className="bw-acceptance-empty">
            <ClipboardCheck size={34} aria-hidden="true" />
            <h3>还没有验收批次</h3>
            <p>创建第一个批次后，即可按需求组绑定素材并准备验收草稿。</p>
            <div className="bw-acceptance-create-row">
              <label>
                批次名称
                <input
                  value={newBatchLabel}
                  disabled={disabled || createPending}
                  placeholder="例如：第一阶段交付验收"
                  onChange={(event) => setNewBatchLabel(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") createBatch();
                  }}
                />
              </label>
              <button
                type="button"
                className="is-primary"
                disabled={disabled || createPending || !newBatchLabel.trim()}
                aria-busy={createPending || undefined}
                onClick={createBatch}
              >
                <Plus size={16} />{createPending ? "创建中…" : "创建验收批次"}
              </button>
            </div>
            {actionErrors["create-batch"] && (
              <div className="bw-acceptance-action-error" role="alert">{actionErrors["create-batch"]}</div>
            )}
          </div>
        ) : selectedBatch ? (
          <>
            <div className="bw-acceptance-toolbar">
              <label>
                当前批次
                <select value={selectedBatch.id} disabled={disabled} onChange={(event) => setSelectedBatchId(event.target.value)}>
                  {workspace.acceptanceBatches.map((batch) => (
                    <option key={batch.id} value={batch.id}>{batch.label}</option>
                  ))}
                </select>
              </label>
              <div className="bw-acceptance-statuses">
                <span>{BATCH_STATUS_LABELS[selectedBatch.status]}</span>
                <span className={selectedBatch.readiness.isReady ? "is-ready" : "is-blocked"}>
                  {selectedBatch.readiness.isReady ? "素材齐备" : `${selectedBatch.readiness.blockers.length} 项阻塞`}
                </span>
              </div>
            </div>

            <div className="bw-acceptance-layout">
              <main className="bw-acceptance-requirements">
                <div className="bw-acceptance-section-heading">
                  <div>
                    <h3>验收素材需求</h3>
                    <p>每个 groupKey 代表一组独立交付，只有已确认且非重复素材计入 provided。</p>
                  </div>
                </div>

                {selectedBatch.requirements.length === 0 ? (
                  <div className="bw-acceptance-inline-empty">当前批次尚未配置素材需求。</div>
                ) : (
                  <div className="bw-acceptance-requirement-list">
                    {selectedBatch.requirements.map((requirement) => {
                      const progress = acceptanceRequirementProgress(selectedBatch, requirement);
                      const draft = materialDrafts[requirement.id] ?? defaultMaterialDraft();
                      const canBind = Boolean(draft.assetId && draft.groupKey.trim() && draft.confirmed);
                      const matchingAssets = assets.filter((asset) => acceptanceAssetMatchesRequirement(asset, requirement.kind));
                      const materialActionKey = `add-material:${selectedBatch.id}:${requirement.id}`;
                      const materialPending = pendingActions.has(materialActionKey);
                      return (
                        <article className={`bw-acceptance-requirement${progress.missing > 0 ? " is-missing" : ""}`} key={requirement.id}>
                          <div className="bw-acceptance-requirement-head">
                            <div>
                              <span className="bw-acceptance-kind">{requirement.kind}</span>
                              <h4>{requirement.label}</h4>
                            </div>
                            <dl className="bw-acceptance-progress" aria-label={`${requirement.label}素材进度`}>
                              <div><dt>required</dt><dd>{progress.required}</dd></div>
                              <div><dt>provided</dt><dd>{progress.provided}</dd></div>
                              <div className={progress.missing > 0 ? "is-missing" : ""}><dt>missing</dt><dd>{progress.missing}</dd></div>
                            </dl>
                          </div>

                          {progress.missing > 0 ? (
                            <div className="bw-acceptance-blocker" role="alert">
                              缺少 {progress.missing} 组确认素材，文档审批与生成已阻止。
                            </div>
                          ) : (
                            <div className="bw-acceptance-complete">该需求素材已齐备。</div>
                          )}

                          <div className="bw-acceptance-material-form">
                            <label>
                              选择资产
                              <select value={draft.assetId} disabled={disabled || materialPending} onChange={(event) => updateMaterialDraft(requirement.id, { assetId: event.target.value })}>
                                <option value="">请选择资产</option>
                                {matchingAssets.map((asset) => (
                                  <option key={asset.id} value={asset.id}>{asset.name} · {asset.kind}</option>
                                ))}
                              </select>
                            </label>
                            <label>
                              groupKey
                              <input
                                value={draft.groupKey}
                                disabled={disabled || materialPending}
                                placeholder="例如：video-01"
                                onChange={(event) => updateMaterialDraft(requirement.id, { groupKey: event.target.value })}
                              />
                            </label>
                            <label className="bw-acceptance-confirm">
                              <input
                                type="checkbox"
                                checked={draft.confirmed}
                                disabled={disabled || materialPending}
                                onChange={(event) => updateMaterialDraft(requirement.id, { confirmed: event.target.checked })}
                              />
                              已人工确认
                            </label>
                            <button
                              type="button"
                              disabled={disabled || materialPending || !canBind}
                              aria-busy={materialPending || undefined}
                              onClick={() => addMaterial(requirement)}
                            >
                              {materialPending ? "绑定中…" : "绑定素材"}
                            </button>
                          </div>
                          {actionErrors[materialActionKey] && (
                            <div className="bw-acceptance-action-error" role="alert">{actionErrors[materialActionKey]}</div>
                          )}
                        </article>
                      );
                    })}
                  </div>
                )}
              </main>

              <aside className="bw-acceptance-documents">
                <div className="bw-acceptance-section-heading">
                  <div>
                    <h3>验收文档</h3>
                    <p>草稿可提前准备，素材齐备后才能推进审批和生成。</p>
                  </div>
                  <FileCheck2 size={22} aria-hidden="true" />
                </div>

                <button
                  type="button"
                  className="bw-acceptance-prepare"
                  disabled={disabled || pendingActions.has(`prepare:${selectedBatch.id}`)}
                  aria-busy={pendingActions.has(`prepare:${selectedBatch.id}`) || undefined}
                  onClick={() => void runExclusive(
                    `prepare:${selectedBatch.id}`,
                    "准备验收草稿",
                    () => onPrepare(selectedBatch.id),
                  )}
                >
                  {pendingActions.has(`prepare:${selectedBatch.id}`) ? "准备中…" : "准备验收草稿"}
                </button>
                {actionErrors[`prepare:${selectedBatch.id}`] && (
                  <div className="bw-acceptance-action-error" role="alert">{actionErrors[`prepare:${selectedBatch.id}`]}</div>
                )}

                {!selectedBatch.readiness.isReady && (
                  <div className="bw-acceptance-readiness-warning" role="alert">
                    素材尚未齐备：可以先准备草稿，但暂时不能审批或生成验收成果。
                  </div>
                )}

                <div className="bw-acceptance-document-list">
                  {documents.length === 0 ? (
                    <div className="bw-acceptance-inline-empty">尚无验收文档，先准备草稿。</div>
                  ) : documents.map((document) => {
                    const action = acceptanceDocumentAction(document, selectedBatch.readiness.isReady);
                    const documentActionKey = `document:${document.id}`;
                    const documentPending = pendingActions.has(documentActionKey);
                    return (
                      <article className="bw-acceptance-document" key={document.id}>
                        <div>
                          <span>{DOCUMENT_STATUS_LABELS[document.status]}</span>
                          <h4>{document.title || document.documentNumber || `验收文档 V${document.sequenceNumber}`}</h4>
                          <p>{document.outputFormat ? document.outputFormat.toUpperCase() : "待生成"}</p>
                        </div>
                        <button
                          type="button"
                          className={action.kind === "open" ? "is-primary" : ""}
                          disabled={disabled || action.disabled || documentPending}
                          aria-busy={documentPending || undefined}
                          onClick={() => {
                            if (action.kind === "advance") {
                              void runExclusive(documentActionKey, action.label, () => onAdvanceDocument(document.id));
                            }
                            if (action.kind === "open" && action.assetId) {
                              const assetId = action.assetId;
                              void runExclusive(documentActionKey, action.label, () => onOpenAsset(assetId));
                            }
                          }}
                        >
                          {documentPending ? `${action.label}中…` : action.label}
                        </button>
                        {actionErrors[documentActionKey] && (
                          <div className="bw-acceptance-action-error" role="alert">{actionErrors[documentActionKey]}</div>
                        )}
                      </article>
                    );
                  })}
                </div>
              </aside>
            </div>
          </>
        ) : null}
      </section>
    </div>
  );
}
