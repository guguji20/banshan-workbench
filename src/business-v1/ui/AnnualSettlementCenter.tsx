import { useMemo, useState } from "react";
import { CalendarRange, CheckCircle2, Pencil, RotateCcw, Trash2, X } from "lucide-react";
import "./annual-settlement.css";

export type AnnualSettlementCadence = "monthly" | "quarterly" | "perOrder" | "oneOff" | "mixed";
export type AnnualSettlementBatchStatus = "draft" | "confirmed" | "voided";

export interface AnnualSettlementDeliverable {
  id: string;
  milestoneId: string;
  milestoneTitle: string;
  name: string;
  unit: string;
  contractQuantity: number;
  executedQuantity: number;
  acceptedQuantity: number;
  settledQuantity: number;
}

export interface AnnualSettlementWorkspace {
  id: string;
  projectTitle: string;
  projectCode?: string;
  customerName?: string;
  deliverables: readonly AnnualSettlementDeliverable[];
}

export interface AnnualSettlementLine {
  deliverableId: string;
  deliverableName: string;
  milestoneTitle: string;
  unit: string;
  quantity: number;
}

export interface AnnualSettlementBatch {
  id: string;
  workspaceId: string;
  period: string;
  cadence: AnnualSettlementCadence;
  status: AnnualSettlementBatchStatus;
  lines: readonly AnnualSettlementLine[];
  note: string;
  createdAt: number;
  updatedAt: number;
  voidedAt?: number | null;
}

export interface AnnualSettlementBatchInput {
  id: string | null;
  workspaceId: string;
  period: string;
  cadence: AnnualSettlementCadence;
  lines: AnnualSettlementLine[];
  note: string;
}

export interface AnnualSettlementCenterProps {
  workspace: AnnualSettlementWorkspace;
  settlementBatches: readonly AnnualSettlementBatch[];
  onUpsert: (input: AnnualSettlementBatchInput) => void | Promise<void>;
  onVoid: (batch: AnnualSettlementBatch) => void | Promise<void>;
  disabled?: boolean;
  onClose?: () => void;
}

const CADENCE_LABELS: Record<AnnualSettlementCadence, string> = {
  monthly: "月度结算",
  quarterly: "季度结算",
  perOrder: "按单结算",
  oneOff: "一次性结算",
  mixed: "混合结算",
};

const STATUS_LABELS: Record<AnnualSettlementBatchStatus, string> = {
  draft: "草稿",
  confirmed: "已确认",
  voided: "已作废",
};

function normalizedQuantity(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

export function settlementAvailableQuantity(deliverable: AnnualSettlementDeliverable): number {
  const contractRemaining = normalizedQuantity(deliverable.contractQuantity) - normalizedQuantity(deliverable.settledQuantity);
  const acceptedRemaining = normalizedQuantity(deliverable.acceptedQuantity) - normalizedQuantity(deliverable.settledQuantity);
  return Math.max(0, Math.min(contractRemaining, acceptedRemaining));
}

export function eligibleAnnualSettlementDeliverables(
  workspace: AnnualSettlementWorkspace,
  settlementBatches: readonly AnnualSettlementBatch[],
  editingBatchId: string | null = null,
): AnnualSettlementDeliverable[] {
  const reservedDeliverableIds = new Set(
    settlementBatches
      .filter((batch) => batch.status !== "voided" && batch.id !== editingBatchId)
      .flatMap((batch) => batch.lines.map((line) => line.deliverableId)),
  );
  return workspace.deliverables.filter(
    (deliverable) => !reservedDeliverableIds.has(deliverable.id) && settlementAvailableQuantity(deliverable) > 0,
  );
}

export function validateAnnualSettlementInput(
  input: AnnualSettlementBatchInput,
  workspace: AnnualSettlementWorkspace,
  settlementBatches: readonly AnnualSettlementBatch[],
): string[] {
  const errors: string[] = [];
  if (!input.period.trim()) errors.push("请填写结算期间");
  if (input.lines.length === 0) errors.push("请至少选择一个可结算交付项");

  const deliverablesById = new Map(workspace.deliverables.map((deliverable) => [deliverable.id, deliverable]));
  const eligibleIds = new Set(
    eligibleAnnualSettlementDeliverables(workspace, settlementBatches, input.id).map((deliverable) => deliverable.id),
  );
  const seen = new Set<string>();
  for (const line of input.lines) {
    if (seen.has(line.deliverableId)) {
      errors.push(`交付项“${line.deliverableName}”不能重复选择`);
      continue;
    }
    seen.add(line.deliverableId);
    const deliverable = deliverablesById.get(line.deliverableId);
    if (!deliverable || !eligibleIds.has(line.deliverableId)) {
      errors.push(`交付项“${line.deliverableName}”已结算或不可结算`);
      continue;
    }
    const availableQuantity = settlementAvailableQuantity(deliverable);
    if (!Number.isFinite(line.quantity) || line.quantity <= 0) {
      errors.push(`交付项“${deliverable.name}”的本期结算数量必须大于 0`);
    } else if (line.quantity > availableQuantity) {
      errors.push(`交付项“${deliverable.name}”最多可结算 ${formatQuantity(availableQuantity)} ${deliverable.unit}`);
    }
  }
  return errors;
}

function formatQuantity(value: number): string {
  return new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 4 }).format(value);
}

function formatDate(timestamp: number): string {
  return new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit" }).format(timestamp);
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  return "年框结算操作失败，请重试。";
}

export function AnnualSettlementCenter({ workspace, settlementBatches, onUpsert, onVoid, disabled = false, onClose }: AnnualSettlementCenterProps) {
  const [editingBatchId, setEditingBatchId] = useState<string | null>(null);
  const [period, setPeriod] = useState("");
  const [cadence, setCadence] = useState<AnnualSettlementCadence>("quarterly");
  const [note, setNote] = useState("");
  const [selectedQuantities, setSelectedQuantities] = useState<Record<string, number>>({});
  const [error, setError] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);

  const eligibleDeliverables = useMemo(
    () => eligibleAnnualSettlementDeliverables(workspace, settlementBatches, editingBatchId),
    [editingBatchId, settlementBatches, workspace],
  );

  const resetForm = () => {
    setEditingBatchId(null);
    setPeriod("");
    setCadence("quarterly");
    setNote("");
    setSelectedQuantities({});
    setError(null);
  };

  const toggleDeliverable = (deliverable: AnnualSettlementDeliverable, checked: boolean) => {
    setSelectedQuantities((current) => {
      if (!checked) {
        const next = { ...current };
        delete next[deliverable.id];
        return next;
      }
      return { ...current, [deliverable.id]: settlementAvailableQuantity(deliverable) };
    });
    setError(null);
  };

  const editBatch = (batch: AnnualSettlementBatch) => {
    setEditingBatchId(batch.id);
    setPeriod(batch.period);
    setCadence(batch.cadence);
    setNote(batch.note);
    setSelectedQuantities(Object.fromEntries(batch.lines.map((line) => [line.deliverableId, line.quantity])));
    setError(null);
  };

  const submitBatch = async () => {
    const deliverablesById = new Map(workspace.deliverables.map((deliverable) => [deliverable.id, deliverable]));
    const input: AnnualSettlementBatchInput = {
      id: editingBatchId,
      workspaceId: workspace.id,
      period: period.trim(),
      cadence,
      lines: Object.entries(selectedQuantities).map(([deliverableId, quantity]) => {
        const deliverable = deliverablesById.get(deliverableId);
        return {
          deliverableId,
          deliverableName: deliverable?.name ?? deliverableId,
          milestoneTitle: deliverable?.milestoneTitle ?? "未分组",
          unit: deliverable?.unit ?? "项",
          quantity,
        };
      }),
      note: note.trim(),
    };
    const errors = validateAnnualSettlementInput(input, workspace, settlementBatches);
    if (errors.length > 0) {
      setError(errors[0]);
      return;
    }
    setBusyAction("upsert");
    setError(null);
    try {
      await onUpsert(input);
      resetForm();
    } catch (submitError) {
      setError(errorMessage(submitError));
    } finally {
      setBusyAction(null);
    }
  };

  const voidBatch = async (batch: AnnualSettlementBatch) => {
    setBusyAction(`void:${batch.id}`);
    setError(null);
    try {
      await onVoid(batch);
      if (editingBatchId === batch.id) resetForm();
    } catch (voidError) {
      setError(errorMessage(voidError));
    } finally {
      setBusyAction(null);
    }
  };

  const selectedCount = Object.keys(selectedQuantities).length;
  const isBusy = busyAction !== null;

  return (
    <section className="bw-annual-settlement" aria-labelledby="bw-annual-settlement-title">
      <header className="bw-annual-settlement__header">
        <div className="bw-annual-settlement__heading">
          <span className="bw-annual-settlement__icon"><CalendarRange size={19} /></span>
          <div>
            <strong id="bw-annual-settlement-title">年框结算中心</strong>
            <small>{workspace.projectTitle}{workspace.projectCode ? ` · ${workspace.projectCode}` : ""}</small>
          </div>
        </div>
        <div className="bw-annual-settlement__header-actions">
          <div className="bw-annual-settlement__summary" aria-label="结算批次摘要">
            <span><b>{settlementBatches.filter((batch) => batch.status !== "voided").length}</b> 有效批次</span>
            <span><b>{eligibleDeliverables.length}</b> 可结算交付项</span>
          </div>
          {onClose ? <button type="button" className="bw-annual-settlement__close" onClick={onClose} disabled={isBusy} aria-label="关闭年框结算中心" title="关闭"><X size={17} /></button> : null}
        </div>
      </header>

      {error ? <div className="bw-annual-settlement__message" role="alert">{error}</div> : null}

      <div className="bw-annual-settlement__layout">
        <section className="bw-annual-settlement-form" aria-labelledby="bw-annual-settlement-form-title">
          <header>
            <div>
              <strong id="bw-annual-settlement-form-title">{editingBatchId ? "编辑结算批次" : "新建结算批次"}</strong>
              <small>已进入有效批次的交付项会自动排除，作废后可重新选择。</small>
            </div>
            {editingBatchId ? <button type="button" className="bw-annual-settlement__text-button" onClick={resetForm} disabled={isBusy}><RotateCcw size={14} />取消编辑</button> : null}
          </header>

          <div className="bw-annual-settlement-form__fields">
            <label><span>结算期间</span><input value={period} onChange={(event) => { setPeriod(event.target.value); setError(null); }} placeholder="例如：2026 年第三季度" disabled={disabled || isBusy} /></label>
            <label>
              <span>结算口径</span>
              <select value={cadence} onChange={(event) => setCadence(event.target.value as AnnualSettlementCadence)} disabled={disabled || isBusy}>
                {Object.entries(CADENCE_LABELS).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
              </select>
            </label>
          </div>

          <div className="bw-annual-settlement-deliverables" aria-label="可结算交付项">
            {eligibleDeliverables.length === 0 ? (
              <div className="bw-annual-settlement__empty">当前没有可结算交付项。请先完成验收，或作废占用该交付项的批次。</div>
            ) : eligibleDeliverables.map((deliverable) => {
              const availableQuantity = settlementAvailableQuantity(deliverable);
              const selected = Object.prototype.hasOwnProperty.call(selectedQuantities, deliverable.id);
              return (
                <article key={deliverable.id} className={`bw-annual-settlement-deliverable${selected ? " is-selected" : ""}`}>
                  <label className="bw-annual-settlement-deliverable__choice">
                    <input type="checkbox" checked={selected} onChange={(event) => toggleDeliverable(deliverable, event.target.checked)} disabled={disabled || isBusy} />
                    <span><strong>{deliverable.name}</strong><small>{deliverable.milestoneTitle}</small></span>
                  </label>
                  <div className="bw-annual-settlement-deliverable__metrics">
                    <span>合同 <b>{formatQuantity(deliverable.contractQuantity)}</b></span>
                    <span>累计执行 <b>{formatQuantity(deliverable.executedQuantity)}</b></span>
                    <span>累计验收 <b>{formatQuantity(deliverable.acceptedQuantity)}</b></span>
                    <span>累计结算 <b>{formatQuantity(deliverable.settledQuantity)}</b></span>
                    <span>剩余可结算 <b>{formatQuantity(availableQuantity)} {deliverable.unit}</b></span>
                  </div>
                  <label className="bw-annual-settlement-deliverable__quantity">
                    <span>本期结算</span>
                    <input type="number" min="0.0001" max={availableQuantity} step="any" value={selected ? selectedQuantities[deliverable.id] : ""} onChange={(event) => { setSelectedQuantities((current) => ({ ...current, [deliverable.id]: Number(event.target.value) })); setError(null); }} disabled={!selected || disabled || isBusy} aria-label={`${deliverable.name}本期结算数量`} />
                    <em>{deliverable.unit}</em>
                  </label>
                </article>
              );
            })}
          </div>

          <label className="bw-annual-settlement-form__note"><span>结算备注</span><textarea value={note} onChange={(event) => setNote(event.target.value)} rows={3} placeholder="记录本期范围、例外项或开票说明" disabled={disabled || isBusy} /></label>

          <footer className="bw-annual-settlement-form__footer">
            <small>已选择 {selectedCount} 个交付项</small>
            <button type="button" className="bw-annual-settlement__primary-button" onClick={() => void submitBatch()} disabled={disabled || isBusy || eligibleDeliverables.length === 0}><CheckCircle2 size={15} />{busyAction === "upsert" ? "保存中…" : editingBatchId ? "保存批次" : "创建批次"}</button>
          </footer>
        </section>

        <section className="bw-annual-settlement-batches" aria-labelledby="bw-annual-settlement-batches-title">
          <header><div><strong id="bw-annual-settlement-batches-title">结算批次</strong><small>按最近更新时间排序展示</small></div></header>
          <div className="bw-annual-settlement-batches__list">
            {settlementBatches.length === 0 ? <div className="bw-annual-settlement__empty">还没有结算批次。</div> : [...settlementBatches].sort((left, right) => right.updatedAt - left.updatedAt).map((batch) => (
              <article key={batch.id} className={`bw-annual-settlement-batch is-${batch.status}`}>
                <header><div><strong>{batch.period}</strong><small>{CADENCE_LABELS[batch.cadence]} · {formatDate(batch.updatedAt)}</small></div><span className={`bw-annual-settlement-status is-${batch.status}`}>{STATUS_LABELS[batch.status]}</span></header>
                <div className="bw-annual-settlement-batch__lines">{batch.lines.map((line) => <div key={line.deliverableId}><span>{line.deliverableName}<small>{line.milestoneTitle}</small></span><b>{formatQuantity(line.quantity)} {line.unit}</b></div>)}</div>
                {batch.note ? <p>{batch.note}</p> : null}
                {batch.status !== "voided" ? <footer>
                  {batch.status === "draft" ? <button type="button" className="bw-annual-settlement__text-button" onClick={() => editBatch(batch)} disabled={disabled || isBusy}><Pencil size={13} />编辑</button> : null}
                  <button type="button" className="bw-annual-settlement__danger-button" onClick={() => void voidBatch(batch)} disabled={disabled || isBusy}><Trash2 size={13} />{busyAction === `void:${batch.id}` ? "作废中…" : "作废"}</button>
                </footer> : null}
              </article>
            ))}
          </div>
        </section>
      </div>
    </section>
  );
}
