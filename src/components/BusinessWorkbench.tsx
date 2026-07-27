import { useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import {
  Archive,
  Check,
  ChartNoAxesCombined,
  Ellipsis,
  ChevronRight,
  ClipboardList,
  FilePenLine,
  FileSearch2,
  FolderKanban,
  GalleryHorizontalEnd,
  Images,
  LayoutDashboard,
  ListTodo,
  LoaderCircle,
  LogOut,
  Menu,
  MessageSquareText,
  MonitorCog,
  PanelLeftClose,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
  Pencil,
  Plus,
  ReceiptText,
  RefreshCw,
  Search,
  Settings2,
  Trash2,
  X,
} from "lucide-react";
import type { BusinessCustomerReceivableSummary } from "../generated/bsaigc/BusinessCustomerReceivableSummary";
import type { BusinessWorkspaceDomainEvent } from "../generated/bsaigc/BusinessWorkspaceDomainEvent";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import type { ProjectStage } from "../generated/bsaigc/ProjectStage";
import type { TaskRecord } from "../generated/bsaigc/TaskRecord";
import { AssetVault } from "./AssetVault";
import {
  BrainCenter,
  type BrainAttachment,
  type BrainWorkspace,
} from "./BrainCenter";
import type { BrainAccessMode } from "../generated/bsaigc/BrainAccessMode";
import { CaseLibrary } from "./CaseLibrary";
import { ExecutionBriefCenter } from "./ExecutionBriefCenter";
import { friendlyBrainThreadTitle } from "./brainPresentation";
import {
  ContractReviewCenter,
  type ContractReviewCenterProps,
} from "./ContractReviewCenter";
import type {
  DesktopSection,
  DesktopShellProps,
} from "./DesktopShell";
import type { ReviewFindingRecord } from "../generated/bsaigc/ReviewFindingRecord";
import { BusinessAgentDock } from "./BusinessAgentDock";
import { BusinessDocumentsCenter, type BusinessDocumentsCenterActions, type QuoteHistorySource } from "./BusinessDocumentsCenter";
import { BusinessReceivablesDrawer } from "./BusinessReceivablesDrawer";
import { RequirementBriefCenter } from "./RequirementBriefCenter";
import { TaskCenter } from "./TaskCenter";
import "./BusinessWorkbench.css";

export type BusinessWorkbenchProps = DesktopShellProps &
  ContractReviewCenterProps &
  BusinessDocumentsCenterActions & {
    businessWorkspace: BusinessWorkspaceRecord | null;
    quoteHistorySources: readonly QuoteHistorySource[];
    contractAgentFindings: readonly ReviewFindingRecord[];
    onArchiveBrainThread: (threadId: string, archived: boolean) => void;
    onRenameBrainThread: (threadId: string, title: string) => void;
    onDeleteBrainThread: (threadId: string) => void;
    onBrainAttach?: () => void;
    brainAttachments?: readonly BrainAttachment[];
    brainWorkspace?: BrainWorkspace | null;
    brainAccessMode?: BrainAccessMode;
    isBrainAttaching?: boolean;
    onRemoveBrainAttachment?: (assetId: string) => void;
    onSelectBrainWorkspace?: () => void;
    onClearBrainWorkspace?: () => void;
    onBrainAccessModeChange?: (mode: BrainAccessMode) => void;
    onBrainDropPaths?: (paths: string[]) => void;
    onBrainPasteImages?: (files: File[]) => void;
    businessWorkspaceEvents: readonly BusinessWorkspaceDomainEvent[];
    businessBusyAction: string | null;
    businessCustomers: readonly BusinessCustomerReceivableSummary[];
    businessCustomersLoading: boolean;
    businessCustomersError: string | null;
    businessCustomerQuery: string;
    onOpenSettings: () => void;
    onLogout: () => void;
    onBusinessCustomerQueryChange: (query: string) => void;
    onRefreshBusinessCustomers: () => void;
    onSelectBusinessCustomer: (
      customer: BusinessCustomerReceivableSummary,
    ) => void | boolean | Promise<void | boolean>;
    onDismissBusinessError?: () => void;
  };
export type { DesktopSection } from "./DesktopShell";

type BusinessSection = Extract<
  DesktopSection,
  | "workspace"
  | "brain"
  | "requirements"
  | "projects"
  | "creative"
  | "execution"
  | "tasks"
  | "assets"
  | "system"
>;

type StatusTone = "ready" | "warning" | "danger" | "muted";

const BUSINESS_SECTIONS = new Set<DesktopSection>([
  "workspace",
  "brain",
  "requirements",
  "projects",
  "creative",
  "execution",
  "tasks",
  "assets",
  "system",
]);

const NAV_GROUPS: ReadonlyArray<{
  label: string;
  items: ReadonlyArray<{
    id: BusinessSection;
    label: string;
    description: string;
    icon: LucideIcon;
  }>;
}> = [
  {
    label: "业务主线",
    items: [
      {
        id: "workspace",
        label: "工作台",
        description: "商务助手与项目总览",
        icon: LayoutDashboard,
      },
      {
        id: "requirements",
        label: "需求访谈",
        description: "访谈、追问与确认",
        icon: ClipboardList,
      },
      {
        id: "projects",
        label: "报价 / 单据",
        description: "报价、合同、请款与验收",
        icon: ReceiptText,
      },
      {
        id: "brain",
        label: "合同审查",
        description: "风险、证据与人工决策",
        icon: FileSearch2,
      },
    ],
  },
  {
    label: "交付执行",
    items: [
      {
        id: "execution",
        label: "执行单",
        description: "拍前执行与确认",
        icon: FilePenLine,
      },
      {
        id: "tasks",
        label: "任务中心",
        description: "后台任务与审批",
        icon: ListTodo,
      },
      {
        id: "creative",
        label: "案例库",
        description: "参考案例与素材沉淀",
        icon: Images,
      },
    ],
  },
  {
    label: "资料状态",
    items: [
      {
        id: "assets",
        label: "商务归档",
        description: "本地文件与版本",
        icon: Archive,
      },
      {
        id: "system",
        label: "系统状态",
        description: "运行环境与事件",
        icon: MonitorCog,
      },
    ],
  },
];

const NAV_ITEMS = NAV_GROUPS.flatMap((group) => group.items);

const STAGES: ReadonlyArray<{
  id: ProjectStage;
  label: string;
  shortLabel: string;
}> = [
  { id: "intake", label: "客户接洽", shortLabel: "接洽" },
  { id: "briefing", label: "需求确认", shortLabel: "需求" },
  { id: "creative", label: "报价确认", shortLabel: "报价" },
  { id: "production", label: "合同签署", shortLabel: "合同" },
  { id: "postProduction", label: "请款跟进", shortLabel: "请款" },
  { id: "review", label: "验收确认", shortLabel: "验收" },
  { id: "delivery", label: "回款交付", shortLabel: "回款" },
  { id: "closed", label: "项目归档", shortLabel: "归档" },
];

const SECTION_TITLES: Record<BusinessSection, string> = {
  workspace: "商务工作台",
  brain: "合同审查",
  requirements: "需求中心",
  projects: "报价与单据",
  creative: "案例库",
  execution: "拍摄执行单",
  tasks: "任务中心",
  assets: "商务归档",
  system: "系统状态",
};

export function BusinessWorkbench(props: BusinessWorkbenchProps) {
  const [navigationOpen, setNavigationOpen] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [railCompact, setRailCompact] = useState(false);
  const [contextOpen, setContextOpen] = useState(false);
  const [createProjectOpen, setCreateProjectOpen] = useState(false);
  const [receivablesOpen, setReceivablesOpen] = useState(false);

  const activeSection: BusinessSection = BUSINESS_SECTIONS.has(props.activeSection)
    ? (props.activeSection as BusinessSection)
    : "workspace";

  useEffect(() => {
    if (!BUSINESS_SECTIONS.has(props.activeSection)) {
      props.onNavigate("workspace");
    }
  }, [props.activeSection, props.onNavigate]);

  useEffect(() => {
    setContextOpen(false);
    setCreateProjectOpen(false);
    setReceivablesOpen(false);
  }, [activeSection]);

  const selectedProject = useMemo(
    () =>
      props.projects.find((project) => project.id === props.selectedProjectId) ??
      null,
    [props.projects, props.selectedProjectId],
  );

  const filteredProjects = useMemo(() => {
    const query = props.projectQuery.trim().toLocaleLowerCase("zh-CN");
    return [...props.projects]
      .filter((project) => {
        if (!query) return true;
        return [project.name, project.clientName]
          .join(" ")
          .toLocaleLowerCase("zh-CN")
          .includes(query);
      })
      .sort((left, right) => right.updatedAt - left.updatedAt);
  }, [props.projectQuery, props.projects]);

  const projectNames = useMemo(
    () =>
      Object.fromEntries(
        props.projects.map((project) => [project.id, project.name]),
      ),
    [props.projects],
  );

  const selectedBrainThread = useMemo(
    () =>
      props.brainThreads.find(
        (thread) => thread.id === props.selectedBrainThreadId,
      ) ?? null,
    [props.brainThreads, props.selectedBrainThreadId],
  );

  const [threadMenuId, setThreadMenuId] = useState<string | null>(null);
  const [threadDeleteConfirmId, setThreadDeleteConfirmId] = useState<string | null>(null);
  const [threadRenameId, setThreadRenameId] = useState<string | null>(null);
  const [threadRenameDraft, setThreadRenameDraft] = useState("");

  useEffect(() => {
    if (!threadMenuId) return;
    const close = () => {
      setThreadMenuId(null);
      setThreadDeleteConfirmId(null);
    };
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [threadMenuId]);

  const submitThreadRename = (event: FormEvent<HTMLFormElement>, threadId: string) => {
    event.preventDefault();
    const title = threadRenameDraft.trim();
    if (!title) return;
    props.onRenameBrainThread(threadId, title);
    setThreadRenameId(null);
    setThreadRenameDraft("");
  };

  const recentBrainThreads = useMemo(
    () =>
      [...props.brainThreads]
        .filter((thread) => thread.status !== "archived")
        .sort((left, right) => right.updatedAt - left.updatedAt),
    [props.brainThreads],
  );

  const assetProjects = useMemo(
    () =>
      props.projects.map((project) => ({
        id: project.id,
        name: project.name,
        clientName: project.clientName,
      })),
    [props.projects],
  );

  const selectedTasks = useMemo(
    () =>
      props.tasks
        .filter(
          (task) =>
            !selectedProject || task.projectId === selectedProject.id,
        )
        .sort((left, right) => right.updatedAt - left.updatedAt),
    [props.tasks, selectedProject],
  );

  const selectedRequirement = useMemo(
    () =>
      selectedProject
        ? props.requirementBriefs.find(
            (brief) => brief.projectId === selectedProject.id,
          ) ?? null
        : null,
    [props.requirementBriefs, selectedProject],
  );

  const latestConfirmedRequirement = useMemo(
    () =>
      selectedProject
        ? [...props.requirementBriefs]
            .filter(
              (brief) =>
                brief.projectId === selectedProject.id && brief.status === "confirmed",
            )
            .sort(
              (left, right) =>
                right.revision - left.revision || right.updatedAt - left.updatedAt,
            )[0] ?? null
        : null,
    [props.requirementBriefs, selectedProject],
  );

  const selectedAssets = useMemo(
    () =>
      props.assets.filter(
        (asset) => !selectedProject || asset.projectId === selectedProject.id,
      ),
    [props.assets, selectedProject],
  );

  const displayError = normalizeError(props.error);
  const brainDegraded = Boolean(
    props.brainHealth &&
      props.brainHealth.state !== "ready" &&
      props.brainHealth.state !== "stopped",
  );

  const navigate = (section: BusinessSection) => {
    props.onNavigate(section);
    setNavigationOpen(false);
  };

  const selectProject = (projectId: string) => {
    props.onSelectProject(projectId);
    setCreateProjectOpen(false);
    setNavigationOpen(false);
  };

  const handleCreateProject = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (
      !props.createProjectDraft.name.trim() ||
      !props.createProjectDraft.clientName.trim()
    ) {
      return;
    }
    props.onCreateProject({
      name: props.createProjectDraft.name.trim(),
      clientName: props.createProjectDraft.clientName.trim(),
    });
    setCreateProjectOpen(false);
  };

  const brainCenter = (
    <BrainCenter
      threads={props.brainThreads}
      turns={props.brainTurns}
      selectedThreadId={props.selectedBrainThreadId}
      projectNames={projectNames}
      models={props.brainModels}
      selectedModel={props.selectedBrainModel}
      draft={props.brainDraft}
      attachments={props.brainAttachments}
      workspace={props.brainWorkspace}
      accessMode={props.brainAccessMode}
      streamingDelta={props.brainStreamingDelta}
      isLoadingThreads={props.isLoading || props.isLoadingBrainThreads}
      isLoadingTurns={props.isLoadingBrainTurns}
      isStartingThread={props.isStartingBrainThread}
      isSending={props.isSendingBrainTurn}
      isAttaching={props.isBrainAttaching}
      isDegraded={brainDegraded}
      degradedReason={
        brainDegraded ? "智能助手暂时无法使用，历史记录仍可查看。" : null
      }
      error={displayError}
      showThreadList={false}
      onSelectThread={props.onSelectBrainThread}
      onAttach={props.onBrainAttach}
      onRemoveAttachment={props.onRemoveBrainAttachment}
      onSelectWorkspace={props.onSelectBrainWorkspace}
      onClearWorkspace={props.onClearBrainWorkspace}
      onAccessModeChange={props.onBrainAccessModeChange}
      onDropPaths={props.onBrainDropPaths}
      onPasteImages={props.onBrainPasteImages}
      onModelChange={props.onBrainModelChange}
      onDraftChange={props.onBrainDraftChange}
      onSend={props.onSendBrainTurn}
      onInterrupt={props.onInterruptBrainTurn}
      onNewThread={props.onNewBrainThread}
      onReload={props.onRefreshBrain}
    />
  );

  return (
    <div
      className={`business-workbench ${contextOpen ? "is-context-open" : "is-context-closed"} ${sidebarCollapsed ? "is-sidebar-collapsed" : "is-sidebar-open"} ${railCompact ? "is-rail-compact" : ""}`}
    >
      <header className="business-workbench__topbar">
        <button
          type="button"
          className="business-workbench__mobile-menu"
          onClick={() => setNavigationOpen((current) => !current)}
          aria-label={navigationOpen ? "关闭商务导航" : "打开商务导航"}
          aria-expanded={navigationOpen}
        >
          {navigationOpen ? <X size={17} /> : <Menu size={17} />}
        </button>

        <div className="business-workbench__topbar-heading">
          <strong className="business-workbench__topbar-title">
            {activeSection === "workspace"
              ? selectedBrainThread
                ? friendlyBrainThreadTitle(selectedBrainThread.title)
                : "商务工作台"
              : SECTION_TITLES[activeSection]}
          </strong>
          <small className="business-workbench__topbar-subtitle">
            {NAV_ITEMS.find((item) => item.id === activeSection)?.description ??
              ""}
          </small>
        </div>

        <div className="business-workbench__topbar-actions">
          <button
            type="button"
            className={`business-workbench__topbar-button is-overview ${receivablesOpen ? "is-active" : ""}`}
            onClick={() => {
              setReceivablesOpen((current) => !current);
              setContextOpen(false);
            }}
            aria-label={receivablesOpen ? "关闭经营概览" : "打开经营概览"}
            aria-expanded={receivablesOpen}
          >
            <ChartNoAxesCombined size={15} />
            <span>经营概览</span>
          </button>
          <button
            type="button"
            className="business-workbench__topbar-button is-settings"
            onClick={props.onOpenSettings}
            aria-label="打开设置"
            title="设置"
          >
            <Settings2 size={16} />
          </button>
          <button
            type="button"
            className="business-workbench__topbar-button is-logout"
            onClick={props.onLogout}
            aria-label="退出账号"
            title="退出账号"
          >
            <LogOut size={16} />
          </button>
          {selectedProject && activeSection !== "workspace" && (
            <button
              type="button"
              className="business-workbench__topbar-button"
              onClick={() => setContextOpen((current) => !current)}
              aria-label={contextOpen ? "收起项目详情" : "展开项目详情"}
              aria-expanded={contextOpen}
              title="项目详情"
            >
              {contextOpen ? (
                <PanelRightClose size={16} />
              ) : (
                <PanelRightOpen size={16} />
              )}
            </button>
          )}
        </div>
      </header>

      {displayError && (
        <div className="business-workbench__error-banner" role="alert">
          <span className="business-workbench__error-text">{displayError}</span>
          <span className="business-workbench__error-actions">
            {props.onRetry && (
              <button type="button" onClick={props.onRetry}>
                重试连接
              </button>
            )}
            {(props.onDismissError ?? props.onDismissBusinessError) && (
              <button
                type="button"
                onClick={props.onDismissError ?? props.onDismissBusinessError}
                aria-label="关闭错误提示"
              >
                <X size={13} />
              </button>
            )}
          </span>
        </div>
      )}

      <div className="business-workbench__body">
        {navigationOpen && (
          <button
            type="button"
            className="business-workbench__backdrop"
            aria-label="关闭商务导航"
            onClick={() => setNavigationOpen(false)}
          />
        )}

        <aside className="business-workbench__activity-rail" aria-label="商务主导航">
          <nav className="business-workbench__activity-nav" aria-label="商务功能">
            {NAV_GROUPS.map((group) => (
              <div className="business-workbench__nav-group" key={group.label}>
                <span
                  className="business-workbench__nav-group-label"
                  aria-hidden="true"
                >
                  {group.label}
                </span>
                {group.items.map((item) => {
                  const Icon = item.icon;
                  const active = activeSection === item.id;
                  return (
                    <button
                      type="button"
                      key={item.id}
                      className={active ? "is-active" : ""}
                      onClick={() => navigate(item.id)}
                      aria-label={item.label}
                      aria-current={active ? "page" : undefined}
                      title={railCompact ? item.label : item.description}
                    >
                      <Icon size={17} strokeWidth={1.8} />
                      <span className="business-workbench__nav-label">
                        {item.label}
                      </span>
                    </button>
                  );
                })}
              </div>
            ))}
          </nav>

          <div className="business-workbench__activity-footer">
            <button
              type="button"
              onClick={() => setRailCompact((current) => !current)}
              aria-label={railCompact ? "展开导航" : "精简导航"}
              aria-expanded={!railCompact}
              title={railCompact ? "展开导航" : "精简导航"}
            >
              {railCompact ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
              <span className="business-workbench__nav-label">精简导航</span>
            </button>
            <button
              type="button"
              onClick={() => setSidebarCollapsed((current) => !current)}
              aria-label={sidebarCollapsed ? "展开列表栏" : "隐藏列表栏"}
              aria-expanded={!sidebarCollapsed}
              title={sidebarCollapsed ? "展开列表栏" : "隐藏列表栏"}
            >
              {sidebarCollapsed ? <GalleryHorizontalEnd size={16} /> : <GalleryHorizontalEnd size={16} />}
              <span className="business-workbench__nav-label">
                {sidebarCollapsed ? "展开列表" : "隐藏列表"}
              </span>
            </button>
          </div>
        </aside>
        <aside
          className={`business-workbench__navigation ${navigationOpen ? "is-open" : ""}`}
          aria-label="商务工作台导航"
        >
          <div className="business-workbench__nav-heading">
            <strong>{activeSection === "workspace" ? "对话" : "项目"}</strong>
            {activeSection === "workspace" ? (
              <>
                <button
                  type="button"
                  className="business-workbench__nav-add"
                  onClick={props.onRefreshBrain}
                  disabled={props.isLoadingBrainThreads}
                  aria-label="刷新对话列表"
                  title="刷新对话列表"
                >
                  {props.isLoadingBrainThreads ? (
                    <LoaderCircle size={14} className="business-workbench__spin" />
                  ) : (
                    <RefreshCw size={14} />
                  )}
                </button>
                <button
                  type="button"
                  className="business-workbench__nav-add"
                  onClick={props.onNewBrainThread}
                  disabled={props.isStartingBrainThread || brainDegraded}
                  aria-label="新建对话"
                  title="新建对话"
                >
                  {props.isStartingBrainThread ? (
                    <LoaderCircle size={14} className="business-workbench__spin" />
                  ) : (
                    <Plus size={14} />
                  )}
                </button>
              </>
            ) : (
              <button
                type="button"
                className="business-workbench__nav-add"
                onClick={() => setCreateProjectOpen((current) => !current)}
                aria-label={createProjectOpen ? "关闭新建项目" : "新建项目"}
                aria-expanded={createProjectOpen}
                title="新建项目"
              >
                {createProjectOpen ? <X size={14} /> : <Plus size={14} />}
              </button>
            )}
          </div>

          {activeSection === "workspace" ? (
            <section className="business-workbench__thread-rail is-primary" aria-label="对话">
              <div className="business-workbench__thread-list">
                {recentBrainThreads.length > 0 ? (
                  recentBrainThreads.map((thread) => {
                    const active = thread.id === props.selectedBrainThreadId;
                    const menuOpen = threadMenuId === thread.id;
                    const renaming = threadRenameId === thread.id;
                    const title = friendlyBrainThreadTitle(thread.title);
                    return (
                      <div
                        key={thread.id}
                        className={`business-workbench__thread-item ${active ? "is-active" : ""} ${menuOpen ? "is-menu-open" : ""}`}
                      >
                        {renaming ? (
                          <form
                            className="business-workbench__thread-rename"
                            onSubmit={(event) => submitThreadRename(event, thread.id)}
                          >
                            <input
                              value={threadRenameDraft}
                              onChange={(event) => setThreadRenameDraft(event.currentTarget.value)}
                              onKeyDown={(event) => {
                                if (event.key === "Escape") {
                                  setThreadRenameId(null);
                                  setThreadRenameDraft("");
                                }
                              }}
                              maxLength={240}
                              aria-label="对话名称"
                              autoFocus
                            />
                            <button
                              type="submit"
                              disabled={!threadRenameDraft.trim()}
                              aria-label="保存名称"
                              title="保存"
                            >
                              <Check size={13} />
                            </button>
                            <button
                              type="button"
                              onClick={() => {
                                setThreadRenameId(null);
                                setThreadRenameDraft("");
                              }}
                              aria-label="取消重命名"
                              title="取消"
                            >
                              <X size={13} />
                            </button>
                          </form>
                        ) : (
                          <>
                            <button
                              type="button"
                              className="business-workbench__thread-open"
                              onClick={() => props.onSelectBrainThread(thread.id)}
                              aria-pressed={active}
                              title={title}
                            >
                              <MessageSquareText size={14} strokeWidth={1.7} aria-hidden="true" />
                              <span>{title}</span>
                              <span
                                className={`business-workbench__thread-status is-${thread.status}`}
                                aria-hidden="true"
                              />
                            </button>
                            <button
                              type="button"
                              className="business-workbench__thread-menu-trigger"
                              aria-label={`对话「${title}」操作`}
                              aria-expanded={menuOpen}
                              onClick={(event) => {
                                event.stopPropagation();
                                setThreadMenuId(menuOpen ? null : thread.id);
                                setThreadDeleteConfirmId(null);
                              }}
                            >
                              <Ellipsis size={14} />
                            </button>
                            {menuOpen && (
                              <div
                                className="business-workbench__thread-menu"
                                role="menu"
                                onClick={(event) => event.stopPropagation()}
                              >
                                <button
                                  type="button"
                                  role="menuitem"
                                  onClick={() => {
                                    setThreadRenameId(thread.id);
                                    setThreadRenameDraft(title);
                                    setThreadMenuId(null);
                                    setThreadDeleteConfirmId(null);
                                  }}
                                >
                                  <Pencil size={13} />
                                  重命名
                                </button>
                                <button
                                  type="button"
                                  role="menuitem"
                                  onClick={() => {
                                    props.onArchiveBrainThread(thread.id, true);
                                    setThreadMenuId(null);
                                  }}
                                >
                                  <Archive size={13} />
                                  归档对话
                                </button>
                                <button
                                  type="button"
                                  role="menuitem"
                                  className="is-danger"
                                  onClick={() => {
                                    if (threadDeleteConfirmId === thread.id) {
                                      props.onDeleteBrainThread(thread.id);
                                      setThreadMenuId(null);
                                      setThreadDeleteConfirmId(null);
                                    } else {
                                      setThreadDeleteConfirmId(thread.id);
                                    }
                                  }}
                                >
                                  <Trash2 size={13} />
                                  {threadDeleteConfirmId === thread.id ? "再点一次确认删除" : "删除对话"}
                                </button>
                              </div>
                            )}
                          </>
                        )}
                      </div>
                    );
                  })
                ) : (
                  <div className="business-workbench__rail-empty is-compact">
                    <MessageSquareText size={17} />
                    <span>暂无对话</span>
                  </div>
                )}
              </div>
            </section>
          ) : (
            <section className="business-workbench__project-rail" aria-label="商务项目">
              {createProjectOpen && (
                <form
                  className="business-workbench__create-project"
                  onSubmit={handleCreateProject}
                >
                  <label>
                    <span>项目名称</span>
                    <input
                      value={props.createProjectDraft.name}
                      onChange={(event) =>
                        props.onCreateProjectDraftChange({
                          ...props.createProjectDraft,
                          name: event.currentTarget.value,
                        })
                      }
                      placeholder="例如：年度品牌视频"
                      autoFocus
                    />
                  </label>
                  <label>
                    <span>客户名称</span>
                    <input
                      value={props.createProjectDraft.clientName}
                      onChange={(event) =>
                        props.onCreateProjectDraftChange({
                          ...props.createProjectDraft,
                          clientName: event.currentTarget.value,
                        })
                      }
                      placeholder="客户或公司名称"
                    />
                  </label>
                  <button
                    type="submit"
                    disabled={
                      props.isCreatingProject ||
                      !props.isDesktopRuntime ||
                      !props.createProjectDraft.name.trim() ||
                      !props.createProjectDraft.clientName.trim()
                    }
                  >
                    {props.isCreatingProject ? (
                      <LoaderCircle size={13} className="business-workbench__spin" />
                    ) : (
                      <Plus size={13} />
                    )}
                    创建项目
                  </button>
                </form>
              )}

              <label className="business-workbench__project-search">
                <Search size={13} aria-hidden="true" />
                <input
                  value={props.projectQuery}
                  onChange={(event) =>
                    props.onProjectQueryChange(event.currentTarget.value)
                  }
                  placeholder="搜索项目"
                  aria-label="搜索项目或客户"
                />
              </label>

              <div className="business-workbench__project-list">
                {filteredProjects.length > 0 ? (
                  filteredProjects.map((project) => {
                    const active = project.id === props.selectedProjectId;
                    return (
                      <button
                        type="button"
                        key={project.id}
                        className={active ? "is-active" : ""}
                        onClick={() => selectProject(project.id)}
                        aria-pressed={active}
                      >
                        <span className="business-workbench__project-avatar">
                          {initials(project.name)}
                        </span>
                        <span className="business-workbench__project-copy">
                          <strong>{project.name}</strong>
                          <small>{project.clientName}</small>
                        </span>
                      </button>
                    );
                  })
                ) : (
                  <div className="business-workbench__rail-empty">
                    <FolderKanban size={18} />
                    <span>{props.projectQuery ? "没有匹配项目" : "暂无项目"}</span>
                  </div>
                )}
              </div>
            </section>
          )}
        </aside>

        <main className="business-workbench__main">
          {activeSection === "workspace" && (
            <WorkspaceHome
              dock={
                <BusinessAgentDock
                  context={{
                    projectName: selectedProject?.name ?? null,
                    customerName:
                      selectedProject?.clientName ??
                      props.businessWorkspace?.profile.customerName ??
                      null,
                    brief: latestConfirmedRequirement,
                    workspace: props.businessWorkspace,
                    contractFindings: props.contractAgentFindings,
                  }}
                  disabled={brainDegraded}
                  onComposeSkill={props.onBrainDraftChange}
                />
              }
            >
              {brainCenter}
            </WorkspaceHome>
          )}

          {activeSection === "brain" && (
            <div className="business-workbench__component-stage business-workbench__contract-stage">
              <ContractReviewCenter
                reviews={props.reviews}
                selectedReviewId={props.selectedReviewId}
                selectedReview={props.selectedReview}
                findings={props.findings}
                selectedFindingId={props.selectedFindingId}
                evidenceContext={props.evidenceContext}
                backups={props.backups}
                businessWorkspace={props.businessWorkspace}
                assetActionCapabilities={props.assetActionCapabilities}
                selectedSource={props.selectedSource}
                hasSelectedProject={props.hasSelectedProject}
                isDesktopRuntime={props.isDesktopRuntime}
                isLoading={props.isLoading}
                busyAction={props.busyAction}
                error={displayError}
                onChooseSource={props.onChooseSource}
                onClearSource={props.onClearSource}
                onImportSource={props.onImportSource}
                onRefresh={props.onRefresh}
                onSelectReview={props.onSelectReview}
                onStartReview={props.onStartReview}
                onCancelReview={props.onCancelReview}
                onRetryStage={props.onRetryStage}
                onSelectFinding={props.onSelectFinding}
                onSelectEvidence={props.onSelectEvidence}
                onDecideFinding={props.onDecideFinding}
                onGenerateReport={props.onGenerateReport}
                onPromoteReviewedContract={props.onPromoteReviewedContract}
                onOpenAsset={props.onOpenAsset}
                onExportAsset={props.onExportAsset}
                onRetryBackup={props.onRetryBackup}
                onRestoreBackup={props.onRestoreBackup}
              />
            </div>
          )}

          {activeSection === "requirements" && (
            <div className="business-workbench__component-stage">
              <RequirementBriefCenter
                projects={props.projects}
                briefs={props.requirementBriefs}
                cases={props.cases}
                selectedProjectId={props.selectedProjectId}
                draft={props.requirementBriefDraft}
                hasConflict={props.requirementBriefConflict}
                isRefreshing={props.isRefreshingRequirementBriefs}
                isSaving={props.isSavingRequirementBrief}
                error={displayError}
                onSelectProject={props.onSelectProject}
                onDraftChange={props.onRequirementBriefDraftChange}
                onCreate={props.onCreateRequirementBrief}
                onSave={props.onSaveRequirementBrief}
                onChangeStatus={props.onChangeRequirementBriefStatus}
                onRefresh={props.onRefreshRequirementBriefs}
                onReloadConflict={props.onReloadRequirementBrief}
                onRebaseConflict={props.onRebaseRequirementBrief}
              />
            </div>
          )}

          {activeSection === "projects" && (
            <div className="business-workbench__component-stage">
              <BusinessDocumentsCenter
                projects={props.projects}
                selectedProjectId={props.selectedProjectId}
                workspace={props.businessWorkspace}
                quoteHistorySources={props.quoteHistorySources}
                latestConfirmedRequirement={latestConfirmedRequirement}
                workspaceEvents={props.businessWorkspaceEvents}
                assets={props.assets}
                businessCustomers={props.businessCustomers}
                busyAction={props.businessBusyAction}
                error={displayError}
                isDesktopRuntime={props.isDesktopRuntime}
                assetActionCapabilities={props.assetActionCapabilities}
                onSelectProject={props.onSelectProject}
                onOpenAsset={props.onOpenAsset}
                onExportAsset={props.onExportAsset}
                onCreateBusinessWorkspace={props.onCreateBusinessWorkspace}
                onListBusinessWorkspacePrefillCandidates={props.onListBusinessWorkspacePrefillCandidates}
                onPreviewBusinessWorkspacePrefill={props.onPreviewBusinessWorkspacePrefill}
                onRefreshBusinessWorkspaces={props.onRefreshBusinessWorkspaces}
                onUpdateBusinessProfile={props.onUpdateBusinessProfile}
                onCreateBusinessDocument={props.onCreateBusinessDocument}
                onChangeBusinessDocumentStatus={props.onChangeBusinessDocumentStatus}
                onGenerateBusinessDocument={props.onGenerateBusinessDocument}
                onUpsertBusinessPayment={props.onUpsertBusinessPayment}
                onConfirmBusinessQuote={props.onConfirmBusinessQuote}
                onRecordBusinessReceipt={props.onRecordBusinessReceipt}
                onReverseBusinessReceipt={props.onReverseBusinessReceipt}
                onAdoptLatestConfirmedRequirement={props.onAdoptLatestConfirmedRequirement}
                onChangeBusinessWorkspaceStatus={props.onChangeBusinessWorkspaceStatus}
                onUpsertBusinessCustomer={props.onUpsertBusinessCustomer}
                onAssignBusinessCustomer={props.onAssignBusinessCustomer}
                onUpsertBusinessMilestone={props.onUpsertBusinessMilestone}
                onRegisterBusinessDeliverableVersion={props.onRegisterBusinessDeliverableVersion}
                onRecordBusinessDeliverySent={props.onRecordBusinessDeliverySent}
                onRecordBusinessDeliverySignoff={props.onRecordBusinessDeliverySignoff}
                onRecordBusinessInvoiceIssued={props.onRecordBusinessInvoiceIssued}
                onRecordBusinessInvoiceRedCorrection={props.onRecordBusinessInvoiceRedCorrection}
                onAttachBusinessInvoiceAsset={props.onAttachBusinessInvoiceAsset}
                onCreateBusinessArchiveSnapshot={props.onCreateBusinessArchiveSnapshot}
                onImportBusinessAsset={props.onImportBusinessAsset}
                onDismissError={props.onDismissBusinessError}
              />
            </div>
          )}

          {activeSection === "tasks" && (
            <div className="business-workbench__component-stage">
              <TaskCenter
                tasks={props.tasks}
                statusFilter={props.taskStatusFilter}
                projectNames={projectNames}
                busyTaskIds={props.busyTaskIds}
                isLoading={props.isLoading || props.isRefreshingTasks}
                error={displayError}
                onStatusFilterChange={props.onTaskStatusFilterChange}
                onCancel={props.onCancelTask}
                onRetry={props.onRetryTask}
                onReload={props.onRefreshTasks}
              />
            </div>
          )}

          {activeSection === "assets" && (
            <div className="business-workbench__component-stage">
              <AssetVault
                assets={props.assets}
                projects={assetProjects}
                projectFilter={props.assetProjectFilter}
                viewMode={props.assetViewMode}
                selectedSource={props.selectedAssetSource}
                importProjectId={props.importProjectId}
                isLoading={props.isLoading || props.isRefreshingAssets}
                isSelectingSource={props.isSelectingAssetSource}
                isImporting={props.isImportingAsset}
                error={displayError}
                onProjectFilterChange={props.onAssetProjectFilterChange}
                onViewModeChange={props.onAssetViewModeChange}
                onChooseSource={props.onChooseAssetSource}
                onClearSource={props.onClearAssetSource}
                onImportProjectChange={props.onImportProjectChange}
                onImport={props.onImportAsset}
                onReload={props.onRefreshAssets}
              />
            </div>
          )}

          {activeSection === "creative" && (
            <div className="business-workbench__component-stage">
              <CaseLibrary
                cases={props.cases}
                assets={props.assets}
                filters={props.caseFilters}
                viewMode={props.caseViewMode}
                editor={props.caseEditor}
                isLoading={props.isLoading || props.isRefreshingCases}
                isSaving={props.isSavingCase}
                error={displayError}
                onFiltersChange={props.onCaseFiltersChange}
                onViewModeChange={props.onCaseViewModeChange}
                onOpenCreate={props.onOpenCreateCase}
                onOpenEdit={props.onOpenEditCase}
                onEditorChange={props.onCaseEditorChange}
                onCloseEditor={props.onCloseCaseEditor}
                onSave={props.onSaveCase}
                onReload={props.onRefreshCases}
              />
            </div>
          )}

          {activeSection === "execution" && (
            <div className="business-workbench__component-stage">
              <ExecutionBriefCenter
                projects={props.projects}
                briefs={props.executionBriefs}
                selectedProjectId={props.selectedProjectId}
                draft={props.executionBriefDraft}
                isRefreshing={props.isRefreshingExecutionBriefs}
                isSaving={props.isSavingExecutionBrief}
                error={displayError}
                onSelectProject={props.onSelectProject}
                onDraftChange={props.onExecutionBriefDraftChange}
                onCreate={props.onCreateExecutionBrief}
                onSave={props.onSaveExecutionBrief}
                onChangeStatus={props.onChangeExecutionBriefStatus}
                onRefresh={props.onRefreshExecutionBriefs}
              />
            </div>
          )}

          {activeSection === "system" && (
            <div className="business-workbench__component-stage">
              <SystemStatusPanel
                hostStatus={props.hostStatus}
                codexStatus={props.codexStatus}
                mediaHealth={props.mediaHealth}
                recentEvents={props.recentEvents}
                isProbingCodex={props.isProbingCodex}
                onProbeCodex={props.onProbeCodex}
              />
            </div>
          )}
        </main>

        <aside
          className={`business-workbench__context ${contextOpen ? "is-open" : ""}`}
          aria-label="项目上下文"
          hidden={!contextOpen}
        >
          <div className="business-workbench__context-header">
            <strong>项目详情</strong>
            <button
              type="button"
              onClick={() => setContextOpen(false)}
              aria-label="收起项目上下文"
            >
              <X size={15} />
            </button>
          </div>

          {selectedProject ? (
            <>
              <section className="business-workbench__context-section">
                <div className="business-workbench__project-context-card">
                  <span className="business-workbench__project-avatar is-large">
                    {initials(selectedProject.name)}
                  </span>
                  <div>
                    <strong>{selectedProject.name}</strong>
                    <span>{selectedProject.clientName}</span>
                    <small>更新于 {formatDateTime(selectedProject.updatedAt)}</small>
                  </div>
                </div>
                <div className="business-workbench__stage-summary">
                  <span>当前环节</span>
                  <strong>{stageLabel(selectedProject.stage)}</strong>
                </div>
                <div
                  className="business-workbench__stage-stepper"
                  role="group"
                  aria-label="推进项目环节"
                >
                  {STAGES.map((stage) => {
                    const activeIndex = STAGES.findIndex(
                      (item) => item.id === selectedProject.stage,
                    );
                    const index = STAGES.findIndex((item) => item.id === stage.id);
                    const reached = index <= activeIndex;
                    const current = stage.id === selectedProject.stage;
                    return (
                      <button
                        type="button"
                        key={stage.id}
                        className={`business-workbench__stage-chip ${reached ? "is-reached" : ""} ${current ? "is-current" : ""}`}
                        disabled={current || props.isChangingStage}
                        title={
                          current
                            ? `当前处于「${stage.label}」`
                            : `切换到「${stage.label}」`
                        }
                        onClick={() =>
                          props.onChangeStage(selectedProject.id, stage.id)
                        }
                      >
                        {stage.shortLabel}
                      </button>
                    );
                  })}
                </div>
              </section>

              <section className="business-workbench__context-section">
                <div className="business-workbench__context-title">
                  <span>项目进度</span>
                </div>
                <ContextStatus
                  icon={ClipboardList}
                  label="需求"
                  value={requirementStatusLabel(selectedRequirement?.status)}
                  tone={
                    selectedRequirement?.status === "confirmed"
                      ? "ready"
                      : selectedRequirement
                        ? "warning"
                        : "muted"
                  }
                  onClick={() => navigate("requirements")}
                />
                <ContextStatus
                  icon={Archive}
                  label="文件"
                  value={`${selectedAssets.length} 个`}
                  tone="ready"
                  onClick={() => navigate("assets")}
                />
              </section>

              <section className="business-workbench__context-section">
                <div className="business-workbench__context-title">
                  <span>任务动态</span>
                  <button type="button" onClick={() => navigate("tasks")}>
                    查看 <ChevronRight size={12} />
                  </button>
                </div>
                <div className="business-workbench__mini-task-list">
                  {selectedTasks.length > 0 ? (
                    selectedTasks.slice(0, 4).map((task) => (
                      <MiniTask key={task.id} task={task} />
                    ))
                  ) : (
                    <div className="business-workbench__context-empty">
                      当前项目暂无待处理任务
                    </div>
                  )}
                </div>
              </section>
            </>
          ) : (
            <section className="business-workbench__context-empty-card">
              <FolderKanban size={24} />
              <strong>选择项目</strong>
            </section>
          )}

        </aside>

        <BusinessReceivablesDrawer
          open={receivablesOpen}
          customers={props.businessCustomers}
          loading={props.businessCustomersLoading}
          error={props.businessCustomersError}
          query={props.businessCustomerQuery}
          selectedWorkspaceId={props.businessWorkspace?.id ?? null}
          onClose={() => setReceivablesOpen(false)}
          onQueryChange={props.onBusinessCustomerQueryChange}
          onRetry={props.onRefreshBusinessCustomers}
          onSelectCustomer={(customer) => {
            void (async () => {
              const result = await props.onSelectBusinessCustomer(customer);
              if (result !== false) {
                setReceivablesOpen(false);
              }
            })();
          }}
        />
      </div>
    </div>
  );
}

interface WorkspaceHomeProps {
  dock?: ReactNode;
  children: ReactNode;
}

function WorkspaceHome({ dock, children }: WorkspaceHomeProps) {
  return (
    <section className="business-workbench__section business-workbench__home">
      {dock}
      <div className="business-workbench__agent-stage">{children}</div>
    </section>
  );
}

function SystemStatusPanel({
  hostStatus,
  codexStatus,
  mediaHealth,
  recentEvents,
  isProbingCodex,
  onProbeCodex,
}: {
  hostStatus: BusinessWorkbenchProps["hostStatus"];
  codexStatus: BusinessWorkbenchProps["codexStatus"];
  mediaHealth: BusinessWorkbenchProps["mediaHealth"];
  recentEvents: BusinessWorkbenchProps["recentEvents"];
  isProbingCodex?: boolean;
  onProbeCodex: () => void;
}) {
  return (
    <section className="business-workbench__system" aria-label="系统状态">
      <div className="business-workbench__system-grid">
        <article className="business-workbench__system-card">
          <header>
            <strong>本地主权威</strong>
          </header>
          {hostStatus ? (
            <ul>
              <li>
                协议版本 <code>{hostStatus.protocolVersion}</code>
              </li>
              <li>数据库 {hostStatus.databaseReady ? "就绪" : "异常"}</li>
              <li>Local Vault {hostStatus.vaultReady ? "就绪" : "异常"}</li>
              <li>
                项目 {hostStatus.projectCount} · 任务 {hostStatus.taskCount} · 文件{" "}
                {hostStatus.assetCount}
              </li>
              <li>事件序列 {hostStatus.lastEventSequence}</li>
            </ul>
          ) : (
            <p>尚未获取主机状态。</p>
          )}
        </article>

        <article className="business-workbench__system-card">
          <header>
            <strong>Codex 智能审查</strong>
            <button
              type="button"
              onClick={onProbeCodex}
              disabled={isProbingCodex}
            >
              {isProbingCodex ? (
                <LoaderCircle size={13} className="business-workbench__spin" />
              ) : (
                <RefreshCw size={13} />
              )}
              验证握手
            </button>
          </header>
          {codexStatus ? (
            <ul>
              <li>{codexStatus.available ? "可用" : "不可用"}</li>
              <li>
                运行时 <code>{codexStatus.runtime}</code>
              </li>
              {codexStatus.handshakeAt ? (
                <li>最近握手 {formatDateTime(codexStatus.handshakeAt)}</li>
              ) : null}
              {codexStatus.error ? (
                <li className="is-danger">{codexStatus.error}</li>
              ) : null}
            </ul>
          ) : (
            <p>尚未执行握手检测。</p>
          )}
        </article>

        <article className="business-workbench__system-card">
          <header>
            <strong>本地媒体引擎</strong>
          </header>
          {mediaHealth ? (
            <ul>
              <li>状态 {mediaHealth.state}</li>
              <li>FFmpeg {mediaHealth.ffmpegAvailable ? "就绪" : "缺失"}</li>
              <li>FFprobe {mediaHealth.ffprobeAvailable ? "就绪" : "缺失"}</li>
            </ul>
          ) : (
            <p>尚未获取媒体引擎状态。</p>
          )}
        </article>
      </div>

      <article className="business-workbench__system-card is-events">
        <header>
          <strong>最近项目事件</strong>
        </header>
        {recentEvents.length > 0 ? (
          <ul>
            {recentEvents.slice(0, 10).map((event) => (
              <li key={event.eventId}>
                <code>#{event.sequence}</code> {event.eventType} ·{" "}
                {event.project.name} · {formatDateTime(event.occurredAt)}
              </li>
            ))}
          </ul>
        ) : (
          <p>暂无事件记录。</p>
        )}
      </article>
    </section>
  );
}

function ContextStatus({
  icon: Icon,
  label,
  value,
  tone,
  onClick,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  tone: StatusTone;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="business-workbench__context-status"
      onClick={onClick}
    >
      <span className={`business-workbench__status-icon is-${tone}`}>
        <Icon size={14} />
      </span>
      <span>
        <strong>{label}</strong>
        <small>{value}</small>
      </span>
      <ChevronRight size={13} />
    </button>
  );
}

function MiniTask({ task }: { task: TaskRecord }) {
  const status = taskStatusMeta(task.status);
  return (
    <div className="business-workbench__mini-task">
      <span className={`business-workbench__task-dot is-${status.tone}`} />
      <span>
        <strong>{task.kind}</strong>
        <small>
          {status.label}
          {task.status === "running" ? ` · ${Math.round(task.progress)}%` : ""}
        </small>
      </span>
      <time dateTime={dateTimeValue(task.updatedAt)}>
        {formatShortTime(task.updatedAt)}
      </time>
    </div>
  );
}

function stageLabel(stage: ProjectStage): string {
  return STAGES.find((item) => item.id === stage)?.label ?? stage;
}

function requirementStatusLabel(status?: string): string {
  if (status === "confirmed") return "已确认";
  if (status === "review") return "待复核";
  if (status === "interviewing") return "访谈中";
  return "未建立";
}

function taskStatusMeta(status: TaskRecord["status"]): {
  label: string;
  tone: StatusTone;
} {
  switch (status) {
    case "succeeded":
      return { label: "已完成", tone: "ready" };
    case "failed":
      return { label: "失败", tone: "danger" };
    case "running":
      return { label: "执行中", tone: "warning" };
    case "awaitingApproval":
      return { label: "待审批", tone: "warning" };
    case "canceled":
      return { label: "已取消", tone: "muted" };
    default:
      return { label: "排队中", tone: "muted" };
  }
}

function normalizeError(error: string | null): string | null {
  return error;
}

function initials(value: string): string {
  const normalized = value.trim();
  if (!normalized) return "项";
  return normalized.slice(0, 2).toLocaleUpperCase("zh-CN");
}

function normalizeTimestamp(value: number): number {
  return value < 10_000_000_000 ? value * 1000 : value;
}

function formatDateTime(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(normalizeTimestamp(value)));
}

function formatShortTime(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(normalizeTimestamp(value)));
}

function dateTimeValue(value: number): string {
  return new Date(normalizeTimestamp(value)).toISOString();
}

export default BusinessWorkbench;
