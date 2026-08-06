import {
  AlertCircle,
  AlertTriangle,
  Check,
  ChevronRight,
  FileSearch,
  FileSpreadsheet,
  FileText,
  Files,
  History,
  LayoutTemplate,
  LoaderCircle,
  PanelRightClose,
  RotateCcw,
  Scale,
  ShieldCheck,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type {
  ApprovalDecision,
  BusinessWorkspaceActions,
  ContextTab,
  PreviewDocument,
  WorkspaceContext,
} from "./types";

interface ContextDrawerProps {
  context: WorkspaceContext;
  activeTab: ContextTab;
  actions: BusinessWorkspaceActions;
  onTabChange: (tab: ContextTab) => void;
  onClose: () => void;
}

const tabs: Array<{ id: ContextTab; label: string; icon: LucideIcon }> = [
  { id: "issues", label: "问题", icon: AlertCircle },
  { id: "template", label: "模板", icon: LayoutTemplate },
  { id: "preview", label: "预览", icon: FileSearch },
  { id: "approval", label: "审批", icon: ShieldCheck },
  { id: "versions", label: "版本", icon: History },
];

export function ContextDrawer({ context, activeTab, actions, onTabChange, onClose }: ContextDrawerProps) {
  const issueCount = context.missingMaterials.length + context.conflicts.length + context.legalRisks.length;
  const pendingApprovalCount = context.approvals.filter((item) => item.status === "pending").length;

  const counts: Partial<Record<ContextTab, number>> = {
    issues: issueCount,
    template: context.templates.length,
    preview: context.previews.length,
    approval: pendingApprovalCount,
    versions: context.versions.length,
  };

  return (
    <aside className="bw-context" aria-label="任务上下文">
      <header className="bw-context__header">
        <div>
          <span>任务上下文</span>
          <strong>{contextHeading(activeTab)}</strong>
        </div>
        <button className="bw-icon-button" type="button" onClick={onClose} title="关闭上下文">
          <PanelRightClose size={17} />
        </button>
      </header>

      <nav className="bw-context-tabs" aria-label="上下文视图">
        {tabs.map(({ id, label, icon: Icon }) => (
          <button
            className={activeTab === id ? "is-active" : ""}
            type="button"
            key={id}
            onClick={() => onTabChange(id)}
            title={label}
            aria-label={label}
          >
            <Icon size={16} />
            {counts[id] ? <span>{counts[id]}</span> : null}
          </button>
        ))}
      </nav>

      <div className="bw-context__body">
        {activeTab === "issues" ? <IssuesPanel context={context} actions={actions} /> : null}
        {activeTab === "template" ? <TemplatesPanel context={context} actions={actions} /> : null}
        {activeTab === "preview" ? <PreviewPanel context={context} actions={actions} /> : null}
        {activeTab === "approval" ? <ApprovalPanel context={context} actions={actions} /> : null}
        {activeTab === "versions" ? <VersionsPanel context={context} actions={actions} /> : null}
      </div>
    </aside>
  );
}

function IssuesPanel({ context, actions }: { context: WorkspaceContext; actions: BusinessWorkspaceActions }) {
  const isEmpty = !context.missingMaterials.length && !context.conflicts.length && !context.legalRisks.length;
  if (isEmpty) return <ContextEmpty icon={Check} title="未发现阻塞问题" detail="当前资料和字段校验均已通过。" />;

  return (
    <>
      {context.missingMaterials.length ? (
        <ContextSection title="缺失资料" count={context.missingMaterials.length}>
          {context.missingMaterials.map((material) => (
            <button className={`bw-context-item is-${material.severity}`} type="button" key={material.id} onClick={() => actions.onResolveMissingMaterial(material.id)}>
              <span className="bw-context-item__icon"><AlertTriangle size={15} /></span>
              <span className="bw-context-item__copy">
                <strong>{material.title}</strong>
                <small>{material.detail}</small>
              </span>
              <ChevronRight size={14} />
            </button>
          ))}
        </ContextSection>
      ) : null}

      {context.conflicts.length ? (
        <ContextSection title="字段冲突" count={context.conflicts.length}>
          {context.conflicts.map((conflict) => (
            <button className="bw-conflict-item" type="button" key={conflict.id} onClick={() => actions.onResolveConflict(conflict.id)}>
              <span className="bw-conflict-item__header">
                <strong>{conflict.fieldLabel}</strong>
                <ChevronRight size={14} />
              </span>
              <span><b>{conflict.primaryValue}</b><small>{conflict.primarySource}</small></span>
              <span><b>{conflict.secondaryValue}</b><small>{conflict.secondarySource}</small></span>
            </button>
          ))}
        </ContextSection>
      ) : null}

      {context.legalRisks.length ? (
        <ContextSection title="法务风险" count={context.legalRisks.length}>
          {context.legalRisks.map((risk) => (
            <button className={`bw-context-item is-risk-${risk.level}`} type="button" key={risk.id} onClick={() => actions.onReviewLegalRisk(risk.id)}>
              <span className="bw-context-item__icon"><Scale size={15} /></span>
              <span className="bw-context-item__copy">
                <strong>{risk.title}</strong>
                <small>{risk.detail}</small>
                {risk.sourceLabel ? <em>{risk.sourceLabel}</em> : null}
              </span>
              <ChevronRight size={14} />
            </button>
          ))}
        </ContextSection>
      ) : null}
    </>
  );
}

function TemplatesPanel({ context, actions }: { context: WorkspaceContext; actions: BusinessWorkspaceActions }) {
  if (!context.templates.length) return <ContextEmpty icon={LayoutTemplate} title="尚未匹配模板" detail="添加客户模板或历史成功件后，可在这里确认匹配结果。" />;

  return (
    <ContextSection title="匹配顺序" hint="项目最近成功模板优先">
      {context.templates.map((template) => (
        <button
          className={`bw-template-item ${template.isSelected ? "is-selected" : ""}`}
          type="button"
          key={template.id}
          onClick={() => actions.onSelectTemplate(template.id)}
        >
          <span className="bw-template-item__icon"><LayoutTemplate size={17} /></span>
          <span>
            <strong>{template.name}</strong>
            <small>{template.versionLabel} · {template.sourceLabel}</small>
          </span>
          <span className="bw-template-score">{Math.round(template.confidence * 100)}%</span>
          {template.isSelected ? <Check size={15} /> : null}
        </button>
      ))}
    </ContextSection>
  );
}

function PreviewPanel({ context, actions }: { context: WorkspaceContext; actions: BusinessWorkspaceActions }) {
  if (!context.acceptanceBatches.length && !context.previews.length) {
    return <ContextEmpty icon={FileSearch} title="暂无文档预览" detail="生成 DOCX、XLSX 或 PDF 后可在这里逐份检查。" />;
  }

  return (
    <>
      {context.acceptanceBatches.length ? (
        <ContextSection title="验收批次" count={context.acceptanceBatches.length}>
          <div className="bw-acceptance-batch-list">
            {context.acceptanceBatches.map((batch) => {
              const allPrepared = batch.totalCount > 0 && batch.preparedCount === batch.totalCount;
              return (
                <article className="bw-acceptance-batch" key={batch.id}>
                  <header>
                    <strong>{batch.label}</strong>
                    <span>已准备 {batch.preparedCount}/{batch.totalCount}</span>
                  </header>
                  <div className="bw-acceptance-batch__status">
                    <span className={batch.isReady ? "is-ready" : "is-blocked"}>
                      {batch.isReady ? "材料已齐" : batch.blockerText ?? "材料未齐"}
                    </span>
                    <small>{batch.isReady ? "材料已齐，可以进入批准和生成。" : "可先准备草稿；材料补齐前不能批准或生成。"}</small>
                  </div>
                  <button
                    className="bw-secondary-button bw-acceptance-batch__prepare"
                    type="button"
                    onClick={() => actions.onPrepareAcceptanceDocuments(batch.id)}
                    disabled={Boolean(batch.prepareDisabledReason)}
                    title={batch.prepareDisabledReason}
                  >
                    {batch.isPreparing ? <LoaderCircle className="bw-spin" size={15} /> : <Files size={15} />}
                    {batch.isPreparing ? "正在准备…" : allPrepared ? "验收文件已准备" : "准备验收文件"}
                  </button>
                </article>
              );
            })}
          </div>
        </ContextSection>
      ) : null}
      {context.previews.length ? (
        <ContextSection title="生成文档" count={context.previews.length}>
          {context.previews.map((preview) => {
            const Icon = previewIcon(preview);
            return (
              <button className={`bw-preview-item is-${preview.status}`} type="button" key={preview.id} onClick={() => actions.onOpenPreview(preview.id)} disabled={preview.status !== "ready"}>
                <span className="bw-preview-item__thumb"><Icon size={22} /></span>
                <span>
                  <strong>{preview.name}</strong>
                  <small>{preview.pageLabel ?? preview.format.toUpperCase()}</small>
                </span>
                {preview.status === "generating" ? <LoaderCircle className="bw-spin" size={15} /> : null}
                {preview.status === "blocked" ? <AlertTriangle size={15} /> : <ChevronRight size={14} />}
              </button>
            );
          })}
        </ContextSection>
      ) : null}
    </>
  );
}

function ApprovalPanel({ context, actions }: { context: WorkspaceContext; actions: BusinessWorkspaceActions }) {
  if (!context.approvals.length) return <ContextEmpty icon={ShieldCheck} title="暂无审批事项" detail="正式导出、共享和敏感字段使用会出现在这里。" />;

  return (
    <ContextSection title="人工把关" count={context.approvals.filter((item) => item.status === "pending").length}>
      {context.approvals.map((approval) => (
        <article className={`bw-approval-item is-${approval.status} ${approval.blocked ? "is-blocked" : ""}`} key={approval.id}>
          <header>
            <span><ShieldCheck size={16} /></span>
            <div><strong>{approval.title}</strong><small>{approval.requestedBy} · {approval.requestedAt}</small></div>
          </header>
          <p>{approval.detail}</p>
          {approval.blocked ? (
            <div className="bw-approval-blocker" role="status">
              <AlertTriangle size={14} />
              <span>{approval.blocked}</span>
            </div>
          ) : null}
          {approval.status === "pending" ? (
            <footer>
              <button className="bw-secondary-button" type="button" onClick={() => decision(actions, approval.id, "reject")}><X size={14} />退回</button>
              <button className="bw-primary-button" type="button" onClick={() => decision(actions, approval.id, "approve")} disabled={Boolean(approval.blocked)} title={approval.blocked}><Check size={14} />确认</button>
            </footer>
          ) : <span className="bw-approval-state">{approval.status === "approved" ? "已确认" : "已退回"}</span>}
        </article>
      ))}
    </ContextSection>
  );
}

function VersionsPanel({ context, actions }: { context: WorkspaceContext; actions: BusinessWorkspaceActions }) {
  if (!context.versions.length) return <ContextEmpty icon={History} title="暂无历史版本" detail="每次生成和人工确认都会保留独立版本。" />;

  return (
    <ContextSection title="版本记录" count={context.versions.length}>
      <div className="bw-version-list">
        {context.versions.map((version) => (
          <article className={`bw-version-item ${version.isCurrent ? "is-current" : ""}`} key={version.id}>
            <span className="bw-version-item__dot" />
            <header><strong>{version.label}</strong>{version.isCurrent ? <span>当前</span> : null}</header>
            <p>{version.note}</p>
            <small>{version.authorName} · {version.createdAt}</small>
            {!version.isCurrent ? (
              <button type="button" onClick={() => actions.onRestoreVersion(version.id)}><RotateCcw size={13} />恢复此版本</button>
            ) : null}
          </article>
        ))}
      </div>
    </ContextSection>
  );
}

function decision(actions: BusinessWorkspaceActions, approvalId: string, value: ApprovalDecision) {
  actions.onApprovalDecision(approvalId, value);
}

interface ContextSectionProps {
  title: string;
  count?: number;
  hint?: string;
  children: React.ReactNode;
}

function ContextSection({ title, count, hint, children }: ContextSectionProps) {
  return (
    <section className="bw-context-section">
      <header><strong>{title}</strong>{typeof count === "number" ? <span>{count}</span> : null}{hint ? <small>{hint}</small> : null}</header>
      <div className="bw-context-section__content">{children}</div>
    </section>
  );
}

interface ContextEmptyProps {
  icon: LucideIcon;
  title: string;
  detail: string;
}

function ContextEmpty({ icon: Icon, title, detail }: ContextEmptyProps) {
  return (
    <div className="bw-context-empty">
      <span><Icon size={20} /></span>
      <strong>{title}</strong>
      <p>{detail}</p>
    </div>
  );
}

function previewIcon(preview: PreviewDocument): LucideIcon {
  return preview.format === "xlsx" ? FileSpreadsheet : FileText;
}

function contextHeading(tab: ContextTab) {
  return tabs.find((item) => item.id === tab)?.label ?? "上下文";
}
