import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { Library, RefreshCw, Send, ShieldCheck, Trash2, X } from "lucide-react";
import type { BsaigcClient } from "../../client-sdk";
import type { AppUserRecord } from "../../generated/bsaigc/AppUserRecord";
import type { CaseRecord } from "../../generated/bsaigc/CaseRecord";
import type { SharedCaseGrant } from "../../generated/bsaigc/SharedCaseGrant";
import type { SharedCaseDomainEvent } from "../../generated/bsaigc/SharedCaseDomainEvent";
import type { SharedCasePermission } from "../../generated/bsaigc/SharedCasePermission";
import type { SharedCasePublicationRecord } from "../../generated/bsaigc/SharedCasePublicationRecord";

const SHARED_CASE_PERMISSIONS: readonly SharedCasePermission[] = [
  "discover",
  "preview",
  "reference",
  "download",
];

const SHARED_CASE_PERMISSION_LABELS: Record<SharedCasePermission, string> = {
  discover: "可发现",
  preview: "可预览",
  reference: "可引用",
  download: "可下载",
};

const SHARED_CASE_STATUS_LABELS: Record<SharedCasePublicationRecord["status"], string> = {
  pendingBackup: "等待云端备份",
  published: "已发布",
  withdrawn: "已撤回",
};

export function parseSharedCaseGrants(value: string): SharedCaseGrant[] {
  const grants = new Map<string, SharedCasePermission[]>();
  for (const rawLine of value.split(/\r?\n/u)) {
    const line = rawLine.trim();
    if (!line) continue;
    const separator = line.search(/[:=：]/u);
    if (separator <= 0) throw new Error(`授权格式错误：${line}`);
    const username = line.slice(0, separator).trim();
    const permissionValues = line
      .slice(separator + 1)
      .split(/[,，\s]+/u)
      .map((permission) => permission.trim())
      .filter(Boolean);
    if (!username || permissionValues.length === 0) throw new Error(`授权格式错误：${line}`);
    const permissions = grants.get(username) ?? [];
    for (const permissionValue of permissionValues) {
      if (!SHARED_CASE_PERMISSIONS.includes(permissionValue as SharedCasePermission)) {
        throw new Error(`不支持的案例权限：${permissionValue}`);
      }
      const permission = permissionValue as SharedCasePermission;
      if (!permissions.includes(permission)) permissions.push(permission);
    }
    grants.set(username, permissions);
  }
  if (grants.size === 0) throw new Error("至少需要一条案例授权");
  return [...grants.entries()].map(([username, permissions]) => ({ username, permissions }));
}

export function formatSharedCaseGrants(grants: readonly SharedCaseGrant[]): string {
  return grants.map((grant) => `${grant.username}: ${grant.permissions.join(", ")}`).join("\n");
}

export function ensureSharedCaseManagerGrant(
  grants: readonly SharedCaseGrant[],
  username: string,
): SharedCaseGrant[] {
  const next = grants.map((grant) => ({ ...grant, permissions: [...grant.permissions] }));
  const managerGrant = next.find((grant) => grant.username === username);
  if (!managerGrant) next.unshift({ username, permissions: [...SHARED_CASE_PERMISSIONS] });
  else if (!managerGrant.permissions.includes("discover")) managerGrant.permissions.unshift("discover");
  return next;
}

function defaultSharedCaseGrantText(username: string): string {
  return `${username}: ${SHARED_CASE_PERMISSIONS.join(", ")}`;
}

export function filterPublishableSharedCaseCandidates(
  cases: readonly CaseRecord[],
  publications: readonly SharedCasePublicationRecord[],
  activeProjectId: string | null,
): CaseRecord[] {
  if (!activeProjectId) return [];
  const activePublicationCaseIds = new Set(
    publications
      .filter((publication) => publication.status !== "withdrawn")
      .map((publication) => publication.caseId),
  );
  return cases.filter(
    (caseRecord) => caseRecord.projectId === activeProjectId && !activePublicationCaseIds.has(caseRecord.id),
  );
}

function sharedCaseErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "object" && error && "message" in error) {
    const message = String(error.message).trim();
    if (message) return message;
  }
  return "共享案例操作失败，请重试。";
}

export interface SharedCasePanelProps {
  open: boolean;
  isAdmin: boolean;
  currentUsername: string;
  cases: readonly CaseRecord[];
  publications: readonly SharedCasePublicationRecord[];
  lastEventSequence?: number | null;
  selectedCaseId: string;
  publishGrantText: string;
  grantDrafts: Readonly<Record<string, string>>;
  loading: boolean;
  busyAction: string | null;
  error: string | null;
  notice: string | null;
  onClose: () => void;
  onRefresh: () => void;
  onSelectedCaseChange: (caseId: string) => void;
  onPublishGrantTextChange: (value: string) => void;
  onPublish: () => void;
  onGrantDraftChange: (publicationId: string, value: string) => void;
  onSaveGrants: (publication: SharedCasePublicationRecord) => void;
  onWithdraw: (publication: SharedCasePublicationRecord) => void;
}

export function SharedCasePanel({
  open,
  isAdmin,
  currentUsername,
  cases,
  publications,
  lastEventSequence = null,
  selectedCaseId,
  publishGrantText,
  grantDrafts,
  loading,
  busyAction,
  error,
  notice,
  onClose,
  onRefresh,
  onSelectedCaseChange,
  onPublishGrantTextChange,
  onPublish,
  onGrantDraftChange,
  onSaveGrants,
  onWithdraw,
}: SharedCasePanelProps) {
  if (!open) return null;
  return (
    <div className="bw-shared-case-backdrop" role="presentation">
      <section className="bw-shared-case-panel" role="dialog" aria-modal="true" aria-labelledby="bw-shared-case-title">
        <header className="bw-shared-case-panel__header">
          <div>
            <span className="bw-shared-case-panel__icon"><Library size={18} /></span>
            <div>
              <strong id="bw-shared-case-title">共享案例库</strong>
              <small>{isAdmin ? "发布、授权和撤回内部案例" : "查看已授权给你的案例"}</small>
            </div>
          </div>
          <div>
            <button type="button" className="bw-icon-button" onClick={onRefresh} disabled={loading || Boolean(busyAction)} aria-label="刷新共享案例" title="刷新共享案例">
              <RefreshCw size={15} className={loading ? "bw-spin" : undefined} />
            </button>
            <button type="button" className="bw-icon-button" onClick={onClose} disabled={Boolean(busyAction)} aria-label="关闭共享案例" title="关闭共享案例">
              <X size={17} />
            </button>
          </div>
        </header>

        {error ? <div className="bw-shared-case-message is-error" role="alert">{error}</div> : null}
        {notice ? <div className="bw-shared-case-message is-success" role="status">{notice}</div> : null}
        {isAdmin ? (
          <div className="bw-shared-case-event-sequence" aria-label="共享案例事件序列">
            管理事件序列：{lastEventSequence === null ? "暂无事件" : `#${lastEventSequence}`}
          </div>
        ) : null}

        <div className="bw-shared-case-panel__body">
          {isAdmin ? (
            <section className="bw-shared-case-publish" aria-labelledby="bw-shared-case-publish-title">
              <header>
                <div>
                  <strong id="bw-shared-case-publish-title">发布本地案例</strong>
                  <small>直接复用案例库资产和现有 R2 备份链路</small>
                </div>
                <ShieldCheck size={17} />
              </header>
              <label>
                <span>选择案例</span>
                <select value={selectedCaseId} onChange={(event) => onSelectedCaseChange(event.target.value)} disabled={loading || Boolean(busyAction)}>
                  <option value="">选择一个本地案例</option>
                  {cases.map((caseRecord) => (
                    <option key={caseRecord.id} value={caseRecord.id}>{caseRecord.title} · {caseRecord.clientName}</option>
                  ))}
                </select>
              </label>
              <label>
                <span>初始授权</span>
                <textarea
                  value={publishGrantText}
                  onChange={(event) => onPublishGrantTextChange(event.target.value)}
                  rows={3}
                  spellCheck={false}
                  placeholder="username: discover, preview, reference"
                  disabled={Boolean(busyAction)}
                />
                <small>每行一个用户；权限支持 discover、preview、reference、download。管理员会自动保留 discover 权限。</small>
              </label>
              <button className="bw-primary-button" type="button" onClick={onPublish} disabled={!selectedCaseId || loading || Boolean(busyAction)}>
                <Send size={14} />
                {busyAction === "publish" ? "发布中…" : "发布案例"}
              </button>
            </section>
          ) : null}

          <section className="bw-shared-case-list" aria-labelledby="bw-shared-case-list-title">
            <header>
              <div>
                <strong id="bw-shared-case-list-title">{isAdmin ? "可管理案例" : "已授权案例"}</strong>
                <small>{publications.length} 个案例 · 当前用户 {currentUsername}</small>
              </div>
            </header>
            {loading && publications.length === 0 ? <div className="bw-shared-case-empty">正在读取共享案例…</div> : null}
            {!loading && publications.length === 0 ? <div className="bw-shared-case-empty">暂无可发现的共享案例</div> : null}
            {publications.map((publication) => {
              const ownGrant = publication.grants.find((grant) => grant.username === currentUsername);
              return (
                <article className="bw-shared-case-card" key={publication.id}>
                  <header>
                    <div>
                      <strong>{publication.title}</strong>
                      <small>{publication.clientName} · 修订 {publication.revision}</small>
                    </div>
                    <span className={`bw-shared-case-status is-${publication.status}`}>{SHARED_CASE_STATUS_LABELS[publication.status]}</span>
                  </header>
                  <div className="bw-shared-case-card__meta">
                    <span>发布人：{publication.publisherUsername}</span>
                    <span>SHA：{publication.contentSha256.slice(0, 12)}</span>
                  </div>
                  <div className="bw-shared-case-permissions" aria-label="当前用户权限">
                    {(ownGrant?.permissions ?? []).map((permission) => <span key={permission}>{SHARED_CASE_PERMISSION_LABELS[permission]}</span>)}
                  </div>
                  {isAdmin ? (
                    <div className="bw-shared-case-card__admin">
                      <label>
                        <span>授权清单</span>
                        <textarea
                          value={grantDrafts[publication.id] ?? formatSharedCaseGrants(publication.grants)}
                          onChange={(event) => onGrantDraftChange(publication.id, event.target.value)}
                          rows={Math.max(2, publication.grants.length)}
                          spellCheck={false}
                          disabled={Boolean(busyAction)}
                        />
                      </label>
                      <footer>
                        <button className="bw-secondary-button" type="button" onClick={() => onSaveGrants(publication)} disabled={Boolean(busyAction)}>
                          <ShieldCheck size={13} />
                          {busyAction === `grants:${publication.id}` ? "保存中…" : "保存授权"}
                        </button>
                        <button className="bw-danger-button" type="button" onClick={() => onWithdraw(publication)} disabled={Boolean(busyAction)}>
                          <Trash2 size={13} />
                          {busyAction === `withdraw:${publication.id}` ? "撤回中…" : "撤回"}
                        </button>
                      </footer>
                    </div>
                  ) : null}
                </article>
              );
            })}
          </section>
        </div>
      </section>
    </div>
  );
}

export interface SharedCaseCenterProps {
  client: BsaigcClient;
  currentUser: AppUserRecord | null;
  activeProjectId: string | null;
}

export function SharedCaseCenter({ client, currentUser, activeProjectId }: SharedCaseCenterProps) {
  const [open, setOpen] = useState(false);
  const [launcherBottom, setLauncherBottom] = useState<number | null>(null);
  const [cases, setCases] = useState<readonly CaseRecord[]>([]);
  const [publications, setPublications] = useState<readonly SharedCasePublicationRecord[]>([]);
  const [lastEventSequence, setLastEventSequence] = useState<number | null>(null);
  const [selectedCaseId, setSelectedCaseId] = useState("");
  const currentUsername = currentUser?.username ?? "local-user";
  const isAdmin = currentUser?.role === "admin";
  const [publishGrantText, setPublishGrantText] = useState(() => defaultSharedCaseGrantText(currentUsername));
  const [grantDrafts, setGrantDrafts] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    setPublishGrantText(defaultSharedCaseGrantText(currentUsername));
  }, [currentUsername]);

  useEffect(() => {
    if (typeof window === "undefined" || typeof document === "undefined") return;
    const media = window.matchMedia("(max-width: 900px)");
    const composer = document.querySelector<HTMLElement>(".bw-composer-zone");
    if (!composer) return;

    let animationFrame = 0;
    const updateLauncherBottom = () => {
      if (!media.matches) {
        setLauncherBottom(null);
        return;
      }
      const nextBottom = Math.max(12, Math.ceil(window.innerHeight - composer.getBoundingClientRect().top + 8));
      setLauncherBottom((current) => current === nextBottom ? current : nextBottom);
    };
    const scheduleUpdate = () => {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = window.requestAnimationFrame(updateLauncherBottom);
    };

    updateLauncherBottom();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(scheduleUpdate);
    observer?.observe(composer);
    media.addEventListener("change", scheduleUpdate);
    window.addEventListener("resize", scheduleUpdate);
    return () => {
      window.cancelAnimationFrame(animationFrame);
      observer?.disconnect();
      media.removeEventListener("change", scheduleUpdate);
      window.removeEventListener("resize", scheduleUpdate);
    };
  }, []);

  const loadSharedCases = async () => {
    setLoading(true);
    setError(null);
    try {
      const [authorized, localCases, events] = await Promise.all([
        client.listAuthorizedSharedCases(),
        isAdmin ? client.refreshCases() : Promise.resolve([] as readonly CaseRecord[]),
        isAdmin
          ? client.replaySharedCaseEvents(0, 1000)
          : Promise.resolve([] as readonly SharedCaseDomainEvent[]),
      ]);
      const candidates = filterPublishableSharedCaseCandidates(localCases, authorized, activeProjectId);
      setPublications(authorized);
      setCases(candidates);
      setLastEventSequence(events.length > 0 ? events[events.length - 1].sequence : null);
      setGrantDrafts(Object.fromEntries(authorized.map((publication) => [publication.id, formatSharedCaseGrants(publication.grants)])));
      setSelectedCaseId((current) => candidates.some((candidate) => candidate.id === current) ? current : (candidates[0]?.id ?? ""));
    } catch (loadError) {
      setError(sharedCaseErrorMessage(loadError));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (open) void loadSharedCases();
  }, [open, isAdmin, activeProjectId]);

  const runMutation = async (actionId: string, successMessage: string, mutation: () => Promise<unknown>) => {
    setBusyAction(actionId);
    setError(null);
    setNotice(null);
    try {
      await mutation();
      setNotice(successMessage);
      await loadSharedCases();
    } catch (mutationError) {
      setError(sharedCaseErrorMessage(mutationError));
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <>
      <button
        className="bw-shared-case-launcher"
        type="button"
        onClick={() => setOpen(true)}
        aria-label="打开共享案例库"
        style={launcherBottom === null ? undefined : {
          "--bw-shared-case-launcher-bottom": `${launcherBottom}px`,
        } as CSSProperties}
      >
        <Library size={17} />
        <span>共享案例</span>
      </button>
      <SharedCasePanel
        open={open}
        isAdmin={isAdmin}
        currentUsername={currentUsername}
        cases={cases}
        publications={publications}
        lastEventSequence={lastEventSequence}
        selectedCaseId={selectedCaseId}
        publishGrantText={publishGrantText}
        grantDrafts={grantDrafts}
        loading={loading}
        busyAction={busyAction}
        error={error}
        notice={notice}
        onClose={() => setOpen(false)}
        onRefresh={() => void loadSharedCases()}
        onSelectedCaseChange={setSelectedCaseId}
        onPublishGrantTextChange={setPublishGrantText}
        onPublish={() => void runMutation("publish", "案例已发布。", () => client.publishSharedCase({
          caseId: selectedCaseId,
          grants: ensureSharedCaseManagerGrant(parseSharedCaseGrants(publishGrantText), currentUsername),
        }, null, { projectId: activeProjectId }))}
        onGrantDraftChange={(publicationId, value) => setGrantDrafts((current) => ({ ...current, [publicationId]: value }))}
        onSaveGrants={(publication) => {
          const grants = ensureSharedCaseManagerGrant(
            parseSharedCaseGrants(grantDrafts[publication.id] ?? formatSharedCaseGrants(publication.grants)),
            currentUsername,
          );
          void runMutation(`grants:${publication.id}`, "案例授权已更新。", () => client.updateSharedCaseGrants({
            publicationId: publication.id,
            grants,
          }, publication.revision, { projectId: publication.projectId ?? activeProjectId }));
        }}
        onWithdraw={(publication) => {
          if (!window.confirm(`确认撤回共享案例“${publication.title}”？`)) return;
          void runMutation(`withdraw:${publication.id}`, "案例已撤回。", () => client.withdrawSharedCase({
            publicationId: publication.id,
          }, publication.revision, { projectId: publication.projectId ?? activeProjectId }));
        }}
      />
    </>
  );
}
