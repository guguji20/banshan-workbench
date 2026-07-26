import { useEffect, useState, type FormEvent } from "react";
import type { LucideIcon } from "lucide-react";
import packageMetadata from "../../package.json";
import {
  Activity,
  AlertCircle,
  Archive,
  Bot,
  BriefcaseBusiness,
  Check,
  CheckCircle2,
  CircleDot,
  ClipboardList,
  Clock3,
  Database,
  FilePenLine,
  FolderKanban,
  GalleryHorizontalEnd,
  HardDrive,
  Inbox,
  Images,
  LayoutDashboard,
  ListTodo,
  LoaderCircle,
  MonitorCog,
  Network,
  PanelsTopLeft,
  Plus,
  RefreshCw,
  Save,
  Search,
  Server,
  WandSparkles,
  X,
} from "lucide-react";
import type { BriefRecord } from "../generated/bsaigc/BriefRecord";
import type { CodexProbeStatus } from "../generated/bsaigc/CodexProbeStatus";
import type { CreateProjectPayload } from "../generated/bsaigc/CreateProjectPayload";
import type { DomainEvent } from "../generated/bsaigc/DomainEvent";
import type { HostError } from "../generated/bsaigc/HostError";
import type { HostStatus } from "../generated/bsaigc/HostStatus";
import type { ProjectRecord } from "../generated/bsaigc/ProjectRecord";
import type { ProjectStage } from "../generated/bsaigc/ProjectStage";
import type { AssetRecord } from "../generated/bsaigc/AssetRecord";
import type { AssetSourceSelection } from "../generated/bsaigc/AssetSourceSelection";
import type { TaskRecord } from "../generated/bsaigc/TaskRecord";
import type { BrainHostHealth } from "../generated/bsaigc/BrainHostHealth";
import type { BrainThreadRecord } from "../generated/bsaigc/BrainThreadRecord";
import type { BrainTurnRecord } from "../generated/bsaigc/BrainTurnRecord";
import type { NativeMediaHealth } from "../generated/bsaigc/NativeMediaHealth";
import type { CaseRecord } from "../generated/bsaigc/CaseRecord";
import type { ExecutionBriefContent } from "../generated/bsaigc/ExecutionBriefContent";
import type { ExecutionBriefRecord } from "../generated/bsaigc/ExecutionBriefRecord";
import type { ExecutionBriefStatus } from "../generated/bsaigc/ExecutionBriefStatus";
import type { RequirementBriefRecord } from "../generated/bsaigc/RequirementBriefRecord";
import type { RequirementBriefStatus } from "../generated/bsaigc/RequirementBriefStatus";
import type { RequirementBriefDraft } from "../requirementBriefDrafts";
import {
  AssetVault,
  type AssetProjectFilter,
  type AssetVaultViewMode,
} from "./AssetVault";
import { TaskCenter, type TaskStatusFilter } from "./TaskCenter";
import { BrainCenter, type BrainModelOption } from "./BrainCenter";
import {
  CaseLibrary,
  type CaseEditorState,
  type CaseLibraryFilters,
  type CaseLibraryViewMode,
} from "./CaseLibrary";
import { ExecutionBriefCenter } from "./ExecutionBriefCenter";
import { RequirementBriefCenter } from "./RequirementBriefCenter";
import "./DesktopShell.css";

export type DesktopSection =
  | "workspace"
  | "brain"
  | "projects"
  | "requirements"
  | "creative"
  | "execution"
  | "canvas"
  | "tasks"
  | "assets"
  | "system";

export interface DesktopShellProps {
  activeSection: DesktopSection;
  projects: readonly ProjectRecord[];
  selectedProjectId: string | null;
  projectQuery: string;
  createProjectDraft: CreateProjectPayload;
  briefDraft: BriefRecord | null;
  hostStatus: HostStatus | null;
  codexStatus: CodexProbeStatus | null;
  recentEvents: readonly DomainEvent[];
  tasks: readonly TaskRecord[];
  taskStatusFilter: TaskStatusFilter;
  busyTaskIds: readonly string[];
  assets: readonly AssetRecord[];
  cases: readonly CaseRecord[];
  executionBriefs: readonly ExecutionBriefRecord[];
  executionBriefDraft: ExecutionBriefContent | null;
  requirementBriefs: readonly RequirementBriefRecord[];
  requirementBriefDraft: RequirementBriefDraft | null;
  requirementBriefConflict: boolean;
  caseFilters: CaseLibraryFilters;
  caseViewMode: CaseLibraryViewMode;
  caseEditor: CaseEditorState | null;
  brainThreads: readonly BrainThreadRecord[];
  brainTurns: readonly BrainTurnRecord[];
  brainHealth: BrainHostHealth | null;
  mediaHealth: NativeMediaHealth | null;
  brainModels: readonly BrainModelOption[];
  selectedBrainThreadId: string | null;
  selectedBrainModel: string;
  brainDraft: string;
  brainStreamingDelta: string;
  assetProjectFilter: AssetProjectFilter;
  assetViewMode: AssetVaultViewMode;
  selectedAssetSource: AssetSourceSelection | null;
  importProjectId: string | null;
  error?: HostError | string | null;
  isDesktopRuntime: boolean;
  isLoading?: boolean;
  isCreatingProject?: boolean;
  isSavingBrief?: boolean;
  isChangingStage?: boolean;
  isProbingCodex?: boolean;
  isRefreshingTasks?: boolean;
  isRefreshingAssets?: boolean;
  isSelectingAssetSource?: boolean;
  isImportingAsset?: boolean;
  isLoadingBrainThreads?: boolean;
  isLoadingBrainTurns?: boolean;
  isStartingBrainThread?: boolean;
  isSendingBrainTurn?: boolean;
  isRefreshingCases?: boolean;
  isSavingCase?: boolean;
  isRefreshingExecutionBriefs?: boolean;
  isSavingExecutionBrief?: boolean;
  isRefreshingRequirementBriefs?: boolean;
  isSavingRequirementBrief?: boolean;
  onNavigate: (section: DesktopSection) => void;
  onSelectProject: (projectId: string) => void;
  onProjectQueryChange: (query: string) => void;
  onCreateProjectDraftChange: (draft: CreateProjectPayload) => void;
  onCreateProject: (draft: CreateProjectPayload) => void;
  onBriefDraftChange: (brief: BriefRecord) => void;
  onSaveBrief: (projectId: string, brief: BriefRecord) => void;
  onChangeStage: (projectId: string, stage: ProjectStage) => void;
  onProbeCodex: () => void;
  onTaskStatusFilterChange: (status: TaskStatusFilter) => void;
  onCancelTask: (task: TaskRecord) => void;
  onRetryTask: (task: TaskRecord, approved: boolean) => void;
  onRefreshTasks: () => void;
  onAssetProjectFilterChange: (projectId: AssetProjectFilter) => void;
  onAssetViewModeChange: (mode: AssetVaultViewMode) => void;
  onChooseAssetSource: () => void;
  onClearAssetSource: () => void;
  onImportProjectChange: (projectId: string | null) => void;
  onImportAsset: (
    source: AssetSourceSelection,
    projectId: string | null,
  ) => void;
  onRefreshAssets: () => void;
  onSelectBrainThread: (threadId: string) => void;
  onBrainModelChange: (modelId: string) => void;
  onBrainDraftChange: (value: string) => void;
  onSendBrainTurn: () => void;
  onInterruptBrainTurn: () => void;
  onNewBrainThread: () => void;
  onRefreshBrain: () => void;
  onCaseFiltersChange: (filters: CaseLibraryFilters) => void;
  onCaseViewModeChange: (mode: CaseLibraryViewMode) => void;
  onOpenCreateCase: () => void;
  onOpenEditCase: (caseRecord: CaseRecord) => void;
  onCaseEditorChange: (editor: CaseEditorState) => void;
  onCloseCaseEditor: () => void;
  onSaveCase: (editor: CaseEditorState) => void;
  onRefreshCases: () => void;
  onExecutionBriefDraftChange: (draft: ExecutionBriefContent) => void;
  onCreateExecutionBrief: (
    projectId: string,
    draft: ExecutionBriefContent,
  ) => void;
  onSaveExecutionBrief: (
    record: ExecutionBriefRecord,
    draft: ExecutionBriefContent,
  ) => void;
  onChangeExecutionBriefStatus: (
    record: ExecutionBriefRecord,
    status: ExecutionBriefStatus,
  ) => void;
  onRefreshExecutionBriefs: () => void;
  onRequirementBriefDraftChange: (draft: RequirementBriefDraft) => void;
  onCreateRequirementBrief: (projectId: string) => void;
  onSaveRequirementBrief: (
    record: RequirementBriefRecord,
    draft: RequirementBriefDraft,
  ) => void;
  onChangeRequirementBriefStatus: (
    record: RequirementBriefRecord,
    status: RequirementBriefStatus,
    draft: RequirementBriefDraft,
  ) => void;
  onRefreshRequirementBriefs: () => void;
  onReloadRequirementBrief: (record: RequirementBriefRecord) => void;
  onRebaseRequirementBrief: (record: RequirementBriefRecord) => void;
  onRetry?: () => void;
  onDismissError?: () => void;
}

type StatusTone = "healthy" | "warning" | "danger" | "neutral";

const NAV_ITEMS: ReadonlyArray<{
  id: DesktopSection;
  label: string;
  icon: LucideIcon;
  available: boolean;
}> = [
  { id: "brain", label: "System Brain", icon: Bot, available: true },
  { id: "workspace", label: "执行中心", icon: LayoutDashboard, available: true },
  { id: "projects", label: "项目生产台", icon: FolderKanban, available: true },
  { id: "requirements", label: "需求访谈", icon: ClipboardList, available: true },
  { id: "creative", label: "创意中心", icon: WandSparkles, available: true },
  { id: "canvas", label: "无限画布", icon: PanelsTopLeft, available: false },
  { id: "tasks", label: "任务中心", icon: ListTodo, available: true },
  { id: "assets", label: "资产库", icon: Images, available: true },
  { id: "system", label: "系统状态", icon: MonitorCog, available: false },
];

const STAGES: ReadonlyArray<{ id: ProjectStage; label: string }> = [
  { id: "intake", label: "接洽" },
  { id: "briefing", label: "需求" },
  { id: "creative", label: "创意" },
  { id: "production", label: "拍摄" },
  { id: "postProduction", label: "后期" },
  { id: "review", label: "审阅" },
  { id: "delivery", label: "交付" },
  { id: "closed", label: "归档" },
];

const EVENT_LABELS: Record<DomainEvent["eventType"], string> = {
  "project.created": "创建项目",
  "project.briefUpdated": "更新 Brief",
  "project.stageChanged": "变更阶段",
};

const DATE_FORMATTER = new Intl.DateTimeFormat("zh-CN", {
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});

function formatDate(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return "--";
  return DATE_FORMATTER.format(new Date(timestamp));
}

function splitLines(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function displayError(error: HostError | string): string {
  return typeof error === "string" ? error : error.message;
}

function stageLabel(stage: ProjectStage): string {
  return STAGES.find((item) => item.id === stage)?.label ?? stage;
}

function SystemStatusRow({
  icon: Icon,
  label,
  detail,
  tone,
}: {
  icon: LucideIcon;
  label: string;
  detail: string;
  tone: StatusTone;
}) {
  return (
    <div className="desktop-shell__status-row">
      <div className="desktop-shell__status-icon" aria-hidden="true">
        <Icon size={17} strokeWidth={1.8} />
      </div>
      <div className="desktop-shell__status-copy">
        <span className="desktop-shell__status-label">{label}</span>
        <span className="desktop-shell__status-detail">{detail}</span>
      </div>
      <span
        className={`desktop-shell__status-dot desktop-shell__status-dot--${tone}`}
        aria-label={
          tone === "healthy"
            ? "正常"
            : tone === "neutral"
              ? "待检测"
              : tone === "warning"
                ? "需注意"
                : "异常"
        }
      />
    </div>
  );
}

function ListField({
  id,
  label,
  value,
  placeholder,
  onChange,
}: {
  id: string;
  label: string;
  value: readonly string[];
  placeholder: string;
  onChange: (value: string[]) => void;
}) {
  const canonicalValue = value.join("\n");
  const [draft, setDraft] = useState(canonicalValue);

  useEffect(() => {
    // Keep a trailing newline while the next list item is being entered.
    if (splitLines(draft).join("\n") !== canonicalValue) {
      setDraft(canonicalValue);
    }
  }, [canonicalValue, draft]);

  return (
    <label className="desktop-shell__field" htmlFor={id}>
      <span className="desktop-shell__field-label">{label}</span>
      <textarea
        id={id}
        rows={4}
        value={draft}
        placeholder={placeholder}
        onChange={(event) => {
          const nextDraft = event.currentTarget.value;
          setDraft(nextDraft);
          onChange(splitLines(nextDraft));
        }}
      />
      {value.length > 0 && (
        <span className="desktop-shell__field-count">{value.length} 项</span>
      )}
    </label>
  );
}

function LoadingProjectList() {
  return (
    <div className="desktop-shell__project-skeletons" aria-label="正在加载项目">
      {[0, 1, 2, 3].map((item) => (
        <div className="desktop-shell__project-skeleton" key={item}>
          <span />
          <span />
          <span />
        </div>
      ))}
    </div>
  );
}

export function DesktopShell({
  activeSection,
  projects,
  selectedProjectId,
  projectQuery,
  createProjectDraft,
  briefDraft,
  hostStatus,
  codexStatus,
  recentEvents,
  tasks,
  taskStatusFilter,
  busyTaskIds,
  assets,
  cases,
  executionBriefs,
  executionBriefDraft,
  requirementBriefs,
  requirementBriefDraft,
  requirementBriefConflict,
  caseFilters,
  caseViewMode,
  caseEditor,
  brainThreads,
  brainTurns,
  brainHealth,
  mediaHealth,
  brainModels,
  selectedBrainThreadId,
  selectedBrainModel,
  brainDraft,
  brainStreamingDelta,
  assetProjectFilter,
  assetViewMode,
  selectedAssetSource,
  importProjectId,
  error = null,
  isDesktopRuntime,
  isLoading = false,
  isCreatingProject = false,
  isSavingBrief = false,
  isChangingStage = false,
  isProbingCodex = false,
  isRefreshingTasks = false,
  isRefreshingAssets = false,
  isSelectingAssetSource = false,
  isImportingAsset = false,
  isLoadingBrainThreads = false,
  isLoadingBrainTurns = false,
  isStartingBrainThread = false,
  isSendingBrainTurn = false,
  isRefreshingCases = false,
  isSavingCase = false,
  isRefreshingExecutionBriefs = false,
  isSavingExecutionBrief = false,
  isRefreshingRequirementBriefs = false,
  isSavingRequirementBrief = false,
  onNavigate,
  onSelectProject,
  onProjectQueryChange,
  onCreateProjectDraftChange,
  onCreateProject,
  onBriefDraftChange,
  onSaveBrief,
  onChangeStage,
  onProbeCodex,
  onTaskStatusFilterChange,
  onCancelTask,
  onRetryTask,
  onRefreshTasks,
  onAssetProjectFilterChange,
  onAssetViewModeChange,
  onChooseAssetSource,
  onClearAssetSource,
  onImportProjectChange,
  onImportAsset,
  onRefreshAssets,
  onSelectBrainThread,
  onBrainModelChange,
  onBrainDraftChange,
  onSendBrainTurn,
  onInterruptBrainTurn,
  onNewBrainThread,
  onRefreshBrain,
  onCaseFiltersChange,
  onCaseViewModeChange,
  onOpenCreateCase,
  onOpenEditCase,
  onCaseEditorChange,
  onCloseCaseEditor,
  onSaveCase,
  onRefreshCases,
  onExecutionBriefDraftChange,
  onCreateExecutionBrief,
  onSaveExecutionBrief,
  onChangeExecutionBriefStatus,
  onRefreshExecutionBriefs,
  onRequirementBriefDraftChange,
  onCreateRequirementBrief,
  onSaveRequirementBrief,
  onChangeRequirementBriefStatus,
  onRefreshRequirementBriefs,
  onReloadRequirementBrief,
  onRebaseRequirementBrief,
  onRetry,
  onDismissError,
}: DesktopShellProps) {
  const normalizedQuery = projectQuery.trim().toLocaleLowerCase("zh-CN");
  const filteredProjects = normalizedQuery
    ? projects.filter((project) =>
        `${project.name} ${project.clientName}`.toLocaleLowerCase("zh-CN").includes(normalizedQuery),
      )
    : projects;
  const selectedProject = projects.find((project) => project.id === selectedProjectId) ?? null;
  const selectedRequirementAuthority = requirementBriefs.find(
    (brief) => brief.projectId === selectedProjectId,
  ) ?? null;
  const projectNames = Object.fromEntries(
    projects.map((project) => [project.id, project.name]),
  );
  const assetProjects = projects.map(({ id, name }) => ({ id, name }));
  const canCreate =
    createProjectDraft.name.trim().length >= 2 &&
    createProjectDraft.clientName.trim().length > 0 &&
    !isCreatingProject &&
    isDesktopRuntime;
  const canSaveBrief = Boolean(
    selectedProject &&
      briefDraft &&
      !selectedRequirementAuthority &&
      !isSavingBrief &&
      isDesktopRuntime,
  );

  const handleCreateProject = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (canCreate) onCreateProject(createProjectDraft);
  };

  return (
    <div className="desktop-shell">
      <nav className="desktop-shell__nav" aria-label="主导航">
        <div className="desktop-shell__brand">
          <span className="desktop-shell__brand-mark">BS</span>
          <span className="desktop-shell__brand-copy">
            <strong>半山 AIGC</strong>
            <small>DESKTOP</small>
          </span>
        </div>

        <div className="desktop-shell__nav-list">
          {NAV_ITEMS.map(({ id, label, icon: Icon, available }) => (
            <button
              type="button"
              key={id}
              className={`${activeSection === id ? "desktop-shell__nav-item is-active" : "desktop-shell__nav-item"} ${available ? "" : "is-unavailable"}`.trim()}
              onClick={() => onNavigate(id)}
              disabled={!available}
              title={available ? label : `${label}（待接入）`}
              aria-current={activeSection === id ? "page" : undefined}
            >
              <Icon size={19} strokeWidth={1.8} />
              <span>{label}</span>
            </button>
          ))}
        </div>

        <div className="desktop-shell__nav-footer">
          <span className={isDesktopRuntime ? "is-online" : "is-offline"} />
          <span>{isDesktopRuntime ? "本地工作区" : "浏览器预览"}</span>
        </div>
      </nav>

      <main className="desktop-shell__main">
        {!isDesktopRuntime && (
          <div className="desktop-shell__notice" role="status">
            <MonitorCog size={18} />
            <div>
              <strong>当前为浏览器预览</strong>
              <span>项目数据和后台能力仅在半山 AIGC 桌面容器中可用。</span>
            </div>
          </div>
        )}

        {error && activeSection === "projects" && (
          <div className="desktop-shell__error" role="alert">
            <AlertCircle size={18} />
            <span>{displayError(error)}</span>
            {onRetry && (
              <button type="button" onClick={onRetry}>
                重试
              </button>
            )}
            {onDismissError && (
              <button
                type="button"
                className="desktop-shell__icon-button"
                onClick={onDismissError}
                aria-label="关闭错误提示"
                title="关闭"
              >
                <X size={17} />
              </button>
            )}
          </div>
        )}

        {activeSection === "projects" && (
          <>
        <header className="desktop-shell__page-header">
          <div>
            <span className="desktop-shell__eyebrow">PROJECT OPERATIONS</span>
            <h1>项目生产台</h1>
            <p>
              {projects.length} 个项目
              <span aria-hidden="true"> · </span>
              事件序列 #{hostStatus?.lastEventSequence ?? 0}
            </p>
          </div>
          <div className="desktop-shell__header-status">
            {isLoading ? (
              <>
                <LoaderCircle className="desktop-shell__spin" size={16} />
                正在同步
              </>
            ) : (
              <>
                <CheckCircle2 size={16} />
                本地数据已就绪
              </>
            )}
          </div>
        </header>

        <section className="desktop-shell__workspace" aria-label="项目工作区">
          <aside className="desktop-shell__project-rail">
            <div className="desktop-shell__section-heading">
              <div>
                <h2>项目</h2>
                <span>{filteredProjects.length}</span>
              </div>
            </div>

            <form className="desktop-shell__create-form" onSubmit={handleCreateProject}>
              <div className="desktop-shell__create-title">
                <Plus size={15} />
                <span>新建项目</span>
              </div>
              <input
                aria-label="项目名称"
                value={createProjectDraft.name}
                placeholder="项目名称"
                onChange={(event) =>
                  onCreateProjectDraftChange({
                    ...createProjectDraft,
                    name: event.currentTarget.value,
                  })
                }
              />
              <input
                aria-label="客户名称"
                value={createProjectDraft.clientName}
                placeholder="客户名称"
                onChange={(event) =>
                  onCreateProjectDraftChange({
                    ...createProjectDraft,
                    clientName: event.currentTarget.value,
                  })
                }
              />
              <button className="desktop-shell__primary-button" type="submit" disabled={!canCreate}>
                {isCreatingProject ? (
                  <LoaderCircle className="desktop-shell__spin" size={16} />
                ) : (
                  <Plus size={16} />
                )}
                创建
              </button>
            </form>

            <label className="desktop-shell__search">
              <Search size={16} />
              <input
                value={projectQuery}
                placeholder="搜索项目或客户"
                aria-label="搜索项目或客户"
                onChange={(event) => onProjectQueryChange(event.currentTarget.value)}
              />
            </label>

            <div className="desktop-shell__project-list">
              {isLoading && projects.length === 0 ? (
                <LoadingProjectList />
              ) : filteredProjects.length > 0 ? (
                filteredProjects.map((project) => (
                  <button
                    type="button"
                    key={project.id}
                    className={
                      project.id === selectedProjectId
                        ? "desktop-shell__project-item is-selected"
                        : "desktop-shell__project-item"
                    }
                    onClick={() => onSelectProject(project.id)}
                  >
                    <span className="desktop-shell__project-item-topline">
                      <strong>{project.name}</strong>
                      <small>R{project.revision}</small>
                    </span>
                    <span className="desktop-shell__project-client">{project.clientName}</span>
                    <span className="desktop-shell__project-meta">
                      <span>{stageLabel(project.stage)}</span>
                      <time dateTime={new Date(project.updatedAt).toISOString()}>
                        {formatDate(project.updatedAt)}
                      </time>
                    </span>
                  </button>
                ))
              ) : (
                <div className="desktop-shell__rail-empty">
                  <Inbox size={24} strokeWidth={1.5} />
                  <strong>{projects.length === 0 ? "还没有项目" : "没有匹配项目"}</strong>
                  <span>{projects.length === 0 ? "当前工作区为空" : "请调整筛选条件"}</span>
                </div>
              )}
            </div>
          </aside>

          <div className="desktop-shell__editor">
            {selectedProject && briefDraft ? (
              <>
                <header className="desktop-shell__editor-header">
                  <div className="desktop-shell__editor-title">
                    <span className="desktop-shell__project-avatar">
                      {selectedProject.name.trim().slice(0, 1).toLocaleUpperCase("zh-CN")}
                    </span>
                    <div>
                      <div className="desktop-shell__editor-name-line">
                        <h2>{selectedProject.name}</h2>
                        <span>R{selectedProject.revision}</span>
                      </div>
                      <p>{selectedProject.clientName}</p>
                    </div>
                  </div>
                  <div className="desktop-shell__editor-actions">
                    <span className="desktop-shell__updated-at">
                      <Clock3 size={14} />
                      {formatDate(selectedProject.updatedAt)}
                    </span>
                    {selectedRequirementAuthority ? (
                      <button
                        type="button"
                        className="desktop-shell__save-button"
                        onClick={() => onNavigate("requirements")}
                      >
                        <ClipboardList size={16} />
                        需求访谈
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="desktop-shell__save-button"
                        disabled={!canSaveBrief}
                        onClick={() => onSaveBrief(selectedProject.id, briefDraft)}
                      >
                        {isSavingBrief ? (
                          <LoaderCircle className="desktop-shell__spin" size={16} />
                        ) : (
                          <Save size={16} />
                        )}
                        保存 Brief
                      </button>
                    )}
                  </div>
                </header>

                <section className="desktop-shell__stage-section" aria-labelledby="project-stage-title">
                  <div className="desktop-shell__section-title-row">
                    <div>
                      <span className="desktop-shell__section-icon">
                        <Activity size={16} />
                      </span>
                      <div>
                        <h3 id="project-stage-title">项目阶段</h3>
                        <p>当前：{stageLabel(selectedProject.stage)}</p>
                      </div>
                    </div>
                    {isChangingStage && (
                      <span className="desktop-shell__inline-busy">
                        <LoaderCircle className="desktop-shell__spin" size={14} />
                        正在更新
                      </span>
                    )}
                  </div>
                  <div className="desktop-shell__stage-control">
                    {STAGES.map((stage, index) => {
                      const currentIndex = STAGES.findIndex((item) => item.id === selectedProject.stage);
                      const isCurrent = stage.id === selectedProject.stage;
                      const isComplete = index < currentIndex;
                      return (
                        <button
                          type="button"
                          key={stage.id}
                          disabled={isChangingStage || !isDesktopRuntime || isCurrent}
                          className={`${isCurrent ? "is-current" : ""} ${isComplete ? "is-complete" : ""}`.trim()}
                          onClick={() => onChangeStage(selectedProject.id, stage.id)}
                          aria-pressed={isCurrent}
                        >
                          <span>{isComplete ? <Check size={13} /> : index + 1}</span>
                          {stage.label}
                        </button>
                      );
                    })}
                  </div>
                </section>

                <section className="desktop-shell__brief-section" aria-labelledby="brief-title">
                  <div className="desktop-shell__section-title-row">
                    <div>
                      <span className="desktop-shell__section-icon">
                        <FilePenLine size={16} />
                      </span>
                      <div>
                        <h3 id="brief-title">需求 Brief</h3>
                        <p>项目目标与执行边界</p>
                      </div>
                    </div>
                  </div>

                  {selectedRequirementAuthority ? (
                    <div className="desktop-shell__brief-authority">
                      <span
                        className={`is-${selectedRequirementAuthority.status}`}
                      >
                        {selectedRequirementAuthority.status === "confirmed"
                          ? "已确认"
                          : selectedRequirementAuthority.status === "review"
                            ? "待确认"
                            : "访谈中"}
                      </span>
                      <div>
                        <strong>
                          {selectedRequirementAuthority.content.objective ||
                            "需求访谈已接管项目 Brief"}
                        </strong>
                        <p>
                          {selectedRequirementAuthority.content.keyMessage ||
                            "结构化需求、问题答案和确认状态在需求访谈中维护。"}
                        </p>
                      </div>
                      <button
                        type="button"
                        onClick={() => onNavigate("requirements")}
                      >
                        <ClipboardList size={15} />
                        打开
                      </button>
                    </div>
                  ) : (
                  <div className="desktop-shell__brief-grid">
                    <label className="desktop-shell__field" htmlFor="brief-objective">
                      <span className="desktop-shell__field-label">目标</span>
                      <textarea
                        id="brief-objective"
                        rows={4}
                        value={briefDraft.objective}
                        placeholder="本次项目要解决的核心问题"
                        onChange={(event) =>
                          onBriefDraftChange({ ...briefDraft, objective: event.currentTarget.value })
                        }
                      />
                    </label>
                    <label className="desktop-shell__field" htmlFor="brief-audience">
                      <span className="desktop-shell__field-label">受众</span>
                      <textarea
                        id="brief-audience"
                        rows={4}
                        value={briefDraft.audience}
                        placeholder="核心人群、场景与认知阶段"
                        onChange={(event) =>
                          onBriefDraftChange({ ...briefDraft, audience: event.currentTarget.value })
                        }
                      />
                    </label>
                    <ListField
                      id="brief-deliverables"
                      label="交付物"
                      value={briefDraft.deliverables}
                      placeholder={"品牌片 1 条\n短视频 3 条\n横竖版封面"}
                      onChange={(deliverables) => onBriefDraftChange({ ...briefDraft, deliverables })}
                    />
                    <ListField
                      id="brief-style-keywords"
                      label="风格关键词"
                      value={briefDraft.styleKeywords}
                      placeholder={"克制\n真实质感\n明快节奏"}
                      onChange={(styleKeywords) => onBriefDraftChange({ ...briefDraft, styleKeywords })}
                    />
                    <ListField
                      id="brief-mandatory-items"
                      label="必含项"
                      value={briefDraft.mandatoryItems}
                      placeholder={"品牌 Logo\n核心产品\n行动引导"}
                      onChange={(mandatoryItems) => onBriefDraftChange({ ...briefDraft, mandatoryItems })}
                    />
                    <ListField
                      id="brief-constraints"
                      label="约束"
                      value={briefDraft.constraints}
                      placeholder={"拍摄档期\n场地限制\n合规要求"}
                      onChange={(constraints) => onBriefDraftChange({ ...briefDraft, constraints })}
                    />
                    <ListField
                      id="brief-risks"
                      label="风险"
                      value={briefDraft.risks}
                      placeholder={"客户方向未确认\n演员时间不稳定"}
                      onChange={(risks) => onBriefDraftChange({ ...briefDraft, risks })}
                    />
                    <label
                      className="desktop-shell__field desktop-shell__field--wide"
                      htmlFor="brief-reference-notes"
                    >
                      <span className="desktop-shell__field-label">参考备注</span>
                      <textarea
                        id="brief-reference-notes"
                        rows={4}
                        value={briefDraft.referenceNotes}
                        placeholder="案例、样片、资料位置与补充说明"
                        onChange={(event) =>
                          onBriefDraftChange({
                            ...briefDraft,
                            referenceNotes: event.currentTarget.value,
                          })
                        }
                      />
                    </label>
                  </div>
                  )}
                </section>
              </>
            ) : (
              <div className="desktop-shell__editor-empty">
                <span className="desktop-shell__empty-icon">
                  <BriefcaseBusiness size={30} strokeWidth={1.5} />
                </span>
                <h2>选择一个项目</h2>
                <p>当前未选中项目</p>
              </div>
            )}
          </div>
        </section>
          </>
        )}

        {activeSection === "creative" && (
          <CaseLibrary
            cases={cases}
            assets={assets}
            filters={caseFilters}
            viewMode={caseViewMode}
            editor={caseEditor}
            isLoading={isLoading || isRefreshingCases}
            isSaving={isSavingCase}
            error={error ? displayError(error) : null}
            onFiltersChange={onCaseFiltersChange}
            onViewModeChange={onCaseViewModeChange}
            onOpenCreate={onOpenCreateCase}
            onOpenEdit={onOpenEditCase}
            onEditorChange={onCaseEditorChange}
            onCloseEditor={onCloseCaseEditor}
            onSave={onSaveCase}
            onReload={onRefreshCases}
          />
        )}

        {activeSection === "workspace" && (
          <ExecutionBriefCenter
            projects={projects}
            briefs={executionBriefs}
            selectedProjectId={selectedProjectId}
            draft={executionBriefDraft}
            isRefreshing={isRefreshingExecutionBriefs}
            isSaving={isSavingExecutionBrief}
            error={error ? displayError(error) : null}
            onSelectProject={onSelectProject}
            onDraftChange={onExecutionBriefDraftChange}
            onCreate={onCreateExecutionBrief}
            onSave={onSaveExecutionBrief}
            onChangeStatus={onChangeExecutionBriefStatus}
            onRefresh={onRefreshExecutionBriefs}
          />
        )}

        {activeSection === "requirements" && (
          <RequirementBriefCenter
            projects={projects}
            briefs={requirementBriefs}
            cases={cases}
            selectedProjectId={selectedProjectId}
            draft={requirementBriefDraft}
            hasConflict={requirementBriefConflict}
            isRefreshing={isRefreshingRequirementBriefs}
            isSaving={isSavingRequirementBrief}
            error={error ? displayError(error) : null}
            onSelectProject={onSelectProject}
            onDraftChange={onRequirementBriefDraftChange}
            onCreate={onCreateRequirementBrief}
            onSave={onSaveRequirementBrief}
            onChangeStatus={onChangeRequirementBriefStatus}
            onRefresh={onRefreshRequirementBriefs}
            onReloadConflict={onReloadRequirementBrief}
            onRebaseConflict={onRebaseRequirementBrief}
          />
        )}

        {activeSection === "brain" && (
          <BrainCenter
            threads={brainThreads}
            turns={brainTurns}
            selectedThreadId={selectedBrainThreadId}
            projectNames={projectNames}
            models={brainModels}
            selectedModel={selectedBrainModel}
            draft={brainDraft}
            streamingDelta={brainStreamingDelta}
            isLoadingThreads={isLoading || isLoadingBrainThreads}
            isLoadingTurns={isLoadingBrainTurns}
            isStartingThread={isStartingBrainThread}
            isSending={isSendingBrainTurn}
            isDegraded={Boolean(
              brainHealth &&
                brainHealth.state !== "ready" &&
                brainHealth.state !== "stopped",
            )}
            degradedReason={brainHealth?.lastErrorCode ?? null}
            error={error ? displayError(error) : null}
            onSelectThread={onSelectBrainThread}
            onModelChange={onBrainModelChange}
            onDraftChange={onBrainDraftChange}
            onSend={onSendBrainTurn}
            onInterrupt={onInterruptBrainTurn}
            onNewThread={onNewBrainThread}
            onReload={onRefreshBrain}
          />
        )}

        {activeSection === "tasks" && (
          <TaskCenter
            tasks={tasks}
            statusFilter={taskStatusFilter}
            projectNames={projectNames}
            busyTaskIds={busyTaskIds}
            isLoading={isLoading || isRefreshingTasks}
            error={error ? displayError(error) : null}
            onStatusFilterChange={onTaskStatusFilterChange}
            onCancel={onCancelTask}
            onRetry={onRetryTask}
            onReload={onRefreshTasks}
          />
        )}

        {activeSection === "assets" && (
          <AssetVault
            assets={assets}
            projects={assetProjects}
            projectFilter={assetProjectFilter}
            viewMode={assetViewMode}
            selectedSource={selectedAssetSource}
            importProjectId={importProjectId}
            isLoading={isLoading || isRefreshingAssets}
            isSelectingSource={isSelectingAssetSource}
            isImporting={isImportingAsset}
            error={error ? displayError(error) : null}
            onProjectFilterChange={onAssetProjectFilterChange}
            onViewModeChange={onAssetViewModeChange}
            onChooseSource={onChooseAssetSource}
            onClearSource={onClearAssetSource}
            onImportProjectChange={onImportProjectChange}
            onImport={onImportAsset}
            onReload={onRefreshAssets}
          />
        )}
      </main>

      <aside className="desktop-shell__system" aria-label="系统状态">
        <header className="desktop-shell__system-header">
          <div>
            <span>LOCAL SERVICES</span>
            <h2>系统状态</h2>
          </div>
          <span
            className={
              hostStatus && codexStatus?.available && mediaHealth?.state === "ready"
                ? "desktop-shell__health is-ready"
                : "desktop-shell__health"
            }
          >
            {hostStatus
              ? `${3 + Number(Boolean(codexStatus?.available)) + Number(mediaHealth?.state === "ready")}/5`
              : "0/5"}
          </span>
        </header>

        <section className="desktop-shell__system-section" aria-labelledby="services-title">
          <div className="desktop-shell__aside-title">
            <h3 id="services-title">本地服务</h3>
            <span>{hostStatus?.protocolVersion ?? "--"}</span>
          </div>
          <div className="desktop-shell__status-list">
            <SystemStatusRow
              icon={Server}
              label="Rust Host"
              detail={hostStatus?.runtime ?? "等待连接"}
              tone={hostStatus ? "healthy" : "neutral"}
            />
            <SystemStatusRow
              icon={Database}
              label="SQLite"
              detail={hostStatus?.databaseReady ? `${hostStatus.projectCount} 个项目` : "未就绪"}
              tone={hostStatus?.databaseReady ? "healthy" : hostStatus ? "danger" : "neutral"}
            />
            <SystemStatusRow
              icon={HardDrive}
              label="Vault"
              detail={hostStatus?.vaultReady ? "本地媒体库可用" : "未就绪"}
              tone={hostStatus?.vaultReady ? "healthy" : hostStatus ? "danger" : "neutral"}
            />
            <SystemStatusRow
              icon={Bot}
              label="Codex app-server"
              detail={
                codexStatus?.available
                  ? codexStatus.transport || "stdio"
                  : codexStatus?.error
                    ? "握手失败"
                    : "等待检测"
              }
              tone={
                codexStatus?.available
                  ? "healthy"
                  : codexStatus?.error
                    ? "danger"
                    : "neutral"
              }
            />
            <SystemStatusRow
              icon={Activity}
              label="Native Media"
              detail={
                mediaHealth?.state === "ready"
                  ? "FFmpeg / ffprobe ready"
                  : mediaHealth?.state === "degraded"
                    ? "Partial capability"
                    : "Runtime unavailable"
              }
              tone={
                mediaHealth?.state === "ready"
                  ? "healthy"
                  : mediaHealth
                    ? "warning"
                    : "neutral"
              }
            />
          </div>
          <button
            type="button"
            className="desktop-shell__probe-button"
            disabled={isProbingCodex || !isDesktopRuntime}
            onClick={onProbeCodex}
          >
            <RefreshCw className={isProbingCodex ? "desktop-shell__spin" : undefined} size={16} />
            {isProbingCodex ? "正在握手" : "验证 Codex 握手"}
          </button>
          {codexStatus?.available && (
            <dl className="desktop-shell__codex-meta">
              <div>
                <dt>Runtime</dt>
                <dd>{codexStatus.runtime}</dd>
              </div>
              <div>
                <dt>Platform</dt>
                <dd>{codexStatus.platformOs ?? codexStatus.platformFamily ?? "--"}</dd>
              </div>
              <div>
                <dt>Home</dt>
                <dd>{codexStatus.codexHomeReady ? "Ready" : "Unavailable"}</dd>
              </div>
            </dl>
          )}
        </section>

        <section className="desktop-shell__system-section desktop-shell__events" aria-labelledby="events-title">
          <div className="desktop-shell__aside-title">
            <h3 id="events-title">最近事件</h3>
            <span>#{hostStatus?.lastEventSequence ?? 0}</span>
          </div>
          <div className="desktop-shell__event-list">
            {recentEvents.length > 0 ? (
              recentEvents.slice(-10).reverse().map((event) => (
                <article className="desktop-shell__event" key={event.eventId}>
                  <span className="desktop-shell__event-marker">
                    <CircleDot size={14} />
                  </span>
                  <div>
                    <strong>{EVENT_LABELS[event.eventType]}</strong>
                    <p>{event.project.name}</p>
                    <span>
                      #{event.sequence} · R{event.revision} · {formatDate(event.occurredAt)}
                    </span>
                  </div>
                </article>
              ))
            ) : (
              <div className="desktop-shell__event-empty">
                <Archive size={22} strokeWidth={1.5} />
                <span>暂无项目事件</span>
              </div>
            )}
          </div>
        </section>

        <footer className="desktop-shell__system-footer">
          <Network size={14} />
          <span>Desktop {packageMetadata.version}</span>
          <GalleryHorizontalEnd size={14} />
        </footer>
      </aside>
    </div>
  );
}

export default DesktopShell;
