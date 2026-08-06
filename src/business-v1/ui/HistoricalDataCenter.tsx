import { useEffect, useMemo, useState } from "react";
import { ArchiveRestore, FileArchive, FolderOpen, RefreshCw, Search, X } from "lucide-react";
import type { BsaigcClientSnapshot } from "../../client-sdk";
import {
  buildHistoryRecords,
  filterHistoryRecords,
  formatHistoryTimestamp,
  HISTORY_CATEGORY_LABELS,
  historyCategoryCounts,
  type HistoryCategory,
  type HistoryRecordView,
} from "../application/historyDataView";
import "./historical-data.css";

const HISTORY_CATEGORIES = Object.keys(HISTORY_CATEGORY_LABELS) as HistoryCategory[];

export interface HistoricalDataCenterProps {
  open: boolean;
  snapshot: Pick<
    BsaigcClientSnapshot,
    | "projects"
    | "tasks"
    | "cases"
    | "requirementBriefs"
    | "executionBriefs"
    | "assets"
    | "brainThreads"
  >;
  activeProjectId: string | null;
  onClose: () => void;
  onRefresh: () => Promise<void>;
  onOpenAsset: (assetId: string) => Promise<void>;
  onRestoreThread: (threadId: string) => Promise<void>;
}

export function HistoricalDataCenter({
  open,
  snapshot,
  activeProjectId,
  onClose,
  onRefresh,
  onOpenAsset,
  onRestoreThread,
}: HistoricalDataCenterProps) {
  const [activeCategory, setActiveCategory] = useState<HistoryCategory>("tasks");
  const [query, setQuery] = useState("");
  const [currentProjectOnly, setCurrentProjectOnly] = useState(false);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const records = useMemo(() => buildHistoryRecords(snapshot), [snapshot]);
  const projectFilter = currentProjectOnly && activeProjectId ? activeProjectId : null;
  const counts = useMemo(
    () => historyCategoryCounts(records, projectFilter),
    [projectFilter, records],
  );
  const visibleRecords = useMemo(
    () => filterHistoryRecords(records, activeCategory, query, projectFilter),
    [activeCategory, projectFilter, query, records],
  );

  useEffect(() => {
    if (!open) return undefined;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busyKey) onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busyKey, onClose, open]);

  if (!open) return null;

  const runAction = async (key: string, action: () => Promise<void>, successMessage?: string) => {
    if (busyKey) return;
    setBusyKey(key);
    setError(null);
    setNotice(null);
    try {
      await action();
      if (successMessage) setNotice(successMessage);
    } catch (actionError) {
      setError(historyErrorMessage(actionError));
    } finally {
      setBusyKey(null);
    }
  };

  return (
    <div
      className="bw-history-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busyKey) onClose();
      }}
    >
      <section className="bw-history-panel" role="dialog" aria-modal="true" aria-label="历史资料">
        <header className="bw-history-panel__header">
          <div>
            <span className="bw-history-panel__eyebrow">资料归档</span>
            <h2>历史资料</h2>
            <p>集中查看现有项目资料与归档记录，原件和历史版本保持不变。</p>
          </div>
          <div className="bw-history-panel__header-actions">
            <button
              className="bw-secondary-button"
              type="button"
              disabled={Boolean(busyKey)}
              onClick={() => void runAction("refresh", onRefresh, "历史资料已刷新。")}
            >
              <RefreshCw size={15} className={busyKey === "refresh" ? "is-spinning" : undefined} />
              刷新
            </button>
            <button className="bw-icon-button" type="button" onClick={onClose} disabled={Boolean(busyKey)} title="关闭历史资料">
              <X size={18} />
            </button>
          </div>
        </header>

        <div className="bw-history-panel__toolbar">
          <label className="bw-history-search">
            <Search size={15} aria-hidden="true" />
            <input
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="搜索标题、项目、状态"
              aria-label="搜索历史资料"
            />
          </label>
          <label className="bw-history-scope">
            <input
              type="checkbox"
              checked={currentProjectOnly}
              disabled={!activeProjectId}
              onChange={(event) => setCurrentProjectOnly(event.target.checked)}
            />
            仅当前项目
          </label>
        </div>

        <nav className="bw-history-tabs" aria-label="历史资料分类">
          {HISTORY_CATEGORIES.map((category) => (
            <button
              type="button"
              className={category === activeCategory ? "is-active" : undefined}
              onClick={() => setActiveCategory(category)}
              aria-pressed={category === activeCategory}
              key={category}
            >
              <span>{HISTORY_CATEGORY_LABELS[category]}</span>
              <small>{counts[category]}</small>
            </button>
          ))}
        </nav>

        <div className="bw-history-panel__body">
          {error ? <div className="bw-history-message is-error" role="alert">{error}</div> : null}
          {notice ? <div className="bw-history-message is-success" role="status">{notice}</div> : null}
          {visibleRecords.length ? (
            <div className="bw-history-list">
              {visibleRecords.map((record) => (
                <HistoryRecordCard
                  record={record}
                  busy={busyKey === record.id}
                  disabled={Boolean(busyKey)}
                  onOpenAsset={(assetId) => void runAction(record.id, () => onOpenAsset(assetId))}
                  onRestore={(threadId) => void runAction(record.id, () => onRestoreThread(threadId), "归档会话已恢复。")}
                  key={record.category + ":" + record.id}
                />
              ))}
            </div>
          ) : (
            <div className="bw-history-empty">
              <FileArchive size={26} />
              <strong>没有匹配的{HISTORY_CATEGORY_LABELS[activeCategory]}</strong>
              <span>{query ? "清空搜索或切换项目范围后再看。" : "现有底座中暂时没有这类历史记录。"}</span>
            </div>
          )}
        </div>

        <footer className="bw-history-panel__footer">
          <span>除恢复归档会话外，其余资料只读；原件和历史版本不会被覆盖。</span>
          <strong>{visibleRecords.length} 条</strong>
        </footer>
      </section>
    </div>
  );
}

interface HistoryRecordCardProps {
  record: HistoryRecordView;
  busy: boolean;
  disabled: boolean;
  onOpenAsset: (assetId: string) => void;
  onRestore: (threadId: string) => void;
}

function HistoryRecordCard({
  record,
  busy,
  disabled,
  onOpenAsset,
  onRestore,
}: HistoryRecordCardProps) {
  return (
    <article className="bw-history-card">
      <div className="bw-history-card__icon" aria-hidden="true">
        {record.restoreThreadId ? <ArchiveRestore size={18} /> : <FileArchive size={18} />}
      </div>
      <div className="bw-history-card__copy">
        <header>
          <strong>{record.title}</strong>
          <span className={"bw-history-status is-" + record.statusTone}>{record.statusLabel}</span>
        </header>
        <p>{record.detail}</p>
        <footer>
          <span>{record.projectLabel}</span>
          <time>{formatHistoryTimestamp(record.updatedAt)}</time>
        </footer>
      </div>
      {record.assetId || record.restoreThreadId ? (
        <div className="bw-history-card__actions">
          {record.assetId ? (
            <button className="bw-secondary-button" type="button" disabled={disabled} onClick={() => onOpenAsset(record.assetId!)}>
              <FolderOpen size={14} />
              打开
            </button>
          ) : null}
          {record.restoreThreadId ? (
            <button className="bw-primary-button" type="button" disabled={disabled} onClick={() => onRestore(record.restoreThreadId!)}>
              <ArchiveRestore size={14} className={busy ? "is-spinning" : undefined} />
              恢复
            </button>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}

function historyErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  return "历史资料操作失败，请重试。";
}
