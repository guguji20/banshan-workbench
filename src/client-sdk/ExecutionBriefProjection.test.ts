import { describe, expect, it } from "vitest";
import type { ExecutionBriefContent } from "../generated/bsaigc/ExecutionBriefContent";
import type { ExecutionBriefDomainEvent } from "../generated/bsaigc/ExecutionBriefDomainEvent";
import type { ExecutionBriefRecord } from "../generated/bsaigc/ExecutionBriefRecord";
import { ExecutionBriefProjection } from "./ExecutionBriefProjection";

const CONTENT: ExecutionBriefContent = {
  shootAt: null,
  clientGoal: "Launch campaign",
  visualStyle: "Documentary",
  primaryShots: ["Hero shot"],
  secondaryShots: [],
  requiredShots: ["Product close-up"],
  fallbackShots: [],
  riskPoints: [],
  waitingTimeActions: [],
  equipmentNotes: "",
  postShootHighlights: [],
};

function executionBrief(
  id: string,
  revision: number,
  updatedAt = revision,
): ExecutionBriefRecord {
  return {
    id,
    projectId: `project-${id}`,
    content: { ...CONTENT, clientGoal: `${CONTENT.clientGoal} r${revision}` },
    status: revision > 1 ? "ready" : "draft",
    revision,
    createdAt: 1,
    updatedAt,
  };
}

function event(
  sequence: number,
  revision = sequence,
  id = "brief-1",
): ExecutionBriefDomainEvent {
  return {
    sequence,
    eventId: `event-${sequence}-${id}`,
    eventType:
      revision === 1
        ? "executionBrief.created"
        : "executionBrief.updated",
    aggregateId: id,
    revision,
    occurredAt: sequence,
    traceId: `trace-${sequence}`,
    executionBrief: executionBrief(id, revision),
  };
}

describe("ExecutionBriefProjection", () => {
  it("hydrates and upserts only higher revisions with stable sorting", () => {
    const projection = new ExecutionBriefProjection();

    expect(
      projection.hydrate([
        executionBrief("brief-c", 1, 10),
        executionBrief("brief-b", 1, 20),
        executionBrief("brief-a", 1, 20),
      ]),
    ).toBe(true);
    expect(projection.upsert(executionBrief("brief-b", 2, 30))).toBe(true);
    expect(projection.upsert(executionBrief("brief-b", 1, 99))).toBe(false);

    const snapshot = projection.snapshot();
    expect(snapshot.executionBriefs.map(({ id }) => id)).toEqual([
      "brief-b",
      "brief-a",
      "brief-c",
    ]);
    expect(snapshot.executionBriefs[0]).toMatchObject({
      revision: 2,
      updatedAt: 30,
    });
  });

  it("keeps a contiguous cursor while buffering out-of-order events", () => {
    const projection = new ExecutionBriefProjection();

    expect(projection.applyEvent(event(5))).toBe(true);
    expect(projection.applyEvent(event(3))).toBe(true);
    expect(projection.snapshot()).toMatchObject({
      executionBriefs: [],
      events: [],
      lastSequence: 0,
    });

    expect(projection.applyEvent(event(1))).toBe(true);
    expect(projection.snapshot().lastSequence).toBe(1);
    expect(projection.snapshot().executionBriefs[0]?.revision).toBe(1);
    expect(projection.snapshot().events.map(({ sequence }) => sequence)).toEqual([
      1,
    ]);

    for (const sequence of [2, 4]) {
      expect(projection.applyEvent(event(sequence))).toBe(true);
    }

    const snapshot = projection.snapshot();
    expect(snapshot.lastSequence).toBe(5);
    expect(snapshot.executionBriefs[0]?.revision).toBe(5);
    expect(snapshot.events.map(({ sequence }) => sequence)).toEqual([
      1, 2, 3, 4, 5,
    ]);
  });

  it("deduplicates both applied and buffered sequences before mutation", () => {
    const projection = new ExecutionBriefProjection();
    const first = event(1, 1, "brief-original");
    const future = event(3, 3, "brief-future");

    expect(projection.applyEvent(first)).toBe(true);
    expect(
      projection.applyEvent({
        ...event(1, 99, "brief-duplicate"),
        eventId: "different-event-id",
      }),
    ).toBe(false);
    expect(projection.applyEvent(future)).toBe(true);
    expect(projection.applyEvent({ ...future, eventId: "duplicate" })).toBe(
      false,
    );

    projection.applyEvent(event(2, 2, "brief-original"));
    const snapshot = projection.snapshot();
    expect(snapshot.lastSequence).toBe(3);
    expect(snapshot.events).toHaveLength(3);
    expect(snapshot.executionBriefs.some(({ id }) => id === "brief-duplicate")).toBe(
      false,
    );
  });

  it("retains only the configured number of newest sequenced events", () => {
    const projection = new ExecutionBriefProjection(3);

    for (let sequence = 1; sequence <= 5; sequence += 1) {
      projection.applyEvent(event(sequence));
    }

    const snapshot = projection.snapshot();
    expect(snapshot.events.map(({ sequence }) => sequence)).toEqual([3, 4, 5]);
    expect(snapshot.executionBriefs[0]?.revision).toBe(5);
    expect(snapshot.lastSequence).toBe(5);
  });
});
