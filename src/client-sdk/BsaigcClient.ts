import type { AuthChangePasswordPayload } from "../generated/bsaigc/AuthChangePasswordPayload";
import type { AuthCreateUserPayload } from "../generated/bsaigc/AuthCreateUserPayload";
import type { AuthCredentials } from "../generated/bsaigc/AuthCredentials";
import type { AuthDeleteUserPayload } from "../generated/bsaigc/AuthDeleteUserPayload";
import type { AuthResetPasswordPayload } from "../generated/bsaigc/AuthResetPasswordPayload";
import type { AuthStatus } from "../generated/bsaigc/AuthStatus";
import type { AuthUsersSnapshot } from "../generated/bsaigc/AuthUsersSnapshot";
import type { BriefRecord } from "../generated/bsaigc/BriefRecord";
import type { ChangeProjectStagePayload } from "../generated/bsaigc/ChangeProjectStagePayload";
import type { CodexProbeStatus } from "../generated/bsaigc/CodexProbeStatus";
import type { CommandEnvelope } from "../generated/bsaigc/CommandEnvelope";
import type { CommandResponse } from "../generated/bsaigc/CommandResponse";
import type { CreateProjectPayload } from "../generated/bsaigc/CreateProjectPayload";
import type { DomainEvent } from "../generated/bsaigc/DomainEvent";
import type { HostError } from "../generated/bsaigc/HostError";
import type { HostStatus } from "../generated/bsaigc/HostStatus";
import type { OperationContext } from "../generated/bsaigc/OperationContext";
import type { ProjectStage } from "../generated/bsaigc/ProjectStage";
import type { UpdateProjectBriefPayload } from "../generated/bsaigc/UpdateProjectBriefPayload";
import type { ApprovalRecord } from "../generated/bsaigc/ApprovalRecord";
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
import type { CreateTaskPayload } from "../generated/bsaigc/CreateTaskPayload";
import type { TaskCommandEnvelope } from "../generated/bsaigc/TaskCommandEnvelope";
import type { TaskCommandResponse } from "../generated/bsaigc/TaskCommandResponse";
import type { TaskDomainEvent } from "../generated/bsaigc/TaskDomainEvent";
import type { TaskRecord } from "../generated/bsaigc/TaskRecord";
import type { BrainHostHealth } from "../generated/bsaigc/BrainHostHealth";
import type { BrainStreamEvent } from "../generated/bsaigc/BrainStreamEvent";
import type { BrainThreadRecord } from "../generated/bsaigc/BrainThreadRecord";
import type { BrainTurnRecord } from "../generated/bsaigc/BrainTurnRecord";
import type { BrainTurnStartResult } from "../generated/bsaigc/BrainTurnStartResult";
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
import type { CreateCasePayload } from "../generated/bsaigc/CreateCasePayload";
import type { UpdateCasePayload } from "../generated/bsaigc/UpdateCasePayload";
import type { CreateExecutionBriefPayload } from "../generated/bsaigc/CreateExecutionBriefPayload";
import type { ExecutionBriefCommandEnvelope } from "../generated/bsaigc/ExecutionBriefCommandEnvelope";
import type { ExecutionBriefCommandResponse } from "../generated/bsaigc/ExecutionBriefCommandResponse";
import type { ExecutionBriefDomainEvent } from "../generated/bsaigc/ExecutionBriefDomainEvent";
import type { ExecutionBriefRecord } from "../generated/bsaigc/ExecutionBriefRecord";
import type { ExecutionBriefStatus } from "../generated/bsaigc/ExecutionBriefStatus";
import type { UpdateExecutionBriefPayload } from "../generated/bsaigc/UpdateExecutionBriefPayload";
import type { CreateRequirementBriefPayload } from "../generated/bsaigc/CreateRequirementBriefPayload";
import type { RequirementBriefCommandEnvelope } from "../generated/bsaigc/RequirementBriefCommandEnvelope";
import type { RequirementBriefCommandResponse } from "../generated/bsaigc/RequirementBriefCommandResponse";
import type { RequirementBriefDomainEvent } from "../generated/bsaigc/RequirementBriefDomainEvent";
import type { RequirementBriefRecord } from "../generated/bsaigc/RequirementBriefRecord";
import type { RequirementBriefStatus } from "../generated/bsaigc/RequirementBriefStatus";
import type { UpdateRequirementBriefPayload } from "../generated/bsaigc/UpdateRequirementBriefPayload";
import type { BusinessCustomerReceivableSummary } from "../generated/bsaigc/BusinessCustomerReceivableSummary";
import type { BusinessDocumentStatus } from "../generated/bsaigc/BusinessDocumentStatus";
import type { BusinessWorkspaceCommandEnvelope } from "../generated/bsaigc/BusinessWorkspaceCommandEnvelope";
import type { BusinessWorkspaceCommandResponse } from "../generated/bsaigc/BusinessWorkspaceCommandResponse";
import type { BusinessWorkspaceDomainEvent } from "../generated/bsaigc/BusinessWorkspaceDomainEvent";
import type { BusinessWorkspacePrefillCandidate } from "../generated/bsaigc/BusinessWorkspacePrefillCandidate";
import type { BusinessWorkspacePrefillPreview } from "../generated/bsaigc/BusinessWorkspacePrefillPreview";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import type { BusinessWorkspaceStatus } from "../generated/bsaigc/BusinessWorkspaceStatus";
import type { AttachBusinessInvoiceAssetPayload } from "../generated/bsaigc/AttachBusinessInvoiceAssetPayload";
import type { AssignBusinessCustomerPayload } from "../generated/bsaigc/AssignBusinessCustomerPayload";
import type { CreateBusinessArchiveSnapshotPayload } from "../generated/bsaigc/CreateBusinessArchiveSnapshotPayload";
import type { RecordBusinessDeliverySentPayload } from "../generated/bsaigc/RecordBusinessDeliverySentPayload";
import type { RecordBusinessDeliverySignoffPayload } from "../generated/bsaigc/RecordBusinessDeliverySignoffPayload";
import type { RecordBusinessInvoiceIssuedPayload } from "../generated/bsaigc/RecordBusinessInvoiceIssuedPayload";
import type { RecordBusinessInvoiceRedCorrectionPayload } from "../generated/bsaigc/RecordBusinessInvoiceRedCorrectionPayload";
import type { RegisterBusinessDeliverableVersionPayload } from "../generated/bsaigc/RegisterBusinessDeliverableVersionPayload";
import type { UpsertBusinessCustomerPayload } from "../generated/bsaigc/UpsertBusinessCustomerPayload";
import type { UpsertBusinessMilestonePayload } from "../generated/bsaigc/UpsertBusinessMilestonePayload";
import type { ChangeBusinessDocumentStatusPayload } from "../generated/bsaigc/ChangeBusinessDocumentStatusPayload";
import type { ChangeBusinessWorkspaceStatusPayload } from "../generated/bsaigc/ChangeBusinessWorkspaceStatusPayload";
import type { ConfirmBusinessQuotePayload } from "../generated/bsaigc/ConfirmBusinessQuotePayload";
import type { RecordBusinessReceiptPayload } from "../generated/bsaigc/RecordBusinessReceiptPayload";
import type { ReverseBusinessReceiptPayload } from "../generated/bsaigc/ReverseBusinessReceiptPayload";
import type { AdoptLatestConfirmedRequirementPayload } from "../generated/bsaigc/AdoptLatestConfirmedRequirementPayload";
import type { CreateBusinessDocumentPayload } from "../generated/bsaigc/CreateBusinessDocumentPayload";
import type { GenerateBusinessDocumentPayload } from "../generated/bsaigc/GenerateBusinessDocumentPayload";
import type { PromoteReviewedContractPayload } from "../generated/bsaigc/PromoteReviewedContractPayload";
import type { ListBusinessWorkspacePrefillCandidatesRequest } from "../generated/bsaigc/ListBusinessWorkspacePrefillCandidatesRequest";
import type { PreviewBusinessWorkspacePrefillRequest } from "../generated/bsaigc/PreviewBusinessWorkspacePrefillRequest";
import type { UpdateBusinessProfilePayload } from "../generated/bsaigc/UpdateBusinessProfilePayload";
import type { UpsertBusinessPaymentPayload } from "../generated/bsaigc/UpsertBusinessPaymentPayload";
import type { AssetBackupRecord } from "../generated/bsaigc/AssetBackupRecord";
import type { BackupCommandEnvelope } from "../generated/bsaigc/BackupCommandEnvelope";
import type { BackupCommandResponse } from "../generated/bsaigc/BackupCommandResponse";
import type { BackupDomainEvent } from "../generated/bsaigc/BackupDomainEvent";
import type { CancelAssetBackupPayload } from "../generated/bsaigc/CancelAssetBackupPayload";
import type { CancelContractReviewPayload } from "../generated/bsaigc/CancelContractReviewPayload";
import type { ContractReviewCommandEnvelope } from "../generated/bsaigc/ContractReviewCommandEnvelope";
import type { ContractReviewCommandResponse } from "../generated/bsaigc/ContractReviewCommandResponse";
import type { ContractReviewDomainEvent } from "../generated/bsaigc/ContractReviewDomainEvent";
import type { ContractReviewRecord } from "../generated/bsaigc/ContractReviewRecord";
import type { CreateContractReviewPayload } from "../generated/bsaigc/CreateContractReviewPayload";
import type { DecideReviewFindingPayload } from "../generated/bsaigc/DecideReviewFindingPayload";
import type { EvidenceContext } from "../generated/bsaigc/EvidenceContext";
import type { GenerateReviewReportPayload } from "../generated/bsaigc/GenerateReviewReportPayload";
import type { GetContractReviewRequest } from "../generated/bsaigc/GetContractReviewRequest";
import type { GetEvidenceContextRequest } from "../generated/bsaigc/GetEvidenceContextRequest";
import type { QueueAssetBackupPayload } from "../generated/bsaigc/QueueAssetBackupPayload";
import type { RestoreAssetBackupPayload } from "../generated/bsaigc/RestoreAssetBackupPayload";
import type { RetryAssetBackupPayload } from "../generated/bsaigc/RetryAssetBackupPayload";
import type { RetryContractReviewStagePayload } from "../generated/bsaigc/RetryContractReviewStagePayload";
import type { ReviewFindingRecord } from "../generated/bsaigc/ReviewFindingRecord";
import type { AiCredentialCommandEnvelope } from "../generated/bsaigc/AiCredentialCommandEnvelope";
import type { AiCredentialCommandResponse } from "../generated/bsaigc/AiCredentialCommandResponse";
import type { AiCredentialStatus } from "../generated/bsaigc/AiCredentialStatus";
import type { DiscoverAiProviderModelsPayload } from "../generated/bsaigc/DiscoverAiProviderModelsPayload";
import type { UpsertAiProviderPayload } from "../generated/bsaigc/UpsertAiProviderPayload";
import type { DesktopSettingsCommandEnvelope } from "../generated/bsaigc/DesktopSettingsCommandEnvelope";
import type { DesktopSettingsCommandResponse } from "../generated/bsaigc/DesktopSettingsCommandResponse";
import type { DesktopSettingsSnapshot } from "../generated/bsaigc/DesktopSettingsSnapshot";
import type { StorageLocationTarget } from "../generated/bsaigc/StorageLocationTarget";
import type { StartContractReviewPayload } from "../generated/bsaigc/StartContractReviewPayload";
import { AssetProjection } from "./AssetProjection";
import { BrainProjection } from "./BrainProjection";
import { BusinessWorkspaceProjection } from "./BusinessWorkspaceProjection";
import { CaseProjection } from "./CaseProjection";
import { EventProjection } from "./EventProjection";
import { ExecutionBriefProjection } from "./ExecutionBriefProjection";
import { RequirementBriefProjection } from "./RequirementBriefProjection";
import { TaskProjection } from "./TaskProjection";
import {
  BSAIGC_PROTOCOL_VERSION,
  BUSINESS_WORKSPACE_PROTOCOL_VERSION,
  type AssetActionCapabilities,
  type BackupEventListener,
  type ContractReviewEventListener,
  type CreateBusinessWorkspaceInput,
  type HostAdapter,
  type ListBusinessCustomersInput,
  type ListBusinessWorkspacePrefillCandidatesInput,
  type ListContractReviewsInput,
  type ListReviewFindingsInput,
  type Unsubscribe,
} from "./HostAdapter";

const REPLAY_PAGE_SIZE = 200;
const EXECUTION_BRIEF_GAP_RECOVERY_ATTEMPTS = 3;
const REQUIREMENT_BRIEF_GAP_RECOVERY_ATTEMPTS = 3;
const BUSINESS_WORKSPACE_GAP_RECOVERY_ATTEMPTS = 3;
const DEFAULT_COMMAND_DEADLINE_MS = 30_000;

export interface BsaigcClientOptions {
  readonly actorId?: string;
  readonly accountId?: string | null;
  readonly windowId?: string;
  readonly protocolVersion?: string;
  readonly businessWorkspaceProtocolVersion?: string;
  readonly commandDeadlineMs?: number;
  readonly now?: () => number;
  readonly uuid?: () => string;
}

export interface CommandOptions {
  readonly actorId?: string;
  readonly accountId?: string | null;
  readonly windowId?: string;
  readonly commandId?: string;
  readonly traceId?: string;
  readonly idempotencyKey?: string;
  readonly deadlineAt?: number | null;
  readonly deadlineMs?: number;
}

export interface ScopedCommandOptions extends CommandOptions {
  readonly projectId?: string | null;
}

export interface UpdateProjectBriefInput extends UpdateProjectBriefPayload {
  readonly expectedRevision: number;
}

export interface ChangeProjectStageInput extends ChangeProjectStagePayload {
  readonly expectedRevision: number;
}

export interface BsaigcClientSnapshot {
  readonly projects: ReturnType<EventProjection["snapshot"]>["projects"];
  readonly events: ReturnType<EventProjection["snapshot"]>["events"];
  readonly lastSequence: number;
  readonly tasks: ReturnType<TaskProjection["snapshot"]>["tasks"];
  readonly taskEvents: ReturnType<TaskProjection["snapshot"]>["events"];
  readonly taskLastSequence: number;
  readonly assets: ReturnType<AssetProjection["snapshot"]>["assets"];
  readonly assetEvents: ReturnType<AssetProjection["snapshot"]>["events"];
  readonly assetLastSequence: number;
  readonly brainThreads: readonly BrainThreadRecord[];
  readonly brainTurns: readonly BrainTurnRecord[];
  readonly brainStreamingByTurn: Readonly<Record<string, string>>;
  readonly lastBrainEvent: BrainStreamEvent | null;
  readonly cases: readonly CaseRecord[];
  readonly caseEvents: readonly CaseDomainEvent[];
  readonly caseLastSequence: number;
  readonly executionBriefs: readonly ExecutionBriefRecord[];
  readonly executionBriefEvents: readonly ExecutionBriefDomainEvent[];
  readonly executionBriefLastSequence: number;
  readonly requirementBriefs: readonly RequirementBriefRecord[];
  readonly requirementBriefEvents: readonly RequirementBriefDomainEvent[];
  readonly requirementBriefLastSequence: number;
  readonly businessWorkspaces: readonly BusinessWorkspaceRecord[];
  readonly businessWorkspaceEvents: readonly BusinessWorkspaceDomainEvent[];
  readonly businessWorkspaceLastSequence: number;
  readonly started: boolean;
  readonly synchronizing: boolean;
  readonly error: HostError | null;
}

type StoreListener = () => void;

export class BsaigcClient {
  private readonly projection = new EventProjection();
  private readonly taskProjection = new TaskProjection();
  private readonly assetProjection = new AssetProjection();
  private readonly brainProjection = new BrainProjection();
  private readonly caseProjection = new CaseProjection();
  private readonly executionBriefProjection = new ExecutionBriefProjection();
  private readonly requirementBriefProjection = new RequirementBriefProjection();
  private readonly businessWorkspaceProjection =
    new BusinessWorkspaceProjection();
  private readonly listeners = new Set<StoreListener>();
  private readonly actorId: string;
  private readonly accountId: string | null;
  private readonly windowId: string;
  private readonly protocolVersion: string;
  private readonly businessWorkspaceProtocolVersion: string;
  private readonly commandDeadlineMs: number;
  private readonly now: () => number;
  private readonly uuid: () => string;
  private unsubscribeHost: Unsubscribe | null = null;
  private bufferedEvents: DomainEvent[] = [];
  private bufferedTaskEvents: TaskDomainEvent[] = [];
  private bufferedAssetEvents: AssetDomainEvent[] = [];
  private bufferedBrainEvents: BrainStreamEvent[] = [];
  private bufferedCaseEvents: CaseDomainEvent[] = [];
  private bufferedExecutionBriefEvents: ExecutionBriefDomainEvent[] = [];
  private bufferedRequirementBriefEvents: RequirementBriefDomainEvent[] = [];
  private bufferedBusinessWorkspaceEvents: BusinessWorkspaceDomainEvent[] = [];
  private bufferingEvents = false;
  private lifecycleGeneration = 0;
  private startPromise: Promise<void> | null = null;
  private caseGapRecovery: Promise<void> | null = null;
  private caseGapTargetSequence = 0;
  private executionBriefGapRecovery: Promise<void> | null = null;
  private executionBriefGapTargetSequence = 0;
  private requirementBriefGapRecovery: Promise<void> | null = null;
  private requirementBriefGapTargetSequence = 0;
  private requirementBriefGapError: HostError | null = null;
  private businessWorkspaceGapRecovery: Promise<void> | null = null;
  private businessWorkspaceGapTargetSequence = 0;
  private businessWorkspaceGapError: HostError | null = null;
  private started = false;
  private synchronizing = false;
  private error: HostError | null = null;
  private currentSnapshot: BsaigcClientSnapshot;

  constructor(
    private readonly host: HostAdapter,
    options: BsaigcClientOptions = {},
  ) {
    this.actorId = options.actorId ?? "local-user";
    this.accountId = options.accountId ?? null;
    this.windowId = options.windowId ?? "main-window";
    this.protocolVersion = options.protocolVersion ?? BSAIGC_PROTOCOL_VERSION;
    this.businessWorkspaceProtocolVersion =
      options.businessWorkspaceProtocolVersion ?? BUSINESS_WORKSPACE_PROTOCOL_VERSION;
    this.commandDeadlineMs =
      options.commandDeadlineMs ?? DEFAULT_COMMAND_DEADLINE_MS;
    this.now = options.now ?? Date.now;
    this.uuid = options.uuid ?? createUuid;
    this.currentSnapshot = this.buildSnapshot();
  }

  readonly getSnapshot = (): BsaigcClientSnapshot => this.currentSnapshot;

  readonly subscribe = (listener: StoreListener): Unsubscribe => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  start(): Promise<void> {
    if (this.started && !this.synchronizing) {
      return Promise.resolve();
    }
    if (this.startPromise) {
      return this.startPromise;
    }

    const generation = ++this.lifecycleGeneration;
    this.bufferedEvents = [];
    this.bufferedTaskEvents = [];
    this.bufferedAssetEvents = [];
    this.bufferedBrainEvents = [];
    this.bufferedCaseEvents = [];
    this.bufferedExecutionBriefEvents = [];
    this.bufferedRequirementBriefEvents = [];
    this.bufferedBusinessWorkspaceEvents = [];
    this.bufferingEvents = true;
    this.started = false;
    this.synchronizing = true;
    this.error = null;
    // Reset any recovery state left over from a failed prior start so a stale
    // gap target can never inflate the next legitimate recovery window.
    this.caseGapRecovery = null;
    this.caseGapTargetSequence = 0;
    this.executionBriefGapRecovery = null;
    this.executionBriefGapTargetSequence = 0;
    this.requirementBriefGapRecovery = null;
    this.requirementBriefGapTargetSequence = 0;
    this.requirementBriefGapError = null;
    this.businessWorkspaceGapRecovery = null;
    this.businessWorkspaceGapTargetSequence = 0;
    this.businessWorkspaceGapError = null;
    this.publish();

    const operation = this.performStart(generation);
    this.startPromise = operation;
    void operation.then(
      () => this.clearStartPromise(operation),
      () => this.clearStartPromise(operation),
    );
    return operation;
  }

  stop(): void {
    ++this.lifecycleGeneration;
    this.startPromise = null;
    this.caseGapRecovery = null;
    this.caseGapTargetSequence = 0;
    this.executionBriefGapRecovery = null;
    this.executionBriefGapTargetSequence = 0;
    this.requirementBriefGapRecovery = null;
    this.requirementBriefGapTargetSequence = 0;
    this.requirementBriefGapError = null;
    this.businessWorkspaceGapRecovery = null;
    this.businessWorkspaceGapTargetSequence = 0;
    this.businessWorkspaceGapError = null;
    this.bufferingEvents = false;
    this.bufferedEvents = [];
    this.bufferedTaskEvents = [];
    this.bufferedAssetEvents = [];
    this.bufferedBrainEvents = [];
    this.bufferedCaseEvents = [];
    this.bufferedExecutionBriefEvents = [];
    this.bufferedRequirementBriefEvents = [];
    this.bufferedBusinessWorkspaceEvents = [];
    this.unsubscribeHost?.();
    this.unsubscribeHost = null;

    const businessWorkspaceChanged = this.businessWorkspaceProjection.reset();
    const changed =
      this.started || this.synchronizing || businessWorkspaceChanged;
    this.started = false;
    this.synchronizing = false;
    if (changed) {
      this.publish();
    }
  }

  private clearStreamGapError(code: string): void {
    if (this.error?.code === code) {
      this.error = null;
    }
  }

  clearError(): void {
    if (
      this.error ||
      this.requirementBriefGapError ||
      this.businessWorkspaceGapError
    ) {
      this.error = null;
      // Gap errors are surfaced through the same snapshot error slot, so a
      // user-facing dismiss must clear them too; recovery still continues in
      // the background and re-raises if the gap persists.
      this.requirementBriefGapError = null;
      this.businessWorkspaceGapError = null;
      this.publish();
    }
  }

  createProject(
    payload: CreateProjectPayload,
    options?: CommandOptions,
  ): Promise<CommandResponse>;
  createProject(
    name: string,
    clientName: string,
    options?: CommandOptions,
  ): Promise<CommandResponse>;
  createProject(
    payloadOrName: CreateProjectPayload | string,
    optionsOrClientName: CommandOptions | string = {},
    positionalOptions: CommandOptions = {},
  ): Promise<CommandResponse> {
    const payload: CreateProjectPayload =
      typeof payloadOrName === "string"
        ? { name: payloadOrName, clientName: String(optionsOrClientName) }
        : payloadOrName;
    const options =
      typeof payloadOrName === "string"
        ? positionalOptions
        : (optionsOrClientName as CommandOptions);

    const envelope: CommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "project.create",
      payload,
      expectedRevision: null,
    };
    return this.execute(envelope);
  }

  updateProjectBrief(
    input: UpdateProjectBriefInput,
    options?: CommandOptions,
  ): Promise<CommandResponse>;
  updateProjectBrief(
    projectId: string,
    brief: BriefRecord,
    expectedRevision: number,
    options?: CommandOptions,
  ): Promise<CommandResponse>;
  updateProjectBrief(
    inputOrProjectId: UpdateProjectBriefInput | string,
    optionsOrBrief: CommandOptions | BriefRecord = {},
    positionalRevision?: number,
    positionalOptions: CommandOptions = {},
  ): Promise<CommandResponse> {
    const input: UpdateProjectBriefInput =
      typeof inputOrProjectId === "string"
        ? {
            projectId: inputOrProjectId,
            brief: optionsOrBrief as BriefRecord,
            expectedRevision: requireRevision(positionalRevision),
          }
        : inputOrProjectId;
    const options =
      typeof inputOrProjectId === "string"
        ? positionalOptions
        : (optionsOrBrief as CommandOptions);
    const payload: UpdateProjectBriefPayload = {
      projectId: input.projectId,
      brief: input.brief,
    };
    const envelope: CommandEnvelope = {
      ...this.commandBase(input.projectId, options),
      commandType: "project.updateBrief",
      payload,
      expectedRevision: input.expectedRevision,
    };
    return this.execute(envelope);
  }

  changeProjectStage(
    input: ChangeProjectStageInput,
    options?: CommandOptions,
  ): Promise<CommandResponse>;
  changeProjectStage(
    projectId: string,
    stage: ProjectStage,
    expectedRevision: number,
    options?: CommandOptions,
  ): Promise<CommandResponse>;
  changeProjectStage(
    inputOrProjectId: ChangeProjectStageInput | string,
    optionsOrStage: CommandOptions | ProjectStage = {},
    positionalRevision?: number,
    positionalOptions: CommandOptions = {},
  ): Promise<CommandResponse> {
    const input: ChangeProjectStageInput =
      typeof inputOrProjectId === "string"
        ? {
            projectId: inputOrProjectId,
            stage: optionsOrStage as ProjectStage,
            expectedRevision: requireRevision(positionalRevision),
          }
        : inputOrProjectId;
    const options =
      typeof inputOrProjectId === "string"
        ? positionalOptions
        : (optionsOrStage as CommandOptions);
    const payload: ChangeProjectStagePayload = {
      projectId: input.projectId,
      stage: input.stage,
    };
    const envelope: CommandEnvelope = {
      ...this.commandBase(input.projectId, options),
      commandType: "project.changeStage",
      payload,
      expectedRevision: input.expectedRevision,
    };
    return this.execute(envelope);
  }

  createTask(
    payload: CreateTaskPayload,
    options: CommandOptions = {},
  ): Promise<TaskCommandResponse> {
    const envelope: TaskCommandEnvelope = {
      ...this.commandBase(payload.projectId, options),
      commandType: "task.create",
      payload,
      expectedRevision: null,
    };
    return this.executeTask(envelope);
  }

  cancelTask(
    taskId: string,
    expectedRevision: number,
    reason: string | null = null,
    options: CommandOptions = {},
  ): Promise<TaskCommandResponse> {
    const task = this.taskProjection
      .snapshot()
      .tasks.find((candidate) => candidate.id === taskId);
    const envelope: TaskCommandEnvelope = {
      ...this.commandBase(task?.projectId ?? null, options),
      commandType: "task.cancel",
      payload: { taskId, reason },
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.executeTask(envelope);
  }

  retryTask(
    taskId: string,
    expectedRevision: number,
    approved: boolean,
    options: CommandOptions = {},
  ): Promise<TaskCommandResponse> {
    const task = this.taskProjection
      .snapshot()
      .tasks.find((candidate) => candidate.id === taskId);
    const envelope: TaskCommandEnvelope = {
      ...this.commandBase(task?.projectId ?? null, options),
      commandType: "task.retry",
      payload: { taskId, approved },
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.executeTask(envelope);
  }

  selectAssetSource(): Promise<AssetSourceSelection | null> {
    return this.callHost(() => this.host.selectAssetSource());
  }

  selectAssetSources(): Promise<AssetSourceSelection[]> {
    return this.callHost(async () => {
      if (this.host.selectAssetSources) return this.host.selectAssetSources();
      const source = await this.host.selectAssetSource();
      return source ? [source] : [];
    });
  }

  selectBrainWorkspace(): Promise<BrainWorkspaceSelection | null> {
    if (!this.host.selectBrainWorkspace) {
      return Promise.reject(hostError(
        "BRAIN_WORKSPACE_UNAVAILABLE",
        "当前运行环境不支持选择工作区。",
        false,
      ));
    }
    return this.callHost(() => this.host.selectBrainWorkspace!());
  }

  registerBrainDroppedPaths(paths: string[]): Promise<BrainDroppedItems> {
    if (!this.host.registerBrainDroppedPaths) {
      return Promise.reject(hostError(
        "BRAIN_DROP_UNAVAILABLE",
        "当前运行环境不支持拖放本地文件。",
        false,
      ));
    }
    return this.callHost(() => this.host.registerBrainDroppedPaths!(paths));
  }

  stageClipboardImage(
    request: StageClipboardImageRequest,
  ): Promise<AssetSourceSelection> {
    if (!this.host.stageClipboardImage) {
      return Promise.reject(hostError(
        "BRAIN_CLIPBOARD_UNAVAILABLE",
        "当前运行环境不支持粘贴截图。",
        false,
      ));
    }
    return this.callHost(() => this.host.stageClipboardImage!(request));
  }

  getBrainAttachmentPreview(
    assetId: string,
  ): Promise<BrainAttachmentPreview | null> {
    if (!this.host.getBrainAttachmentPreview) return Promise.resolve(null);
    return this.callHost(() => this.host.getBrainAttachmentPreview!(assetId));
  }

  importAsset(
    sourceToken: string,
    projectId: string | null,
    options: CommandOptions = {},
  ): Promise<AssetCommandResponse> {
    const envelope: AssetCommandEnvelope = {
      ...this.commandBase(projectId, options),
      commandType: "asset.import",
      payload: { sourceToken, projectId },
      expectedRevision: null,
    };
    return this.executeAsset(envelope);
  }

  refreshTasks(): Promise<readonly TaskRecord[]> {
    return this.callHost(async () => {
      const tasks = await this.host.listTasks();
      this.taskProjection.hydrate(tasks);
      await this.replayTaskPages(
        this.taskProjection.snapshot().lastSequence,
        this.lifecycleGeneration,
      );
      this.publish();
      return this.taskProjection.snapshot().tasks;
    });
  }

  getAssetActionCapabilities(assetId: string): Promise<AssetActionCapabilities> {
    const stableAssetId = requireStableAssetId(assetId);
    if (!this.host.getAssetActionCapabilities) {
      return Promise.resolve({
        assetId: stableAssetId,
        canOpen: false,
        canExport: false,
        reason: "当前运行环境不支持打开或导出文件。",
      });
    }
    return this.callHost(() => this.host.getAssetActionCapabilities!(stableAssetId));
  }

  async openAsset(assetId: string): Promise<void> {
    const stableAssetId = requireStableAssetId(assetId);
    const capabilities = await this.getAssetActionCapabilities(stableAssetId);
    if (!capabilities.canOpen || !this.host.openAsset) {
      throw hostError(
        "ASSET_OPEN_UNAVAILABLE",
        capabilities.reason ?? "当前文件暂时无法打开。",
        false,
      );
    }
    await this.callHost(() => this.host.openAsset!(stableAssetId));
  }

  async exportAsset(assetId: string): Promise<boolean> {
    const stableAssetId = requireStableAssetId(assetId);
    const capabilities = await this.getAssetActionCapabilities(stableAssetId);
    if (!capabilities.canExport || !this.host.exportAsset) {
      throw hostError(
        "ASSET_EXPORT_UNAVAILABLE",
        capabilities.reason ?? "当前文件暂时无法导出。",
        false,
      );
    }
    return this.callHost(() => this.host.exportAsset!(stableAssetId));
  }

  refreshAssets(): Promise<readonly AssetRecord[]> {
    return this.callHost(async () => {
      const assets = await this.host.listAssets();
      this.assetProjection.hydrate(assets);
      await this.replayAssetPages(
        this.assetProjection.snapshot().lastSequence,
        this.lifecycleGeneration,
      );
      this.publish();
      return this.assetProjection.snapshot().assets;
    });
  }

  getHostStatus(): Promise<HostStatus> {
    return this.callHost(() => this.host.getHostStatus());
  }

  listPendingApprovals(): Promise<ApprovalRecord[]> {
    return this.callHost(() => this.host.listPendingApprovals());
  }

  resolveApproval(
    approvalId: string,
    approved: boolean,
  ): Promise<ApprovalRecord> {
    return this.callHost(() =>
      this.host.resolveApproval({ approvalId, approved }),
    );
  }

  probeCodex(): Promise<CodexProbeStatus> {
    return this.callHost(() => this.host.probeCodex());
  }

  startBrainThread(request: StartBrainThreadRequest): Promise<BrainThreadRecord> {
    return this.callHost(async () => {
      const thread = await this.host.startBrainThread(request);
      this.brainProjection.upsertThreads([thread]);
      this.publish();
      return thread;
    });
  }

  resumeBrainThread(request: ResumeBrainThreadRequest): Promise<BrainThreadRecord> {
    return this.callHost(async () => {
      const thread = await this.host.resumeBrainThread(request);
      this.brainProjection.upsertThreads([thread]);
      this.publish();
      return thread;
    });
  }

  listRemoteBrainThreads(
    request: ListRemoteBrainThreadsRequest,
  ): Promise<RemoteBrainThreadPage> {
    return this.callHost(async () => {
      const page = await this.host.listRemoteBrainThreads(request);
      this.brainProjection.upsertThreads(page.threads);
      this.publish();
      return page;
    });
  }

  startBrainTurn(
    request: StartBrainTurnRequest,
    context?: BrainTurnContext,
  ): Promise<BrainTurnStartResult> {
    return this.callHost(async () => {
      const result = await this.host.startBrainTurn(request, context);
      this.brainProjection.upsertTurns([result.turn]);
      this.publish();
      return result;
    });
  }

  interruptBrainTurn(threadId: string, turnId: string): Promise<BrainTurnRecord> {
    return this.callHost(async () => {
      const turn = await this.host.interruptBrainTurn({ threadId, turnId });
      this.brainProjection.upsertTurns([turn]);
      this.brainProjection.clearStreaming(turn.id);
      this.publish();
      return turn;
    });
  }

  refreshBrainThreads(projectId: string | null = null): Promise<readonly BrainThreadRecord[]> {
    return this.callHost(async () => {
      const threads = await this.host.listLocalBrainThreads(projectId);
      if (projectId === null) this.brainProjection.replaceThreads(threads);
      else this.brainProjection.upsertThreads(threads);
      this.publish();
      return threads;
    });
  }

  refreshBrainTurns(threadId: string): Promise<readonly BrainTurnRecord[]> {
    return this.callHost(async () => {
      const turns = await this.host.listLocalBrainTurns(threadId);
      this.brainProjection.replaceTurns(threadId, turns);
      this.publish();
      return turns;
    });
  }

  getBrainHealth(): Promise<BrainHostHealth> {
    return this.callHost(() => this.host.getBrainHealth());
  }

  getNativeMediaHealth(): Promise<NativeMediaHealth> {
    return this.callHost(() => this.host.getNativeMediaHealth());
  }

  createCase(
    payload: CreateCasePayload,
    options: CommandOptions = {},
  ): Promise<CaseCommandResponse> {
    const command: CaseCommandEnvelope = {
      ...this.commandBase(payload.projectId, options),
      commandType: "case.create",
      payload,
      expectedRevision: null,
    };
    return this.executeCase(command);
  }

  updateCase(
    payload: UpdateCasePayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<CaseCommandResponse> {
    const record = this.caseProjection
      .snapshot()
      .cases.find((candidate) => candidate.id === payload.caseId);
    const command: CaseCommandEnvelope = {
      ...this.commandBase(record?.projectId ?? null, options),
      commandType: "case.update",
      payload,
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.executeCase(command);
  }

  refreshCases(): Promise<readonly CaseRecord[]> {
    return this.callHost(async () => {
      const cases = await this.host.listCases();
      this.caseProjection.hydrate(cases);
      await this.replayCasePages(
        this.caseProjection.snapshot().lastSequence,
        this.lifecycleGeneration,
      );
      this.publish();
      return this.caseProjection.snapshot().cases;
    });
  }

  createRequirementBrief(
    payload: CreateRequirementBriefPayload,
    options: CommandOptions = {},
  ): Promise<RequirementBriefCommandResponse> {
    const command: RequirementBriefCommandEnvelope = {
      ...this.commandBase(payload.projectId, options),
      commandType: "requirementBrief.create",
      payload,
      expectedRevision: null,
    };
    return this.executeRequirementBrief(command);
  }

  updateRequirementBrief(
    payload: UpdateRequirementBriefPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<RequirementBriefCommandResponse> {
    const requirementBrief = this.requirementBriefProjection
      .snapshot()
      .requirementBriefs.find((candidate) => candidate.id === payload.briefId);
    const command: RequirementBriefCommandEnvelope = {
      ...this.commandBase(requirementBrief?.projectId ?? null, options),
      commandType: "requirementBrief.update",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeRequirementBrief(command);
  }

  changeRequirementBriefStatus(
    briefId: string,
    status: RequirementBriefStatus,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<RequirementBriefCommandResponse> {
    const requirementBrief = this.requirementBriefProjection
      .snapshot()
      .requirementBriefs.find((candidate) => candidate.id === briefId);
    const command: RequirementBriefCommandEnvelope = {
      ...this.commandBase(requirementBrief?.projectId ?? null, options),
      commandType: "requirementBrief.changeStatus",
      payload: { briefId, status },
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeRequirementBrief(command);
  }

  refreshRequirementBriefs(): Promise<readonly RequirementBriefRecord[]> {
    return this.callHost(async () => {
      const requirementBriefs = await this.host.listRequirementBriefs();
      this.requirementBriefProjection.hydrate(requirementBriefs);
      await this.replayRequirementBriefPages(
        this.requirementBriefProjection.snapshot().lastSequence,
        this.lifecycleGeneration,
      );
      this.settleRequirementBriefGapIfCaughtUp();
      this.publish();
      return this.requirementBriefProjection.snapshot().requirementBriefs;
    });
  }

  createBusinessWorkspace(
    payload: CreateBusinessWorkspaceInput,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(payload.projectId, options),
      commandType: "businessWorkspace.create",
      payload: {
        projectId: payload.projectId,
        customerId: payload.customerId ?? null,
        prefillSourceWorkspaceId: payload.prefillSourceWorkspaceId ?? null,
      },
      expectedRevision: null,
    };
    return this.executeBusinessWorkspace(command);
  }

  updateBusinessProfile(
    payload: UpdateBusinessProfilePayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.updateProfile",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  createBusinessDocument(
    payload: CreateBusinessDocumentPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.createDocument",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  promoteReviewedContract(
    payload: PromoteReviewedContractPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.promoteReviewedContract",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  changeBusinessDocumentStatus(
    payload: ChangeBusinessDocumentStatusPayload,
    expectedRevision: number,
    options?: CommandOptions,
  ): Promise<BusinessWorkspaceCommandResponse>;
  changeBusinessDocumentStatus(
    workspaceId: string,
    documentId: string,
    status: BusinessDocumentStatus,
    expectedRevision: number,
    options?: CommandOptions,
  ): Promise<BusinessWorkspaceCommandResponse>;
  changeBusinessDocumentStatus(
    payloadOrWorkspaceId: ChangeBusinessDocumentStatusPayload | string,
    expectedRevisionOrDocumentId: number | string,
    optionsOrStatus: CommandOptions | BusinessDocumentStatus = {},
    positionalExpectedRevision?: number,
    positionalOptions: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const payload: ChangeBusinessDocumentStatusPayload =
      typeof payloadOrWorkspaceId === "string"
        ? {
            workspaceId: payloadOrWorkspaceId,
            documentId: expectedRevisionOrDocumentId as string,
            status: optionsOrStatus as BusinessDocumentStatus,
            evidence: null,
            manualWaiver: null,
            reason: "",
          }
        : payloadOrWorkspaceId;
    const expectedRevision =
      typeof payloadOrWorkspaceId === "string"
        ? positionalExpectedRevision
        : (expectedRevisionOrDocumentId as number);
    const options =
      typeof payloadOrWorkspaceId === "string"
        ? positionalOptions
        : (optionsOrStatus as CommandOptions);
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.changeDocumentStatus",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  generateBusinessDocument(
    payload: GenerateBusinessDocumentPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.generateDocument",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  upsertBusinessPayment(
    payload: UpsertBusinessPaymentPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.upsertPayment",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  confirmBusinessQuote(
    payload: ConfirmBusinessQuotePayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.confirmQuote",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  recordBusinessReceipt(
    payload: RecordBusinessReceiptPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.recordReceipt",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  reverseBusinessReceipt(
    payload: ReverseBusinessReceiptPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.reverseReceipt",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  adoptLatestConfirmedRequirement(
    payload: AdoptLatestConfirmedRequirementPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.adoptLatestConfirmedRequirement",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  upsertBusinessCustomer(
    payload: UpsertBusinessCustomerPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.upsertCustomer",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  assignBusinessCustomer(
    payload: AssignBusinessCustomerPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.assignCustomer",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  upsertBusinessMilestone(
    payload: UpsertBusinessMilestonePayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.upsertMilestone",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  registerBusinessDeliverableVersion(
    payload: RegisterBusinessDeliverableVersionPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.registerDeliverableVersion",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  recordBusinessDeliverySent(
    payload: RecordBusinessDeliverySentPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.recordDeliverySent",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  recordBusinessDeliverySignoff(
    payload: RecordBusinessDeliverySignoffPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.recordDeliverySignoff",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  recordBusinessInvoiceIssued(
    payload: RecordBusinessInvoiceIssuedPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.recordInvoiceIssued",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  recordBusinessInvoiceRedCorrection(
    payload: RecordBusinessInvoiceRedCorrectionPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.recordInvoiceRedCorrection",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  attachBusinessInvoiceAsset(
    payload: AttachBusinessInvoiceAssetPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.attachInvoiceAsset",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  createBusinessArchiveSnapshot(
    payload: CreateBusinessArchiveSnapshotPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.createArchiveSnapshot",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  changeBusinessWorkspaceStatus(
    payload: ChangeBusinessWorkspaceStatusPayload,
    expectedRevision: number,
    options?: CommandOptions,
  ): Promise<BusinessWorkspaceCommandResponse>;
  changeBusinessWorkspaceStatus(
    workspaceId: string,
    status: BusinessWorkspaceStatus,
    expectedRevision: number,
    options?: CommandOptions,
  ): Promise<BusinessWorkspaceCommandResponse>;
  changeBusinessWorkspaceStatus(
    payloadOrWorkspaceId: ChangeBusinessWorkspaceStatusPayload | string,
    expectedRevisionOrStatus: number | BusinessWorkspaceStatus,
    optionsOrExpectedRevision: CommandOptions | number = {},
    positionalOptions: CommandOptions = {},
  ): Promise<BusinessWorkspaceCommandResponse> {
    const payload: ChangeBusinessWorkspaceStatusPayload =
      typeof payloadOrWorkspaceId === "string"
        ? {
            workspaceId: payloadOrWorkspaceId,
            status: expectedRevisionOrStatus as BusinessWorkspaceStatus,
          }
        : payloadOrWorkspaceId;
    const expectedRevision =
      typeof payloadOrWorkspaceId === "string"
        ? (optionsOrExpectedRevision as number)
        : (expectedRevisionOrStatus as number);
    const options =
      typeof payloadOrWorkspaceId === "string"
        ? positionalOptions
        : (optionsOrExpectedRevision as CommandOptions);
    const command: BusinessWorkspaceCommandEnvelope = {
      ...this.commandBase(
        this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "businessWorkspace.changeStatus",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBusinessWorkspace(command);
  }

  refreshBusinessWorkspaces(): Promise<readonly BusinessWorkspaceRecord[]> {
    return this.callHost(async () => {
      const businessWorkspaces = await this.host.listBusinessWorkspaces();
      this.businessWorkspaceProjection.hydrate(businessWorkspaces);
      await this.replayBusinessWorkspacePages(
        this.businessWorkspaceProjection.snapshot().lastSequence,
        this.lifecycleGeneration,
      );
      this.settleBusinessWorkspaceGapIfCaughtUp();
      this.publish();
      return this.businessWorkspaceProjection.snapshot().businessWorkspaces;
    });
  }

  listBusinessCustomers(
    request: ListBusinessCustomersInput = {},
  ): Promise<BusinessCustomerReceivableSummary[]> {
    return this.callHost(() =>
      this.host.listBusinessCustomers({
        query: request.query ?? "",
        // `null` is a meaningful "no limit" request; only default when the
        // caller omitted the field entirely.
        limit: request.limit === undefined ? 100 : request.limit,
      }),
    );
  }

  brainThreadArchive(
    threadId: string,
    archived: boolean,
  ): Promise<BrainThreadRecord> {
    return this.host.brainThreadArchive(threadId, archived);
  }

  brainThreadRename(threadId: string, title: string): Promise<BrainThreadRecord> {
    return this.host.brainThreadRename(threadId, title);
  }

  brainThreadDelete(threadId: string): Promise<void> {
    return this.host.brainThreadDelete(threadId);
  }

  authStatus(): Promise<AuthStatus> {
    return this.host.authStatus();
  }

  authInitializeAdmin(credentials: AuthCredentials): Promise<AuthStatus> {
    return this.host.authInitializeAdmin(credentials);
  }

  authLogin(credentials: AuthCredentials): Promise<AuthStatus> {
    return this.host.authLogin(credentials);
  }

  authLogout(): Promise<AuthStatus> {
    return this.host.authLogout();
  }

  authRememberedCredentials(): Promise<AuthCredentials | null> {
    return this.host.authRememberedCredentials();
  }

  authRememberCredentials(credentials: AuthCredentials): Promise<void> {
    return this.host.authRememberCredentials(credentials);
  }

  authForgetCredentials(): Promise<void> {
    return this.host.authForgetCredentials();
  }

  authChangePassword(payload: AuthChangePasswordPayload): Promise<AuthStatus> {
    return this.host.authChangePassword(payload);
  }

  authListUsers(): Promise<AuthUsersSnapshot> {
    return this.host.authListUsers();
  }

  authCreateUser(payload: AuthCreateUserPayload): Promise<AuthUsersSnapshot> {
    return this.host.authCreateUser(payload);
  }

  authResetPassword(payload: AuthResetPasswordPayload): Promise<AuthUsersSnapshot> {
    return this.host.authResetPassword(payload);
  }

  authDeleteUser(payload: AuthDeleteUserPayload): Promise<AuthUsersSnapshot> {
    return this.host.authDeleteUser(payload);
  }

  authRefreshRegistry(): Promise<AuthStatus> {
    return this.host.authRefreshRegistry();
  }

  listBusinessWorkspacePrefillCandidates(
    request: ListBusinessWorkspacePrefillCandidatesInput,
  ): Promise<BusinessWorkspacePrefillCandidate[]> {
    const hostRequest: ListBusinessWorkspacePrefillCandidatesRequest =
      request.limit === undefined
        ? { ...request, limit: 50 }
        : (request as ListBusinessWorkspacePrefillCandidatesRequest);
    return this.callHost(() =>
      this.host.listBusinessWorkspacePrefillCandidates(hostRequest),
    );
  }

  previewBusinessWorkspacePrefill(
    request: PreviewBusinessWorkspacePrefillRequest,
  ): Promise<BusinessWorkspacePrefillPreview> {
    return this.callHost(() =>
      this.host.previewBusinessWorkspacePrefill(request),
    );
  }

  executeContractReviewCommand(
    command: ContractReviewCommandEnvelope,
  ): Promise<ContractReviewCommandResponse> {
    return this.callHost(() => this.host.executeContractReviewCommand(command));
  }

  createContractReview(
    payload: CreateContractReviewPayload,
    options: ScopedCommandOptions = {},
  ): Promise<ContractReviewCommandResponse> {
    const command: ContractReviewCommandEnvelope = {
      ...this.commandBase(
        options.projectId ?? this.businessWorkspaceProjectId(payload.workspaceId),
        options,
      ),
      commandType: "contractReview.create",
      payload,
      expectedRevision: null,
    };
    return this.executeContractReviewCommand(command);
  }

  startContractReview(
    payload: StartContractReviewPayload,
    expectedRevision: number,
    options: ScopedCommandOptions = {},
  ): Promise<ContractReviewCommandResponse> {
    const command: ContractReviewCommandEnvelope = {
      ...this.commandBase(options.projectId ?? null, options),
      commandType: "contractReview.start",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeContractReviewCommand(command);
  }

  cancelContractReview(
    payload: CancelContractReviewPayload,
    expectedRevision: number,
    options: ScopedCommandOptions = {},
  ): Promise<ContractReviewCommandResponse> {
    const command: ContractReviewCommandEnvelope = {
      ...this.commandBase(options.projectId ?? null, options),
      commandType: "contractReview.cancel",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeContractReviewCommand(command);
  }

  decideReviewFinding(
    payload: DecideReviewFindingPayload,
    expectedRevision: number,
    options: ScopedCommandOptions = {},
  ): Promise<ContractReviewCommandResponse> {
    const command: ContractReviewCommandEnvelope = {
      ...this.commandBase(options.projectId ?? null, options),
      commandType: "contractReview.decideFinding",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeContractReviewCommand(command);
  }

  generateReviewReport(
    payload: GenerateReviewReportPayload,
    expectedRevision: number,
    options: ScopedCommandOptions = {},
  ): Promise<ContractReviewCommandResponse> {
    const command: ContractReviewCommandEnvelope = {
      ...this.commandBase(options.projectId ?? null, options),
      commandType: "contractReview.generateReport",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeContractReviewCommand(command);
  }

  retryContractReviewStage(
    payload: RetryContractReviewStagePayload,
    expectedRevision: number,
    options: ScopedCommandOptions = {},
  ): Promise<ContractReviewCommandResponse> {
    const command: ContractReviewCommandEnvelope = {
      ...this.commandBase(options.projectId ?? null, options),
      commandType: "contractReview.retryStage",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeContractReviewCommand(command);
  }

  listContractReviews(
    request: ListContractReviewsInput = {},
  ): Promise<readonly ContractReviewRecord[]> {
    return this.callHost(() =>
      this.host.listContractReviews({
        workspaceId: request.workspaceId ?? null,
        status: request.status ?? null,
        limit:
          request.limit === undefined || request.limit === null
            ? null
            : requirePositiveLimit(request.limit),
      }),
    );
  }

  getContractReview(
    requestOrReviewId: GetContractReviewRequest | string,
  ): Promise<ContractReviewRecord> {
    const request =
      typeof requestOrReviewId === "string"
        ? { reviewId: requestOrReviewId }
        : requestOrReviewId;
    return this.callHost(() => this.host.getContractReview(request));
  }

  listReviewFindings(
    request: ListReviewFindingsInput,
  ): Promise<readonly ReviewFindingRecord[]> {
    return this.callHost(() =>
      this.host.listReviewFindings({
        reviewId: request.reviewId,
        status: request.status ?? null,
      }),
    );
  }

  getEvidenceContext(
    requestOrEvidenceId: GetEvidenceContextRequest | string,
  ): Promise<EvidenceContext> {
    const request =
      typeof requestOrEvidenceId === "string"
        ? { evidenceId: requestOrEvidenceId }
        : requestOrEvidenceId;
    return this.callHost(() => this.host.getEvidenceContext(request));
  }

  replayContractReviewEvents(
    afterSequence: number,
    limit: number,
  ): Promise<readonly ContractReviewDomainEvent[]> {
    return this.callHost(() =>
      this.host.replayContractReviewEvents(
        requireNonNegativeSequence(afterSequence),
        requirePositiveLimit(limit),
      ),
    );
  }

  subscribeContractReviewEvents(
    listener: ContractReviewEventListener,
  ): Promise<Unsubscribe> {
    return this.callHost(() => this.host.subscribeContractReviewEvents(listener));
  }

  getAiCredentialStatus(
    options: CommandOptions = {},
  ): Promise<AiCredentialStatus> {
    const command: AiCredentialCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "aiCredentials.status",
      expectedRevision: null,
    };
    return this.callHost(async () =>
      (await this.host.executeAiCredentialCommand(command)).status
    );
  }

  saveBsaigcApiKey(
    apiKey: string,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<AiCredentialStatus> {
    const normalizedApiKey = apiKey.trim();
    if (!normalizedApiKey) {
      throw new TypeError("apiKey must not be empty");
    }
    const command: AiCredentialCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "aiCredentials.saveBsaigcApiKey",
      payload: { apiKey: normalizedApiKey },
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.callHost(async () =>
      (await this.host.executeAiCredentialCommand(command)).status
    );
  }

  clearBsaigcApiKey(
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<AiCredentialStatus> {
    const command: AiCredentialCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "aiCredentials.clearBsaigcApiKey",
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.callHost(async () =>
      (await this.host.executeAiCredentialCommand(command)).status
    );
  }

  upsertProvider(
    input: UpsertAiProviderPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<AiCredentialStatus> {
    const provider = normalizeAiProviderUpsertInput(input);
    const command: AiCredentialCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "aiCredentials.upsertProvider",
      payload: provider,
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.callHost(async () =>
      (await this.host.executeAiCredentialCommand(command)).status
    );
  }

  removeProvider(
    providerId: string,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<AiCredentialStatus> {
    const command: AiCredentialCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "aiCredentials.removeProvider",
      payload: { providerId: requireProviderId(providerId) },
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.callHost(async () =>
      (await this.host.executeAiCredentialCommand(command)).status
    );
  }

  selectProvider(
    providerId: string,
    model: string,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<AiCredentialStatus> {
    const normalizedModel = requireNonEmptyText(model, "model");
    const command: AiCredentialCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "aiCredentials.selectProvider",
      payload: {
        providerId: requireProviderId(providerId),
        model: normalizedModel,
      },
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.callHost(async () =>
      (await this.host.executeAiCredentialCommand(command)).status
    );
  }

  testProvider(
    providerId: string,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<AiCredentialCommandResponse> {
    const command: AiCredentialCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "aiCredentials.testProvider",
      payload: { providerId: requireProviderId(providerId) },
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.callHost(() => this.host.executeAiCredentialCommand(command));
  }

  discoverProviderModels(
    input: DiscoverAiProviderModelsPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<AiCredentialCommandResponse> {
    const payload = normalizeAiProviderDiscoveryInput(input);
    const command: AiCredentialCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "aiCredentials.discoverModels",
      payload,
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.callHost(() => this.host.executeAiCredentialCommand(command));
  }

  clearProviderApiKey(
    providerId: string,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<AiCredentialStatus> {
    const command: AiCredentialCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "aiCredentials.clearProviderApiKey",
      payload: { providerId: requireProviderId(providerId) },
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.callHost(async () =>
      (await this.host.executeAiCredentialCommand(command)).status
    );
  }

  getDesktopSettingsStatus(
    options: CommandOptions = {},
  ): Promise<DesktopSettingsSnapshot> {
    const command: DesktopSettingsCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "settings.status",
      expectedRevision: null,
    };
    return this.executeDesktopSettings(command).then(({ snapshot }) => snapshot);
  }

  openStorageLocation(
    target: StorageLocationTarget,
    expectedRevision: number | null = null,
    options: CommandOptions = {},
  ): Promise<DesktopSettingsSnapshot> {
    const command: DesktopSettingsCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "settings.openStorageLocation",
      payload: { target: requireStorageLocationTarget(target) },
      expectedRevision: optionalRevision(expectedRevision),
    };
    return this.executeDesktopSettings(command).then(({ snapshot }) => snapshot);
  }

  clearCache(
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<DesktopSettingsCommandResponse> {
    const command: DesktopSettingsCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "settings.clearCache",
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.executeDesktopSettings(command);
  }

  checkForUpdates(
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<DesktopSettingsSnapshot> {
    const command: DesktopSettingsCommandEnvelope = {
      ...this.commandBase(null, options),
      commandType: "settings.checkForUpdates",
      expectedRevision: requireRevision(expectedRevision),
    };
    return this.executeDesktopSettings(command).then(({ snapshot }) => snapshot);
  }

  executeBackupCommand(
    command: BackupCommandEnvelope,
  ): Promise<BackupCommandResponse> {
    return this.callHost(() => this.host.executeBackupCommand(command));
  }

  queueAssetBackup(
    payload: QueueAssetBackupPayload,
    expectedRevision: number | null = null,
    options: ScopedCommandOptions = {},
  ): Promise<BackupCommandResponse> {
    const command: BackupCommandEnvelope = {
      ...this.commandBase(options.projectId ?? null, options),
      commandType: "backup.queue",
      payload,
      expectedRevision: optionalPositiveRevision(expectedRevision),
    };
    return this.executeBackupCommand(command);
  }

  retryAssetBackup(
    payload: RetryAssetBackupPayload,
    expectedRevision: number,
    options: ScopedCommandOptions = {},
  ): Promise<BackupCommandResponse> {
    const command: BackupCommandEnvelope = {
      ...this.commandBase(options.projectId ?? null, options),
      commandType: "backup.retry",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBackupCommand(command);
  }

  cancelAssetBackup(
    payload: CancelAssetBackupPayload,
    expectedRevision: number,
    options: ScopedCommandOptions = {},
  ): Promise<BackupCommandResponse> {
    const command: BackupCommandEnvelope = {
      ...this.commandBase(options.projectId ?? null, options),
      commandType: "backup.cancel",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBackupCommand(command);
  }

  restoreAssetBackup(
    payload: RestoreAssetBackupPayload,
    expectedRevision: number,
    options: ScopedCommandOptions = {},
  ): Promise<BackupCommandResponse> {
    const command: BackupCommandEnvelope = {
      ...this.commandBase(options.projectId ?? null, options),
      commandType: "backup.restore",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeBackupCommand(command);
  }

  listAssetBackups(limit = REPLAY_PAGE_SIZE): Promise<readonly AssetBackupRecord[]> {
    return this.callHost(() => this.host.listAssetBackups(requirePositiveLimit(limit)));
  }

  replayBackupEvents(
    afterSequence: number,
    limit: number,
  ): Promise<readonly BackupDomainEvent[]> {
    return this.callHost(() =>
      this.host.replayBackupEvents(
        requireNonNegativeSequence(afterSequence),
        requirePositiveLimit(limit),
      ),
    );
  }

  subscribeBackupEvents(listener: BackupEventListener): Promise<Unsubscribe> {
    return this.callHost(() => this.host.subscribeBackupEvents(listener));
  }

  createExecutionBrief(
    payload: CreateExecutionBriefPayload,
    options: CommandOptions = {},
  ): Promise<ExecutionBriefCommandResponse> {
    const command: ExecutionBriefCommandEnvelope = {
      ...this.commandBase(payload.projectId, options),
      commandType: "executionBrief.create",
      payload,
      expectedRevision: null,
    };
    return this.executeExecutionBrief(command);
  }

  updateExecutionBrief(
    payload: UpdateExecutionBriefPayload,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<ExecutionBriefCommandResponse> {
    const executionBrief = this.executionBriefProjection
      .snapshot()
      .executionBriefs.find((candidate) => candidate.id === payload.briefId);
    const command: ExecutionBriefCommandEnvelope = {
      ...this.commandBase(executionBrief?.projectId ?? null, options),
      commandType: "executionBrief.update",
      payload,
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeExecutionBrief(command);
  }

  changeExecutionBriefStatus(
    briefId: string,
    status: ExecutionBriefStatus,
    expectedRevision: number,
    options: CommandOptions = {},
  ): Promise<ExecutionBriefCommandResponse> {
    const executionBrief = this.executionBriefProjection
      .snapshot()
      .executionBriefs.find((candidate) => candidate.id === briefId);
    const command: ExecutionBriefCommandEnvelope = {
      ...this.commandBase(executionBrief?.projectId ?? null, options),
      commandType: "executionBrief.changeStatus",
      payload: { briefId, status },
      expectedRevision: requirePositiveRevision(expectedRevision),
    };
    return this.executeExecutionBrief(command);
  }

  refreshExecutionBriefs(): Promise<readonly ExecutionBriefRecord[]> {
    return this.callHost(async () => {
      const executionBriefs = await this.host.listExecutionBriefs();
      this.executionBriefProjection.hydrate(executionBriefs);
      await this.replayExecutionBriefPages(
        this.executionBriefProjection.snapshot().lastSequence,
        this.lifecycleGeneration,
      );
      this.settleExecutionBriefGapIfCaughtUp();
      this.publish();
      return this.executionBriefProjection.snapshot().executionBriefs;
    });
  }

  private async performStart(generation: number): Promise<void> {
    let subscription: Unsubscribe | null = null;
    const pendingSubscriptions: Unsubscribe[] = [];

    try {
      pendingSubscriptions.push(await this.host.subscribeDomainEvents((event) => {
        if (generation !== this.lifecycleGeneration) {
          return;
        }
        if (this.bufferingEvents) {
          this.bufferedEvents.push(event);
          return;
        }
        if (this.projection.applyEvent(event)) {
          this.publish();
        }
      }));

      pendingSubscriptions.push(await this.host.subscribeTaskEvents((event) => {
        if (generation !== this.lifecycleGeneration) {
          return;
        }
        if (this.bufferingEvents) {
          this.bufferedTaskEvents.push(event);
          return;
        }
        if (this.taskProjection.applyEvent(event)) {
          this.publish();
          if (
            event.eventType === "task.succeeded" &&
            event.task.kind.startsWith("media.")
          ) {
            this.refreshAssetsAfterNativeTask(generation);
          }
        }
      }));

      pendingSubscriptions.push(await this.host.subscribeAssetEvents((event) => {
        if (generation !== this.lifecycleGeneration) {
          return;
        }
        if (this.bufferingEvents) {
          this.bufferedAssetEvents.push(event);
          return;
        }
        if (this.assetProjection.applyEvent(event)) {
          this.publish();
        }
      }));

      pendingSubscriptions.push(await this.host.subscribeBrainEvents((event) => {
        if (generation !== this.lifecycleGeneration) return;
        if (this.bufferingEvents) {
          this.bufferedBrainEvents.push(event);
          return;
        }
        if (this.brainProjection.applyEvent(event)) {
          this.publish();
          this.refreshBrainRecordsAfterEvent(event, generation);
        }
      }));

      pendingSubscriptions.push(await this.host.subscribeCaseEvents((event) => {
        if (generation !== this.lifecycleGeneration) return;
        if (this.bufferingEvents) {
          this.bufferedCaseEvents.push(event);
          return;
        }
        const cursorBeforeEvent = this.caseProjection.snapshot().lastSequence;
        if (this.caseProjection.applyEvent(event)) this.publish();
        if (event.sequence > cursorBeforeEvent + 1) {
          this.recoverCaseEventGap(event.sequence, generation);
        }
      }));

      pendingSubscriptions.push(
        await this.host.subscribeExecutionBriefEvents((event) => {
          if (generation !== this.lifecycleGeneration) return;
          if (this.bufferingEvents) {
            this.bufferedExecutionBriefEvents.push(event);
            return;
          }
          const cursorBeforeEvent =
            this.executionBriefProjection.snapshot().lastSequence;
          if (this.executionBriefProjection.applyEvent(event)) this.publish();
          if (event.sequence > cursorBeforeEvent + 1) {
            this.recoverExecutionBriefEventGap(event.sequence, generation);
          }
        }),
      );

      pendingSubscriptions.push(
        await this.host.subscribeRequirementBriefEvents((event) => {
          if (generation !== this.lifecycleGeneration) return;
          if (this.bufferingEvents) {
            this.bufferedRequirementBriefEvents.push(event);
            return;
          }
          const cursorBeforeEvent =
            this.requirementBriefProjection.snapshot().lastSequence;
          const hadGapTarget = this.requirementBriefGapTargetSequence > 0;
          this.requirementBriefProjection.applyEvent(event);
          if (
            this.requirementBriefProjection.snapshot().lastSequence >
            cursorBeforeEvent
          ) {
            this.publish();
          }
          if (
            hadGapTarget &&
            this.settleRequirementBriefGapIfCaughtUp()
          ) {
            this.publish();
          }
          if (event.sequence > cursorBeforeEvent + 1) {
            this.recoverRequirementBriefEventGap(event.sequence, generation);
          }
        }),
      );

      pendingSubscriptions.push(
        await this.host.subscribeBusinessWorkspaceEvents((event) => {
          if (generation !== this.lifecycleGeneration) return;
          if (this.bufferingEvents) {
            this.bufferedBusinessWorkspaceEvents.push(event);
            return;
          }
          const cursorBeforeEvent =
            this.businessWorkspaceProjection.snapshot().lastSequence;
          const hadGapTarget = this.businessWorkspaceGapTargetSequence > 0;
          this.businessWorkspaceProjection.applyEvent(event);
          if (
            this.businessWorkspaceProjection.snapshot().lastSequence >
            cursorBeforeEvent
          ) {
            this.publish();
          }
          if (
            hadGapTarget &&
            this.settleBusinessWorkspaceGapIfCaughtUp()
          ) {
            this.publish();
          }
          if (event.sequence > cursorBeforeEvent + 1) {
            this.recoverBusinessWorkspaceEventGap(event.sequence, generation);
          }
        }),
      );

      subscription = combineUnsubscribes(pendingSubscriptions);

      if (!this.activateSubscription(generation, subscription)) {
        return;
      }

      const [
        projects,
        tasks,
        assets,
        brainThreads,
        cases,
        executionBriefs,
        requirementBriefs,
        businessWorkspaces,
      ] = await Promise.all([
        this.host.listProjects(),
        this.host.listTasks(),
        this.host.listAssets(),
        this.host.listLocalBrainThreads(null),
        this.host.listCases(),
        this.host.listExecutionBriefs(),
        this.host.listRequirementBriefs(),
        this.host.listBusinessWorkspaces(),
      ]);
      if (!this.isGenerationActive(generation)) {
        return;
      }
      this.projection.hydrateProjects(projects);
      this.taskProjection.hydrate(tasks);
      this.assetProjection.hydrate(assets);
      this.brainProjection.replaceThreads(brainThreads);
      this.caseProjection.hydrate(cases);
      this.executionBriefProjection.hydrate(executionBriefs);
      this.requirementBriefProjection.hydrate(requirementBriefs);
      this.businessWorkspaceProjection.hydrate(businessWorkspaces);

      await Promise.all([
        this.replayProjectPages(0, generation),
        this.replayTaskPages(0, generation),
        this.replayAssetPages(0, generation),
        this.replayCasePages(0, generation),
        this.replayExecutionBriefPages(0, generation),
        this.replayRequirementBriefPages(0, generation),
        this.replayBusinessWorkspacePages(0, generation),
      ]);
      if (!this.isGenerationActive(generation)) {
        return;
      }

      this.bufferedEvents.sort(compareBufferedEvents);
      for (const event of this.bufferedEvents) {
        this.projection.applyEvent(event);
      }
      this.bufferedTaskEvents.sort(compareBufferedEvents);
      for (const event of this.bufferedTaskEvents) {
        this.taskProjection.applyEvent(event);
      }
      this.bufferedAssetEvents.sort(compareBufferedEvents);
      for (const event of this.bufferedAssetEvents) {
        this.assetProjection.applyEvent(event);
      }
      this.bufferedBrainEvents.sort(compareBufferedEvents);
      for (const event of this.bufferedBrainEvents) {
        this.brainProjection.applyEvent(event);
      }
      this.bufferedCaseEvents.sort(compareBufferedEvents);
      const bufferedCaseMaxSequence =
        this.bufferedCaseEvents[this.bufferedCaseEvents.length - 1]?.sequence ??
        0;
      for (const event of this.bufferedCaseEvents) {
        this.caseProjection.applyEvent(event);
      }
      this.bufferedExecutionBriefEvents.sort(compareBufferedEvents);
      const bufferedExecutionBriefMaxSequence =
        this.bufferedExecutionBriefEvents[
          this.bufferedExecutionBriefEvents.length - 1
        ]?.sequence ?? 0;
      for (const event of this.bufferedExecutionBriefEvents) {
        this.executionBriefProjection.applyEvent(event);
      }
      this.bufferedRequirementBriefEvents.sort(compareBufferedEvents);
      const bufferedRequirementBriefMaxSequence =
        this.bufferedRequirementBriefEvents[
          this.bufferedRequirementBriefEvents.length - 1
        ]?.sequence ?? 0;
      for (const event of this.bufferedRequirementBriefEvents) {
        this.requirementBriefProjection.applyEvent(event);
      }
      this.bufferedBusinessWorkspaceEvents.sort(compareBufferedEvents);
      const bufferedBusinessWorkspaceMaxSequence =
        this.bufferedBusinessWorkspaceEvents[
          this.bufferedBusinessWorkspaceEvents.length - 1
        ]?.sequence ?? 0;
      for (const event of this.bufferedBusinessWorkspaceEvents) {
        this.businessWorkspaceProjection.applyEvent(event);
      }
      this.bufferedEvents = [];
      this.bufferedTaskEvents = [];
      this.bufferedAssetEvents = [];
      this.bufferedBrainEvents = [];
      this.bufferedCaseEvents = [];
      this.bufferedExecutionBriefEvents = [];
      this.bufferedRequirementBriefEvents = [];
      this.bufferedBusinessWorkspaceEvents = [];
      this.bufferingEvents = false;
      this.started = true;
      this.synchronizing = false;
      this.error = null;
      this.publish();
      if (
        bufferedCaseMaxSequence >
        this.caseProjection.snapshot().lastSequence
      ) {
        this.recoverCaseEventGap(bufferedCaseMaxSequence, generation);
      }
      if (
        bufferedExecutionBriefMaxSequence >
        this.executionBriefProjection.snapshot().lastSequence
      ) {
        this.recoverExecutionBriefEventGap(
          bufferedExecutionBriefMaxSequence,
          generation,
        );
      }
      if (
        bufferedRequirementBriefMaxSequence >
        this.requirementBriefProjection.snapshot().lastSequence
      ) {
        this.recoverRequirementBriefEventGap(
          bufferedRequirementBriefMaxSequence,
          generation,
        );
      }
      if (
        bufferedBusinessWorkspaceMaxSequence >
        this.businessWorkspaceProjection.snapshot().lastSequence
      ) {
        this.recoverBusinessWorkspaceEventGap(
          bufferedBusinessWorkspaceMaxSequence,
          generation,
        );
      }
    } catch (error) {
      if (!this.isGenerationActive(generation)) {
        (subscription ?? combineUnsubscribes(pendingSubscriptions))();
        return;
      }

      (subscription ?? combineUnsubscribes(pendingSubscriptions))();
      if (this.unsubscribeHost === subscription) {
        this.unsubscribeHost = null;
      }
      this.bufferedEvents = [];
      this.bufferedTaskEvents = [];
      this.bufferedAssetEvents = [];
      this.bufferedBrainEvents = [];
      this.bufferedCaseEvents = [];
      this.bufferedExecutionBriefEvents = [];
      this.bufferedRequirementBriefEvents = [];
      this.bufferedBusinessWorkspaceEvents = [];
      this.bufferingEvents = false;
      this.started = false;
      this.synchronizing = false;
      this.error = normalizeHostError(error);
      this.publish();
      throw this.error;
    }
  }

  private async replayProjectPages(
    initialSequence: number,
    generation: number,
  ): Promise<void> {
    let afterSequence = initialSequence;
    while (true) {
      const page = await this.host.replayEvents(afterSequence, REPLAY_PAGE_SIZE);
      if (!this.isGenerationActive(generation)) return;
      const pageLastSequence = applyPage(
        page,
        afterSequence,
        (event) => this.projection.applyEvent(event),
      );
      if (page.length < REPLAY_PAGE_SIZE) return;
      ensureSequenceAdvanced(afterSequence, pageLastSequence, "project");
      afterSequence = pageLastSequence;
    }
  }

  private async replayTaskPages(
    initialSequence: number,
    generation: number,
  ): Promise<void> {
    let afterSequence = initialSequence;
    while (true) {
      const page = await this.host.replayTaskEvents(
        afterSequence,
        REPLAY_PAGE_SIZE,
      );
      if (!this.isGenerationActive(generation)) return;
      const pageLastSequence = applyPage(
        page,
        afterSequence,
        (event) => this.taskProjection.applyEvent(event),
      );
      if (page.length < REPLAY_PAGE_SIZE) return;
      ensureSequenceAdvanced(afterSequence, pageLastSequence, "task");
      afterSequence = pageLastSequence;
    }
  }

  private async replayAssetPages(
    initialSequence: number,
    generation: number,
  ): Promise<void> {
    let afterSequence = initialSequence;
    while (true) {
      const page = await this.host.replayAssetEvents(
        afterSequence,
        REPLAY_PAGE_SIZE,
      );
      if (!this.isGenerationActive(generation)) return;
      const pageLastSequence = applyPage(
        page,
        afterSequence,
        (event) => this.assetProjection.applyEvent(event),
      );
      if (page.length < REPLAY_PAGE_SIZE) return;
      ensureSequenceAdvanced(afterSequence, pageLastSequence, "asset");
      afterSequence = pageLastSequence;
    }
  }

  private async replayCasePages(
    initialSequence: number,
    generation: number,
  ): Promise<void> {
    let afterSequence = initialSequence;
    while (true) {
      const page = await this.host.replayCaseEvents(
        afterSequence,
        REPLAY_PAGE_SIZE,
      );
      if (!this.isGenerationActive(generation)) return;
      const pageLastSequence = applyPage(
        page,
        afterSequence,
        (event) => this.caseProjection.applyEvent(event),
      );
      const contiguousSequence = this.caseProjection.snapshot().lastSequence;
      if (pageLastSequence > contiguousSequence) {
        throw hostError(
          "CASE_EVENT_SEQUENCE_GAP",
          `Case replay stopped at ${contiguousSequence} before event ${pageLastSequence}`,
          true,
        );
      }
      if (page.length < REPLAY_PAGE_SIZE) return;
      ensureSequenceAdvanced(afterSequence, pageLastSequence, "case");
      afterSequence = pageLastSequence;
    }
  }

  private async replayExecutionBriefPages(
    initialSequence: number,
    generation: number,
  ): Promise<void> {
    let afterSequence = initialSequence;
    while (true) {
      const page = await this.host.replayExecutionBriefEvents(
        afterSequence,
        REPLAY_PAGE_SIZE,
      );
      if (!this.isGenerationActive(generation)) return;
      const pageLastSequence = applyPage(
        page,
        afterSequence,
        (event) => this.executionBriefProjection.applyEvent(event),
      );
      const contiguousSequence =
        this.executionBriefProjection.snapshot().lastSequence;
      if (pageLastSequence > contiguousSequence) {
        throw hostError(
          "EXECUTION_BRIEF_EVENT_SEQUENCE_GAP",
          `Execution brief replay stopped at ${contiguousSequence} before event ${pageLastSequence}`,
          true,
        );
      }
      if (page.length < REPLAY_PAGE_SIZE) return;
      ensureSequenceAdvanced(
        afterSequence,
        pageLastSequence,
        "execution brief",
      );
      afterSequence = pageLastSequence;
    }
  }

  private async replayRequirementBriefPages(
    initialSequence: number,
    generation: number,
  ): Promise<void> {
    let afterSequence = initialSequence;
    while (true) {
      const page = await this.host.replayRequirementBriefEvents(
        afterSequence,
        REPLAY_PAGE_SIZE,
      );
      if (!this.isGenerationActive(generation)) return;
      const pageLastSequence = applyPage(
        page,
        afterSequence,
        (event) => this.requirementBriefProjection.applyEvent(event),
      );
      const contiguousSequence =
        this.requirementBriefProjection.snapshot().lastSequence;
      if (pageLastSequence > contiguousSequence) {
        throw hostError(
          "REQUIREMENT_BRIEF_EVENT_SEQUENCE_GAP",
          `Requirement brief replay stopped at ${contiguousSequence} before event ${pageLastSequence}`,
          true,
        );
      }
      if (page.length < REPLAY_PAGE_SIZE) return;
      ensureSequenceAdvanced(
        afterSequence,
        pageLastSequence,
        "requirement brief",
      );
      afterSequence = pageLastSequence;
    }
  }

  private async replayBusinessWorkspacePages(
    initialSequence: number,
    generation: number,
  ): Promise<void> {
    let afterSequence = initialSequence;
    while (true) {
      const page = await this.host.replayBusinessWorkspaceEvents(
        afterSequence,
        REPLAY_PAGE_SIZE,
      );
      if (!this.isGenerationActive(generation)) return;
      const pageLastSequence = applyPage(
        page,
        afterSequence,
        (event) => this.businessWorkspaceProjection.applyEvent(event),
      );
      const contiguousSequence =
        this.businessWorkspaceProjection.snapshot().lastSequence;
      if (pageLastSequence > contiguousSequence) {
        throw hostError(
          "BUSINESS_WORKSPACE_EVENT_SEQUENCE_GAP",
          `Business workspace replay stopped at ${contiguousSequence} before event ${pageLastSequence}`,
          true,
        );
      }
      if (page.length < REPLAY_PAGE_SIZE) return;
      ensureSequenceAdvanced(
        afterSequence,
        pageLastSequence,
        "business workspace",
      );
      afterSequence = pageLastSequence;
    }
  }

  private recoverCaseEventGap(
    targetSequence: number,
    generation: number,
  ): void {
    this.caseGapTargetSequence = Math.max(
      this.caseGapTargetSequence,
      targetSequence,
    );
    if (this.caseGapRecovery || !this.started) return;

    const recovery = (async () => {
      try {
        while (this.isGenerationActive(generation)) {
          const target = this.caseGapTargetSequence;
          await this.replayCasePages(
            this.caseProjection.snapshot().lastSequence,
            generation,
          );
          if (!this.isGenerationActive(generation)) return;
          const cursor = this.caseProjection.snapshot().lastSequence;
          if (cursor < target) {
            throw hostError(
              "CASE_EVENT_SEQUENCE_GAP",
              `Case event ${target} arrived before contiguous event ${cursor + 1}`,
              true,
            );
          }
          if (cursor >= this.caseGapTargetSequence) break;
        }
        if (this.isGenerationActive(generation)) {
          this.clearStreamGapError("CASE_EVENT_SEQUENCE_GAP");
          this.publish();
        }
      } catch (error) {
        if (this.isGenerationActive(generation)) {
          this.error = normalizeHostError(error);
          this.publish();
        }
      }
    })();
    this.caseGapRecovery = recovery;
    void recovery.finally(() => {
      if (this.caseGapRecovery === recovery) {
        this.caseGapRecovery = null;
        this.caseGapTargetSequence = 0;
      }
    });
  }

  private recoverExecutionBriefEventGap(
    targetSequence: number,
    generation: number,
  ): void {
    this.executionBriefGapTargetSequence = Math.max(
      this.executionBriefGapTargetSequence,
      targetSequence,
    );
    if (this.executionBriefGapRecovery || !this.started) return;

    const recovery = (async () => {
      let lastError: unknown = null;
      for (
        let attempt = 0;
        attempt < EXECUTION_BRIEF_GAP_RECOVERY_ATTEMPTS;
        attempt += 1
      ) {
        if (!this.isGenerationActive(generation)) return;
        if (this.settleExecutionBriefGapIfCaughtUp()) {
          this.clearStreamGapError("EXECUTION_BRIEF_EVENT_SEQUENCE_GAP");
          this.publish();
          return;
        }

        const target = this.executionBriefGapTargetSequence;
        try {
          await this.replayExecutionBriefPages(
            this.executionBriefProjection.snapshot().lastSequence,
            generation,
          );
          if (!this.isGenerationActive(generation)) return;
          const cursor =
            this.executionBriefProjection.snapshot().lastSequence;
          if (cursor < target) {
            throw hostError(
              "EXECUTION_BRIEF_EVENT_SEQUENCE_GAP",
              `Execution brief event ${target} arrived before contiguous event ${cursor + 1}`,
              true,
            );
          }
          if (this.settleExecutionBriefGapIfCaughtUp()) {
            this.clearStreamGapError("EXECUTION_BRIEF_EVENT_SEQUENCE_GAP");
            this.publish();
            return;
          }
          lastError = hostError(
            "EXECUTION_BRIEF_EVENT_SEQUENCE_GAP",
            `Execution brief event ${this.executionBriefGapTargetSequence} arrived before contiguous event ${cursor + 1}`,
            true,
          );
        } catch (error) {
          lastError = error;
          if (
            this.isGenerationActive(generation) &&
            this.settleExecutionBriefGapIfCaughtUp()
          ) {
            this.clearStreamGapError("EXECUTION_BRIEF_EVENT_SEQUENCE_GAP");
            this.publish();
            return;
          }
        }
      }

      if (this.isGenerationActive(generation)) {
        if (this.settleExecutionBriefGapIfCaughtUp()) {
          this.clearStreamGapError("EXECUTION_BRIEF_EVENT_SEQUENCE_GAP");
        } else {
          const cursor =
            this.executionBriefProjection.snapshot().lastSequence;
          this.error = normalizeHostError(
            lastError ??
              hostError(
                "EXECUTION_BRIEF_EVENT_SEQUENCE_GAP",
                `Execution brief event ${this.executionBriefGapTargetSequence} arrived before contiguous event ${cursor + 1}`,
                true,
              ),
          );
        }
        this.publish();
      }
    })();
    this.executionBriefGapRecovery = recovery;
    void recovery.finally(() => {
      if (this.executionBriefGapRecovery === recovery) {
        this.executionBriefGapRecovery = null;
      }
    });
  }

  private settleExecutionBriefGapIfCaughtUp(): boolean {
    if (
      this.executionBriefProjection.snapshot().lastSequence <
      this.executionBriefGapTargetSequence
    ) {
      return false;
    }
    this.executionBriefGapTargetSequence = 0;
    return true;
  }

  private recoverRequirementBriefEventGap(
    targetSequence: number,
    generation: number,
  ): void {
    this.requirementBriefGapTargetSequence = Math.max(
      this.requirementBriefGapTargetSequence,
      targetSequence,
    );
    if (this.requirementBriefGapRecovery || !this.started) return;

    const recovery = (async () => {
      let lastError: unknown = null;
      for (
        let attempt = 0;
        attempt < REQUIREMENT_BRIEF_GAP_RECOVERY_ATTEMPTS;
        attempt += 1
      ) {
        if (!this.isGenerationActive(generation)) return;
        if (this.settleRequirementBriefGapIfCaughtUp()) {
          this.publish();
          return;
        }

        const target = this.requirementBriefGapTargetSequence;
        try {
          await this.replayRequirementBriefPages(
            this.requirementBriefProjection.snapshot().lastSequence,
            generation,
          );
          if (!this.isGenerationActive(generation)) return;
          const cursor =
            this.requirementBriefProjection.snapshot().lastSequence;
          if (cursor < target) {
            throw hostError(
              "REQUIREMENT_BRIEF_EVENT_SEQUENCE_GAP",
              `Requirement brief event ${target} arrived before contiguous event ${cursor + 1}`,
              true,
            );
          }
          if (this.settleRequirementBriefGapIfCaughtUp()) {
            this.publish();
            return;
          }
          lastError = hostError(
            "REQUIREMENT_BRIEF_EVENT_SEQUENCE_GAP",
            `Requirement brief event ${this.requirementBriefGapTargetSequence} arrived before contiguous event ${cursor + 1}`,
            true,
          );
        } catch (error) {
          lastError = error;
          if (
            this.isGenerationActive(generation) &&
            this.settleRequirementBriefGapIfCaughtUp()
          ) {
            this.publish();
            return;
          }
        }
      }

      if (this.isGenerationActive(generation)) {
        if (!this.settleRequirementBriefGapIfCaughtUp()) {
          const cursor =
            this.requirementBriefProjection.snapshot().lastSequence;
          this.requirementBriefGapError = normalizeHostError(
            lastError ??
              hostError(
                "REQUIREMENT_BRIEF_EVENT_SEQUENCE_GAP",
                `Requirement brief event ${this.requirementBriefGapTargetSequence} arrived before contiguous event ${cursor + 1}`,
                true,
              ),
          );
        }
        this.publish();
      }
    })();
    this.requirementBriefGapRecovery = recovery;
    void recovery.finally(() => {
      if (this.requirementBriefGapRecovery === recovery) {
        this.requirementBriefGapRecovery = null;
      }
    });
  }

  private settleRequirementBriefGapIfCaughtUp(): boolean {
    if (
      this.requirementBriefProjection.snapshot().lastSequence <
      this.requirementBriefGapTargetSequence
    ) {
      return false;
    }
    this.requirementBriefGapTargetSequence = 0;
    this.requirementBriefGapError = null;
    return true;
  }

  private recoverBusinessWorkspaceEventGap(
    targetSequence: number,
    generation: number,
  ): void {
    this.businessWorkspaceGapTargetSequence = Math.max(
      this.businessWorkspaceGapTargetSequence,
      targetSequence,
    );
    if (this.businessWorkspaceGapRecovery || !this.started) return;

    const recovery = (async () => {
      let lastError: unknown = null;
      for (
        let attempt = 0;
        attempt < BUSINESS_WORKSPACE_GAP_RECOVERY_ATTEMPTS;
        attempt += 1
      ) {
        if (!this.isGenerationActive(generation)) return;
        if (this.settleBusinessWorkspaceGapIfCaughtUp()) {
          this.publish();
          return;
        }

        const target = this.businessWorkspaceGapTargetSequence;
        try {
          await this.replayBusinessWorkspacePages(
            this.businessWorkspaceProjection.snapshot().lastSequence,
            generation,
          );
          if (!this.isGenerationActive(generation)) return;
          const cursor =
            this.businessWorkspaceProjection.snapshot().lastSequence;
          if (cursor < target) {
            throw hostError(
              "BUSINESS_WORKSPACE_EVENT_SEQUENCE_GAP",
              `Business workspace event ${target} arrived before contiguous event ${cursor + 1}`,
              true,
            );
          }
          if (this.settleBusinessWorkspaceGapIfCaughtUp()) {
            this.publish();
            return;
          }
          lastError = hostError(
            "BUSINESS_WORKSPACE_EVENT_SEQUENCE_GAP",
            `Business workspace event ${this.businessWorkspaceGapTargetSequence} arrived before contiguous event ${cursor + 1}`,
            true,
          );
        } catch (error) {
          lastError = error;
          if (
            this.isGenerationActive(generation) &&
            this.settleBusinessWorkspaceGapIfCaughtUp()
          ) {
            this.publish();
            return;
          }
        }
      }

      if (this.isGenerationActive(generation)) {
        if (!this.settleBusinessWorkspaceGapIfCaughtUp()) {
          const cursor =
            this.businessWorkspaceProjection.snapshot().lastSequence;
          this.businessWorkspaceGapError = normalizeHostError(
            lastError ??
              hostError(
                "BUSINESS_WORKSPACE_EVENT_SEQUENCE_GAP",
                `Business workspace event ${this.businessWorkspaceGapTargetSequence} arrived before contiguous event ${cursor + 1}`,
                true,
              ),
          );
        }
        this.publish();
      }
    })();
    this.businessWorkspaceGapRecovery = recovery;
    void recovery.finally(() => {
      if (this.businessWorkspaceGapRecovery === recovery) {
        this.businessWorkspaceGapRecovery = null;
      }
    });
  }

  private settleBusinessWorkspaceGapIfCaughtUp(): boolean {
    if (
      this.businessWorkspaceProjection.snapshot().lastSequence <
      this.businessWorkspaceGapTargetSequence
    ) {
      return false;
    }
    this.businessWorkspaceGapTargetSequence = 0;
    this.businessWorkspaceGapError = null;
    return true;
  }

  private refreshBrainRecordsAfterEvent(
    event: BrainStreamEvent,
    generation: number,
  ): void {
    const refreshable = new Set([
      "brain.threadStarted",
      "brain.threadStatusChanged",
      "brain.turnStarted",
      "brain.turnCompleted",
      "brain.approvalRequired",
      "brain.error",
    ]);
    if (!refreshable.has(event.eventType)) return;

    void (async () => {
      const [threads, turns] = await Promise.all([
        this.host.listLocalBrainThreads(null),
        event.threadId
          ? this.host.listLocalBrainTurns(event.threadId)
          : Promise.resolve<BrainTurnRecord[]>([]),
      ]);
      if (!this.isGenerationActive(generation)) return;
      this.brainProjection.replaceThreads(threads);
      if (event.threadId) this.brainProjection.replaceTurns(event.threadId, turns);
      this.publish();
    })().catch(() => undefined);
  }

  private refreshAssetsAfterNativeTask(generation: number): void {
    void this.host
      .listAssets()
      .then((assets) => {
        if (!this.isGenerationActive(generation)) return;
        this.assetProjection.hydrate(assets);
        this.publish();
      })
      .catch(() => undefined);
  }

  private activateSubscription(
    generation: number,
    subscription: Unsubscribe,
  ): boolean {
    if (!this.isGenerationActive(generation)) {
      subscription();
      return false;
    }
    this.unsubscribeHost?.();
    this.unsubscribeHost = subscription;
    return true;
  }

  private isGenerationActive(generation: number): boolean {
    return generation === this.lifecycleGeneration;
  }

  private businessWorkspaceProjectId(workspaceId: string): string | null {
    return (
      this.businessWorkspaceProjection
        .snapshot()
        .businessWorkspaces.find((candidate) => candidate.id === workspaceId)
        ?.projectId ?? null
    );
  }

  private commandBase(
    projectId: string | null,
    options: CommandOptions,
  ): Omit<CommandEnvelope, "commandType" | "payload" | "expectedRevision"> {
    const traceId = options.traceId ?? this.uuid();
    const context: OperationContext = {
      actorId: options.actorId ?? this.actorId,
      accountId: options.accountId ?? this.accountId,
      projectId,
      windowId: options.windowId ?? this.windowId,
      traceId,
    };

    return {
      commandId: options.commandId ?? this.uuid(),
      protocolVersion: this.protocolVersion,
      context,
      idempotencyKey: options.idempotencyKey ?? this.uuid(),
      deadlineAt:
        options.deadlineAt !== undefined
          ? options.deadlineAt
          : this.now() + (options.deadlineMs ?? this.commandDeadlineMs),
    };
  }

  private async execute(envelope: CommandEnvelope): Promise<CommandResponse> {
    return this.callHost(async () => {
      const response = await this.host.executeCommand(envelope);
      this.projection.hydrateProjects([response.project]);
      this.publish();
      return response;
    });
  }

  private async executeTask(
    envelope: TaskCommandEnvelope,
  ): Promise<TaskCommandResponse> {
    return this.callHost(async () => {
      const response = await this.host.executeTaskCommand(envelope);
      this.taskProjection.hydrate([response.task]);
      this.publish();
      return response;
    });
  }

  private async executeAsset(
    envelope: AssetCommandEnvelope,
  ): Promise<AssetCommandResponse> {
    return this.callHost(async () => {
      const response = await this.host.executeAssetCommand(envelope);
      this.assetProjection.hydrate([response.asset]);
      this.publish();
      return response;
    });
  }

  private async executeDesktopSettings(
    envelope: DesktopSettingsCommandEnvelope,
  ): Promise<DesktopSettingsCommandResponse> {
    return this.callHost(async () => {
      const response = await this.host.executeDesktopSettingsCommand(envelope);
      assertSafeDesktopSettingsSnapshot(response.snapshot);
      return response;
    });
  }

  private async executeCase(
    command: CaseCommandEnvelope,
  ): Promise<CaseCommandResponse> {
    return this.callHost(async () => {
      const response = await this.host.executeCaseCommand(command);
      this.caseProjection.upsert(response.caseRecord);
      if (
        response.receipt.lastEventSequence >
        this.caseProjection.snapshot().lastSequence
      ) {
        this.recoverCaseEventGap(
          response.receipt.lastEventSequence,
          this.lifecycleGeneration,
        );
      }
      this.publish();
      return response;
    });
  }

  private async executeExecutionBrief(
    command: ExecutionBriefCommandEnvelope,
  ): Promise<ExecutionBriefCommandResponse> {
    return this.callHost(async () => {
      const response = await this.host.executeExecutionBriefCommand(command);
      this.executionBriefProjection.upsert(response.executionBrief);
      if (
        response.receipt.lastEventSequence >
        this.executionBriefProjection.snapshot().lastSequence
      ) {
        this.recoverExecutionBriefEventGap(
          response.receipt.lastEventSequence,
          this.lifecycleGeneration,
        );
      }
      this.publish();
      return response;
    });
  }

  private async executeRequirementBrief(
    command: RequirementBriefCommandEnvelope,
  ): Promise<RequirementBriefCommandResponse> {
    return this.callHost(async () => {
      const response = await this.host.executeRequirementBriefCommand(command);
      this.requirementBriefProjection.upsert(response.requirementBrief);
      if (
        response.receipt.lastEventSequence >
        this.requirementBriefProjection.snapshot().lastSequence
      ) {
        this.recoverRequirementBriefEventGap(
          response.receipt.lastEventSequence,
          this.lifecycleGeneration,
        );
      }
      this.publish();
      return response;
    });
  }

  private async executeBusinessWorkspace(
    command: BusinessWorkspaceCommandEnvelope,
  ): Promise<BusinessWorkspaceCommandResponse> {
    return this.callHost(async () => {
      const response = await this.host.executeBusinessWorkspaceCommand({
        ...command,
        protocolVersion: this.businessWorkspaceProtocolVersion,
      });
      this.businessWorkspaceProjection.upsert(response.businessWorkspace);
      if (
        response.receipt.lastEventSequence >
        this.businessWorkspaceProjection.snapshot().lastSequence
      ) {
        this.recoverBusinessWorkspaceEventGap(
          response.receipt.lastEventSequence,
          this.lifecycleGeneration,
        );
      }
      this.publish();
      return response;
    });
  }

  private clearStartPromise(operation: Promise<void>): void {
    if (this.startPromise === operation) {
      this.startPromise = null;
    }
  }

  private async callHost<T>(operation: () => Promise<T>): Promise<T> {
    try {
      const result = await operation();
      if (this.error) {
        this.error = null;
        this.publish();
      }
      return result;
    } catch (error) {
      this.error = normalizeHostError(error);
      this.publish();
      throw this.error;
    }
  }

  private publish(): void {
    this.currentSnapshot = this.buildSnapshot();
    for (const listener of [...this.listeners]) {
      listener();
    }
  }

  private buildSnapshot(): BsaigcClientSnapshot {
    const projection = this.projection.snapshot();
    const taskProjection = this.taskProjection.snapshot();
    const assetProjection = this.assetProjection.snapshot();
    const brainProjection = this.brainProjection.snapshot();
    const caseProjection = this.caseProjection.snapshot();
    const executionBriefProjection = this.executionBriefProjection.snapshot();
    const requirementBriefProjection =
      this.requirementBriefProjection.snapshot();
    const businessWorkspaceProjection =
      this.businessWorkspaceProjection.snapshot();
    return {
      projects: projection.projects,
      events: projection.events,
      lastSequence: projection.lastSequence,
      tasks: taskProjection.tasks,
      taskEvents: taskProjection.events,
      taskLastSequence: taskProjection.lastSequence,
      assets: assetProjection.assets,
      assetEvents: assetProjection.events,
      assetLastSequence: assetProjection.lastSequence,
      brainThreads: brainProjection.threads,
      brainTurns: brainProjection.turns,
      brainStreamingByTurn: brainProjection.streamingByTurn,
      lastBrainEvent: brainProjection.lastEvent,
      cases: caseProjection.cases,
      caseEvents: caseProjection.events,
      caseLastSequence: caseProjection.lastSequence,
      executionBriefs: executionBriefProjection.executionBriefs,
      executionBriefEvents: executionBriefProjection.events,
      executionBriefLastSequence: executionBriefProjection.lastSequence,
      requirementBriefs: requirementBriefProjection.requirementBriefs,
      requirementBriefEvents: requirementBriefProjection.events,
      requirementBriefLastSequence: requirementBriefProjection.lastSequence,
      businessWorkspaces: businessWorkspaceProjection.businessWorkspaces,
      businessWorkspaceEvents: businessWorkspaceProjection.events,
      businessWorkspaceLastSequence: businessWorkspaceProjection.lastSequence,
      started: this.started,
      synchronizing: this.synchronizing,
      // The freshest command failure wins; latched replay-gap errors are the
      // fallback so they can never mask a newer, actionable host error.
      error:
        this.error ??
        this.requirementBriefGapError ??
        this.businessWorkspaceGapError,
    };
  }
}

export function normalizeHostError(error: unknown): HostError {
  if (typeof error === "string") {
    try {
      return normalizeHostError(JSON.parse(error) as unknown);
    } catch {
      return hostError("HOST_ERROR", error, false);
    }
  }

  if (isRecord(error)) {
    const code = typeof error.code === "string" ? error.code : "HOST_ERROR";
    const message =
      typeof error.message === "string" ? error.message : "Host operation failed";
    const retryable =
      typeof error.retryable === "boolean" ? error.retryable : false;
    return hostError(code, message, retryable);
  }

  return hostError("HOST_ERROR", "Host operation failed", false);
}

function createUuid(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  throw new Error("Secure UUID generation is unavailable");
}

function normalizeAiProviderUpsertInput(
  input: UpsertAiProviderPayload,
): UpsertAiProviderPayload {
  const providerId = input.providerId
    ? requireProviderId(input.providerId)
    : null;
  const name = requireNonEmptyText(input.name, "name");
  const kind = requireNonEmptyText(input.kind, "kind");
  if (kind !== "openAiCompatible") {
    throw new TypeError("kind must be openAiCompatible");
  }
  const baseUrl = normalizeProviderBaseUrl(input.baseUrl);
  const apiKey = input.apiKey?.trim() || null;
  const models = [...new Set(input.models.map((model) => model.trim()).filter(Boolean))];
  if (models.length === 0) {
    throw new TypeError("models must contain at least one model");
  }
  const defaultModel = requireNonEmptyText(input.defaultModel, "defaultModel");
  if (!models.includes(defaultModel)) {
    throw new TypeError("defaultModel must exist in models");
  }
  return {
    providerId,
    name,
    kind,
    baseUrl,
    apiKey,
    models,
    defaultModel,
    setDefault: input.setDefault,
    enabled: input.enabled,
  };
}

function normalizeAiProviderDiscoveryInput(
  input: DiscoverAiProviderModelsPayload,
): DiscoverAiProviderModelsPayload {
  const providerId = input.providerId
    ? requireProviderId(input.providerId)
    : null;
  const kind = requireNonEmptyText(input.kind, "kind");
  if (kind !== "openAiCompatible") {
    throw new TypeError("kind must be openAiCompatible");
  }
  return {
    providerId,
    kind,
    baseUrl: normalizeProviderBaseUrl(input.baseUrl),
    apiKey: input.apiKey?.trim() || null,
  };
}

function requireProviderId(providerId: string): string {
  return requireNonEmptyText(providerId, "providerId");
}

function requireNonEmptyText(value: string, field: string): string {
  const normalized = value.trim();
  if (!normalized) {
    throw new TypeError(`${field} must not be empty`);
  }
  return normalized;
}

function normalizeProviderBaseUrl(value: string): string {
  const normalized = requireNonEmptyText(value, "baseUrl").replace(/\/+$/, "");
  let parsed: URL;
  try {
    parsed = new URL(normalized);
  } catch {
    throw new TypeError("baseUrl must be a valid URL");
  }
  const localHost = ["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname);
  if (parsed.protocol !== "https:" && !(parsed.protocol === "http:" && localHost)) {
    throw new TypeError("baseUrl must use HTTPS unless it targets localhost");
  }
  if (parsed.username || parsed.password || parsed.search || parsed.hash) {
    throw new TypeError("baseUrl must not include credentials, query, or fragment");
  }
  return normalized;
}

function requireStableAssetId(assetId: string): string {
  const normalized = assetId.trim();
  if (!normalized) throw new TypeError("assetId must not be empty");
  if (/[/\\:]/.test(normalized) || normalized.toLowerCase().startsWith("file")) {
    throw new TypeError("assetId must be a stable identifier, not a path or URL");
  }
  return normalized;
}

function requireRevision(revision: number | undefined): number {
  if (!Number.isInteger(revision) || (revision ?? -1) < 0) {
    throw new TypeError("expectedRevision must be a non-negative integer");
  }
  return revision as number;
}

function optionalRevision(revision: number | null): number | null {
  return revision === null ? null : requireRevision(revision);
}

const STORAGE_LOCATION_PATHS: Record<StorageLocationTarget, string> = {
  dataRoot: "bsaigc-storage://data-root",
  ledger: "bsaigc-storage://ledger",
  vault: "bsaigc-storage://vault",
  cache: "bsaigc-storage://cache",
  staging: "bsaigc-storage://staging",
  credentials: "bsaigc-storage://credentials",
};

function requireStorageLocationTarget(
  target: StorageLocationTarget,
): StorageLocationTarget {
  if (!Object.prototype.hasOwnProperty.call(STORAGE_LOCATION_PATHS, target)) {
    throw new TypeError("target must be a managed storage location");
  }
  return target;
}

function assertSafeDesktopSettingsSnapshot(
  snapshot: DesktopSettingsSnapshot,
): void {
  if (snapshot.storage.dataRoot !== STORAGE_LOCATION_PATHS.dataRoot) {
    throw hostError(
      "UNSAFE_STORAGE_PATH",
      "Host returned an unsafe storage root capability",
      false,
    );
  }
  for (const location of snapshot.storage.locations) {
    if (location.path !== STORAGE_LOCATION_PATHS[location.target]) {
      throw hostError(
        "UNSAFE_STORAGE_PATH",
        "Host returned an unsafe storage location capability",
        false,
      );
    }
  }
}

function requirePositiveRevision(revision: number | undefined): number {
  if (!Number.isInteger(revision) || (revision ?? 0) <= 0) {
    throw new TypeError("expectedRevision must be a positive integer");
  }
  return revision as number;
}

function optionalPositiveRevision(revision: number | null): number | null {
  return revision === null ? null : requirePositiveRevision(revision);
}

function requirePositiveLimit(limit: number): number {
  if (!Number.isInteger(limit) || limit <= 0) {
    throw new TypeError("limit must be a positive integer");
  }
  return limit;
}

function requireNonNegativeSequence(sequence: number): number {
  if (!Number.isInteger(sequence) || sequence < 0) {
    throw new TypeError("afterSequence must be a non-negative integer");
  }
  return sequence;
}

interface SequencedEvent {
  readonly sequence: number;
}

function compareBufferedEvents(left: SequencedEvent, right: SequencedEvent): number {
  return left.sequence - right.sequence;
}

function applyPage<T extends SequencedEvent>(
  page: readonly T[],
  afterSequence: number,
  apply: (event: T) => void,
): number {
  let pageLastSequence = afterSequence;
  for (const event of page) {
    apply(event);
    pageLastSequence = Math.max(pageLastSequence, event.sequence);
  }
  return pageLastSequence;
}

function ensureSequenceAdvanced(
  previousSequence: number,
  nextSequence: number,
  stream: string,
): void {
  if (nextSequence <= previousSequence) {
    throw hostError(
      "INVALID_EVENT_PAGE",
      `Host returned a full ${stream} event page without advancing sequence`,
      false,
    );
  }
}

function combineUnsubscribes(subscriptions: readonly Unsubscribe[]): Unsubscribe {
  let active = true;
  return () => {
    if (!active) return;
    active = false;
    for (const unsubscribe of subscriptions) {
      unsubscribe();
    }
  };
}

function hostError(
  code: string,
  message: string,
  retryable: boolean,
): HostError {
  return { code, message, retryable };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
