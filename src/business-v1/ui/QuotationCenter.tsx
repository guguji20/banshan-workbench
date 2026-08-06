import { useEffect, useMemo, useState } from "react";
import { FileSpreadsheet, Plus, Trash2, X } from "lucide-react";
import type { BusinessDocumentRecord } from "../../generated/bsaigc/BusinessDocumentRecord";
import type { BusinessTaxMode } from "../../generated/bsaigc/BusinessTaxMode";
import type { BusinessWorkspaceRecord } from "../../generated/bsaigc/BusinessWorkspaceRecord";
import { createQuotationDraft, formatCny } from "../application/quotationWorkflow";
import "./quotation-center.css";

export interface QuotationLineDraft {
  id: string | null;
  name: string;
  description: string;
  quantityMillis: number;
  unit: string;
  unitPriceCents: number;
  taxRateBps: number;
}

export interface QuotationCenterInput {
  lineItems: QuotationLineDraft[];
  projectDiscountCents: number;
  defaultTaxRateBps: number;
  taxMode: BusinessTaxMode;
}

export interface QuotationCenterProps {
  workspace: BusinessWorkspaceRecord;
  disabled?: boolean;
  onSave: (input: QuotationCenterInput) => void | Promise<void>;
  onAdvanceApproval: (documentId: string | null) => void | Promise<void>;
  onGenerate: (documentId: string) => void | Promise<void>;
  onOpenAsset: (assetId: string) => void | Promise<void>;
  onClose: () => void;
}

const STATUS_LABELS: Record<BusinessDocumentRecord["status"], string> = {
  draft: "草稿",
  inReview: "待人工确认",
  approved: "已人工确认",
  generated: "已生成",
  effective: "已生效",
  voided: "已作废",
};

export function currentQuotationDocument(workspace: BusinessWorkspaceRecord): BusinessDocumentRecord | null {
  const currentId = workspace.currentDocuments.quoteDocumentId;
  const current = currentId
    ? workspace.documents.find((document) => document.id === currentId)
    : null;
  if (current && current.status !== "voided") return current;
  return [...workspace.documents]
    .filter((document) => document.kind === "quote" && document.status !== "voided")
    .sort((left, right) => right.sequenceNumber - left.sequenceNumber || right.updatedAt - left.updatedAt)[0] ?? null;
}

export function quotationCenterInput(workspace: BusinessWorkspaceRecord): QuotationCenterInput {
  const defaultTaxRateBps = Number.isInteger(workspace.profile.defaultTaxRateBps)
    ? workspace.profile.defaultTaxRateBps
    : 0;
  const lineItems = workspace.profile.lineItems.length
    ? workspace.profile.lineItems.map((line) => ({
        id: line.id,
        name: line.name,
        description: line.description,
        quantityMillis: line.quantityMillis,
        unit: line.unit,
        unitPriceCents: line.unitPriceCents,
        taxRateBps: Number.isInteger(line.taxRateBps) ? line.taxRateBps : defaultTaxRateBps,
      }))
    : [{
        id: null,
        name: "服务项目",
        description: "",
        quantityMillis: 1_000,
        unit: "项",
        unitPriceCents: 0,
        taxRateBps: defaultTaxRateBps,
      }];
  return {
    lineItems,
    projectDiscountCents: Number.isInteger(workspace.profile.projectDiscountCents)
      ? workspace.profile.projectDiscountCents
      : 0,
    defaultTaxRateBps,
    taxMode: workspace.profile.taxMode === "taxExclusive" ? "taxExclusive" : "taxInclusive",
  };
}

export function validateQuotationCenterInput(input: QuotationCenterInput): string[] {
  const errors: string[] = [];
  if (!input.lineItems.length) errors.push("请至少保留一个报价行项");
  input.lineItems.forEach((line, index) => {
    const label = line.name.trim() || `第 ${index + 1} 行`;
    if (!line.name.trim()) errors.push(`第 ${index + 1} 行缺少服务名称`);
    if (!Number.isInteger(line.quantityMillis) || line.quantityMillis <= 0) errors.push(`“${label}”数量必须大于 0`);
    if (!Number.isInteger(line.unitPriceCents) || line.unitPriceCents < 0) errors.push(`“${label}”单价不能为负数`);
    if (!Number.isInteger(line.taxRateBps) || line.taxRateBps < 0 || line.taxRateBps > 10_000) errors.push(`“${label}”税率必须在 0% 到 100% 之间`);
  });
  if (!Number.isInteger(input.projectDiscountCents) || input.projectDiscountCents < 0) errors.push("项目优惠不能为负数");
  if (!Number.isInteger(input.defaultTaxRateBps) || input.defaultTaxRateBps < 0 || input.defaultTaxRateBps > 10_000) errors.push("默认税率必须在 0% 到 100% 之间");
  return errors;
}

function quantityValue(quantityMillis: number): string {
  return String(quantityMillis / 1_000);
}

function moneyValue(cents: number): string {
  return (cents / 100).toFixed(2);
}

function parseNumber(value: string): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function approvalLabel(document: BusinessDocumentRecord | null): string {
  if (!document || document.status === "generated" || document.status === "effective" || document.status === "voided") {
    return document ? "创建新版本并提交确认" : "提交人工确认";
  }
  if (document.status === "draft") return "提交人工确认";
  if (document.status === "inReview") return "确认报价";
  return "已人工确认";
}

export function QuotationCenter({ workspace, disabled = false, onSave, onAdvanceApproval, onGenerate, onOpenAsset, onClose }: QuotationCenterProps) {
  const [draft, setDraft] = useState<QuotationCenterInput>(() => quotationCenterInput(workspace));
  const [error, setError] = useState<string | null>(null);
  const document = currentQuotationDocument(workspace);

  useEffect(() => {
    setDraft(quotationCenterInput(workspace));
    setError(null);
  }, [workspace.id, workspace.revision]);

  const preview = useMemo(() => {
    const validationErrors = validateQuotationCenterInput(draft);
    if (validationErrors.length) return { errors: validationErrors, totals: null };
    try {
      const result = createQuotationDraft({
        id: document?.id ?? `quote-preview:${workspace.id}`,
        companyId: "business-workbench",
        customerProjectId: workspace.projectId,
        title: workspace.profile.projectTitle || "项目报价",
        lines: draft.lineItems.map((line, index) => ({
          id: line.id ?? `quote-line:${index + 1}`,
          description: line.name.trim(),
          quantity: line.quantityMillis / 1_000,
          unitPriceCents: line.unitPriceCents,
        })),
        projectDiscountCents: draft.projectDiscountCents,
        taxBasisPoints: draft.defaultTaxRateBps,
        taxMode: draft.taxMode,
        actorId: "quotation-center",
        sourceKind: "userInput",
        sourceId: workspace.id,
        sourceLabel: "报价中心人工编辑",
        createdAt: new Date(0).toISOString(),
      });
      return { errors: [] as string[], totals: result.totals };
    } catch (previewError) {
      return {
        errors: [previewError instanceof Error ? previewError.message : "报价金额计算失败"],
        totals: null,
      };
    }
  }, [document?.id, draft, workspace.id, workspace.profile.projectTitle, workspace.projectId]);

  const run = async (operation: () => void | Promise<void>) => {
    setError(null);
    try {
      await operation();
    } catch (operationError) {
      setError(operationError instanceof Error ? operationError.message : "报价操作失败，请重试。");
    }
  };

  const updateLine = (index: number, patch: Partial<QuotationLineDraft>) => {
    setDraft((current) => ({
      ...current,
      lineItems: current.lineItems.map((line, lineIndex) => lineIndex === index ? { ...line, ...patch } : line),
    }));
  };

  return (
    <div className="bw-quotation-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget && !disabled) onClose();
    }}>
      <section className="bw-quotation-center" role="dialog" aria-modal="true" aria-labelledby="bw-quotation-title">
        <header className="bw-quotation-header">
          <div>
            <span className="bw-quotation-eyebrow">独立报价任务</span>
            <h2 id="bw-quotation-title">报价中心</h2>
            <p>{workspace.profile.projectTitle || workspace.profile.projectCode || "当前客户项目"}</p>
          </div>
          <button type="button" className="bw-icon-button" aria-label="关闭报价中心" disabled={disabled} onClick={onClose}><X size={18} /></button>
        </header>

        <div className="bw-quotation-statusbar">
          <span>版本：{document ? `V${document.sequenceNumber}` : "尚未创建"}</span>
          <span>状态：{document ? STATUS_LABELS[document.status] : "编辑中"}</span>
          <span>输出：{document?.outputFormat?.toUpperCase() ?? "XLSX"}</span>
        </div>

        <div className="bw-quotation-grid">
          <div className="bw-quotation-editor">
            <div className="bw-quotation-section-title">
              <div><strong>报价行项</strong><span>数量和金额分别保存，不允许用改单价凑总价。</span></div>
              <button type="button" disabled={disabled} onClick={() => setDraft((current) => ({
                ...current,
                lineItems: [...current.lineItems, {
                  id: null,
                  name: "新增服务",
                  description: "",
                  quantityMillis: 1_000,
                  unit: "项",
                  unitPriceCents: 0,
                  taxRateBps: current.defaultTaxRateBps,
                }],
              }))}><Plus size={16} />新增行项</button>
            </div>

            <div className="bw-quotation-lines">
              {draft.lineItems.map((line, index) => (
                <article className="bw-quotation-line" key={line.id ?? `draft:${index}`}>
                  <div className="bw-quotation-line-head">
                    <strong>行项 {index + 1}</strong>
                    <button type="button" aria-label={`删除行项 ${index + 1}`} disabled={disabled || draft.lineItems.length === 1} onClick={() => setDraft((current) => ({
                      ...current,
                      lineItems: current.lineItems.filter((_, lineIndex) => lineIndex !== index),
                    }))}><Trash2 size={15} /></button>
                  </div>
                  <label>服务名称<input value={line.name} disabled={disabled} onChange={(event) => updateLine(index, { name: event.target.value })} /></label>
                  <label>说明<input value={line.description} disabled={disabled} onChange={(event) => updateLine(index, { description: event.target.value })} /></label>
                  <div className="bw-quotation-line-fields">
                    <label>数量<input type="number" min="0.001" step="0.001" value={quantityValue(line.quantityMillis)} disabled={disabled} onChange={(event) => updateLine(index, { quantityMillis: Math.round(parseNumber(event.target.value) * 1_000) })} /></label>
                    <label>单位<input value={line.unit} disabled={disabled} onChange={(event) => updateLine(index, { unit: event.target.value })} /></label>
                    <label>含税单价（元）<input type="number" min="0" step="0.01" value={moneyValue(line.unitPriceCents)} disabled={disabled} onChange={(event) => updateLine(index, { unitPriceCents: Math.round(parseNumber(event.target.value) * 100) })} /></label>
                    <label>税率（%）<input type="number" min="0" max="100" step="0.01" value={line.taxRateBps / 100} disabled={disabled} onChange={(event) => updateLine(index, { taxRateBps: Math.round(parseNumber(event.target.value) * 100) })} /></label>
                  </div>
                </article>
              ))}
            </div>

            <div className="bw-quotation-settings">
              <label>项目优惠（元）<input type="number" min="0" step="0.01" value={moneyValue(draft.projectDiscountCents)} disabled={disabled} onChange={(event) => setDraft((current) => ({ ...current, projectDiscountCents: Math.round(parseNumber(event.target.value) * 100) }))} /></label>
              <label>默认税率（%）<input type="number" min="0" max="100" step="0.01" value={draft.defaultTaxRateBps / 100} disabled={disabled} onChange={(event) => setDraft((current) => ({ ...current, defaultTaxRateBps: Math.round(parseNumber(event.target.value) * 100) }))} /></label>
              <label>计税方式<select value={draft.taxMode} disabled={disabled} onChange={(event) => setDraft((current) => ({ ...current, taxMode: event.target.value as BusinessTaxMode }))}><option value="taxInclusive">含税价</option><option value="taxExclusive">未税价</option></select></label>
            </div>
          </div>

          <aside className="bw-quotation-summary">
            <FileSpreadsheet size={24} />
            <h3>金额预览</h3>
            {preview.totals ? (
              <dl>
                <div><dt>原价合计</dt><dd>{formatCny(preview.totals.subtotal.cents)}</dd></div>
                <div><dt>项目优惠</dt><dd>-{formatCny(preview.totals.discountTotal.cents)}</dd></div>
                <div><dt>税额</dt><dd>{formatCny(preview.totals.taxAmount.cents)}</dd></div>
                <div className="is-total"><dt>最终报价</dt><dd>{formatCny(preview.totals.finalTotal.cents)}</dd></div>
              </dl>
            ) : null}
            {preview.errors.length ? <div className="bw-quotation-error" role="alert">{preview.errors.join("；")}</div> : null}
            {error ? <div className="bw-quotation-error" role="alert">{error}</div> : null}
            <p className="bw-quotation-note">正式金额由现有业务服务重新计算并冻结到文档快照；此处仅用于编辑校验。</p>
          </aside>
        </div>

        <footer className="bw-quotation-footer">
          <button type="button" disabled={disabled || preview.errors.length > 0} onClick={() => void run(() => onSave(draft))}>保存报价参数</button>
          <button type="button" className="is-primary" disabled={disabled || preview.errors.length > 0 || document?.status === "approved"} onClick={() => void run(() => onAdvanceApproval(document?.id ?? null))}>{approvalLabel(document)}</button>
          <button type="button" className="is-primary" disabled={disabled || document?.status !== "approved"} onClick={() => document && void run(() => onGenerate(document.id))}>生成 XLSX</button>
          {document?.outputAssetId ? <button type="button" onClick={() => void run(() => onOpenAsset(document.outputAssetId!))}>打开成果</button> : null}
        </footer>
      </section>
    </div>
  );
}
