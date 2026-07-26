export { BsaigcClient, normalizeHostError } from "./BsaigcClient";
export type {
  BsaigcClientOptions,
  BsaigcClientSnapshot,
  ChangeProjectStageInput,
  CommandOptions,
  ScopedCommandOptions,
  UpdateProjectBriefInput,
} from "./BsaigcClient";
export { DesktopHostAdapter, isTauriRuntime } from "./DesktopHostAdapter";
export { EventProjection } from "./EventProjection";
export { TaskProjection } from "./TaskProjection";
export type { TaskProjectionSnapshot } from "./TaskProjection";
export { AssetProjection } from "./AssetProjection";
export type { AssetProjectionSnapshot } from "./AssetProjection";
export { ExecutionBriefProjection } from "./ExecutionBriefProjection";
export type { ExecutionBriefProjectionSnapshot } from "./ExecutionBriefProjection";
export { RequirementBriefProjection } from "./RequirementBriefProjection";
export type { RequirementBriefProjectionSnapshot } from "./RequirementBriefProjection";
export { BusinessWorkspaceProjection } from "./BusinessWorkspaceProjection";
export type { BusinessWorkspaceProjectionSnapshot } from "./BusinessWorkspaceProjection";
export type { EventProjectionSnapshot } from "./EventProjection";
export {
  ASSET_EVENT_CHANNEL,
  BACKUP_EVENT_CHANNEL,
  BSAIGC_PROTOCOL_VERSION,
  BUSINESS_WORKSPACE_PROTOCOL_VERSION,
  BUSINESS_WORKSPACE_EVENT_CHANNEL,
  CONTRACT_REVIEW_EVENT_CHANNEL,
  DOMAIN_EVENT_CHANNEL,
  EXECUTION_BRIEF_EVENT_CHANNEL,
  REQUIREMENT_BRIEF_EVENT_CHANNEL,
  TASK_EVENT_CHANNEL,
} from "./HostAdapter";
export type {
  AssetActionCapabilities,
  AssetEventListener,
  BackupEventListener,
  BusinessWorkspaceEventListener,
  ContractReviewEventListener,
  CreateBusinessWorkspaceInput,
  DomainEventListener,
  ExecutionBriefEventListener,
  HostAdapter,
  ListBusinessCustomersInput,
  ListBusinessWorkspacePrefillCandidatesInput,
  ListContractReviewsInput,
  ListReviewFindingsInput,
  RequirementBriefEventListener,
  TaskEventListener,
  Unsubscribe,
} from "./HostAdapter";
export { WebHostAdapter } from "./WebHostAdapter";
export type { BusinessCustomerReceivableSummary } from "../generated/bsaigc/BusinessCustomerReceivableSummary";
export type { ListBusinessCustomersRequest } from "../generated/bsaigc/ListBusinessCustomersRequest";
export type { BusinessWorkspacePrefillCandidate } from "../generated/bsaigc/BusinessWorkspacePrefillCandidate";
export type { BusinessWorkspacePrefillChange } from "../generated/bsaigc/BusinessWorkspacePrefillChange";
export type { BusinessWorkspacePrefillDecision } from "../generated/bsaigc/BusinessWorkspacePrefillDecision";
export type { BusinessWorkspacePrefillField } from "../generated/bsaigc/BusinessWorkspacePrefillField";
export type { BusinessWorkspacePrefillMatchKind } from "../generated/bsaigc/BusinessWorkspacePrefillMatchKind";
export type { BusinessWorkspacePrefillPreview } from "../generated/bsaigc/BusinessWorkspacePrefillPreview";
export type { ListBusinessWorkspacePrefillCandidatesRequest } from "../generated/bsaigc/ListBusinessWorkspacePrefillCandidatesRequest";
export type { PreviewBusinessWorkspacePrefillRequest } from "../generated/bsaigc/PreviewBusinessWorkspacePrefillRequest";

export type { AssetBackupRecord } from "../generated/bsaigc/AssetBackupRecord";
export type { BackupCommandEnvelope } from "../generated/bsaigc/BackupCommandEnvelope";
export type { BackupCommandResponse } from "../generated/bsaigc/BackupCommandResponse";
export type { BackupDomainEvent } from "../generated/bsaigc/BackupDomainEvent";
export type { BackupState } from "../generated/bsaigc/BackupState";
export type { ContractReviewCommandEnvelope } from "../generated/bsaigc/ContractReviewCommandEnvelope";
export type { ContractReviewCommandResponse } from "../generated/bsaigc/ContractReviewCommandResponse";
export type { ContractReviewDomainEvent } from "../generated/bsaigc/ContractReviewDomainEvent";
export type { ContractReviewRecord } from "../generated/bsaigc/ContractReviewRecord";
export type { EvidenceContext } from "../generated/bsaigc/EvidenceContext";
export type { GetContractReviewRequest } from "../generated/bsaigc/GetContractReviewRequest";
export type { GetEvidenceContextRequest } from "../generated/bsaigc/GetEvidenceContextRequest";
export type { ListContractReviewsRequest } from "../generated/bsaigc/ListContractReviewsRequest";
export type { ListReviewFindingsRequest } from "../generated/bsaigc/ListReviewFindingsRequest";
export type { ReviewFindingRecord } from "../generated/bsaigc/ReviewFindingRecord";
export type { BusinessCurrentDocuments } from "../generated/bsaigc/BusinessCurrentDocuments";
export type { BusinessDocumentStatus } from "../generated/bsaigc/BusinessDocumentStatus";
export type { BusinessFinancialSummary } from "../generated/bsaigc/BusinessFinancialSummary";
export type { BusinessLifecycleStage } from "../generated/bsaigc/BusinessLifecycleStage";
export type { BusinessWorkspaceDomainEvent } from "../generated/bsaigc/BusinessWorkspaceDomainEvent";
export type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
export type { BusinessWorkspaceStatus } from "../generated/bsaigc/BusinessWorkspaceStatus";
