import { describe, expect, it } from "vitest";
import type { AssetDomainEvent } from "../generated/bsaigc/AssetDomainEvent";
import type { AssetRecord } from "../generated/bsaigc/AssetRecord";
import type { TaskDomainEvent } from "../generated/bsaigc/TaskDomainEvent";
import type { TaskRecord } from "../generated/bsaigc/TaskRecord";
import { AssetProjection } from "./AssetProjection";
import { TaskProjection } from "./TaskProjection";

function task(revision: number, status: TaskRecord["status"]): TaskRecord {
  return {
    id: "task-1",
    kind: "system.noop",
    projectId: null,
    input: {},
    output: null,
    status,
    priority: "normal",
    replayPolicy: "safe",
    progress: 0,
    attempt: 0,
    maxAttempts: 2,
    revision,
    createdAt: 1,
    updatedAt: revision,
    startedAt: null,
    finishedAt: null,
    lastError: null,
    dependencies: [],
  };
}

function taskEvent(sequence: number, revision: number): TaskDomainEvent {
  return {
    sequence,
    eventId: `task-event-${sequence}`,
    eventType: revision === 1 ? "task.created" : "task.progressed",
    aggregateId: "task-1",
    revision,
    occurredAt: sequence,
    traceId: "trace",
    task: task(revision, revision > 1 ? "running" : "queued"),
  };
}

function asset(revision: number): AssetRecord {
  return {
    id: "asset-1",
    projectId: null,
    originalName: "reference.png",
    kind: "image",
    mimeType: "image/png",
    sizeBytes: 10,
    sha256: "a".repeat(64),
    status: "ready",
    revision,
    createdAt: 1,
    updatedAt: revision,
    previewAvailable: false,
  };
}

describe("module projections", () => {
  it("keeps the highest task revision under duplicate and out-of-order delivery", () => {
    const projection = new TaskProjection();
    projection.applyEvent(taskEvent(2, 2));
    projection.applyEvent(taskEvent(1, 1));
    projection.applyEvent(taskEvent(2, 2));
    expect(projection.snapshot().tasks[0]?.revision).toBe(2);
    expect(projection.snapshot().events).toHaveLength(2);
    expect(projection.snapshot().lastSequence).toBe(2);
  });

  it("hydrates assets and deduplicates import events", () => {
    const projection = new AssetProjection();
    projection.hydrate([asset(1)]);
    const event: AssetDomainEvent = {
      sequence: 1,
      eventId: "asset-event-1",
      eventType: "asset.imported",
      aggregateId: "asset-1",
      revision: 1,
      occurredAt: 1,
      traceId: "trace",
      asset: asset(1),
    };
    projection.applyEvent(event);
    projection.applyEvent(event);
    expect(projection.snapshot().assets).toHaveLength(1);
    expect(projection.snapshot().events).toHaveLength(1);
  });
});
