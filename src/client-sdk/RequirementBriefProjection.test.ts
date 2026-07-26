import { describe, expect, it } from "vitest";
import type { RequirementBriefContent } from "../generated/bsaigc/RequirementBriefContent";
import type { RequirementBriefDomainEvent } from "../generated/bsaigc/RequirementBriefDomainEvent";
import type { RequirementBriefRecord } from "../generated/bsaigc/RequirementBriefRecord";
import { RequirementBriefProjection } from "./RequirementBriefProjection";

const CONTENT: RequirementBriefContent = {
  objective: "Launch campaign",
  audience: "Creative teams",
  keyMessage: "Ship with confidence",
  deliverables: ["Launch film"],
  channels: ["Web"],
  styleKeywords: ["Documentary"],
  mandatoryItems: ["Brand mark"],
  constraints: [],
  acceptanceCriteria: ["Approved master"],
  risks: [],
  deadlineAt: null,
  budgetNotes: "",
  referenceCaseIds: [],
  referenceNotes: "",
};

function requirementBrief(
  id: string,
  revision: number,
  updatedAt = revision,
): RequirementBriefRecord {
  return {
    id,
    projectId: `project-${id}`,
    questionSetVersion: "1.0",
    answers: [],
    content: { ...CONTENT, objective: `${CONTENT.objective} r${revision}` },
    status: revision > 1 ? "review" : "interviewing",
    confirmedAt: null,
    confirmedBy: null,
    revision,
    createdAt: 1,
    updatedAt,
  };
}

function event(
  sequence: number,
  revision = sequence,
  id = "requirement-1",
): RequirementBriefDomainEvent {
  return {
    sequence,
    eventId: `event-${sequence}-${id}`,
    eventType:
      revision === 1
        ? "requirementBrief.created"
        : "requirementBrief.updated",
    aggregateId: id,
    revision,
    occurredAt: sequence,
    traceId: `trace-${sequence}`,
    requirementBrief: requirementBrief(id, revision),
  };
}

describe("RequirementBriefProjection", () => {
  it("hydrates and upserts only higher revisions with stable sorting", () => {
    const projection = new RequirementBriefProjection();

    expect(
      projection.hydrate([
        requirementBrief("brief-c", 1, 10),
        requirementBrief("brief-b", 1, 20),
        requirementBrief("brief-a", 1, 20),
      ]),
    ).toBe(true);
    expect(projection.upsert(requirementBrief("brief-b", 2, 30))).toBe(true);
    expect(projection.upsert(requirementBrief("brief-b", 1, 99))).toBe(false);

    const snapshot = projection.snapshot();
    expect(snapshot.requirementBriefs.map(({ id }) => id)).toEqual([
      "brief-b",
      "brief-a",
      "brief-c",
    ]);
    expect(snapshot.requirementBriefs[0]).toMatchObject({
      revision: 2,
      updatedAt: 30,
    });
  });

  it("does not publish an event or record before its sequence gap closes", () => {
    const projection = new RequirementBriefProjection();

    expect(projection.applyEvent(event(3))).toBe(true);
    expect(projection.applyEvent(event(2))).toBe(true);
    expect(projection.snapshot()).toEqual({
      requirementBriefs: [],
      events: [],
      lastSequence: 0,
    });

    expect(projection.applyEvent(event(1))).toBe(true);
    expect(projection.snapshot().lastSequence).toBe(3);
    expect(projection.snapshot().requirementBriefs[0]?.revision).toBe(3);
    expect(projection.snapshot().events.map(({ sequence }) => sequence)).toEqual([
      1, 2, 3,
    ]);
  });

  it("deduplicates both applied and buffered sequences before mutation", () => {
    const projection = new RequirementBriefProjection();
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
    expect(
      snapshot.requirementBriefs.some(({ id }) => id === "brief-duplicate"),
    ).toBe(false);
  });

  it("retains only the configured number of newest sequenced events", () => {
    const projection = new RequirementBriefProjection(3);

    for (let sequence = 1; sequence <= 5; sequence += 1) {
      projection.applyEvent(event(sequence));
    }

    const snapshot = projection.snapshot();
    expect(snapshot.events.map(({ sequence }) => sequence)).toEqual([3, 4, 5]);
    expect(snapshot.requirementBriefs[0]?.revision).toBe(5);
    expect(snapshot.lastSequence).toBe(5);
  });
});
