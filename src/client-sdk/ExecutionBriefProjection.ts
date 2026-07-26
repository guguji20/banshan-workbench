import type { ExecutionBriefDomainEvent } from "../generated/bsaigc/ExecutionBriefDomainEvent";
import type { ExecutionBriefRecord } from "../generated/bsaigc/ExecutionBriefRecord";

const DEFAULT_EVENT_LIMIT = 80;

export interface ExecutionBriefProjectionSnapshot {
  readonly executionBriefs: readonly ExecutionBriefRecord[];
  readonly events: readonly ExecutionBriefDomainEvent[];
  readonly lastSequence: number;
}

/** In-memory read model. Durable execution brief state remains owned by the host. */
export class ExecutionBriefProjection {
  private readonly executionBriefsById = new Map<
    string,
    ExecutionBriefRecord
  >();
  private readonly pendingEvents = new Map<
    number,
    ExecutionBriefDomainEvent
  >();
  private readonly eventLimit: number;
  private recentEvents: ExecutionBriefDomainEvent[] = [];
  private currentSnapshot: ExecutionBriefProjectionSnapshot = {
    executionBriefs: [],
    events: [],
    lastSequence: 0,
  };

  constructor(eventLimit = DEFAULT_EVENT_LIMIT) {
    if (!Number.isInteger(eventLimit) || eventLimit < 1) {
      throw new RangeError("eventLimit must be a positive integer");
    }
    this.eventLimit = eventLimit;
  }

  hydrate(records: readonly ExecutionBriefRecord[]): boolean {
    let changed = false;
    for (const record of records) {
      changed = this.upsertRecord(record) || changed;
    }
    if (changed) this.rebuild();
    return changed;
  }

  upsert(record: ExecutionBriefRecord): boolean {
    if (!this.upsertRecord(record)) return false;
    this.rebuild();
    return true;
  }

  applyEvent(event: ExecutionBriefDomainEvent): boolean {
    if (
      event.sequence <= this.currentSnapshot.lastSequence ||
      this.pendingEvents.has(event.sequence)
    ) {
      return false;
    }
    this.pendingEvents.set(event.sequence, event);

    let contiguousSequence = this.currentSnapshot.lastSequence;
    while (true) {
      const contiguousEvent = this.pendingEvents.get(contiguousSequence + 1);
      if (!contiguousEvent) break;

      this.pendingEvents.delete(contiguousEvent.sequence);
      this.upsertRecord(contiguousEvent.executionBrief);
      this.recentEvents.push(contiguousEvent);
      contiguousSequence += 1;
    }

    if (contiguousSequence !== this.currentSnapshot.lastSequence) {
      this.recentEvents.sort(compareEvents);
      if (this.recentEvents.length > this.eventLimit) {
        this.recentEvents = this.recentEvents.slice(-this.eventLimit);
      }
      this.rebuild(contiguousSequence);
    }
    return true;
  }

  snapshot(): ExecutionBriefProjectionSnapshot {
    return this.currentSnapshot;
  }

  private upsertRecord(record: ExecutionBriefRecord): boolean {
    const current = this.executionBriefsById.get(record.id);
    if (current && record.revision <= current.revision) return false;
    this.executionBriefsById.set(record.id, record);
    return true;
  }

  private rebuild(lastSequence = this.currentSnapshot.lastSequence): void {
    this.currentSnapshot = {
      executionBriefs: [...this.executionBriefsById.values()].sort(
        compareExecutionBriefs,
      ),
      events: [...this.recentEvents],
      lastSequence,
    };
  }
}

function compareExecutionBriefs(
  left: ExecutionBriefRecord,
  right: ExecutionBriefRecord,
): number {
  return right.updatedAt - left.updatedAt || left.id.localeCompare(right.id);
}

function compareEvents(
  left: ExecutionBriefDomainEvent,
  right: ExecutionBriefDomainEvent,
): number {
  return (
    left.sequence - right.sequence ||
    left.occurredAt - right.occurredAt ||
    left.eventId.localeCompare(right.eventId)
  );
}
