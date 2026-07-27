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
import type { BrainAttachmentPreview } from "../generated/bsaigc/BrainAttachmentPreview";
import type { BrainDroppedItems } from "../generated/bsaigc/BrainDroppedItems";
import type { BrainTurnContext } from "../generated/bsaigc/BrainTurnContext";
import type { BrainWorkspaceSelection } from "../generated/bsaigc/BrainWorkspaceSelection";
import type { StageClipboardImageRequest } from "../generated/bsaigc/StageClipboardImageRequest";
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
import type { CreateBusinessWorkspacePayload } from "../generated/bsaigc/CreateBusinessWorkspacePayload";
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

export const DOMAIN_EVENT_CHANNEL = "bsaigc://domain-event";
export const TASK_EVENT_CHANNEL = "bsaigc://task-event";
export const ASSET_EVENT_CHANNEL = "bsaigc://asset-event";
export const BRAIN_EVENT_CHANNEL = "bsaigc://brain-event";
export const CASE_EVENT_CHANNEL = "bsaigc://case-event";
export const EXECUTION_BRIEF_EVENT_CHANNEL =
  "bsaigc://execution-brief-event";
export const REQUIREMENT_BRIEF_EVENT_CHANNEL =
  "bsaigc://requirement-brief-event";
export const BUSINESS_WORKSPACE_EVENT_CHANNEL =
  "bsaigc://business-workspace-event";
export const CONTRACT_REVIEW_EVENT_CHANNEL =
  "bsaigc://contract-review-event";
export const BACKUP_EVENT_CHANNEL = "bsaigc://backup-event";
export const BSAIGC_PROTOCOL_VERSION = "1.5";
export const BUSINESS_WORKSPACE_PROTOCOL_VERSION = "1.6";

export type CreateBusinessWorkspaceInput = Omit<
  CreateBusinessWorkspacePayload,
  "customerId" | "prefillSourceWorkspaceId"
> & {
  readonly customerId?: string | null;
  readonly prefillSourceWorkspaceId?: string | null;
};

export type ListBusinessCustomersInput = Partial<ListBusinessCustomersRequest>;

export type ListBusinessWorkspacePrefillCandidatesInput = Omit<
  ListBusinessWorkspacePrefillCandidatesRequest,
  "limit"
> & {
  readonly limit?: number | null;
};

export type ListContractReviewsInput = {
  readonly workspaceId?: ListContractReviewsRequest["workspaceId"];
  readonly status?: ListContractReviewsRequest["status"];
  readonly limit?: ListContractReviewsRequest["limit"];
};

export type ListReviewFindingsInput = Omit<
  ListReviewFindingsRequest,
  "status"
> & {
  readonly status?: ListReviewFindingsRequest["status"];
};

export type Unsubscribe = () => void;
export type DomainEventListener = (event: DomainEvent) => void;
export type TaskEventListener = (event: TaskDomainEvent) => void;
export type AssetEventListener = (event: AssetDomainEvent) => void;
export type BrainEventListener = (event: BrainStreamEvent) => void;
export type CaseEventListener = (event: CaseDomainEvent) => void;
export type ExecutionBriefEventListener = (
  event: ExecutionBriefDomainEvent,
) => void;
export type RequirementBriefEventListener = (
  event: RequirementBriefDomainEvent,
) => void;
export type BusinessWorkspaceEventListener = (
  event: BusinessWorkspaceDomainEvent,
) => void;
export type ContractReviewEventListener = (
  event: ContractReviewDomainEvent,
) => void;
export type BackupEventListener = (event: BackupDomainEvent) => void;

export interface AssetActionCapabilities {
  readonly assetId: string;
  readonly canOpen: boolean;
  readonly canExport: boolean;
  readonly reason: string | null;
}

export interface HostAdapter {
  readonly kind: "desktop" | "web";
  executeCommand(command: CommandEnvelope): Promise<CommandResponse>;
  listProjects(): Promise<ProjectRecord[]>;
  replayEvents(afterSequence: number, limit: number): Promise<DomainEvent[]>;
  executeTaskCommand(command: TaskCommandEnvelope): Promise<TaskCommandResponse>;
  listTasks(): Promise<TaskRecord[]>;
  replayTaskEvents(afterSequence: number, limit: number): Promise<TaskDomainEvent[]>;
  selectAssetSource(): Promise<AssetSourceSelection | null>;
  selectAssetSources?(): Promise<AssetSourceSelection[]>;
  selectBrainWorkspace?(): Promise<BrainWorkspaceSelection | null>;
  registerBrainDroppedPaths?(paths: string[]): Promise<BrainDroppedItems>;
  stageClipboardImage?(
    request: StageClipboardImageRequest,
  ): Promise<AssetSourceSelection>;
  getBrainAttachmentPreview?(
    assetId: string,
  ): Promise<BrainAttachmentPreview | null>;
  executeAssetCommand(command: AssetCommandEnvelope): Promise<AssetCommandResponse>;
  listAssets(): Promise<AssetRecord[]>;
  replayAssetEvents(afterSequence: number, limit: number): Promise<AssetDomainEvent[]>;
  getAssetActionCapabilities?(assetId: string): Promise<AssetActionCapabilities>;
  openAsset?(assetId: string): Promise<void>;
  exportAsset?(assetId: string): Promise<boolean>;
  startBrainThread(request: StartBrainThreadRequest): Promise<BrainThreadRecord>;
  resumeBrainThread(request: ResumeBrainThreadRequest): Promise<BrainThreadRecord>;
  listRemoteBrainThreads(
    request: ListRemoteBrainThreadsRequest,
  ): Promise<RemoteBrainThreadPage>;
  startBrainTurn(
    request: StartBrainTurnRequest,
    context?: BrainTurnContext,
  ): Promise<BrainTurnStartResult>;
  interruptBrainTurn(request: InterruptBrainTurnRequest): Promise<BrainTurnRecord>;
  listLocalBrainThreads(projectId: string | null): Promise<BrainThreadRecord[]>;
  listLocalBrainTurns(threadId: string): Promise<BrainTurnRecord[]>;
  getBrainHealth(): Promise<BrainHostHealth>;
  getNativeMediaHealth(): Promise<NativeMediaHealth>;
  executeCaseCommand(command: CaseCommandEnvelope): Promise<CaseCommandResponse>;
  listCases(): Promise<CaseRecord[]>;
  replayCaseEvents(afterSequence: number, limit: number): Promise<CaseDomainEvent[]>;
  executeExecutionBriefCommand(
    command: ExecutionBriefCommandEnvelope,
  ): Promise<ExecutionBriefCommandResponse>;
  listExecutionBriefs(): Promise<ExecutionBriefRecord[]>;
  replayExecutionBriefEvents(
    afterSequence: number,
    limit: number,
  ): Promise<ExecutionBriefDomainEvent[]>;
  executeRequirementBriefCommand(
    command: RequirementBriefCommandEnvelope,
  ): Promise<RequirementBriefCommandResponse>;
  listRequirementBriefs(): Promise<RequirementBriefRecord[]>;
  replayRequirementBriefEvents(
    afterSequence: number,
    limit: number,
  ): Promise<RequirementBriefDomainEvent[]>;
  executeBusinessWorkspaceCommand(
    command: BusinessWorkspaceCommandEnvelope,
  ): Promise<BusinessWorkspaceCommandResponse>;
  listBusinessWorkspaces(): Promise<BusinessWorkspaceRecord[]>;
  listBusinessCustomers(
    request: ListBusinessCustomersRequest,
  ): Promise<BusinessCustomerReceivableSummary[]>;
  brainThreadArchive(threadId: string, archived: boolean): Promise<BrainThreadRecord>;
  brainThreadRename(threadId: string, title: string): Promise<BrainThreadRecord>;
  brainThreadDelete(threadId: string): Promise<void>;
  authStatus(): Promise<AuthStatus>;
  authInitializeAdmin(credentials: AuthCredentials): Promise<AuthStatus>;
  authLogin(credentials: AuthCredentials): Promise<AuthStatus>;
  authLogout(): Promise<AuthStatus>;
  authRememberedCredentials(): Promise<AuthCredentials | null>;
  authRememberCredentials(credentials: AuthCredentials): Promise<void>;
  authForgetCredentials(): Promise<void>;
  authChangePassword(payload: AuthChangePasswordPayload): Promise<AuthStatus>;
  authListUsers(): Promise<AuthUsersSnapshot>;
  authCreateUser(payload: AuthCreateUserPayload): Promise<AuthUsersSnapshot>;
  authResetPassword(payload: AuthResetPasswordPayload): Promise<AuthUsersSnapshot>;
  authDeleteUser(payload: AuthDeleteUserPayload): Promise<AuthUsersSnapshot>;
  authRefreshRegistry(): Promise<AuthStatus>;
  listBusinessWorkspacePrefillCandidates(
    request: ListBusinessWorkspacePrefillCandidatesRequest,
  ): Promise<BusinessWorkspacePrefillCandidate[]>;
  previewBusinessWorkspacePrefill(
    request: PreviewBusinessWorkspacePrefillRequest,
  ): Promise<BusinessWorkspacePrefillPreview>;
  replayBusinessWorkspaceEvents(
    afterSequence: number,
    limit: number,
  ): Promise<BusinessWorkspaceDomainEvent[]>;
  executeAiCredentialCommand(
    command: AiCredentialCommandEnvelope,
  ): Promise<AiCredentialCommandResponse>;
  executeDesktopSettingsCommand(
    command: DesktopSettingsCommandEnvelope,
  ): Promise<DesktopSettingsCommandResponse>;
  executeContractReviewCommand(
    command: ContractReviewCommandEnvelope,
  ): Promise<ContractReviewCommandResponse>;
  listContractReviews(
    request: ListContractReviewsRequest,
  ): Promise<ContractReviewRecord[]>;
  getContractReview(
    request: GetContractReviewRequest,
  ): Promise<ContractReviewRecord>;
  listReviewFindings(
    request: ListReviewFindingsRequest,
  ): Promise<ReviewFindingRecord[]>;
  getEvidenceContext(
    request: GetEvidenceContextRequest,
  ): Promise<EvidenceContext>;
  replayContractReviewEvents(
    afterSequence: number,
    limit: number,
  ): Promise<ContractReviewDomainEvent[]>;
  executeBackupCommand(
    command: BackupCommandEnvelope,
  ): Promise<BackupCommandResponse>;
  listAssetBackups(limit: number): Promise<AssetBackupRecord[]>;
  replayBackupEvents(
    afterSequence: number,
    limit: number,
  ): Promise<BackupDomainEvent[]>;
  getHostStatus(): Promise<HostStatus>;
  listPendingApprovals(): Promise<ApprovalRecord[]>;
  resolveApproval(payload: ResolveApprovalPayload): Promise<ApprovalRecord>;
  probeCodex(): Promise<CodexProbeStatus>;
  subscribeDomainEvents(listener: DomainEventListener): Promise<Unsubscribe>;
  subscribeTaskEvents(listener: TaskEventListener): Promise<Unsubscribe>;
  subscribeAssetEvents(listener: AssetEventListener): Promise<Unsubscribe>;
  subscribeBrainEvents(listener: BrainEventListener): Promise<Unsubscribe>;
  subscribeCaseEvents(listener: CaseEventListener): Promise<Unsubscribe>;
  subscribeExecutionBriefEvents(
    listener: ExecutionBriefEventListener,
  ): Promise<Unsubscribe>;
  subscribeRequirementBriefEvents(
    listener: RequirementBriefEventListener,
  ): Promise<Unsubscribe>;
  subscribeBusinessWorkspaceEvents(
    listener: BusinessWorkspaceEventListener,
  ): Promise<Unsubscribe>;
  subscribeContractReviewEvents(
    listener: ContractReviewEventListener,
  ): Promise<Unsubscribe>;
  subscribeBackupEvents(listener: BackupEventListener): Promise<Unsubscribe>;
}
