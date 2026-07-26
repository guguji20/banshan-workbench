import { useLayoutEffect, useRef } from "react";
import type { FormEvent, KeyboardEvent } from "react";
import {
  AlertCircle,
  Bot,
  Circle,
  LoaderCircle,
  MessageSquareText,
  Paperclip,
  Plus,
  RefreshCw,
  Send,
  Square,
  WifiOff,
} from "lucide-react";
import type { BrainThreadRecord } from "../generated/bsaigc/BrainThreadRecord";
import type { BrainThreadStatus } from "../generated/bsaigc/BrainThreadStatus";
import type { BrainTurnRecord } from "../generated/bsaigc/BrainTurnRecord";
import type { BrainTurnStatus } from "../generated/bsaigc/BrainTurnStatus";
import {
  friendlyBrainThreadTitle,
  presentBrainTurn,
  presentOrphanBrainStreaming,
} from "./brainPresentation";
import {
  isBrainScrollNearBottom,
  shouldFollowBrainScroll,
  type BrainScrollReason,
} from "./brainScroll";
import "./BrainCenter.css";

export interface BrainModelOption {
  id: string;
  label: string;
  available?: boolean;
}

export interface BrainCenterProps {
  threads: readonly BrainThreadRecord[];
  turns: readonly BrainTurnRecord[];
  selectedThreadId: string | null;
  projectNames?: Readonly<Record<string, string>>;
  models: readonly BrainModelOption[];
  selectedModel: string;
  draft: string;
  streamingDelta?: string;
  isLoadingThreads?: boolean;
  isLoadingTurns?: boolean;
  isStartingThread?: boolean;
  isSending?: boolean;
  isDegraded?: boolean;
  degradedReason?: string | null;
  error?: string | null;
  showThreadList?: boolean;
  onSelectThread: (threadId: string) => void;
  onModelChange: (modelId: string) => void;
  onDraftChange: (value: string) => void;
  onSend: () => void;
  onInterrupt: () => void;
  onAttach?: () => void;
  onNewThread: () => void;
  onReload?: () => void;
}

const THREAD_STATUS: Record<
  BrainThreadStatus,
  { label: string; tone: string }
> = {
  ready: { label: "就绪", tone: "ready" },
  running: { label: "运行中", tone: "running" },
  error: { label: "异常", tone: "error" },
  archived: { label: "已归档", tone: "archived" },
};

const TURN_STATUS: Record<BrainTurnStatus, string> = {
  running: "生成中",
  completed: "已完成",
  interrupted: "已中断",
  failed: "失败",
};

function formatTime(value: number): string {
  const milliseconds = value < 10_000_000_000 ? value * 1000 : value;
  const date = new Date(milliseconds);
  if (Number.isNaN(date.getTime())) return "--";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}

function dateTimeValue(value: number): string | undefined {
  const milliseconds = value < 10_000_000_000 ? value * 1000 : value;
  const date = new Date(milliseconds);
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString();
}

function projectLabel(
  thread: BrainThreadRecord,
  projectNames: Readonly<Record<string, string>>,
): string {
  if (!thread.projectId) return "未关联项目";
  return projectNames[thread.projectId] ?? "关联项目";
}

function ThreadStatusBadge({ status }: { status: BrainThreadStatus }) {
  const meta = THREAD_STATUS[status];
  return (
    <span className={`brain-center__thread-status brain-center__thread-status--${meta.tone}`}>
      {status === "running" ? (
        <LoaderCircle size={11} className="brain-center__spin" aria-hidden="true" />
      ) : (
        <Circle size={7} fill="currentColor" aria-hidden="true" />
      )}
      {meta.label}
    </span>
  );
}

function TurnStatusBadge({ status }: { status: BrainTurnStatus }) {
  return (
    <span className={`brain-center__turn-status brain-center__turn-status--${status}`}>
      {status === "running" && (
        <LoaderCircle size={11} className="brain-center__spin" aria-hidden="true" />
      )}
      {TURN_STATUS[status]}
    </span>
  );
}

export function BrainCenter({
  threads,
  turns,
  selectedThreadId,
  projectNames = {},
  models,
  selectedModel,
  draft,
  streamingDelta = "",
  isLoadingThreads = false,
  isLoadingTurns = false,
  isStartingThread = false,
  isSending = false,
  isDegraded = false,
  degradedReason = null,
  error = null,
  showThreadList = true,
  onSelectThread,
  onModelChange,
  onDraftChange,
  onSend,
  onInterrupt,
  onAttach,
  onNewThread,
  onReload,
}: BrainCenterProps) {
  const historyRef = useRef<HTMLDivElement>(null);
  const followsLatestRef = useRef(true);
  const previousThreadIdRef = useRef<string | null | undefined>(undefined);
  const selectedThread =
    threads.find((thread) => thread.id === selectedThreadId) ?? null;
  const selectedTurns = selectedThreadId
    ? turns.filter((turn) => turn.threadId === selectedThreadId)
    : [];
  const latestSelectedTurn = selectedTurns[selectedTurns.length - 1] ?? null;
  const isRunning =
    isSending ||
    selectedThread?.status === "running" ||
    selectedTurns.some((turn) => turn.status === "running");
  const streamingTurnId = [...selectedTurns]
    .reverse()
    .find((turn) => turn.status === "running")?.id;
  const selectedThreadIsInternal =
    selectedTurns.some((turn) => presentBrainTurn(turn).internal) ||
    friendlyBrainThreadTitle(selectedThread?.title) === "合同审查";
  const orphanStreaming = streamingTurnId
    ? null
    : presentOrphanBrainStreaming(streamingDelta, selectedThreadIsInternal);
  const canSend =
    !isRunning &&
    !isDegraded &&
    !isStartingThread &&
    draft.trim().length > 0 &&
    selectedModel.length > 0;

  function scrollHistoryToBottom(reason: BrainScrollReason) {
    if (!shouldFollowBrainScroll(reason, followsLatestRef.current)) return;

    followsLatestRef.current = true;
    const history = historyRef.current;
    if (history) history.scrollTop = history.scrollHeight;
  }

  function handleHistoryScroll() {
    const history = historyRef.current;
    if (!history) return;

    followsLatestRef.current = isBrainScrollNearBottom({
      scrollTop: history.scrollTop,
      scrollHeight: history.scrollHeight,
      clientHeight: history.clientHeight,
    });
  }

  function sendCurrentDraft() {
    if (!canSend) return;
    scrollHistoryToBottom("user-send");
    onSend();
  }

  useLayoutEffect(() => {
    const threadChanged = previousThreadIdRef.current !== selectedThreadId;
    previousThreadIdRef.current = selectedThreadId;
    scrollHistoryToBottom(threadChanged ? "thread-switch" : "content-update");
  }, [
    selectedThreadId,
    selectedTurns.length,
    latestSelectedTurn?.id,
    latestSelectedTurn?.status,
    latestSelectedTurn?.inputText,
    latestSelectedTurn?.assistantText,
    latestSelectedTurn?.error,
    latestSelectedTurn?.updatedAt,
    streamingDelta,
    isLoadingTurns,
  ]);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    sendCurrentDraft();
  }

  function handleDraftKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (
      event.key === "Enter" &&
      !event.shiftKey &&
      !event.nativeEvent.isComposing &&
      canSend
    ) {
      event.preventDefault();
      sendCurrentDraft();
    }
  }

  return (
    <section
      className={`brain-center${showThreadList ? "" : " brain-center--conversation-only"}`}
      aria-labelledby={showThreadList ? "brain-center-title" : undefined}
      aria-label={showThreadList ? undefined : "商务助手对话"}
    >
      {showThreadList && (
      <aside className="brain-center__threads" aria-label="对话列表">
        <header className="brain-center__threads-header">
          <div>
            <span>商务助手</span>
            <h1 id="brain-center-title">对话</h1>
          </div>
          <div className="brain-center__header-actions">
            {onReload && (
              <button
                type="button"
                className="brain-center__icon-button"
                onClick={onReload}
                disabled={isLoadingThreads}
                title="刷新对话"
                aria-label="刷新对话"
              >
                <RefreshCw
                  size={16}
                  className={isLoadingThreads ? "brain-center__spin" : undefined}
                />
              </button>
            )}
            <button
              type="button"
              className="brain-center__icon-button brain-center__icon-button--accent"
              onClick={onNewThread}
              disabled={isStartingThread || isDegraded}
              title="新建对话"
              aria-label="新建对话"
            >
              {isStartingThread ? (
                <LoaderCircle size={16} className="brain-center__spin" />
              ) : (
                <Plus size={17} />
              )}
            </button>
          </div>
        </header>

        <div className="brain-center__thread-count">
          <MessageSquareText size={13} aria-hidden="true" />
          <span>{threads.length} 个对话</span>
        </div>

        <div className="brain-center__thread-list" aria-busy={isLoadingThreads}>
          {isLoadingThreads && threads.length === 0 ? (
            <div className="brain-center__rail-state">
              <LoaderCircle size={19} className="brain-center__spin" aria-hidden="true" />
              <span>正在读取对话</span>
            </div>
          ) : threads.length === 0 ? (
            <div className="brain-center__rail-state">
              <MessageSquareText size={19} aria-hidden="true" />
              <strong>暂无对话</strong>
              <span>新建对话后开始协作</span>
            </div>
          ) : (
            threads.map((thread) => {
              const active = thread.id === selectedThreadId;
              const title = friendlyBrainThreadTitle(thread.title);
              const project = projectLabel(thread, projectNames);
              return (
                <button
                  key={thread.id}
                  type="button"
                  className={`brain-center__thread${active ? " is-active" : ""}`}
                  onClick={() => onSelectThread(thread.id)}
                  aria-pressed={active}
                  title={`${title} · ${project}`}
                >
                  <span className="brain-center__thread-title-row">
                    <strong>{title}</strong>
                    <ThreadStatusBadge status={thread.status} />
                  </span>
                  <span className="brain-center__thread-project">{project}</span>
                  <span className="brain-center__thread-meta">
                    <span>{thread.model || "默认模型"}</span>
                    <time dateTime={dateTimeValue(thread.updatedAt)}>
                      {formatTime(thread.updatedAt)}
                    </time>
                  </span>
                </button>
              );
            })
          )}
        </div>
      </aside>
      )}

      <div className="brain-center__conversation">
        <header className="brain-center__conversation-header">
          <div className="brain-center__conversation-title">
            <Bot size={18} aria-hidden="true" />
            <div>
              <strong>{selectedThread ? friendlyBrainThreadTitle(selectedThread.title) : "新对话"}</strong>
              <span>
                {selectedThread
                  ? projectLabel(selectedThread, projectNames)
                  : "选择或新建对话"}
              </span>
            </div>
          </div>
          {selectedThread && <ThreadStatusBadge status={selectedThread.status} />}
        </header>

        {isDegraded && (
          <div className="brain-center__notice brain-center__notice--degraded" role="status">
            <WifiOff size={16} aria-hidden="true" />
            <div>
              <strong>智能助手暂不可用</strong>
              <span>{degradedReason || "历史记录仍可查看，发送功能已暂停。"}</span>
            </div>
          </div>
        )}

        {error && (
          <div className="brain-center__notice brain-center__notice--error" role="alert">
            <AlertCircle size={16} aria-hidden="true" />
            <div>
              <strong>操作未完成</strong>
              <span>{error}</span>
            </div>
          </div>
        )}

        <div
          ref={historyRef}
          className="brain-center__history"
          onScroll={handleHistoryScroll}
          role="log"
          aria-label="对话历史"
          aria-busy={isLoadingTurns}
          aria-live="polite"
          aria-relevant="additions text"
        >
          {!selectedThread ? (
            <div className="brain-center__empty">
              <strong>开始一项商务任务</strong>
            </div>
          ) : isLoadingTurns && selectedTurns.length === 0 ? (
            <div className="brain-center__empty">
              <LoaderCircle size={22} className="brain-center__spin" aria-hidden="true" />
              <strong>正在读取历史</strong>
            </div>
          ) : selectedTurns.length === 0 && !streamingDelta ? (
            <div className="brain-center__empty">
              <strong>开始一项商务任务</strong>
            </div>
          ) : (
            <div className="brain-center__turns">
              {selectedTurns.map((turn) => {
                const turnDelta = turn.id === streamingTurnId ? streamingDelta : "";
                const presentation = presentBrainTurn(turn, turnDelta);
                const statusText =
                  presentation.statusText ??
                  (turn.status === "running" ? "正在处理" : "本轮已结束");
                return (
                  <article
                    className={`brain-center__turn${presentation.internal ? " brain-center__turn--internal" : ""}`}
                    key={turn.id}
                  >
                    {presentation.userText && (
                      <div className="brain-center__message brain-center__message--user">
                        <div className="brain-center__message-meta">
                          <strong>你</strong>
                          <time>{formatTime(turn.createdAt)}</time>
                        </div>
                        <p>{presentation.userText}</p>
                      </div>
                    )}
                    <div className="brain-center__message brain-center__message--assistant">
                      <div className="brain-center__message-meta">
                        <strong>助手</strong>
                        {turn.status !== "completed" && (
                          <TurnStatusBadge status={turn.status} />
                        )}
                      </div>
                      {presentation.assistantText ? (
                        <p>{presentation.assistantText}</p>
                      ) : presentation.isStreaming ? (
                        <div className="brain-center__thinking">
                          <LoaderCircle size={14} className="brain-center__spin" />
                          <span>{statusText}</span>
                        </div>
                      ) : (
                        <p className="brain-center__muted-output">{statusText}</p>
                      )}
                      {presentation.errorText && (
                        <div className="brain-center__turn-error" role="alert">
                          <AlertCircle size={13} aria-hidden="true" />
                          <span>{presentation.errorText}</span>
                        </div>
                      )}
                      {turnDelta && (
                        <span className="brain-center__cursor" aria-hidden="true" />
                      )}
                    </div>
                  </article>
                );
              })}

              {orphanStreaming && (
                <article className="brain-center__turn brain-center__turn--streaming">
                  <div className="brain-center__message brain-center__message--assistant">
                    <div className="brain-center__message-meta">
                      <strong>助手</strong>
                      <TurnStatusBadge status="running" />
                    </div>
                    {orphanStreaming.text ? (
                      <p>{orphanStreaming.text}</p>
                    ) : (
                      <div className="brain-center__thinking">
                        <LoaderCircle size={14} className="brain-center__spin" />
                        <span>{orphanStreaming.statusText}</span>
                      </div>
                    )}
                    <span className="brain-center__cursor" aria-hidden="true" />
                  </div>
                </article>
              )}
            </div>
          )}
        </div>

        <form className="brain-center__composer" onSubmit={submit}>
          <div className="brain-center__composer-toolbar">
            <div className="brain-center__composer-tools">
              {onAttach && (
                <button
                  type="button"
                  className="brain-center__attachment-button"
                  onClick={onAttach}
                  disabled={isRunning || isDegraded}
                  title="添加附件"
                  aria-label="添加附件"
                >
                  <Paperclip size={15} />
                </button>
              )}
              <label className="brain-center__model-control">
                <span>模型</span>
                <select
                  value={selectedModel}
                  onChange={(event) => onModelChange(event.currentTarget.value)}
                  disabled={isRunning || isDegraded || models.length === 0}
                  aria-label="选择模型"
                >
                  {models.length === 0 && <option value="">暂无可用模型</option>}
                  {models.map((model) => (
                    <option
                      key={model.id}
                      value={model.id}
                      disabled={model.available === false}
                    >
                      {model.label}{model.available === false ? "（不可用）" : ""}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <span className="brain-center__draft-count">{draft.length}</span>
          </div>
          <div className="brain-center__composer-main">
            <textarea
              value={draft}
              onChange={(event) => onDraftChange(event.currentTarget.value)}
              onKeyDown={handleDraftKeyDown}
              disabled={isDegraded}
              placeholder={isDegraded ? "智能助手暂不可用" : "输入消息或任务"}
              rows={3}
              aria-label="消息内容"
            />
            {isRunning ? (
              <button
                type="button"
                className="brain-center__send-button brain-center__send-button--stop"
                onClick={onInterrupt}
                disabled={!selectedTurns.some((turn) => turn.status === "running")}
                title={
                  selectedTurns.some((turn) => turn.status === "running")
                    ? "中断当前回复"
                    : "当前没有可中断的回复，请稍候或新建对话"
                }
                aria-label="中断当前回复"
              >
                <Square size={15} fill="currentColor" />
              </button>
            ) : (
              <button
                type="submit"
                className="brain-center__send-button"
                disabled={!canSend}
                title="发送消息"
                aria-label="发送消息"
              >
                <Send size={16} />
              </button>
            )}
          </div>
        </form>
      </div>
    </section>
  );
}
