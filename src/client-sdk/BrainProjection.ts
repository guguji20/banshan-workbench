import type { BrainStreamEvent } from "../generated/bsaigc/BrainStreamEvent";
import type { BrainThreadRecord } from "../generated/bsaigc/BrainThreadRecord";
import type { BrainThreadStatus } from "../generated/bsaigc/BrainThreadStatus";
import type { BrainTurnRecord } from "../generated/bsaigc/BrainTurnRecord";
import type { BrainTurnStatus } from "../generated/bsaigc/BrainTurnStatus";

const MAX_STREAM_CHARS = 2_000_000;

export interface BrainProjectionSnapshot {
  readonly threads: readonly BrainThreadRecord[];
  readonly turns: readonly BrainTurnRecord[];
  readonly streamingByTurn: Readonly<Record<string, string>>;
  readonly lastEvent: BrainStreamEvent | null;
}

export class BrainProjection {
  private readonly threads = new Map<string, BrainThreadRecord>();
  private readonly turns = new Map<string, BrainTurnRecord>();
  private readonly streamingByTurn = new Map<string, string>();
  private lastEvent: BrainStreamEvent | null = null;

  replaceThreads(records: readonly BrainThreadRecord[]): void {
    this.threads.clear();
    this.upsertThreads(records);
  }

  upsertThreads(records: readonly BrainThreadRecord[]): void {
    for (const record of records) this.threads.set(record.id, record);
  }

  replaceTurns(threadId: string, records: readonly BrainTurnRecord[]): void {
    for (const [id, turn] of this.turns) {
      if (turn.threadId === threadId) this.turns.delete(id);
    }
    this.upsertTurns(records);
  }

  upsertTurns(records: readonly BrainTurnRecord[]): void {
    for (const record of records) {
      this.turns.set(record.id, record);
      if (record.status !== "running") this.streamingByTurn.delete(record.id);
    }
  }

  applyEvent(event: BrainStreamEvent): boolean {
    if (this.lastEvent && event.sequence <= this.lastEvent.sequence) return false;
    this.lastEvent = event;

    if (event.eventType === "brain.agentMessageDelta" && event.turnId && event.delta) {
      const current = this.streamingByTurn.get(event.turnId) ?? "";
      this.streamingByTurn.set(
        event.turnId,
        `${current}${event.delta}`.slice(-MAX_STREAM_CHARS),
      );
    }

    const threadStatus = payloadStatus<BrainThreadStatus>(event.payload, [
      "ready",
      "running",
      "error",
      "archived",
    ]);
    if (threadStatus && event.threadId) {
      this.updateThreadStatus(event.threadId, threadStatus, event.occurredAt);
    } else if (event.eventType === "brain.turnStarted" && event.threadId) {
      this.updateThreadStatus(event.threadId, "running", event.occurredAt);
    }

    const turnStatus = payloadStatus<BrainTurnStatus>(event.payload, [
      "running",
      "completed",
      "interrupted",
      "failed",
    ]);
    if (turnStatus && event.turnId) {
      this.updateTurnStatus(event.turnId, turnStatus, event.occurredAt);
    }
    return true;
  }

  clearStreaming(turnId: string): void {
    this.streamingByTurn.delete(turnId);
  }

  snapshot(): BrainProjectionSnapshot {
    const threads = [...this.threads.values()].sort(
      (left, right) => right.updatedAt - left.updatedAt || left.id.localeCompare(right.id),
    );
    const turns = [...this.turns.values()].sort(
      (left, right) => left.createdAt - right.createdAt || left.id.localeCompare(right.id),
    );
    return {
      threads,
      turns,
      streamingByTurn: Object.fromEntries(this.streamingByTurn),
      lastEvent: this.lastEvent,
    };
  }

  private updateThreadStatus(
    threadId: string,
    status: BrainThreadStatus,
    occurredAt: number,
  ): void {
    const record = this.threads.get(threadId);
    if (!record) return;
    this.threads.set(threadId, {
      ...record,
      status,
      updatedAt: Math.max(record.updatedAt, occurredAt),
    });
  }

  private updateTurnStatus(
    turnId: string,
    status: BrainTurnStatus,
    occurredAt: number,
  ): void {
    const record = this.turns.get(turnId);
    if (!record) return;
    this.turns.set(turnId, {
      ...record,
      status,
      updatedAt: Math.max(record.updatedAt, occurredAt),
    });
  }
}

function payloadStatus<T extends string>(
  payload: unknown,
  allowed: readonly T[],
): T | null {
  if (!payload || typeof payload !== "object" || !("status" in payload)) return null;
  const status = (payload as { status?: unknown }).status;
  return typeof status === "string" && allowed.includes(status as T)
    ? (status as T)
    : null;
}
