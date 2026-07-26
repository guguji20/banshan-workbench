import { describe, expect, it } from "vitest";
import type { CaseDomainEvent } from "../generated/bsaigc/CaseDomainEvent";
import type { CaseRecord } from "../generated/bsaigc/CaseRecord";
import { CaseProjection } from "./CaseProjection";

function caseRecord(
  id: string,
  revision: number,
  updatedAt = revision,
): CaseRecord {
  return {
    id,
    assetId: `asset-${id}`,
    projectId: null,
    title: `Case ${id} r${revision}`,
    clientName: "Client",
    contentType: "brand",
    presentation: "liveAction",
    hasActors: false,
    isAigc: false,
    qualityTier: "reference",
    tags: [],
    notes: "",
    revision,
    createdAt: 1,
    updatedAt,
  };
}

function event(
  sequence: number,
  revision = sequence,
  id = "case-1",
): CaseDomainEvent {
  const record = caseRecord(id, revision);
  return {
    sequence,
    eventId: `event-${sequence}-${id}`,
    eventType: revision === 1 ? "case.created" : "case.updated",
    aggregateId: id,
    revision,
    occurredAt: sequence,
    traceId: `trace-${sequence}`,
    caseRecord: record,
  };
}

describe("CaseProjection", () => {
  it("hydrates and upserts only higher revisions with stable case sorting", () => {
    const projection = new CaseProjection();

    expect(
      projection.hydrate([
        caseRecord("case-c", 1, 10),
        caseRecord("case-b", 1, 20),
        caseRecord("case-a", 1, 20),
      ]),
    ).toBe(true);
    expect(projection.upsert(caseRecord("case-b", 2, 30))).toBe(true);
    expect(projection.upsert(caseRecord("case-b", 1, 99))).toBe(false);

    const snapshot = projection.snapshot();
    expect(snapshot.cases.map(({ id }) => id)).toEqual([
      "case-b",
      "case-a",
      "case-c",
    ]);
    expect(snapshot.cases[0]).toMatchObject({ revision: 2, updatedAt: 30 });
  });

  it("keeps the replay cursor contiguous while accepting out-of-order events", () => {
    const projection = new CaseProjection();

    expect(projection.applyEvent(event(5, 5))).toBe(true);
    expect(projection.applyEvent(event(3, 3))).toBe(true);

    expect(projection.snapshot().lastSequence).toBe(0);
    for (const sequence of [1, 2, 4]) {
      expect(projection.applyEvent(event(sequence, sequence))).toBe(true);
    }

    const snapshot = projection.snapshot();
    expect(snapshot.cases[0]?.revision).toBe(5);
    expect(snapshot.events.map(({ sequence }) => sequence)).toEqual([
      1, 2, 3, 4, 5,
    ]);
    expect(snapshot.lastSequence).toBe(5);
  });

  it("deduplicates by sequence before a duplicate can mutate state", () => {
    const projection = new CaseProjection();
    const original = event(1, 1, "case-original");

    expect(projection.applyEvent(original)).toBe(true);
    expect(
      projection.applyEvent({
        ...event(1, 99, "case-duplicate"),
        eventId: "different-event-id",
      }),
    ).toBe(false);

    const snapshot = projection.snapshot();
    expect(snapshot.events).toEqual([original]);
    expect(snapshot.cases.map(({ id }) => id)).toEqual(["case-original"]);
    expect(snapshot.lastSequence).toBe(1);
  });

  it("deduplicates a buffered future event before the gap is filled", () => {
    const projection = new CaseProjection();
    const future = event(3, 3);

    expect(projection.applyEvent(future)).toBe(true);
    expect(projection.applyEvent({ ...future, eventId: "duplicate" })).toBe(
      false,
    );
    expect(projection.snapshot().lastSequence).toBe(0);

    projection.applyEvent(event(1, 1));
    projection.applyEvent(event(2, 2));
    expect(projection.snapshot().lastSequence).toBe(3);
    expect(projection.snapshot().events).toHaveLength(3);
  });

  it("projects new cases from events and sorts equal sequences deterministically", () => {
    const projection = new CaseProjection();
    const second = { ...event(2, 1, "case-b"), occurredAt: 20 };
    const first = { ...event(1, 1, "case-a"), occurredAt: 10 };

    projection.applyEvent(second);
    projection.applyEvent(first);

    const snapshot = projection.snapshot();
    expect(snapshot.cases.map(({ id }) => id)).toEqual(["case-a", "case-b"]);
    expect(snapshot.events.map(({ sequence }) => sequence)).toEqual([1, 2]);
  });

  it("retains only the configured number of newest sequenced events", () => {
    const projection = new CaseProjection(3);

    for (let sequence = 1; sequence <= 5; sequence += 1) {
      projection.applyEvent(event(sequence));
    }

    const snapshot = projection.snapshot();
    expect(snapshot.events.map(({ sequence }) => sequence)).toEqual([3, 4, 5]);
    expect(snapshot.cases[0]?.revision).toBe(5);
    expect(snapshot.lastSequence).toBe(5);
  });
});
