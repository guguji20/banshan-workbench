import {
  assertSafeArithmetic,
  assertValid,
  nonNegativeInteger,
  positiveInteger,
  requiredText,
  type ValidationIssue,
} from "./validation";

export type AnnualSettlementCadence =
  | "monthly"
  | "quarterly"
  | "perOrder"
  | "oneOff"
  | "mixed";

export type AnnualSettlementBatchStatus =
  | "draft"
  | "confirmed"
  | "invoiced"
  | "paid"
  | "voided";

export interface AnnualSettlementDeliverable {
  id: string;
  contractQuantity: number;
  executedQuantityBeforePeriod: number;
  currentExecutedQuantity: number;
  acceptedQuantityBeforePeriod: number;
  currentAcceptedQuantity: number;
}

export interface AnnualSettlementBatchLine {
  deliverableId: string;
  settlementQuantity: number;
}

export interface AnnualSettlementBatch {
  id: string;
  cadence: AnnualSettlementCadence;
  status: AnnualSettlementBatchStatus;
  lines: readonly AnnualSettlementBatchLine[];
}

export interface AnnualSettlementCalculation {
  deliverableId: string;
  contractQuantity: number;
  cumulativeExecutedQuantity: number;
  currentExecutedQuantity: number;
  cumulativeAcceptedQuantity: number;
  currentAcceptedQuantity: number;
  cumulativeSettledQuantity: number;
  currentSettledQuantity: number;
  remainingQuantity: number;
}

const settlementCadences = new Set<AnnualSettlementCadence>([
  "monthly",
  "quarterly",
  "perOrder",
  "oneOff",
  "mixed",
]);

const settlementStatuses = new Set<AnnualSettlementBatchStatus>([
  "draft",
  "confirmed",
  "invoiced",
  "paid",
  "voided",
]);

export function validateAnnualSettlementBatch(batch: AnnualSettlementBatch): void {
  const issues: ValidationIssue[] = [];
  requiredText(batch.id, "annualSettlementBatch.id", issues);
  if (!settlementCadences.has(batch.cadence)) {
    issues.push({
      field: "annualSettlementBatch.cadence",
      code: "unsupported_cadence",
      message: "must be monthly, quarterly, perOrder, oneOff, or mixed",
    });
  }
  if (!settlementStatuses.has(batch.status)) {
    issues.push({
      field: "annualSettlementBatch.status",
      code: "unsupported_status",
      message: "is not supported",
    });
  }
  if (batch.lines.length === 0) {
    issues.push({
      field: "annualSettlementBatch.lines",
      code: "line_required",
      message: "must not be empty",
    });
  }

  const deliverableIds = new Set<string>();
  batch.lines.forEach((line, index) => {
    const field = `annualSettlementBatch.lines[${index}]`;
    requiredText(line.deliverableId, `${field}.deliverableId`, issues);
    positiveInteger(line.settlementQuantity, `${field}.settlementQuantity`, issues);
    if (deliverableIds.has(line.deliverableId)) {
      issues.push({
        field: `${field}.deliverableId`,
        code: "duplicate_deliverable",
        message: "must be unique within a settlement batch",
      });
    }
    deliverableIds.add(line.deliverableId);
  });

  assertValid(issues);
}

export function upsertAnnualSettlementBatch(
  batches: readonly AnnualSettlementBatch[],
  batch: AnnualSettlementBatch,
): readonly AnnualSettlementBatch[] {
  validateAnnualSettlementBatch(batch);
  const otherBatches = batches.filter((candidate) => candidate.id !== batch.id);
  validateAnnualSettlementBatches(otherBatches);
  assertDeliverablesAvailable(otherBatches, batch);

  const existingIndex = batches.findIndex((candidate) => candidate.id === batch.id);
  if (existingIndex === -1) {
    return [...batches, batch];
  }

  return batches.map((candidate, index) => (index === existingIndex ? batch : candidate));
}

export function calculateAnnualSettlement(
  deliverables: readonly AnnualSettlementDeliverable[],
  batches: readonly AnnualSettlementBatch[],
  currentBatchId?: string,
): readonly AnnualSettlementCalculation[] {
  validateAnnualSettlementBatches(batches);
  const deliverableMap = validateAnnualSettlementDeliverables(deliverables);
  const activeBatches = batches.filter((batch) => batch.status !== "voided");
  const settledByDeliverable = new Map<string, number>();
  const currentSettledByDeliverable = new Map<string, number>();

  activeBatches.forEach((batch) => {
    batch.lines.forEach((line) => {
      if (!deliverableMap.has(line.deliverableId)) {
        throwUnknownDeliverable(batch.id, line.deliverableId);
      }
      settledByDeliverable.set(
        line.deliverableId,
        assertSafeArithmetic(
          (settledByDeliverable.get(line.deliverableId) ?? 0) + line.settlementQuantity,
          `annualSettlement.${line.deliverableId}.cumulativeSettledQuantity`,
        ),
      );
      if (batch.id === currentBatchId) {
        currentSettledByDeliverable.set(line.deliverableId, line.settlementQuantity);
      }
    });
  });

  return deliverables.map((deliverable) => {
    const cumulativeExecutedQuantity = assertSafeArithmetic(
      deliverable.executedQuantityBeforePeriod + deliverable.currentExecutedQuantity,
      `annualSettlement.${deliverable.id}.cumulativeExecutedQuantity`,
    );
    const cumulativeAcceptedQuantity = assertSafeArithmetic(
      deliverable.acceptedQuantityBeforePeriod + deliverable.currentAcceptedQuantity,
      `annualSettlement.${deliverable.id}.cumulativeAcceptedQuantity`,
    );
    const cumulativeSettledQuantity = settledByDeliverable.get(deliverable.id) ?? 0;
    const issues: ValidationIssue[] = [];

    if (cumulativeExecutedQuantity > deliverable.contractQuantity) {
      issues.push({
        field: `annualSettlement.${deliverable.id}.cumulativeExecutedQuantity`,
        code: "contract_quantity_exceeded",
        message: "must not exceed contract quantity",
      });
    }
    if (cumulativeAcceptedQuantity > cumulativeExecutedQuantity) {
      issues.push({
        field: `annualSettlement.${deliverable.id}.cumulativeAcceptedQuantity`,
        code: "executed_quantity_exceeded",
        message: "must not exceed cumulative executed quantity",
      });
    }
    if (cumulativeSettledQuantity > cumulativeAcceptedQuantity) {
      issues.push({
        field: `annualSettlement.${deliverable.id}.cumulativeSettledQuantity`,
        code: "accepted_quantity_exceeded",
        message: "must not exceed cumulative accepted quantity",
      });
    }
    assertValid(issues);

    return Object.freeze({
      deliverableId: deliverable.id,
      contractQuantity: deliverable.contractQuantity,
      cumulativeExecutedQuantity,
      currentExecutedQuantity: deliverable.currentExecutedQuantity,
      cumulativeAcceptedQuantity,
      currentAcceptedQuantity: deliverable.currentAcceptedQuantity,
      cumulativeSettledQuantity,
      currentSettledQuantity: currentSettledByDeliverable.get(deliverable.id) ?? 0,
      remainingQuantity: assertSafeArithmetic(
        deliverable.contractQuantity - cumulativeSettledQuantity,
        `annualSettlement.${deliverable.id}.remainingQuantity`,
      ),
    });
  });
}

function validateAnnualSettlementDeliverables(
  deliverables: readonly AnnualSettlementDeliverable[],
): ReadonlyMap<string, AnnualSettlementDeliverable> {
  const issues: ValidationIssue[] = [];
  const deliverableMap = new Map<string, AnnualSettlementDeliverable>();

  if (deliverables.length === 0) {
    issues.push({
      field: "annualSettlement.deliverables",
      code: "deliverable_required",
      message: "must not be empty",
    });
  }

  deliverables.forEach((deliverable, index) => {
    const field = `annualSettlement.deliverables[${index}]`;
    requiredText(deliverable.id, `${field}.id`, issues);
    positiveInteger(deliverable.contractQuantity, `${field}.contractQuantity`, issues);
    nonNegativeInteger(
      deliverable.executedQuantityBeforePeriod,
      `${field}.executedQuantityBeforePeriod`,
      issues,
    );
    nonNegativeInteger(
      deliverable.currentExecutedQuantity,
      `${field}.currentExecutedQuantity`,
      issues,
    );
    nonNegativeInteger(
      deliverable.acceptedQuantityBeforePeriod,
      `${field}.acceptedQuantityBeforePeriod`,
      issues,
    );
    nonNegativeInteger(
      deliverable.currentAcceptedQuantity,
      `${field}.currentAcceptedQuantity`,
      issues,
    );
    if (deliverableMap.has(deliverable.id)) {
      issues.push({
        field: `${field}.id`,
        code: "duplicate_deliverable",
        message: "must be unique",
      });
    }
    deliverableMap.set(deliverable.id, deliverable);
  });

  assertValid(issues);
  return deliverableMap;
}

function validateAnnualSettlementBatches(
  batches: readonly AnnualSettlementBatch[],
): void {
  const batchIds = new Set<string>();
  const deliverableAssignments = new Map<string, string>();

  batches.forEach((batch) => {
    validateAnnualSettlementBatch(batch);
    if (batchIds.has(batch.id)) {
      throwDuplicateBatch(batch.id);
    }
    batchIds.add(batch.id);
    if (batch.status === "voided") return;

    batch.lines.forEach((line) => {
      const existingBatchId = deliverableAssignments.get(line.deliverableId);
      if (existingBatchId && existingBatchId !== batch.id) {
        throwAlreadySettled(line.deliverableId, existingBatchId);
      }
      deliverableAssignments.set(line.deliverableId, batch.id);
    });
  });
}

function assertDeliverablesAvailable(
  batches: readonly AnnualSettlementBatch[],
  incoming: AnnualSettlementBatch,
): void {
  if (incoming.status === "voided") return;
  const assignments = new Map<string, string>();
  batches.forEach((batch) => {
    if (batch.status === "voided") return;
    batch.lines.forEach((line) => assignments.set(line.deliverableId, batch.id));
  });
  incoming.lines.forEach((line) => {
    const existingBatchId = assignments.get(line.deliverableId);
    if (existingBatchId) {
      throwAlreadySettled(line.deliverableId, existingBatchId);
    }
  });
}

function throwAlreadySettled(deliverableId: string, batchId: string): never {
  assertValid([
    {
      field: `annualSettlement.deliverables.${deliverableId}`,
      code: "already_settled",
      message: `is already referenced by non-voided settlement batch ${batchId}`,
    },
  ]);
  throw new Error("unreachable");
}

function throwDuplicateBatch(batchId: string): never {
  assertValid([
    {
      field: "annualSettlementBatch.id",
      code: "duplicate_batch",
      message: `must be unique: ${batchId}`,
    },
  ]);
  throw new Error("unreachable");
}

function throwUnknownDeliverable(batchId: string, deliverableId: string): never {
  assertValid([
    {
      field: `annualSettlementBatch.${batchId}.deliverableId`,
      code: "unknown_deliverable",
      message: `references unknown deliverable ${deliverableId}`,
    },
  ]);
  throw new Error("unreachable");
}
