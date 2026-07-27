import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ClipboardEvent, FormEvent, KeyboardEvent } from "react";
import {
  AlertCircle,
  Bot,
  Circle,
  FileImage,
  FileText,
  Folder,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  MessageSquareText,
  Plus,
  RefreshCw,
  Send,
  ShieldAlert,
  ShieldCheck,
  ShieldQuestion,
  Square,
  X,
  WifiOff,
} from "lucide-react";
import type { AssetKind } from "../generated/bsaigc/AssetKind";
import type { BrainAccessMode } from "../generated/bsaigc/BrainAccessMode";
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

export interface BrainAttachment {
  assetId: string;
  displayName: string;
  kind: AssetKind;
  mimeType: string;
  sizeBytes: number;
  previewUrl: string | null;
}

export interface BrainWorkspace {
  workspaceToken: string;
  displayName: string;
}

export interface BrainCenterProps {
  threads: readonly BrainThreadRecord[];
  turns: readonly BrainTurnRecord[];
  selectedThreadId: string | null;
  projectNames?: Readonly<Record<string, string>>;
  models: readonly BrainModelOption[];
  selectedModel: string;
  draft: string;
  attachments?: readonly BrainAttachment[];
  workspace?: BrainWorkspace | null;
  accessMode?: BrainAccessMode;
  streamingDelta?: string;
  isLoadingThreads?: boolean;
  isLoadingTurns?: boolean;
  isStartingThread?: boolean;
  isSending?: boolean;
  isAttaching?: boolean;
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
  onRemoveAttachment?: (assetId: string) => void;
  onSelectWorkspace?: () => void;
  onClearWorkspace?: () => void;
  onAccessModeChange?: (mode: BrainAccessMode) => void;
  onDropPaths?: (paths: string[]) => void;
  onPasteImages?: (files: File[]) => void;
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

const ACCESS_OPTIONS: ReadonlyArray<{
  mode: BrainAccessMode;
  label: string;
  detail: string;
}> = [
  {
    mode: "requestApproval",
    label: "请求批准",
    detail: "编辑工作区和联网操作会先询问",
  },
  {
    mode: "autoApprove",
    label: "替我审批",
    detail: "仅检测到风险操作时请求批准",
  },
  {
    mode: "fullAccess",
    label: "完全访问",
    detail: "可不受限制地访问电脑文件和互联网",
  },
];

function formatFileSize(sizeBytes: number): string {
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  if (sizeBytes < 1024 * 1024) return `${Math.round(sizeBytes / 1024)} KB`;
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

function AttachmentIcon({ kind }: { kind: AssetKind }) {
  if (kind === "image") return <FileImage size={17} aria-hidden="true" />;
  if (kind === "document") return <FileText size={17} aria-hidden="true" />;
  return <HardDrive size={17} aria-hidden="true" />;
}

function AccessIcon({ mode }: { mode: BrainAccessMode }) {
  if (mode === "fullAccess") return <ShieldAlert size={14} aria-hidden="true" />;
  if (mode === "autoApprove") return <ShieldCheck size={14} aria-hidden="true" />;
  return <ShieldQuestion size={14} aria-hidden="true" />;
}

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
  attachments = [],
  workspace = null,
  accessMode = "requestApproval",
  streamingDelta = "",
  isLoadingThreads = false,
  isLoadingTurns = false,
  isStartingThread = false,
  isSending = false,
  isAttaching = false,
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
  onRemoveAttachment,
  onSelectWorkspace,
  onClearWorkspace,
  onAccessModeChange,
  onDropPaths,
  onPasteImages,
  onNewThread,
  onReload,
}: BrainCenterProps) {
  const historyRef = useRef<HTMLDivElement>(null);
  const draftInputRef = useRef<HTMLTextAreaElement>(null);
  const composerRef = useRef<HTMLFormElement>(null);
  const attachmentMenuRef = useRef<HTMLDivElement>(null);
  const accessMenuRef = useRef<HTMLDivElement>(null);
  const followsLatestRef = useRef(true);
  const previousThreadIdRef = useRef<string | null | undefined>(undefined);
  const [attachmentMenuOpen, setAttachmentMenuOpen] = useState(false);
  const [accessMenuOpen, setAccessMenuOpen] = useState(false);
  const [dragActive, setDragActive] = useState(false);
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
    !isAttaching &&
    (draft.trim().length > 0 || attachments.length > 0) &&
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

  useLayoutEffect(() => {
    const textarea = draftInputRef.current;
    if (!textarea) return;

    const resizeDraftInput = () => {
      textarea.style.height = "auto";
      const maxHeight = Math.min(320, Math.max(140, window.innerHeight * 0.44));
      const nextHeight = Math.min(textarea.scrollHeight, maxHeight);
      textarea.style.height = `${nextHeight}px`;
      textarea.style.overflowY = textarea.scrollHeight > maxHeight ? "auto" : "hidden";
    };

    resizeDraftInput();
    window.addEventListener("resize", resizeDraftInput);
    return () => window.removeEventListener("resize", resizeDraftInput);
  }, [draft]);

  useEffect(() => {
    const closeMenus = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!attachmentMenuRef.current?.contains(target)) setAttachmentMenuOpen(false);
      if (!accessMenuRef.current?.contains(target)) setAccessMenuOpen(false);
    };
    document.addEventListener("pointerdown", closeMenus);
    return () => document.removeEventListener("pointerdown", closeMenus);
  }, []);

  useEffect(() => {
    if (!onDropPaths) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/webviewWindow")
      .then(({ getCurrentWebviewWindow }) =>
        getCurrentWebviewWindow().onDragDropEvent((event) => {
          if (event.payload.type === "over") {
            setDragActive(true);
            return;
          }
          if (event.payload.type === "leave") {
            setDragActive(false);
            return;
          }
          setDragActive(false);
          if (event.payload.type === "drop" && event.payload.paths.length > 0) {
            onDropPaths(event.payload.paths);
          }
        }),
      )
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => {
        // The web preview has no native drag-drop event bridge.
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [onDropPaths]);

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

  function handlePaste(event: ClipboardEvent<HTMLTextAreaElement>) {
    if (!onPasteImages) return;
    const images = Array.from(event.clipboardData?.items ?? [])
      .filter((item) => item.type.startsWith("image/"))
      .map((item) => item.getAsFile())
      .filter((file): file is File => file !== null);
    if (images.length === 0) return;
    event.preventDefault();
    onPasteImages(images.slice(0, 5));
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

        <form
          ref={composerRef}
          className={`brain-center__composer${dragActive ? " is-drag-active" : ""}`}
          onSubmit={submit}
        >
          <div className="brain-center__composer-surface">
            {attachments.length > 0 && (
              <div className="brain-center__attachments" aria-label="待发送附件">
                {attachments.map((attachment) => (
                  <div className="brain-center__attachment-card" key={attachment.assetId}>
                    {attachment.kind === "image" && attachment.previewUrl ? (
                      <img src={attachment.previewUrl} alt="" />
                    ) : (
                      <span className="brain-center__attachment-icon">
                        <AttachmentIcon kind={attachment.kind} />
                      </span>
                    )}
                    <span className="brain-center__attachment-copy">
                      <strong title={attachment.displayName}>{attachment.displayName}</strong>
                      <small>{attachment.kind === "image" ? "图片" : attachment.mimeType} · {formatFileSize(attachment.sizeBytes)}</small>
                    </span>
                    {onRemoveAttachment && (
                      <button
                        type="button"
                        className="brain-center__attachment-remove"
                        onClick={() => onRemoveAttachment(attachment.assetId)}
                        disabled={isRunning || isAttaching}
                        title={`移除 ${attachment.displayName}`}
                        aria-label={`移除 ${attachment.displayName}`}
                      >
                        <X size={13} />
                      </button>
                    )}
                  </div>
                ))}
              </div>
            )}

            <div className="brain-center__composer-main">
              <textarea
                ref={draftInputRef}
                value={draft}
                onChange={(event) => onDraftChange(event.currentTarget.value)}
                onKeyDown={handleDraftKeyDown}
                onPaste={handlePaste}
                disabled={isDegraded}
                placeholder={
                  isDegraded
                    ? "智能助手暂不可用"
                    : attachments.length > 0
                      ? "为附件补充说明"
                      : "输入消息或任务"
                }
                rows={1}
                aria-label="消息内容"
              />
            </div>

            <div className="brain-center__composer-footer">
              <div className="brain-center__composer-tools">
                {(onAttach || onSelectWorkspace) && (
                  <div className="brain-center__menu-anchor" ref={attachmentMenuRef}>
                    <button
                      type="button"
                      className="brain-center__attachment-button"
                      onClick={() => setAttachmentMenuOpen((current) => !current)}
                      disabled={isRunning || isDegraded || isAttaching}
                      title="添加文件或工作区"
                      aria-label="添加文件或工作区"
                      aria-expanded={attachmentMenuOpen}
                    >
                      {isAttaching ? <LoaderCircle size={16} className="brain-center__spin" /> : <Plus size={17} />}
                    </button>
                    {attachmentMenuOpen && (
                      <div className="brain-center__composer-menu" role="menu">
                        {onAttach && (
                          <button
                            type="button"
                            onClick={() => {
                              setAttachmentMenuOpen(false);
                              onAttach();
                            }}
                          >
                            <FileText size={15} aria-hidden="true" />
                            <span><strong>文件</strong><small>选择一个或多个文件</small></span>
                          </button>
                        )}
                        {onSelectWorkspace && (
                          <button
                            type="button"
                            onClick={() => {
                              setAttachmentMenuOpen(false);
                              onSelectWorkspace();
                            }}
                          >
                            <FolderOpen size={15} aria-hidden="true" />
                            <span><strong>工作区文件夹</strong><small>让助手在选中目录内执行</small></span>
                          </button>
                        )}
                      </div>
                    )}
                  </div>
                )}

                <div className="brain-center__menu-anchor" ref={accessMenuRef}>
                  <button
                    type="button"
                    className={`brain-center__access-button brain-center__access-button--${accessMode}`}
                    onClick={() => setAccessMenuOpen((current) => !current)}
                    disabled={isRunning || isDegraded}
                    aria-expanded={accessMenuOpen}
                  >
                    <AccessIcon mode={accessMode} />
                    <span>{ACCESS_OPTIONS.find((option) => option.mode === accessMode)?.label}</span>
                  </button>
                  {accessMenuOpen && onAccessModeChange && (
                    <div className="brain-center__composer-menu brain-center__composer-menu--access" role="menu">
                      {ACCESS_OPTIONS.map((option) => (
                        <button
                          key={option.mode}
                          type="button"
                          className={option.mode === accessMode ? "is-selected" : undefined}
                          onClick={() => {
                            onAccessModeChange(option.mode);
                            setAccessMenuOpen(false);
                          }}
                        >
                          <AccessIcon mode={option.mode} />
                          <span><strong>{option.label}</strong><small>{option.detail}</small></span>
                          {option.mode === accessMode && <span className="brain-center__menu-check">✓</span>}
                        </button>
                      ))}
                    </div>
                  )}
                </div>

                {workspace && (
                  <span className="brain-center__workspace-chip" title="当前 Codex 工作区">
                    <Folder size={14} aria-hidden="true" />
                    <span>{workspace.displayName}</span>
                    {onClearWorkspace && (
                      <button
                        type="button"
                        onClick={onClearWorkspace}
                        disabled={isRunning}
                        aria-label="移除工作区"
                        title="移除工作区"
                      >
                        <X size={12} />
                      </button>
                    )}
                  </span>
                )}
              </div>

              <div className="brain-center__composer-meta">
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
                <span className="brain-center__draft-count">{draft.length}</span>
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
            </div>
          </div>
        </form>
      </div>
    </section>
  );
}
