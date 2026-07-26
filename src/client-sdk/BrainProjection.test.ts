import { describe, expect, it } from "vitest";
import type { BrainStreamEvent } from "../generated/bsaigc/BrainStreamEvent";
import type { BrainThreadRecord } from "../generated/bsaigc/BrainThreadRecord";
import type { BrainThreadStatus } from "../generated/bsaigc/BrainThreadStatus";
import type { BrainTurnRecord } from "../generated/bsaigc/BrainTurnRecord";
import type { BrainTurnStatus } from "../generated/bsaigc/BrainTurnStatus";
import { BrainProjection } from "./BrainProjection";

const MAX_STREAM_CHARS = 2_000_000;

function thread(
  id: string,
  updatedAt: number,
  status: BrainThreadStatus = "ready",
): BrainThreadRecord {
  return {
    id,
    projectId: null,
    title: `Thread ${id}`,
    model: "gpt-test",
    status,
    createdAt: 1,
    updatedAt,
  };
}

function turn(
  id: string,
  threadId: string,
  createdAt: number,
  status: BrainTurnStatus = "running",
  updatedAt = createdAt,
): BrainTurnRecord {
  return {
    id,
    threadId,
    status,
    inputText: `Input ${id}`,
    assistantText: "",
    error: null,
    createdAt,
    updatedAt,
  };
}

function event(
  sequence: number,
  overrides: Partial<BrainStreamEvent> = {},
): BrainStreamEvent {
  return {
    sequence,
    eventType: "brain.unknown",
    threadId: null,
    turnId: null,
    itemId: null,
    delta: null,
    payload: null,
    occurredAt: sequence,
    ...overrides,
  };
}

describe("BrainProjection", () => {
  it("hydrates threads and turns with deterministic sorting", () => {
    const projection = new BrainProjection();
    projection.upsertThreads([thread("stale-thread", 99)]);
    projection.replaceThreads([
      thread("thread-c", 20),
      thread("thread-b", 10),
      thread("thread-a", 20),
    ]);
    projection.upsertThreads([thread("thread-b", 30, "archived")]);

    projection.upsertTurns([turn("turn-foreign", "thread-2", 5)]);
    projection.upsertTurns([turn("turn-stale", "thread-1", 1)]);
    projection.replaceTurns("thread-1", [
      turn("turn-late", "thread-1", 20),
      turn("turn-b", "thread-1", 10),
      turn("turn-a", "thread-1", 10),
    ]);

    const snapshot = projection.snapshot();
    expect(snapshot.threads.map(({ id }) => id)).toEqual([
      "thread-b",
      "thread-a",
      "thread-c",
    ]);
    expect(snapshot.threads.find(({ id }) => id === "thread-b")?.status).toBe(
      "archived",
    );
    expect(snapshot.threads.some(({ id }) => id === "stale-thread")).toBe(false);
    expect(snapshot.turns.map(({ id }) => id)).toEqual([
      "turn-foreign",
      "turn-a",
      "turn-b",
      "turn-late",
    ]);
    expect(snapshot.turns.some(({ id }) => id === "turn-stale")).toBe(false);
  });

  it("accumulates agent deltas while retaining only the bounded suffix", () => {
    const projection = new BrainProjection();
    projection.upsertTurns([turn("turn-1", "thread-1", 1)]);

    projection.applyEvent(
      event(1, {
        eventType: "brain.agentMessageDelta",
        turnId: "turn-1",
        delta: "discard-me",
      }),
    );
    projection.applyEvent(
      event(2, {
        eventType: "brain.agentMessageDelta",
        turnId: "turn-1",
        delta: `${"x".repeat(MAX_STREAM_CHARS - 3)}END`,
      }),
    );

    const bounded = projection.snapshot().streamingByTurn["turn-1"];
    expect(bounded).toHaveLength(MAX_STREAM_CHARS);
    expect(bounded?.startsWith("x")).toBe(true);
    expect(bounded?.includes("discard-me")).toBe(false);
    expect(bounded?.endsWith("END")).toBe(true);

    projection.applyEvent(
      event(3, {
        eventType: "brain.agentMessageDelta",
        turnId: "turn-1",
        delta: "TAIL",
      }),
    );
    expect(projection.snapshot().streamingByTurn["turn-1"]).toHaveLength(
      MAX_STREAM_CHARS,
    );
    expect(projection.snapshot().streamingByTurn["turn-1"]?.endsWith("TAIL")).toBe(
      true,
    );
  });

  it("ignores out-of-order and duplicate events without mutating state", () => {
    const projection = new BrainProjection();
    projection.upsertTurns([turn("turn-1", "thread-1", 1)]);

    const newest = event(10, {
      eventType: "brain.agentMessageDelta",
      turnId: "turn-1",
      delta: "newest",
    });
    expect(projection.applyEvent(newest)).toBe(true);
    expect(
      projection.applyEvent(
        event(9, {
          eventType: "brain.turnCompleted",
          turnId: "turn-1",
          payload: { status: "completed" },
        }),
      ),
    ).toBe(false);
    expect(
      projection.applyEvent({ ...newest, delta: "duplicate-mutation" }),
    ).toBe(false);

    const snapshot = projection.snapshot();
    expect(snapshot.turns[0]?.status).toBe("running");
    expect(snapshot.streamingByTurn["turn-1"]).toBe("newest");
    expect(snapshot.lastEvent).toEqual(newest);
  });

  it("updates known thread and turn statuses from lifecycle events", () => {
    const projection = new BrainProjection();
    projection.upsertThreads([thread("thread-1", 5)]);
    projection.upsertTurns([turn("turn-1", "thread-1", 2)]);

    projection.applyEvent(
      event(1, {
        eventType: "brain.turnStarted",
        threadId: "thread-1",
        turnId: "turn-1",
        occurredAt: 10,
      }),
    );
    projection.applyEvent(
      event(2, {
        eventType: "brain.threadStatusChanged",
        threadId: "thread-1",
        payload: { status: "archived" },
        occurredAt: 20,
      }),
    );
    projection.applyEvent(
      event(3, {
        eventType: "brain.turnCompleted",
        turnId: "turn-1",
        payload: { status: "completed" },
        occurredAt: 30,
      }),
    );

    const snapshot = projection.snapshot();
    expect(snapshot.threads[0]).toMatchObject({
      id: "thread-1",
      status: "archived",
      updatedAt: 20,
    });
    expect(snapshot.turns[0]).toMatchObject({
      id: "turn-1",
      status: "completed",
      updatedAt: 30,
    });
  });

  it.each(["completed", "interrupted", "failed"] as const)(
    "clears streaming text when a %s turn is hydrated",
    (status) => {
      const projection = new BrainProjection();
      projection.upsertTurns([turn("turn-1", "thread-1", 1)]);
      projection.applyEvent(
        event(1, {
          eventType: "brain.agentMessageDelta",
          turnId: "turn-1",
          delta: "temporary stream",
        }),
      );
      expect(projection.snapshot().streamingByTurn["turn-1"]).toBe(
        "temporary stream",
      );

      projection.upsertTurns([turn("turn-1", "thread-1", 1, status, 2)]);

      expect(projection.snapshot().turns[0]?.status).toBe(status);
      expect(projection.snapshot().streamingByTurn["turn-1"]).toBeUndefined();
    },
  );

  it("handles unknown and malformed payloads without changing records", () => {
    const projection = new BrainProjection();
    projection.upsertThreads([thread("thread-1", 100)]);
    projection.upsertTurns([turn("turn-1", "thread-1", 1, "running", 100)]);
    const payloads: unknown[] = [
      null,
      true,
      42,
      "status",
      [],
      {},
      { status: 42 },
      { status: null },
      { status: "unknown-status" },
    ];

    expect(() => {
      payloads.forEach((payload, index) => {
        projection.applyEvent(
          event(index + 1, {
            eventType: "brain.futureEvent",
            threadId: "thread-1",
            turnId: "turn-1",
            payload,
          }),
        );
      });
    }).not.toThrow();

    const snapshot = projection.snapshot();
    expect(snapshot.threads[0]).toMatchObject({ status: "ready", updatedAt: 100 });
    expect(snapshot.turns[0]).toMatchObject({ status: "running", updatedAt: 100 });
    expect(snapshot.lastEvent?.sequence).toBe(payloads.length);
  });
});
