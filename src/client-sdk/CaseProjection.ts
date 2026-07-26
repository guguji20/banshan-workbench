import type { CaseDomainEvent } from "../generated/bsaigc/CaseDomainEvent";
import type { CaseRecord } from "../generated/bsaigc/CaseRecord";

const DEFAULT_EVENT_LIMIT = 80;

export interface CaseProjectionSnapshot {
  readonly cases: readonly CaseRecord[];
  readonly events: readonly CaseDomainEvent[];
  readonly lastSequence: number;
}

/** In-memory read model. Durable case state remains owned by the host. */
export class CaseProjection {
  private readonly casesById = new Map<string, CaseRecord>();
  private readonly pendingSequences = new Set<number>();
  private readonly eventLimit: number;
  private recentEvents: CaseDomainEvent[] = [];
  private currentSnapshot: CaseProjectionSnapshot = {
    cases: [],
    events: [],
    lastSequence: 0,
  };

  constructor(eventLimit = DEFAULT_EVENT_LIMIT) {
    if (!Number.isInteger(eventLimit) || eventLimit < 1) {
      throw new RangeError("eventLimit must be a positive integer");
    }
    this.eventLimit = eventLimit;
  }

  hydrate(records: readonly CaseRecord[]): boolean {
    let changed = false;
    for (const record of records) {
      changed = this.upsertRecord(record) || changed;
    }
    if (changed) this.rebuild();
    return changed;
  }

  upsert(record: CaseRecord): boolean {
    if (!this.upsertRecord(record)) return false;
    this.rebuild();
    return true;
  }

  applyEvent(event: CaseDomainEvent): boolean {
    if (
      event.sequence <= this.currentSnapshot.lastSequence ||
      this.pendingSequences.has(event.sequence)
    ) {
      return false;
    }
    this.pendingSequences.add(event.sequence);

    this.upsertRecord(event.caseRecord);
    this.recentEvents.push(event);
    this.recentEvents.sort(compareEvents);
    if (this.recentEvents.length > this.eventLimit) {
      this.recentEvents = this.recentEvents.slice(-this.eventLimit);
    }

    let contiguousSequence = this.currentSnapshot.lastSequence;
    while (this.pendingSequences.delete(contiguousSequence + 1)) {
      contiguousSequence += 1;
    }

    this.rebuild(contiguousSequence);
    return true;
  }

  snapshot(): CaseProjectionSnapshot {
    return this.currentSnapshot;
  }

  private upsertRecord(record: CaseRecord): boolean {
    const current = this.casesById.get(record.id);
    if (current && record.revision <= current.revision) return false;
    this.casesById.set(record.id, record);
    return true;
  }

  private rebuild(lastSequence = this.currentSnapshot.lastSequence): void {
    this.currentSnapshot = {
      cases: [...this.casesById.values()].sort(compareCases),
      events: [...this.recentEvents],
      lastSequence,
    };
  }
}

function compareCases(left: CaseRecord, right: CaseRecord): number {
  return right.updatedAt - left.updatedAt || left.id.localeCompare(right.id);
}

function compareEvents(left: CaseDomainEvent, right: CaseDomainEvent): number {
  return (
    left.sequence - right.sequence ||
    left.occurredAt - right.occurredAt ||
    left.eventId.localeCompare(right.eventId)
  );
}
