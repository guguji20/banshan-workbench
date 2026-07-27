import { describe, expect, it, vi } from "vitest";
import type { BriefRecord } from "../generated/bsaigc/BriefRecord";
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
import type { CreateCasePayload } from "../generated/bsaigc/CreateCasePayload";
import type { UpdateCasePayload } from "../generated/bsaigc/UpdateCasePayload";
import type { CreateExecutionBriefPayload } from "../generated/bsaigc/CreateExecutionBriefPayload";
import type { ExecutionBriefCommandEnvelope } from "../generated/bsaigc/ExecutionBriefCommandEnvelope";
import type { ExecutionBriefCommandResponse } from "../generated/bsaigc/ExecutionBriefCommandResponse";
import type { ExecutionBriefContent } from "../generated/bsaigc/ExecutionBriefContent";
import type { ExecutionBriefDomainEvent } from "../generated/bsaigc/ExecutionBriefDomainEvent";
import type { ExecutionBriefRecord } from "../generated/bsaigc/ExecutionBriefRecord";
import type { UpdateExecutionBriefPayload } from "../generated/bsaigc/UpdateExecutionBriefPayload";
import type { CreateRequirementBriefPayload } from "../generated/bsaigc/CreateRequirementBriefPayload";
import type { RequirementBriefCommandEnvelope } from "../generated/bsaigc/RequirementBriefCommandEnvelope";
import type { RequirementBriefCommandResponse } from "../generated/bsaigc/RequirementBriefCommandResponse";
import type { RequirementBriefContent } from "../generated/bsaigc/RequirementBriefContent";
import type { RequirementBriefDomainEvent } from "../generated/bsaigc/RequirementBriefDomainEvent";
import type { RequirementBriefRecord } from "../generated/bsaigc/RequirementBriefRecord";
import type { UpdateRequirementBriefPayload } from "../generated/bsaigc/UpdateRequirementBriefPayload";
import type { BusinessCustomerReceivableSummary } from "../generated/bsaigc/BusinessCustomerReceivableSummary";
import type { BusinessProfileInput } from "../generated/bsaigc/BusinessProfileInput";
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
import type { AiCredentialCommandEnvelope } from "../generated/bsaigc/AiCredentialCommandEnvelope";
import type { AiCredentialCommandResponse } from "../generated/bsaigc/AiCredentialCommandResponse";
import type { AiCredentialStatus } from "../generated/bsaigc/AiCredentialStatus";
import type { DesktopSettingsCommandEnvelope } from "../generated/bsaigc/DesktopSettingsCommandEnvelope";
import type { DesktopSettingsCommandResponse } from "../generated/bsaigc/DesktopSettingsCommandResponse";
import type { DesktopSettingsSnapshot } from "../generated/bsaigc/DesktopSettingsSnapshot";
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
import {
  BsaigcClient,
  normalizeHostError,
} from "./BsaigcClient";
import type {
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

const EMPTY_BRIEF: BriefRecord = {
  objective: "",
  audience: "",
  deliverables: [],
  styleKeywords: [],
  mandatoryItems: [],
  constraints: [],
  risks: [],
  referenceNotes: "",
};

function project(revision: number, id = "project-1"): ProjectRecord {
  return {
    id,
    name: `Project r${revision}`,
    clientName: "Client",
    brief: EMPTY_BRIEF,
    stage: revision > 1 ? "creative" : "intake",
    revision,
    createdAt: 1,
    updatedAt: revision,
  };
}

function event(sequence: number, revision = sequence): DomainEvent {
  const record = project(revision);
  return {
    sequence,
    eventId: `event-${sequence}`,
    eventType: revision === 1 ? "project.created" : "project.stageChanged",
    aggregateType: "project",
    aggregateId: record.id,
    revision,
    occurredAt: sequence,
    traceId: `trace-${sequence}`,
    project: record,
  };
}

function task(revision = 1, status: TaskRecord["status"] = "queued"): TaskRecord {
  return {
    id: "task-1",
    kind: "media.thumbnail",
    projectId: "project-1",
    input: { assetId: "asset-1" },
    output: null,
    status,
    priority: "high",
    replayPolicy: "safe",
    progress: status === "succeeded" ? 100 : 0,
    attempt: 0,
    maxAttempts: 3,
    revision,
    createdAt: 1,
    updatedAt: revision,
    startedAt: null,
    finishedAt: null,
    lastError: null,
    dependencies: [],
  };
}

function taskEvent(sequence: number, revision = sequence): TaskDomainEvent {
  return {
    sequence,
    eventId: `task-event-${sequence}`,
    eventType: revision === 1 ? "task.created" : "task.progressed",
    aggregateId: "task-1",
    revision,
    occurredAt: sequence,
    traceId: `task-trace-${sequence}`,
    task: task(revision, revision > 1 ? "running" : "queued"),
  };
}

function asset(revision = 1): AssetRecord {
  return {
    id: "asset-1",
    projectId: "project-1",
    originalName: "reference.png",
    kind: "image",
    mimeType: "image/png",
    sizeBytes: 128,
    sha256: "a".repeat(64),
    status: "ready",
    revision,
    createdAt: 1,
    updatedAt: revision,
    previewAvailable: false,
  };
}

function assetEvent(sequence: number, revision = 1): AssetDomainEvent {
  return {
    sequence,
    eventId: `asset-event-${sequence}`,
    eventType: "asset.imported",
    aggregateId: "asset-1",
    revision,
    occurredAt: sequence,
    traceId: `asset-trace-${sequence}`,
    asset: asset(revision),
  };
}

function caseRecord(
  revision = 1,
  projectId: string | null = "project-1",
): CaseRecord {
  return {
    id: "case-1",
    assetId: "asset-case-1",
    projectId,
    title: `Case r${revision}`,
    clientName: "Client",
    contentType: "brand",
    presentation: "liveAction",
    hasActors: true,
    isAigc: false,
    qualityTier: "featured",
    tags: ["campaign"],
    notes: "",
    revision,
    createdAt: 1,
    updatedAt: revision,
  };
}

function caseEvent(sequence: number, revision = sequence): CaseDomainEvent {
  return {
    sequence,
    eventId: `case-event-${sequence}`,
    eventType: revision === 1 ? "case.created" : "case.updated",
    aggregateId: "case-1",
    revision,
    occurredAt: sequence,
    traceId: `case-trace-${sequence}`,
    caseRecord: caseRecord(revision),
  };
}

function createCasePayload(
  projectId: string | null = "project-1",
): CreateCasePayload {
  return {
    assetId: "asset-case-1",
    projectId,
    title: "Launch film",
    clientName: "ACME",
    contentType: "brand",
    presentation: "liveAction",
    hasActors: true,
    isAigc: false,
    qualityTier: "featured",
    tags: ["launch", "film"],
    notes: "Owned by the campaign project",
  };
}

function updateCasePayload(): UpdateCasePayload {
  return {
    caseId: "case-1",
    title: "Launch film v2",
    clientName: "ACME",
    contentType: "brand",
    presentation: "mixedMedia",
    hasActors: true,
    isAigc: true,
    qualityTier: "premium",
    tags: ["launch", "aigc"],
    notes: "Updated treatment",
  };
}

const EXECUTION_BRIEF_CONTENT: ExecutionBriefContent = {
  shootAt: 1_800_000_000_000,
  clientGoal: "Show the product in use",
  visualStyle: "Natural documentary",
  primaryShots: ["Hero walk-through"],
  secondaryShots: ["Detail inserts"],
  requiredShots: ["Pack shot"],
  fallbackShots: ["Static tabletop"],
  riskPoints: ["Weather"],
  waitingTimeActions: ["Capture ambient sound"],
  equipmentNotes: "Bring rain covers",
  postShootHighlights: ["Golden-hour hero"],
};

function executionBriefRecord(
  revision = 1,
  projectId = "project-1",
): ExecutionBriefRecord {
  return {
    id: "brief-1",
    projectId,
    content: {
      ...EXECUTION_BRIEF_CONTENT,
      clientGoal: `${EXECUTION_BRIEF_CONTENT.clientGoal} r${revision}`,
    },
    status: revision > 2 ? "ready" : "draft",
    revision,
    createdAt: 1,
    updatedAt: revision,
  };
}

function executionBriefEvent(
  sequence: number,
  revision = sequence,
): ExecutionBriefDomainEvent {
  return {
    sequence,
    eventId: `execution-brief-event-${sequence}`,
    eventType:
      revision === 1
        ? "executionBrief.created"
        : "executionBrief.updated",
    aggregateId: "brief-1",
    revision,
    occurredAt: sequence,
    traceId: `execution-brief-trace-${sequence}`,
    executionBrief: executionBriefRecord(revision),
  };
}

function createExecutionBriefPayload(
  projectId = "project-1",
): CreateExecutionBriefPayload {
  return { projectId, content: EXECUTION_BRIEF_CONTENT };
}

function updateExecutionBriefPayload(): UpdateExecutionBriefPayload {
  return {
    briefId: "brief-1",
    content: {
      ...EXECUTION_BRIEF_CONTENT,
      equipmentNotes: "Add a compact LED kit",
    },
  };
}

const REQUIREMENT_BRIEF_CONTENT: RequirementBriefContent = {
  objective: "Launch the new product",
  audience: "Design-conscious buyers",
  keyMessage: "Made for daily creative work",
  deliverables: ["Launch film"],
  channels: ["Web", "Social"],
  styleKeywords: ["Natural", "Confident"],
  mandatoryItems: ["Product close-up"],
  constraints: ["No competitor marks"],
  acceptanceCriteria: ["Client approval"],
  risks: ["Late product sample"],
  deadlineAt: 1_800_000_000_000,
  budgetNotes: "Production budget confirmed",
  referenceCaseIds: ["case-1"],
  referenceNotes: "Use the launch-film pacing",
};

function requirementBriefRecord(
  revision = 1,
  projectId = "project-1",
): RequirementBriefRecord {
  return {
    id: "requirement-1",
    projectId,
    questionSetVersion: "1.0",
    answers: [],
    content: {
      ...REQUIREMENT_BRIEF_CONTENT,
      objective: `${REQUIREMENT_BRIEF_CONTENT.objective} r${revision}`,
    },
    status:
      revision > 2 ? "confirmed" : revision > 1 ? "review" : "interviewing",
    confirmedAt: revision > 2 ? revision : null,
    confirmedBy: revision > 2 ? "intake-producer" : null,
    revision,
    createdAt: 1,
    updatedAt: revision,
  };
}

function requirementBriefEvent(
  sequence: number,
  revision = sequence,
): RequirementBriefDomainEvent {
  return {
    sequence,
    eventId: `requirement-brief-event-${sequence}`,
    eventType:
      revision === 1
        ? "requirementBrief.created"
        : "requirementBrief.updated",
    aggregateId: "requirement-1",
    revision,
    occurredAt: sequence,
    traceId: `requirement-brief-trace-${sequence}`,
    requirementBrief: requirementBriefRecord(revision),
  };
}

function createRequirementBriefPayload(
  projectId = "project-1",
): CreateRequirementBriefPayload {
  return { projectId };
}

function updateRequirementBriefPayload(): UpdateRequirementBriefPayload {
  return {
    briefId: "requirement-1",
    answers: [],
    content: {
      ...REQUIREMENT_BRIEF_CONTENT,
      budgetNotes: "Budget includes one contingency day",
    },
  };
}

const BUSINESS_PROFILE_INPUT: BusinessProfileInput = {
  projectTitle: "Launch campaign",
  projectCode: "LAUNCH-001",
  customerName: "Client",
  customerLegalName: "Client Limited",
  customerTaxId: "customer-tax-id",
  customerAddress: "Customer address",
  customerContact: "Customer contact",
  customerPhone: "10000",
  customerEmail: "client@example.com",
  supplierLegalName: "Studio Limited",
  supplierTaxId: "supplier-tax-id",
  supplierAddress: "Supplier address",
  supplierContact: "Supplier contact",
  supplierPhone: "10001",
  supplierBankName: "Business Bank",
  supplierBankAccount: "1000000001",
  currency: "CNY",
  defaultTaxRateBps: 600,
  serviceStartAt: null,
  serviceEndAt: null,
  deliverySummary: "Launch film",
  paymentTerms: "Net 30",
  acceptanceTerms: "Written approval",
  notes: "",
  lineItems: [],
};

const BUSINESS_CUSTOMER: BusinessWorkspaceRecord["customer"] = {
  id: "customer-1",
  displayName: BUSINESS_PROFILE_INPUT.customerName,
  legalName: BUSINESS_PROFILE_INPUT.customerLegalName,
  taxId: BUSINESS_PROFILE_INPUT.customerTaxId,
  billingAddress: BUSINESS_PROFILE_INPUT.customerAddress,
  primaryContactName: BUSINESS_PROFILE_INPUT.customerContact,
  primaryPhone: BUSINESS_PROFILE_INPUT.customerPhone,
  primaryEmail: BUSINESS_PROFILE_INPUT.customerEmail,
  notes: "",
  status: "active",
  revision: 1,
  createdAt: 1,
  updatedAt: 1,
  archivedAt: null,
  archivedBy: null,
};

function businessWorkspaceRecord(
  revision = 1,
  projectId = "project-1",
): BusinessWorkspaceRecord {
  return {
    id: "workspace-1",
    projectId,
    customerId: BUSINESS_CUSTOMER.id,
    customer: { ...BUSINESS_CUSTOMER },
    requirementBriefId: null,
    requirementBriefRevision: null,
    prefillSourceWorkspaceId: null,
    profile: { ...BUSINESS_PROFILE_INPUT, lineItems: [] },
    documents: [],
    payments: [],
    quoteConfirmations: [],
    receipts: [],
    milestones: [],
    deliverySubmissions: [],
    invoices: [],
    archiveSnapshots: [],
    archiveIntegrityStatus: "notCaptured",
    status: revision > 6 ? "archived" : "active",
    archivedAt: revision > 6 ? revision : null,
    archivedBy: revision > 6 ? "operator-1" : null,
    lifecycleStage: revision > 6 ? "archived" : "draft",
    financialSummary: {
      quotedCents: 0,
      contractCents: 0,
      scheduledCents: 0,
      requestedCents: 0,
      receivedCents: 0,
      outstandingCents: 0,
    },
    currentDocuments: {
      quoteDocumentId: null,
      contractDocumentId: null,
      paymentRequestDocumentId: null,
      acceptanceDocumentId: null,
    },
    revision,
    createdAt: 1,
    updatedAt: revision,
  };
}

function businessWorkspaceEvent(
  sequence: number,
  revision = sequence,
): BusinessWorkspaceDomainEvent {
  return {
    sequence,
    eventId: `business-workspace-event-${sequence}`,
    eventType:
      revision === 1
        ? "businessWorkspace.created"
        : "businessWorkspace.profileUpdated",
    aggregateId: "workspace-1",
    revision,
    occurredAt: sequence,
    traceId: `business-workspace-trace-${sequence}`,
    actorId: "actor-1",
    commandId: `business-command-${sequence}`,
    reason: "测试商务操作",
    businessWorkspace: businessWorkspaceRecord(revision),
  };
}

const HOST_STATUS: HostStatus = {
  protocolVersion: "1.5",
  databaseReady: true,
  vaultReady: true,
  projectCount: 0,
  taskCount: 0,
  assetCount: 0,
  lastEventSequence: 0,
  runtime: "test",
  modules: [],
};

const CODEX_STATUS: CodexProbeStatus = {
  available: true,
  runtime: "codex-app-server",
  transport: "stdio",
  userAgent: null,
  platformFamily: null,
  platformOs: null,
  codexHomeReady: true,
  source: null,
  handshakeAt: 1,
  error: null,
};

const CONTRACT_REVIEW: ContractReviewRecord = {
  session: {
    id: "review-1",
    workspaceId: "workspace-1",
    sourceAssetId: "asset-contract",
    sourceAssetSha256: "a".repeat(64),
    sourceFileName: "contract.pdf",
    status: "draft",
    stage: "created",
    extractionId: null,
    reportAssetId: null,
    revision: 1,
    createdAt: 1,
    updatedAt: 1,
    completedAt: null,
    failure: null,
  },
  extraction: null,
  evidence: [],
  findings: [],
  ruleEvaluations: [],
  decisions: [],
  reports: [],
};

const REVIEW_FINDING: ReviewFindingRecord = {
  id: "finding-1",
  reviewId: "review-1",
  source: "rule",
  ruleId: "payment.deadline",
  ruleVersion: "1",
  agentRunId: null,
  category: "payment terms",
  severity: "high",
  title: "Missing payment deadline",
  description: "The contract does not define a payment date.",
  recommendation: "Add a payment date and overdue obligations.",
  evidenceIds: ["evidence-1"],
  missingEvidenceReason: null,
  status: "open",
  decision: "unreviewed",
  revision: 1,
  createdAt: 1,
  updatedAt: 1,
};

const EVIDENCE_CONTEXT: EvidenceContext = {
  evidence: {
    id: "evidence-1",
    extractionId: "extraction-1",
    sourceAssetId: "asset-contract",
    pageIndex: 0,
    blockId: "block-1",
    charStart: 0,
    charEnd: 8,
    bbox: null,
    quotedText: "Payment terms",
    quotedTextSha256: "b".repeat(64),
    contextBefore: "",
    contextAfter: "Payment follows acceptance.",
  },
  page: {
    id: "page-1",
    extractionId: "extraction-1",
    pageIndex: 0,
    text: "Payment terms: payment follows acceptance.",
    textSha256: "c".repeat(64),
    width: null,
    height: null,
    previewAssetId: "asset-page-preview",
  },
  block: {
    id: "block-1",
    extractionId: "extraction-1",
    pageId: "page-1",
    pageIndex: 0,
    orderIndex: 0,
    kind: "paragraph",
    text: "Payment terms: payment follows acceptance.",
    charStart: 0,
    charEnd: 14,
    bbox: null,
  },
};

const ASSET_BACKUP: AssetBackupRecord = {
  assetId: "asset-contract",
  contentSha256: "a".repeat(64),
  state: "queued",
  attemptCount: 0,
  nextAttemptAt: null,
  lastError: null,
  remoteObjectKey: null,
  remoteEtag: null,
  revision: 1,
  createdAt: 1,
  updatedAt: 1,
  backedUpAt: null,
};

function contractReviewEvent(sequence: number): ContractReviewDomainEvent {
  return {
    sequence,
    eventId: `contract-review-event-${sequence}`,
    eventType: "contractReview.created",
    aggregateId: CONTRACT_REVIEW.session.id,
    revision: sequence,
    occurredAt: sequence,
    traceId: `contract-review-trace-${sequence}`,
    contractReview: {
      ...CONTRACT_REVIEW,
      session: { ...CONTRACT_REVIEW.session, revision: sequence },
    },
  };
}

function backupEvent(sequence: number): BackupDomainEvent {
  return {
    sequence,
    eventId: `backup-event-${sequence}`,
    eventType: "backup.queued",
    assetId: ASSET_BACKUP.assetId,
    revision: sequence,
    occurredAt: sequence,
    traceId: `backup-trace-${sequence}`,
    backup: { ...ASSET_BACKUP, revision: sequence },
  };
}

const AI_CREDENTIAL_STATUS: AiCredentialStatus = {
  provider: "bsaigc",
  configured: false,
  persisted: false,
  protection: null,
  revision: 0,
  updatedAt: null,
  appliesOnNextRuntimeStart: false,
  defaultProviderId: null,
  defaultModel: null,
  providers: [],
};

const DESKTOP_SETTINGS_SNAPSHOT: DesktopSettingsSnapshot = {
  storage: {
    dataRoot: "bsaigc-storage://data-root",
    totalBytes: 3_145_728,
    cacheBytes: 1_048_576,
    locations: [
      {
        target: "dataRoot",
        label: "Application data",
        path: "bsaigc-storage://data-root",
        sizeBytes: 3_145_728,
        exists: true,
        authoritative: true,
        clearable: false,
      },
      {
        target: "ledger",
        label: "Local ledger",
        path: "bsaigc-storage://ledger",
        sizeBytes: 1_048_576,
        exists: true,
        authoritative: true,
        clearable: false,
      },
      {
        target: "vault",
        label: "Local Vault",
        path: "bsaigc-storage://vault",
        sizeBytes: 1_048_576,
        exists: true,
        authoritative: true,
        clearable: false,
      },
      {
        target: "cache",
        label: "Regenerable cache",
        path: "bsaigc-storage://cache",
        sizeBytes: 1_048_576,
        exists: true,
        authoritative: false,
        clearable: true,
      },
    ],
  },
  channelAdapters: [
    {
      id: "feishu-cli",
      name: "Feishu CLI",
      state: "planned",
      configured: false,
      capabilities: ["message.receive", "message.send"],
      message: "Feishu CLI is reserved for a later release.",
    },
  ],
  cloudBackup: {
    provider: "cloudflare-r2",
    mode: "asyncBackupOnly",
    configured: false,
    ready: false,
    state: "notConfigured",
    message: "R2 backup is not configured.",
    pendingItems: 0,
  },
  update: {
    currentVersion: "1.0.0",
    buildChannel: "development",
    buildVersion: "1.0.0-dev.1",
    codexRuntimeVersion: "0.144.5",
    updateSourceConfigured: false,
    automaticInstallAllowed: false,
    state: "notConfigured",
    message: "Signed update source is not configured.",
    latestVersion: null,
    downloadUrl: null,
    lastCheckedAt: null,
  },
  revision: 0,
};

class FakeHostAdapter implements HostAdapter {
  readonly kind = "desktop" as const;
  brainThreadArchive(threadId: string, archived: boolean) {
    return Promise.resolve({
      id: threadId,
      projectId: null,
      title: null,
      model: null,
      status: archived ? "archived" : "ready",
      createdAt: 0,
      updatedAt: 0,
    } as import("../generated/bsaigc/BrainThreadRecord").BrainThreadRecord);
  }
  brainThreadRename(threadId: string, title: string) {
    return Promise.resolve({
      id: threadId,
      projectId: null,
      title,
      model: null,
      status: "ready",
      createdAt: 0,
      updatedAt: 0,
    } as import("../generated/bsaigc/BrainThreadRecord").BrainThreadRecord);
  }
  brainThreadDelete(_threadId: string) {
    return Promise.resolve();
  }
  authStatus() {
    return Promise.resolve({
      initialized: true,
      currentUser: null,
      registrySync: "localOnly",
      registryMessage: null,
      registryRevision: 0,
      userCount: 1,
    } as import("../generated/bsaigc/AuthStatus").AuthStatus);
  }
  authInitializeAdmin() {
    return this.authStatus();
  }
  authLogin() {
    return this.authStatus();
  }
  authLogout() {
    return this.authStatus();
  }
  authRememberedCredentials() {
    return Promise.resolve(null);
  }
  authRememberCredentials() {
    return Promise.resolve();
  }
  authForgetCredentials() {
    return Promise.resolve();
  }
  authChangePassword() {
    return this.authStatus();
  }
  authListUsers() {
    return Promise.resolve({
      users: [],
      registrySync: "localOnly",
      registryMessage: null,
      registryRevision: 0,
    } as import("../generated/bsaigc/AuthUsersSnapshot").AuthUsersSnapshot);
  }
  authCreateUser() {
    return this.authListUsers();
  }
  authResetPassword() {
    return this.authListUsers();
  }
  authDeleteUser() {
    return this.authListUsers();
  }
  authRefreshRegistry() {
    return this.authStatus();
  }
  readonly replayCalls: Array<[number, number]> = [];
  readonly taskReplayCalls: Array<[number, number]> = [];
  readonly assetReplayCalls: Array<[number, number]> = [];
  readonly caseReplayCalls: Array<[number, number]> = [];
  readonly executionBriefReplayCalls: Array<[number, number]> = [];
  readonly requirementBriefReplayCalls: Array<[number, number]> = [];
  readonly businessWorkspaceReplayCalls: Array<[number, number]> = [];
  readonly contractReviewReplayCalls: Array<[number, number]> = [];
  readonly backupReplayCalls: Array<[number, number]> = [];
  readonly commands: CommandEnvelope[] = [];
  readonly taskCommands: TaskCommandEnvelope[] = [];
  readonly assetCommands: AssetCommandEnvelope[] = [];
  readonly caseCommands: CaseCommandEnvelope[] = [];
  readonly executionBriefCommands: ExecutionBriefCommandEnvelope[] = [];
  readonly requirementBriefCommands: RequirementBriefCommandEnvelope[] = [];
  readonly businessWorkspaceCommands: BusinessWorkspaceCommandEnvelope[] = [];
  readonly contractReviewCommands: ContractReviewCommandEnvelope[] = [];
  readonly backupCommands: BackupCommandEnvelope[] = [];
  readonly aiCredentialCommands: AiCredentialCommandEnvelope[] = [];
  readonly desktopSettingsCommands: DesktopSettingsCommandEnvelope[] = [];
  readonly businessCustomerListRequests: ListBusinessCustomersRequest[] = [];
  readonly businessWorkspacePrefillCandidateRequests: ListBusinessWorkspacePrefillCandidatesRequest[] = [];
  readonly businessWorkspacePrefillPreviewRequests: PreviewBusinessWorkspacePrefillRequest[] = [];
  readonly contractReviewListRequests: ListContractReviewsRequest[] = [];
  readonly contractReviewGetRequests: GetContractReviewRequest[] = [];
  readonly reviewFindingListRequests: ListReviewFindingsRequest[] = [];
  readonly evidenceContextGetRequests: GetEvidenceContextRequest[] = [];
  readonly assetBackupListLimits: number[] = [];
  projects: ProjectRecord[] = [];
  events: DomainEvent[] = [];
  tasks: TaskRecord[] = [];
  taskEvents: TaskDomainEvent[] = [];
  assets: AssetRecord[] = [];
  assetEvents: AssetDomainEvent[] = [];
  cases: CaseRecord[] = [];
  caseEvents: CaseDomainEvent[] = [];
  executionBriefs: ExecutionBriefRecord[] = [];
  executionBriefEvents: ExecutionBriefDomainEvent[] = [];
  requirementBriefs: RequirementBriefRecord[] = [];
  requirementBriefEvents: RequirementBriefDomainEvent[] = [];
  businessWorkspaces: BusinessWorkspaceRecord[] = [];
  businessCustomers: BusinessCustomerReceivableSummary[] = [];
  businessWorkspaceEvents: BusinessWorkspaceDomainEvent[] = [];
  contractReviews: ContractReviewRecord[] = [CONTRACT_REVIEW];
  reviewFindings: ReviewFindingRecord[] = [REVIEW_FINDING];
  contractReviewEvents: ContractReviewDomainEvent[] = [];
  evidenceContext: EvidenceContext = EVIDENCE_CONTEXT;
  desktopSettingsSnapshot: DesktopSettingsSnapshot = DESKTOP_SETTINGS_SNAPSHOT;
  assetBackups: AssetBackupRecord[] = [ASSET_BACKUP];
  backupEvents: BackupDomainEvent[] = [];
  listener: DomainEventListener | null = null;
  taskListener: TaskEventListener | null = null;
  assetListener: AssetEventListener | null = null;
  brainListener: BrainEventListener | null = null;
  caseListener: CaseEventListener | null = null;
  executionBriefListener: ExecutionBriefEventListener | null = null;
  requirementBriefListener: RequirementBriefEventListener | null = null;
  businessWorkspaceListener: BusinessWorkspaceEventListener | null = null;
  contractReviewListener: ContractReviewEventListener | null = null;
  backupListener: BackupEventListener | null = null;
  unsubscribed = false;
  listProjectsImpl: () => Promise<ProjectRecord[]> = async () => this.projects;
  listTasksImpl: () => Promise<TaskRecord[]> = async () => this.tasks;
  listAssetsImpl: () => Promise<AssetRecord[]> = async () => this.assets;
  listCasesImpl: () => Promise<CaseRecord[]> = async () => this.cases;
  listExecutionBriefsImpl: () => Promise<ExecutionBriefRecord[]> = async () =>
    this.executionBriefs;
  listRequirementBriefsImpl: () =>
    Promise<RequirementBriefRecord[]> = async () => this.requirementBriefs;
  listBusinessWorkspacesImpl: () => Promise<BusinessWorkspaceRecord[]> =
    async () => this.businessWorkspaces;
  listBusinessCustomersImpl: (
    _request: ListBusinessCustomersRequest,
  ) => Promise<BusinessCustomerReceivableSummary[]> = async () =>
    this.businessCustomers;
  listBusinessWorkspacePrefillCandidatesImpl: (
    request: ListBusinessWorkspacePrefillCandidatesRequest,
  ) => Promise<BusinessWorkspacePrefillCandidate[]> = async (_request) => [];
  previewBusinessWorkspacePrefillImpl: (
    request: PreviewBusinessWorkspacePrefillRequest,
  ) => Promise<BusinessWorkspacePrefillPreview> = async (_request) => {
    throw new Error("previewBusinessWorkspacePrefillImpl is not configured");
  };
  executeImpl: (command: CommandEnvelope) => Promise<CommandResponse> = async (
    command,
  ) => {
    const resultProject =
      command.commandType === "project.create"
        ? project(1)
        : project((command.expectedRevision ?? 0) + 1);
    return {
      receipt: {
        commandId: command.commandId,
        idempotencyKey: command.idempotencyKey,
        commandType: command.commandType,
        aggregateId: resultProject.id,
        revision: resultProject.revision,
        lastEventSequence: resultProject.revision,
        completedAt: 1,
      },
      project: resultProject,
      replayed: false,
    };
  };

  async executeCommand(command: CommandEnvelope): Promise<CommandResponse> {
    this.commands.push(command);
    return this.executeImpl(command);
  }

  listProjects(): Promise<ProjectRecord[]> {
    return this.listProjectsImpl();
  }

  async replayEvents(afterSequence: number, limit: number): Promise<DomainEvent[]> {
    this.replayCalls.push([afterSequence, limit]);
    return this.events
      .filter(({ sequence }) => sequence > afterSequence)
      .slice(0, limit);
  }

  async executeTaskCommand(
    command: TaskCommandEnvelope,
  ): Promise<TaskCommandResponse> {
    this.taskCommands.push(command);
    const revision = command.expectedRevision === null ? 1 : command.expectedRevision + 1;
    const status = command.commandType === "task.cancel" ? "canceled" : "queued";
    const record = {
      ...task(revision, status),
      projectId: command.context.projectId,
      kind:
        command.commandType === "task.create"
          ? command.payload.kind
          : task().kind,
    };
    return {
      receipt: receiptFor(command, record.id, revision),
      task: record,
      replayed: false,
    };
  }

  listTasks(): Promise<TaskRecord[]> {
    return this.listTasksImpl();
  }

  async replayTaskEvents(
    afterSequence: number,
    limit: number,
  ): Promise<TaskDomainEvent[]> {
    this.taskReplayCalls.push([afterSequence, limit]);
    return this.taskEvents
      .filter(({ sequence }) => sequence > afterSequence)
      .slice(0, limit);
  }

  selectAssetSource(): Promise<AssetSourceSelection | null> {
    return Promise.resolve({
      sourceToken: "source-token",
      displayName: "reference.png",
      detectedKind: "image",
      sizeBytes: 128,
    });
  }

  async executeAssetCommand(
    command: AssetCommandEnvelope,
  ): Promise<AssetCommandResponse> {
    this.assetCommands.push(command);
    const record = { ...asset(), projectId: command.payload.projectId };
    return {
      receipt: receiptFor(command, record.id, record.revision),
      asset: record,
      replayed: false,
    };
  }

  listAssets(): Promise<AssetRecord[]> {
    return this.listAssetsImpl();
  }

  async replayAssetEvents(
    afterSequence: number,
    limit: number,
  ): Promise<AssetDomainEvent[]> {
    this.assetReplayCalls.push([afterSequence, limit]);
    return this.assetEvents
      .filter(({ sequence }) => sequence > afterSequence)
      .slice(0, limit);
  }

  startBrainThread(_request: StartBrainThreadRequest): Promise<BrainThreadRecord> {
    return Promise.reject(new Error("brain thread start not configured"));
  }

  resumeBrainThread(_request: ResumeBrainThreadRequest): Promise<BrainThreadRecord> {
    return Promise.reject(new Error("brain thread resume not configured"));
  }

  listRemoteBrainThreads(
    _request: ListRemoteBrainThreadsRequest,
  ): Promise<RemoteBrainThreadPage> {
    return Promise.resolve({ threads: [], nextCursor: null });
  }

  startBrainTurn(_request: StartBrainTurnRequest): Promise<BrainTurnStartResult> {
    return Promise.reject(new Error("brain turn start not configured"));
  }

  interruptBrainTurn(_request: InterruptBrainTurnRequest): Promise<BrainTurnRecord> {
    return Promise.reject(new Error("brain turn interrupt not configured"));
  }

  listLocalBrainThreads(_projectId: string | null): Promise<BrainThreadRecord[]> {
    return Promise.resolve([]);
  }

  listLocalBrainTurns(_threadId: string): Promise<BrainTurnRecord[]> {
    return Promise.resolve([]);
  }

  getBrainHealth(): Promise<BrainHostHealth> {
    return Promise.resolve({
      state: "ready",
      running: true,
      initialized: true,
      pendingRequests: 0,
      subscribers: 1,
      startedAt: 1,
      lastMessageAt: 1,
      lastErrorCode: null,
    });
  }

  getNativeMediaHealth(): Promise<NativeMediaHealth> {
    return Promise.resolve({
      state: "unavailable",
      ffmpegAvailable: false,
      ffprobeAvailable: false,
      ffmpegSource: null,
      ffprobeSource: null,
    });
  }

  executeCaseImpl: (command: CaseCommandEnvelope) => Promise<CaseCommandResponse> =
    async (command) => {
      const revision =
        command.expectedRevision === null ? 1 : command.expectedRevision + 1;
      const current = this.cases.find(({ id }) => id === "case-1");
      let record: CaseRecord;
      if (command.commandType === "case.create") {
        record = {
          ...caseRecord(revision, command.payload.projectId),
          ...command.payload,
        };
      } else {
        const { caseId, ...changes } = command.payload;
        record = {
          ...caseRecord(revision, command.context.projectId),
          ...current,
          ...changes,
          id: caseId,
          revision,
          updatedAt: revision,
        };
      }
      this.cases = [record];
      return {
        receipt: receiptFor(command, record.id, revision),
        caseRecord: record,
        replayed: false,
      };
    };

  executeCaseCommand(command: CaseCommandEnvelope): Promise<CaseCommandResponse> {
    this.caseCommands.push(command);
    return this.executeCaseImpl(command);
  }

  listCases(): Promise<CaseRecord[]> {
    return this.listCasesImpl();
  }

  replayCaseEvents(
    afterSequence: number,
    limit: number,
  ): Promise<CaseDomainEvent[]> {
    this.caseReplayCalls.push([afterSequence, limit]);
    return Promise.resolve(
      this.caseEvents
        .filter(({ sequence }) => sequence > afterSequence)
        .slice(0, limit),
    );
  }

  executeExecutionBriefImpl: (
    command: ExecutionBriefCommandEnvelope,
  ) => Promise<ExecutionBriefCommandResponse> = async (command) => {
    const revision =
      command.expectedRevision === null ? 1 : command.expectedRevision + 1;
    const current = this.executionBriefs.find(({ id }) => id === "brief-1");
    let record = executionBriefRecord(
      revision,
      command.context.projectId ?? "project-1",
    );
    if (command.commandType === "executionBrief.create") {
      record = {
        ...record,
        projectId: command.payload.projectId,
        content: command.payload.content,
      };
    } else if (command.commandType === "executionBrief.update") {
      record = {
        ...record,
        ...current,
        id: command.payload.briefId,
        content: command.payload.content,
        revision,
        updatedAt: revision,
      };
    } else {
      record = {
        ...record,
        ...current,
        id: command.payload.briefId,
        status: command.payload.status,
        revision,
        updatedAt: revision,
      };
    }
    this.executionBriefs = [record];
    return {
      receipt: receiptFor(command, record.id, revision),
      executionBrief: record,
      replayed: false,
    };
  };

  executeExecutionBriefCommand(
    command: ExecutionBriefCommandEnvelope,
  ): Promise<ExecutionBriefCommandResponse> {
    this.executionBriefCommands.push(command);
    return this.executeExecutionBriefImpl(command);
  }

  listExecutionBriefs(): Promise<ExecutionBriefRecord[]> {
    return this.listExecutionBriefsImpl();
  }

  replayExecutionBriefEvents(
    afterSequence: number,
    limit: number,
  ): Promise<ExecutionBriefDomainEvent[]> {
    this.executionBriefReplayCalls.push([afterSequence, limit]);
    return Promise.resolve(
      this.executionBriefEvents
        .filter(({ sequence }) => sequence > afterSequence)
        .slice(0, limit),
    );
  }

  executeRequirementBriefImpl: (
    command: RequirementBriefCommandEnvelope,
  ) => Promise<RequirementBriefCommandResponse> = async (command) => {
    const revision =
      command.expectedRevision === null ? 1 : command.expectedRevision + 1;
    const current = this.requirementBriefs.find(
      ({ id }) => id === "requirement-1",
    );
    let record = requirementBriefRecord(
      revision,
      command.context.projectId ?? "project-1",
    );
    if (command.commandType === "requirementBrief.create") {
      record = {
        ...record,
        projectId: command.payload.projectId,
      };
    } else if (command.commandType === "requirementBrief.update") {
      record = {
        ...record,
        ...current,
        id: command.payload.briefId,
        content: command.payload.content,
        revision,
        updatedAt: revision,
      };
    } else {
      record = {
        ...record,
        ...current,
        id: command.payload.briefId,
        status: command.payload.status,
        confirmedAt:
          command.payload.status === "confirmed" ? revision : null,
        confirmedBy:
          command.payload.status === "confirmed"
            ? command.context.actorId
            : null,
        revision,
        updatedAt: revision,
      };
    }
    this.requirementBriefs = [record];
    return {
      receipt: receiptFor(command, record.id, revision),
      requirementBrief: record,
      replayed: false,
    };
  };

  executeRequirementBriefCommand(
    command: RequirementBriefCommandEnvelope,
  ): Promise<RequirementBriefCommandResponse> {
    this.requirementBriefCommands.push(command);
    return this.executeRequirementBriefImpl(command);
  }

  listRequirementBriefs(): Promise<RequirementBriefRecord[]> {
    return this.listRequirementBriefsImpl();
  }

  replayRequirementBriefEvents(
    afterSequence: number,
    limit: number,
  ): Promise<RequirementBriefDomainEvent[]> {
    this.requirementBriefReplayCalls.push([afterSequence, limit]);
    return Promise.resolve(
      this.requirementBriefEvents
        .filter(({ sequence }) => sequence > afterSequence)
        .slice(0, limit),
    );
  }

  executeBusinessWorkspaceImpl: (
    command: BusinessWorkspaceCommandEnvelope,
  ) => Promise<BusinessWorkspaceCommandResponse> = async (command) => {
    const revision =
      command.expectedRevision === null ? 1 : command.expectedRevision + 1;
    const current = this.businessWorkspaces.find(
      ({ id }) => id === "workspace-1",
    );
    let record: BusinessWorkspaceRecord = {
      ...businessWorkspaceRecord(
        revision,
        command.context.projectId ?? "project-1",
      ),
      ...current,
      revision,
      updatedAt: revision,
    };
    if (command.commandType === "businessWorkspace.create") {
      record = {
        ...record,
        projectId: command.payload.projectId,
        prefillSourceWorkspaceId: command.payload.prefillSourceWorkspaceId,
      };
    } else if (command.commandType === "businessWorkspace.updateProfile") {
      record = {
        ...record,
        profile: {
          ...record.profile,
          ...command.payload.profile,
          lineItems: record.profile.lineItems,
        },
      };
    } else if (command.commandType === "businessWorkspace.upsertPayment") {
      const currentPayment =
        command.payload.payment.id === null
          ? undefined
          : record.payments.find(({ id }) => id === command.payload.payment.id);
      const payment: BusinessWorkspaceRecord["payments"][number] = {
        ...command.payload.payment,
        id: command.payload.payment.id ?? "payment-1",
        revision: (currentPayment?.revision ?? 0) + 1,
        createdAt: currentPayment?.createdAt ?? revision,
        updatedAt: revision,
      };
      record = {
        ...record,
        payments: [
          ...record.payments.filter(({ id }) => id !== payment.id),
          payment,
        ],
      };
    } else if (command.commandType === "businessWorkspace.createDocument") {
      const payment =
        command.payload.paymentId === null
          ? null
          : (record.payments.find(
              ({ id }) => id === command.payload.paymentId,
            ) ?? null);
      const document: BusinessWorkspaceRecord["documents"][number] = {
        id: "document-1",
        kind: command.payload.kind,
        sequenceNumber:
          record.documents.filter(({ kind }) => kind === command.payload.kind)
            .length + 1,
        documentNumber: command.payload.documentNumber,
        title: command.payload.title,
        templateKey: command.payload.templateKey,
        status: "draft",
        snapshot: {
          workspaceRevision: command.expectedRevision ?? record.revision,
          customerId: record.customerId,
          customer: { ...record.customer },
          profile: {
            ...record.profile,
            lineItems: record.profile.lineItems.map((lineItem) => ({ ...lineItem })),
          },
          payment: payment === null ? null : { ...payment },
        },
        sourceAssetId: null,
        reviewId: null,
        reportAssetId: null,
        evidence: null,
        manualWaiver: null,
        voidedAt: null,
        voidedBy: null,
        voidReason: "",
        outputAssetId: null,
        outputFormat: null,
        approvedAt: null,
        approvedBy: null,
        generatedAt: null,
        revision: 1,
        createdAt: revision,
        updatedAt: revision,
      };
      record = {
        ...record,
        documents: [...record.documents, document],
      };
    } else if (command.commandType === "businessWorkspace.changeStatus") {
      record = {
        ...record,
        status: command.payload.status,
      };
    }
    this.businessWorkspaces = [record];
    return {
      receipt: receiptFor(command, record.id, revision),
      businessWorkspace: record,
      replayed: false,
    };
  };

  executeBusinessWorkspaceCommand(
    command: BusinessWorkspaceCommandEnvelope,
  ): Promise<BusinessWorkspaceCommandResponse> {
    this.businessWorkspaceCommands.push(command);
    return this.executeBusinessWorkspaceImpl(command);
  }

  listBusinessWorkspaces(): Promise<BusinessWorkspaceRecord[]> {
    return this.listBusinessWorkspacesImpl();
  }

  listBusinessCustomers(
    request: ListBusinessCustomersRequest,
  ): Promise<BusinessCustomerReceivableSummary[]> {
    this.businessCustomerListRequests.push(request);
    return this.listBusinessCustomersImpl(request);
  }

  listBusinessWorkspacePrefillCandidates(
    request: ListBusinessWorkspacePrefillCandidatesRequest,
  ): Promise<BusinessWorkspacePrefillCandidate[]> {
    this.businessWorkspacePrefillCandidateRequests.push(request);
    return this.listBusinessWorkspacePrefillCandidatesImpl(request);
  }

  previewBusinessWorkspacePrefill(
    request: PreviewBusinessWorkspacePrefillRequest,
  ): Promise<BusinessWorkspacePrefillPreview> {
    this.businessWorkspacePrefillPreviewRequests.push(request);
    return this.previewBusinessWorkspacePrefillImpl(request);
  }

  replayBusinessWorkspaceEvents(
    afterSequence: number,
    limit: number,
  ): Promise<BusinessWorkspaceDomainEvent[]> {
    this.businessWorkspaceReplayCalls.push([afterSequence, limit]);
    return Promise.resolve(
      this.businessWorkspaceEvents
        .filter(({ sequence }) => sequence > afterSequence)
        .slice(0, limit),
    );
  }

  async executeContractReviewCommand(
    command: ContractReviewCommandEnvelope,
  ): Promise<ContractReviewCommandResponse> {
    this.contractReviewCommands.push(command);
    const revision =
      command.expectedRevision === null ? 1 : command.expectedRevision + 1;
    const reviewId =
      "reviewId" in command.payload ? command.payload.reviewId : "review-1";
    const record: ContractReviewRecord = {
      ...CONTRACT_REVIEW,
      session: {
        ...CONTRACT_REVIEW.session,
        id: reviewId,
        workspaceId:
          command.commandType === "contractReview.create"
            ? command.payload.workspaceId
            : CONTRACT_REVIEW.session.workspaceId,
        sourceAssetId:
          command.commandType === "contractReview.create"
            ? command.payload.sourceAssetId
            : CONTRACT_REVIEW.session.sourceAssetId,
        revision,
        updatedAt: revision,
      },
    };
    this.contractReviews = [record];
    return {
      receipt: receiptFor(command, reviewId, revision),
      contractReview: record,
      replayed: false,
    };
  }

  listContractReviews(
    request: ListContractReviewsRequest,
  ): Promise<ContractReviewRecord[]> {
    this.contractReviewListRequests.push(request);
    return Promise.resolve(this.contractReviews);
  }

  getContractReview(
    request: GetContractReviewRequest,
  ): Promise<ContractReviewRecord> {
    this.contractReviewGetRequests.push(request);
    return Promise.resolve(
      this.contractReviews.find(({ session }) => session.id === request.reviewId) ??
        CONTRACT_REVIEW,
    );
  }

  listReviewFindings(
    request: ListReviewFindingsRequest,
  ): Promise<ReviewFindingRecord[]> {
    this.reviewFindingListRequests.push(request);
    return Promise.resolve(this.reviewFindings);
  }

  getEvidenceContext(
    request: GetEvidenceContextRequest,
  ): Promise<EvidenceContext> {
    this.evidenceContextGetRequests.push(request);
    return Promise.resolve(this.evidenceContext);
  }

  replayContractReviewEvents(
    afterSequence: number,
    limit: number,
  ): Promise<ContractReviewDomainEvent[]> {
    this.contractReviewReplayCalls.push([afterSequence, limit]);
    return Promise.resolve(
      this.contractReviewEvents
        .filter(({ sequence }) => sequence > afterSequence)
        .slice(0, limit),
    );
  }

  async executeAiCredentialCommand(
    command: AiCredentialCommandEnvelope,
  ): Promise<AiCredentialCommandResponse> {
    this.aiCredentialCommands.push(command);
    const revision =
      command.expectedRevision === null
        ? AI_CREDENTIAL_STATUS.revision
        : command.expectedRevision + 1;
    const configured =
      command.commandType === "aiCredentials.status"
        ? AI_CREDENTIAL_STATUS.configured
        : command.commandType === "aiCredentials.saveBsaigcApiKey";
    const status: AiCredentialStatus = {
      ...AI_CREDENTIAL_STATUS,
      configured,
      persisted: configured,
      revision,
      updatedAt: configured ? 1_721_600_000 : null,
    };
    return {
      receipt: receiptFor(command, "bsaigc", revision),
      status,
      connectionTest:
        command.commandType === "aiCredentials.testProvider"
          ? {
              state: "warning",
              message: "??????????????",
              latencyMs: null,
              testedAt: 1_753_158_600_000,
              discoveredModels: [],
            }
          : command.commandType === "aiCredentials.discoverModels"
          ? {
              state: "ready",
              message: "连接成功，已拉取 2 个模型",
              latencyMs: 42,
              testedAt: 1_753_158_600_000,
              discoveredModels: ["gpt-5.6-mini", "gpt-5.6-sol"],
            }
          : null,
      replayed: false,
    };
  }

  async executeDesktopSettingsCommand(
    command: DesktopSettingsCommandEnvelope,
  ): Promise<DesktopSettingsCommandResponse> {
    this.desktopSettingsCommands.push(command);
    const mutatesRevision =
      command.commandType === "settings.clearCache" ||
      command.commandType === "settings.checkForUpdates";
    const revision = mutatesRevision
      ? (command.expectedRevision ?? this.desktopSettingsSnapshot.revision) + 1
      : this.desktopSettingsSnapshot.revision;
    const storage =
      command.commandType === "settings.clearCache"
        ? {
            ...this.desktopSettingsSnapshot.storage,
            cacheBytes: 0,
            totalBytes:
              this.desktopSettingsSnapshot.storage.totalBytes -
              this.desktopSettingsSnapshot.storage.cacheBytes,
            locations: this.desktopSettingsSnapshot.storage.locations.map(
              (location) =>
                location.target === "cache"
                  ? { ...location, sizeBytes: 0 }
                  : location,
            ),
          }
        : this.desktopSettingsSnapshot.storage;
    const update =
      command.commandType === "settings.checkForUpdates"
        ? { ...this.desktopSettingsSnapshot.update, lastCheckedAt: 1_753_158_600_000 }
        : this.desktopSettingsSnapshot.update;
    this.desktopSettingsSnapshot = {
      ...this.desktopSettingsSnapshot,
      storage,
      update,
      revision,
    };
    return {
      receipt: receiptFor(command, "desktop-settings", revision),
      snapshot: this.desktopSettingsSnapshot,
      cacheClear:
        command.commandType === "settings.clearCache"
          ? { freedBytes: 1_048_576, clearedLocations: ["cache"] }
          : null,
      replayed: false,
    };
  }

  async executeBackupCommand(
    command: BackupCommandEnvelope,
  ): Promise<BackupCommandResponse> {
    this.backupCommands.push(command);
    const revision =
      command.expectedRevision === null ? 1 : command.expectedRevision + 1;
    const record: AssetBackupRecord = {
      ...ASSET_BACKUP,
      assetId: command.payload.assetId,
      revision,
      updatedAt: revision,
    };
    this.assetBackups = [record];
    return {
      receipt: receiptFor(command, record.assetId, revision),
      backup: record,
      replayed: false,
    };
  }

  listAssetBackups(limit: number): Promise<AssetBackupRecord[]> {
    this.assetBackupListLimits.push(limit);
    return Promise.resolve(this.assetBackups.slice(0, limit));
  }

  replayBackupEvents(
    afterSequence: number,
    limit: number,
  ): Promise<BackupDomainEvent[]> {
    this.backupReplayCalls.push([afterSequence, limit]);
    return Promise.resolve(
      this.backupEvents
        .filter(({ sequence }) => sequence > afterSequence)
        .slice(0, limit),
    );
  }

  getHostStatus(): Promise<HostStatus> {
    return Promise.resolve(HOST_STATUS);
  }

  listPendingApprovals(): Promise<ApprovalRecord[]> {
    return Promise.resolve([]);
  }

  resolveApproval(_payload: ResolveApprovalPayload): Promise<ApprovalRecord> {
    return Promise.reject(new Error("no approval in fake host"));
  }

  probeCodex(): Promise<CodexProbeStatus> {
    return Promise.resolve(CODEX_STATUS);
  }

  subscribeDomainEvents(listener: DomainEventListener): Promise<Unsubscribe> {
    this.listener = listener;
    return Promise.resolve(() => {
      this.unsubscribed = true;
      if (this.listener === listener) {
        this.listener = null;
      }
    });
  }

  subscribeTaskEvents(listener: TaskEventListener): Promise<Unsubscribe> {
    this.taskListener = listener;
    return Promise.resolve(() => {
      this.unsubscribed = true;
      if (this.taskListener === listener) this.taskListener = null;
    });
  }

  subscribeAssetEvents(listener: AssetEventListener): Promise<Unsubscribe> {
    this.assetListener = listener;
    return Promise.resolve(() => {
      this.unsubscribed = true;
      if (this.assetListener === listener) this.assetListener = null;
    });
  }

  subscribeBrainEvents(listener: BrainEventListener): Promise<Unsubscribe> {
    this.brainListener = listener;
    return Promise.resolve(() => {
      this.unsubscribed = true;
      if (this.brainListener === listener) this.brainListener = null;
    });
  }

  subscribeCaseEvents(listener: CaseEventListener): Promise<Unsubscribe> {
    this.caseListener = listener;
    return Promise.resolve(() => {
      this.unsubscribed = true;
      if (this.caseListener === listener) this.caseListener = null;
    });
  }

  subscribeExecutionBriefEvents(
    listener: ExecutionBriefEventListener,
  ): Promise<Unsubscribe> {
    this.executionBriefListener = listener;
    return Promise.resolve(() => {
      this.unsubscribed = true;
      if (this.executionBriefListener === listener) {
        this.executionBriefListener = null;
      }
    });
  }

  subscribeRequirementBriefEvents(
    listener: RequirementBriefEventListener,
  ): Promise<Unsubscribe> {
    this.requirementBriefListener = listener;
    return Promise.resolve(() => {
      this.unsubscribed = true;
      if (this.requirementBriefListener === listener) {
        this.requirementBriefListener = null;
      }
    });
  }

  subscribeBusinessWorkspaceEvents(
    listener: BusinessWorkspaceEventListener,
  ): Promise<Unsubscribe> {
    this.businessWorkspaceListener = listener;
    return Promise.resolve(() => {
      this.unsubscribed = true;
      if (this.businessWorkspaceListener === listener) {
        this.businessWorkspaceListener = null;
      }
    });
  }

  subscribeContractReviewEvents(
    listener: ContractReviewEventListener,
  ): Promise<Unsubscribe> {
    this.contractReviewListener = listener;
    return Promise.resolve(() => {
      this.unsubscribed = true;
      if (this.contractReviewListener === listener) {
        this.contractReviewListener = null;
      }
    });
  }

  subscribeBackupEvents(listener: BackupEventListener): Promise<Unsubscribe> {
    this.backupListener = listener;
    return Promise.resolve(() => {
      this.unsubscribed = true;
      if (this.backupListener === listener) {
        this.backupListener = null;
      }
    });
  }

  emit(value: DomainEvent): void {
    this.listener?.(value);
  }

  emitTask(value: TaskDomainEvent): void {
    this.taskListener?.(value);
  }

  emitAsset(value: AssetDomainEvent): void {
    this.assetListener?.(value);
  }

  emitCase(value: CaseDomainEvent): void {
    this.caseListener?.(value);
  }

  emitExecutionBrief(value: ExecutionBriefDomainEvent): void {
    this.executionBriefListener?.(value);
  }

  emitRequirementBrief(value: RequirementBriefDomainEvent): void {
    this.requirementBriefListener?.(value);
  }

  emitBusinessWorkspace(value: BusinessWorkspaceDomainEvent): void {
    this.businessWorkspaceListener?.(value);
  }

  emitContractReview(value: ContractReviewDomainEvent): void {
    this.contractReviewListener?.(value);
  }

  emitBackup(value: BackupDomainEvent): void {
    this.backupListener?.(value);
  }
}

function receiptFor(
  command:
    | TaskCommandEnvelope
    | AssetCommandEnvelope
    | CaseCommandEnvelope
    | ExecutionBriefCommandEnvelope
    | RequirementBriefCommandEnvelope
    | BusinessWorkspaceCommandEnvelope
    | ContractReviewCommandEnvelope
    | BackupCommandEnvelope
    | AiCredentialCommandEnvelope
    | DesktopSettingsCommandEnvelope,
  aggregateId: string,
  revision: number,
) {
  return {
    commandId: command.commandId,
    idempotencyKey: command.idempotencyKey,
    commandType: command.commandType,
    aggregateId,
    revision,
    lastEventSequence: revision,
    completedAt: 1,
  };
}

function deferred<T>(): {
  promise: Promise<T>;
  resolve: (value: T) => void;
} {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolver) => {
    resolve = resolver;
  });
  return { promise, resolve };
}

describe("BsaigcClient", () => {
  it("executes typed AI credential commands without exposing the key in snapshots", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host, { actorId: "operator-1", windowId: "main" });
    const secret = "sk-bsaigc-secret";

    const initial = await client.getAiCredentialStatus();
    expect(initial.configured).toBe(false);
    expect(host.aiCredentialCommands[0]).toMatchObject({
      commandType: "aiCredentials.status",
      expectedRevision: null,
    });

    const saved = await client.saveBsaigcApiKey(`  ${secret}  `, 0);
    expect(saved).toMatchObject({ configured: true, revision: 1 });
    expect(host.aiCredentialCommands[1]).toMatchObject({
      commandType: "aiCredentials.saveBsaigcApiKey",
      expectedRevision: 0,
      payload: { apiKey: secret },
    });
    expect(JSON.stringify(client.getSnapshot())).not.toContain(secret);

    const cleared = await client.clearBsaigcApiKey(1);
    expect(cleared).toMatchObject({ configured: false, revision: 2 });
    expect(host.aiCredentialCommands[2]).toMatchObject({
      commandType: "aiCredentials.clearBsaigcApiKey",
      expectedRevision: 1,
    });
  });

  it("routes multi-provider settings through typed Host commands without retaining secrets", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host, {
      actorId: "operator-1",
      windowId: "settings",
    });
    const secret = "sk-provider-test-only";

    await client.upsertProvider(
      {
        providerId: null,
        name: "  Internal Gateway  ",
        kind: "openAiCompatible",
        baseUrl: "https://gateway.example.com/v1/",
        apiKey: `  ${secret}  `,
        models: ["gpt-4.1", "gpt-4.1", " gpt-4.1-mini "],
        defaultModel: "gpt-4.1",
        setDefault: true,
        enabled: true,
      },
      0,
    );
    await client.selectProvider(" provider-1 ", " gpt-4.1-mini ", 1);
    const tested = await client.testProvider("provider-1", 2);
    await client.clearProviderApiKey("provider-1", 3);
    await client.removeProvider("provider-1", 4);

    expect(host.aiCredentialCommands).toHaveLength(5);
    expect(host.aiCredentialCommands[0]).toMatchObject({
      commandType: "aiCredentials.upsertProvider",
      expectedRevision: 0,
      payload: {
        providerId: null,
        name: "Internal Gateway",
        kind: "openAiCompatible",
        baseUrl: "https://gateway.example.com/v1",
        apiKey: secret,
        models: ["gpt-4.1", "gpt-4.1-mini"],
        defaultModel: "gpt-4.1",
        setDefault: true,
        enabled: true,
      },
    });
    expect(host.aiCredentialCommands[1]).toMatchObject({
      commandType: "aiCredentials.selectProvider",
      expectedRevision: 1,
      payload: { providerId: "provider-1", model: "gpt-4.1-mini" },
    });
    expect(host.aiCredentialCommands[2]).toMatchObject({
      commandType: "aiCredentials.testProvider",
      expectedRevision: 2,
      payload: { providerId: "provider-1" },
    });
    expect(tested.connectionTest).toMatchObject({
      state: "warning",
      message: "??????????????",
    });
    expect(host.aiCredentialCommands[3]).toMatchObject({
      commandType: "aiCredentials.clearProviderApiKey",
      expectedRevision: 3,
      payload: { providerId: "provider-1" },
    });
    expect(host.aiCredentialCommands[4]).toMatchObject({
      commandType: "aiCredentials.removeProvider",
      expectedRevision: 4,
      payload: { providerId: "provider-1" },
    });
    expect(JSON.stringify(client.getSnapshot())).not.toContain(secret);
  });

  it("normalizes draft model discovery without storing the API key in client state", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host, {
      actorId: "operator-1",
      windowId: "settings",
    });
    const secret = "sk-discovery-only-secret";

    const discovered = await client.discoverProviderModels(
      {
        providerId: null,
        kind: "openAiCompatible",
        baseUrl: " https://gateway.example.com/v1/ ",
        apiKey: `  ${secret}  `,
      },
      0,
    );

    expect(host.aiCredentialCommands[0]).toMatchObject({
      commandType: "aiCredentials.discoverModels",
      expectedRevision: 0,
      payload: {
        providerId: null,
        kind: "openAiCompatible",
        baseUrl: "https://gateway.example.com/v1",
        apiKey: secret,
      },
    });
    expect(discovered.connectionTest).toMatchObject({
      state: "ready",
      discoveredModels: ["gpt-5.6-mini", "gpt-5.6-sol"],
    });
    expect(JSON.stringify(client.getSnapshot())).not.toContain(secret);
  });

  it("routes desktop settings through typed commands and exposes capability paths only", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host, {
      actorId: "operator-1",
      windowId: "settings",
    });

    const initial = await client.getDesktopSettingsStatus();
    await client.openStorageLocation("vault", initial.revision);
    const cleared = await client.clearCache(initial.revision);
    const updated = await client.checkForUpdates(cleared.snapshot.revision);

    expect(host.desktopSettingsCommands.map(({ commandType }) => commandType)).toEqual([
      "settings.status",
      "settings.openStorageLocation",
      "settings.clearCache",
      "settings.checkForUpdates",
    ]);
    expect(host.desktopSettingsCommands[1]).toMatchObject({
      payload: { target: "vault" },
      expectedRevision: 0,
    });
    expect(cleared.cacheClear).toEqual({
      freedBytes: 1_048_576,
      clearedLocations: ["cache"],
    });
    expect(updated).toMatchObject({
      revision: 2,
      update: { lastCheckedAt: 1_753_158_600_000 },
    });
    expect(JSON.stringify(updated)).not.toContain("C:\\\\");
    expect(
      updated.storage.locations.every(({ path }) =>
        path.startsWith("bsaigc-storage://"),
      ),
    ).toBe(true);
  });

  it("rejects unsafe storage paths returned by a Host adapter", async () => {
    const host = new FakeHostAdapter();
    host.desktopSettingsSnapshot = {
      ...DESKTOP_SETTINGS_SNAPSHOT,
      storage: {
        ...DESKTOP_SETTINGS_SNAPSHOT.storage,
        locations: DESKTOP_SETTINGS_SNAPSHOT.storage.locations.map((location) =>
          location.target === "vault"
            ? { ...location, path: "C:\\private\\vault" }
            : location,
        ),
      },
    };

    await expect(
      new BsaigcClient(host).getDesktopSettingsStatus(),
    ).rejects.toMatchObject({ code: "UNSAFE_STORAGE_PATH" });
  });

  it("rejects empty keys and invalid AI credential revisions", () => {
    const client = new BsaigcClient(new FakeHostAdapter());
    expect(() => client.saveBsaigcApiKey("   ", 0)).toThrow(
      "apiKey must not be empty",
    );
    expect(() => client.saveBsaigcApiKey("sk-test", -1)).toThrow();
    expect(() => client.saveBsaigcApiKey("sk-test", 1.5)).toThrow();
    expect(() => client.clearBsaigcApiKey(-1)).toThrow();
    expect(() => client.clearBsaigcApiKey(1.5)).toThrow();
  });

  it("replays events in 200-item pages and exposes a React-compatible snapshot", async () => {
    const host = new FakeHostAdapter();
    host.projects = [project(401)];
    host.events = Array.from({ length: 401 }, (_, index) => event(index + 1));
    const client = new BsaigcClient(host);
    const listener = vi.fn();
    client.subscribe(listener);

    await client.start();

    expect(host.replayCalls).toEqual([
      [0, 200],
      [200, 200],
      [400, 200],
    ]);
    expect(client.getSnapshot()).toMatchObject({
      started: true,
      synchronizing: false,
      lastSequence: 401,
    });
    expect(client.getSnapshot().events).toHaveLength(80);
    expect(client.getSnapshot().projects[0]?.revision).toBe(401);
    expect(listener).toHaveBeenCalled();
    expect(client.getSnapshot()).toBe(client.getSnapshot());
  });

  it("buffers live events before list/replay synchronization completes", async () => {
    const host = new FakeHostAdapter();
    const projectsGate = deferred<ProjectRecord[]>();
    host.listProjectsImpl = () => projectsGate.promise;
    host.events = [event(1, 1)];
    const client = new BsaigcClient(host);

    const starting = client.start();
    await Promise.resolve();
    host.emit(event(2, 2));
    projectsGate.resolve([project(1)]);
    await starting;

    expect(client.getSnapshot().projects[0]?.revision).toBe(2);
    expect(client.getSnapshot().events.map(({ sequence }) => sequence)).toEqual([
      1, 2,
    ]);
  });

  it("subscribes and buffers task and asset events during startup synchronization", async () => {
    const host = new FakeHostAdapter();
    const projectsGate = deferred<ProjectRecord[]>();
    const tasksGate = deferred<TaskRecord[]>();
    const assetsGate = deferred<AssetRecord[]>();
    host.listProjectsImpl = () => projectsGate.promise;
    host.listTasksImpl = () => tasksGate.promise;
    host.listAssetsImpl = () => assetsGate.promise;
    host.taskEvents = [taskEvent(1, 1)];
    host.assetEvents = [assetEvent(1, 1)];
    const client = new BsaigcClient(host);

    const starting = client.start();
    await vi.waitFor(() => {
      expect(host.listener).not.toBeNull();
      expect(host.taskListener).not.toBeNull();
      expect(host.assetListener).not.toBeNull();
    });
    host.emitTask(taskEvent(2, 2));
    host.emitAsset(assetEvent(2, 2));
    projectsGate.resolve([project(1)]);
    tasksGate.resolve([task(1)]);
    assetsGate.resolve([asset(1)]);
    await starting;

    expect(client.getSnapshot()).toMatchObject({
      taskLastSequence: 2,
      assetLastSequence: 2,
    });
    expect(client.getSnapshot().tasks[0]?.revision).toBe(2);
    expect(client.getSnapshot().assets[0]?.revision).toBe(2);
    expect(client.getSnapshot().taskEvents.map(({ sequence }) => sequence)).toEqual([1, 2]);
    expect(client.getSnapshot().assetEvents.map(({ sequence }) => sequence)).toEqual([1, 2]);
  });

  it("replays case pages and buffers live case events during startup", async () => {
    const host = new FakeHostAdapter();
    const casesGate = deferred<CaseRecord[]>();
    host.listCasesImpl = () => casesGate.promise;
    host.caseEvents = Array.from({ length: 201 }, (_, index) =>
      caseEvent(index + 1),
    );
    const client = new BsaigcClient(host);

    const starting = client.start();
    await vi.waitFor(() => expect(host.caseListener).not.toBeNull());
    host.emitCase(caseEvent(202));
    casesGate.resolve([caseRecord(1)]);
    await starting;

    expect(host.caseReplayCalls).toEqual([
      [0, 200],
      [200, 200],
    ]);
    expect(client.getSnapshot()).toMatchObject({
      started: true,
      synchronizing: false,
      caseLastSequence: 202,
    });
    expect(client.getSnapshot().cases[0]?.revision).toBe(202);
    expect(client.getSnapshot().caseEvents).toHaveLength(80);
    expect(client.getSnapshot().caseEvents[0]?.sequence).toBe(123);
    expect(client.getSnapshot().caseEvents[79]?.sequence).toBe(202);
  });

  it("exposes case replay failures as a stopped startup state", async () => {
    const host = new FakeHostAdapter();
    host.replayCaseEvents = async () => {
      throw {
        code: "CASE_REPLAY_UNAVAILABLE",
        message: "Case replay is unavailable",
        retryable: true,
      };
    };
    const client = new BsaigcClient(host);

    await expect(client.start()).rejects.toEqual({
      code: "CASE_REPLAY_UNAVAILABLE",
      message: "Case replay is unavailable",
      retryable: true,
    });
    expect(client.getSnapshot()).toMatchObject({
      started: false,
      synchronizing: false,
      error: {
        code: "CASE_REPLAY_UNAVAILABLE",
        message: "Case replay is unavailable",
        retryable: true,
      },
    });
    expect(host.unsubscribed).toBe(true);
    expect(host.caseListener).toBeNull();
  });

  it("replays from the contiguous cursor when a live case event skips a sequence", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();

    host.caseEvents = [caseEvent(1), caseEvent(2)];
    host.emitCase(caseEvent(2));

    await vi.waitFor(() => {
      expect(client.getSnapshot().caseLastSequence).toBe(2);
    });
    expect(host.caseReplayCalls).toEqual([
      [0, 200],
      [0, 200],
    ]);
    expect(client.getSnapshot().cases[0]?.revision).toBe(2);
    expect(client.getSnapshot().caseEvents.map(({ sequence }) => sequence)).toEqual([
      1, 2,
    ]);
  });

  it("builds command envelopes and merges command responses without an emitted event", async () => {
    const host = new FakeHostAdapter();
    const ids = [
      "trace-create",
      "command-create",
      "idem-create",
      "trace-update",
      "command-update",
      "idem-update",
      "trace-stage",
      "command-stage",
      "idem-stage",
    ];
    const client = new BsaigcClient(host, {
      actorId: "actor-1",
      accountId: "account-1",
      windowId: "window-1",
      now: () => 1_000,
      uuid: () => ids.shift() ?? "unexpected-id",
    });

    await client.createProject({ name: "Campaign", clientName: "ACME" });
    await client.updateProjectBrief("project-1", EMPTY_BRIEF, 1);
    await client.changeProjectStage("project-1", "creative", 2);

    expect(host.commands[0]).toEqual({
      commandType: "project.create",
      commandId: "command-create",
      protocolVersion: "1.5",
      context: {
        actorId: "actor-1",
        accountId: "account-1",
        projectId: null,
        windowId: "window-1",
        traceId: "trace-create",
      },
      payload: { name: "Campaign", clientName: "ACME" },
      idempotencyKey: "idem-create",
      expectedRevision: null,
      deadlineAt: 31_000,
    });
    expect(host.commands[1]).toMatchObject({
      commandType: "project.updateBrief",
      commandId: "command-update",
      expectedRevision: 1,
      context: { projectId: "project-1", traceId: "trace-update" },
    });
    expect(host.commands[2]).toMatchObject({
      commandType: "project.changeStage",
      commandId: "command-stage",
      expectedRevision: 2,
      payload: { projectId: "project-1", stage: "creative" },
    });
    expect(client.getSnapshot().projects[0]?.revision).toBe(3);
    expect(client.getSnapshot().events).toHaveLength(0);
  });

  it("builds pure JSON task and asset commands and merges their responses", async () => {
    const host = new FakeHostAdapter();
    const ids = [
      "task-create-trace",
      "task-create-command",
      "task-create-idem",
      "task-cancel-trace",
      "task-cancel-command",
      "task-cancel-idem",
      "task-retry-trace",
      "task-retry-command",
      "task-retry-idem",
      "asset-import-trace",
      "asset-import-command",
      "asset-import-idem",
    ];
    const client = new BsaigcClient(host, {
      actorId: "operator",
      windowId: "main",
      now: () => 5_000,
      uuid: () => ids.shift() ?? "unexpected-id",
    });

    await client.createTask({
      kind: "media.thumbnail",
      projectId: "project-1",
      input: { assetId: "asset-1" },
      priority: "high",
      replayPolicy: "safe",
      maxAttempts: 3,
      dependencyTaskIds: [],
    });
    await client.cancelTask("task-1", 1, "operator canceled");
    await client.retryTask("task-1", 2, true);
    await client.importAsset("source-token", "project-1");

    expect(host.taskCommands[0]).toEqual({
      commandType: "task.create",
      commandId: "task-create-command",
      protocolVersion: "1.5",
      context: {
        actorId: "operator",
        accountId: null,
        projectId: "project-1",
        windowId: "main",
        traceId: "task-create-trace",
      },
      payload: {
        kind: "media.thumbnail",
        projectId: "project-1",
        input: { assetId: "asset-1" },
        priority: "high",
        replayPolicy: "safe",
        maxAttempts: 3,
        dependencyTaskIds: [],
      },
      idempotencyKey: "task-create-idem",
      expectedRevision: null,
      deadlineAt: 35_000,
    });
    expect(host.taskCommands[1]).toMatchObject({
      commandType: "task.cancel",
      payload: { taskId: "task-1", reason: "operator canceled" },
      expectedRevision: 1,
      context: { projectId: "project-1" },
    });
    expect(host.taskCommands[2]).toMatchObject({
      commandType: "task.retry",
      payload: { taskId: "task-1", approved: true },
      expectedRevision: 2,
    });
    expect(host.assetCommands[0]).toMatchObject({
      commandType: "asset.import",
      payload: { sourceToken: "source-token", projectId: "project-1" },
      expectedRevision: null,
    });
    expect(JSON.stringify(host.assetCommands[0])).not.toMatch(/path|url/i);
    expect(client.getSnapshot().tasks[0]?.revision).toBe(3);
    expect(client.getSnapshot().assets[0]?.id).toBe("asset-1");
  });

  it("builds case create/update envelopes with ownership and concurrency context", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host, {
      actorId: "case-editor",
      accountId: "agency-1",
      windowId: "case-library",
      now: () => 10_000,
    });
    const createPayload = createCasePayload("project-owned");
    const updatePayload = updateCasePayload();

    await client.createCase(createPayload, {
      commandId: "case-create-command",
      traceId: "case-create-trace",
      idempotencyKey: "case-create-idem",
    });
    await client.updateCase(updatePayload, 7, {
      commandId: "case-update-command",
      traceId: "case-update-trace",
      idempotencyKey: "case-update-idem",
    });

    expect(host.caseCommands[0]).toEqual({
      commandType: "case.create",
      commandId: "case-create-command",
      protocolVersion: "1.5",
      context: {
        actorId: "case-editor",
        accountId: "agency-1",
        projectId: "project-owned",
        windowId: "case-library",
        traceId: "case-create-trace",
      },
      payload: createPayload,
      idempotencyKey: "case-create-idem",
      expectedRevision: null,
      deadlineAt: 40_000,
    });
    expect(host.caseCommands[1]).toEqual({
      commandType: "case.update",
      commandId: "case-update-command",
      protocolVersion: "1.5",
      context: {
        actorId: "case-editor",
        accountId: "agency-1",
        projectId: "project-owned",
        windowId: "case-library",
        traceId: "case-update-trace",
      },
      payload: updatePayload,
      idempotencyKey: "case-update-idem",
      expectedRevision: 7,
      deadlineAt: 40_000,
    });
    expect(client.getSnapshot().cases[0]).toMatchObject({
      id: "case-1",
      projectId: "project-owned",
      revision: 8,
    });
  });

  it("exposes case command failures and clears the error after recovery", async () => {
    const host = new FakeHostAdapter();
    const successfulExecute = host.executeCaseImpl;
    const client = new BsaigcClient(host);
    const explicitOptions = {
      commandId: "case-command",
      traceId: "case-trace",
      idempotencyKey: "case-idem",
    };
    await client.createCase(createCasePayload(), explicitOptions);
    host.executeCaseImpl = async () => {
      throw {
        code: "CASE_REVISION_CONFLICT",
        message: "Case revision is stale",
        retryable: true,
      };
    };

    await expect(
      client.updateCase(updateCasePayload(), 1, explicitOptions),
    ).rejects.toEqual({
      code: "CASE_REVISION_CONFLICT",
      message: "Case revision is stale",
      retryable: true,
    });
    expect(client.getSnapshot().error).toEqual({
      code: "CASE_REVISION_CONFLICT",
      message: "Case revision is stale",
      retryable: true,
    });
    expect(client.getSnapshot().cases[0]?.revision).toBe(1);

    host.executeCaseImpl = successfulExecute;
    await client.updateCase(updateCasePayload(), 1, explicitOptions);

    expect(client.getSnapshot().error).toBeNull();
    expect(client.getSnapshot().cases[0]?.revision).toBe(2);
    expect(() =>
      client.updateCase(updateCasePayload(), -1, explicitOptions),
    ).toThrow("expectedRevision must be a non-negative integer");
    expect(host.caseCommands).toHaveLength(3);
  });

  it("replays execution brief pages and buffers live events during startup", async () => {
    const host = new FakeHostAdapter();
    const executionBriefsGate = deferred<ExecutionBriefRecord[]>();
    host.listExecutionBriefsImpl = () => executionBriefsGate.promise;
    host.executionBriefEvents = Array.from({ length: 201 }, (_, index) =>
      executionBriefEvent(index + 1),
    );
    const client = new BsaigcClient(host);

    const starting = client.start();
    await vi.waitFor(() => expect(host.executionBriefListener).not.toBeNull());
    host.emitExecutionBrief(executionBriefEvent(202));
    executionBriefsGate.resolve([executionBriefRecord(1)]);
    await starting;

    expect(host.executionBriefReplayCalls).toEqual([
      [0, 200],
      [200, 200],
    ]);
    expect(client.getSnapshot()).toMatchObject({
      started: true,
      synchronizing: false,
      executionBriefLastSequence: 202,
    });
    expect(client.getSnapshot().executionBriefs[0]?.revision).toBe(202);
    expect(client.getSnapshot().executionBriefEvents).toHaveLength(80);
    expect(client.getSnapshot().executionBriefEvents[0]?.sequence).toBe(123);
    expect(client.getSnapshot().executionBriefEvents[79]?.sequence).toBe(202);
  });

  it("recovers a live execution brief gap from the contiguous cursor", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();

    host.executionBriefEvents = [
      executionBriefEvent(1),
      executionBriefEvent(2),
    ];
    host.emitExecutionBrief(executionBriefEvent(2));

    expect(client.getSnapshot()).toMatchObject({
      executionBriefs: [],
      executionBriefEvents: [],
      executionBriefLastSequence: 0,
    });

    await vi.waitFor(() => {
      expect(client.getSnapshot().executionBriefLastSequence).toBe(2);
    });
    expect(host.executionBriefReplayCalls).toEqual([
      [0, 200],
      [0, 200],
    ]);
    expect(client.getSnapshot().executionBriefs[0]?.revision).toBe(2);
    expect(
      client
        .getSnapshot()
        .executionBriefEvents.map(({ sequence }) => sequence),
    ).toEqual([1, 2]);
  });

  it("retries a failed execution brief gap replay without dropping a newer target", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();
    host.executionBriefEvents = [
      executionBriefEvent(1),
      executionBriefEvent(2),
      executionBriefEvent(3),
    ];

    let rejectFirstReplay!: (reason?: unknown) => void;
    const firstReplay = new Promise<ExecutionBriefDomainEvent[]>((_, reject) => {
      rejectFirstReplay = reject;
    });
    let replayAttempt = 0;
    const replayExecutionBriefEvents = vi.fn(
      (afterSequence: number, limit: number) => {
        replayAttempt += 1;
        if (replayAttempt === 1) return firstReplay;
        return Promise.resolve(
          host.executionBriefEvents
            .filter(({ sequence }) => sequence > afterSequence)
            .slice(0, limit),
        );
      },
    );
    host.replayExecutionBriefEvents = replayExecutionBriefEvents;

    host.emitExecutionBrief(executionBriefEvent(2));
    await vi.waitFor(() =>
      expect(replayExecutionBriefEvents).toHaveBeenCalledTimes(1),
    );
    host.emitExecutionBrief(executionBriefEvent(3));
    expect(client.getSnapshot()).toMatchObject({
      executionBriefs: [],
      executionBriefEvents: [],
      executionBriefLastSequence: 0,
    });

    rejectFirstReplay({
      code: "EXECUTION_BRIEF_REPLAY_UNAVAILABLE",
      message: "Execution brief replay is temporarily unavailable",
      retryable: true,
    });

    await vi.waitFor(() =>
      expect(client.getSnapshot().executionBriefLastSequence).toBe(3),
    );
    expect(replayExecutionBriefEvents.mock.calls).toEqual([
      [0, 200],
      [0, 200],
    ]);
    expect(
      client
        .getSnapshot()
        .executionBriefEvents.map(({ sequence }) => sequence),
    ).toEqual([1, 2, 3]);
    expect(client.getSnapshot().executionBriefs[0]?.revision).toBe(3);
    expect(client.getSnapshot().error).toBeNull();
  });

  it("bounds failed execution brief gap retries and converges on refresh", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();
    host.executionBriefEvents = [
      executionBriefEvent(1),
      executionBriefEvent(2),
    ];

    let replayAvailable = false;
    const replayExecutionBriefEvents = vi.fn(
      (afterSequence: number, limit: number) => {
        if (!replayAvailable) {
          return Promise.reject({
            code: "EXECUTION_BRIEF_REPLAY_UNAVAILABLE",
            message: "Execution brief replay is temporarily unavailable",
            retryable: true,
          });
        }
        return Promise.resolve(
          host.executionBriefEvents
            .filter(({ sequence }) => sequence > afterSequence)
            .slice(0, limit),
        );
      },
    );
    host.replayExecutionBriefEvents = replayExecutionBriefEvents;

    host.emitExecutionBrief(executionBriefEvent(2));

    await vi.waitFor(() => {
      expect(replayExecutionBriefEvents).toHaveBeenCalledTimes(3);
      expect(client.getSnapshot().error).toMatchObject({
        code: "EXECUTION_BRIEF_REPLAY_UNAVAILABLE",
        retryable: true,
      });
    });
    expect(client.getSnapshot()).toMatchObject({
      executionBriefs: [],
      executionBriefEvents: [],
      executionBriefLastSequence: 0,
    });

    replayAvailable = true;
    await expect(client.refreshExecutionBriefs()).resolves.toEqual([
      executionBriefRecord(2),
    ]);
    expect(replayExecutionBriefEvents).toHaveBeenCalledTimes(4);
    expect(client.getSnapshot().executionBriefLastSequence).toBe(2);
    expect(client.getSnapshot().executionBriefs[0]?.revision).toBe(2);
    expect(client.getSnapshot().error).toBeNull();
  });

  it("builds execution brief commands with project ownership and concurrency", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host, {
      actorId: "shoot-producer",
      accountId: "agency-1",
      windowId: "execution-brief",
      now: () => 10_000,
    });
    const createPayload = createExecutionBriefPayload("project-shoot");
    const updatePayload = updateExecutionBriefPayload();

    await client.createExecutionBrief(createPayload, {
      commandId: "brief-create-command",
      traceId: "brief-create-trace",
      idempotencyKey: "brief-create-idem",
    });
    await client.updateExecutionBrief(updatePayload, 7, {
      commandId: "brief-update-command",
      traceId: "brief-update-trace",
      idempotencyKey: "brief-update-idem",
    });
    await client.changeExecutionBriefStatus("brief-1", "ready", 8, {
      commandId: "brief-status-command",
      traceId: "brief-status-trace",
      idempotencyKey: "brief-status-idem",
    });

    expect(host.executionBriefCommands[0]).toEqual({
      commandType: "executionBrief.create",
      commandId: "brief-create-command",
      protocolVersion: "1.5",
      context: {
        actorId: "shoot-producer",
        accountId: "agency-1",
        projectId: "project-shoot",
        windowId: "execution-brief",
        traceId: "brief-create-trace",
      },
      payload: createPayload,
      idempotencyKey: "brief-create-idem",
      expectedRevision: null,
      deadlineAt: 40_000,
    });
    expect(host.executionBriefCommands[1]).toMatchObject({
      commandType: "executionBrief.update",
      context: {
        projectId: "project-shoot",
        traceId: "brief-update-trace",
      },
      payload: updatePayload,
      idempotencyKey: "brief-update-idem",
      expectedRevision: 7,
      deadlineAt: 40_000,
    });
    expect(host.executionBriefCommands[2]).toMatchObject({
      commandType: "executionBrief.changeStatus",
      context: {
        projectId: "project-shoot",
        traceId: "brief-status-trace",
      },
      payload: { briefId: "brief-1", status: "ready" },
      idempotencyKey: "brief-status-idem",
      expectedRevision: 8,
      deadlineAt: 40_000,
    });
    expect(client.getSnapshot().executionBriefs[0]).toMatchObject({
      id: "brief-1",
      projectId: "project-shoot",
      revision: 9,
      status: "ready",
    });

    host.executionBriefs = [executionBriefRecord(10, "project-shoot")];
    await expect(client.refreshExecutionBriefs()).resolves.toHaveLength(1);
    expect(client.getSnapshot().executionBriefs[0]?.revision).toBe(10);
    expect(host.executionBriefReplayCalls).toEqual([[0, 200]]);
    expect(() =>
      client.changeExecutionBriefStatus("brief-1", "ready", -1),
    ).toThrow("expectedRevision must be a positive integer");
  });

  it("rejects revision zero for execution brief update and status commands", () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);

    expect(() => client.updateExecutionBrief(updateExecutionBriefPayload(), 0)).toThrow(
      "expectedRevision must be a positive integer",
    );
    expect(() =>
      client.changeExecutionBriefStatus("brief-1", "ready", 0),
    ).toThrow("expectedRevision must be a positive integer");
    expect(host.executionBriefCommands).toHaveLength(0);
  });

  it("replays requirement brief pages and buffers live events during startup", async () => {
    const host = new FakeHostAdapter();
    const requirementBriefsGate = deferred<RequirementBriefRecord[]>();
    host.listRequirementBriefsImpl = () => requirementBriefsGate.promise;
    host.requirementBriefEvents = Array.from({ length: 201 }, (_, index) =>
      requirementBriefEvent(index + 1),
    );
    const client = new BsaigcClient(host);

    const starting = client.start();
    await vi.waitFor(() => expect(host.requirementBriefListener).not.toBeNull());
    host.emitRequirementBrief(requirementBriefEvent(202));
    requirementBriefsGate.resolve([requirementBriefRecord(1)]);
    await starting;

    expect(host.requirementBriefReplayCalls).toEqual([
      [0, 200],
      [200, 200],
    ]);
    expect(client.getSnapshot()).toMatchObject({
      started: true,
      synchronizing: false,
      requirementBriefLastSequence: 202,
    });
    expect(client.getSnapshot().requirementBriefs[0]?.revision).toBe(202);
    expect(client.getSnapshot().requirementBriefEvents).toHaveLength(80);
    expect(client.getSnapshot().requirementBriefEvents[0]?.sequence).toBe(123);
    expect(client.getSnapshot().requirementBriefEvents[79]?.sequence).toBe(202);
  });

  it("keeps a live requirement brief gap private until replay closes it", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();
    const snapshotListener = vi.fn();
    client.subscribe(snapshotListener);

    host.requirementBriefEvents = [
      requirementBriefEvent(1),
      requirementBriefEvent(2),
    ];
    host.emitRequirementBrief(requirementBriefEvent(2));

    expect(snapshotListener).not.toHaveBeenCalled();
    expect(client.getSnapshot()).toMatchObject({
      requirementBriefs: [],
      requirementBriefEvents: [],
      requirementBriefLastSequence: 0,
    });

    await vi.waitFor(() => {
      expect(client.getSnapshot().requirementBriefLastSequence).toBe(2);
    });
    expect(host.requirementBriefReplayCalls).toEqual([
      [0, 200],
      [0, 200],
    ]);
    expect(client.getSnapshot().requirementBriefs[0]?.revision).toBe(2);
    expect(
      client
        .getSnapshot()
        .requirementBriefEvents.map(({ sequence }) => sequence),
    ).toEqual([1, 2]);
  });

  it("bounds failed requirement brief gap retries and converges on refresh", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();
    host.requirementBriefEvents = [
      requirementBriefEvent(1),
      requirementBriefEvent(2),
    ];

    let replayAvailable = false;
    const replayRequirementBriefEvents = vi.fn(
      (afterSequence: number, limit: number) => {
        if (!replayAvailable) {
          return Promise.reject({
            code: "REQUIREMENT_BRIEF_REPLAY_UNAVAILABLE",
            message: "Requirement brief replay is temporarily unavailable",
            retryable: true,
          });
        }
        return Promise.resolve(
          host.requirementBriefEvents
            .filter(({ sequence }) => sequence > afterSequence)
            .slice(0, limit),
        );
      },
    );
    host.replayRequirementBriefEvents = replayRequirementBriefEvents;

    host.emitRequirementBrief(requirementBriefEvent(2));

    await vi.waitFor(() => {
      expect(replayRequirementBriefEvents).toHaveBeenCalledTimes(3);
      expect(client.getSnapshot().error).toMatchObject({
        code: "REQUIREMENT_BRIEF_REPLAY_UNAVAILABLE",
        retryable: true,
      });
    });
    expect(client.getSnapshot()).toMatchObject({
      requirementBriefs: [],
      requirementBriefEvents: [],
      requirementBriefLastSequence: 0,
    });

    replayAvailable = true;
    host.requirementBriefs = [requirementBriefRecord(2)];
    await expect(client.refreshRequirementBriefs()).resolves.toEqual([
      requirementBriefRecord(2),
    ]);
    expect(replayRequirementBriefEvents).toHaveBeenCalledTimes(4);
    expect(client.getSnapshot().requirementBriefLastSequence).toBe(2);
    expect(client.getSnapshot().requirementBriefs[0]?.revision).toBe(2);
    expect(client.getSnapshot().error).toBeNull();
  });

  it("keeps an exhausted requirement brief gap error until the missing event arrives late", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();
    const replayRequirementBriefEvents = vi.fn(() =>
      Promise.reject({
        code: "REQUIREMENT_BRIEF_REPLAY_UNAVAILABLE",
        message: "Requirement brief replay is temporarily unavailable",
        retryable: true,
      }),
    );
    host.replayRequirementBriefEvents = replayRequirementBriefEvents;

    host.emitRequirementBrief(requirementBriefEvent(2));

    await vi.waitFor(() => {
      expect(replayRequirementBriefEvents).toHaveBeenCalledTimes(3);
      expect(client.getSnapshot().error).toMatchObject({
        code: "REQUIREMENT_BRIEF_REPLAY_UNAVAILABLE",
      });
    });

    await expect(client.getHostStatus()).resolves.toEqual(HOST_STATUS);
    expect(client.getSnapshot().error).toMatchObject({
      code: "REQUIREMENT_BRIEF_REPLAY_UNAVAILABLE",
    });

    host.getHostStatus = () =>
      Promise.reject({
        code: "DB_BUSY",
        message: "Database is busy",
        retryable: true,
      });
    await expect(client.getHostStatus()).rejects.toMatchObject({
      code: "DB_BUSY",
    });
    // The freshest command failure takes priority over the latched gap error.
    expect(client.getSnapshot().error).toMatchObject({
      code: "DB_BUSY",
    });

    host.emitRequirementBrief(requirementBriefEvent(1));

    await vi.waitFor(() => {
      expect(client.getSnapshot()).toMatchObject({
        requirementBriefLastSequence: 2,
        error: { code: "DB_BUSY" },
      });
    });
    expect(
      client
        .getSnapshot()
        .requirementBriefEvents.map(({ sequence }) => sequence),
    ).toEqual([1, 2]);

    host.getHostStatus = () => Promise.resolve(HOST_STATUS);
    await expect(client.getHostStatus()).resolves.toEqual(HOST_STATUS);
    expect(client.getSnapshot().error).toBeNull();
  });

  it("replays requirement brief events up to a command receipt sequence", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();
    host.requirementBriefEvents = [
      requirementBriefEvent(1),
      requirementBriefEvent(2),
    ];
    const executeRequirementBrief = host.executeRequirementBriefImpl;
    host.executeRequirementBriefImpl = async (command) => {
      const response = await executeRequirementBrief(command);
      return {
        ...response,
        receipt: {
          ...response.receipt,
          lastEventSequence: 2,
        },
      };
    };

    await client.createRequirementBrief(
      createRequirementBriefPayload("project-intake"),
    );

    await vi.waitFor(() => {
      expect(client.getSnapshot().requirementBriefLastSequence).toBe(2);
    });
    expect(host.requirementBriefReplayCalls).toEqual([
      [0, 200],
      [0, 200],
    ]);
    expect(
      client
        .getSnapshot()
        .requirementBriefEvents.map(({ sequence }) => sequence),
    ).toEqual([1, 2]);
  });

  it("builds typed requirement brief commands and requires positive revisions", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host, {
      actorId: "intake-producer",
      accountId: "agency-1",
      windowId: "requirement-brief",
      now: () => 10_000,
    });
    const createPayload = createRequirementBriefPayload("project-intake");
    const updatePayload = updateRequirementBriefPayload();

    await client.createRequirementBrief(createPayload, {
      commandId: "requirement-create-command",
      traceId: "requirement-create-trace",
      idempotencyKey: "requirement-create-idem",
    });
    await client.updateRequirementBrief(updatePayload, 7, {
      commandId: "requirement-update-command",
      traceId: "requirement-update-trace",
      idempotencyKey: "requirement-update-idem",
    });
    await client.changeRequirementBriefStatus(
      "requirement-1",
      "confirmed",
      8,
      {
        commandId: "requirement-status-command",
        traceId: "requirement-status-trace",
        idempotencyKey: "requirement-status-idem",
      },
    );

    expect(host.requirementBriefCommands[0]).toEqual({
      commandType: "requirementBrief.create",
      commandId: "requirement-create-command",
      protocolVersion: "1.5",
      context: {
        actorId: "intake-producer",
        accountId: "agency-1",
        projectId: "project-intake",
        windowId: "requirement-brief",
        traceId: "requirement-create-trace",
      },
      payload: createPayload,
      idempotencyKey: "requirement-create-idem",
      expectedRevision: null,
      deadlineAt: 40_000,
    });
    expect(host.requirementBriefCommands[1]).toMatchObject({
      commandType: "requirementBrief.update",
      context: {
        projectId: "project-intake",
        traceId: "requirement-update-trace",
      },
      payload: updatePayload,
      idempotencyKey: "requirement-update-idem",
      expectedRevision: 7,
      deadlineAt: 40_000,
    });
    expect(host.requirementBriefCommands[2]).toMatchObject({
      commandType: "requirementBrief.changeStatus",
      context: {
        projectId: "project-intake",
        traceId: "requirement-status-trace",
      },
      payload: { briefId: "requirement-1", status: "confirmed" },
      idempotencyKey: "requirement-status-idem",
      expectedRevision: 8,
      deadlineAt: 40_000,
    });
    expect(client.getSnapshot().requirementBriefs[0]).toMatchObject({
      id: "requirement-1",
      projectId: "project-intake",
      revision: 9,
      status: "confirmed",
    });

    expect(() => client.updateRequirementBrief(updatePayload, 0)).toThrow(
      "expectedRevision must be a positive integer",
    );
    expect(() =>
      client.changeRequirementBriefStatus("requirement-1", "confirmed", -1),
    ).toThrow("expectedRevision must be a positive integer");
    expect(host.requirementBriefCommands).toHaveLength(3);
  });

  it("defaults business workspace prefill candidate limit to 50", async () => {
    const host = new FakeHostAdapter();
    const candidates: BusinessWorkspacePrefillCandidate[] = [
      {
        sourceWorkspaceId: "source-workspace",
        sourceProjectId: "source-project",
        sourceProjectTitle: "Source project",
        customerName: "Acme",
        customerLegalName: "Acme LLC",
        supplierLegalName: "Studio LLC",
        matchKind: "both",
        populatedFields: ["customerLegalName", "supplierLegalName"],
        status: "active",
        sourceRevision: 7,
        sourceUpdatedAt: 1_234,
      },
    ];
    host.listBusinessWorkspacePrefillCandidatesImpl = async () => candidates;
    const client = new BsaigcClient(host);

    const result = await client.listBusinessWorkspacePrefillCandidates({
      targetProjectId: "target-project",
    });

    expect(host.businessWorkspacePrefillCandidateRequests).toEqual([
      { targetProjectId: "target-project", limit: 50 },
    ]);
    expect(result).toBe(candidates);
  });

  it("passes explicit business workspace prefill candidate requests through unchanged", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    const request: ListBusinessWorkspacePrefillCandidatesRequest = {
      targetProjectId: "target-project",
      limit: null,
    };

    await client.listBusinessWorkspacePrefillCandidates(request);

    expect(host.businessWorkspacePrefillCandidateRequests).toEqual([request]);
    expect(host.businessWorkspacePrefillCandidateRequests[0]).toBe(request);
  });

  it("returns business workspace prefill previews without recomputing them", async () => {
    const host = new FakeHostAdapter();
    const preview: BusinessWorkspacePrefillPreview = {
      targetProjectId: "target-project",
      targetProjectTitle: "Target project",
      targetCustomerName: "Acme",
      targetRequirementBriefId: null,
      sourceWorkspaceId: "source-workspace",
      sourceProjectId: "source-project",
      sourceProjectTitle: "Source project",
      matchKind: "both",
      sourceRevision: 7,
      sourceUpdatedAt: 1_234,
      changes: [
        {
          field: "customerLegalName",
          targetValue: "",
          sourceValue: "Acme LLC",
          resultValue: "Acme LLC",
          decision: "filled",
        },
      ],
    };
    host.previewBusinessWorkspacePrefillImpl = async () => preview;
    const client = new BsaigcClient(host);
    const request: PreviewBusinessWorkspacePrefillRequest = {
      targetProjectId: "target-project",
      sourceWorkspaceId: "source-workspace",
    };

    const result = await client.previewBusinessWorkspacePrefill(request);

    expect(host.businessWorkspacePrefillPreviewRequests).toEqual([request]);
    expect(host.businessWorkspacePrefillPreviewRequests[0]).toBe(request);
    expect(result).toBe(preview);
  });

  it("replays business workspace pages and buffers live events during startup", async () => {
    const host = new FakeHostAdapter();
    const businessWorkspacesGate = deferred<BusinessWorkspaceRecord[]>();
    host.listBusinessWorkspacesImpl = () => businessWorkspacesGate.promise;
    host.businessWorkspaceEvents = Array.from({ length: 201 }, (_, index) =>
      businessWorkspaceEvent(index + 1),
    );
    const client = new BsaigcClient(host);

    const starting = client.start();
    await vi.waitFor(() => expect(host.businessWorkspaceListener).not.toBeNull());
    host.emitBusinessWorkspace(businessWorkspaceEvent(202));
    businessWorkspacesGate.resolve([businessWorkspaceRecord(1)]);
    await starting;

    expect(host.businessWorkspaceReplayCalls).toEqual([
      [0, 200],
      [200, 200],
    ]);
    expect(client.getSnapshot()).toMatchObject({
      started: true,
      synchronizing: false,
      businessWorkspaceLastSequence: 202,
    });
    expect(client.getSnapshot().businessWorkspaces[0]?.revision).toBe(202);
    expect(client.getSnapshot().businessWorkspaceEvents).toHaveLength(80);
    expect(client.getSnapshot().businessWorkspaceEvents[0]?.sequence).toBe(123);
    expect(client.getSnapshot().businessWorkspaceEvents[79]?.sequence).toBe(202);
  });

  it("bounds business workspace gap retries and converges on a late event without clearing other errors", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();
    const replayBusinessWorkspaceEvents = vi.fn(() =>
      Promise.reject({
        code: "BUSINESS_WORKSPACE_REPLAY_UNAVAILABLE",
        message: "Business workspace replay is temporarily unavailable",
        retryable: true,
      }),
    );
    host.replayBusinessWorkspaceEvents = replayBusinessWorkspaceEvents;

    host.emitBusinessWorkspace(businessWorkspaceEvent(2));

    await vi.waitFor(() => {
      expect(replayBusinessWorkspaceEvents).toHaveBeenCalledTimes(3);
      expect(client.getSnapshot().error).toMatchObject({
        code: "BUSINESS_WORKSPACE_REPLAY_UNAVAILABLE",
      });
    });
    expect(client.getSnapshot()).toMatchObject({
      businessWorkspaces: [],
      businessWorkspaceEvents: [],
      businessWorkspaceLastSequence: 0,
    });

    host.getHostStatus = () =>
      Promise.reject({
        code: "DB_BUSY",
        message: "Database is busy",
        retryable: true,
      });
    await expect(client.getHostStatus()).rejects.toMatchObject({
      code: "DB_BUSY",
    });
    // The freshest command failure takes priority over the latched gap error.
    expect(client.getSnapshot().error).toMatchObject({
      code: "DB_BUSY",
    });

    host.emitBusinessWorkspace(businessWorkspaceEvent(1));

    await vi.waitFor(() => {
      expect(client.getSnapshot()).toMatchObject({
        businessWorkspaceLastSequence: 2,
        error: { code: "DB_BUSY" },
      });
    });
    expect(
      client
        .getSnapshot()
        .businessWorkspaceEvents.map(({ sequence }) => sequence),
    ).toEqual([1, 2]);
  });

  it("recovers business workspace events up to a command receipt sequence", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();
    host.businessWorkspaceEvents = [
      businessWorkspaceEvent(1),
      businessWorkspaceEvent(2),
    ];
    const executeBusinessWorkspace = host.executeBusinessWorkspaceImpl;
    host.executeBusinessWorkspaceImpl = async (command) => {
      const response = await executeBusinessWorkspace(command);
      return {
        ...response,
        receipt: {
          ...response.receipt,
          lastEventSequence: 2,
        },
      };
    };

    await client.createBusinessWorkspace({ projectId: "business-project" });
    expect(host.businessWorkspaceCommands[0]?.payload).toEqual({
      projectId: "business-project",
      customerId: null,
      prefillSourceWorkspaceId: null,
    });

    await vi.waitFor(() => {
      expect(client.getSnapshot().businessWorkspaceLastSequence).toBe(2);
    });
    expect(host.businessWorkspaceReplayCalls).toEqual([
      [0, 200],
      [0, 200],
    ]);
  });

  it("builds all seven typed business workspace commands and validates revisions", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host, {
      actorId: "business-producer",
      accountId: "agency-1",
      windowId: "business-workspace",
      now: () => 10_000,
    });

    await client.createBusinessWorkspace(
      {
        projectId: "business-project",
        customerId: null,
        prefillSourceWorkspaceId: "source-workspace",
      },
      {
        commandId: "business-create-command",
        traceId: "business-create-trace",
        idempotencyKey: "business-create-idem",
      },
    );
    await client.updateBusinessProfile(
      { workspaceId: "workspace-1", profile: BUSINESS_PROFILE_INPUT },
      1,
    );
    const paymentResponse = await client.upsertBusinessPayment(
      {
        workspaceId: "workspace-1",
        payment: {
          id: null,
          label: "Deposit",
          amountCents: 100_000,
          dueAt: 20_000,
          occurredAt: null,
          status: "planned",
          reference: "deposit-ref",
          notes: "Initial deposit",
        },
      },
      2,
    );
    const payment = paymentResponse.businessWorkspace.payments[0];
    const paymentRequestResponse = await client.createBusinessDocument(
      {
        workspaceId: "workspace-1",
        kind: "paymentRequest",
        documentNumber: "PR-001",
        title: "Launch deposit request",
        templateKey: "builtin.payment-request.standard.v1",
        paymentId: payment.id,
      },
      3,
    );
    const paymentRequestViewModel =
      paymentRequestResponse.businessWorkspace.documents[0];
    await client.changeBusinessDocumentStatus(
      "workspace-1",
      "document-1",
      "approved",
      4,
    );
    await client.generateBusinessDocument(
      {
        workspaceId: "workspace-1",
        documentId: "document-1",
        format: "docx",
      },
      5,
    );
    await client.changeBusinessWorkspaceStatus(
      "workspace-1",
      "archived",
      6,
    );

    expect(
      host.businessWorkspaceCommands.map(
        ({ commandType, expectedRevision }) => ({
          commandType,
          expectedRevision,
        }),
      ),
    ).toEqual([
      { commandType: "businessWorkspace.create", expectedRevision: null },
      { commandType: "businessWorkspace.updateProfile", expectedRevision: 1 },
      { commandType: "businessWorkspace.upsertPayment", expectedRevision: 2 },
      { commandType: "businessWorkspace.createDocument", expectedRevision: 3 },
      {
        commandType: "businessWorkspace.changeDocumentStatus",
        expectedRevision: 4,
      },
      { commandType: "businessWorkspace.generateDocument", expectedRevision: 5 },
      { commandType: "businessWorkspace.changeStatus", expectedRevision: 6 },
    ]);
    expect(host.businessWorkspaceCommands[0]).toMatchObject({
      protocolVersion: "1.6",
      context: {
        actorId: "business-producer",
        accountId: "agency-1",
        projectId: "business-project",
        windowId: "business-workspace",
        traceId: "business-create-trace",
      },
      idempotencyKey: "business-create-idem",
      deadlineAt: 40_000,
      payload: {
        projectId: "business-project",
        prefillSourceWorkspaceId: "source-workspace",
      },
    });
    const frozenPayment = {
      id: "payment-1",
      label: "Deposit",
      amountCents: 100_000,
      dueAt: 20_000,
      occurredAt: null,
      status: "planned",
      reference: "deposit-ref",
      notes: "Initial deposit",
      revision: 1,
      createdAt: 3,
      updatedAt: 3,
    };
    expect(host.businessWorkspaceCommands[3]).toMatchObject({
      commandType: "businessWorkspace.createDocument",
      payload: {
        kind: "paymentRequest",
        paymentId: "payment-1",
      },
    });
    expect(paymentRequestViewModel).toMatchObject({
      id: "document-1",
      kind: "paymentRequest",
      snapshot: {
        workspaceRevision: 3,
        payment: frozenPayment,
      },
    });
    expect(paymentRequestViewModel.snapshot.payment).not.toBe(payment);

    const projectedWorkspaceRecord = client.getSnapshot().businessWorkspaces[0];
    expect(projectedWorkspaceRecord).toMatchObject({
      id: "workspace-1",
      projectId: "business-project",
      revision: 7,
      status: "archived",
      payments: [frozenPayment],
      documents: [
        {
          id: "document-1",
          kind: "paymentRequest",
          snapshot: {
            workspaceRevision: 3,
            payment: frozenPayment,
          },
        },
      ],
    });
    const projectedPaymentRequestRecord = projectedWorkspaceRecord.documents[0];
    expect(projectedPaymentRequestRecord.snapshot.payment).toEqual(
      paymentRequestViewModel.snapshot.payment,
    );
    expect(projectedPaymentRequestRecord.snapshot.payment).not.toBe(
      projectedWorkspaceRecord.payments[0],
    );

    expect(() =>
      client.updateBusinessProfile(
        { workspaceId: "workspace-1", profile: BUSINESS_PROFILE_INPUT },
        0,
      ),
    ).toThrow("expectedRevision must be a positive integer");
    expect(host.businessWorkspaceCommands).toHaveLength(7);
  });

  it("builds every 1.6 customer, delivery, invoice, and archive command as durable JSON", async () => {
    const host = new FakeHostAdapter();
    host.businessWorkspaces = [businessWorkspaceRecord(10, "business-project")];
    const client = new BsaigcClient(host, {
      actorId: "closure-operator",
      accountId: "agency-1",
      windowId: "business-closure",
      now: () => 50_000,
      uuid: (() => {
        let next = 0;
        return () => `closure-uuid-${++next}`;
      })(),
    });
    await client.start();

    await client.upsertBusinessCustomer(
      {
        workspaceId: "workspace-1",
        customerId: null,
        customer: {
          displayName: "Half Mountain",
          legalName: "Half Mountain Media Co., Ltd.",
          taxId: "91330000123456789X",
          billingAddress: "Hangzhou",
          primaryContactName: "Buyer",
          primaryPhone: "13800000000",
          primaryEmail: "buyer@example.com",
          notes: "Key account",
        },
      },
      10,
      {
        commandId: "customer-command",
        traceId: "customer-trace",
        idempotencyKey: "customer-idempotency",
        deadlineAt: null,
      },
    );
    await client.assignBusinessCustomer(
      { workspaceId: "workspace-1", customerId: "customer-2" },
      11,
    );
    await client.upsertBusinessMilestone(
      {
        workspaceId: "workspace-1",
        milestone: {
          id: null,
          title: "Final delivery",
          description: "Approved master and source package",
          dueAt: 60_000,
          acceptanceCriteria: "Written customer signoff",
          required: true,
          status: "planned",
        },
      },
      12,
    );
    await client.registerBusinessDeliverableVersion(
      {
        workspaceId: "workspace-1",
        milestoneId: "milestone-1",
        deliverableId: null,
        name: "Master film",
        required: true,
        assetId: "asset-deliverable-v1",
        notes: "First formal delivery",
      },
      13,
    );
    await client.recordBusinessDeliverySent(
      {
        workspaceId: "workspace-1",
        milestoneId: "milestone-1",
        versionIds: ["version-1"],
        recipient: "buyer@example.com",
        channel: "email",
        sentAt: 61_000,
        note: "Sent for acceptance",
      },
      14,
    );
    await client.recordBusinessDeliverySignoff(
      {
        workspaceId: "workspace-1",
        submissionId: "submission-1",
        acceptedVersionIds: ["version-1"],
        rejectedVersionIds: [],
        customerRepresentative: "Buyer",
        evidence: {
          assetId: "asset-signoff",
          occurredAt: 62_000,
          note: "Signed acceptance",
        },
        note: "Accepted",
        occurredAt: 62_000,
      },
      15,
    );
    await client.recordBusinessInvoiceIssued(
      {
        workspaceId: "workspace-1",
        paymentId: "payment-1",
        invoiceCode: "INV-CODE",
        invoiceNumber: "INV-001",
        amountCents: 100_000,
        taxCents: 6_000,
        issuedAt: 63_000,
        assetIds: ["asset-invoice"],
      },
      16,
    );
    await client.recordBusinessInvoiceRedCorrection(
      {
        workspaceId: "workspace-1",
        originalInvoiceId: "invoice-1",
        invoiceCode: "RED-CODE",
        invoiceNumber: "RED-001",
        amountCents: 20_000,
        taxCents: 1_200,
        issuedAt: 64_000,
        reason: "Customer information correction",
        assetIds: ["asset-red-invoice"],
      },
      17,
    );
    await client.attachBusinessInvoiceAsset(
      {
        workspaceId: "workspace-1",
        invoiceId: "invoice-1",
        assetId: "asset-invoice-receipt",
        role: "receipt",
      },
      18,
    );
    await client.createBusinessArchiveSnapshot(
      { workspaceId: "workspace-1" },
      19,
    );

    const closureCommands = host.businessWorkspaceCommands;
    expect(
      closureCommands.map(({ commandType, expectedRevision }) => ({
        commandType,
        expectedRevision,
      })),
    ).toEqual([
      { commandType: "businessWorkspace.upsertCustomer", expectedRevision: 10 },
      { commandType: "businessWorkspace.assignCustomer", expectedRevision: 11 },
      { commandType: "businessWorkspace.upsertMilestone", expectedRevision: 12 },
      {
        commandType: "businessWorkspace.registerDeliverableVersion",
        expectedRevision: 13,
      },
      { commandType: "businessWorkspace.recordDeliverySent", expectedRevision: 14 },
      {
        commandType: "businessWorkspace.recordDeliverySignoff",
        expectedRevision: 15,
      },
      { commandType: "businessWorkspace.recordInvoiceIssued", expectedRevision: 16 },
      {
        commandType: "businessWorkspace.recordInvoiceRedCorrection",
        expectedRevision: 17,
      },
      { commandType: "businessWorkspace.attachInvoiceAsset", expectedRevision: 18 },
      {
        commandType: "businessWorkspace.createArchiveSnapshot",
        expectedRevision: 19,
      },
    ]);
    expect(closureCommands[0]).toMatchObject({
      commandId: "customer-command",
      protocolVersion: "1.6",
      context: {
        actorId: "closure-operator",
        accountId: "agency-1",
        projectId: "business-project",
        windowId: "business-closure",
        traceId: "customer-trace",
      },
      idempotencyKey: "customer-idempotency",
      deadlineAt: null,
      expectedRevision: 10,
    });
    for (const command of closureCommands) {
      expect(JSON.parse(JSON.stringify(command))).toEqual(command);
      expect(command.protocolVersion).toBe("1.6");
      expect(command.context.projectId).toBe("business-project");
      expect(command.idempotencyKey).not.toBe("");
    }

    expect(() =>
      client.createBusinessArchiveSnapshot(
        { workspaceId: "workspace-1" },
        0,
      ),
    ).toThrow("expectedRevision must be a positive integer");
    expect(host.businessWorkspaceCommands).toHaveLength(10);
  });

  it("builds contract review commands and forwards review queries, replay, and events", async () => {
    const host = new FakeHostAdapter();
    host.contractReviewEvents = [contractReviewEvent(1), contractReviewEvent(2)];
    const client = new BsaigcClient(host, {
      actorId: "contract-operator",
      accountId: "agency-1",
      windowId: "business-workbench",
      now: () => 20_000,
    });
    const commandOptions = {
      projectId: "project-contract",
      commandId: "contract-create-command",
      traceId: "contract-create-trace",
      idempotencyKey: "contract-create-idem",
      deadlineMs: 5_000,
    } as const;

    await client.createContractReview(
      { workspaceId: "workspace-1", sourceAssetId: "asset-contract" },
      commandOptions,
    );
    await client.startContractReview(
      { reviewId: "review-1" },
      1,
      { projectId: "project-contract" },
    );
    await client.cancelContractReview(
      { reviewId: "review-1", reason: "superseded contract" },
      2,
      { projectId: "project-contract" },
    );
    await client.decideReviewFinding(
      {
        reviewId: "review-1",
        findingId: "finding-1",
        decision: "needsRevision",
        comment: "Add a fixed deadline.",
      },
      3,
      { projectId: "project-contract" },
    );
    await client.generateReviewReport(
      { reviewId: "review-1", format: "docx" },
      4,
      { projectId: "project-contract" },
    );
    await client.retryContractReviewStage(
      { reviewId: "review-1", stage: "reviewingAgent" },
      5,
      { projectId: "project-contract" },
    );

    expect(host.contractReviewCommands[0]).toEqual({
      commandType: "contractReview.create",
      commandId: "contract-create-command",
      protocolVersion: "1.5",
      context: {
        actorId: "contract-operator",
        accountId: "agency-1",
        projectId: "project-contract",
        windowId: "business-workbench",
        traceId: "contract-create-trace",
      },
      payload: { workspaceId: "workspace-1", sourceAssetId: "asset-contract" },
      idempotencyKey: "contract-create-idem",
      expectedRevision: null,
      deadlineAt: 25_000,
    });
    expect(
      host.contractReviewCommands.map(({ commandType, expectedRevision }) => ({
        commandType,
        expectedRevision,
      })),
    ).toEqual([
      { commandType: "contractReview.create", expectedRevision: null },
      { commandType: "contractReview.start", expectedRevision: 1 },
      { commandType: "contractReview.cancel", expectedRevision: 2 },
      { commandType: "contractReview.decideFinding", expectedRevision: 3 },
      { commandType: "contractReview.generateReport", expectedRevision: 4 },
      { commandType: "contractReview.retryStage", expectedRevision: 5 },
    ]);

    await expect(client.listContractReviews()).resolves.toEqual(host.contractReviews);
    await expect(
      client.listContractReviews({
        workspaceId: "workspace-1",
        status: "completed",
        limit: 25,
      }),
    ).resolves.toEqual(host.contractReviews);
    await expect(client.getContractReview("review-1")).resolves.toEqual(
      host.contractReviews[0],
    );
    await expect(
      client.listReviewFindings({ reviewId: "review-1" }),
    ).resolves.toEqual([REVIEW_FINDING]);
    await expect(client.getEvidenceContext("evidence-1")).resolves.toEqual(
      EVIDENCE_CONTEXT,
    );
    await expect(client.replayContractReviewEvents(0, 1)).resolves.toEqual([
      contractReviewEvent(1),
    ]);

    expect(host.contractReviewListRequests).toEqual([
      { workspaceId: null, status: null, limit: null },
      { workspaceId: "workspace-1", status: "completed", limit: 25 },
    ]);
    expect(host.contractReviewGetRequests).toEqual([{ reviewId: "review-1" }]);
    expect(host.reviewFindingListRequests).toEqual([
      { reviewId: "review-1", status: null },
    ]);
    expect(host.evidenceContextGetRequests).toEqual([
      { evidenceId: "evidence-1" },
    ]);
    expect(host.contractReviewReplayCalls).toEqual([[0, 1]]);

    const listener = vi.fn();
    const unsubscribe = await client.subscribeContractReviewEvents(listener);
    host.emitContractReview(contractReviewEvent(3));
    expect(listener).toHaveBeenCalledWith(contractReviewEvent(3));
    unsubscribe();
    expect(host.contractReviewListener).toBeNull();

    const publicJson = JSON.stringify({
      commands: host.contractReviewCommands,
      review: host.contractReviews[0],
      finding: REVIEW_FINDING,
      evidence: EVIDENCE_CONTEXT,
    });
    expect(publicJson).not.toMatch(/https?:\/\//i);
    expect(publicJson).not.toMatch(/[A-Za-z]:[\\/]/);
    expect(publicJson).not.toMatch(/r2Url|credential|secret|accessKey/i);
  });

  it("builds backup commands without exposing storage location details", async () => {
    const host = new FakeHostAdapter();
    host.backupEvents = [backupEvent(1), backupEvent(2)];
    const client = new BsaigcClient(host, {
      actorId: "backup-operator",
      accountId: "agency-1",
      windowId: "business-workbench",
      now: () => 30_000,
    });

    await client.queueAssetBackup(
      { assetId: "asset-contract" },
      null,
      {
        projectId: "project-contract",
        commandId: "backup-queue-command",
        traceId: "backup-queue-trace",
        idempotencyKey: "backup-queue-idem",
        deadlineMs: 2_000,
      },
    );
    await client.retryAssetBackup(
      { assetId: "asset-contract" },
      1,
      { projectId: "project-contract" },
    );
    await client.cancelAssetBackup(
      { assetId: "asset-contract" },
      2,
      { projectId: "project-contract" },
    );
    await client.restoreAssetBackup(
      { assetId: "asset-contract", expectedSha256: "a".repeat(64) },
      3,
      { projectId: "project-contract" },
    );

    expect(host.backupCommands[0]).toEqual({
      commandType: "backup.queue",
      commandId: "backup-queue-command",
      protocolVersion: "1.5",
      context: {
        actorId: "backup-operator",
        accountId: "agency-1",
        projectId: "project-contract",
        windowId: "business-workbench",
        traceId: "backup-queue-trace",
      },
      payload: { assetId: "asset-contract" },
      idempotencyKey: "backup-queue-idem",
      expectedRevision: null,
      deadlineAt: 32_000,
    });
    expect(host.backupCommands[3]).toMatchObject({
      commandType: "backup.restore",
      context: {
        projectId: "project-contract",
      },
      payload: {
        assetId: "asset-contract",
        expectedSha256: "a".repeat(64),
      },
      expectedRevision: 3,
    });
    expect(
      host.backupCommands.map(({ commandType, expectedRevision }) => ({
        commandType,
        expectedRevision,
      })),
    ).toEqual([
      { commandType: "backup.queue", expectedRevision: null },
      { commandType: "backup.retry", expectedRevision: 1 },
      { commandType: "backup.cancel", expectedRevision: 2 },
      { commandType: "backup.restore", expectedRevision: 3 },
    ]);

    await expect(client.listAssetBackups()).resolves.toEqual(host.assetBackups);
    expect(host.assetBackupListLimits).toEqual([200]);
    await expect(client.replayBackupEvents(0, 1)).resolves.toEqual([
      backupEvent(1),
    ]);
    expect(host.backupReplayCalls).toEqual([[0, 1]]);

    const listener = vi.fn();
    const unsubscribe = await client.subscribeBackupEvents(listener);
    host.emitBackup(backupEvent(3));
    expect(listener).toHaveBeenCalledWith(backupEvent(3));
    unsubscribe();
    expect(host.backupListener).toBeNull();

    expect(Object.keys(host.assetBackups[0])).not.toEqual(
      expect.arrayContaining([
        "url",
        "r2Url",
        "localPath",
        "absolutePath",
        "credentials",
        "secret",
        "accessKey",
      ]),
    );
    const publicJson = JSON.stringify({
      commands: host.backupCommands,
      backups: host.assetBackups,
      events: host.backupEvents,
    });
    expect(publicJson).not.toMatch(/https?:\/\//i);
    expect(publicJson).not.toMatch(/[A-Za-z]:[\\/]/);
    expect(publicJson).not.toMatch(/r2Url|credential|secret|accessKey/i);
  });

  it("rejects invalid contract review and backup revisions, limits, and replay cursors", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);

    expect(() =>
      client.startContractReview({ reviewId: "review-1" }, 0),
    ).toThrow("expectedRevision must be a positive integer");
    expect(() =>
      client.cancelContractReview(
        { reviewId: "review-1", reason: "cancel" },
        -1,
      ),
    ).toThrow("expectedRevision must be a positive integer");
    expect(() =>
      client.queueAssetBackup({ assetId: "asset-contract" }, 0),
    ).toThrow("expectedRevision must be a positive integer");
    expect(() =>
      client.retryAssetBackup({ assetId: "asset-contract" }, 1.5),
    ).toThrow("expectedRevision must be a positive integer");
    expect(() =>
      client.restoreAssetBackup(
        { assetId: "asset-contract", expectedSha256: "a".repeat(64) },
        0,
      ),
    ).toThrow("expectedRevision must be a positive integer");

    await expect(client.listContractReviews({ limit: 0 })).rejects.toMatchObject({
      code: "HOST_ERROR",
      message: "limit must be a positive integer",
      retryable: false,
    });
    await expect(client.listAssetBackups(-1)).rejects.toMatchObject({
      code: "HOST_ERROR",
      message: "limit must be a positive integer",
      retryable: false,
    });
    await expect(client.replayContractReviewEvents(-1, 10)).rejects.toMatchObject({
      code: "HOST_ERROR",
      message: "afterSequence must be a non-negative integer",
      retryable: false,
    });
    await expect(client.replayBackupEvents(0, 0)).rejects.toMatchObject({
      code: "HOST_ERROR",
      message: "limit must be a positive integer",
      retryable: false,
    });
    expect(host.contractReviewListRequests).toEqual([]);
    expect(host.assetBackupListLimits).toEqual([]);
    expect(host.contractReviewReplayCalls).toEqual([]);
    expect(host.backupReplayCalls).toEqual([]);

    host.getContractReview = () =>
      Promise.reject({
        code: "REVIEW_BUSY",
        message: "Review is busy",
        retryable: true,
      });
    await expect(client.getContractReview("review-1")).rejects.toEqual({
      code: "REVIEW_BUSY",
      message: "Review is busy",
      retryable: true,
    });
    expect(client.getSnapshot().error?.code).toBe("REVIEW_BUSY");
  });

  it("resets business workspace projection and pending lifecycle state on stop", async () => {
    const host = new FakeHostAdapter();
    host.businessWorkspaceEvents = [businessWorkspaceEvent(1)];
    const client = new BsaigcClient(host);
    await client.start();
    expect(client.getSnapshot().businessWorkspaceLastSequence).toBe(1);

    client.stop();
    host.emitBusinessWorkspace(businessWorkspaceEvent(2));

    expect(host.businessWorkspaceListener).toBeNull();
    expect(client.getSnapshot()).toMatchObject({
      started: false,
      businessWorkspaces: [],
      businessWorkspaceEvents: [],
      businessWorkspaceLastSequence: 0,
    });
  });

  it("stops the host subscription and ignores later live events", async () => {
    const host = new FakeHostAdapter();
    const client = new BsaigcClient(host);
    await client.start();

    client.stop();
    host.emit(event(1));

    expect(host.unsubscribed).toBe(true);
    expect(client.getSnapshot().started).toBe(false);
    expect(client.getSnapshot().events).toHaveLength(0);
  });

  it("normalizes structured, JSON, Error, and unknown host failures", async () => {
    expect(
      normalizeHostError({ code: "BUSY", message: "Try later", retryable: true }),
    ).toEqual({ code: "BUSY", message: "Try later", retryable: true });
    expect(
      normalizeHostError(
        JSON.stringify({ code: "OFFLINE", message: "No host", retryable: true }),
      ),
    ).toEqual({ code: "OFFLINE", message: "No host", retryable: true });
    expect(normalizeHostError(new Error("Broken"))).toEqual({
      code: "HOST_ERROR",
      message: "Broken",
      retryable: false,
    });
    expect(normalizeHostError(null)).toEqual({
      code: "HOST_ERROR",
      message: "Host operation failed",
      retryable: false,
    });

    const host = new FakeHostAdapter();
    host.getHostStatus = () =>
      Promise.reject({ code: "DB_BUSY", message: "Database busy", retryable: true });
    const client = new BsaigcClient(host);

    await expect(client.getHostStatus()).rejects.toEqual({
      code: "DB_BUSY",
      message: "Database busy",
      retryable: true,
    });
    expect(client.getSnapshot().error?.code).toBe("DB_BUSY");

    host.getHostStatus = () => Promise.resolve(HOST_STATUS);
    await expect(client.getHostStatus()).resolves.toEqual(HOST_STATUS);
    expect(client.getSnapshot().error).toBeNull();
  });
});

describe("BsaigcClient asset actions", () => {
  it("opens and exports real assets through stable asset IDs only", async () => {
    const getAssetActionCapabilities = vi.fn(async (assetId: string) => ({
      assetId,
      canOpen: true,
      canExport: true,
      reason: null,
    }));
    const openAsset = vi.fn(async (_assetId: string) => undefined);
    const exportAsset = vi.fn(async (_assetId: string) => true);
    const host = Object.assign(new FakeHostAdapter(), {
      getAssetActionCapabilities,
      openAsset,
      exportAsset,
    });
    const client = new BsaigcClient(host);

    await expect(client.getAssetActionCapabilities("asset-report")).resolves.toMatchObject({
      assetId: "asset-report",
      canOpen: true,
      canExport: true,
    });
    await expect(client.openAsset("asset-report")).resolves.toBeUndefined();
    await expect(client.exportAsset("asset-report")).resolves.toBe(true);

    expect(getAssetActionCapabilities).toHaveBeenCalledTimes(3);
    expect(getAssetActionCapabilities).toHaveBeenCalledWith("asset-report");
    expect(openAsset).toHaveBeenCalledWith("asset-report");
    expect(exportAsset).toHaveBeenCalledWith("asset-report");
    const serializedCalls = JSON.stringify({
      capabilities: getAssetActionCapabilities.mock.calls,
      open: openAsset.mock.calls,
      export: exportAsset.mock.calls,
    });
    expect(serializedCalls).not.toMatch(/https?:\/\//i);
    expect(serializedCalls).not.toMatch(/[A-Za-z]:[\\/]/);
    expect(serializedCalls).not.toMatch(/path|url|header|credential|secret/i);
  });

  it("rejects empty IDs, absolute paths, file URLs, and web URLs before host calls", async () => {
    const getAssetActionCapabilities = vi.fn();
    const openAsset = vi.fn();
    const exportAsset = vi.fn();
    const host = Object.assign(new FakeHostAdapter(), {
      getAssetActionCapabilities,
      openAsset,
      exportAsset,
    });
    const client = new BsaigcClient(host);

    expect(() => client.getAssetActionCapabilities("   ")).toThrow(
      "assetId must not be empty",
    );
    await expect(client.openAsset("C:\\contracts\\report.docx")).rejects.toThrow(
      "assetId must be a stable identifier",
    );
    await expect(client.exportAsset("file:///contracts/report.docx")).rejects.toThrow(
      "assetId must be a stable identifier",
    );
    await expect(client.exportAsset("https://example.com/report.docx")).rejects.toThrow(
      "assetId must be a stable identifier",
    );

    expect(getAssetActionCapabilities).not.toHaveBeenCalled();
    expect(openAsset).not.toHaveBeenCalled();
    expect(exportAsset).not.toHaveBeenCalled();
  });
});
