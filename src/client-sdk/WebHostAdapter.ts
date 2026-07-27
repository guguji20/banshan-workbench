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
import type { HostError } from "../generated/bsaigc/HostError";
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
import type {
  AssetActionCapabilities,
  AssetEventListener,
  BackupEventListener,
  BrainEventListener,
  BusinessWorkspaceEventListener,
  CaseEventListener,
  ContractReviewEventListener,
  DomainEventListener,
  ExecutionBriefEventListener,
  HostAdapter,
  RequirementBriefEventListener,
  TaskEventListener,
  Unsubscribe,
} from "./HostAdapter";

/**
 * Protocol reservation only. Desktop 1.0 intentionally ships no cloud host.
 * A future implementation maps the same JSON commands and events to HTTPS/WebSocket.
 */
export class WebHostAdapter implements HostAdapter {
  readonly kind = "web" as const;

  private unavailable(): HostError {
    return {
      code: "NOT_CONFIGURED",
      message:
        "WebHostAdapter only reserves the HTTPS/WebSocket protocol mapping and is not implemented. Use DesktopHostAdapter.",
      retryable: false,
    };
  }

  brainThreadArchive(_threadId: string, _archived: boolean): Promise<BrainThreadRecord> {
    return Promise.reject(this.unavailable());
  }

  brainThreadRename(_threadId: string, _title: string): Promise<BrainThreadRecord> {
    return Promise.reject(this.unavailable());
  }

  brainThreadDelete(_threadId: string): Promise<void> {
    return Promise.reject(this.unavailable());
  }

  authStatus(): Promise<AuthStatus> {
    return Promise.reject(this.unavailable());
  }

  authInitializeAdmin(_credentials: AuthCredentials): Promise<AuthStatus> {
    return Promise.reject(this.unavailable());
  }

  authLogin(_credentials: AuthCredentials): Promise<AuthStatus> {
    return Promise.reject(this.unavailable());
  }

  authLogout(): Promise<AuthStatus> {
    return Promise.reject(this.unavailable());
  }

  authRememberedCredentials(): Promise<AuthCredentials | null> {
    return Promise.resolve(null);
  }

  authRememberCredentials(_credentials: AuthCredentials): Promise<void> {
    return Promise.reject(this.unavailable());
  }

  authForgetCredentials(): Promise<void> {
    return Promise.resolve();
  }

  authChangePassword(_payload: AuthChangePasswordPayload): Promise<AuthStatus> {
    return Promise.reject(this.unavailable());
  }

  authListUsers(): Promise<AuthUsersSnapshot> {
    return Promise.reject(this.unavailable());
  }

  authCreateUser(_payload: AuthCreateUserPayload): Promise<AuthUsersSnapshot> {
    return Promise.reject(this.unavailable());
  }

  authResetPassword(_payload: AuthResetPasswordPayload): Promise<AuthUsersSnapshot> {
    return Promise.reject(this.unavailable());
  }

  authDeleteUser(_payload: AuthDeleteUserPayload): Promise<AuthUsersSnapshot> {
    return Promise.reject(this.unavailable());
  }

  authRefreshRegistry(): Promise<AuthStatus> {
    return Promise.reject(this.unavailable());
  }

  executeCommand(_command: CommandEnvelope): Promise<CommandResponse> {
    return Promise.reject(this.unavailable());
  }

  listProjects(): Promise<ProjectRecord[]> {
    return Promise.reject(this.unavailable());
  }

  replayEvents(_afterSequence: number, _limit: number): Promise<DomainEvent[]> {
    return Promise.reject(this.unavailable());
  }

  executeTaskCommand(_command: TaskCommandEnvelope): Promise<TaskCommandResponse> {
    return Promise.reject(this.unavailable());
  }

  listTasks(): Promise<TaskRecord[]> {
    return Promise.reject(this.unavailable());
  }

  replayTaskEvents(
    _afterSequence: number,
    _limit: number,
  ): Promise<TaskDomainEvent[]> {
    return Promise.reject(this.unavailable());
  }

  selectAssetSource(): Promise<AssetSourceSelection | null> {
    return Promise.reject(this.unavailable());
  }

  executeAssetCommand(
    _command: AssetCommandEnvelope,
  ): Promise<AssetCommandResponse> {
    return Promise.reject(this.unavailable());
  }

  listAssets(): Promise<AssetRecord[]> {
    return Promise.reject(this.unavailable());
  }

  replayAssetEvents(
    _afterSequence: number,
    _limit: number,
  ): Promise<AssetDomainEvent[]> {
    return Promise.reject(this.unavailable());
  }

  getAssetActionCapabilities(assetId: string): Promise<AssetActionCapabilities> {
    return Promise.resolve({
      assetId,
      canOpen: false,
      canExport: false,
      reason: "网页版暂未启用文件打开和导出能力。",
    });
  }

  openAsset(_assetId: string): Promise<void> {
    return Promise.reject(this.unavailable());
  }

  exportAsset(_assetId: string): Promise<boolean> {
    return Promise.reject(this.unavailable());
  }

  startBrainThread(
    _request: StartBrainThreadRequest,
  ): Promise<BrainThreadRecord> {
    return Promise.reject(this.unavailable());
  }

  resumeBrainThread(
    _request: ResumeBrainThreadRequest,
  ): Promise<BrainThreadRecord> {
    return Promise.reject(this.unavailable());
  }

  listRemoteBrainThreads(
    _request: ListRemoteBrainThreadsRequest,
  ): Promise<RemoteBrainThreadPage> {
    return Promise.reject(this.unavailable());
  }

  startBrainTurn(
    _request: StartBrainTurnRequest,
  ): Promise<BrainTurnStartResult> {
    return Promise.reject(this.unavailable());
  }

  interruptBrainTurn(
    _request: InterruptBrainTurnRequest,
  ): Promise<BrainTurnRecord> {
    return Promise.reject(this.unavailable());
  }

  listLocalBrainThreads(_projectId: string | null): Promise<BrainThreadRecord[]> {
    return Promise.reject(this.unavailable());
  }

  listLocalBrainTurns(_threadId: string): Promise<BrainTurnRecord[]> {
    return Promise.reject(this.unavailable());
  }

  getBrainHealth(): Promise<BrainHostHealth> {
    return Promise.reject(this.unavailable());
  }

  getNativeMediaHealth(): Promise<NativeMediaHealth> {
    return Promise.reject(this.unavailable());
  }

  executeCaseCommand(_command: CaseCommandEnvelope): Promise<CaseCommandResponse> {
    return Promise.reject(this.unavailable());
  }

  listCases(): Promise<CaseRecord[]> {
    return Promise.reject(this.unavailable());
  }

  replayCaseEvents(
    _afterSequence: number,
    _limit: number,
  ): Promise<CaseDomainEvent[]> {
    return Promise.reject(this.unavailable());
  }

  executeExecutionBriefCommand(
    _command: ExecutionBriefCommandEnvelope,
  ): Promise<ExecutionBriefCommandResponse> {
    return Promise.reject(this.unavailable());
  }

  listExecutionBriefs(): Promise<ExecutionBriefRecord[]> {
    return Promise.reject(this.unavailable());
  }

  replayExecutionBriefEvents(
    _afterSequence: number,
    _limit: number,
  ): Promise<ExecutionBriefDomainEvent[]> {
    return Promise.reject(this.unavailable());
  }

  executeRequirementBriefCommand(
    _command: RequirementBriefCommandEnvelope,
  ): Promise<RequirementBriefCommandResponse> {
    return Promise.reject(this.unavailable());
  }

  listRequirementBriefs(): Promise<RequirementBriefRecord[]> {
    return Promise.reject(this.unavailable());
  }

  replayRequirementBriefEvents(
    _afterSequence: number,
    _limit: number,
  ): Promise<RequirementBriefDomainEvent[]> {
    return Promise.reject(this.unavailable());
  }

  executeBusinessWorkspaceCommand(
    _command: BusinessWorkspaceCommandEnvelope,
  ): Promise<BusinessWorkspaceCommandResponse> {
    return Promise.reject(this.unavailable());
  }

  listBusinessWorkspaces(): Promise<BusinessWorkspaceRecord[]> {
    return Promise.reject(this.unavailable());
  }

  listBusinessCustomers(
    _request: ListBusinessCustomersRequest,
  ): Promise<BusinessCustomerReceivableSummary[]> {
    return Promise.reject(this.unavailable());
  }

  listBusinessWorkspacePrefillCandidates(
    _request: ListBusinessWorkspacePrefillCandidatesRequest,
  ): Promise<BusinessWorkspacePrefillCandidate[]> {
    return Promise.reject(this.unavailable());
  }

  previewBusinessWorkspacePrefill(
    _request: PreviewBusinessWorkspacePrefillRequest,
  ): Promise<BusinessWorkspacePrefillPreview> {
    return Promise.reject(this.unavailable());
  }

  replayBusinessWorkspaceEvents(
    _afterSequence: number,
    _limit: number,
  ): Promise<BusinessWorkspaceDomainEvent[]> {
    return Promise.reject(this.unavailable());
  }

  executeAiCredentialCommand(
    _command: AiCredentialCommandEnvelope,
  ): Promise<AiCredentialCommandResponse> {
    return Promise.reject(this.unavailable());
  }

  executeDesktopSettingsCommand(
    _command: DesktopSettingsCommandEnvelope,
  ): Promise<DesktopSettingsCommandResponse> {
    return Promise.reject(this.unavailable());
  }

  executeContractReviewCommand(
    _command: ContractReviewCommandEnvelope,
  ): Promise<ContractReviewCommandResponse> {
    return Promise.reject(this.unavailable());
  }

  listContractReviews(
    _request: ListContractReviewsRequest,
  ): Promise<ContractReviewRecord[]> {
    return Promise.reject(this.unavailable());
  }

  getContractReview(
    _request: GetContractReviewRequest,
  ): Promise<ContractReviewRecord> {
    return Promise.reject(this.unavailable());
  }

  listReviewFindings(
    _request: ListReviewFindingsRequest,
  ): Promise<ReviewFindingRecord[]> {
    return Promise.reject(this.unavailable());
  }

  getEvidenceContext(
    _request: GetEvidenceContextRequest,
  ): Promise<EvidenceContext> {
    return Promise.reject(this.unavailable());
  }

  replayContractReviewEvents(
    _afterSequence: number,
    _limit: number,
  ): Promise<ContractReviewDomainEvent[]> {
    return Promise.reject(this.unavailable());
  }

  executeBackupCommand(
    _command: BackupCommandEnvelope,
  ): Promise<BackupCommandResponse> {
    return Promise.reject(this.unavailable());
  }

  listAssetBackups(_limit: number): Promise<AssetBackupRecord[]> {
    return Promise.reject(this.unavailable());
  }

  replayBackupEvents(
    _afterSequence: number,
    _limit: number,
  ): Promise<BackupDomainEvent[]> {
    return Promise.reject(this.unavailable());
  }

  getHostStatus(): Promise<HostStatus> {
    return Promise.reject(this.unavailable());
  }

  listPendingApprovals(): Promise<ApprovalRecord[]> {
    return Promise.reject(this.unavailable());
  }

  resolveApproval(_payload: ResolveApprovalPayload): Promise<ApprovalRecord> {
    return Promise.reject(this.unavailable());
  }

  probeCodex(): Promise<CodexProbeStatus> {
    return Promise.reject(this.unavailable());
  }

  subscribeDomainEvents(_listener: DomainEventListener): Promise<Unsubscribe> {
    return Promise.reject(this.unavailable());
  }

  subscribeTaskEvents(_listener: TaskEventListener): Promise<Unsubscribe> {
    return Promise.reject(this.unavailable());
  }

  subscribeAssetEvents(_listener: AssetEventListener): Promise<Unsubscribe> {
    return Promise.reject(this.unavailable());
  }

  subscribeBrainEvents(_listener: BrainEventListener): Promise<Unsubscribe> {
    return Promise.reject(this.unavailable());
  }

  subscribeCaseEvents(_listener: CaseEventListener): Promise<Unsubscribe> {
    return Promise.reject(this.unavailable());
  }

  subscribeExecutionBriefEvents(
    _listener: ExecutionBriefEventListener,
  ): Promise<Unsubscribe> {
    return Promise.reject(this.unavailable());
  }

  subscribeRequirementBriefEvents(
    _listener: RequirementBriefEventListener,
  ): Promise<Unsubscribe> {
    return Promise.reject(this.unavailable());
  }

  subscribeBusinessWorkspaceEvents(
    _listener: BusinessWorkspaceEventListener,
  ): Promise<Unsubscribe> {
    return Promise.reject(this.unavailable());
  }

  subscribeContractReviewEvents(
    _listener: ContractReviewEventListener,
  ): Promise<Unsubscribe> {
    return Promise.reject(this.unavailable());
  }

  subscribeBackupEvents(
    _listener: BackupEventListener,
  ): Promise<Unsubscribe> {
    return Promise.reject(this.unavailable());
  }
}
