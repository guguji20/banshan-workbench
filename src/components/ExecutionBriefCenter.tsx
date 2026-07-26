import { useEffect, useState, type ChangeEvent } from "react";
import {
  AlertCircle,
  CalendarClock,
  Check,
  CheckCircle2,
  ClipboardCheck,
  Clock3,
  FilePlus2,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  Save,
  ShieldAlert,
} from "lucide-react";
import type { ExecutionBriefContent } from "../generated/bsaigc/ExecutionBriefContent";
import type { ExecutionBriefRecord } from "../generated/bsaigc/ExecutionBriefRecord";
import type { ProjectRecord } from "../generated/bsaigc/ProjectRecord";
import type { ExecutionBriefStatus } from "../generated/bsaigc/ExecutionBriefStatus";
import "./ExecutionBriefCenter.css";

export interface ExecutionBriefCenterProps {
  projects: readonly ProjectRecord[];
  briefs: readonly ExecutionBriefRecord[];
  selectedProjectId: string | null;
  draft: ExecutionBriefContent | null;
  isRefreshing?: boolean;
  isSaving?: boolean;
  error?: string | null;
  onSelectProject: (projectId: string) => void;
  onDraftChange: (draft: ExecutionBriefContent) => void;
  onCreate: (projectId: string, draft: ExecutionBriefContent) => void;
  onSave: (record: ExecutionBriefRecord, draft: ExecutionBriefContent) => void;
  onChangeStatus: (
    record: ExecutionBriefRecord,
    status: ExecutionBriefStatus,
  ) => void;
  onRefresh: () => void;
}

const REQUIRED_SECTIONS: ReadonlyArray<{
  key: keyof ExecutionBriefContent;
  label: string;
}> = [
  { key: "shootAt", label: "拍摄时间" },
  { key: "clientGoal", label: "客户目标" },
  { key: "visualStyle", label: "画面风格" },
  { key: "primaryShots", label: "主镜头" },
  { key: "requiredShots", label: "必拍镜头" },
  { key: "riskPoints", label: "风险点" },
];

function isFilled(value: ExecutionBriefContent[keyof ExecutionBriefContent]): boolean {
  if (Array.isArray(value)) return value.length > 0;
  if (typeof value === "string") return value.trim().length > 0;
  return value !== null;
}

function toDateTimeLocal(value: number | null): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
}

function fromDateTimeLocal(value: string): number | null {
  if (!value) return null;
  const timestamp = new Date(value).getTime();
  return Number.isFinite(timestamp) ? timestamp : null;
}

function formatUpdatedAt(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

function lines(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function ListEditor({
  label,
  value,
  placeholder,
  onChange,
  prominent = false,
}: {
  label: string;
  value: readonly string[];
  placeholder: string;
  onChange: (value: string[]) => void;
  prominent?: boolean;
}) {
  // Keep the raw text locally so newlines and in-progress blank lines are not
  // stripped by the normalized value round-trip (otherwise Enter is swallowed
  // and multi-line entry is impossible).
  const [rawValue, setRawValue] = useState(value.join("\n"));
  const normalized = value.join("\n");
  useEffect(() => {
    setRawValue((current) => (lines(current).join("\n") === normalized ? current : normalized));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [normalized]);
  return (
    <label className={prominent ? "execution-brief__field is-prominent" : "execution-brief__field"}>
      <span>{label}</span>
      <textarea
        rows={prominent ? 5 : 4}
        value={rawValue}
        placeholder={placeholder}
        onChange={(event) => {
          setRawValue(event.currentTarget.value);
          onChange(lines(event.currentTarget.value));
        }}
      />
      <small>{value.length} 项</small>
    </label>
  );
}

export function ExecutionBriefCenter({
  projects,
  briefs,
  selectedProjectId,
  draft,
  isRefreshing = false,
  isSaving = false,
  error = null,
  onSelectProject,
  onDraftChange,
  onCreate,
  onSave,
  onChangeStatus,
  onRefresh,
}: ExecutionBriefCenterProps) {
  const selectedProject =
    projects.find((project) => project.id === selectedProjectId) ?? null;
  const record =
    briefs.find((brief) => brief.projectId === selectedProjectId) ?? null;
  const missing = draft
    ? REQUIRED_SECTIONS.filter(({ key }) => !isFilled(draft[key]))
    : REQUIRED_SECTIONS;
  const readyForStatus = missing.length === 0;

  function update<Key extends keyof ExecutionBriefContent>(
    key: Key,
    value: ExecutionBriefContent[Key],
  ) {
    if (draft) onDraftChange({ ...draft, [key]: value });
  }

  function updateDate(event: ChangeEvent<HTMLInputElement>) {
    update("shootAt", fromDateTimeLocal(event.currentTarget.value));
  }

  return (
    <section className="execution-brief" aria-labelledby="execution-brief-title">
      <header className="execution-brief__header">
        <div>
          <span>PRODUCTION HANDOFF</span>
          <h1 id="execution-brief-title">拍摄执行单</h1>
          <p>{briefs.length} 个项目已建执行单</p>
        </div>
        <button
          type="button"
          className="execution-brief__icon-button"
          onClick={onRefresh}
          disabled={isRefreshing}
          title="刷新执行单"
          aria-label="刷新执行单"
        >
          <RefreshCw size={16} className={isRefreshing ? "is-spinning" : undefined} />
        </button>
      </header>

      {error && (
        <div className="execution-brief__error" role="alert">
          <AlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}

      <div className="execution-brief__layout">
        <aside className="execution-brief__projects" aria-label="执行项目">
          <div className="execution-brief__rail-heading">
            <strong>项目</strong>
            <span>{projects.length}</span>
          </div>
          <div className="execution-brief__project-list">
            {projects.map((project) => {
              const brief = briefs.find((item) => item.projectId === project.id);
              return (
                <button
                  type="button"
                  key={project.id}
                  className={project.id === selectedProjectId ? "is-selected" : undefined}
                  onClick={() => onSelectProject(project.id)}
                >
                  <span>
                    <strong>{project.name}</strong>
                    <small>{project.clientName}</small>
                  </span>
                  <em className={brief?.status === "ready" ? "is-ready" : undefined}>
                    {brief ? (brief.status === "ready" ? "可执行" : "草稿") : "未创建"}
                  </em>
                </button>
              );
            })}
          </div>
        </aside>

        <div className="execution-brief__workspace">
          {!selectedProject || !draft ? (
            <div className="execution-brief__empty">
              <ClipboardCheck size={30} strokeWidth={1.5} />
              <strong>选择项目</strong>
              <span>当前没有可编辑的执行单</span>
            </div>
          ) : (
            <>
              <div className="execution-brief__toolbar">
                <div>
                  <span>{selectedProject.clientName}</span>
                  <h2>{selectedProject.name}</h2>
                </div>
                <div className="execution-brief__actions">
                  {record && (
                    <span className={`execution-brief__status is-${record.status}`}>
                      {record.status === "ready" ? <CheckCircle2 size={14} /> : <Clock3 size={14} />}
                      {record.status === "ready" ? "可执行" : "草稿"} · R{record.revision}
                    </span>
                  )}
                  {record ? (
                    <>
                      <button
                        type="button"
                        className="execution-brief__secondary-button"
                        disabled={
                          isSaving ||
                          (record.status !== "ready" && !readyForStatus)
                        }
                        onClick={() =>
                          onChangeStatus(
                            record,
                            record.status === "ready" ? "draft" : "ready",
                          )
                        }
                      >
                        {record.status === "ready" ? <RotateCcw size={15} /> : <Check size={15} />}
                        {record.status === "ready" ? "退回草稿" : "确认可执行"}
                      </button>
                      <button
                        type="button"
                        className="execution-brief__primary-button"
                        disabled={isSaving}
                        onClick={() => onSave(record, draft)}
                      >
                        {isSaving ? <LoaderCircle size={15} className="is-spinning" /> : <Save size={15} />}
                        保存
                      </button>
                    </>
                  ) : (
                    <button
                      type="button"
                      className="execution-brief__primary-button"
                      disabled={isSaving}
                      onClick={() => onCreate(selectedProject.id, draft)}
                    >
                      {isSaving ? <LoaderCircle size={15} className="is-spinning" /> : <FilePlus2 size={15} />}
                      创建执行单
                    </button>
                  )}
                </div>
              </div>

              <div className="execution-brief__readiness" aria-label="执行完整度">
                <div className="execution-brief__readiness-title">
                  <ShieldAlert size={16} />
                  <strong>拍前确认</strong>
                  <span>{REQUIRED_SECTIONS.length - missing.length}/{REQUIRED_SECTIONS.length}</span>
                </div>
                <div className="execution-brief__checks">
                  {REQUIRED_SECTIONS.map(({ key, label }) => {
                    const filled = isFilled(draft[key]);
                    return (
                      <span key={key} className={filled ? "is-complete" : undefined}>
                        {filled ? <Check size={12} /> : <span />}
                        {label}
                      </span>
                    );
                  })}
                </div>
                {!readyForStatus && record?.status !== "ready" && (
                  <small>仍有 {missing.length} 项待确认</small>
                )}
              </div>

              <div className="execution-brief__form">
                <label className="execution-brief__field">
                  <span>拍摄时间</span>
                  <div className="execution-brief__date-input">
                    <CalendarClock size={16} />
                    <input
                      type="datetime-local"
                      value={toDateTimeLocal(draft.shootAt)}
                      onChange={updateDate}
                    />
                  </div>
                </label>
                <label className="execution-brief__field">
                  <span>客户目标</span>
                  <textarea
                    rows={4}
                    value={draft.clientGoal}
                    placeholder="本次成片必须解决的核心目标"
                    onChange={(event) => update("clientGoal", event.currentTarget.value)}
                  />
                </label>
                <label className="execution-brief__field">
                  <span>画面风格</span>
                  <textarea
                    rows={4}
                    value={draft.visualStyle}
                    placeholder="构图、光线、色彩、节奏和参考方向"
                    onChange={(event) => update("visualStyle", event.currentTarget.value)}
                  />
                </label>
                <ListEditor
                  label="主镜头"
                  value={draft.primaryShots}
                  placeholder={"空间主叙事镜头\n核心人物动作"}
                  prominent
                  onChange={(value) => update("primaryShots", value)}
                />
                <ListEditor
                  label="次镜头"
                  value={draft.secondaryShots}
                  placeholder={"环境关系\n材质与氛围补充"}
                  onChange={(value) => update("secondaryShots", value)}
                />
                <ListEditor
                  label="必拍镜头"
                  value={draft.requiredShots}
                  placeholder={"品牌标识\n核心产品\n指定人物"}
                  prominent
                  onChange={(value) => update("requiredShots", value)}
                />
                <ListEditor
                  label="可替代镜头"
                  value={draft.fallbackShots}
                  placeholder={"下雨时改室内\n人物缺席时的替代画面"}
                  onChange={(value) => update("fallbackShots", value)}
                />
                <ListEditor
                  label="风险点"
                  value={draft.riskPoints}
                  placeholder={"天气\n场地\n演员时间\n客户临时变更"}
                  prominent
                  onChange={(value) => update("riskPoints", value)}
                />
                <ListEditor
                  label="等待时间利用"
                  value={draft.waitingTimeActions}
                  placeholder={"看景与备选机位\n灯位与构图\n演员动作沟通"}
                  onChange={(value) => update("waitingTimeActions", value)}
                />
                <label className="execution-brief__field">
                  <span>器材与现场备注</span>
                  <textarea
                    rows={4}
                    value={draft.equipmentNotes}
                    placeholder="镜头、灯光、收音、运动设备和现场限制"
                    onChange={(event) => update("equipmentNotes", event.currentTarget.value)}
                  />
                </label>
                <ListEditor
                  label="拍后素材亮点"
                  value={draft.postShootHighlights}
                  placeholder={"重点设计镜头\n摄影认为必须使用的素材"}
                  onChange={(value) => update("postShootHighlights", value)}
                />
              </div>

              {record && (
                <footer className="execution-brief__footer">
                  <span>最近更新 {formatUpdatedAt(record.updatedAt)}</span>
                  <span>{record.id.slice(0, 8)}</span>
                </footer>
              )}
            </>
          )}
        </div>
      </div>
    </section>
  );
}
