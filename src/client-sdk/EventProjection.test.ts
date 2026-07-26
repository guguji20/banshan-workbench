import { describe, expect, it } from "vitest";
import type { BriefRecord } from "../generated/bsaigc/BriefRecord";
import type { DomainEvent } from "../generated/bsaigc/DomainEvent";
import type { ProjectRecord } from "../generated/bsaigc/ProjectRecord";
import { EventProjection } from "./EventProjection";

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

function project(id: string, revision: number): ProjectRecord {
  return {
    id,
    name: `Project ${id} r${revision}`,
    clientName: "Client",
    brief: EMPTY_BRIEF,
    stage: revision > 1 ? "creative" : "intake",
    revision,
    createdAt: 1,
    updatedAt: revision,
  };
}

function event(sequence: number, revision = sequence): DomainEvent {
  const record = project("project-1", revision);
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

describe("EventProjection", () => {
  it("hydrates projects and keeps the higher revision during out-of-order replay", () => {
    const projection = new EventProjection();
    projection.hydrateProjects([project("project-1", 1), project("project-2", 1)]);

    projection.applyEvent(event(3, 3));
    projection.applyEvent(event(2, 2));

    const snapshot = projection.snapshot();
    expect(snapshot.projects).toHaveLength(2);
    expect(snapshot.projects.find(({ id }) => id === "project-1")?.revision).toBe(3);
    expect(snapshot.events.map(({ sequence }) => sequence)).toEqual([2, 3]);
    expect(snapshot.lastSequence).toBe(3);
  });

  it("deduplicates by eventId without allowing duplicate delivery into the log", () => {
    const projection = new EventProjection();
    const original = event(1, 1);

    expect(projection.applyEvent(original)).toBe(true);
    expect(projection.applyEvent({ ...original, sequence: 99 })).toBe(false);

    expect(projection.snapshot().events).toEqual([original]);
    expect(projection.snapshot().lastSequence).toBe(1);
  });

  it("retains only the latest 80 events while preserving the highest project revision", () => {
    const projection = new EventProjection();

    for (let sequence = 1; sequence <= 85; sequence += 1) {
      projection.applyEvent(event(sequence));
    }
    projection.applyEvent({ ...event(4, 4), eventId: "late-event-4" });

    const snapshot = projection.snapshot();
    expect(snapshot.events).toHaveLength(80);
    expect(snapshot.events[0]?.sequence).toBe(6);
    expect(snapshot.events[snapshot.events.length - 1]?.sequence).toBe(85);
    expect(snapshot.projects[0]?.revision).toBe(85);
    expect(snapshot.lastSequence).toBe(85);
  });

  it("returns a stable snapshot until projection state changes", () => {
    const projection = new EventProjection();
    const initial = projection.snapshot();

    expect(projection.snapshot()).toBe(initial);
    projection.hydrateProjects([project("project-1", 1)]);
    expect(projection.snapshot()).not.toBe(initial);
    expect(projection.snapshot()).toBe(projection.snapshot());
  });
});
