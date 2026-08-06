export type AcceptanceBatchStatus =
  | "collecting"
  | "documentsPrepared"
  | "approved"
  | "generated";

export interface AcceptanceBatchInput {
  id: string;
  status: AcceptanceBatchStatus;
}

export interface AcceptanceRequirementInput {
  id: string;
  label: string;
  requiredGroupCount: number;
}

export interface AcceptanceMaterialInput {
  id: string;
  requirementId: string;
  groupId: string;
  confirmed: boolean;
  duplicateOfMaterialId?: string | null;
}

export interface CreateAcceptanceWorkflowInput {
  batch: AcceptanceBatchInput;
  requirements: readonly AcceptanceRequirementInput[];
  materials: readonly AcceptanceMaterialInput[];
}

export type AcceptanceMaterialExclusionReason = "duplicate" | "unconfirmed";

export interface AcceptanceMaterialProjection {
  id: string;
  groupId: string;
  counted: boolean;
  exclusionReason: AcceptanceMaterialExclusionReason | null;
}

export interface AcceptanceMaterialMatrixRow {
  requirementId: string;
  requirementLabel: string;
  requiredGroupCount: number;
  providedGroupCount: number;
  missingGroupCount: number;
  status: "ready" | "missing";
  materials: AcceptanceMaterialProjection[];
}

export interface AcceptanceWorkflowBlocker {
  code: "missingMaterialGroups";
  requirementId: string;
  requirementLabel: string;
  requiredGroupCount: number;
  providedGroupCount: number;
  missingGroupCount: number;
}

export type AcceptanceTaskStageId =
  | "materials"
  | "classification"
  | "mapping"
  | "validation"
  | "documents"
  | "approval"
  | "generation";

export type AcceptanceTaskStageStatus = "pending" | "active" | "blocked" | "completed";

export interface AcceptanceTaskStage {
  id: AcceptanceTaskStageId;
  status: AcceptanceTaskStageStatus;
}

export interface AcceptanceWorkflowResult {
  batchId: string;
  isReady: boolean;
  materialMatrix: AcceptanceMaterialMatrixRow[];
  blockers: AcceptanceWorkflowBlocker[];
  stages: AcceptanceTaskStage[];
}

export function createAcceptanceWorkflow(
  input: CreateAcceptanceWorkflowInput,
): AcceptanceWorkflowResult {
  const materialMatrix = input.requirements.map((requirement) =>
    projectRequirement(requirement, input.materials),
  );
  const blockers = materialMatrix.flatMap<AcceptanceWorkflowBlocker>((row) =>
    row.missingGroupCount === 0
      ? []
      : [{
          code: "missingMaterialGroups",
          requirementId: row.requirementId,
          requirementLabel: row.requirementLabel,
          requiredGroupCount: row.requiredGroupCount,
          providedGroupCount: row.providedGroupCount,
          missingGroupCount: row.missingGroupCount,
        }],
  );
  const isReady = blockers.length === 0;

  return {
    batchId: input.batch.id,
    isReady,
    materialMatrix,
    blockers,
    stages: projectStages(input.batch.status, isReady),
  };
}

function projectRequirement(
  requirement: AcceptanceRequirementInput,
  materials: readonly AcceptanceMaterialInput[],
): AcceptanceMaterialMatrixRow {
  const seenGroupIds = new Set<string>();
  const projectedMaterials = materials
    .filter((material) => material.requirementId === requirement.id)
    .map<AcceptanceMaterialProjection>((material) => {
      if (!material.confirmed) {
        return {
          id: material.id,
          groupId: material.groupId,
          counted: false,
          exclusionReason: "unconfirmed",
        };
      }

      const duplicate = Boolean(material.duplicateOfMaterialId) || seenGroupIds.has(material.groupId);
      if (!duplicate) seenGroupIds.add(material.groupId);
      return {
        id: material.id,
        groupId: material.groupId,
        counted: !duplicate,
        exclusionReason: duplicate ? "duplicate" : null,
      };
    });
  const requiredGroupCount = Math.max(0, requirement.requiredGroupCount);
  const providedGroupCount = seenGroupIds.size;
  const missingGroupCount = Math.max(0, requiredGroupCount - providedGroupCount);

  return {
    requirementId: requirement.id,
    requirementLabel: requirement.label,
    requiredGroupCount,
    providedGroupCount,
    missingGroupCount,
    status: missingGroupCount === 0 ? "ready" : "missing",
    materials: projectedMaterials,
  };
}

function projectStages(
  batchStatus: AcceptanceBatchStatus,
  isReady: boolean,
): AcceptanceTaskStage[] {
  const documentsPrepared = batchStatus !== "collecting";
  const approved = batchStatus === "approved" || batchStatus === "generated";
  const generated = batchStatus === "generated";

  return [
    { id: "materials", status: isReady ? "completed" : "active" },
    { id: "classification", status: isReady ? "completed" : "active" },
    { id: "mapping", status: isReady ? "completed" : "active" },
    { id: "validation", status: isReady ? "completed" : "blocked" },
    {
      id: "documents",
      status: !isReady ? "pending" : documentsPrepared ? "completed" : "active",
    },
    {
      id: "approval",
      status: !isReady || !documentsPrepared ? "pending" : approved ? "completed" : "active",
    },
    {
      id: "generation",
      status: !approved ? "pending" : generated ? "completed" : "active",
    },
  ];
}
