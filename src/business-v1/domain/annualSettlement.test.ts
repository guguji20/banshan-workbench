import { describe, expect, it } from "vitest";
import {
  calculateAnnualSettlement,
  upsertAnnualSettlementBatch,
  validateAnnualSettlementBatch,
  type AnnualSettlementBatch,
  type AnnualSettlementCadence,
  type AnnualSettlementDeliverable,
} from "./index";

function deliverable(
  id: string,
  overrides: Partial<AnnualSettlementDeliverable> = {},
): AnnualSettlementDeliverable {
  return {
    id,
    contractQuantity: 10,
    executedQuantityBeforePeriod: 4,
    currentExecutedQuantity: 3,
    acceptedQuantityBeforePeriod: 3,
    currentAcceptedQuantity: 3,
    ...overrides,
  };
}

function batch(
  id: string,
  deliverableId: string,
  settlementQuantity: number,
  overrides: Partial<AnnualSettlementBatch> = {},
): AnnualSettlementBatch {
  return {
    id,
    cadence: "quarterly",
    status: "confirmed",
    lines: [{ deliverableId, settlementQuantity }],
    ...overrides,
  };
}

describe("annual settlement cadence and calculations", () => {
  it("supports monthly, quarterly, per-order, one-off, and mixed settlement", () => {
    const cadences: readonly AnnualSettlementCadence[] = [
      "monthly",
      "quarterly",
      "perOrder",
      "oneOff",
      "mixed",
    ];

    cadences.forEach((cadence) => {
      expect(() =>
        validateAnnualSettlementBatch(
          batch(`batch-${cadence}`, `deliverable-${cadence}`, 1, { cadence }),
        ),
      ).not.toThrow();
    });
  });

  it("calculates contract, current, cumulative, settled, and remaining quantities", () => {
    const result = calculateAnnualSettlement(
      [deliverable("production")],
      [batch("settlement-q1", "production", 5)],
      "settlement-q1",
    );

    expect(result).toEqual([
      {
        deliverableId: "production",
        contractQuantity: 10,
        cumulativeExecutedQuantity: 7,
        currentExecutedQuantity: 3,
        cumulativeAcceptedQuantity: 6,
        currentAcceptedQuantity: 3,
        cumulativeSettledQuantity: 5,
        currentSettledQuantity: 5,
        remainingQuantity: 5,
      },
    ]);
  });

  it("rejects settlement beyond accepted quantity", () => {
    expect(() =>
      calculateAnnualSettlement(
        [deliverable("production")],
        [batch("settlement-q1", "production", 7)],
        "settlement-q1",
      ),
    ).toThrowError(/must not exceed cumulative accepted quantity/);
  });
});

describe("annual settlement batch reservation", () => {
  it("prevents one deliverable from appearing in two non-voided batches", () => {
    const q1 = batch("settlement-q1", "production", 5);
    const q2 = batch("settlement-q2", "production", 1);
    const batches = upsertAnnualSettlementBatch([], q1);

    expect(() => upsertAnnualSettlementBatch(batches, q2)).toThrowError(
      /already referenced by non-voided settlement batch settlement-q1/,
    );
  });

  it("releases a deliverable when its previous batch is voided", () => {
    const q1 = batch("settlement-q1", "production", 5);
    const voided = { ...q1, status: "voided" as const };
    const batches = upsertAnnualSettlementBatch(
      upsertAnnualSettlementBatch([], q1),
      voided,
    );
    const reassigned = upsertAnnualSettlementBatch(
      batches,
      batch("settlement-q2", "production", 5),
    );

    expect(reassigned.map((item) => item.id)).toEqual([
      "settlement-q1",
      "settlement-q2",
    ]);
  });

  it("runs two consecutive quarters and one standalone settlement without reuse", () => {
    const deliverables = [
      deliverable("delivery-q1", {
        contractQuantity: 10,
        executedQuantityBeforePeriod: 0,
        currentExecutedQuantity: 10,
        acceptedQuantityBeforePeriod: 0,
        currentAcceptedQuantity: 10,
      }),
      deliverable("delivery-q2", {
        contractQuantity: 20,
        executedQuantityBeforePeriod: 8,
        currentExecutedQuantity: 12,
        acceptedQuantityBeforePeriod: 8,
        currentAcceptedQuantity: 12,
      }),
      deliverable("delivery-standalone", {
        contractQuantity: 1,
        executedQuantityBeforePeriod: 0,
        currentExecutedQuantity: 1,
        acceptedQuantityBeforePeriod: 0,
        currentAcceptedQuantity: 1,
      }),
    ];

    const q1 = batch("settlement-2026-q1", "delivery-q1", 10);
    const q2 = batch("settlement-2026-q2", "delivery-q2", 20);
    const standalone = batch("settlement-one-off", "delivery-standalone", 1, {
      cadence: "oneOff",
    });
    const afterQ1 = upsertAnnualSettlementBatch([], q1);
    const afterQ2 = upsertAnnualSettlementBatch(afterQ1, q2);
    const completed = upsertAnnualSettlementBatch(afterQ2, standalone);

    const result = calculateAnnualSettlement(
      deliverables,
      completed,
      "settlement-one-off",
    );

    expect(completed).toHaveLength(3);
    expect(result.map((item) => ({
      deliverableId: item.deliverableId,
      cumulativeSettledQuantity: item.cumulativeSettledQuantity,
      currentSettledQuantity: item.currentSettledQuantity,
      remainingQuantity: item.remainingQuantity,
    }))).toEqual([
      {
        deliverableId: "delivery-q1",
        cumulativeSettledQuantity: 10,
        currentSettledQuantity: 0,
        remainingQuantity: 0,
      },
      {
        deliverableId: "delivery-q2",
        cumulativeSettledQuantity: 20,
        currentSettledQuantity: 0,
        remainingQuantity: 0,
      },
      {
        deliverableId: "delivery-standalone",
        cumulativeSettledQuantity: 1,
        currentSettledQuantity: 1,
        remainingQuantity: 0,
      },
    ]);
  });
});
