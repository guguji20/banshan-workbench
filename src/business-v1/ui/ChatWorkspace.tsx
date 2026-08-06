import { useEffect, useId, useRef, useState } from "react";
import type { ClipboardEvent, DragEvent, FormEvent, KeyboardEvent } from "react";
import {
  AlertTriangle,
  Archive,
  Bot,
  Check,
  CheckCircle2,
  ChevronDown,
  CircleDollarSign,
  ClipboardCheck,
  Clock3,
  File,
  FileCheck2,
  FileSpreadsheet,
  FileText,
  Folder,
  FolderOpen,
  Globe2,
  LoaderCircle,
  Menu,
  Image as ImageIcon,
  Paperclip,
  PanelRightOpen,
  RefreshCw,
  Scale,
  Search,
  Send,
  ShieldCheck,
  Sparkles,
  SquareStack,
  X,
  ExternalLink,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type {
  AttachmentKind,
  BusinessTaskKind,
  BusinessWorkspaceActions,
  ChatMessage,
  ComposerState,
  OutputArtifact,
  SelectOption,
  TaskStatus,
  WorkspaceAttachment,
  WorkspaceConversation,
  WorkspaceProject,
  WorkspaceTask,
} from "./types";
import {
  extractClipboardImages,
  extractDroppedFiles,
  extractHostDropPaths,
  hasFileDrop,
} from "./attachmentInput";
import {
  extractWebResearchSources,
  isPublicHttpUrl,
  WEB_RESEARCH_DATA_POLICY,
} from "../application/webResearchView";

interface ChatWorkspaceProps {
  project?: WorkspaceProject;
  conversation?: WorkspaceConversation;
  messages: ChatMessage[];
  composer: ComposerState;
  modelOptions: SelectOption[];
  actions: BusinessWorkspaceActions;
  isSidebarOpen: boolean;
  isContextOpen: boolean;
  onOpenSidebar: () => void;
  onToggleContext: () => void;
}

const taskOptions: Array<{ kind: BusinessTaskKind; label: string; icon: LucideIcon }> = [
  { kind: "quotation", label: "生成报价", icon: CircleDollarSign },
  { kind: "acceptance", label: "本次验收", icon: ClipboardCheck },
  { kind: "contract-review", label: "合同审查", icon: Scale },
  { kind: "settlement", label: "发起结算", icon: FileCheck2 },
  { kind: "archive", label: "整理归档", icon: Archive },
  { kind: "search", label: "资料检索", icon: Search },
];

const sourceScopeOptions = [
  { value: "workspace", label: "仅当前项目" },
  { value: "workspace-shared", label: "项目 + 共享案例" },
] satisfies ReadonlyArray<{ value: ComposerState["sourceScope"]; label: string }>;

const networkScopeOptions = [
  { value: "local-only", label: "仅本地" },
  { value: "web-enabled", label: "允许联网" },
] satisfies ReadonlyArray<{ value: ComposerState["networkScope"]; label: string }>;

export function ChatWorkspace({
  project,
  conversation,
  messages,
  composer,
  modelOptions,
  actions,
  isSidebarOpen,
  isContextOpen,
  onOpenSidebar,
  onToggleContext,
}: ChatWorkspaceProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const dragDepthRef = useRef(0);
  const [isDropActive, setIsDropActive] = useState(false);

  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = "0px";
    textarea.style.height = `${Math.min(textarea.scrollHeight, 180)}px`;
  }, [composer.value]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ block: "end" });
  }, [messages]);

  const canSubmit = Boolean(composer.value.trim() || composer.attachments.length) && !composer.isSubmitting;

  const submit = (event?: FormEvent) => {
    event?.preventDefault();
    if (canSubmit) actions.onSendMessage();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      if (canSubmit) actions.onSendMessage();
    }
  };

  const handlePaste = (event: ClipboardEvent<HTMLFormElement>) => {
    const images = extractClipboardImages(event.clipboardData);
    if (!images.length) return;

    event.preventDefault();
    actions.onPasteScreenshot(images);
  };

  const handleDragEnter = (event: DragEvent<HTMLFormElement>) => {
    if (!acceptsDrop(event, actions)) return;
    event.preventDefault();
    dragDepthRef.current += 1;
    setIsDropActive(true);
  };

  const handleDragOver = (event: DragEvent<HTMLFormElement>) => {
    if (!acceptsDrop(event, actions)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
  };

  const handleDragLeave = (event: DragEvent<HTMLFormElement>) => {
    if (!isDropActive) return;
    event.preventDefault();
    dragDepthRef.current = Math.max(0, dragDepthRef.current - 1);
    if (dragDepthRef.current === 0) setIsDropActive(false);
  };

  const handleDrop = (event: DragEvent<HTMLFormElement>) => {
    if (!acceptsDrop(event, actions)) return;
    event.preventDefault();
    dragDepthRef.current = 0;
    setIsDropActive(false);

    const files = extractDroppedFiles(event.dataTransfer);
    const paths = extractHostDropPaths(event.nativeEvent);
    if (paths.length) actions.onDropPaths?.(paths);
    else if (files.length) actions.onDropFiles?.(files);
  };

  return (
    <main className="bw-chat">
      <header className="bw-chat-header">
        {!isSidebarOpen ? (
          <button className="bw-icon-button" type="button" onClick={onOpenSidebar} title="打开项目栏">
            <Menu size={18} />
          </button>
        ) : null}
        <div className="bw-chat-header__copy">
          <div className="bw-breadcrumb">
            <span>{project?.customerName ?? "未选择客户"}</span>
            <span>/</span>
            <span>{project?.name ?? "选择项目"}</span>
          </div>
          <h1>{conversation?.title ?? "新建商务任务"}</h1>
        </div>
        {project?.localPath ? (
          <button className="bw-workspace-path" type="button" onClick={actions.onOpenWorkspaceFolder} title={project.localPath}>
            <FolderOpen size={14} />
            <span>本地工作区</span>
          </button>
        ) : null}
        <button
          className={`bw-icon-button ${isContextOpen ? "is-active" : ""}`}
          type="button"
          onClick={onToggleContext}
          title={isContextOpen ? "关闭上下文" : "打开上下文"}
        >
          <PanelRightOpen size={18} />
        </button>
      </header>

      <section className="bw-chat-scroll" aria-live="polite">
        <div className="bw-chat-stream">
          {messages.length === 0 ? (
            <EmptyConversation projectName={project?.name} onStartTask={actions.onStartTask} />
          ) : (
            messages.map((message) => (
              <MessageItem
                key={message.id}
                message={message}
                onOpenArtifact={actions.onOpenArtifact}
                onRetryTask={actions.onRetryTask}
                onConfirmTask={actions.onConfirmTask}
              />
            ))
          )}
          <div ref={messagesEndRef} />
        </div>
      </section>

      <section className="bw-composer-zone">
        <div className="bw-quick-actions" aria-label="快捷任务">
          {taskOptions.map(({ kind, label, icon: Icon }) => (
            <button type="button" key={kind} onClick={() => actions.onStartTask(kind)}>
              <Icon size={14} />
              <span>{label}</span>
            </button>
          ))}
        </div>

        <form
          className={`bw-composer ${isDropActive ? "is-drop-active" : ""}`}
          onSubmit={submit}
          onPaste={handlePaste}
          onDragEnter={handleDragEnter}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          {isDropActive ? (
            <div className="bw-composer-drop-overlay" aria-hidden="true">
              <FolderOpen size={22} />
              <strong>释放以添加文件或文件夹</strong>
            </div>
          ) : null}
          {composer.attachments.length ? (
            <div className="bw-composer-attachments">
              {composer.attachments.map((attachment) => (
                <AttachmentChip
                  key={attachment.id}
                  attachment={attachment}
                  onRemove={() => actions.onRemoveAttachment(attachment.id)}
                />
              ))}
            </div>
          ) : null}

          <textarea
            ref={textareaRef}
            value={composer.value}
            rows={1}
            placeholder={composer.placeholder ?? "描述要完成的商务任务，或直接说“生成报价”“做本次验收”"}
            onChange={(event) => actions.onComposerChange(event.target.value)}
            onKeyDown={handleKeyDown}
            aria-label="任务输入"
          />

          <div className="bw-composer-toolbar">
            <div className="bw-composer-toolbar__group">
              <button className="bw-icon-button" type="button" onClick={actions.onAddFiles} title="添加文件">
                <Paperclip size={17} />
              </button>
              <button className="bw-icon-button" type="button" onClick={actions.onAddFolder} title="添加文件夹">
                <Folder size={17} />
              </button>
            </div>

            <div className="bw-composer-toolbar__selectors">
              <CompactMenu
                ariaLabel="资料范围"
                icon={SquareStack}
                value={composer.sourceScope}
                options={sourceScopeOptions}
                onChange={actions.onSourceScopeChange}
              />
              <CompactMenu
                ariaLabel="联网范围"
                icon={Globe2}
                value={composer.networkScope}
                options={networkScopeOptions}
                onChange={actions.onNetworkScopeChange}
              />
              <CompactMenu
                ariaLabel="模型"
                icon={Sparkles}
                value={composer.modelId}
                options={modelOptions}
                onChange={actions.onModelChange}
              />
            </div>

            <button className="bw-send-button" type="submit" disabled={!canSubmit} title="发送">
              {composer.isSubmitting ? <LoaderCircle className="bw-spin" size={17} /> : <Send size={17} />}
            </button>
          </div>
        </form>
        <p className="bw-composer-note">源文件默认只读，正式导出、共享与删除需要人工确认。</p>
      </section>
    </main>
  );
}

function acceptsDrop(
  event: DragEvent<HTMLFormElement>,
  actions: BusinessWorkspaceActions,
): boolean {
  const hasFiles = Boolean(actions.onDropFiles) && hasFileDrop(event.dataTransfer);
  const hasPaths = Boolean(actions.onDropPaths) && extractHostDropPaths(event.nativeEvent).length > 0;
  return hasFiles || hasPaths;
}

interface CompactMenuProps<Value extends string> {
  ariaLabel: string;
  icon: LucideIcon;
  value: Value;
  options: ReadonlyArray<{ value: Value; label: string }>;
  onChange: (value: Value) => void;
}

function CompactMenu<Value extends string>({
  ariaLabel,
  icon: Icon,
  value,
  options,
  onChange,
}: CompactMenuProps<Value>) {
  const menuId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const selectedIndex = Math.max(0, options.findIndex((option) => option.value === value));
  const [isOpen, setIsOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(selectedIndex);
  const selectedOption = options[selectedIndex] ?? options[0];

  useEffect(() => {
    if (!isOpen) setActiveIndex(selectedIndex);
  }, [isOpen, selectedIndex]);

  useEffect(() => {
    if (!isOpen) return;

    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setIsOpen(false);
    };
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer);
  }, [isOpen]);

  useEffect(() => {
    if (isOpen) optionRefs.current[activeIndex]?.focus();
  }, [activeIndex, isOpen]);

  const openMenu = (index = selectedIndex) => {
    setActiveIndex(Math.max(0, Math.min(index, options.length - 1)));
    setIsOpen(true);
  };

  const closeMenu = (restoreFocus = false) => {
    setIsOpen(false);
    if (restoreFocus) triggerRef.current?.focus();
  };

  const selectOption = (index: number) => {
    const option = options[index];
    if (!option) return;
    onChange(option.value);
    closeMenu(true);
  };

  const handleTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault();
      const direction = event.key === "ArrowUp" ? -1 : 1;
      openMenu(compactMenuNextIndex(selectedIndex, direction, options.length));
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (isOpen) closeMenu();
      else openMenu();
      return;
    }
    if (event.key === "Escape" && isOpen) {
      event.preventDefault();
      closeMenu(true);
    }
  };

  const handleOptionKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex(compactMenuNextIndex(index, event.key === "ArrowUp" ? -1 : 1, options.length));
      return;
    }
    if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      setActiveIndex(event.key === "Home" ? 0 : options.length - 1);
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectOption(index);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      closeMenu(true);
      return;
    }
    if (event.key === "Tab") closeMenu();
  };

  return (
    <div className="bw-compact-menu" ref={rootRef}>
      <button
        className="bw-compact-menu__trigger"
        type="button"
        ref={triggerRef}
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-controls={isOpen ? menuId : undefined}
        title={ariaLabel}
        onClick={() => (isOpen ? closeMenu() : openMenu())}
        onKeyDown={handleTriggerKeyDown}
      >
        <Icon size={15} />
        <span>{selectedOption?.label ?? ariaLabel}</span>
        <ChevronDown size={13} aria-hidden="true" />
      </button>
      {isOpen ? (
        <div className="bw-compact-menu__list" id={menuId} role="listbox" aria-label={ariaLabel}>
          {options.map((option, index) => (
            <button
              className={index === selectedIndex ? "is-selected" : undefined}
              type="button"
              key={option.value}
              ref={(element) => { optionRefs.current[index] = element; }}
              role="option"
              aria-selected={index === selectedIndex}
              tabIndex={index === activeIndex ? 0 : -1}
              onClick={() => selectOption(index)}
              onKeyDown={(event) => handleOptionKeyDown(event, index)}
              onPointerEnter={() => setActiveIndex(index)}
            >
              <span>{option.label}</span>
              {index === selectedIndex ? <Check size={14} aria-hidden="true" /> : null}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

export function compactMenuNextIndex(currentIndex: number, direction: -1 | 1, optionCount: number): number {
  if (optionCount <= 0) return 0;
  return (currentIndex + direction + optionCount) % optionCount;
}

interface EmptyConversationProps {
  projectName?: string;
  onStartTask: (kind: BusinessTaskKind) => void;
}

function EmptyConversation({ projectName, onStartTask }: EmptyConversationProps) {
  return (
    <div className="bw-empty-chat">
      <div className="bw-empty-chat__mark">
        <Sparkles size={22} />
      </div>
      <h2>{projectName ? `继续处理 ${projectName}` : "从一个真实商务任务开始"}</h2>
      <p>报价、合同、验收和结算可以独立启动，也可以复用当前项目的资料、模板与历史成功件。</p>
      <div className="bw-empty-chat__tasks">
        {taskOptions.slice(0, 4).map(({ kind, label, icon: Icon }) => (
          <button type="button" key={kind} onClick={() => onStartTask(kind)}>
            <Icon size={18} />
            <span>{label}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

interface MessageItemProps {
  message: ChatMessage;
  onOpenArtifact: (artifactId: string) => void;
  onRetryTask: (taskId: string) => void;
  onConfirmTask: (taskId: string) => void;
}

function MessageItem({ message, onOpenArtifact, onRetryTask, onConfirmTask }: MessageItemProps) {
  if (message.role === "system") {
    return <div className="bw-system-message">{message.content}</div>;
  }

  const sources = message.role === "assistant"
    ? (message.sources?.length ? message.sources : extractWebResearchSources(message.content))
      .filter((source) => isPublicHttpUrl(source.url))
    : [];

  return (
    <article className={`bw-message bw-message--${message.role}`}>
      <div className="bw-message__avatar">{message.role === "assistant" ? <Bot size={17} /> : message.authorName.slice(0, 1)}</div>
      <div className="bw-message__body">
        <header>
          <strong>{message.authorName}</strong>
          <time>{message.createdAt}</time>
        </header>
        <div className="bw-message__content">{message.content}</div>
        {sources.length ? <WebResearchSources sources={sources} /> : null}
        {message.attachments?.length ? (
          <div className="bw-message-attachments">
            {message.attachments.map((attachment) => (
              <AttachmentCard attachment={attachment} key={attachment.id} />
            ))}
          </div>
        ) : null}
        {message.task ? (
          <TaskResultCard
            task={message.task}
            onOpenArtifact={onOpenArtifact}
            onRetry={() => onRetryTask(message.task?.id ?? "")}
            onConfirm={() => onConfirmTask(message.task?.id ?? "")}
          />
        ) : null}
      </div>
    </article>
  );
}

function WebResearchSources({ sources }: { sources: NonNullable<ChatMessage["sources"]> }) {
  return (
    <section className="bw-web-sources" aria-label="联网来源">
      <header className="bw-web-sources__header">
        <span><Globe2 size={14} />联网来源</span>
        <small>{WEB_RESEARCH_DATA_POLICY}</small>
      </header>
      <div className="bw-web-sources__list">
        {sources.map((source) => (
          <a
            className="bw-web-source-card"
            href={source.url}
            key={source.id}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={`打开来源：${source.title}`}
          >
            <span className="bw-web-source-card__main">
              <strong>{source.title}</strong>
              <span><Globe2 size={12} />{source.domain}</span>
            </span>
            <span className="bw-web-source-card__meta">
              <time dateTime={new Date(source.accessedAt).toISOString()}>访问于 {source.accessedDate}</time>
              <em>{source.verificationLabel}</em>
              <ExternalLink size={13} aria-hidden="true" />
            </span>
          </a>
        ))}
      </div>
      <p>外部来源只作为参考证据展示，不会自动覆盖客户、合同、报价或其他正式业务数据。</p>
    </section>
  );
}

function AttachmentCard({ attachment }: { attachment: WorkspaceAttachment }) {
  const Icon = attachmentIcon(attachment.kind);
  return (
    <div className="bw-attachment-card">
      <span className="bw-file-icon"><Icon size={18} /></span>
      <span>
        <strong>{attachment.name}</strong>
        <small>{[attachment.sizeLabel, attachment.sourceLabel].filter(Boolean).join(" · ")}</small>
      </span>
      {attachment.status === "reading" ? <LoaderCircle className="bw-spin" size={15} /> : null}
      {attachment.status === "ready" ? <Check size={15} /> : null}
      {attachment.status === "failed" ? <AlertTriangle size={15} /> : null}
    </div>
  );
}

interface AttachmentChipProps {
  attachment: WorkspaceAttachment;
  onRemove: () => void;
}

function AttachmentChip({ attachment, onRemove }: AttachmentChipProps) {
  const Icon = attachmentIcon(attachment.kind);
  return (
    <div className="bw-attachment-chip">
      <Icon size={14} />
      <span>{attachment.name}</span>
      <button type="button" onClick={onRemove} title="移除附件">
        <X size={13} />
      </button>
    </div>
  );
}

interface TaskResultCardProps {
  task: WorkspaceTask;
  onOpenArtifact: (artifactId: string) => void;
  onRetry: () => void;
  onConfirm: () => void;
}

function TaskResultCard({ task, onOpenArtifact, onRetry, onConfirm }: TaskResultCardProps) {
  const status = taskStatusMeta(task.status);
  const StatusIcon = status.icon;
  const progress = Math.max(0, Math.min(task.progress, 100));

  return (
    <section className="bw-task-card">
      <header>
        <span className={`bw-task-status is-${task.status}`}><StatusIcon className={task.status === "running" ? "bw-spin" : ""} size={16} /></span>
        <div>
          <strong>{task.title}</strong>
          <small>{status.label} · {task.stageLabel}</small>
        </div>
        <span className={`bw-status-badge is-${task.status}`}>{status.label}</span>
      </header>

      <p>{task.detail}</p>
      {task.status === "running" || task.status === "queued" ? (
        <div className="bw-task-progress">
          <span style={{ width: `${progress}%` }} />
        </div>
      ) : null}

      {task.outputs?.length ? (
        <div className="bw-output-list">
          {task.outputs.map((output) => (
            <button type="button" key={output.id} onClick={() => onOpenArtifact(output.id)} disabled={output.status === "blocked"}>
              <OutputIcon format={output.format} />
              <span>
                <strong>{output.name}</strong>
                <small>{output.versionLabel}{output.detail ? ` · ${output.detail}` : ""}</small>
              </span>
              <span className={`bw-output-state is-${output.status}`}>
                {output.status === "blocked" ? "已阻止" : output.status === "ready" ? "打开" : "草稿"}
              </span>
            </button>
          ))}
        </div>
      ) : null}

      {task.confirmationBlockedReason ? (
        <div className="bw-task-blocker" role="status">
          <AlertTriangle size={14} />
          <span>{task.confirmationBlockedReason}</span>
        </div>
      ) : null}

      {task.status === "failed" || task.requiresConfirmation ? (
        <footer>
          {task.status === "failed" ? (
            <button className="bw-secondary-button" type="button" onClick={onRetry}>
              <RefreshCw size={15} />
              重试任务
            </button>
          ) : null}
          {task.requiresConfirmation ? (
            <button
              className="bw-primary-button"
              type="button"
              onClick={onConfirm}
              disabled={Boolean(task.confirmationBlockedReason)}
              title={task.confirmationBlockedReason}
            >
              <ShieldCheck size={15} />
              人工确认
            </button>
          ) : null}
        </footer>
      ) : null}
    </section>
  );
}

function OutputIcon({ format }: { format: OutputArtifact["format"] }) {
  if (format === "xlsx") return <FileSpreadsheet size={18} />;
  if (format === "folder") return <Folder size={18} />;
  return <FileText size={18} />;
}

function attachmentIcon(kind: AttachmentKind): LucideIcon {
  if (kind === "folder") return Folder;
  if (kind === "image") return ImageIcon;
  if (kind === "spreadsheet") return FileSpreadsheet;
  if (kind === "pdf" || kind === "document") return FileText;
  return File;
}

function taskStatusMeta(status: TaskStatus): { label: string; icon: LucideIcon } {
  const states: Record<TaskStatus, { label: string; icon: LucideIcon }> = {
    queued: { label: "等待中", icon: Clock3 },
    running: { label: "处理中", icon: LoaderCircle },
    "waiting-confirmation": { label: "待确认", icon: ShieldCheck },
    completed: { label: "已完成", icon: CheckCircle2 },
    failed: { label: "失败", icon: AlertTriangle },
  };
  return states[status];
}
