import type { CSSProperties } from "react";
import {
  AlertCircle,
  Ban,
  CheckCircle2,
  CirclePause,
  Clock3,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  XCircle,
} from "lucide-react";
import type { TaskPriority } from "../generated/bsaigc/TaskPriority";
import type { TaskRecord } from "../generated/bsaigc/TaskRecord";
import type { TaskReplayPolicy } from "../generated/bsaigc/TaskReplayPolicy";
import type { TaskStatus } from "../generated/bsaigc/TaskStatus";
import "./TaskCenter.css";

export type TaskStatusFilter = "all" | TaskStatus;

export interface TaskCenterProps {
  tasks: readonly TaskRecord[];
  statusFilter: TaskStatusFilter;
  projectNames?: Readonly<Record<string, string>>;
  busyTaskIds?: readonly string[];
  isLoading?: boolean;
  error?: string | null;
  onStatusFilterChange: (status: TaskStatusFilter) => void;
  onCancel: (task: TaskRecord) => void;
  onRetry: (task: TaskRecord, approved: boolean) => void;
  onReload?: () => void;
}

const STATUS_META: Record<
  TaskStatus,
  { label: string; tone: string }
> = {
  queued: { label: "排队中", tone: "queued" },
  running: { label: "执行中", tone: "running" },
  succeeded: { label: "已完成", tone: "succeeded" },
  failed: { label: "失败", tone: "failed" },
  canceled: { label: "已取消", tone: "canceled" },
  awaitingApproval: { label: "待审批", tone: "approval" },
};

const PRIORITY_LABELS: Record<TaskPriority, string> = {
  low: "低",
  normal: "普通",
  high: "高",
  critical: "紧急",
};

const REPLAY_META: Record<
  TaskReplayPolicy,
  { label: string; detail: string }
> = {
  safe: { label: "安全重放", detail: "中断后可自动恢复" },
  manual: { label: "人工审批", detail: "批准后才可重新执行" },
  never: { label: "禁止自动重放", detail: "仅限人工强制批准" },
};

const FILTERS: ReadonlyArray<{ id: TaskStatusFilter; label: string }> = [
  { id: "all", label: "全部" },
  { id: "queued", label: "排队中" },
  { id: "running", label: "执行中" },
  { id: "awaitingApproval", label: "待审批" },
  { id: "failed", label: "失败" },
  { id: "succeeded", label: "已完成" },
  { id: "canceled", label: "已取消" },
];

function clampProgress(progress: number): number {
  if (!Number.isFinite(progress)) return 0;
  return Math.min(100, Math.max(0, progress));
}


function canCancel(status: TaskStatus): boolean {
  return (
    status === "queued" ||
    status === "running" ||
    status === "awaitingApproval"
  );
}

function canRetry(status: TaskStatus): boolean {
  return (
    status === "failed" ||
    status === "canceled" ||
    status === "awaitingApproval"
  );
}

function retryLabel(task: TaskRecord): string {
  if (task.replayPolicy === "manual") return "批准并重试";
  if (task.replayPolicy === "never") return "强制批准重试";
  return task.status === "awaitingApproval" ? "批准重试" : "重试";
}

function TaskStatusBadge({ status }: { status: TaskStatus }) {
  const meta = STATUS_META[status];
  return (
    <span className={`task-center__status task-center__status--${meta.tone}`}>
      {status === "running" && (
        <LoaderCircle size={13} className="task-center__spin" aria-hidden="true" />
      )}
      {status === "awaitingApproval" && (
        <CirclePause size={13} aria-hidden="true" />
      )}
      {meta.label}
    </span>
  );
}

export function TaskCenter({
  tasks,
  statusFilter,
  projectNames = {},
  busyTaskIds = [],
  isLoading = false,
  error = null,
  onStatusFilterChange,
  onCancel,
  onRetry,
  onReload,
}: TaskCenterProps) {
  const busyIds = new Set(busyTaskIds);
  const filteredTasks =
    statusFilter === "all"
      ? tasks
      : tasks.filter((task) => task.status === statusFilter);
  const counts = tasks.reduce(
    (result, task) => {
      if (task.status in result) {
        result[task.status as keyof typeof result] += 1;
      }
      return result;
    },
    { queued: 0, running: 0, awaitingApproval: 0, failed: 0 },
  );

  return (
    <section className="task-center" aria-labelledby="task-center-title">
      <header className="task-center__header">
        <div className="task-center__heading">
          <span className="task-center__eyebrow">后台执行</span>
          <h1 id="task-center-title">任务中心</h1>
        </div>
        {onReload && (
          <button
            type="button"
            className="task-center__icon-button"
            onClick={onReload}
            disabled={isLoading}
            title="刷新任务"
            aria-label="刷新任务"
          >
            <RefreshCw
              size={17}
              className={isLoading ? "task-center__spin" : undefined}
            />
          </button>
        )}
      </header>

      <div className="task-center__stats" aria-label="任务统计">
        <div className="task-center__stat task-center__stat--queued">
          <Clock3 size={17} aria-hidden="true" />
          <span>排队中</span>
          <strong>{counts.queued}</strong>
        </div>
        <div className="task-center__stat task-center__stat--running">
          <LoaderCircle size={17} aria-hidden="true" />
          <span>执行中</span>
          <strong>{counts.running}</strong>
        </div>
        <div className="task-center__stat task-center__stat--approval">
          <ShieldAlert size={17} aria-hidden="true" />
          <span>待审批</span>
          <strong>{counts.awaitingApproval}</strong>
        </div>
        <div className="task-center__stat task-center__stat--failed">
          <AlertCircle size={17} aria-hidden="true" />
          <span>失败</span>
          <strong>{counts.failed}</strong>
        </div>
      </div>

      {error && (
        <div className="task-center__error" role="alert">
          <AlertCircle size={17} aria-hidden="true" />
          <span>{error}</span>
          {onReload && (
            <button type="button" onClick={onReload} disabled={isLoading}>
              重试加载
            </button>
          )}
        </div>
      )}

      <div className="task-center__toolbar">
        <div className="task-center__filters" aria-label="按任务状态筛选">
          {FILTERS.map((filter) => (
            <button
              key={filter.id}
              type="button"
              className={statusFilter === filter.id ? "is-active" : undefined}
              aria-pressed={statusFilter === filter.id}
              onClick={() => onStatusFilterChange(filter.id)}
            >
              {filter.label}
            </button>
          ))}
        </div>
        <span className="task-center__result-count">
          {filteredTasks.length} 个任务
        </span>
      </div>

      <div className="task-center__table" aria-busy={isLoading}>
        <div className="task-center__table-head" aria-hidden="true">
          <span>任务</span>
          <span>状态与进度</span>
          <span>执行策略</span>
          <span>操作</span>
        </div>

        {isLoading && tasks.length === 0 ? (
          <div className="task-center__state">
            <LoaderCircle size={22} className="task-center__spin" aria-hidden="true" />
            <strong>正在读取任务</strong>
            <span>任务状态由本地 Host 提供</span>
          </div>
        ) : filteredTasks.length === 0 ? (
          <div className="task-center__state">
            <CheckCircle2 size={22} aria-hidden="true" />
            <strong>{tasks.length === 0 ? "暂无后台任务" : "当前筛选没有任务"}</strong>
            <span>
              {tasks.length === 0
                ? "生成、媒体处理和 Agent 工作会统一显示在这里"
                : "切换状态筛选查看其他任务"}
            </span>
          </div>
        ) : (
          <div className="task-center__rows">
            {filteredTasks.map((task) => {
              const progress = clampProgress(task.progress);
              const replay = REPLAY_META[task.replayPolicy];
              const isBusy = busyIds.has(task.id);
              const projectLabel = task.projectId
                ? projectNames[task.projectId] ?? "关联项目"
                : "未关联项目";
              const progressStyle = {
                "--task-progress": `${progress}%`,
              } as CSSProperties;

              return (
                <article className="task-center__row" key={task.id}>
                  <div className="task-center__identity">
                    <div className="task-center__kind-line">
                      <strong title={task.kind}>{task.kind}</strong>
                      <span
                        className={`task-center__priority task-center__priority--${task.priority}`}
                      >
                        {PRIORITY_LABELS[task.priority]}
                      </span>
                    </div>
                    <span className="task-center__project" title={projectLabel}>
                      {projectLabel}
                    </span>

                  </div>

                  <div className="task-center__execution">
                    <div className="task-center__execution-line">
                      <TaskStatusBadge status={task.status} />
                      <span>{progress}%</span>
                    </div>
                    <div
                      className="task-center__progress"
                      style={progressStyle}
                      role="progressbar"
                      aria-label={`${task.kind} 进度`}
                      aria-valuemin={0}
                      aria-valuemax={100}
                      aria-valuenow={progress}
                    >
                      <span />
                    </div>
                    <span className="task-center__technical">
                      第 {task.attempt}/{task.maxAttempts} 次尝试
                    </span>
                    {task.lastError && (
                      <span className="task-center__last-error" title={task.lastError}>
                        <XCircle size={13} aria-hidden="true" />
                        {task.lastError}
                      </span>
                    )}
                  </div>

                  <div className="task-center__policy">
                    <span
                      className={`task-center__policy-name task-center__policy-name--${task.replayPolicy}`}
                    >
                      {task.replayPolicy !== "safe" && (
                        <ShieldAlert size={13} aria-hidden="true" />
                      )}
                      {replay.label}
                    </span>
                    <span>{replay.detail}</span>
                  </div>

                  <div className="task-center__actions">
                    {canRetry(task.status) && (
                      <button
                        type="button"
                        className={
                          task.replayPolicy === "safe"
                            ? "task-center__action"
                            : "task-center__action task-center__action--approval"
                        }
                        onClick={() => onRetry(task, true)}
                        disabled={isBusy}
                        title={retryLabel(task)}
                      >
                        {isBusy ? (
                          <LoaderCircle size={15} className="task-center__spin" />
                        ) : (
                          <RotateCcw size={15} />
                        )}
                        <span>{retryLabel(task)}</span>
                      </button>
                    )}
                    {canCancel(task.status) && (
                      <button
                        type="button"
                        className="task-center__action task-center__action--cancel"
                        onClick={() => onCancel(task)}
                        disabled={isBusy}
                        title="取消任务"
                      >
                        {isBusy ? (
                          <LoaderCircle size={15} className="task-center__spin" />
                        ) : (
                          <Ban size={15} />
                        )}
                        <span>取消</span>
                      </button>
                    )}
                    {!canRetry(task.status) && !canCancel(task.status) && (
                      <span className="task-center__settled">无需操作</span>
                    )}
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}

