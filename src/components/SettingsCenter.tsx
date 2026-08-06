import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import type { AiProviderConnectionState } from "../generated/bsaigc/AiProviderConnectionState";
import type { AiProviderKind } from "../generated/bsaigc/AiProviderKind";
import type { ChannelAdapterState } from "../generated/bsaigc/ChannelAdapterState";
import type { StorageLocationTarget } from "../generated/bsaigc/StorageLocationTarget";
import {
  AlertTriangle,
  Bot,
  Check,
  CheckCircle2,
  Cloud,
  CloudOff,
  Database,
  Download,
  Eye,
  EyeOff,
  FolderOpen,
  HardDrive,
  Info,
  LoaderCircle,
  MessageSquare,
  Plus,
  RefreshCw,
  Save,
  Server,
  LogOut,
  Sparkles,
  Star,
  Trash2,
  UserRound,
  UserRoundPlus,
  X,
  XCircle,
  Zap,
  type LucideIcon,
} from "lucide-react";
import type { AppUserRole } from "../generated/bsaigc/AppUserRole";
import type { AuthStatus } from "../generated/bsaigc/AuthStatus";
import type { AuthUsersSnapshot } from "../generated/bsaigc/AuthUsersSnapshot";
import { PRODUCT_LOGO_PATH, PRODUCT_NAME } from "../brand";
import { AUTH_ROLE_LABELS, AUTH_SYNC_LABELS, localizeAuthError } from "./authText";
import "./SettingsCenter.css";

export type SettingsCategoryId = "account" | "ai" | "channels" | "storage" | "backup" | "updates";
export type ProviderConnectionState = AiProviderConnectionState | "testing";

export interface ProviderKindOption { value: AiProviderKind; label: string; }
export interface AiProviderSettings {
  id: string;
  name: string;
  providerKind: AiProviderKind;
  baseUrl: string;
  models: string[];
  defaultModel: string;
  isDefault: boolean;
  apiKeyConfigured: boolean;
  apiKeyHint?: string | null;
  connectionState: ProviderConnectionState;
  connectionMessage?: string | null;
  lastTestedAt?: string | null;
}
export interface AiProviderInput {
  name: string;
  providerKind: AiProviderKind;
  baseUrl: string;
  apiKey?: string;
  clearApiKey: boolean;
  models: string[];
  defaultModel: string;
}
export interface ProviderConnectionTestResult {
  state: Exclude<ProviderConnectionState, "untested" | "testing">;
  message: string;
  checkedAt?: string | null;
  providerId?: string;
  models?: string[];
}
export interface FeishuChannelStatus {
  state: ChannelAdapterState;
  cliDetected: boolean;
  version?: string | null;
  authorized: boolean;
  agentDiscoverable: boolean;
  detail?: string | null;
  lastCheckedAt?: string | null;
}
export type StorageLocationKind = "ledger" | "vault" | "cache" | "staging" | "credentials" | "other";
export interface StorageLocation {
  id: StorageLocationTarget;
  label: string;
  path: string;
  sizeBytes: number;
  kind: StorageLocationKind;
  authoritative: boolean;
  exists?: boolean;
  description?: string | null;
}
export interface CacheCleanupTarget {
  id: StorageLocationTarget;
  label: string;
  path: string;
  sizeBytes: number;
  enabled: boolean;
  selectedByDefault?: boolean;
}
export type R2BackupState = "not_configured" | "adapter_pending" | "idle" | "syncing" | "degraded" | "disabled";
export interface R2BackupStatus {
  state: R2BackupState;
  configured: boolean;
  pendingItems: number;
  lastBackupAt?: string | null;
  destinationLabel?: string | null;
  detail?: string | null;
}
export type UpdateCheckState = "idle" | "checking" | "up_to_date" | "available" | "failed";
export interface DesktopUpdateStatus {
  appVersion: string;
  buildChannel: "stable" | "development" | string;
  buildVersion?: string | null;
  codexVersion: string;
  updateSource?: string | null;
  updateSourceConfigured: boolean;
  automaticInstallAllowed?: boolean;
  checkState: UpdateCheckState;
  latestVersion?: string | null;
  downloadUrl?: string | null;
  checkedAt?: string | null;
  message?: string | null;
}
export interface SettingsCenterProps {
  open: boolean;
  initialCategory?: SettingsCategoryId;
  providers: AiProviderSettings[];
  providerKinds?: ProviderKindOption[];
  providerBusy?: boolean;
  providerError?: string | null;
  onClose: () => void;
  onCreateProvider: (input: AiProviderInput) => string | void | Promise<string | void>;
  onUpdateProvider: (providerId: string, input: AiProviderInput) => void | Promise<void>;
  onDeleteProvider: (providerId: string) => void | Promise<void>;
  onSetDefaultProvider: (providerId: string) => void | Promise<void>;
  onTestProviderConnection: (providerId: string | null, input: AiProviderInput) => ProviderConnectionTestResult | Promise<ProviderConnectionTestResult>;
  feishuChannel: FeishuChannelStatus;
  onRefreshFeishuChannel?: () => void | Promise<void>;
  onOpenFeishuSetup?: () => void | Promise<void>;
  storageLocations: StorageLocation[];
  cacheTargets: CacheCleanupTarget[];
  storageTotalBytes?: number;
  storageBusy?: boolean;
  onOpenStorageLocation: (locationId: StorageLocationTarget) => void | Promise<void>;
  onClearCache: (
    targetIds: StorageLocationTarget[],
  ) => void | number | Promise<void | number>;
  r2Backup: R2BackupStatus;
  onOpenR2Settings?: () => void | Promise<void>;
  update: DesktopUpdateStatus;
  onCheckForUpdates: () => void | Promise<void>;
  authStatus?: AuthStatus | null;
  onLogout?: () => void | Promise<void>;
  onAuthChangePassword?: (oldPassword: string, newPassword: string) => Promise<AuthStatus>;
  onAuthListUsers?: () => Promise<AuthUsersSnapshot>;
  onAuthCreateUser?: (
    username: string,
    password: string,
    role: AppUserRole,
  ) => Promise<AuthUsersSnapshot>;
  onAuthResetPassword?: (username: string, newPassword: string) => Promise<AuthUsersSnapshot>;
  onAuthDeleteUser?: (username: string) => Promise<AuthUsersSnapshot>;
  onAuthRefreshRegistry?: () => Promise<AuthStatus>;
}

interface ProviderDraft {
  name: string;
  providerKind: AiProviderKind;
  baseUrl: string;
  apiKey: string;
  clearApiKey: boolean;
  models: string[];
  defaultModel: string;
}
interface NavigationItem { id: SettingsCategoryId; label: string; icon: LucideIcon; }

const DEFAULT_PROVIDER_KINDS: ProviderKindOption[] = [
  { value: "openAiCompatible", label: "OpenAI \u517c\u5bb9" },
];
const NAVIGATION_ITEMS: NavigationItem[] = [
  { id: "account", label: "账号与用户", icon: UserRound },
  { id: "ai", label: "AI 服务", icon: Bot },
  { id: "channels", label: "渠道", icon: MessageSquare },
  { id: "storage", label: "存储与缓存", icon: HardDrive },
  { id: "backup", label: "云备份", icon: Cloud },
  { id: "updates", label: "更新与关于", icon: Info },
];
const SETTINGS_FOCUSABLE_SELECTOR = [
  "button:not([disabled])",
  "[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

function settingsFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(SETTINGS_FOCUSABLE_SELECTOR),
  ).filter(
    (element) =>
      element.tabIndex >= 0 &&
      element.getAttribute("aria-hidden") !== "true" &&
      !element.closest("[inert]"),
  );
}

function emptyProviderDraft(providerKind: AiProviderKind): ProviderDraft {
  return { name: "", providerKind, baseUrl: "https://api.openai.com/v1", apiKey: "", clearApiKey: false, models: [], defaultModel: "" };
}
function providerToDraft(provider: AiProviderSettings): ProviderDraft {
  return { name: provider.name, providerKind: provider.providerKind, baseUrl: provider.baseUrl, apiKey: "", clearApiKey: false, models: [...provider.models], defaultModel: provider.defaultModel };
}
export function syncProviderBaseUrlDraft<T extends { baseUrl: string }>(
  event: { currentTarget: { value: string } },
  updateDraft: (update: (current: T) => T) => void,
) {
  const baseUrl = event.currentTarget.value;
  updateDraft((current) => current.baseUrl === baseUrl ? current : { ...current, baseUrl });
}
function providerDraftToInput(draft: ProviderDraft): AiProviderInput {
  const models = [...new Set(draft.models.map((model) => model.trim()).filter(Boolean))];
  return { name: draft.name.trim(), providerKind: draft.providerKind, baseUrl: draft.baseUrl.trim().replace(/\/+$/, ""), apiKey: draft.apiKey.trim() || undefined, clearApiKey: draft.clearApiKey, models, defaultModel: draft.defaultModel.trim() };
}
function providerFingerprint(provider: AiProviderSettings | null): string {
  if (!provider) return "";
  return JSON.stringify([provider.id, provider.name, provider.providerKind, provider.baseUrl, provider.models, provider.defaultModel, provider.apiKeyConfigured]);
}
function validateProviderConnection(input: AiProviderInput, hasStoredKey: boolean): string | null {
  if (!input.baseUrl) return "请输入 Base URL";
  try {
    const parsed = new URL(input.baseUrl);
    const isLocal = ["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname);
    if (parsed.protocol !== "https:" && !(parsed.protocol === "http:" && isLocal)) return "远程服务必须使用 HTTPS，本地服务可使用 HTTP";
    if (parsed.username || parsed.password || parsed.search) return "Base URL 不能包含账号、密码或查询参数";
  } catch { return "Base URL 格式不正确"; }
  if (!hasStoredKey && !input.apiKey && !input.clearApiKey) return "请输入 API Key";
  return null;
}
function validateProvider(input: AiProviderInput, hasStoredKey: boolean): string | null {
  if (!input.name) return "请输入服务名称";
  const connectionError = validateProviderConnection(input, hasStoredKey);
  if (connectionError) return connectionError;
  if (input.models.length === 0) return "至少添加一个模型";
  if (!input.defaultModel || !input.models.includes(input.defaultModel)) return "请选择列表内的默认模型";
  return null;
}
export function formatSettingsBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const unitIndex = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** unitIndex;
  return `${value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}
function formatTime(value?: string | null): string {
  if (!value) return "尚未记录";
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) return value;
  return new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hour12: false }).format(timestamp);
}
function errorMessage(error: unknown): string { return error instanceof Error ? error.message : String(error); }
function StatusPill({ tone, children }: { tone: "neutral" | "success" | "warning" | "danger" | "progress"; children: ReactNode; }) {
  return <span className={`settings-center__status is-${tone}`}>{children}</span>;
}
function Section({ title, description, action, children }: { title: string; description?: string; action?: ReactNode; children: ReactNode; }) {
  return <section className="settings-center__section"><div className="settings-center__section-heading"><div><h3>{title}</h3>{description ? <p>{description}</p> : null}</div>{action ? <div className="settings-center__section-action">{action}</div> : null}</div>{children}</section>;
}
function Card({ className = "", children }: { className?: string; children: ReactNode; }) {
  return <div className={`settings-center__card ${className}`.trim()}>{children}</div>;
}
function providerConnectionPresentation(state: ProviderConnectionState): { label: string; tone: "neutral" | "success" | "warning" | "danger" | "progress" } {
  switch (state) {
    case "ready": return { label: "可用", tone: "success" };
    case "warning": return { label: "需确认", tone: "warning" };
    case "failed": return { label: "连接失败", tone: "danger" };
    case "testing": return { label: "检查中", tone: "progress" };
    default: return { label: "未测试", tone: "neutral" };
  }
}
function channelPresentation(state: ChannelAdapterState): { label: string; tone: "neutral" | "success" | "warning" | "danger" } {
  switch (state) {
    case "available": return { label: "可用", tone: "success" };
    case "configured": return { label: "已配置", tone: "success" };
    case "degraded": return { label: "需处理", tone: "warning" };
    default: return { label: "接口预留", tone: "neutral" };
  }
}
function backupPresentation(state: R2BackupState): { label: string; tone: "neutral" | "success" | "warning" | "progress" } {
  switch (state) {
    case "idle": return { label: "已就绪", tone: "success" };
    case "adapter_pending": return { label: "接口待接入", tone: "warning" };
    case "syncing": return { label: "备份中", tone: "progress" };
    case "degraded": return { label: "备份异常", tone: "warning" };
    case "disabled": return { label: "已停用", tone: "neutral" };
    default: return { label: "未配置", tone: "neutral" };
  }
}
function storageIcon(kind: StorageLocationKind) {
  if (kind === "ledger") return <Database size={17} />;
  if (kind === "vault") return <Server size={17} />;
  return <HardDrive size={17} />;
}

export function SettingsCenter(props: SettingsCenterProps) {
  const providerKinds = props.providerKinds && props.providerKinds.length > 0 ? props.providerKinds : DEFAULT_PROVIDER_KINDS;
  const firstProvider = props.providers.find((provider) => provider.isDefault) ?? props.providers[0] ?? null;
  const [activeCategory, setActiveCategory] = useState<SettingsCategoryId>(props.initialCategory ?? "ai");
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(firstProvider?.id ?? null);
  const [providerMode, setProviderMode] = useState<"create" | "edit">(firstProvider ? "edit" : "create");
  const [draft, setDraft] = useState<ProviderDraft>(() => firstProvider ? providerToDraft(firstProvider) : emptyProviderDraft(providerKinds[0]?.value ?? "openAiCompatible"));
  const [modelInput, setModelInput] = useState("");
  const [showApiKey, setShowApiKey] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionNotice, setActionNotice] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<ProviderConnectionTestResult | null>(null);
  const [providerToDelete, setProviderToDelete] = useState<string | null>(null);
  const backdropRef = useRef<HTMLDivElement>(null);
  const dialogRef = useRef<HTMLElement>(null);
  const restoreFocusRef = useRef<HTMLElement | null>(null);
  const cacheTargetSignature = props.cacheTargets.map((target) => `${target.id}:${target.enabled}:${target.selectedByDefault ?? false}`).join("|");
  const [selectedCacheIds, setSelectedCacheIds] = useState<StorageLocationTarget[]>(() => props.cacheTargets.filter((target) => target.enabled && target.selectedByDefault !== false).map((target) => target.id));
  const selectedProvider = useMemo(() => props.providers.find((provider) => provider.id === selectedProviderId) ?? null, [props.providers, selectedProviderId]);
  const selectedProviderFingerprint = providerFingerprint(selectedProvider);
  const providerInput = useMemo(() => providerDraftToInput(draft), [draft]);
  const totalStorageBytes = useMemo(() => props.storageTotalBytes ?? props.storageLocations.reduce((total, location) => total + location.sizeBytes, 0), [props.storageLocations, props.storageTotalBytes]);
  const selectedCacheBytes = useMemo(() => props.cacheTargets.filter((target) => selectedCacheIds.includes(target.id)).reduce((total, target) => total + target.sizeBytes, 0), [props.cacheTargets, selectedCacheIds]);

  useEffect(() => { if (props.open) setActiveCategory(props.initialCategory ?? "ai"); }, [props.initialCategory, props.open]);
  useEffect(() => {
    if (!props.open) return;

    const dialog = dialogRef.current;
    const backdrop = backdropRef.current;
    const activeElement = document.activeElement;
    if (activeElement instanceof HTMLElement && !dialog?.contains(activeElement)) {
      restoreFocusRef.current = activeElement;
    }

    const backgroundStates = backdrop?.parentElement
      ? Array.from(backdrop.parentElement.children)
          .filter(
            (element): element is HTMLElement =>
              element instanceof HTMLElement && element !== backdrop,
          )
          .map((element) => ({
            element,
            hadInert: element.hasAttribute("inert"),
            ariaHidden: element.getAttribute("aria-hidden"),
          }))
      : [];

    for (const { element } of backgroundStates) {
      element.setAttribute("inert", "");
      element.setAttribute("aria-hidden", "true");
    }

    const frame = window.requestAnimationFrame(() => {
      const currentDialog = dialogRef.current;
      if (!currentDialog) return;
      const initialFocus =
        currentDialog.querySelector<HTMLElement>(
          "[data-settings-initial-focus]",
        ) ?? settingsFocusableElements(currentDialog)[0] ?? currentDialog;
      initialFocus.focus({ preventScroll: true });
    });

    return () => {
      window.cancelAnimationFrame(frame);
      for (const { element, hadInert, ariaHidden } of backgroundStates) {
        if (!hadInert) element.removeAttribute("inert");
        if (ariaHidden === null) element.removeAttribute("aria-hidden");
        else element.setAttribute("aria-hidden", ariaHidden);
      }

      const restoreTarget = restoreFocusRef.current;
      restoreFocusRef.current = null;
      if (restoreTarget?.isConnected) {
        restoreTarget.focus({ preventScroll: true });
      }
    };
  }, [props.open]);
  useEffect(() => {
    if (providerMode === "edit" && selectedProvider) {
      setDraft(providerToDraft(selectedProvider)); setModelInput(""); setFormError(null); setTestResult(null);
    }
  }, [providerMode, selectedProviderFingerprint]);
  useEffect(() => {
    if (providerMode === "create" || selectedProvider) return;
    const fallback = props.providers.find((provider) => provider.isDefault) ?? props.providers[0] ?? null;
    if (fallback) { setSelectedProviderId(fallback.id); setProviderMode("edit"); }
    else { setSelectedProviderId(null); setProviderMode("create"); setDraft(emptyProviderDraft(providerKinds[0]?.value ?? "openAiCompatible")); }
  }, [providerKinds, providerMode, props.providers, selectedProvider]);
  useEffect(() => {
    const validIds = new Set(props.cacheTargets.filter((target) => target.enabled).map((target) => target.id));
    setSelectedCacheIds((current) => {
      const next = new Set(current.filter((id) => validIds.has(id)));
      for (const target of props.cacheTargets) if (target.enabled && target.selectedByDefault && !current.includes(target.id)) next.add(target.id);
      return [...next];
    });
  }, [cacheTargetSignature]);
  useEffect(() => {
    if (!props.open) return;
    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape" && !busyAction && !props.providerBusy && !props.storageBusy) props.onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busyAction, props.onClose, props.open, props.providerBusy, props.storageBusy]);

  if (!props.open) return null;

  const runAction = async (name: string, action: () => void | Promise<void>) => {
    if (busyAction) return;
    setBusyAction(name); setActionError(null); setActionNotice(null);
    try { await action(); } catch (error) { setActionError(errorMessage(error)); } finally { setBusyAction(null); }
  };
  const startCreateProvider = () => {
    setProviderMode("create"); setSelectedProviderId(null); setDraft(emptyProviderDraft(providerKinds[0]?.value ?? "openAiCompatible")); setModelInput(""); setShowApiKey(false); setFormError(null); setActionNotice(null); setTestResult(null); setProviderToDelete(null);
  };
  const selectProvider = (provider: AiProviderSettings) => {
    setSelectedProviderId(provider.id); setProviderMode("edit"); setDraft(providerToDraft(provider)); setModelInput(""); setShowApiKey(false); setFormError(null); setActionNotice(null); setTestResult(null); setProviderToDelete(null);
  };
  const addModel = () => {
    const model = modelInput.trim(); if (!model) return;
    setDraft((current) => current.models.includes(model) ? current : { ...current, models: [...current.models, model], defaultModel: current.defaultModel || model });
    setModelInput("");
  };
  const removeModel = (model: string) => setDraft((current) => {
    const models = current.models.filter((candidate) => candidate !== model);
    return { ...current, models, defaultModel: current.defaultModel === model ? (models[0] ?? "") : current.defaultModel };
  });
  const handleModelKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter" || event.key === ",") { event.preventDefault(); addModel(); }
  };
  const saveProvider = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const hasStoredKey = Boolean(providerMode === "edit" && selectedProvider?.apiKeyConfigured && !draft.clearApiKey);
    const validationError = validateProvider(providerInput, hasStoredKey);
    if (validationError) { setFormError(validationError); return; }
    setFormError(null); setActionError(null); setActionNotice(null); setBusyAction("provider-save");
    try {
      if (providerMode === "create") {
        const createdId = await props.onCreateProvider(providerInput);
        if (createdId) { setSelectedProviderId(createdId); setProviderMode("edit"); }
      } else if (selectedProviderId) await props.onUpdateProvider(selectedProviderId, providerInput);
      setDraft((current) => ({ ...current, apiKey: "", clearApiKey: false })); setShowApiKey(false); setActionNotice("AI 服务已保存");
    } catch (error) { setActionError(errorMessage(error)); } finally { setBusyAction(null); }
  };
  const testConnection = async () => {
    const hasStoredKey = Boolean(providerMode === "edit" && selectedProvider?.apiKeyConfigured && !draft.clearApiKey);
    const validationError = validateProviderConnection(providerInput, hasStoredKey);
    if (validationError) { setFormError(validationError); return; }
    setFormError(null); setActionError(null); setActionNotice(null); setTestResult(null); setBusyAction("provider-test");
    try {
      const result = await props.onTestProviderConnection(selectedProviderId, providerInput);
      setTestResult(result);
      const models = [...new Set((result.models ?? []).map((model) => model.trim()).filter(Boolean))];
      if (models.length > 0) {
        setDraft((current) => ({
          ...current,
          models,
          defaultModel: models.includes(current.defaultModel) ? current.defaultModel : models[0],
        }));
        setActionNotice(`已拉取 ${models.length} 个模型，请确认默认模型后保存`);
      }
    }
    catch (error) { setTestResult({ state: "failed", message: errorMessage(error) }); }
    finally { setBusyAction(null); }
  };
  const deleteProvider = async (providerId: string) => {
    await runAction(`provider-delete:${providerId}`, async () => {
      await props.onDeleteProvider(providerId); setProviderToDelete(null);
      if (selectedProviderId === providerId) {
        const fallback = props.providers.find((provider) => provider.id !== providerId) ?? null;
        if (fallback) selectProvider(fallback); else startCreateProvider();
      }
    });
  };

  const renderAiSettings = () => {
    const displayConnection = testResult ? providerConnectionPresentation(testResult.state) : providerConnectionPresentation(selectedProvider?.connectionState ?? "untested");
    const connectionMessage = testResult?.message ?? selectedProvider?.connectionMessage ?? "保存后可随时切换默认服务";
    const apiKeyConfigured = Boolean(providerMode === "edit" && selectedProvider?.apiKeyConfigured && !draft.clearApiKey);
    return (
      <Section title="AI 服务" description="统一管理供应商、密钥和默认模型。" action={
        <button type="button" className="settings-center__button is-primary" onClick={startCreateProvider}><Plus size={15} />新增服务</button>
      }>
        <div className="settings-center__provider-layout">
          <div className="settings-center__provider-list" aria-label="AI 服务列表">
            {props.providers.length === 0 ? (
              <button type="button" className="settings-center__empty-provider" onClick={startCreateProvider}>
                <Sparkles size={18} /><strong>添加第一个 AI 服务</strong><span>支持 OpenAI 兼容接口</span>
              </button>
            ) : props.providers.map((provider) => {
              const connection = providerConnectionPresentation(provider.connectionState);
              const active = providerMode === "edit" && provider.id === selectedProviderId;
              return (
                <div key={provider.id} className={`settings-center__provider-item${active ? " is-active" : ""}`}>
                  <button type="button" onClick={() => selectProvider(provider)}>
                    <span className="settings-center__provider-mark"><Bot size={16} /></span>
                    <span className="settings-center__provider-copy">
                      <strong>{provider.name}{provider.isDefault ? <Star size={12} fill="currentColor" /> : null}</strong>
                      <small>{provider.defaultModel || "未选择模型"}</small>
                    </span>
                    <StatusPill tone={connection.tone}>{connection.label}</StatusPill>
                  </button>
                </div>
              );
            })}
          </div>

          <form className="settings-center__provider-editor" onSubmit={saveProvider}>
            <div className="settings-center__editor-header">
              <div><span className="settings-center__eyebrow">{providerMode === "create" ? "NEW PROVIDER" : "PROVIDER"}</span><h4>{providerMode === "create" ? "新增 AI 服务" : selectedProvider?.name}</h4></div>
              <StatusPill tone={displayConnection.tone}>{displayConnection.label}</StatusPill>
            </div>
            <div className="settings-center__field-grid">
              <label className="settings-center__field">
                <span>服务名称</span>
                <input value={draft.name} onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))} placeholder="例如：华邦互娱 AI" autoComplete="off" />
              </label>
              <label className="settings-center__field">
                <span>接口类型</span>
                <select value={draft.providerKind} onChange={(event) => setDraft((current) => ({ ...current, providerKind: event.target.value as AiProviderKind }))}>
                  {providerKinds.map((kind) => <option key={kind.value} value={kind.value}>{kind.label}</option>)}
                </select>
              </label>
              <label className="settings-center__field is-wide">
                <span>Base URL</span>
                <input value={draft.baseUrl} onInput={(event) => syncProviderBaseUrlDraft(event, setDraft)} onChange={(event) => syncProviderBaseUrlDraft(event, setDraft)} placeholder="https://api.example.com/v1" inputMode="url" spellCheck={false} />
              </label>
              <label className="settings-center__field is-wide">
                <span>API Key{apiKeyConfigured ? <small>{selectedProvider?.apiKeyHint || "已安全保存"}</small> : null}</span>
                <div className="settings-center__secret-input">
                  <input type={showApiKey ? "text" : "password"} value={draft.apiKey} onChange={(event) => setDraft((current) => ({ ...current, apiKey: event.target.value, clearApiKey: false }))} placeholder={apiKeyConfigured ? "留空则继续使用已保存密钥" : "输入 API Key"} autoComplete="new-password" spellCheck={false} />
                  <button type="button" onClick={() => setShowApiKey((visible) => !visible)} aria-label={showApiKey ? "隐藏 API Key" : "显示 API Key"} title={showApiKey ? "隐藏 API Key" : "显示 API Key"}>{showApiKey ? <EyeOff size={16} /> : <Eye size={16} />}</button>
                </div>
                {selectedProvider?.apiKeyConfigured ? (
                  <button type="button" className={`settings-center__inline-link${draft.clearApiKey ? " is-danger" : ""}`} onClick={() => setDraft((current) => ({ ...current, apiKey: "", clearApiKey: !current.clearApiKey }))}>
                    {draft.clearApiKey ? "将移除已保存密钥，点击撤销" : "移除已保存密钥"}
                  </button>
                ) : null}
              </label>
            </div>

            <div className="settings-center__model-editor">
              <div className="settings-center__field-label"><span>模型列表</span><small>可手动添加或从服务拉取</small></div>
              <div className="settings-center__model-entry">
                <input value={modelInput} onChange={(event) => setModelInput(event.target.value)} onKeyDown={handleModelKeyDown} placeholder="例如：gpt-4.1" spellCheck={false} />
                <button type="button" onClick={addModel} disabled={!modelInput.trim()}><Plus size={15} />添加</button>
              </div>
              <div className="settings-center__model-list">
                {draft.models.length === 0 ? <span className="settings-center__empty-inline">暂未添加模型</span> : draft.models.map((model) => (
                  <span key={model} className="settings-center__model-chip">{model}<button type="button" aria-label={`移除模型 ${model}`} onClick={() => removeModel(model)}><X size={12} /></button></span>
                ))}
              </div>
            </div>

            <label className="settings-center__field">
              <span>默认模型</span>
              <select value={draft.defaultModel} onChange={(event) => setDraft((current) => ({ ...current, defaultModel: event.target.value }))} disabled={draft.models.length === 0}>
                <option value="">选择模型</option>{draft.models.map((model) => <option key={model} value={model}>{model}</option>)}
              </select>
            </label>

            <div className="settings-center__connection-line">
              {displayConnection.tone === "success" ? <CheckCircle2 size={16} /> : displayConnection.tone === "danger" ? <XCircle size={16} /> : <Zap size={16} />}
              <span>{connectionMessage}</span>
              {(testResult?.checkedAt || selectedProvider?.lastTestedAt) && <time>{formatTime(testResult?.checkedAt ?? selectedProvider?.lastTestedAt)}</time>}
            </div>
            {formError || props.providerError || actionError ? <div className="settings-center__error" role="alert"><AlertTriangle size={15} />{formError || props.providerError || actionError}</div> : null}
            {actionNotice ? <div className="settings-center__notice" role="status"><CheckCircle2 size={15} />{actionNotice}</div> : null}
            {providerToDelete && selectedProvider?.id === providerToDelete ? (
              <div className="settings-center__confirm-row"><span>删除后不会影响已生成的本地资料。</span><button type="button" onClick={() => setProviderToDelete(null)}>取消</button><button type="button" className="is-danger" onClick={() => void deleteProvider(providerToDelete)}>确认删除</button></div>
            ) : null}
            <div className="settings-center__editor-actions">
              {providerMode === "edit" && selectedProvider ? (
                <div className="settings-center__editor-secondary">
                  <button type="button" className="settings-center__button is-ghost" onClick={() => void runAction("provider-default", async () => { await props.onSetDefaultProvider(selectedProvider.id); setActionNotice("已设为默认 AI 服务"); })} title={selectedProvider.defaultModel ? undefined : "请先拉取并保存模型"} disabled={selectedProvider.isDefault || !selectedProvider.defaultModel || Boolean(busyAction)}><Star size={15} />{selectedProvider.isDefault ? "当前默认" : "设为默认"}</button>
                  <button type="button" className="settings-center__icon-button is-danger" aria-label="删除 AI 服务" title={props.providers.length <= 1 ? "至少保留一个 AI 服务" : "删除 AI 服务"} onClick={() => setProviderToDelete(selectedProvider.id)} disabled={Boolean(busyAction) || props.providers.length <= 1}><Trash2 size={15} /></button>
                </div>
              ) : <span />}
              <div className="settings-center__editor-primary">
                <button type="button" className="settings-center__button" onClick={() => void testConnection()} disabled={Boolean(busyAction) || Boolean(props.providerBusy)}>{busyAction === "provider-test" ? <LoaderCircle className="is-spinning" size={15} /> : <RefreshCw size={15} />}拉取模型</button>
                <button type="submit" className="settings-center__button is-primary" disabled={Boolean(busyAction) || Boolean(props.providerBusy)}>{busyAction === "provider-save" || props.providerBusy ? <LoaderCircle className="is-spinning" size={15} /> : <Save size={15} />}保存</button>
              </div>
            </div>
          </form>
        </div>
      </Section>
    );
  };

  const renderChannelSettings = () => {
    const status = channelPresentation(props.feishuChannel.state);
    return (
      <Section title="渠道" description="本版本仅保留渠道接口，不同步飞书业务数据。">
        <Card>
          <div className="settings-center__integration-row">
            <span className="settings-center__integration-icon is-feishu"><MessageSquare size={18} /></span>
            <div className="settings-center__integration-copy">
              <div><strong>飞书 CLI</strong><StatusPill tone={status.tone}>{status.label}</StatusPill></div>
              <p>{props.feishuChannel.detail || "等待下一版本接入完整渠道能力"}</p>
            </div>
            <div className="settings-center__integration-actions">
              {props.onRefreshFeishuChannel ? (
                <button type="button" className="settings-center__icon-button" aria-label="刷新飞书状态" title="刷新飞书状态" onClick={() => void runAction("feishu-refresh", props.onRefreshFeishuChannel!)} disabled={Boolean(busyAction)}>
                  {busyAction === "feishu-refresh" ? <LoaderCircle className="is-spinning" size={15} /> : <RefreshCw size={15} />}
                </button>
              ) : null}
              {props.onOpenFeishuSetup ? <button type="button" className="settings-center__button" onClick={() => void runAction("feishu-setup", props.onOpenFeishuSetup!)} disabled={Boolean(busyAction)}>配置入口</button> : null}
            </div>
          </div>
          <div className="settings-center__facts-grid">
            <div><span>CLI</span><strong>{props.feishuChannel.cliDetected ? props.feishuChannel.version || "已检测" : "未检测"}</strong></div>
            <div><span>授权</span><strong>{props.feishuChannel.authorized ? "已授权" : "未授权"}</strong></div>
            <div><span>Agent 发现</span><strong>{props.feishuChannel.agentDiscoverable ? "可用" : "待接入"}</strong></div>
            <div><span>最近检查</span><strong>{formatTime(props.feishuChannel.lastCheckedAt)}</strong></div>
          </div>
        </Card>
      </Section>
    );
  };

  const renderStorageSettings = () => (
    <>
      <Section title="本地存储" description={`SQLite 与本地 Vault 为数据权威，共占用 ${formatSettingsBytes(totalStorageBytes)}。`}>
        <Card><div className="settings-center__storage-list">
          {props.storageLocations.map((location) => (
            <div className="settings-center__storage-row" key={location.id}>
              <span className="settings-center__storage-icon">{storageIcon(location.kind)}</span>
              <div className="settings-center__storage-copy">
                <div><strong>{location.label}</strong>{location.authoritative ? <StatusPill tone="success">权威数据</StatusPill> : null}</div>
                <code title={location.path}>{location.path}</code>
                {location.description ? <p>{location.description}</p> : null}
              </div>
              <strong className="settings-center__storage-size">{formatSettingsBytes(location.sizeBytes)}</strong>
              <button type="button" className="settings-center__icon-button" aria-label={`打开${location.label}目录`} title="打开目录" onClick={() => void runAction(`storage-open:${location.id}`, () => props.onOpenStorageLocation(location.id))} disabled={Boolean(busyAction)}><FolderOpen size={15} /></button>
            </div>
          ))}
        </div></Card>
      </Section>
      <Section title="缓存清理" description="仅清理可重新生成的缓存，不触碰账本、原件和凭据。">
        <Card>
          <div className="settings-center__cache-list">
            {props.cacheTargets.map((target) => (
              <label key={target.id} className={`settings-center__cache-row${target.enabled ? "" : " is-disabled"}`}>
                <input type="checkbox" checked={selectedCacheIds.includes(target.id)} disabled={!target.enabled || Boolean(props.storageBusy)} onChange={(event) => setSelectedCacheIds((current) => event.target.checked ? [...new Set([...current, target.id])] : current.filter((id) => id !== target.id))} />
                <span><strong>{target.label}</strong><code title={target.path}>{target.path}</code></span>
                <b>{formatSettingsBytes(target.sizeBytes)}</b>
              </label>
            ))}
          </div>
          <div className="settings-center__cache-footer">
            <span>预计释放 {formatSettingsBytes(selectedCacheBytes)}</span>
            <button type="button" className="settings-center__button is-danger" disabled={selectedCacheIds.length === 0 || Boolean(props.storageBusy) || Boolean(busyAction)} onClick={() => void runAction("cache-clear", async () => { const freed = await props.onClearCache(selectedCacheIds); setActionNotice(typeof freed === "number" ? `缓存清理完成，已释放 ${formatSettingsBytes(freed)}` : "缓存清理完成"); })}>
              {busyAction === "cache-clear" || props.storageBusy ? <LoaderCircle className="is-spinning" size={15} /> : <Trash2 size={15} />}清理所选缓存
            </button>
          </div>
        </Card>
      </Section>
    </>
  );

  const renderBackupSettings = () => {
    const status = backupPresentation(props.r2Backup.state);
    return (
      <Section title="云备份" description="本地先落盘，R2 只做异步备份。">
        <Card className="settings-center__backup-card">
          <div className="settings-center__backup-hero">
            <span className={`settings-center__backup-icon is-${status.tone}`}>{props.r2Backup.configured ? <Cloud size={22} /> : <CloudOff size={22} />}</span>
            <div><div><strong>Cloudflare R2</strong><StatusPill tone={status.tone}>{status.label}</StatusPill></div><p>{props.r2Backup.detail || "配置壳已预留，后续接入凭据和真实同步链路。"}</p></div>
            {props.onOpenR2Settings ? <button type="button" className="settings-center__button" onClick={() => void runAction("r2-settings", props.onOpenR2Settings!)} disabled={Boolean(busyAction)}>配置</button> : null}
          </div>
          <div className="settings-center__facts-grid is-three">
            <div><span>工作模式</span><strong>仅异步备份</strong></div>
            <div><span>等待备份</span><strong>{props.r2Backup.pendingItems} 项</strong></div>
            <div><span>最近备份</span><strong>{formatTime(props.r2Backup.lastBackupAt)}</strong></div>
          </div>
          {props.r2Backup.destinationLabel ? <div className="settings-center__destination"><Server size={15} /><span>{props.r2Backup.destinationLabel}</span></div> : null}
        </Card>
      </Section>
    );
  };

  const renderUpdateSettings = () => {
    const isChecking = props.update.checkState === "checking" || busyAction === "update-check";
    const updateAvailable = props.update.checkState === "available";
    return (
      <>
        <Section title="版本">
          <Card>
            <div className="settings-center__about-head">
              <span className="settings-center__app-mark"><img src={PRODUCT_LOGO_PATH} alt="" /></span>
              <div><strong>{PRODUCT_NAME}</strong><p>桌面端商务全流程系统</p></div>
              <StatusPill tone={props.update.buildChannel === "stable" ? "success" : "warning"}>{props.update.buildChannel === "stable" ? "Stable" : "Development"}</StatusPill>
            </div>
            <div className="settings-center__version-list">
              <div><span>应用版本</span><strong>{props.update.appVersion}</strong></div>
              {props.update.buildVersion ? <div><span>构建版本</span><strong>{props.update.buildVersion}</strong></div> : null}
              <div><span>构建通道</span><strong>{props.update.buildChannel}</strong></div>
              <div><span>Codex Runtime</span><strong>{props.update.codexVersion}</strong></div>
            </div>
          </Card>
        </Section>
        <Section title="软件更新">
          <Card>
            <div className="settings-center__update-row">
              <span className="settings-center__integration-icon"><Download size={18} /></span>
              <div className="settings-center__integration-copy">
                <div><strong>{updateAvailable ? `发现 ${props.update.latestVersion || "可用更新"}` : "检查更新"}</strong><StatusPill tone={props.update.updateSourceConfigured ? "success" : "warning"}>{props.update.updateSourceConfigured ? "更新源已配置" : "更新源未配置"}</StatusPill></div>
                <p>{props.update.message || (props.update.updateSourceConfigured ? `最近检查：${formatTime(props.update.checkedAt)}` : "配置签名更新源后才会启用安装流程")}</p>
                {props.update.updateSource ? <code>{props.update.updateSource}</code> : null}
              </div>
              {updateAvailable && props.update.downloadUrl ? (
                <a
                  className="settings-center__button is-primary"
                  href={props.update.downloadUrl}
                  target="_blank"
                  rel="noreferrer"
                  onClick={() => {
                    void navigator.clipboard?.writeText(props.update.downloadUrl ?? "");
                  }}
                  title="点击在浏览器打开下载（链接已同时复制，可粘贴到任意浏览器）"
                >
                  <Download size={15} />下载安装
                </a>
              ) : null}
              <button type="button" className="settings-center__button" onClick={() => void runAction("update-check", props.onCheckForUpdates)} disabled={isChecking || Boolean(busyAction)}>
                {isChecking ? <LoaderCircle className="is-spinning" size={15} /> : <RefreshCw size={15} />}手动检查
              </button>
            </div>
          </Card>
        </Section>
      </>
    );
  };

  const renderActiveCategory = () => {
    switch (activeCategory) {
      case "account":
        return (
          <AccountPanel
            authStatus={props.authStatus ?? null}
            onLogout={props.onLogout}
            onChangePassword={props.onAuthChangePassword}
            onListUsers={props.onAuthListUsers}
            onCreateUser={props.onAuthCreateUser}
            onResetPassword={props.onAuthResetPassword}
            onDeleteUser={props.onAuthDeleteUser}
            onRefreshRegistry={props.onAuthRefreshRegistry}
          />
        );
      case "channels": return renderChannelSettings();
      case "storage": return renderStorageSettings();
      case "backup": return renderBackupSettings();
      case "updates": return renderUpdateSettings();
      default: return renderAiSettings();
    }
  };
  const activeNavigation = NAVIGATION_ITEMS.find((item) => item.id === activeCategory) ?? NAVIGATION_ITEMS[0];
  const handleDialogKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== "Tab") return;

    const dialog = dialogRef.current;
    if (!dialog) return;
    const focusable = settingsFocusableElements(dialog);
    if (focusable.length === 0) {
      event.preventDefault();
      dialog.focus({ preventScroll: true });
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const activeElement =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const outsideDialog = !activeElement || !dialog.contains(activeElement);

    if (event.shiftKey && (outsideDialog || activeElement === first)) {
      event.preventDefault();
      last.focus({ preventScroll: true });
    } else if (!event.shiftKey && (outsideDialog || activeElement === last)) {
      event.preventDefault();
      first.focus({ preventScroll: true });
    }
  };

  return (
    <div ref={backdropRef} className="settings-center__backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget && !busyAction && !props.providerBusy && !props.storageBusy) props.onClose();
    }}>
      <section ref={dialogRef} className="settings-center" role="dialog" aria-modal="true" aria-labelledby="settings-center-title" tabIndex={-1} onKeyDown={handleDialogKeyDown}>
        <header className="settings-center__header">
          <div><h2 id="settings-center-title">设置</h2><span>{activeNavigation.label}</span></div>
          <button type="button" className="settings-center__icon-button" aria-label="关闭设置" title="关闭设置" data-settings-initial-focus onClick={props.onClose} disabled={Boolean(busyAction) || Boolean(props.providerBusy) || Boolean(props.storageBusy)}><X size={16} /></button>
        </header>
        <div className="settings-center__layout">
          <nav className="settings-center__nav" aria-label="设置分类" role="tablist">
            {NAVIGATION_ITEMS.map((item) => {
              const Icon = item.icon; const active = item.id === activeCategory;
              return <button key={item.id} type="button" role="tab" aria-selected={active} aria-controls={`settings-panel-${item.id}`} className={active ? "is-active" : ""} onClick={() => { setActiveCategory(item.id); setActionError(null); }}><Icon size={16} /><span>{item.label}</span>{active ? <Check size={13} className="settings-center__nav-check" /> : null}</button>;
            })}
          </nav>
          <main id={`settings-panel-${activeCategory}`} className="settings-center__content" role="tabpanel" aria-label={activeNavigation.label}>
            {actionError && activeCategory !== "ai" ? <div className="settings-center__error is-global" role="alert"><AlertTriangle size={15} />{actionError}</div> : null}
            {renderActiveCategory()}
          </main>
        </div>
      </section>
    </div>
  );
}

interface AccountPanelProps {
  authStatus: AuthStatus | null;
  onLogout?: () => void | Promise<void>;
  onChangePassword?: (oldPassword: string, newPassword: string) => Promise<AuthStatus>;
  onListUsers?: () => Promise<AuthUsersSnapshot>;
  onCreateUser?: (username: string, password: string, role: AppUserRole) => Promise<AuthUsersSnapshot>;
  onResetPassword?: (username: string, newPassword: string) => Promise<AuthUsersSnapshot>;
  onDeleteUser?: (username: string) => Promise<AuthUsersSnapshot>;
  onRefreshRegistry?: () => Promise<AuthStatus>;
}

function AccountPanel({ authStatus, onLogout, onChangePassword, onListUsers, onCreateUser, onResetPassword, onDeleteUser, onRefreshRegistry }: AccountPanelProps) {
  const currentUser = authStatus?.currentUser ?? null;
  const isAdmin = currentUser?.role === "admin";
  const [snapshot, setSnapshot] = useState<AuthUsersSnapshot | null>(null);
  const [busy, setBusy] = useState(false);
  const [panelError, setPanelError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [oldPassword, setOldPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [newUserName, setNewUserName] = useState("");
  const [newUserPassword, setNewUserPassword] = useState("");
  const [newUserRole, setNewUserRole] = useState<AppUserRole>("member");
  const [resetTarget, setResetTarget] = useState<string | null>(null);
  const [resetValue, setResetValue] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  useEffect(() => {
    if (!isAdmin || !onListUsers) return;
    let cancelled = false;
    setBusy(true);
    onListUsers()
      .then((result) => { if (!cancelled) setSnapshot(result); })
      .catch((error: unknown) => { if (!cancelled) setPanelError(localizeAuthError(error)); })
      .finally(() => { if (!cancelled) setBusy(false); });
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isAdmin]);

  const run = async (operation: () => Promise<AuthUsersSnapshot>, successNotice: string) => {
    setBusy(true); setPanelError(null); setNotice(null);
    try {
      const result = await operation();
      setSnapshot(result);
      setNotice(successNotice);
      return true;
    } catch (error: unknown) {
      setPanelError(localizeAuthError(error));
      return false;
    } finally {
      setBusy(false);
    }
  };

  if (!authStatus || !currentUser) {
    return (
      <Section title="账号" description="当前主机版本不支持账号功能，或尚未登录。">
        <Card><p className="settings-center__hint">重启软件后从登录页进入即可。</p></Card>
      </Section>
    );
  }

  const syncTone: "neutral" | "success" | "warning" = authStatus.registrySync === "synced" ? "success" : authStatus.registrySync === "degraded" ? "warning" : "neutral";
  const syncLabel = authStatus.registrySync === "degraded" && authStatus.registryMessage ? authStatus.registryMessage : AUTH_SYNC_LABELS[authStatus.registrySync];

  return (
    <>
      <Section title="当前账号" description="登录状态与密码">
        <Card className="settings-center__account-card">
          <div className="settings-center__account-row">
            <span className="settings-center__account-avatar"><UserRound size={17} /></span>
            <div className="settings-center__account-main">
              <strong>{currentUser.username}</strong>
              <small>{AUTH_ROLE_LABELS[currentUser.role]}</small>
            </div>
            <StatusPill tone={syncTone}>{syncLabel}</StatusPill>
            {onRefreshRegistry && (
              <button type="button" className="settings-center__ghost-button" disabled={busy} onClick={() => { setBusy(true); setPanelError(null); onRefreshRegistry().catch((error: unknown) => setPanelError(localizeAuthError(error))).finally(() => setBusy(false)); }}>
                <RefreshCw size={13} />同步
              </button>
            )}
            {onLogout && (
              <button type="button" className="settings-center__ghost-button is-danger" disabled={busy} onClick={() => void onLogout()}>
                <LogOut size={13} />退出登录
              </button>
            )}
          </div>
          {onChangePassword && (
            <div className="settings-center__account-password">
              <input type="password" placeholder="旧密码" value={oldPassword} onChange={(event) => setOldPassword(event.currentTarget.value)} disabled={busy} />
              <input type="password" placeholder="新密码（至少 6 位）" value={newPassword} onChange={(event) => setNewPassword(event.currentTarget.value)} disabled={busy} />
              <button type="button" className="settings-center__ghost-button" disabled={busy || !oldPassword || !newPassword}
                onClick={() => {
                  setBusy(true); setPanelError(null); setNotice(null);
                  onChangePassword(oldPassword, newPassword)
                    .then(() => { setNotice("密码已修改"); setOldPassword(""); setNewPassword(""); })
                    .catch((error: unknown) => setPanelError(localizeAuthError(error)))
                    .finally(() => setBusy(false));
                }}>
                <Save size={13} />改密码
              </button>
            </div>
          )}
        </Card>
      </Section>

      {isAdmin && (
        <Section title="用户管理" description="加人、删人、重置密码；删除的账号在所有电脑上立即失效（需联网）">
          <Card>
            <div className="settings-center__user-add">
              <input placeholder="用户名（如：市场部小李）" value={newUserName} onChange={(event) => setNewUserName(event.currentTarget.value)} disabled={busy} />
              <input type="password" placeholder="初始密码（至少 6 位）" value={newUserPassword} onChange={(event) => setNewUserPassword(event.currentTarget.value)} disabled={busy} />
              <select value={newUserRole} onChange={(event) => setNewUserRole(event.currentTarget.value as AppUserRole)} disabled={busy} aria-label="角色">
                <option value="member">员工</option>
                <option value="admin">管理员</option>
              </select>
              <button type="button" className="settings-center__primary-button" disabled={busy || !onCreateUser || !newUserName.trim() || !newUserPassword}
                onClick={() => { if (!onCreateUser) return; void run(() => onCreateUser(newUserName.trim(), newUserPassword, newUserRole), "账号已添加").then((ok) => { if (ok) { setNewUserName(""); setNewUserPassword(""); setNewUserRole("member"); } }); }}>
                <UserRoundPlus size={14} />添加账号
              </button>
            </div>

            {panelError && <div className="settings-center__error" role="alert"><AlertTriangle size={14} />{panelError}</div>}
            {notice && !panelError && <div className="settings-center__notice" role="status"><CheckCircle2 size={14} />{notice}</div>}

            <ul className="settings-center__user-list">
              {(snapshot?.users ?? []).map((user) => (
                <li key={user.username}>
                  <span className="settings-center__account-avatar is-small"><UserRound size={13} /></span>
                  <div className="settings-center__account-main">
                    <strong>{user.username}{user.username === currentUser.username ? "（我）" : ""}</strong>
                    <small>{AUTH_ROLE_LABELS[user.role]}{user.status === "disabled" ? " · 已停用" : ""}</small>
                  </div>
                  {resetTarget === user.username ? (
                    <span className="settings-center__user-inline">
                      <input type="password" placeholder="新密码" value={resetValue} onChange={(event) => setResetValue(event.currentTarget.value)} disabled={busy} autoFocus />
                      <button type="button" className="settings-center__ghost-button" disabled={busy || !resetValue || !onResetPassword}
                        onClick={() => { if (!onResetPassword) return; void run(() => onResetPassword(user.username, resetValue), `已重置「${user.username}」的密码`).then((ok) => { if (ok) { setResetTarget(null); setResetValue(""); } }); }}>
                        确定
                      </button>
                      <button type="button" className="settings-center__ghost-button" disabled={busy} onClick={() => { setResetTarget(null); setResetValue(""); }}>取消</button>
                    </span>
                  ) : deleteTarget === user.username ? (
                    <span className="settings-center__user-inline">
                      <small className="settings-center__danger-text">确定删除？该账号将无法再登录</small>
                      <button type="button" className="settings-center__ghost-button is-danger" disabled={busy || !onDeleteUser}
                        onClick={() => { if (!onDeleteUser) return; void run(() => onDeleteUser(user.username), `已删除「${user.username}」`).then(() => setDeleteTarget(null)); }}>
                        确认删除
                      </button>
                      <button type="button" className="settings-center__ghost-button" disabled={busy} onClick={() => setDeleteTarget(null)}>取消</button>
                    </span>
                  ) : (
                    <span className="settings-center__user-inline">
                      <button type="button" className="settings-center__ghost-button" disabled={busy} onClick={() => { setResetTarget(user.username); setDeleteTarget(null); setResetValue(""); }}>重置密码</button>
                      {user.username !== currentUser.username && (
                        <button type="button" className="settings-center__ghost-button is-danger" disabled={busy} onClick={() => { setDeleteTarget(user.username); setResetTarget(null); }}>
                          <Trash2 size={13} />删除
                        </button>
                      )}
                    </span>
                  )}
                </li>
              ))}
              {snapshot && snapshot.users.length === 0 && <li className="settings-center__hint">暂无账号</li>}
              {!snapshot && busy && <li className="settings-center__hint">正在加载账号列表…</li>}
            </ul>
          </Card>
        </Section>
      )}
    </>
  );
}
