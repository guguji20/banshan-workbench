import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
} from "react";
import {
  BsaigcClient,
  DesktopHostAdapter,
  WebHostAdapter,
  isTauriRuntime,
  normalizeHostError,
  type AssetActionCapabilities,
} from "./client-sdk";
import {
  BusinessWorkbench,
  type DesktopSection,
} from "./components/BusinessWorkbench";
import {
  SettingsCenter,
  type AiProviderInput,
  type AiProviderSettings,
  type CacheCleanupTarget,
  type DesktopUpdateStatus as SettingsUpdateStatus,
  type FeishuChannelStatus,
  type ProviderConnectionTestResult,
  type R2BackupStatus,
  type StorageLocation,
  type StorageLocationKind,
} from "./components/SettingsCenter";
import type {
  BusinessDocumentsCenterActions,
  QuoteHistorySource,
} from "./components/BusinessDocumentsCenter";
import { AuthGate } from "./components/AuthGate";
import { localizeAuthError } from "./components/authText";
import { businessWorkspaceAssetIds } from "./businessWorkspaceAssetIds";
import type {
  AssetProjectFilter,
  AssetVaultViewMode,
} from "./components/AssetVault";
import type { TaskStatusFilter } from "./components/TaskCenter";
import type { BriefRecord } from "./generated/bsaigc/BriefRecord";
import type { CodexProbeStatus } from "./generated/bsaigc/CodexProbeStatus";
import type { CreateProjectPayload } from "./generated/bsaigc/CreateProjectPayload";
import type { AuthStatus } from "./generated/bsaigc/AuthStatus";
import type { HostError } from "./generated/bsaigc/HostError";
import type { HostStatus } from "./generated/bsaigc/HostStatus";
import type { AiCredentialStatus } from "./generated/bsaigc/AiCredentialStatus";
import type { DesktopSettingsSnapshot } from "./generated/bsaigc/DesktopSettingsSnapshot";
import type { StorageLocationTarget } from "./generated/bsaigc/StorageLocationTarget";
import type { ProjectStage } from "./generated/bsaigc/ProjectStage";
import type { AssetSourceSelection } from "./generated/bsaigc/AssetSourceSelection";
import type { TaskRecord } from "./generated/bsaigc/TaskRecord";
import type { AssetBackupRecord } from "./generated/bsaigc/AssetBackupRecord";
import type { BusinessCustomerReceivableSummary } from "./generated/bsaigc/BusinessCustomerReceivableSummary";
import type { ContractReviewRecord } from "./generated/bsaigc/ContractReviewRecord";
import type { EvidenceContext } from "./generated/bsaigc/EvidenceContext";
import type { ReviewFindingDecision } from "./generated/bsaigc/ReviewFindingDecision";
import type { ReviewFindingRecord } from "./generated/bsaigc/ReviewFindingRecord";
import type { ReviewReportFormat } from "./generated/bsaigc/ReviewReportFormat";
import type { BrainHostHealth } from "./generated/bsaigc/BrainHostHealth";
import type { NativeMediaHealth } from "./generated/bsaigc/NativeMediaHealth";
import type { CaseRecord } from "./generated/bsaigc/CaseRecord";
import type { ExecutionBriefContent } from "./generated/bsaigc/ExecutionBriefContent";
import type { ExecutionBriefRecord } from "./generated/bsaigc/ExecutionBriefRecord";
import type { ExecutionBriefStatus } from "./generated/bsaigc/ExecutionBriefStatus";
import type { RequirementAnswerInput } from "./generated/bsaigc/RequirementAnswerInput";
import type { RequirementBriefRecord } from "./generated/bsaigc/RequirementBriefRecord";
import type { RequirementBriefStatus } from "./generated/bsaigc/RequirementBriefStatus";
import type {
  CaseEditorState,
  CaseLibraryFilters,
  CaseLibraryViewMode,
} from "./components/CaseLibrary";
import {
  cloneExecutionBriefContent,
  editExecutionBriefDraft,
  executionBriefSourceKey,
  sameExecutionBriefContent,
  settleExecutionBriefDraft,
  syncExecutionBriefDraft,
  type ExecutionBriefDrafts,
} from "./executionBriefDrafts";
import {
  cloneRequirementBriefDraft,
  editRequirementBriefDraft,
  hasRequirementBriefConflict,
  rebaseRequirementBriefDraft,
  reloadRequirementBriefDraft,
  requirementExpectedRevision,
  settleRequirementBriefDraft,
  syncRequirementBriefDraft,
  type RequirementBriefDraft,
  type RequirementBriefDrafts,
} from "./requirementBriefDrafts";
import { prefillExecutionBrief } from "./executionBriefPrefill";
import { latestCustomerWorkspace } from "./components/businessReceivables";
import {
  buildBrainModelOptions,
  normalizeBrainModelSelection,
} from "./brainModelOptions";
import "./App.css";

const desktopRuntime = isTauriRuntime();
const hostAdapter = desktopRuntime
  ? new DesktopHostAdapter()
  : new WebHostAdapter();
const client = new BsaigcClient(hostAdapter, {
  actorId: "local-operator",
  windowId: "main",
});

const EMPTY_PROJECT_DRAFT: CreateProjectPayload = {
  name: "",
  clientName: "",
};


const EMPTY_CASE_FILTERS: CaseLibraryFilters = {
  search: "",
  clientName: "all",
  contentType: "all",
  presentation: "all",
  hasActors: "all",
  isAigc: "all",
  qualityTier: "all",
};

function App() {
  const [authStatus, setAuthStatus] = useState<AuthStatus | null>(null);
  const [authChecked, setAuthChecked] = useState(!desktopRuntime);
  const [authBusy, setAuthBusy] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);

  useEffect(() => {
    if (!desktopRuntime) return;
    let cancelled = false;
    client
      .authStatus()
      .then((status) => {
        if (!cancelled) setAuthStatus(status);
      })
      .catch(() => {
        // Older host without auth commands: skip the gate entirely.
      })
      .finally(() => {
        if (!cancelled) setAuthChecked(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const runAuth = async (operation: () => Promise<AuthStatus>) => {
    setAuthBusy(true);
    setAuthError(null);
    try {
      setAuthStatus(await operation());
    } catch (error: unknown) {
      setAuthError(localizeAuthError(error));
    } finally {
      setAuthBusy(false);
    }
  };

  const handleAuthLogin = (username: string, password: string) =>
    void runAuth(() => client.authLogin({ username, password }));
  const handleAuthInitialize = (username: string, password: string) =>
    void runAuth(() => client.authInitializeAdmin({ username, password }));
  const handleAuthLogout = () => void runAuth(() => client.authLogout());

  const snapshot = useSyncExternalStore(
    client.subscribe,
    client.getSnapshot,
    client.getSnapshot,
  );
  const [activeSection, setActiveSection] =
    useState<DesktopSection>("workspace");
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(
    null,
  );
  const [projectQuery, setProjectQuery] = useState("");
  const [createProjectDraft, setCreateProjectDraft] =
    useState<CreateProjectPayload>(EMPTY_PROJECT_DRAFT);
  const [briefDraft, setBriefDraft] = useState<BriefRecord | null>(null);
  const [hostStatus, setHostStatus] = useState<HostStatus | null>(null);
  const [codexStatus, setCodexStatus] = useState<CodexProbeStatus | null>(null);
  const [aiCredentialStatus, setAiCredentialStatus] =
    useState<AiCredentialStatus | null>(null);
  const [aiCredentialBusy, setAiCredentialBusy] = useState(false);
  const [aiCredentialError, setAiCredentialError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [desktopSettings, setDesktopSettings] =
    useState<DesktopSettingsSnapshot | null>(null);
  const [desktopSettingsBusy, setDesktopSettingsBusy] = useState(false);
  const [desktopSettingsError, setDesktopSettingsError] =
    useState<string | null>(null);
  const [isCreatingProject, setIsCreatingProject] = useState(false);
  const [isSavingBrief, setIsSavingBrief] = useState(false);
  const [isChangingStage, setIsChangingStage] = useState(false);
  const [isProbingCodex, setIsProbingCodex] = useState(false);
  const [taskStatusFilter, setTaskStatusFilter] =
    useState<TaskStatusFilter>("all");
  const [busyTaskIds, setBusyTaskIds] = useState<string[]>([]);
  const [isRefreshingTasks, setIsRefreshingTasks] = useState(false);
  const [assetProjectFilter, setAssetProjectFilter] =
    useState<AssetProjectFilter>("all");
  const [assetViewMode, setAssetViewMode] =
    useState<AssetVaultViewMode>("list");
  const [selectedAssetSource, setSelectedAssetSource] =
    useState<AssetSourceSelection | null>(null);
  const [importProjectId, setImportProjectId] = useState<string | null>(null);
  const [isRefreshingAssets, setIsRefreshingAssets] = useState(false);
  const [isSelectingAssetSource, setIsSelectingAssetSource] = useState(false);
  const [isImportingAsset, setIsImportingAsset] = useState(false);
  const [brainHealth, setBrainHealth] = useState<BrainHostHealth | null>(null);
  const [mediaHealth, setMediaHealth] = useState<NativeMediaHealth | null>(null);
  const [selectedBrainThreadId, setSelectedBrainThreadId] = useState<string | null>(null);
  const [selectedBrainModel, setSelectedBrainModel] = useState("default");
  const [brainDraft, setBrainDraft] = useState("");
  const [isLoadingBrainThreads, setIsLoadingBrainThreads] = useState(false);
  const [isLoadingBrainTurns, setIsLoadingBrainTurns] = useState(false);
  const [isStartingBrainThread, setIsStartingBrainThread] = useState(false);
  const [isSendingBrainTurn, setIsSendingBrainTurn] = useState(false);
  const [caseFilters, setCaseFilters] =
    useState<CaseLibraryFilters>(EMPTY_CASE_FILTERS);
  const [caseViewMode, setCaseViewMode] =
    useState<CaseLibraryViewMode>("list");
  const [caseEditor, setCaseEditor] = useState<CaseEditorState | null>(null);
  const [isRefreshingCases, setIsRefreshingCases] = useState(false);
  const [isSavingCase, setIsSavingCase] = useState(false);
  const [executionBriefDrafts, setExecutionBriefDrafts] =
    useState<ExecutionBriefDrafts>({});
  const [isRefreshingExecutionBriefs, setIsRefreshingExecutionBriefs] =
    useState(false);
  const [isSavingExecutionBrief, setIsSavingExecutionBrief] = useState(false);
  const [requirementBriefDrafts, setRequirementBriefDrafts] =
    useState<RequirementBriefDrafts>({});
  const [isRefreshingRequirementBriefs, setIsRefreshingRequirementBriefs] =
    useState(false);
  const [isSavingRequirementBrief, setIsSavingRequirementBrief] =
    useState(false);
  const [contractReviews, setContractReviews] = useState<ContractReviewRecord[]>([]);
  const [selectedContractReviewId, setSelectedContractReviewId] =
    useState<string | null>(null);
  const [selectedContractReview, setSelectedContractReview] =
    useState<ContractReviewRecord | null>(null);
  const [contractFindings, setContractFindings] = useState<ReviewFindingRecord[]>([]);
  const [selectedContractFindingId, setSelectedContractFindingId] =
    useState<string | null>(null);
  const [contractEvidence, setContractEvidence] =
    useState<EvidenceContext | null>(null);
  const [assetBackups, setAssetBackups] = useState<AssetBackupRecord[]>([]);
  const [isLoadingContractReviews, setIsLoadingContractReviews] = useState(false);
  const [contractBusyAction, setContractBusyAction] = useState<string | null>(null);
  const [businessBusyAction, setBusinessBusyAction] = useState<string | null>(null);
  const [businessCustomers, setBusinessCustomers] = useState<
    BusinessCustomerReceivableSummary[]
  >([]);
  const [businessCustomersLoading, setBusinessCustomersLoading] = useState(false);
  const [businessCustomersError, setBusinessCustomersError] =
    useState<string | null>(null);
  const [businessCustomerQuery, setBusinessCustomerQuery] = useState("");
  const [businessCustomerReloadToken, setBusinessCustomerReloadToken] =
    useState(0);
  const [assetActionCapabilities, setAssetActionCapabilities] = useState<
    Record<string, AssetActionCapabilities>
  >({});

  useEffect(() => {
    document.title = "半山商务工作台";
  }, []);

  const selectedProject = useMemo(
    () =>
      snapshot.projects.find((project) => project.id === selectedProjectId) ??
      null,
    [selectedProjectId, snapshot.projects],
  );
  const selectedBusinessWorkspace = useMemo(
    () =>
      snapshot.businessWorkspaces.find(
        (workspace) => workspace.projectId === selectedProjectId,
      ) ?? null,
    [selectedProjectId, snapshot.businessWorkspaces],
  );
  const businessWorkspaceVersionKey = useMemo(
    () =>
      snapshot.businessWorkspaces
        .map((workspace) => `${workspace.id}:${workspace.revision}`)
        .sort()
        .join("\u0000"),
    [snapshot.businessWorkspaces],
  );
  const quoteHistorySources = useMemo<readonly QuoteHistorySource[]>(
    () =>
      snapshot.businessWorkspaces
        .filter(
          (workspace) =>
            workspace.id !== selectedBusinessWorkspace?.id &&
            workspace.profile.lineItems.length > 0,
        )
        .map((workspace) => ({
          workspaceId: workspace.id,
          projectTitle:
            workspace.profile.projectTitle ||
            snapshot.projects.find(
              (project) => project.id === workspace.projectId,
            )?.name ||
            "历史项目",
          customerName:
            workspace.profile.customerName ||
            workspace.customer.displayName ||
            "",
          updatedAt: workspace.updatedAt,
          lineItems: workspace.profile.lineItems,
        }))
        .sort((left, right) => right.updatedAt - left.updatedAt)
        .slice(0, 12),
    [snapshot.businessWorkspaces, snapshot.projects, selectedBusinessWorkspace],
  );

  useEffect(() => {
    if (!desktopRuntime) {
      setBusinessCustomers([]);
      setBusinessCustomersLoading(false);
      setBusinessCustomersError(null);
      return;
    }

    let active = true;
    const delay = businessCustomerQuery.trim() ? 180 : 0;
    const timeoutId = window.setTimeout(() => {
      if (!active) return;
      setBusinessCustomersLoading(true);
      setBusinessCustomersError(null);
      void client
        .listBusinessCustomers({
          query: businessCustomerQuery.trim(),
          limit: 100,
        })
        .then((customers) => {
          if (active) setBusinessCustomers(customers);
        })
        .catch((error) => {
          if (active) setBusinessCustomersError(localizeHostError(error));
        })
        .finally(() => {
          if (active) setBusinessCustomersLoading(false);
        });
    }, delay);

    return () => {
      active = false;
      window.clearTimeout(timeoutId);
    };
  }, [
    businessCustomerQuery,
    businessCustomerReloadToken,
    businessWorkspaceVersionKey,
  ]);

  const actionableAssetIds = useMemo(() => {
    const assetIds = new Set<string>();
    for (const review of contractReviews) {
      for (const report of review.reports) assetIds.add(report.reportAssetId);
    }
    if (selectedContractReview) {
      for (const report of selectedContractReview.reports) {
        assetIds.add(report.reportAssetId);
      }
    }
    for (const assetId of businessWorkspaceAssetIds(selectedBusinessWorkspace)) {
      assetIds.add(assetId);
    }
    return [...assetIds].sort();
  }, [contractReviews, selectedBusinessWorkspace, selectedContractReview]);

  const actionableAssetKey = actionableAssetIds.join("\u0000");

  useEffect(() => {
    let active = true;
    if (actionableAssetIds.length === 0) {
      setAssetActionCapabilities({});
      return () => {
        active = false;
      };
    }

    void Promise.all(
      actionableAssetIds.map(async (assetId) => [
        assetId,
        await client.getAssetActionCapabilities(assetId),
      ] as const),
    )
      .then((entries) => {
        if (!active) return;
        setAssetActionCapabilities(Object.fromEntries(entries));
      })
      .catch(() => {
        if (!active) return;
        setAssetActionCapabilities({});
      });

    return () => {
      active = false;
    };
  }, [actionableAssetKey]);

  const selectedBrainThread = useMemo(
    () =>
      snapshot.brainThreads.find((thread) => thread.id === selectedBrainThreadId) ??
      null,
    [selectedBrainThreadId, snapshot.brainThreads],
  );
  const selectedExecutionBrief = useMemo(
    () =>
      snapshot.executionBriefs.find(
        (brief) => brief.projectId === selectedProjectId,
      ) ?? null,
    [selectedProjectId, snapshot.executionBriefs],
  );
  const selectedRequirementBrief = useMemo(
    () =>
      snapshot.requirementBriefs.find(
        (brief) => brief.projectId === selectedProjectId,
      ) ?? null,
    [selectedProjectId, snapshot.requirementBriefs],
  );
  const executionBriefDraft = selectedProjectId
    ? executionBriefDrafts[selectedProjectId]?.content ?? null
    : null;
  const requirementBriefDraft = selectedProjectId
    ? requirementBriefDrafts[selectedProjectId]?.draft ?? null
    : null;
  const requirementBriefConflict = selectedRequirementBrief
    ? hasRequirementBriefConflict(
        requirementBriefDrafts,
        selectedRequirementBrief,
      )
    : false;
  const runningBrainTurn = useMemo(
    () =>
      [...snapshot.brainTurns]
        .reverse()
        .find(
          (turn) =>
            turn.threadId === selectedBrainThreadId && turn.status === "running",
        ) ?? null,
    [selectedBrainThreadId, snapshot.brainTurns],
  );
  const brainStreamingDelta = runningBrainTurn
    ? snapshot.brainStreamingByTurn[runningBrainTurn.id] ?? ""
    : "";

  const refreshHostStatus = useCallback(async () => {
    if (!desktopRuntime) return;
    try {
      setHostStatus(await client.getHostStatus());
    } catch {
      // BsaigcClient publishes the normalized error to its snapshot.
    }
  }, []);

  const refreshContractReviewDetails = useCallback(
    async (reviewId: string, preferredFindingId: string | null = null) => {
      const [review, findings] = await Promise.all([
        client.getContractReview(reviewId),
        client.listReviewFindings({ reviewId }),
      ]);
      setSelectedContractReview(review);
      setContractFindings([...findings]);
      const findingId =
        preferredFindingId &&
        findings.some((finding) => finding.id === preferredFindingId)
          ? preferredFindingId
          : findings[0]?.id ?? null;
      setSelectedContractFindingId(findingId);
      const evidenceId = findings
        .find((finding) => finding.id === findingId)
        ?.evidenceIds[0];
      if (evidenceId) {
        setContractEvidence(await client.getEvidenceContext(evidenceId));
      } else {
        setContractEvidence(null);
      }
      return review;
    },
    [],
  );

  const refreshContractReviews = useCallback(async () => {
    if (!desktopRuntime || !selectedBusinessWorkspace) {
      setContractReviews([]);
      setSelectedContractReviewId(null);
      setSelectedContractReview(null);
      setContractFindings([]);
      setSelectedContractFindingId(null);
      setContractEvidence(null);
      return;
    }
    setIsLoadingContractReviews(true);
    try {
      const [reviews, backups] = await Promise.all([
        client.listContractReviews({
          workspaceId: selectedBusinessWorkspace.id,
          limit: 200,
        }),
        client.listAssetBackups(500),
      ]);
      const nextReviews = [...reviews];
      setContractReviews(nextReviews);
      setAssetBackups([...backups]);
      const nextReviewId = nextReviews.some(
        (review) => review.session.id === selectedContractReviewId,
      )
        ? selectedContractReviewId
        : nextReviews[0]?.session.id ?? null;
      setSelectedContractReviewId(nextReviewId);
      if (!nextReviewId) {
        setSelectedContractReview(null);
        setContractFindings([]);
        setSelectedContractFindingId(null);
        setContractEvidence(null);
      }
    } finally {
      setIsLoadingContractReviews(false);
    }
  }, [selectedBusinessWorkspace, selectedContractReviewId]);

  const probeCodex = useCallback(async () => {
    if (!desktopRuntime || isProbingCodex) return;
    setIsProbingCodex(true);
    try {
      setCodexStatus(await client.probeCodex());
    } catch {
      // BsaigcClient publishes the normalized error to its snapshot.
    } finally {
      setIsProbingCodex(false);
    }
  }, [isProbingCodex]);

  const refreshAiCredential = useCallback(async () => {
    if (!desktopRuntime) return;
    setAiCredentialBusy(true);
    try {
      setAiCredentialStatus(await client.getAiCredentialStatus());
      setAiCredentialError(null);
    } catch (error) {
      setAiCredentialError(localizeHostError(error));
      throw error;
    } finally {
      setAiCredentialBusy(false);
    }
  }, []);

  const brainModels = useMemo(
    () => buildBrainModelOptions(aiCredentialStatus),
    [aiCredentialStatus],
  );
  const effectiveBrainModel = normalizeBrainModelSelection(
    selectedBrainModel,
    brainModels,
  );

  useEffect(() => {
    if (selectedBrainModel !== effectiveBrainModel) {
      setSelectedBrainModel(effectiveBrainModel);
    }
  }, [effectiveBrainModel, selectedBrainModel]);

  const settingsProviders = useMemo<AiProviderSettings[]>(
    () =>
      (aiCredentialStatus?.providers ?? []).map((provider) => ({
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

  const settingsStorageLocations = useMemo<StorageLocation[]>(
    () =>
      (desktopSettings?.storage.locations ?? [])
        .map((location) => ({
          id: location.target,
          label: storageLocationLabel(location.target),
          path: location.path,
          sizeBytes: location.sizeBytes,
          kind: storageLocationKind(location.target),
          authoritative: location.authoritative,
          exists: location.exists,
          description: location.exists ? null : "首次使用时自动创建",
        })),
    [desktopSettings],
  );

  const settingsCacheTargets = useMemo<CacheCleanupTarget[]>(
    () =>
      (desktopSettings?.storage.locations ?? [])
        .filter(({ clearable }) => clearable)
        .map((location) => ({
          id: location.target,
          label: storageLocationLabel(location.target),
          path: location.path,
          sizeBytes: location.sizeBytes,
          enabled: location.target === "cache",
          selectedByDefault: location.target === "cache",
        })),
    [desktopSettings],
  );

  const settingsFeishuChannel = useMemo<FeishuChannelStatus>(
    () => mapFeishuChannel(desktopSettings, desktopSettingsError),
    [desktopSettings, desktopSettingsError],
  );
  const settingsR2Backup = useMemo<R2BackupStatus>(
    () => mapR2Backup(desktopSettings, desktopSettingsError),
    [desktopSettings, desktopSettingsError],
  );
  const settingsUpdate = useMemo<SettingsUpdateStatus>(
    () => mapDesktopUpdate(desktopSettings, desktopSettingsError),
    [desktopSettings, desktopSettingsError],
  );

  const persistAiProvider = useCallback(
    async (providerId: string | null, input: AiProviderInput) => {
      const current =
        aiCredentialStatus ?? (await client.getAiCredentialStatus());
      const existingProvider = providerId
        ? current.providers.find((provider) => provider.id === providerId) ?? null
        : null;
      const previousIds = new Set(current.providers.map((provider) => provider.id));
      let next = await client.upsertProvider(
        {
          providerId,
          name: input.name,
          kind: input.providerKind,
          baseUrl: input.baseUrl,
          apiKey: input.apiKey ?? null,
          models: input.models,
          defaultModel: input.defaultModel,
          setDefault: existingProvider?.isDefault ?? current.providers.length === 0,
          enabled: existingProvider?.enabled ?? true,
        },
        current.revision,
      );
      setAiCredentialStatus(next);

      const resolvedProviderId =
        providerId ??
        next.providers.find((provider) => !previousIds.has(provider.id))?.id ??
        null;
      if (!resolvedProviderId) {
        throw new Error("Host 未返回新建的 AI 服务标识");
      }

      if (input.clearApiKey) {
        next = await client.clearProviderApiKey(
          resolvedProviderId,
          next.revision,
        );
        setAiCredentialStatus(next);
      }

      return { providerId: resolvedProviderId, status: next };
    },
    [aiCredentialStatus],
  );

  const createAiProvider = useCallback(
    async (input: AiProviderInput): Promise<string> => {
      setAiCredentialBusy(true);
      try {
        const result = await persistAiProvider(null, input);
        setAiCredentialError(null);
        void client.getBrainHealth().then(setBrainHealth).catch(() => undefined);
        return result.providerId;
      } catch (error) {
        setAiCredentialError(localizeHostError(error));
        throw error;
      } finally {
        setAiCredentialBusy(false);
      }
    },
    [persistAiProvider],
  );

  const updateAiProvider = useCallback(
    async (providerId: string, input: AiProviderInput) => {
      setAiCredentialBusy(true);
      try {
        await persistAiProvider(providerId, input);
        setAiCredentialError(null);
        void client.getBrainHealth().then(setBrainHealth).catch(() => undefined);
      } catch (error) {
        setAiCredentialError(localizeHostError(error));
        throw error;
      } finally {
        setAiCredentialBusy(false);
      }
    },
    [persistAiProvider],
  );

  const deleteAiProvider = useCallback(
    async (providerId: string) => {
      setAiCredentialBusy(true);
      try {
        const current =
          aiCredentialStatus ?? (await client.getAiCredentialStatus());
        const next = await client.removeProvider(providerId, current.revision);
        setAiCredentialStatus(next);
        setAiCredentialError(null);
      } catch (error) {
        setAiCredentialError(localizeHostError(error));
        throw error;
      } finally {
        setAiCredentialBusy(false);
      }
    },
    [aiCredentialStatus],
  );

  const selectAiProvider = useCallback(
    async (providerId: string) => {
      setAiCredentialBusy(true);
      try {
        const current =
          aiCredentialStatus ?? (await client.getAiCredentialStatus());
        const provider = current.providers.find(
          (candidate) => candidate.id === providerId,
        );
        if (!provider) throw new Error("没有找到对应的 AI 服务");
        const next = await client.selectProvider(
          providerId,
          provider.defaultModel,
          current.revision,
        );
        setAiCredentialStatus(next);
        setAiCredentialError(null);
        void client.getBrainHealth().then(setBrainHealth).catch(() => undefined);
      } catch (error) {
        setAiCredentialError(localizeHostError(error));
        throw error;
      } finally {
        setAiCredentialBusy(false);
      }
    },
    [aiCredentialStatus],
  );

  const testAiProvider = useCallback(
    async (
      providerId: string | null,
      input: AiProviderInput,
    ): Promise<ProviderConnectionTestResult> => {
      setAiCredentialBusy(true);
      try {
        const current =
          aiCredentialStatus ?? (await client.getAiCredentialStatus());
        const response = await client.discoverProviderModels(
          {
            providerId,
            kind: input.providerKind,
            baseUrl: input.baseUrl,
            apiKey: input.apiKey ?? null,
          },
          current.revision,
        );
        setAiCredentialStatus(response.status);
        setAiCredentialError(null);
        const test = response.connectionTest;
        return {
          providerId: providerId ?? undefined,
          state:
            test && test.state !== "untested" ? test.state : "failed",
          message: test?.message ?? "Host 未返回连接测试结果",
          checkedAt: timestampToIso(test?.testedAt ?? null),
          models: test?.discoveredModels ?? [],
        };
      } catch (error) {
        setAiCredentialError(localizeHostError(error));
        throw error;
      } finally {
        setAiCredentialBusy(false);
      }
    },
    [aiCredentialStatus],
  );

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
  }, []);

  const openStorageLocation = useCallback(
    async (target: StorageLocationTarget) => {
      if (!desktopRuntime) return;
      setDesktopSettingsBusy(true);
      try {
        const current =
          desktopSettings ?? (await client.getDesktopSettingsStatus());
        const next = await client.openStorageLocation(target, current.revision);
        setDesktopSettings(next);
        setDesktopSettingsError(null);
      } catch (error) {
        setDesktopSettingsError(localizeHostError(error));
        throw error;
      } finally {
        setDesktopSettingsBusy(false);
      }
    },
    [desktopSettings],
  );

  const clearDesktopCache = useCallback(
    async (targets: StorageLocationTarget[]) => {
      if (!desktopRuntime) return;
      if (!targets.includes("cache")) {
        throw new Error("没有选中可清理的缓存");
      }
      setDesktopSettingsBusy(true);
      try {
        const current =
          desktopSettings ?? (await client.getDesktopSettingsStatus());
        const response = await client.clearCache(current.revision);
        setDesktopSettings(response.snapshot);
        setDesktopSettingsError(null);
        return response.cacheClear?.freedBytes;
      } catch (error) {
        setDesktopSettingsError(localizeHostError(error));
        throw error;
      } finally {
        setDesktopSettingsBusy(false);
      }
    },
    [desktopSettings],
  );

  const checkDesktopUpdates = useCallback(async () => {
    if (!desktopRuntime) return;
    setDesktopSettingsBusy(true);
    try {
      const current =
        desktopSettings ?? (await client.getDesktopSettingsStatus());
      const next = await client.checkForUpdates(current.revision);
      setDesktopSettings(next);
      setDesktopSettingsError(null);
    } catch (error) {
      setDesktopSettingsError(localizeHostError(error));
      throw error;
    } finally {
      setDesktopSettingsBusy(false);
    }
  }, [desktopSettings]);

  const openSettings = useCallback(() => {
    setSettingsOpen(true);
    void refreshAiCredential().catch(() => undefined);
    void refreshDesktopSettings().catch(() => undefined);
  }, [refreshAiCredential, refreshDesktopSettings]);

  useEffect(() => {
    if (!desktopRuntime) return;
    let active = true;

    void client.start().catch(() => undefined);
    void client
      .getHostStatus()
      .then((status) => {
        if (active) setHostStatus(status);
      })
      .catch(() => undefined);
    void client
      .probeCodex()
      .then((status) => {
        if (active) setCodexStatus(status);
      })
      .catch(() => undefined);
    void client
      .getAiCredentialStatus()
      .then((status) => {
        if (active) setAiCredentialStatus(status);
      })
      .catch((error) => {
        if (active) setAiCredentialError(localizeHostError(error));
      });
    void client
      .getDesktopSettingsStatus()
      .then((status) => {
        if (active) setDesktopSettings(status);
      })
      .catch((error) => {
        if (active) setDesktopSettingsError(localizeHostError(error));
      });
    void client
      .getBrainHealth()
      .then((health) => {
        if (active) setBrainHealth(health);
      })
      .catch(() => undefined);
    void client
      .getNativeMediaHealth()
      .then((health) => {
        if (active) setMediaHealth(health);
      })
      .catch(() => undefined);

    return () => {
      active = false;
      client.stop();
    };
  }, []);

  useEffect(() => {
    if (snapshot.projects.length === 0) {
      setSelectedProjectId(null);
      return;
    }
    if (!snapshot.projects.some((project) => project.id === selectedProjectId)) {
      setSelectedProjectId(snapshot.projects[0]?.id ?? null);
    }
  }, [selectedProjectId, snapshot.projects]);

  useEffect(() => {
    setBriefDraft(selectedProject ? cloneBrief(selectedProject.brief) : null);
  }, [selectedProject?.id, selectedProject?.revision]);

  useEffect(() => {
    if (!selectedProject) return;
    const sourceKey = executionBriefSourceKey(
      selectedProject.revision,
      selectedExecutionBrief?.revision,
      selectedRequirementBrief?.status === "confirmed"
        ? selectedRequirementBrief.revision
        : undefined,
    );
    const sourceContent = selectedExecutionBrief
      ? selectedExecutionBrief.content
      : prefillExecutionBrief(
          selectedProject.brief,
          selectedRequirementBrief?.status === "confirmed"
            ? selectedRequirementBrief.content
            : null,
        );
    setExecutionBriefDrafts((current) =>
      syncExecutionBriefDraft(
        current,
        selectedProject.id,
        sourceKey,
        sourceContent,
      ),
    );
  }, [
    selectedProject?.id,
    selectedProject?.revision,
    selectedExecutionBrief?.id,
    selectedExecutionBrief?.revision,
    selectedRequirementBrief?.id,
    selectedRequirementBrief?.revision,
    selectedRequirementBrief?.status,
  ]);

  useEffect(() => {
    if (!selectedRequirementBrief) return;
    setRequirementBriefDrafts((current) =>
      syncRequirementBriefDraft(current, selectedRequirementBrief),
    );
  }, [selectedRequirementBrief?.id, selectedRequirementBrief?.revision]);

  useEffect(() => {
    if (snapshot.brainThreads.length === 0) {
      setSelectedBrainThreadId(null);
      return;
    }
    if (!snapshot.brainThreads.some((thread) => thread.id === selectedBrainThreadId)) {
      setSelectedBrainThreadId(snapshot.brainThreads[0]?.id ?? null);
    }
  }, [selectedBrainThreadId, snapshot.brainThreads]);

  useEffect(() => {
    if (!desktopRuntime || !selectedBrainThreadId) return;
    let active = true;
    setIsLoadingBrainTurns(true);
    void client
      .refreshBrainTurns(selectedBrainThreadId)
      .catch(() => undefined)
      .finally(() => {
        if (active) setIsLoadingBrainTurns(false);
      });
    return () => {
      active = false;
    };
  }, [selectedBrainThreadId]);

  useEffect(() => {
    void refreshContractReviews().catch(() => undefined);
  }, [refreshContractReviews]);

  useEffect(() => {
    if (!desktopRuntime || !selectedContractReviewId) return;
    let active = true;
    void refreshContractReviewDetails(
      selectedContractReviewId,
      selectedContractFindingId,
    ).catch(() => {
      if (active) setContractEvidence(null);
    });
    return () => {
      active = false;
    };
  }, [
    refreshContractReviewDetails,
    selectedContractFindingId,
    selectedContractReviewId,
  ]);

  useEffect(() => {
    if (!desktopRuntime) return;
    let disposed = false;
    let unsubscribeContract: (() => void) | null = null;
    let unsubscribeBackup: (() => void) | null = null;

    void client
      .subscribeContractReviewEvents((event) => {
        const review = event.contractReview;
        if (
          selectedBusinessWorkspace &&
          review.session.workspaceId === selectedBusinessWorkspace.id
        ) {
          setContractReviews((current) => upsertContractReview(current, review));
        }
        if (review.session.id === selectedContractReviewId) {
          setSelectedContractReview(review);
          setContractFindings(review.findings);
        }
      })
      .then((unsubscribe) => {
        if (disposed) unsubscribe();
        else unsubscribeContract = unsubscribe;
      })
      .catch(() => undefined);

    void client
      .subscribeBackupEvents((event) => {
        setAssetBackups((current) => upsertAssetBackup(current, event.backup));
      })
      .then((unsubscribe) => {
        if (disposed) unsubscribe();
        else unsubscribeBackup = unsubscribe;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      unsubscribeContract?.();
      unsubscribeBackup?.();
    };
  }, [selectedBusinessWorkspace, selectedContractReviewId]);

  const handleCreateProject = async (draft: CreateProjectPayload) => {
    if (isCreatingProject) return;
    setIsCreatingProject(true);
    client.clearError();
    try {
      const response = await client.createProject({
        name: draft.name.trim(),
        clientName: draft.clientName.trim(),
      });
      setCreateProjectDraft(EMPTY_PROJECT_DRAFT);
      setSelectedProjectId(response.project.id);
      await refreshHostStatus();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsCreatingProject(false);
    }
  };

  const handleSaveBrief = async (
    projectId: string,
    nextBrief: BriefRecord,
  ) => {
    if (isSavingBrief || isSavingRequirementBrief) return;
    const project = snapshot.projects.find((item) => item.id === projectId);
    if (!project) return;

    setIsSavingBrief(true);
    client.clearError();
    try {
      await client.updateProjectBrief(
        projectId,
        cloneBrief(nextBrief),
        project.revision,
      );
      await refreshHostStatus();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsSavingBrief(false);
    }
  };

  const handleChangeStage = async (
    projectId: string,
    stage: ProjectStage,
  ) => {
    if (isChangingStage) return;
    const project = snapshot.projects.find((item) => item.id === projectId);
    if (!project || project.stage === stage) return;

    setIsChangingStage(true);
    client.clearError();
    try {
      await client.changeProjectStage(projectId, stage, project.revision);
      await refreshHostStatus();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsChangingStage(false);
    }
  };

    const runBusinessAction = async (
    action: string,
    operation: () => Promise<unknown>,
  ): Promise<boolean> => {
    if (!desktopRuntime || businessBusyAction !== null) return false;
    setBusinessBusyAction(action);
    client.clearError();
    try {
      await operation();
      return true;
    } catch (error) {
      const hostError = normalizeHostError(error);
      const approvalId =
        hostError.code === "APPROVAL_REQUIRED"
          ? hostError.message.match(/approvalId=([^\s]+)/)?.[1] ?? null
          : null;
      if (approvalId) {
        const approved = window.confirm(
          "此操作会写入不可逆的业务记录。确认审批并继续？",
        );
        try {
          await client.resolveApproval(approvalId, approved);
          if (approved) {
            await operation();
            return true;
          }
        } catch {
          // The client snapshot carries the approval or retry error.
        }
      }
      return false;
    } finally {
      setBusinessBusyAction(null);
    }
  };

  const currentBusinessWorkspace = (workspaceId: string) =>
    snapshot.businessWorkspaces.find((workspace) => workspace.id === workspaceId) ??
    null;

  const handleCreateBusinessWorkspace: BusinessDocumentsCenterActions["onCreateBusinessWorkspace"] =
    async (projectId, prefillSourceWorkspaceId) =>
      runBusinessAction("business:create-workspace", async () => {
        await client.createBusinessWorkspace({
          projectId,
          prefillSourceWorkspaceId: prefillSourceWorkspaceId ?? null,
        });
      });

  const handleListBusinessWorkspacePrefillCandidates: BusinessDocumentsCenterActions["onListBusinessWorkspacePrefillCandidates"] =
    async (projectId) => {
      if (!desktopRuntime) return [];
      return client.listBusinessWorkspacePrefillCandidates({
        targetProjectId: projectId,
        limit: 8,
      });
    };

  const handlePreviewBusinessWorkspacePrefill: BusinessDocumentsCenterActions["onPreviewBusinessWorkspacePrefill"] =
    async (projectId, sourceWorkspaceId) =>
      client.previewBusinessWorkspacePrefill({
        targetProjectId: projectId,
        sourceWorkspaceId,
      });

  const handleRefreshBusinessWorkspaces: BusinessDocumentsCenterActions["onRefreshBusinessWorkspaces"] =
    async () =>
      runBusinessAction("business:refresh", async () => {
        await client.refreshBusinessWorkspaces();
      });

  const handleSelectBusinessCustomer = async (
    customer: BusinessCustomerReceivableSummary,
  ) => {
    let workspace = latestCustomerWorkspace(
      customer,
      snapshot.businessWorkspaces,
    );

    if (!workspace && desktopRuntime) {
      try {
        const refreshed = await client.refreshBusinessWorkspaces();
        workspace = latestCustomerWorkspace(customer, refreshed);
      } catch (error) {
        setBusinessCustomersError(localizeHostError(error));
        return false;
      }
    }

    if (!workspace) {
      setBusinessCustomersError("没有找到该客户可打开的项目。");
      return false;
    }

    setBusinessCustomersError(null);
    setSelectedProjectId(workspace.projectId);
    setActiveSection("projects");
    return true;
  };

  const handleUpdateBusinessProfile: BusinessDocumentsCenterActions["onUpdateBusinessProfile"] =
    async (workspaceId, profile) => {
      const workspace = currentBusinessWorkspace(workspaceId);
      if (!workspace) return false;
      return runBusinessAction("business:update-profile", async () => {
        await client.updateBusinessProfile(
          { workspaceId, profile },
          workspace.revision,
        );
      });
    };

  const handleCreateBusinessDocument: BusinessDocumentsCenterActions["onCreateBusinessDocument"] =
    async (workspaceId, draft) => {
      const workspace = currentBusinessWorkspace(workspaceId);
      if (!workspace) return false;
      return runBusinessAction("business:create-document", async () => {
        await client.createBusinessDocument(
          { workspaceId, ...draft },
          workspace.revision,
        );
      });
    };

  const handleChangeBusinessDocumentStatus: BusinessDocumentsCenterActions["onChangeBusinessDocumentStatus"] =
    async (workspaceId, documentId, status, input) => {
      const workspace = currentBusinessWorkspace(workspaceId);
      if (!workspace) return false;
      const evidenceAssetId = input.attachEvidence
        ? await selectAndImportBusinessEvidence(workspace.projectId)
        : null;
      if (input.attachEvidence && !evidenceAssetId) return false;
      return runBusinessAction(
        `business:document:${documentId}:${status}`,
        async () => {
          await client.changeBusinessDocumentStatus(
            {
              workspaceId,
              documentId,
              status,
              evidence: evidenceAssetId
                ? {
                    assetId: evidenceAssetId,
                    occurredAt: input.evidenceOccurredAt,
                    note: input.evidenceNote,
                  }
                : null,
              manualWaiver: input.manualWaiverReason
                ? { reason: input.manualWaiverReason }
                : null,
              reason: input.reason,
            },
            workspace.revision,
          );
        },
      );
    };

  const handleGenerateBusinessDocument: BusinessDocumentsCenterActions["onGenerateBusinessDocument"] =
    async (workspaceId, documentId, format) => {
      const workspace = currentBusinessWorkspace(workspaceId);
      if (!workspace) return false;
      return runBusinessAction(
        `business:document:${documentId}:generate`,
        async () => {
          await client.generateBusinessDocument(
            { workspaceId, documentId, format },
            workspace.revision,
          );
        },
      );
    };

  const handleUpsertBusinessPayment: BusinessDocumentsCenterActions["onUpsertBusinessPayment"] =
    async (workspaceId, payment) => {
      const workspace = currentBusinessWorkspace(workspaceId);
      if (!workspace) return false;
      const paymentKey = payment.id ?? "new";
      return runBusinessAction(
        `business:payment:${paymentKey}:${payment.status}`,
        async () => {
          await client.upsertBusinessPayment(
            { workspaceId, payment },
            workspace.revision,
          );
        },
      );
    };
  const selectAndImportBusinessEvidence = async (
    projectId: string,
  ): Promise<string | null> => {
    if (!desktopRuntime || businessBusyAction !== null) return null;
    client.clearError();
    try {
      const source = await client.selectAssetSource();
      if (!source) return null;
      const imported = await client.importAsset(source.sourceToken, projectId);
      return imported.asset.id;
    } catch {
      return null;
    }
  };

  const handleConfirmBusinessQuote: BusinessDocumentsCenterActions["onConfirmBusinessQuote"] =
    async (workspaceId, confirmation) => {
      const workspace = currentBusinessWorkspace(workspaceId);
      if (!workspace) return false;
      const evidenceAssetId = await selectAndImportBusinessEvidence(workspace.projectId);
      if (!evidenceAssetId) return false;
      return runBusinessAction(
        `business:quote:${confirmation.quoteDocumentId}:confirm`,
        async () => {
          await client.confirmBusinessQuote(
            {
              workspaceId,
              quoteDocumentId: confirmation.quoteDocumentId,
              confirmationVersion: confirmation.confirmationVersion,
              customerRepresentative: confirmation.customerRepresentative,
              evidence: {
                assetId: evidenceAssetId,
                occurredAt: confirmation.occurredAt,
                note:
                  confirmation.notes.trim() ||
                  `客户 ${confirmation.customerRepresentative} 确认报价 ${confirmation.confirmationVersion}`,
              },
              notes: confirmation.notes,
            },
            workspace.revision,
          );
        },
      );
    };

  const handleRecordBusinessReceipt: BusinessDocumentsCenterActions["onRecordBusinessReceipt"] =
    async (workspaceId, receipt) => {
      const workspace = currentBusinessWorkspace(workspaceId);
      if (!workspace) return false;
      const evidenceAssetId = receipt.includeEvidence
        ? await selectAndImportBusinessEvidence(workspace.projectId)
        : null;
      if (receipt.includeEvidence && !evidenceAssetId) return false;
      return runBusinessAction(
        `business:payment:${receipt.paymentId}:receipt`,
        async () => {
          await client.recordBusinessReceipt(
            {
              workspaceId,
              paymentId: receipt.paymentId,
              amountCents: receipt.amountCents,
              occurredAt: receipt.occurredAt,
              reference: receipt.reference,
              notes: receipt.notes,
              evidence: evidenceAssetId
                ? {
                    assetId: evidenceAssetId,
                    occurredAt: receipt.occurredAt,
                    note: receipt.notes.trim() || `到账凭证 ${receipt.reference}`,
                  }
                : null,
            },
            workspace.revision,
          );
        },
      );
    };

  const handleReverseBusinessReceipt: BusinessDocumentsCenterActions["onReverseBusinessReceipt"] =
    async (workspaceId, reversal) => {
      const workspace = currentBusinessWorkspace(workspaceId);
      if (!workspace) return false;
      return runBusinessAction(
        `business:receipt:${reversal.receiptId}:reverse`,
        async () => {
          await client.reverseBusinessReceipt(
            { workspaceId, ...reversal },
            workspace.revision,
          );
        },
      );
    };

  const handleAdoptLatestConfirmedRequirement: BusinessDocumentsCenterActions["onAdoptLatestConfirmedRequirement"] =
    async (workspaceId) => {
      const workspace = currentBusinessWorkspace(workspaceId);
      if (!workspace) return false;
      return runBusinessAction(
        "business:requirement:adopt",
        async () => {
          await client.adoptLatestConfirmedRequirement(
            { workspaceId },
            workspace.revision,
          );
        },
      );
    };

  const handleChangeBusinessWorkspaceStatus: BusinessDocumentsCenterActions["onChangeBusinessWorkspaceStatus"] =
    async (workspaceId, status) => {
      const workspace = currentBusinessWorkspace(workspaceId);
      if (!workspace) return false;
      return runBusinessAction(
        `business:workspace:${status}`,
        async () => {
          await client.changeBusinessWorkspaceStatus(
            { workspaceId, status },
            workspace.revision,
          );
        },
      );
    };
  const handleUpsertBusinessCustomer: NonNullable<
    BusinessDocumentsCenterActions["onUpsertBusinessCustomer"]
  > = async (payload) => {
    const workspace = currentBusinessWorkspace(payload.workspaceId);
    if (!workspace) return false;
    return runBusinessAction("business:customer:upsert", async () => {
      await client.upsertBusinessCustomer(payload, workspace.revision);
      setBusinessCustomerReloadToken((current) => current + 1);
    });
  };

  const handleAssignBusinessCustomer: NonNullable<
    BusinessDocumentsCenterActions["onAssignBusinessCustomer"]
  > = async (payload) => {
    const workspace = currentBusinessWorkspace(payload.workspaceId);
    if (!workspace) return false;
    return runBusinessAction("business:customer:assign", async () => {
      await client.assignBusinessCustomer(payload, workspace.revision);
      setBusinessCustomerReloadToken((current) => current + 1);
    });
  };

  const handleUpsertBusinessMilestone: NonNullable<
    BusinessDocumentsCenterActions["onUpsertBusinessMilestone"]
  > = async (payload) => {
    const workspace = currentBusinessWorkspace(payload.workspaceId);
    if (!workspace) return false;
    return runBusinessAction("business:milestone:upsert", async () => {
      await client.upsertBusinessMilestone(payload, workspace.revision);
    });
  };

  const handleRegisterBusinessDeliverableVersion: NonNullable<
    BusinessDocumentsCenterActions["onRegisterBusinessDeliverableVersion"]
  > = async (payload) => {
    const workspace = currentBusinessWorkspace(payload.workspaceId);
    if (!workspace) return false;
    return runBusinessAction("business:deliverable:register", async () => {
      await client.registerBusinessDeliverableVersion(
        payload,
        workspace.revision,
      );
    });
  };

  const handleRecordBusinessDeliverySent: NonNullable<
    BusinessDocumentsCenterActions["onRecordBusinessDeliverySent"]
  > = async (payload) => {
    const workspace = currentBusinessWorkspace(payload.workspaceId);
    if (!workspace) return false;
    return runBusinessAction("business:delivery:sent", async () => {
      await client.recordBusinessDeliverySent(payload, workspace.revision);
    });
  };

  const handleRecordBusinessDeliverySignoff: NonNullable<
    BusinessDocumentsCenterActions["onRecordBusinessDeliverySignoff"]
  > = async (payload) => {
    const workspace = currentBusinessWorkspace(payload.workspaceId);
    if (!workspace) return false;
    return runBusinessAction("business:delivery:signoff", async () => {
      await client.recordBusinessDeliverySignoff(payload, workspace.revision);
    });
  };

  const handleRecordBusinessInvoiceIssued: NonNullable<
    BusinessDocumentsCenterActions["onRecordBusinessInvoiceIssued"]
  > = async (payload) => {
    const workspace = currentBusinessWorkspace(payload.workspaceId);
    if (!workspace) return false;
    return runBusinessAction("business:invoice:issued", async () => {
      await client.recordBusinessInvoiceIssued(payload, workspace.revision);
    });
  };

  const handleRecordBusinessInvoiceRedCorrection: NonNullable<
    BusinessDocumentsCenterActions["onRecordBusinessInvoiceRedCorrection"]
  > = async (payload) => {
    const workspace = currentBusinessWorkspace(payload.workspaceId);
    if (!workspace) return false;
    return runBusinessAction("business:invoice:red-correction", async () => {
      await client.recordBusinessInvoiceRedCorrection(
        payload,
        workspace.revision,
      );
    });
  };

  const handleAttachBusinessInvoiceAsset: NonNullable<
    BusinessDocumentsCenterActions["onAttachBusinessInvoiceAsset"]
  > = async (payload) => {
    const workspace = currentBusinessWorkspace(payload.workspaceId);
    if (!workspace) return false;
    return runBusinessAction("business:invoice:asset", async () => {
      await client.attachBusinessInvoiceAsset(payload, workspace.revision);
    });
  };

  const handleCreateBusinessArchiveSnapshot: NonNullable<
    BusinessDocumentsCenterActions["onCreateBusinessArchiveSnapshot"]
  > = async (workspaceId) => {
    const workspace = currentBusinessWorkspace(workspaceId);
    if (!workspace) return false;
    return runBusinessAction("business:archive:snapshot", async () => {
      await client.createBusinessArchiveSnapshot(
        { workspaceId },
        workspace.revision,
      );
    });
  };

  const handleImportBusinessAsset: NonNullable<
    BusinessDocumentsCenterActions["onImportBusinessAsset"]
  > = async (workspaceId) => {
    const workspace = currentBusinessWorkspace(workspaceId);
    if (!workspace || !desktopRuntime || businessBusyAction !== null) return null;
    setBusinessBusyAction("business:asset:import");
    client.clearError();
    try {
      const source = await client.selectAssetSource();
      if (!source) return null;
      const imported = await client.importAsset(
        source.sourceToken,
        workspace.projectId,
      );
      await client.refreshAssets();
      return imported.asset;
    } catch {
      return null;
    } finally {
      setBusinessBusyAction(null);
    }
  };

  const markTaskBusy = (taskId: string, busy: boolean) => {
    setBusyTaskIds((current) =>
      busy
        ? current.includes(taskId)
          ? current
          : [...current, taskId]
        : current.filter((id) => id !== taskId),
    );
  };

  const handleRefreshTasks = async () => {
    if (!desktopRuntime || isRefreshingTasks) return;
    setIsRefreshingTasks(true);
    client.clearError();
    try {
      await client.refreshTasks();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsRefreshingTasks(false);
    }
  };

  const handleCancelTask = async (task: TaskRecord) => {
    if (!desktopRuntime || busyTaskIds.includes(task.id)) return;
    markTaskBusy(task.id, true);
    client.clearError();
    try {
      await client.cancelTask(task.id, task.revision, "用户从任务中心取消");
      await refreshHostStatus();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      markTaskBusy(task.id, false);
    }
  };

  const handleRetryTask = async (task: TaskRecord, approved: boolean) => {
    if (!desktopRuntime || busyTaskIds.includes(task.id)) return;
    markTaskBusy(task.id, true);
    client.clearError();
    try {
      await client.retryTask(task.id, task.revision, approved);
      await refreshHostStatus();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      markTaskBusy(task.id, false);
    }
  };

  const handleRefreshAssets = async () => {
    if (!desktopRuntime || isRefreshingAssets) return;
    setIsRefreshingAssets(true);
    client.clearError();
    try {
      await client.refreshAssets();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsRefreshingAssets(false);
    }
  };

  const handleChooseAssetSource = async () => {
    if (!desktopRuntime || isSelectingAssetSource || isImportingAsset) return;
    setIsSelectingAssetSource(true);
    client.clearError();
    try {
      const source = await client.selectAssetSource();
      if (source) {
        setSelectedAssetSource(source);
        setImportProjectId((current) => current ?? selectedProjectId);
      }
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsSelectingAssetSource(false);
    }
  };

  const handleImportAsset = async (
    source: AssetSourceSelection,
    projectId: string | null,
  ) => {
    if (!desktopRuntime || isImportingAsset) return;
    setIsImportingAsset(true);
    client.clearError();
    try {
      await client.importAsset(source.sourceToken, projectId);
      await refreshHostStatus();
    } catch {
      // Source tokens are single-use; require a fresh native selection on retry.
    } finally {
      setSelectedAssetSource(null);
      setIsImportingAsset(false);
    }
  };

  const runContractAction = async (
    action: string,
    operation: () => Promise<void>,
  ) => {
    if (!desktopRuntime || contractBusyAction) return;
    setContractBusyAction(action);
    client.clearError();
    try {
      await operation();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setContractBusyAction((current) => (current === action ? null : current));
    }
  };

  const handleImportContract = async () => {
    if (!selectedAssetSource || !selectedProjectId) return;
    await runContractAction("import", async () => {
      try {
        const workspace =
          selectedBusinessWorkspace ??
          (
            await client.createBusinessWorkspace({
              projectId: selectedProjectId,
            })
          ).businessWorkspace;
        const imported = await client.importAsset(
          selectedAssetSource.sourceToken,
          selectedProjectId,
        );
        const created = await client.createContractReview(
          {
            workspaceId: workspace.id,
            sourceAssetId: imported.asset.id,
          },
          { projectId: selectedProjectId },
        );
        setContractReviews((current) =>
          upsertContractReview(current, created.contractReview),
        );
        setSelectedContractReviewId(created.contractReview.session.id);
        setSelectedContractReview(created.contractReview);
        setContractFindings(created.contractReview.findings);
        setSelectedContractFindingId(null);
        setContractEvidence(null);
        await Promise.allSettled([refreshHostStatus(), client.refreshAssets()]);
      } finally {
        // Source tokens are single-use; require a fresh native selection on retry.
        setSelectedAssetSource(null);
      }
    });
  };

  const handleSelectContractReview = (reviewId: string) => {
    setSelectedContractReviewId(reviewId);
    setSelectedContractFindingId(null);
    setContractEvidence(null);
  };

  const handleStartContractReview = async (review: ContractReviewRecord) => {
    await runContractAction(`start:${review.session.id}`, async () => {
      const response = await client.startContractReview(
        { reviewId: review.session.id },
        review.session.revision,
        { projectId: selectedProjectId },
      );
      setContractReviews((current) =>
        upsertContractReview(current, response.contractReview),
      );
      await refreshContractReviewDetails(response.contractReview.session.id);
    });
  };

  const handleCancelContractReview = async (review: ContractReviewRecord) => {
    if (!desktopRuntime || contractBusyAction?.startsWith("cancel:")) return;

    const action = `cancel:${review.session.id}`;
    setContractBusyAction(action);
    client.clearError();
    try {
      const latestReview = await client.getContractReview(review.session.id);
      const response = await client.cancelContractReview(
        {
          reviewId: review.session.id,
          reason: "用户取消合同审查",
        },
        latestReview.session.revision,
        { projectId: selectedProjectId },
      );
      setContractReviews((current) =>
        upsertContractReview(current, response.contractReview),
      );
      await refreshContractReviewDetails(response.contractReview.session.id);
    } finally {
      setContractBusyAction((current) => (current === action ? null : current));
    }
  };

  const handleRetryContractReview = async (review: ContractReviewRecord) => {
    if (!review.session.failure) return;
    await runContractAction(`retry:${review.session.id}`, async () => {
      const response = await client.retryContractReviewStage(
        {
          reviewId: review.session.id,
          stage: review.session.failure!.stage === "reviewingAgent" ||
            review.session.failure!.stage === "mergingFindings"
              ? "reviewingAgent"
              : review.session.failure!.stage,
        },
        review.session.revision,
        { projectId: selectedProjectId },
      );
      setContractReviews((current) =>
        upsertContractReview(current, response.contractReview),
      );
      await refreshContractReviewDetails(response.contractReview.session.id);
    });
  };

  const handleSelectContractFinding = async (finding: ReviewFindingRecord) => {
    setSelectedContractFindingId(finding.id);
    const evidenceId = finding.evidenceIds[0];
    if (!evidenceId) {
      setContractEvidence(null);
      return;
    }
    try {
      setContractEvidence(await client.getEvidenceContext(evidenceId));
    } catch {
      setContractEvidence(null);
    }
  };

  const handleSelectContractEvidence = async (evidenceId: string) => {
    try {
      setContractEvidence(await client.getEvidenceContext(evidenceId));
    } catch {
      setContractEvidence(null);
    }
  };

  const handleDecideContractFinding = async (
    finding: ReviewFindingRecord,
    decision: ReviewFindingDecision,
    comment: string,
  ) => {
    if (!selectedContractReview) return;
    await runContractAction(`decision:${finding.id}`, async () => {
      const response = await client.decideReviewFinding(
        {
          reviewId: selectedContractReview.session.id,
          findingId: finding.id,
          decision,
          comment,
        },
        selectedContractReview.session.revision,
        { projectId: selectedProjectId },
      );
      setContractReviews((current) =>
        upsertContractReview(current, response.contractReview),
      );
      await refreshContractReviewDetails(
        response.contractReview.session.id,
        finding.id,
      );
    });
  };

  const handleGenerateContractReport = async (
    review: ContractReviewRecord,
    format: ReviewReportFormat,
  ) => {
    await runContractAction(`report:${review.session.id}`, async () => {
      const response = await client.generateReviewReport(
        { reviewId: review.session.id, format },
        review.session.revision,
        { projectId: selectedProjectId },
      );
      setContractReviews((current) =>
        upsertContractReview(current, response.contractReview),
      );
      await refreshContractReviewDetails(response.contractReview.session.id);
      setAssetBackups([...(await client.listAssetBackups(500))]);
    });
  };

  const handlePromoteReviewedContract = async (review: ContractReviewRecord) => {
    const workspace = currentBusinessWorkspace(review.session.workspaceId);
    if (!workspace || review.session.status !== "completed") return;
    if (workspace.documents.some((document) => document.reviewId === review.session.id)) {
      return;
    }

    const newestReport = (format: ReviewReportFormat) =>
      [...review.reports]
        .filter((report) => report.format === format)
        .sort((left, right) => right.generatedAt - left.generatedAt)[0] ?? null;
    const report = newestReport("docx") ?? newestReport("html");
    if (!report) return;

    const projectCode =
      workspace.profile.projectCode
        .trim()
        .replace(/[^A-Za-z0-9_-]+/g, "-")
        .replace(/^-+|-+$/g, "") || "PROJECT";
    const nextSequence =
      Math.max(
        0,
        ...workspace.documents
          .filter((document) => document.kind === "contract")
          .map((document) => document.sequenceNumber),
      ) + 1;
    const title =
      review.session.sourceFileName.replace(/\.[^.]+$/, "").trim() || "服务合同";
    const action = `promote:${review.session.id}`;

    const signedContractAssetId = await selectAndImportBusinessEvidence(
      workspace.projectId,
    );
    if (!signedContractAssetId) return;

    await runContractAction(action, async () => {
      const currentWorkspace =
        currentBusinessWorkspace(review.session.workspaceId) ?? workspace;
      if (
        currentWorkspace.documents.some(
          (document) => document.reviewId === review.session.id,
        )
      ) {
        return;
      }
      await client.promoteReviewedContract(
        {
          workspaceId: currentWorkspace.id,
          reviewId: review.session.id,
          reportAssetId: report.reportAssetId,
          documentNumber: `C-${projectCode}-${String(nextSequence).padStart(2, "0")}`,
          title,
          evidence: {
            assetId: signedContractAssetId,
            occurredAt: Date.now(),
            note: "审查通过后的签署合同",
          },
          manualWaiver: null,
        },
        currentWorkspace.revision,
        {
          idempotencyKey: `businessWorkspace.promoteReviewedContract:${currentWorkspace.id}:${review.session.id}`,
        },
      );
    });
  };

  const handleOpenAsset = async (assetId: string) => {
    client.clearError();
    try {
      await client.openAsset(assetId);
    } catch {
      // The client snapshot carries the normalized capability error.
    }
  };

  const handleExportAsset = async (assetId: string) => {
    client.clearError();
    try {
      await client.exportAsset(assetId);
    } catch {
      // The client snapshot carries the normalized capability error.
    }
  };

  const handleRetryAssetBackup = async (backup: AssetBackupRecord) => {
    if (backup.state !== "failed") return;
    await runContractAction(`backup:${backup.assetId}`, async () => {
      const response = await client.retryAssetBackup(
        { assetId: backup.assetId },
        backup.revision,
        { projectId: selectedProjectId },
      );
      setAssetBackups((current) => upsertAssetBackup(current, response.backup));
    });
  };

  const handleRestoreAssetBackup = async (backup: AssetBackupRecord) => {
    if (backup.state !== "backedUp") return;
    await runContractAction(`restore:${backup.assetId}`, async () => {
      const response = await client.restoreAssetBackup(
        {
          assetId: backup.assetId,
          expectedSha256: backup.contentSha256,
        },
        backup.revision,
        { projectId: selectedProjectId },
      );
      setAssetBackups((current) => upsertAssetBackup(current, response.backup));
      await Promise.allSettled([client.refreshAssets(), refreshHostStatus()]);
    });
  };

  const createBrainThread = async () => {
    if (!desktopRuntime || isStartingBrainThread) return null;
    setIsStartingBrainThread(true);
    client.clearError();
    try {
      const thread = await client.startBrainThread({
        projectId: selectedProjectId,
        title: selectedProject ? `${selectedProject.name} Brain` : null,
        model: effectiveBrainModel === "default" ? null : effectiveBrainModel,
      });
      setSelectedBrainThreadId(thread.id);
      setBrainHealth(await client.getBrainHealth());
      return thread;
    } catch {
      return null;
    } finally {
      setIsStartingBrainThread(false);
    }
  };

  const handleSendBrainTurn = async () => {
    const inputText = brainDraft.trim();
    if (!desktopRuntime || isSendingBrainTurn || !inputText) return;
    setIsSendingBrainTurn(true);
    client.clearError();
    try {
      const thread = selectedBrainThread ?? (await createBrainThread());
      if (!thread) return;
      await client.startBrainTurn({
        threadId: thread.id,
        inputText,
        model: effectiveBrainModel === "default" ? null : effectiveBrainModel,
        effort: "medium",
      });
      setBrainDraft("");
      setBrainHealth(await client.getBrainHealth());
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsSendingBrainTurn(false);
    }
  };

  const handleInterruptBrainTurn = async () => {
    if (!desktopRuntime || !selectedBrainThread || !runningBrainTurn) return;
    client.clearError();
    try {
      await client.interruptBrainTurn(selectedBrainThread.id, runningBrainTurn.id);
      setBrainHealth(await client.getBrainHealth());
    } catch {
      // Error state is rendered from the client snapshot.
    }
  };

  const handleArchiveBrainThread = async (threadId: string, archived: boolean) => {
    if (!desktopRuntime) return;
    try {
      await client.brainThreadArchive(threadId, archived);
      await client.refreshBrainThreads();
    } catch {
      // Error state is rendered from the client snapshot.
    }
  };

  const handleDeleteBrainThread = async (threadId: string) => {
    if (!desktopRuntime) return;
    try {
      await client.brainThreadDelete(threadId);
      if (selectedBrainThreadId === threadId) {
        setSelectedBrainThreadId(null);
      }
      await client.refreshBrainThreads();
    } catch {
      // Error state is rendered from the client snapshot.
    }
  };

  const handleBrainAttach = async () => {
    if (!desktopRuntime || !selectedProjectId) return;
    try {
      const source = await client.selectAssetSource();
      if (!source) return;
      const imported = await client.importAsset(source.sourceToken, selectedProjectId);
      await client.refreshAssets();
      setBrainDraft((current) => {
        const separator = current && !current.endsWith("\n") ? "\n" : "";
        return `${current}${separator}【附件】${imported.asset.originalName}（已存入资产库）\n`;
      });
    } catch {
      // Error state is rendered from the client snapshot.
    }
  };

  const handleRefreshBrain = async () => {
    if (!desktopRuntime || isLoadingBrainThreads) return;
    setIsLoadingBrainThreads(true);
    client.clearError();
    try {
      const operations: Promise<unknown>[] = [
        client.refreshBrainThreads(),
        client.getBrainHealth().then(setBrainHealth),
      ];
      if (selectedBrainThreadId) {
        operations.push(client.refreshBrainTurns(selectedBrainThreadId));
      }
      await Promise.all(operations);
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsLoadingBrainThreads(false);
    }
  };

  const openCreateCase = () => {
    const preferredAsset =
      snapshot.assets.find(
        (asset) => asset.status === "ready" && asset.projectId === selectedProjectId,
      ) ?? snapshot.assets.find((asset) => asset.status === "ready") ?? null;
    setCaseEditor({
      mode: "create",
      caseId: null,
      draft: {
        assetId: preferredAsset?.id ?? "",
        title: "",
        clientName: selectedProject?.clientName ?? "",
        contentType: "brand",
        presentation: "liveAction",
        hasActors: false,
        isAigc: false,
        qualityTier: "reference",
        tags: "",
        notes: "",
      },
    });
  };

  const openEditCase = (caseRecord: CaseRecord) => {
    setCaseEditor({
      mode: "edit",
      caseId: caseRecord.id,
      draft: {
        assetId: caseRecord.assetId,
        title: caseRecord.title,
        clientName: caseRecord.clientName,
        contentType: caseRecord.contentType,
        presentation: caseRecord.presentation,
        hasActors: caseRecord.hasActors,
        isAigc: caseRecord.isAigc,
        qualityTier: caseRecord.qualityTier,
        tags: caseRecord.tags.join(", "),
        notes: caseRecord.notes,
      },
    });
  };

  const handleSaveCase = async (editor: CaseEditorState) => {
    if (!desktopRuntime || isSavingCase) return;
    setIsSavingCase(true);
    client.clearError();
    try {
      const tags = normalizeCaseTags(editor.draft.tags);
      if (editor.mode === "create") {
        const asset = snapshot.assets.find(
          (candidate) => candidate.id === editor.draft.assetId,
        );
        if (!asset) return;
        await client.createCase({
          assetId: asset.id,
          projectId: asset.projectId,
          title: editor.draft.title.trim(),
          clientName: editor.draft.clientName.trim(),
          contentType: editor.draft.contentType,
          presentation: editor.draft.presentation,
          hasActors: editor.draft.hasActors,
          isAigc: editor.draft.isAigc,
          qualityTier: editor.draft.qualityTier,
          tags,
          notes: editor.draft.notes.trim(),
        });
      } else {
        const current = snapshot.cases.find(
          (candidate) => candidate.id === editor.caseId,
        );
        if (!current) return;
        await client.updateCase(
          {
            caseId: current.id,
            title: editor.draft.title.trim(),
            clientName: editor.draft.clientName.trim(),
            contentType: editor.draft.contentType,
            presentation: editor.draft.presentation,
            hasActors: editor.draft.hasActors,
            isAigc: editor.draft.isAigc,
            qualityTier: editor.draft.qualityTier,
            tags,
            notes: editor.draft.notes.trim(),
          },
          current.revision,
        );
      }
      setCaseEditor(null);
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsSavingCase(false);
    }
  };

  const handleRefreshCases = async () => {
    if (!desktopRuntime || isRefreshingCases) return;
    setIsRefreshingCases(true);
    client.clearError();
    try {
      await client.refreshCases();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsRefreshingCases(false);
    }
  };

  const handleCreateExecutionBrief = async (
    projectId: string,
    content: ExecutionBriefContent,
  ) => {
    if (!desktopRuntime || isSavingExecutionBrief) return;
    setIsSavingExecutionBrief(true);
    client.clearError();
    const submittedContent = cloneExecutionBriefContent(content);
    try {
      const response = await client.createExecutionBrief({
        projectId,
        content: submittedContent,
      });
      setExecutionBriefDrafts((current) =>
        settleExecutionBriefDraft(
          current,
          projectId,
          executionBriefSourceKey(0, response.executionBrief.revision),
          submittedContent,
          response.executionBrief.content,
        ),
      );
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsSavingExecutionBrief(false);
    }
  };

  const handleSaveExecutionBrief = async (
    record: ExecutionBriefRecord,
    content: ExecutionBriefContent,
  ) => {
    if (!desktopRuntime || isSavingExecutionBrief) return;
    setIsSavingExecutionBrief(true);
    client.clearError();
    const submittedContent = cloneExecutionBriefContent(content);
    try {
      const response = await client.updateExecutionBrief(
        {
          briefId: record.id,
          content: submittedContent,
        },
        record.revision,
      );
      setExecutionBriefDrafts((current) =>
        settleExecutionBriefDraft(
          current,
          record.projectId,
          executionBriefSourceKey(0, response.executionBrief.revision),
          submittedContent,
          response.executionBrief.content,
        ),
      );
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsSavingExecutionBrief(false);
    }
  };

  const handleChangeExecutionBriefStatus = async (
    record: ExecutionBriefRecord,
    status: ExecutionBriefStatus,
  ) => {
    if (!desktopRuntime || isSavingExecutionBrief || !executionBriefDraft) return;
    setIsSavingExecutionBrief(true);
    client.clearError();
    const submittedContent = cloneExecutionBriefContent(executionBriefDraft);
    try {
      let revision = record.revision;
      if (!sameExecutionBriefContent(record.content, submittedContent)) {
        const saved = await client.updateExecutionBrief(
          {
            briefId: record.id,
            content: submittedContent,
          },
          revision,
        );
        revision = saved.executionBrief.revision;
      }
      const response = await client.changeExecutionBriefStatus(
        record.id,
        status,
        revision,
      );
      setExecutionBriefDrafts((current) =>
        settleExecutionBriefDraft(
          current,
          record.projectId,
          executionBriefSourceKey(0, response.executionBrief.revision),
          submittedContent,
          response.executionBrief.content,
        ),
      );
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsSavingExecutionBrief(false);
    }
  };

  const handleRefreshExecutionBriefs = async () => {
    if (!desktopRuntime || isRefreshingExecutionBriefs) return;
    setIsRefreshingExecutionBriefs(true);
    client.clearError();
    try {
      await client.refreshExecutionBriefs();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsRefreshingExecutionBriefs(false);
    }
  };

  const handleCreateRequirementBrief = async (projectId: string) => {
    if (!desktopRuntime || isSavingRequirementBrief || isSavingBrief) return;
    const project = snapshot.projects.find((item) => item.id === projectId);
    if (!project) return;
    setIsSavingRequirementBrief(true);
    client.clearError();
    try {
      if (
        selectedProjectId === projectId &&
        briefDraft &&
        !sameBrief(project.brief, briefDraft)
      ) {
        await client.updateProjectBrief(
          projectId,
          cloneBrief(briefDraft),
          project.revision,
        );
      }
      const response = await client.createRequirementBrief({ projectId });
      setRequirementBriefDrafts((current) =>
        settleRequirementBriefDraft(
          current,
          projectId,
          null,
          response.requirementBrief,
        ),
      );
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsSavingRequirementBrief(false);
    }
  };

  const handleSaveRequirementBrief = async (
    record: RequirementBriefRecord,
    draft: RequirementBriefDraft,
  ) => {
    if (!desktopRuntime || isSavingRequirementBrief || isSavingBrief) return;
    setIsSavingRequirementBrief(true);
    client.clearError();
    const submitted = cloneRequirementBriefDraft(draft);
    const expectedRevision = requirementExpectedRevision(
      requirementBriefDrafts,
      record.projectId,
      record.revision,
    );
    try {
      const response = await client.updateRequirementBrief(
        requirementUpdatePayload(record.id, submitted),
        expectedRevision,
      );
      setRequirementBriefDrafts((current) =>
        settleRequirementBriefDraft(
          current,
          record.projectId,
          submitted,
          response.requirementBrief,
        ),
      );
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsSavingRequirementBrief(false);
    }
  };

  const handleChangeRequirementBriefStatus = async (
    record: RequirementBriefRecord,
    status: RequirementBriefStatus,
    draft: RequirementBriefDraft,
  ) => {
    if (!desktopRuntime || isSavingRequirementBrief || isSavingBrief) return;
    setIsSavingRequirementBrief(true);
    client.clearError();
    const submitted = cloneRequirementBriefDraft(draft);
    const expectedRevision = requirementExpectedRevision(
      requirementBriefDrafts,
      record.projectId,
      record.revision,
    );
    try {
      const changed = await client.changeRequirementBriefStatus(
        record.id,
        status,
        expectedRevision,
      );

      setRequirementBriefDrafts((current) =>
        settleRequirementBriefDraft(
          current,
          record.projectId,
          submitted,
          changed.requirementBrief,
        ),
      );
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsSavingRequirementBrief(false);
    }
  };

  const handleRefreshRequirementBriefs = async () => {
    if (!desktopRuntime || isRefreshingRequirementBriefs) return;
    setIsRefreshingRequirementBriefs(true);
    client.clearError();
    try {
      await client.refreshRequirementBriefs();
    } catch {
      // Error state is rendered from the client snapshot.
    } finally {
      setIsRefreshingRequirementBriefs(false);
    }
  };

  const handleReloadRequirementBrief = (record: RequirementBriefRecord) => {
    setRequirementBriefDrafts((current) =>
      reloadRequirementBriefDraft(current, record),
    );
    client.clearError();
  };

  const handleRebaseRequirementBrief = (record: RequirementBriefRecord) => {
    setRequirementBriefDrafts((current) =>
      rebaseRequirementBriefDraft(current, record),
    );
    client.clearError();
  };

  const retryConnection = async () => {
    client.clearError();
    if (!desktopRuntime) return;
    await Promise.allSettled([
      client.start(),
      refreshHostStatus(),
      refreshAiCredential(),
      client.getBrainHealth().then(setBrainHealth),
      client.getNativeMediaHealth().then(setMediaHealth),
    ]);
  };

  if (desktopRuntime && !authChecked) {
    return <div className="app-auth-loading" aria-busy="true" />;
  }

  if (authStatus && !authStatus.currentUser) {
    return (
      <AuthGate
        status={authStatus}
        busy={authBusy}
        error={authError}
        onInitialize={handleAuthInitialize}
        onLogin={handleAuthLogin}
      />
    );
  }

  return (
    <>
      <BusinessWorkbench
      activeSection={activeSection}
      projects={snapshot.projects}
      selectedProjectId={selectedProjectId}
      projectQuery={projectQuery}
      createProjectDraft={createProjectDraft}
      briefDraft={briefDraft}
      hostStatus={hostStatus}
      codexStatus={codexStatus}
      recentEvents={snapshot.events}
      tasks={snapshot.tasks}
      taskStatusFilter={taskStatusFilter}
      busyTaskIds={busyTaskIds}
      assets={snapshot.assets}
      cases={snapshot.cases}
      executionBriefs={snapshot.executionBriefs}
      executionBriefDraft={executionBriefDraft}
      requirementBriefs={snapshot.requirementBriefs}
      requirementBriefDraft={requirementBriefDraft}
      requirementBriefConflict={requirementBriefConflict}
      businessWorkspace={selectedBusinessWorkspace}
      quoteHistorySources={quoteHistorySources}
      contractAgentFindings={contractFindings}
      businessWorkspaceEvents={snapshot.businessWorkspaceEvents}
      businessBusyAction={businessBusyAction}
      businessCustomers={businessCustomers}
      businessCustomersLoading={businessCustomersLoading}
      businessCustomersError={businessCustomersError}
      businessCustomerQuery={businessCustomerQuery}
      assetActionCapabilities={assetActionCapabilities}
      caseFilters={caseFilters}
      caseViewMode={caseViewMode}
      caseEditor={caseEditor}
      brainThreads={snapshot.brainThreads}
      brainTurns={snapshot.brainTurns}
      brainHealth={brainHealth}
      mediaHealth={mediaHealth}
      brainModels={brainModels}
      selectedBrainThreadId={selectedBrainThreadId}
      selectedBrainModel={effectiveBrainModel}
      brainDraft={brainDraft}
      brainStreamingDelta={brainStreamingDelta}
      assetProjectFilter={assetProjectFilter}
      assetViewMode={assetViewMode}
      selectedAssetSource={selectedAssetSource}
      importProjectId={importProjectId}
      reviews={contractReviews}
      selectedReviewId={selectedContractReviewId}
      selectedReview={selectedContractReview}
      findings={contractFindings}
      selectedFindingId={selectedContractFindingId}
      evidenceContext={contractEvidence}
      backups={assetBackups}
      selectedSource={selectedAssetSource}
      hasSelectedProject={Boolean(selectedProjectId)}
      busyAction={contractBusyAction}
      error={snapshot.error ? localizeHostError(snapshot.error) : null}
      isDesktopRuntime={desktopRuntime}
      isLoading={snapshot.synchronizing || isLoadingContractReviews}
      isCreatingProject={isCreatingProject}
      isSavingBrief={isSavingBrief || isSavingRequirementBrief}
      isChangingStage={isChangingStage}
      isProbingCodex={isProbingCodex}
      isRefreshingTasks={isRefreshingTasks}
      isRefreshingAssets={isRefreshingAssets}
      isSelectingAssetSource={isSelectingAssetSource}
      isImportingAsset={isImportingAsset}
      isLoadingBrainThreads={isLoadingBrainThreads}
      isLoadingBrainTurns={isLoadingBrainTurns}
      isStartingBrainThread={isStartingBrainThread}
      isSendingBrainTurn={isSendingBrainTurn}
      isRefreshingCases={isRefreshingCases}
      isSavingCase={isSavingCase}
      isRefreshingExecutionBriefs={isRefreshingExecutionBriefs}
      isSavingExecutionBrief={isSavingExecutionBrief}
      isRefreshingRequirementBriefs={isRefreshingRequirementBriefs}
      isSavingRequirementBrief={isSavingRequirementBrief || isSavingBrief}
      onNavigate={setActiveSection}
      onSelectProject={setSelectedProjectId}
      onBusinessCustomerQueryChange={setBusinessCustomerQuery}
      onRefreshBusinessCustomers={() =>
        setBusinessCustomerReloadToken((current) => current + 1)
      }
      onSelectBusinessCustomer={handleSelectBusinessCustomer}
      onCreateBusinessWorkspace={handleCreateBusinessWorkspace}
      onListBusinessWorkspacePrefillCandidates={handleListBusinessWorkspacePrefillCandidates}
      onPreviewBusinessWorkspacePrefill={handlePreviewBusinessWorkspacePrefill}
      onRefreshBusinessWorkspaces={handleRefreshBusinessWorkspaces}
      onUpdateBusinessProfile={handleUpdateBusinessProfile}
      onCreateBusinessDocument={handleCreateBusinessDocument}
      onChangeBusinessDocumentStatus={handleChangeBusinessDocumentStatus}
      onGenerateBusinessDocument={handleGenerateBusinessDocument}
      onUpsertBusinessPayment={handleUpsertBusinessPayment}
      onConfirmBusinessQuote={handleConfirmBusinessQuote}
      onRecordBusinessReceipt={handleRecordBusinessReceipt}
      onReverseBusinessReceipt={handleReverseBusinessReceipt}
      onAdoptLatestConfirmedRequirement={handleAdoptLatestConfirmedRequirement}
      onChangeBusinessWorkspaceStatus={handleChangeBusinessWorkspaceStatus}
      onUpsertBusinessCustomer={handleUpsertBusinessCustomer}
      onAssignBusinessCustomer={handleAssignBusinessCustomer}
      onUpsertBusinessMilestone={handleUpsertBusinessMilestone}
      onRegisterBusinessDeliverableVersion={handleRegisterBusinessDeliverableVersion}
      onRecordBusinessDeliverySent={handleRecordBusinessDeliverySent}
      onRecordBusinessDeliverySignoff={handleRecordBusinessDeliverySignoff}
      onRecordBusinessInvoiceIssued={handleRecordBusinessInvoiceIssued}
      onRecordBusinessInvoiceRedCorrection={handleRecordBusinessInvoiceRedCorrection}
      onAttachBusinessInvoiceAsset={handleAttachBusinessInvoiceAsset}
      onCreateBusinessArchiveSnapshot={handleCreateBusinessArchiveSnapshot}
      onImportBusinessAsset={handleImportBusinessAsset}
      onDismissBusinessError={() => client.clearError()}
      onOpenSettings={openSettings}
      onProjectQueryChange={setProjectQuery}
      onCreateProjectDraftChange={setCreateProjectDraft}
      onCreateProject={(draft) => void handleCreateProject(draft)}
      onBriefDraftChange={setBriefDraft}
      onSaveBrief={(projectId, brief) =>
        void handleSaveBrief(projectId, brief)
      }
      onChangeStage={(projectId, stage) =>
        void handleChangeStage(projectId, stage)
      }
      onProbeCodex={() => void probeCodex()}
      onTaskStatusFilterChange={setTaskStatusFilter}
      onCancelTask={(task) => void handleCancelTask(task)}
      onRetryTask={(task, approved) => void handleRetryTask(task, approved)}
      onRefreshTasks={() => void handleRefreshTasks()}
      onAssetProjectFilterChange={setAssetProjectFilter}
      onAssetViewModeChange={setAssetViewMode}
      onChooseAssetSource={() => void handleChooseAssetSource()}
      onClearAssetSource={() => setSelectedAssetSource(null)}
      onImportProjectChange={setImportProjectId}
      onImportAsset={(source, projectId) =>
        void handleImportAsset(source, projectId)
      }
      onRefreshAssets={() => void handleRefreshAssets()}
      onChooseSource={() => void handleChooseAssetSource()}
      onClearSource={() => setSelectedAssetSource(null)}
      onImportSource={() => void handleImportContract()}
      onRefresh={() => void refreshContractReviews()}
      onSelectReview={handleSelectContractReview}
      onStartReview={(review) => void handleStartContractReview(review)}
      onCancelReview={(review) => void handleCancelContractReview(review)}
      onRetryStage={(review) => void handleRetryContractReview(review)}
      onSelectFinding={(finding) => void handleSelectContractFinding(finding)}
      onSelectEvidence={(evidenceId) =>
        void handleSelectContractEvidence(evidenceId)
      }
      onDecideFinding={(finding, decision, comment) =>
        void handleDecideContractFinding(finding, decision, comment)
      }
      onGenerateReport={(review, format) =>
        void handleGenerateContractReport(review, format)
      }
      onPromoteReviewedContract={(review) =>
        void handlePromoteReviewedContract(review)
      }
      onOpenAsset={(assetId) => void handleOpenAsset(assetId)}
      onExportAsset={(assetId) => void handleExportAsset(assetId)}
      onRetryBackup={(backup) => void handleRetryAssetBackup(backup)}
      onRestoreBackup={(backup) => void handleRestoreAssetBackup(backup)}
      onSelectBrainThread={setSelectedBrainThreadId}
      onBrainModelChange={setSelectedBrainModel}
      onBrainDraftChange={setBrainDraft}
      onSendBrainTurn={() => void handleSendBrainTurn()}
      onInterruptBrainTurn={() => void handleInterruptBrainTurn()}
      onNewBrainThread={() => void createBrainThread()}
      onArchiveBrainThread={(threadId, archived) =>
        void handleArchiveBrainThread(threadId, archived)
      }
      onDeleteBrainThread={(threadId) => void handleDeleteBrainThread(threadId)}
      onBrainAttach={() => void handleBrainAttach()}
      onRefreshBrain={() => void handleRefreshBrain()}
      onCaseFiltersChange={setCaseFilters}
      onCaseViewModeChange={setCaseViewMode}
      onOpenCreateCase={openCreateCase}
      onOpenEditCase={openEditCase}
      onCaseEditorChange={setCaseEditor}
      onCloseCaseEditor={() => setCaseEditor(null)}
      onSaveCase={(editor) => void handleSaveCase(editor)}
      onRefreshCases={() => void handleRefreshCases()}
      onExecutionBriefDraftChange={(draft) => {
        if (!selectedProjectId) return;
        setExecutionBriefDrafts((current) =>
          editExecutionBriefDraft(current, selectedProjectId, draft),
        );
      }}
      onCreateExecutionBrief={(projectId, draft) =>
        void handleCreateExecutionBrief(projectId, draft)
      }
      onSaveExecutionBrief={(record, draft) =>
        void handleSaveExecutionBrief(record, draft)
      }
      onChangeExecutionBriefStatus={(record, status) =>
        void handleChangeExecutionBriefStatus(record, status)
      }
      onRefreshExecutionBriefs={() => void handleRefreshExecutionBriefs()}
      onRequirementBriefDraftChange={(draft) => {
        if (!selectedProjectId) return;
        setRequirementBriefDrafts((current) =>
          editRequirementBriefDraft(current, selectedProjectId, draft),
        );
      }}
      onCreateRequirementBrief={(projectId) =>
        void handleCreateRequirementBrief(projectId)
      }
      onSaveRequirementBrief={(record, draft) =>
        void handleSaveRequirementBrief(record, draft)
      }
      onChangeRequirementBriefStatus={(record, status, draft) =>
        void handleChangeRequirementBriefStatus(record, status, draft)
      }
      onRefreshRequirementBriefs={() => void handleRefreshRequirementBriefs()}
      onReloadRequirementBrief={handleReloadRequirementBrief}
      onRebaseRequirementBrief={handleRebaseRequirementBrief}
      onRetry={() => void retryConnection()}
      onDismissError={() => client.clearError()}
      />

      <SettingsCenter
        open={settingsOpen}
        authStatus={authStatus}
        onLogout={() => {
          setSettingsOpen(false);
          handleAuthLogout();
        }}
        onAuthChangePassword={(oldPassword, newPassword) =>
          client
            .authChangePassword({ oldPassword, newPassword })
            .then((status) => {
              setAuthStatus(status);
              return status;
            })
        }
        onAuthListUsers={() => client.authListUsers()}
        onAuthCreateUser={(username, password, role) =>
          client.authCreateUser({ username, password, role })
        }
        onAuthResetPassword={(username, newPassword) =>
          client.authResetPassword({ username, newPassword })
        }
        onAuthDeleteUser={(username) => client.authDeleteUser({ username })}
        onAuthRefreshRegistry={() =>
          client.authRefreshRegistry().then((status) => {
            setAuthStatus(status);
            return status;
          })
        }
        providers={settingsProviders}
        providerBusy={aiCredentialBusy}
        providerError={aiCredentialError}
        onClose={() => setSettingsOpen(false)}
        onCreateProvider={createAiProvider}
        onUpdateProvider={updateAiProvider}
        onDeleteProvider={deleteAiProvider}
        onSetDefaultProvider={selectAiProvider}
        onTestProviderConnection={testAiProvider}
        feishuChannel={settingsFeishuChannel}
        onRefreshFeishuChannel={() => refreshDesktopSettings().then(() => undefined)}
        storageLocations={settingsStorageLocations}
        cacheTargets={settingsCacheTargets}
        storageTotalBytes={desktopSettings?.storage.totalBytes}
        storageBusy={desktopSettingsBusy}
        onOpenStorageLocation={openStorageLocation}
        onClearCache={clearDesktopCache}
        r2Backup={settingsR2Backup}
        update={settingsUpdate}
        onCheckForUpdates={checkDesktopUpdates}
      />
    </>
  );
}

function storageLocationLabel(target: StorageLocationTarget): string {
  switch (target) {
    case "ledger":
      return "SQLite 账本";
    case "vault":
      return "本地资料库";
    case "cache":
      return "预览与缩略图缓存";
    case "staging":
      return "任务暂存区";
    case "credentials":
      return "受保护凭据区";
    default:
      return "应用数据";
  }
}

function storageLocationKind(target: StorageLocationTarget): StorageLocationKind {
  if (target === "ledger" || target === "vault" || target === "cache" || target === "staging" || target === "credentials") {
    return target;
  }
  return "other";
}

function mapFeishuChannel(
  snapshot: DesktopSettingsSnapshot | null,
  error: string | null,
): FeishuChannelStatus {
  const channel = snapshot?.channelAdapters.find(
    ({ id }) => id === "feishu-cli",
  ) ?? snapshot?.channelAdapters[0];
  if (!channel) {
    return {
      state: "planned",
      cliDetected: false,
      authorized: false,
      agentDiscoverable: false,
      detail: error ?? "正在读取本地渠道状态",
    };
  }
  const fallbackDetail =
    channel.state === "planned"
      ? "飞书 CLI 仅保留渠道接口，本版本不接入业务同步。"
      : channel.state === "degraded"
        ? "检测到飞书 CLI 配置，但渠道适配器尚未连接。"
        : channel.state === "configured"
          ? "飞书 CLI 渠道已配置。"
          : "飞书 CLI 渠道接口可用。";
  const authorized =
    channel.configured &&
    (channel.state === "configured" || channel.state === "available");
  return {
    state: channel.state,
    cliDetected: channel.state !== "planned",
    authorized,
    agentDiscoverable: authorized,
    detail: channel.message.trim() || fallbackDetail,
  };
}

function mapR2Backup(
  snapshot: DesktopSettingsSnapshot | null,
  error: string | null,
): R2BackupStatus {
  const backup = snapshot?.cloudBackup;
  if (!backup) {
    return {
      state: "not_configured",
      configured: false,
      pendingItems: 0,
      detail: error ?? "正在读取本地备份状态",
    };
  }
  const state: R2BackupStatus["state"] =
    backup.state === "degraded"
      ? "degraded"
      : backup.ready
        ? "idle"
        : backup.configured
          ? "adapter_pending"
          : "not_configured";
  const fallbackDetail = backup.configured
    ? "R2 仅作为异步备份，当前本地资料仍是唯一权威。"
    : "R2 备份尚未配置，本地资料不受影响。";
  return {
    state,
    configured: backup.configured,
    pendingItems: backup.pendingItems,
    detail: backup.message.trim() || fallbackDetail,
    destinationLabel: backup.configured ? "Cloudflare R2 · 异步备份" : null,
  };
}

function mapDesktopUpdate(
  snapshot: DesktopSettingsSnapshot | null,
  error: string | null,
): SettingsUpdateStatus {
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
  const checkState: SettingsUpdateStatus["checkState"] =
    update.state === "available"
      ? "available"
      : update.state === "failed" || update.state === "degraded"
        ? "failed"
        : update.state === "upToDate"
          ? "up_to_date"
          : "idle";
  const fallbackMessage = update.updateSourceConfigured
    ? "更新源已配置，但当前版本不会自动安装更新。"
    : "签名更新源尚未配置，当前不会执行联网检查。";
  const checkedAt = timestampToIso(update.lastCheckedAt);
  const hostMessage = update.message.trim();
  const message = hostMessage
    ? checkedAt
      ? `${hostMessage}（最近检查：${new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }).format(new Date(checkedAt))}）`
      : hostMessage
    : fallbackMessage;
  return {
    appVersion: update.currentVersion,
    buildVersion: update.buildVersion,
    buildChannel: update.buildChannel,
    codexVersion: update.codexRuntimeVersion,
    updateSource: null,
    updateSourceConfigured: update.updateSourceConfigured,
    automaticInstallAllowed: update.automaticInstallAllowed,
    checkState,
    latestVersion: update.latestVersion,
    downloadUrl: update.downloadUrl,
    checkedAt,
    message,
  };
}

function cloneBrief(brief: BriefRecord): BriefRecord {
  return {
    ...brief,
    deliverables: [...brief.deliverables],
    styleKeywords: [...brief.styleKeywords],
    mandatoryItems: [...brief.mandatoryItems],
    constraints: [...brief.constraints],
    risks: [...brief.risks],
  };
}

function sameBrief(left: BriefRecord, right: BriefRecord): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function upsertContractReview(
  current: ContractReviewRecord[],
  review: ContractReviewRecord,
): ContractReviewRecord[] {
  const next = current.filter(
    (candidate) => candidate.session.id !== review.session.id,
  );
  next.push(review);
  return next.sort(
    (left, right) => right.session.updatedAt - left.session.updatedAt,
  );
}

function upsertAssetBackup(
  current: AssetBackupRecord[],
  backup: AssetBackupRecord,
): AssetBackupRecord[] {
  const next = current.filter((candidate) => candidate.assetId !== backup.assetId);
  next.push(backup);
  return next.sort((left, right) => right.updatedAt - left.updatedAt);
}

function normalizeCaseTags(value: string): string[] {
  const seen = new Set<string>();
  const tags: string[] = [];
  for (const raw of value.split(/[,，\n]/)) {
    const tag = raw.trim();
    const key = tag.toLocaleLowerCase("zh-CN");
    if (!tag || seen.has(key)) continue;
    seen.add(key);
    tags.push(tag);
  }
  return tags;
}

function requirementUpdatePayload(
  briefId: string,
  draft: RequirementBriefDraft,
): {
  briefId: string;
  answers: RequirementAnswerInput[];
  content: RequirementBriefDraft["content"];
} {
  return {
    briefId,
    answers: draft.answers.map(({ questionId, answer, disposition }) => ({
      questionId,
      answer,
      disposition,
    })),
    content: draft.content,
  };
}

function timestampToIso(value: number | null): string | null {
  if (value === null) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date.toISOString();
}

function localizeHostError(error: unknown): string {
  const normalized: HostError = normalizeHostError(error);
  const messages: Record<string, string> = {
    REVISION_CONFLICT: "项目已在其他操作中更新，请同步后重试。",
    PROJECT_NOT_FOUND: "项目不存在或已被移除。",
    COMMAND_DEADLINE_EXCEEDED: "操作等待时间过长，请重试。",
    EXECUTION_BRIEF_INCOMPLETE: "拍前确认项未补齐，暂不能标记为可执行。",
    EXECUTION_BRIEF_EXISTS: "该项目已经有执行单。",
    EXECUTION_BRIEF_PROJECT_MISMATCH: "执行单与当前项目不匹配。",
    REQUIREMENT_BRIEF_INCOMPLETE: "需求信息未补齐，暂不能提交复核。",
    REQUIREMENT_BRIEF_FOLLOW_UP_PENDING: "仍有待追问问题，暂不能确认需求。",
    REQUIREMENT_BRIEF_CONFIRMED: "需求已确认，请先重新打开后再修改。",
    REQUIREMENT_BRIEF_EXISTS: "该项目已经建立需求访谈。",
    REQUIREMENT_BRIEF_PROJECT_MISMATCH: "需求 Brief 与当前项目不匹配。",
    REQUIREMENT_BRIEF_STATUS_TRANSITION_INVALID: "当前需求状态不允许执行该操作。",
    PROJECT_BRIEF_SUPERSEDED: "该项目已建立需求访谈，请在需求访谈中继续维护。",
    REFERENCE_CASE_NOT_FOUND: "引用的案例不存在或已被移除。",
    REFERENCE_CASE_PROJECT_MISMATCH: "引用案例属于其他项目，不能加入当前 Brief。",
    PROTOCOL_VERSION_MISMATCH: "客户端与本地 Host 协议版本不一致。",
    WEB_HOST_NOT_CONFIGURED: "当前版本必须在半山 AIGC 桌面容器中运行。",
    NOT_CONFIGURED: "当前版本必须在半山 AIGC 桌面容器中运行。",
    BACKUP_NOT_FOUND: "没有找到这份线上备份记录。",
    BACKUP_NOT_RESTORABLE: "这份文件还没有完成线上备份，暂时不能恢复。",
    BACKUP_REMOTE_IDENTITY_MISSING: "线上备份信息不完整，请重新备份后再恢复。",
    BACKUP_RESTORE_HASH_MISMATCH: "备份文件校验不一致，已停止恢复以保护本地资料。",
    BACKUP_RESTORE_INTEGRITY_FAILED: "备份完整性校验未通过，未写入本地。",
    BACKUP_RESTORE_REMOTE_METADATA_MISMATCH: "线上备份信息不一致，未写入本地。",
    BACKUP_RESTORE_DESTINATION_CONFLICT: "本地已有不同版本，为避免覆盖已停止恢复。",
    BACKUP_RESTORE_CANCELLED: "恢复已取消，本地资料没有变化。",
    BACKUP_RESTORE_DEGRADED: "线上备份暂不可用，本地业务不受影响。",
    BACKUP_RESTORE_FAILED: "恢复失败，请检查网络后重试。",
    BACKUP_REVISION_CONFLICT: "备份记录已更新，请刷新后重试。",
    AI_CREDENTIAL_STORAGE_UNAVAILABLE: "无法准备本机 AI 凭据存储。",
    AI_CREDENTIAL_PROTOCOL_UNSUPPORTED: "AI 配置协议版本不兼容，请更新应用。",
    AI_CREDENTIAL_INVALID: "API Key 格式无效，请检查后重试。",
    AI_CREDENTIAL_REVISION_CONFLICT: "AI 配置已变化，请刷新状态后重试。",
    AI_CREDENTIAL_IDEMPOTENCY_CONFLICT: "本次 AI 配置请求与已完成操作冲突。",
    AI_CREDENTIAL_COMMAND_CONFLICT: "AI 配置请求冲突，请刷新状态后重试。",
    AI_CREDENTIAL_BUSY: "AI 配置正在被其他操作占用，请稍后重试。",
    AI_CREDENTIAL_READ_FAILED: "无法读取本机 AI 配置。",
    AI_CREDENTIAL_WRITE_FAILED: "无法保存本机 AI 配置。",
    AI_CREDENTIAL_CORRUPT: "本机 AI 配置已损坏，请清除后重新配置。",
    AI_CREDENTIAL_PROTECTION_UNAVAILABLE: "当前系统不支持安全保存 AI 凭据。",
    AI_PROVIDER_LAST_REQUIRED: "至少需要保留一个 AI 服务。",
    BUSINESS_EVIDENCE_REQUIRED: "请上传签署或验收凭证，或填写人工豁免原因。",
    BUSINESS_EVIDENCE_AMBIGUOUS: "凭证与人工豁免不能同时提交。",
    BUSINESS_EVIDENCE_PROJECT_MISMATCH: "所选凭证不属于当前项目。",
    BUSINESS_VOID_REASON_REQUIRED: "作废单据必须填写原因。",
    BUSINESS_QUOTE_CONFIRMATION_REQUIRED: "请先登记客户对当前报价版本的确认凭证。",
    BUSINESS_QUOTE_CONFIRMATION_DUPLICATE: "该报价版本已经登记客户确认。",
    BUSINESS_QUOTE_NOT_CONFIRMABLE: "只有当前已生成的报价可以登记客户确认。",
    BUSINESS_PAYMENT_NOT_RECEIVABLE: "只有已请款或部分到账的付款节点可以登记到账。",
    BUSINESS_RECEIPT_EXCEEDS_PAYMENT: "本次到账超过该付款节点的待收金额。",
    BUSINESS_RECEIPT_REFERENCE_DUPLICATE: "该银行流水或凭证号已经登记。",
    BUSINESS_RECEIPT_REVERSAL_EXCEEDS_ORIGINAL: "冲销金额超过原流水剩余可冲销金额。",
    BUSINESS_REQUIREMENT_ADOPTION_BLOCKED: "已有正式单据，不能自动采用新的确认需求。",
    BUSINESS_REQUIREMENT_ALREADY_CURRENT: "当前商务资料已经采用最新确认需求。",
    CONFIRMED_REQUIREMENT_BRIEF_REQUIRED: "当前项目还没有已确认需求。",
    BUSINESS_WORKSPACE_ARCHIVE_BLOCKED: "当前项目仍有未完成单据、回款或验收，暂不能归档。",
    BUSINESS_CLOSURE_ARCHIVE_BLOCKED: "交付、发票或归档快照还未闭环，暂不能归档。",
    BUSINESS_DOCUMENT_INCOMPLETE: "单据快照缺少必填资料；快照在创建时冻结，请作废后补全资料重新创建。",
    BUSINESS_DOCUMENT_NOT_APPROVED: "单据需要先批准才能生成正式文件。",
    BUSINESS_DOCUMENT_AMOUNT_REQUIRED: "服务明细含税合计必须大于 0。",
    BUSINESS_DOCUMENT_STATUS_TRANSITION_INVALID: "当前单据状态不允许执行该操作。",
    BUSINESS_CONTRACT_IN_USE: "合同已有请款、发票或到账记录，请先处理下游记录再作废合同。",
    BUSINESS_DELIVERY_ALREADY_SIGNED: "该批次相关版本已有签收结论，请发送新批次继续交付。",
    BUSINESS_DELIVERY_SUBMISSION_NOT_FOUND: "发送批次不存在或已被移除。",
    BUSINESS_MILESTONE_STATUS_MANAGED: "已交付/已签收状态由系统维护，不能手工设置。",
    BUSINESS_PAYMENT_STATUS_MANAGED: "请款与到账状态由请款单和到账流水维护，不能手工修改。",
    BUSINESS_PAYMENT_STATUS_INVALID: "该付款节点状态不允许此操作。",
    BUSINESS_PAYMENT_STATUS_TRANSITION_INVALID: "该付款节点状态不允许此操作。",
    BUSINESS_PAYMENT_REQUEST_FIELDS_FROZEN: "该付款节点已被请款单捕获，请先作废对应请款单。",
    BUSINESS_PAYMENT_EXCEEDS_CONTRACT: "付款计划总额不能超过生效合同金额。",
    BUSINESS_EFFECTIVE_CONTRACT_REQUIRED: "请先将合同确认生效后再执行该操作。",
    BUSINESS_INVOICE_EXCEEDS_CONTRACT: "开票净额不能超过生效合同金额。",
    BUSINESS_INVOICE_REVERSAL_EXCEEDS_REMAINING: "红冲金额超过该发票剩余可红冲金额。",
    BUSINESS_INVOICE_NOT_FOUND: "原发票不存在或已被移除。",
    BUSINESS_CUSTOMER_IDENTITY_CONFLICT: "该税号已属于另一位客户，请核对客户主数据。",
    BUSINESS_CUSTOMER_BINDING_FROZEN: "业务台账已开始记录，客户绑定不能再变更。",
    BUSINESS_CUSTOMER_ARCHIVED: "该客户已归档，不能编辑或绑定。",
    BUSINESS_WORKSPACE_REVISION_CONFLICT: "资料已被其他操作更新，请刷新后重试。",
  };
  return messages[normalized.code] ?? normalized.message;
}

export default App;
