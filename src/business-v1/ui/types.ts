import type { BusinessAcceptanceBatchStatus } from "../../generated/bsaigc/BusinessAcceptanceBatchStatus";
import type { WebResearchSource } from "../application/webResearchView";

export type { WebResearchSource } from "../application/webResearchView";

export type BusinessTaskKind =
  | "quotation"
  | "contract-review"
  | "acceptance"
  | "settlement"
  | "archive"
  | "search";

export type TaskStatus =
  | "queued"
  | "running"
  | "waiting-confirmation"
  | "completed"
  | "failed";

export type AttachmentKind =
  | "file"
  | "folder"
  | "image"
  | "pdf"
  | "document"
  | "spreadsheet";

export type SourceScope = "workspace" | "workspace-shared";
export type NetworkScope = "local-only" | "web-enabled";
export type ContextTab =
  | "issues"
  | "template"
  | "preview"
  | "approval"
  | "versions";

export type ProjectAction = "pin" | "rename" | "archive" | "delete";
export type ConversationAction = "pin" | "rename" | "archive" | "delete";
export type ApprovalDecision = "approve" | "reject";

export interface WorkspaceProject {
  id: string;
  name: string;
  customerName: string;
  updatedAt: string;
  localPath?: string;
  unreadCount?: number;
  isPinned?: boolean;
  isArchived?: boolean;
}

export interface WorkspaceConversation {
  id: string;
  projectId: string;
  title: string;
  preview: string;
  updatedAt: string;
  taskKind?: BusinessTaskKind;
  unreadCount?: number;
  isPinned?: boolean;
}

export interface WorkspaceUser {
  id: string;
  name: string;
  roleLabel: string;
  initials: string;
  updateAvailable?: boolean;
}

export interface WorkspaceAttachment {
  id: string;
  name: string;
  kind: AttachmentKind;
  sizeLabel?: string;
  sourceLabel?: string;
  status?: "ready" | "reading" | "failed";
}

export interface OutputArtifact {
  id: string;
  name: string;
  format: "docx" | "xlsx" | "pdf" | "folder" | "other";
  versionLabel: string;
  status: "draft" | "ready" | "blocked";
  detail?: string;
}

export interface WorkspaceTask {
  id: string;
  kind: BusinessTaskKind;
  title: string;
  status: TaskStatus;
  stageLabel: string;
  progress: number;
  detail: string;
  startedAt?: string;
  requiresConfirmation?: boolean;
  confirmationBlockedReason?: string;
  outputs?: OutputArtifact[];
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  authorName: string;
  createdAt: string;
  content: string;
  attachments?: WorkspaceAttachment[];
  task?: WorkspaceTask;
  sources?: WebResearchSource[];
}

export interface MissingMaterial {
  id: string;
  title: string;
  detail: string;
  severity: "blocking" | "warning";
}

export interface FieldConflict {
  id: string;
  fieldLabel: string;
  primaryValue: string;
  primarySource: string;
  secondaryValue: string;
  secondarySource: string;
}

export interface MatchedTemplate {
  id: string;
  name: string;
  versionLabel: string;
  sourceLabel: string;
  confidence: number;
  isSelected: boolean;
}

export interface LegalRisk {
  id: string;
  title: string;
  detail: string;
  level: "high" | "medium" | "low";
  sourceLabel?: string;
}

export interface PreviewDocument {
  id: string;
  name: string;
  format: "docx" | "xlsx" | "pdf";
  pageLabel?: string;
  status: "ready" | "generating" | "blocked";
}

export interface ApprovalRequest {
  id: string;
  title: string;
  detail: string;
  requestedBy: string;
  requestedAt: string;
  status: "pending" | "approved" | "rejected";
  blocked?: string;
}

export interface DocumentVersion {
  id: string;
  label: string;
  authorName: string;
  createdAt: string;
  note: string;
  isCurrent: boolean;
}

export interface AcceptanceBatchSummary {
  id: string;
  label: string;
  status: BusinessAcceptanceBatchStatus;
  preparedCount: number;
  totalCount: number;
  isReady: boolean;
  blockerText?: string;
  isPreparing: boolean;
  prepareDisabledReason?: string;
}

export interface WorkspaceContext {
  acceptanceBatches: AcceptanceBatchSummary[];
  missingMaterials: MissingMaterial[];
  conflicts: FieldConflict[];
  templates: MatchedTemplate[];
  legalRisks: LegalRisk[];
  previews: PreviewDocument[];
  approvals: ApprovalRequest[];
  versions: DocumentVersion[];
}

export interface SelectOption {
  value: string;
  label: string;
  description?: string;
}

export interface ComposerState {
  value: string;
  attachments: WorkspaceAttachment[];
  sourceScope: SourceScope;
  networkScope: NetworkScope;
  modelId: string;
  isSubmitting?: boolean;
  placeholder?: string;
}

export interface BusinessWorkspaceActions {
  onCreateProject: () => void;
  onSelectProject: (projectId: string) => void;
  onProjectAction: (projectId: string, action: ProjectAction) => void;
  onCreateConversation: () => void;
  onSelectConversation: (conversationId: string) => void;
  onConversationAction: (conversationId: string, action: ConversationAction) => void;
  onStartTask: (kind: BusinessTaskKind) => void;
  onOpenWorkspaceFolder: () => void;
  onOpenArtifact: (artifactId: string) => void;
  onRetryTask: (taskId: string) => void;
  onConfirmTask: (taskId: string) => void;
  onComposerChange: (value: string) => void;
  onSendMessage: () => void;
  onAddFiles: () => void;
  onAddFolder: () => void;
  onPasteScreenshot: (images?: File[]) => void;
  onDropFiles?: (files: File[]) => void;
  onDropPaths?: (paths: string[]) => void;
  onRemoveAttachment: (attachmentId: string) => void;
  onSourceScopeChange: (scope: SourceScope) => void;
  onNetworkScopeChange: (scope: NetworkScope) => void;
  onModelChange: (modelId: string) => void;
  onResolveMissingMaterial: (materialId: string) => void;
  onResolveConflict: (conflictId: string) => void;
  onSelectTemplate: (templateId: string) => void;
  onReviewLegalRisk: (riskId: string) => void;
  onPrepareAcceptanceDocuments: (batchId: string) => void;
  onOpenPreview: (previewId: string) => void;
  onApprovalDecision: (approvalId: string, decision: ApprovalDecision) => void;
  onRestoreVersion: (versionId: string) => void;
  onOpenHistory: () => void;
  onOpenSettings: () => void;
  onCheckForUpdates: () => void;
  onSignOut: () => void;
}

export interface BusinessWorkspaceShellProps {
  productName?: string;
  projects: WorkspaceProject[];
  conversations: WorkspaceConversation[];
  activeProjectId: string | null;
  activeConversationId: string | null;
  messages: ChatMessage[];
  context: WorkspaceContext;
  contextTab?: ContextTab;
  user: WorkspaceUser;
  composer: ComposerState;
  modelOptions: SelectOption[];
  actions: BusinessWorkspaceActions;
  onContextTabChange?: (tab: ContextTab) => void;
  isLoading?: boolean;
}
