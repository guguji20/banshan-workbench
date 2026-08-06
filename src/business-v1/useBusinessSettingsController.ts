import { useCallback, useEffect, useMemo, useState } from "react";
import { normalizeHostError, type BsaigcClient } from "../client-sdk";
import type {
  AiProviderInput,
  AiProviderSettings,
  CacheCleanupTarget,
  DesktopUpdateStatus as SettingsUpdateStatus,
  FeishuChannelStatus,
  ProviderConnectionTestResult,
  R2BackupStatus,
  SettingsCategoryId,
  SettingsCenterProps,
  StorageLocation,
  StorageLocationKind,
} from "../components/SettingsCenter";
import type { AiCredentialStatus } from "../generated/bsaigc/AiCredentialStatus";
import type { DesktopSettingsSnapshot } from "../generated/bsaigc/DesktopSettingsSnapshot";
import type { StorageLocationTarget } from "../generated/bsaigc/StorageLocationTarget";

type SettingsCenterControllerProps = Omit<
  SettingsCenterProps,
  | "open"
  | "initialCategory"
  | "onClose"
  | "authStatus"
  | "onLogout"
  | "onAuthChangePassword"
  | "onAuthListUsers"
  | "onAuthCreateUser"
  | "onAuthResetPassword"
  | "onAuthDeleteUser"
  | "onAuthRefreshRegistry"
>;

export interface BusinessSettingsController {
  open: boolean;
  category: SettingsCategoryId;
  openSettings: (category?: SettingsCategoryId) => void;
  closeSettings: () => void;
  centerProps: SettingsCenterControllerProps;
}

export function useBusinessSettingsController(
  client: BsaigcClient,
  desktopRuntime: boolean,
): BusinessSettingsController {
  const [open, setOpen] = useState(false);
  const [category, setCategory] = useState<SettingsCategoryId>("ai");
  const [aiCredentialStatus, setAiCredentialStatus] = useState<AiCredentialStatus | null>(null);
  const [aiCredentialBusy, setAiCredentialBusy] = useState(false);
  const [aiCredentialError, setAiCredentialError] = useState<string | null>(null);
  const [desktopSettings, setDesktopSettings] = useState<DesktopSettingsSnapshot | null>(null);
  const [desktopSettingsBusy, setDesktopSettingsBusy] = useState(false);
  const [desktopSettingsError, setDesktopSettingsError] = useState<string | null>(null);

  const refreshAiCredential = useCallback(async () => {
    if (!desktopRuntime) return null;
    setAiCredentialBusy(true);
    try {
      const next = await client.getAiCredentialStatus();
      setAiCredentialStatus(next);
      setAiCredentialError(null);
      return next;
    } catch (error) {
      setAiCredentialError(localizeHostError(error));
      throw error;
    } finally {
      setAiCredentialBusy(false);
    }
  }, [client, desktopRuntime]);

  const refreshDesktopSettings = useCallback(async () => {
    if (!desktopRuntime) return null;
    setDesktopSettingsBusy(true);
    try {
      const next = await client.getDesktopSettingsStatus();
      setDesktopSettings(next);
      setDesktopSettingsError(null);
      return next;
    } catch (error) {
      setDesktopSettingsError(localizeHostError(error));
      throw error;
    } finally {
      setDesktopSettingsBusy(false);
    }
  }, [client, desktopRuntime]);

  useEffect(() => {
    if (!open) return;
    void Promise.allSettled([refreshAiCredential(), refreshDesktopSettings()]);
  }, [open, refreshAiCredential, refreshDesktopSettings]);

  const providers = useMemo<AiProviderSettings[]>(
    () => (aiCredentialStatus?.providers ?? []).map((provider) => ({
      id: provider.id,
      name: provider.name,
      providerKind: provider.kind,
      baseUrl: provider.baseUrl,
      models: [...provider.models],
      defaultModel: provider.defaultModel,
      isDefault: provider.isDefault,
      apiKeyConfigured: provider.apiKeyConfigured,
      apiKeyHint: provider.apiKeyHint,
      connectionState: provider.connection.state,
      connectionMessage: provider.connection.message,
      lastTestedAt: timestampToIso(provider.connection.testedAt),
    })),
    [aiCredentialStatus],
  );

  const storageLocations = useMemo<StorageLocation[]>(
    () => (desktopSettings?.storage.locations ?? []).map((location) => ({
      id: location.target,
      label: location.label || storageLocationLabel(location.target),
      path: location.path,
      sizeBytes: location.sizeBytes,
      kind: storageLocationKind(location.target),
      authoritative: location.authoritative,
      exists: location.exists,
      description: location.exists ? null : "首次使用时自动创建",
    })),
    [desktopSettings],
  );

  const cacheTargets = useMemo<CacheCleanupTarget[]>(
    () => (desktopSettings?.storage.locations ?? [])
      .filter(({ clearable }) => clearable)
      .map((location) => ({
        id: location.target,
        label: location.label || storageLocationLabel(location.target),
        path: location.path,
        sizeBytes: location.sizeBytes,
        enabled: location.target === "cache",
        selectedByDefault: location.target === "cache",
      })),
    [desktopSettings],
  );

  const persistProvider = useCallback(async (providerId: string | null, input: AiProviderInput) => {
    const current = aiCredentialStatus ?? await client.getAiCredentialStatus();
    const existingProvider = providerId
      ? current.providers.find((provider) => provider.id === providerId) ?? null
      : null;
    const previousIds = new Set(current.providers.map((provider) => provider.id));
    let next = await client.upsertProvider({
      providerId,
      name: input.name,
      kind: input.providerKind,
      baseUrl: input.baseUrl,
      apiKey: input.apiKey ?? null,
      models: input.models,
      defaultModel: input.defaultModel,
      setDefault: existingProvider?.isDefault ?? current.providers.length === 0,
      enabled: existingProvider?.enabled ?? true,
    }, current.revision);
    setAiCredentialStatus(next);

    const resolvedProviderId = providerId
      ?? next.providers.find((provider) => !previousIds.has(provider.id))?.id
      ?? null;
    if (!resolvedProviderId) throw new Error("Host 未返回新建的 AI 服务标识");

    if (input.clearApiKey) {
      next = await client.clearProviderApiKey(resolvedProviderId, next.revision);
      setAiCredentialStatus(next);
    }
    return resolvedProviderId;
  }, [aiCredentialStatus, client]);

  const runProviderAction = useCallback(async <T,>(action: () => Promise<T>): Promise<T> => {
    setAiCredentialBusy(true);
    try {
      const result = await action();
      setAiCredentialError(null);
      return result;
    } catch (error) {
      setAiCredentialError(localizeHostError(error));
      throw error;
    } finally {
      setAiCredentialBusy(false);
    }
  }, []);

  const createProvider = useCallback(
    (input: AiProviderInput) => runProviderAction(() => persistProvider(null, input)),
    [persistProvider, runProviderAction],
  );
  const updateProvider = useCallback(
    (providerId: string, input: AiProviderInput) => runProviderAction(async () => {
      await persistProvider(providerId, input);
    }),
    [persistProvider, runProviderAction],
  );
  const deleteProvider = useCallback(
    (providerId: string) => runProviderAction(async () => {
      const current = aiCredentialStatus ?? await client.getAiCredentialStatus();
      setAiCredentialStatus(await client.removeProvider(providerId, current.revision));
    }),
    [aiCredentialStatus, client, runProviderAction],
  );
  const selectProvider = useCallback(
    (providerId: string) => runProviderAction(async () => {
      const current = aiCredentialStatus ?? await client.getAiCredentialStatus();
      const provider = current.providers.find((candidate) => candidate.id === providerId);
      if (!provider) throw new Error("没有找到对应的 AI 服务");
      setAiCredentialStatus(await client.selectProvider(providerId, provider.defaultModel, current.revision));
    }),
    [aiCredentialStatus, client, runProviderAction],
  );
  const testProvider = useCallback(
    (providerId: string | null, input: AiProviderInput) => runProviderAction(async (): Promise<ProviderConnectionTestResult> => {
      const current = aiCredentialStatus ?? await client.getAiCredentialStatus();
      const response = await client.discoverProviderModels({
        providerId,
        kind: input.providerKind,
        baseUrl: input.baseUrl,
        apiKey: input.apiKey ?? null,
      }, current.revision);
      setAiCredentialStatus(response.status);
      const test = response.connectionTest;
      return {
        providerId: providerId ?? undefined,
        state: test && test.state !== "untested" ? test.state : "failed",
        message: test?.message ?? "Host 未返回连接测试结果",
        checkedAt: timestampToIso(test?.testedAt ?? null),
        models: test?.discoveredModels ?? [],
      };
    }),
    [aiCredentialStatus, client, runProviderAction],
  );

  const runDesktopAction = useCallback(async <T,>(action: () => Promise<T>): Promise<T> => {
    setDesktopSettingsBusy(true);
    try {
      const result = await action();
      setDesktopSettingsError(null);
      return result;
    } catch (error) {
      setDesktopSettingsError(localizeHostError(error));
      throw error;
    } finally {
      setDesktopSettingsBusy(false);
    }
  }, []);

  const openStorageLocation = useCallback(
    (target: StorageLocationTarget) => runDesktopAction(async () => {
      if (!desktopRuntime) return;
      const current = desktopSettings ?? await client.getDesktopSettingsStatus();
      setDesktopSettings(await client.openStorageLocation(target, current.revision));
    }),
    [client, desktopRuntime, desktopSettings, runDesktopAction],
  );
  const clearCache = useCallback(
    (targets: StorageLocationTarget[]) => runDesktopAction(async () => {
      if (!desktopRuntime) return undefined;
      if (!targets.includes("cache")) throw new Error("没有选中可清理的缓存");
      const current = desktopSettings ?? await client.getDesktopSettingsStatus();
      const response = await client.clearCache(current.revision);
      setDesktopSettings(response.snapshot);
      return response.cacheClear?.freedBytes;
    }),
    [client, desktopRuntime, desktopSettings, runDesktopAction],
  );
  const checkForUpdates = useCallback(
    () => runDesktopAction(async () => {
      if (!desktopRuntime) return;
      const current = desktopSettings ?? await client.getDesktopSettingsStatus();
      setDesktopSettings(await client.checkForUpdates(current.revision));
    }),
    [client, desktopRuntime, desktopSettings, runDesktopAction],
  );

  const centerProps = useMemo<SettingsCenterControllerProps>(() => ({
    providers,
    providerBusy: aiCredentialBusy,
    providerError: aiCredentialError,
    onCreateProvider: createProvider,
    onUpdateProvider: updateProvider,
    onDeleteProvider: deleteProvider,
    onSetDefaultProvider: selectProvider,
    onTestProviderConnection: testProvider,
    feishuChannel: mapFeishuChannel(desktopSettings, desktopSettingsError),
    onRefreshFeishuChannel: async () => {
      await refreshDesktopSettings();
    },
    storageLocations,
    cacheTargets,
    storageTotalBytes: desktopSettings?.storage.totalBytes,
    storageBusy: desktopSettingsBusy,
    onOpenStorageLocation: openStorageLocation,
    onClearCache: clearCache,
    r2Backup: mapR2Backup(desktopSettings, desktopSettingsError),
    update: mapDesktopUpdate(desktopSettings, desktopSettingsError),
    onCheckForUpdates: checkForUpdates,
  }), [
    aiCredentialBusy,
    aiCredentialError,
    cacheTargets,
    checkForUpdates,
    clearCache,
    createProvider,
    deleteProvider,
    desktopSettings,
    desktopSettingsBusy,
    desktopSettingsError,
    openStorageLocation,
    providers,
    refreshDesktopSettings,
    selectProvider,
    storageLocations,
    testProvider,
    updateProvider,
  ]);

  const openSettings = useCallback((nextCategory: SettingsCategoryId = "ai") => {
    setCategory(nextCategory);
    setOpen(true);
  }, []);
  const closeSettings = useCallback(() => setOpen(false), []);

  return { open, category, openSettings, closeSettings, centerProps };
}

function storageLocationLabel(target: StorageLocationTarget): string {
  switch (target) {
    case "ledger": return "SQLite 账本";
    case "vault": return "本地资料库";
    case "cache": return "预览与缩略图缓存";
    case "staging": return "任务暂存区";
    case "credentials": return "受保护凭据区";
    default: return "应用数据";
  }
}

function storageLocationKind(target: StorageLocationTarget): StorageLocationKind {
  if (["ledger", "vault", "cache", "staging", "credentials"].includes(target)) {
    return target as StorageLocationKind;
  }
  return "other";
}

function mapFeishuChannel(snapshot: DesktopSettingsSnapshot | null, error: string | null): FeishuChannelStatus {
  const channel = snapshot?.channelAdapters.find(({ id }) => id === "feishu-cli")
    ?? snapshot?.channelAdapters[0];
  if (!channel) {
    return {
      state: "planned",
      cliDetected: false,
      authorized: false,
      agentDiscoverable: false,
      detail: error ?? "正在读取本地渠道状态",
    };
  }
  const authorized = channel.configured
    && (channel.state === "configured" || channel.state === "available");
  return {
    state: channel.state,
    cliDetected: channel.state !== "planned",
    authorized,
    agentDiscoverable: authorized,
    detail: channel.message.trim() || "飞书 CLI 渠道接口已预留",
  };
}

function mapR2Backup(snapshot: DesktopSettingsSnapshot | null, error: string | null): R2BackupStatus {
  const backup = snapshot?.cloudBackup;
  if (!backup) {
    return {
      state: "not_configured",
      configured: false,
      pendingItems: 0,
      detail: error ?? "正在读取本地备份状态",
    };
  }
  return {
    state: backup.state === "degraded"
      ? "degraded"
      : backup.ready
        ? "idle"
        : backup.configured
          ? "adapter_pending"
          : "not_configured",
    configured: backup.configured,
    pendingItems: backup.pendingItems,
    detail: backup.message.trim() || "R2 仅作为异步备份，本地资料仍是唯一权威",
    destinationLabel: backup.configured ? "Cloudflare R2 · 异步备份" : null,
  };
}

function mapDesktopUpdate(snapshot: DesktopSettingsSnapshot | null, error: string | null): SettingsUpdateStatus {
  const update = snapshot?.update;
  if (!update) {
    return {
      appVersion: "--",
      buildVersion: null,
      buildChannel: "development",
      codexVersion: "--",
      updateSource: null,
      updateSourceConfigured: false,
      checkState: "idle",
      message: error ?? "正在读取版本状态",
    };
  }
  return {
    appVersion: update.currentVersion,
    buildVersion: update.buildVersion,
    buildChannel: update.buildChannel,
    codexVersion: update.codexRuntimeVersion,
    updateSource: null,
    updateSourceConfigured: update.updateSourceConfigured,
    automaticInstallAllowed: update.automaticInstallAllowed,
    checkState: update.state === "available"
      ? "available"
      : update.state === "failed" || update.state === "degraded"
        ? "failed"
        : update.state === "upToDate"
          ? "up_to_date"
          : "idle",
    latestVersion: update.latestVersion,
    downloadUrl: update.downloadUrl,
    checkedAt: timestampToIso(update.lastCheckedAt),
    message: update.message.trim() || (update.updateSourceConfigured ? "更新源已配置" : "签名更新源尚未配置"),
  };
}

function timestampToIso(value: number | null): string | null {
  if (value === null) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date.toISOString();
}

function localizeHostError(error: unknown): string {
  const normalized = normalizeHostError(error);
  return normalized.message || normalized.code;
}
