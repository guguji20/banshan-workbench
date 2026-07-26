import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AuthChangePasswordPayload } from "../generated/bsaigc/AuthChangePasswordPayload";
import type { AuthCreateUserPayload } from "../generated/bsaigc/AuthCreateUserPayload";
import type { AuthCredentials } from "../generated/bsaigc/AuthCredentials";
import type { AuthDeleteUserPayload } from "../generated/bsaigc/AuthDeleteUserPayload";
import type { AuthResetPasswordPayload } from "../generated/bsaigc/AuthResetPasswordPayload";
import type { AuthStatus } from "../generated/bsaigc/AuthStatus";
import type { AuthUsersSnapshot } from "../generated/bsaigc/AuthUsersSnapshot";
import type { CodexProbeStatus } from "../generated/bsaigc/CodexProbeStatus";
import type { CommandEnvelope } from "../generated/bsaigc/CommandEnvelope";
import type { CommandResponse } from "../generated/bsaigc/CommandResponse";
import type { DomainEvent } from "../generated/bsaigc/DomainEvent";
import type { HostStatus } from "../generated/bsaigc/HostStatus";
import type { ProjectRecord } from "../generated/bsaigc/ProjectRecord";
import type { ApprovalRecord } from "../generated/bsaigc/ApprovalRecord";
import type { ResolveApprovalPayload } from "../generated/bsaigc/ResolveApprovalPayload";
import type { AssetCommandEnvelope } from "../generated/bsaigc/AssetCommandEnvelope";
import type { AssetCommandResponse } from "../generated/bsaigc/AssetCommandResponse";
import type { AssetDomainEvent } from "../generated/bsaigc/AssetDomainEvent";
import type { AssetRecord } from "../generated/bsaigc/AssetRecord";
import type { AssetSourceSelection } from "../generated/bsaigc/AssetSourceSelection";
import type { TaskCommandEnvelope } from "../generated/bsaigc/TaskCommandEnvelope";
import type { TaskCommandResponse } from "../generated/bsaigc/TaskCommandResponse";
import type { TaskDomainEvent } from "../generated/bsaigc/TaskDomainEvent";
import type { TaskRecord } from "../generated/bsaigc/TaskRecord";
import type { BrainHostHealth } from "../generated/bsaigc/BrainHostHealth";
import type { BrainStreamEvent } from "../generated/bsaigc/BrainStreamEvent";
import type { BrainThreadRecord } from "../generated/bsaigc/BrainThreadRecord";
import type { BrainTurnRecord } from "../generated/bsaigc/BrainTurnRecord";
import type { BrainTurnStartResult } from "../generated/bsaigc/BrainTurnStartResult";
import type { InterruptBrainTurnRequest } from "../generated/bsaigc/InterruptBrainTurnRequest";
import type { ListRemoteBrainThreadsRequest } from "../generated/bsaigc/ListRemoteBrainThreadsRequest";
import type { RemoteBrainThreadPage } from "../generated/bsaigc/RemoteBrainThreadPage";
import type { ResumeBrainThreadRequest } from "../generated/bsaigc/ResumeBrainThreadRequest";
import type { StartBrainThreadRequest } from "../generated/bsaigc/StartBrainThreadRequest";
import type { StartBrainTurnRequest } from "../generated/bsaigc/StartBrainTurnRequest";
import type { NativeMediaHealth } from "../generated/bsaigc/NativeMediaHealth";
import type { CaseCommandEnvelope } from "../generated/bsaigc/CaseCommandEnvelope";
import type { CaseCommandResponse } from "../generated/bsaigc/CaseCommandResponse";
import type { CaseDomainEvent } from "../generated/bsaigc/CaseDomainEvent";
import type { CaseRecord } from "../generated/bsaigc/CaseRecord";
import type { ExecutionBriefCommandEnvelope } from "../generated/bsaigc/ExecutionBriefCommandEnvelope";
import type { ExecutionBriefCommandResponse } from "../generated/bsaigc/ExecutionBriefCommandResponse";
import type { ExecutionBriefDomainEvent } from "../generated/bsaigc/ExecutionBriefDomainEvent";
import type { ExecutionBriefRecord } from "../generated/bsaigc/ExecutionBriefRecord";
import type { RequirementBriefCommandEnvelope } from "../generated/bsaigc/RequirementBriefCommandEnvelope";
import type { RequirementBriefCommandResponse } from "../generated/bsaigc/RequirementBriefCommandResponse";
import type { RequirementBriefDomainEvent } from "../generated/bsaigc/RequirementBriefDomainEvent";
import type { RequirementBriefRecord } from "../generated/bsaigc/RequirementBriefRecord";
import type { BusinessCustomerReceivableSummary } from "../generated/bsaigc/BusinessCustomerReceivableSummary";
import type { BusinessWorkspaceCommandEnvelope } from "../generated/bsaigc/BusinessWorkspaceCommandEnvelope";
import type { BusinessWorkspaceCommandResponse } from "../generated/bsaigc/BusinessWorkspaceCommandResponse";
import type { BusinessWorkspaceDomainEvent } from "../generated/bsaigc/BusinessWorkspaceDomainEvent";
import type { BusinessWorkspacePrefillCandidate } from "../generated/bsaigc/BusinessWorkspacePrefillCandidate";
import type { BusinessWorkspacePrefillPreview } from "../generated/bsaigc/BusinessWorkspacePrefillPreview";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import type { ListBusinessCustomersRequest } from "../generated/bsaigc/ListBusinessCustomersRequest";
import type { ListBusinessWorkspacePrefillCandidatesRequest } from "../generated/bsaigc/ListBusinessWorkspacePrefillCandidatesRequest";
import type { PreviewBusinessWorkspacePrefillRequest } from "../generated/bsaigc/PreviewBusinessWorkspacePrefillRequest";
import type { AssetBackupRecord } from "../generated/bsaigc/AssetBackupRecord";
import type { BackupCommandEnvelope } from "../generated/bsaigc/BackupCommandEnvelope";
import type { BackupCommandResponse } from "../generated/bsaigc/BackupCommandResponse";
import type { BackupDomainEvent } from "../generated/bsaigc/BackupDomainEvent";
import type { ContractReviewCommandEnvelope } from "../generated/bsaigc/ContractReviewCommandEnvelope";
import type { ContractReviewCommandResponse } from "../generated/bsaigc/ContractReviewCommandResponse";
import type { ContractReviewDomainEvent } from "../generated/bsaigc/ContractReviewDomainEvent";
import type { ContractReviewRecord } from "../generated/bsaigc/ContractReviewRecord";
import type { EvidenceContext } from "../generated/bsaigc/EvidenceContext";
import type { GetContractReviewRequest } from "../generated/bsaigc/GetContractReviewRequest";
import type { GetEvidenceContextRequest } from "../generated/bsaigc/GetEvidenceContextRequest";
import type { ListContractReviewsRequest } from "../generated/bsaigc/ListContractReviewsRequest";
import type { ListReviewFindingsRequest } from "../generated/bsaigc/ListReviewFindingsRequest";
import type { ReviewFindingRecord } from "../generated/bsaigc/ReviewFindingRecord";
import type { AiCredentialCommandEnvelope } from "../generated/bsaigc/AiCredentialCommandEnvelope";
import type { AiCredentialCommandResponse } from "../generated/bsaigc/AiCredentialCommandResponse";
import type { DesktopSettingsCommandEnvelope } from "../generated/bsaigc/DesktopSettingsCommandEnvelope";
import type { DesktopSettingsCommandResponse } from "../generated/bsaigc/DesktopSettingsCommandResponse";
import {
  ASSET_EVENT_CHANNEL,
  BACKUP_EVENT_CHANNEL,
  BRAIN_EVENT_CHANNEL,
  BUSINESS_WORKSPACE_EVENT_CHANNEL,
  CASE_EVENT_CHANNEL,
  CONTRACT_REVIEW_EVENT_CHANNEL,
  DOMAIN_EVENT_CHANNEL,
  EXECUTION_BRIEF_EVENT_CHANNEL,
  REQUIREMENT_BRIEF_EVENT_CHANNEL,
  TASK_EVENT_CHANNEL,
  type AssetActionCapabilities,
  type AssetEventListener,
  type BackupEventListener,
  type BrainEventListener,
  type BusinessWorkspaceEventListener,
  type CaseEventListener,
  type ContractReviewEventListener,
  type DomainEventListener,
  type ExecutionBriefEventListener,
  type HostAdapter,
  type RequirementBriefEventListener,
  type TaskEventListener,
  type Unsubscribe,
} from "./HostAdapter";

export class DesktopHostAdapter implements HostAdapter {
  readonly kind = "desktop" as const;

  executeCommand(command: CommandEnvelope): Promise<CommandResponse> {
    return invoke<CommandResponse>("execute_command", { command });
  }

  listProjects(): Promise<ProjectRecord[]> {
    return invoke<ProjectRecord[]>("list_projects");
  }

  replayEvents(afterSequence: number, limit: number): Promise<DomainEvent[]> {
    return invoke<DomainEvent[]>("replay_events", {
      request: { afterSequence, limit },
    });
  }

  executeTaskCommand(command: TaskCommandEnvelope): Promise<TaskCommandResponse> {
    return invoke<TaskCommandResponse>("execute_task_command", { command });
  }

  listTasks(): Promise<TaskRecord[]> {
    return invoke<TaskRecord[]>("list_tasks");
  }

  replayTaskEvents(afterSequence: number, limit: number): Promise<TaskDomainEvent[]> {
    return invoke<TaskDomainEvent[]>("replay_task_events", {
      request: { afterSequence, limit },
    });
  }

  selectAssetSource(): Promise<AssetSourceSelection | null> {
    return invoke<AssetSourceSelection | null>("select_asset_source");
  }

  executeAssetCommand(command: AssetCommandEnvelope): Promise<AssetCommandResponse> {
    return invoke<AssetCommandResponse>("execute_asset_command", { command });
  }

  listAssets(): Promise<AssetRecord[]> {
    return invoke<AssetRecord[]>("list_assets");
  }

  replayAssetEvents(afterSequence: number, limit: number): Promise<AssetDomainEvent[]> {
    return invoke<AssetDomainEvent[]>("replay_asset_events", {
      request: { afterSequence, limit },
    });
  }

  getAssetActionCapabilities(assetId: string): Promise<AssetActionCapabilities> {
    return invoke<AssetActionCapabilities>("get_asset_action_capabilities", { assetId });
  }

  openAsset(assetId: string): Promise<void> {
    return invoke<void>("open_asset", { assetId });
  }

  exportAsset(assetId: string): Promise<boolean> {
    return invoke<boolean>("export_asset", { assetId });
  }

  startBrainThread(request: StartBrainThreadRequest): Promise<BrainThreadRecord> {
    return invoke<BrainThreadRecord>("brain_thread_start", { request });
  }

  resumeBrainThread(request: ResumeBrainThreadRequest): Promise<BrainThreadRecord> {
    return invoke<BrainThreadRecord>("brain_thread_resume", { request });
  }

  listRemoteBrainThreads(
    request: ListRemoteBrainThreadsRequest,
  ): Promise<RemoteBrainThreadPage> {
    return invoke<RemoteBrainThreadPage>("brain_thread_list_remote", { request });
  }

  startBrainTurn(request: StartBrainTurnRequest): Promise<BrainTurnStartResult> {
    return invoke<BrainTurnStartResult>("brain_turn_start", { request });
  }

  interruptBrainTurn(request: InterruptBrainTurnRequest): Promise<BrainTurnRecord> {
    return invoke<BrainTurnRecord>("brain_turn_interrupt", { request });
  }

  listLocalBrainThreads(projectId: string | null): Promise<BrainThreadRecord[]> {
    return invoke<BrainThreadRecord[]>("brain_list_local_threads", { projectId });
  }

  listLocalBrainTurns(threadId: string): Promise<BrainTurnRecord[]> {
    return invoke<BrainTurnRecord[]>("brain_list_local_turns", { threadId });
  }

  getBrainHealth(): Promise<BrainHostHealth> {
    return invoke<BrainHostHealth>("get_brain_health");
  }

  getNativeMediaHealth(): Promise<NativeMediaHealth> {
    return invoke<NativeMediaHealth>("get_native_media_health");
  }

  executeCaseCommand(command: CaseCommandEnvelope): Promise<CaseCommandResponse> {
    return invoke<CaseCommandResponse>("execute_case_command", { command });
  }

  listCases(): Promise<CaseRecord[]> {
    return invoke<CaseRecord[]>("list_cases");
  }

  replayCaseEvents(afterSequence: number, limit: number): Promise<CaseDomainEvent[]> {
    return invoke<CaseDomainEvent[]>("replay_case_events", {
      request: { afterSequence, limit },
    });
  }

  executeExecutionBriefCommand(
    command: ExecutionBriefCommandEnvelope,
  ): Promise<ExecutionBriefCommandResponse> {
    return invoke<ExecutionBriefCommandResponse>(
      "execute_execution_brief_command",
      { command },
    );
  }

  listExecutionBriefs(): Promise<ExecutionBriefRecord[]> {
    return invoke<ExecutionBriefRecord[]>("list_execution_briefs");
  }

  replayExecutionBriefEvents(
    afterSequence: number,
    limit: number,
  ): Promise<ExecutionBriefDomainEvent[]> {
    return invoke<ExecutionBriefDomainEvent[]>("replay_execution_brief_events", {
      request: { afterSequence, limit },
    });
  }

  executeRequirementBriefCommand(
    command: RequirementBriefCommandEnvelope,
  ): Promise<RequirementBriefCommandResponse> {
    return invoke<RequirementBriefCommandResponse>(
      "execute_requirement_brief_command",
      { command },
    );
  }

  listRequirementBriefs(): Promise<RequirementBriefRecord[]> {
    return invoke<RequirementBriefRecord[]>("list_requirement_briefs");
  }

  replayRequirementBriefEvents(
    afterSequence: number,
    limit: number,
  ): Promise<RequirementBriefDomainEvent[]> {
    return invoke<RequirementBriefDomainEvent[]>(
      "replay_requirement_brief_events",
      { request: { afterSequence, limit } },
    );
  }

  executeBusinessWorkspaceCommand(
    command: BusinessWorkspaceCommandEnvelope,
  ): Promise<BusinessWorkspaceCommandResponse> {
    return invoke<BusinessWorkspaceCommandResponse>(
      "execute_business_workspace_command",
      { command },
    );
  }

  listBusinessWorkspaces(): Promise<BusinessWorkspaceRecord[]> {
    return invoke<BusinessWorkspaceRecord[]>("list_business_workspaces");
  }

  listBusinessCustomers(
    request: ListBusinessCustomersRequest,
  ): Promise<BusinessCustomerReceivableSummary[]> {
    return invoke<BusinessCustomerReceivableSummary[]>("list_business_customers", {
      request,
    });
  }

  brainThreadArchive(threadId: string, archived: boolean): Promise<BrainThreadRecord> {
    return invoke<BrainThreadRecord>("brain_thread_archive", { threadId, archived });
  }

  brainThreadDelete(threadId: string): Promise<void> {
    return invoke<void>("brain_thread_delete", { threadId });
  }

  authStatus(): Promise<AuthStatus> {
    return invoke<AuthStatus>("auth_status");
  }

  authInitializeAdmin(credentials: AuthCredentials): Promise<AuthStatus> {
    return invoke<AuthStatus>("auth_initialize_admin", { credentials });
  }

  authLogin(credentials: AuthCredentials): Promise<AuthStatus> {
    return invoke<AuthStatus>("auth_login", { credentials });
  }

  authLogout(): Promise<AuthStatus> {
    return invoke<AuthStatus>("auth_logout");
  }

  authChangePassword(payload: AuthChangePasswordPayload): Promise<AuthStatus> {
    return invoke<AuthStatus>("auth_change_password", { payload });
  }

  authListUsers(): Promise<AuthUsersSnapshot> {
    return invoke<AuthUsersSnapshot>("auth_list_users");
  }

  authCreateUser(payload: AuthCreateUserPayload): Promise<AuthUsersSnapshot> {
    return invoke<AuthUsersSnapshot>("auth_create_user", { payload });
  }

  authResetPassword(payload: AuthResetPasswordPayload): Promise<AuthUsersSnapshot> {
    return invoke<AuthUsersSnapshot>("auth_reset_password", { payload });
  }

  authDeleteUser(payload: AuthDeleteUserPayload): Promise<AuthUsersSnapshot> {
    return invoke<AuthUsersSnapshot>("auth_delete_user", { payload });
  }

  authRefreshRegistry(): Promise<AuthStatus> {
    return invoke<AuthStatus>("auth_refresh_registry");
  }

  listBusinessWorkspacePrefillCandidates(
    request: ListBusinessWorkspacePrefillCandidatesRequest,
  ): Promise<BusinessWorkspacePrefillCandidate[]> {
    return invoke<BusinessWorkspacePrefillCandidate[]>(
      "list_business_workspace_prefill_candidates",
      { request },
    );
  }

  previewBusinessWorkspacePrefill(
    request: PreviewBusinessWorkspacePrefillRequest,
  ): Promise<BusinessWorkspacePrefillPreview> {
    return invoke<BusinessWorkspacePrefillPreview>(
      "preview_business_workspace_prefill",
      { request },
    );
  }

  replayBusinessWorkspaceEvents(
    afterSequence: number,
    limit: number,
  ): Promise<BusinessWorkspaceDomainEvent[]> {
    return invoke<BusinessWorkspaceDomainEvent[]>(
      "replay_business_workspace_events",
      { request: { afterSequence, limit } },
    );
  }

  executeAiCredentialCommand(
    command: AiCredentialCommandEnvelope,
  ): Promise<AiCredentialCommandResponse> {
    return invoke<AiCredentialCommandResponse>(
      "execute_ai_credential_command",
      { command },
    );
  }

  executeDesktopSettingsCommand(
    command: DesktopSettingsCommandEnvelope,
  ): Promise<DesktopSettingsCommandResponse> {
    return invoke<DesktopSettingsCommandResponse>(
      "execute_desktop_settings_command",
      { command },
    );
  }

  executeContractReviewCommand(
    command: ContractReviewCommandEnvelope,
  ): Promise<ContractReviewCommandResponse> {
    return invoke<ContractReviewCommandResponse>(
      "execute_contract_review_command",
      { command },
    );
  }

  listContractReviews(
    request: ListContractReviewsRequest,
  ): Promise<ContractReviewRecord[]> {
    return invoke<ContractReviewRecord[]>("list_contract_reviews", { request });
  }

  getContractReview(
    request: GetContractReviewRequest,
  ): Promise<ContractReviewRecord> {
    return invoke<ContractReviewRecord>("get_contract_review", { request });
  }

  listReviewFindings(
    request: ListReviewFindingsRequest,
  ): Promise<ReviewFindingRecord[]> {
    return invoke<ReviewFindingRecord[]>("list_review_findings", { request });
  }

  getEvidenceContext(
    request: GetEvidenceContextRequest,
  ): Promise<EvidenceContext> {
    return invoke<EvidenceContext>("get_evidence_context", { request });
  }

  replayContractReviewEvents(
    afterSequence: number,
    limit: number,
  ): Promise<ContractReviewDomainEvent[]> {
    return invoke<ContractReviewDomainEvent[]>(
      "replay_contract_review_events",
      { request: { afterSequence, limit } },
    );
  }

  executeBackupCommand(
    command: BackupCommandEnvelope,
  ): Promise<BackupCommandResponse> {
    return invoke<BackupCommandResponse>("execute_backup_command", { command });
  }

  listAssetBackups(limit: number): Promise<AssetBackupRecord[]> {
    return invoke<AssetBackupRecord[]>("list_asset_backups", { limit });
  }

  replayBackupEvents(
    afterSequence: number,
    limit: number,
  ): Promise<BackupDomainEvent[]> {
    return invoke<BackupDomainEvent[]>("replay_backup_events", {
      request: { afterSequence, limit },
    });
  }

  getHostStatus(): Promise<HostStatus> {
    return invoke<HostStatus>("get_host_status");
  }

  listPendingApprovals(): Promise<ApprovalRecord[]> {
    return invoke<ApprovalRecord[]>("list_pending_approvals");
  }

  resolveApproval(payload: ResolveApprovalPayload): Promise<ApprovalRecord> {
    return invoke<ApprovalRecord>("resolve_approval", { payload });
  }

  probeCodex(): Promise<CodexProbeStatus> {
    return invoke<CodexProbeStatus>("probe_codex");
  }

  async subscribeDomainEvents(
    listener: DomainEventListener,
  ): Promise<Unsubscribe> {
    return listen<DomainEvent>(DOMAIN_EVENT_CHANNEL, (event) => {
      listener(event.payload);
    });
  }

  async subscribeTaskEvents(listener: TaskEventListener): Promise<Unsubscribe> {
    return listen<TaskDomainEvent>(TASK_EVENT_CHANNEL, (event) => {
      listener(event.payload);
    });
  }

  async subscribeAssetEvents(listener: AssetEventListener): Promise<Unsubscribe> {
    return listen<AssetDomainEvent>(ASSET_EVENT_CHANNEL, (event) => {
      listener(event.payload);
    });
  }

  async subscribeBrainEvents(listener: BrainEventListener): Promise<Unsubscribe> {
    return listen<BrainStreamEvent>(BRAIN_EVENT_CHANNEL, (event) => {
      listener(event.payload);
    });
  }

  async subscribeCaseEvents(listener: CaseEventListener): Promise<Unsubscribe> {
    return listen<CaseDomainEvent>(CASE_EVENT_CHANNEL, (event) => {
      listener(event.payload);
    });
  }

  async subscribeExecutionBriefEvents(
    listener: ExecutionBriefEventListener,
  ): Promise<Unsubscribe> {
    return listen<ExecutionBriefDomainEvent>(
      EXECUTION_BRIEF_EVENT_CHANNEL,
      (event) => {
        listener(event.payload);
      },
    );
  }

  async subscribeRequirementBriefEvents(
    listener: RequirementBriefEventListener,
  ): Promise<Unsubscribe> {
    return listen<RequirementBriefDomainEvent>(
      REQUIREMENT_BRIEF_EVENT_CHANNEL,
      (event) => {
        listener(event.payload);
      },
    );
  }

  async subscribeBusinessWorkspaceEvents(
    listener: BusinessWorkspaceEventListener,
  ): Promise<Unsubscribe> {
    return listen<BusinessWorkspaceDomainEvent>(
      BUSINESS_WORKSPACE_EVENT_CHANNEL,
      (event) => {
        listener(event.payload);
      },
    );
  }

  async subscribeContractReviewEvents(
    listener: ContractReviewEventListener,
  ): Promise<Unsubscribe> {
    return listen<ContractReviewDomainEvent>(
      CONTRACT_REVIEW_EVENT_CHANNEL,
      (event) => {
        listener(event.payload);
      },
    );
  }

  async subscribeBackupEvents(
    listener: BackupEventListener,
  ): Promise<Unsubscribe> {
    return listen<BackupDomainEvent>(BACKUP_EVENT_CHANNEL, (event) => {
      listener(event.payload);
    });
  }
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
