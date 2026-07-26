import type { RequirementBriefDomainEvent } from "../generated/bsaigc/RequirementBriefDomainEvent";
import type { RequirementBriefRecord } from "../generated/bsaigc/RequirementBriefRecord";

const DEFAULT_EVENT_LIMIT = 80;

export interface RequirementBriefProjectionSnapshot {
  readonly requirementBriefs: readonly RequirementBriefRecord[];
  readonly events: readonly RequirementBriefDomainEvent[];
  readonly lastSequence: number;
}

/** In-memory read model. Durable requirement brief state remains owned by the host. */
export class RequirementBriefProjection {
  private readonly requirementBriefsById = new Map<
    string,
    RequirementBriefRecord
  >();
  private readonly pendingEvents = new Map<
    number,
    RequirementBriefDomainEvent
  >();
  private readonly eventLimit: number;
  private recentEvents: RequirementBriefDomainEvent[] = [];
  private currentSnapshot: RequirementBriefProjectionSnapshot = {
    requirementBriefs: [],
    events: [],
    lastSequence: 0,
  };

  constructor(eventLimit = DEFAULT_EVENT_LIMIT) {
    if (!Number.isInteger(eventLimit) || eventLimit < 1) {
      throw new RangeError("eventLimit must be a positive integer");
    }
    this.eventLimit = eventLimit;
  }

  hydrate(records: readonly RequirementBriefRecord[]): boolean {
    let changed = false;
    for (const record of records) {
      changed = this.upsertRecord(record) || changed;
    }
    if (changed) this.rebuild();
    return changed;
  }

  upsert(record: RequirementBriefRecord): boolean {
    if (!this.upsertRecord(record)) return false;
    this.rebuild();
    return true;
  }

  applyEvent(event: RequirementBriefDomainEvent): boolean {
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
      this.upsertRecord(contiguousEvent.requirementBrief);
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

  snapshot(): RequirementBriefProjectionSnapshot {
    return this.currentSnapshot;
  }

  private upsertRecord(record: RequirementBriefRecord): boolean {
    const current = this.requirementBriefsById.get(record.id);
    if (current && record.revision <= current.revision) return false;
    this.requirementBriefsById.set(record.id, record);
    return true;
  }

  private rebuild(lastSequence = this.currentSnapshot.lastSequence): void {
    this.currentSnapshot = {
      requirementBriefs: [...this.requirementBriefsById.values()].sort(
        compareRequirementBriefs,
      ),
      events: [...this.recentEvents],
      lastSequence,
    };
  }
}

function compareRequirementBriefs(
  left: RequirementBriefRecord,
  right: RequirementBriefRecord,
): number {
  return right.updatedAt - left.updatedAt || left.id.localeCompare(right.id);
}

function compareEvents(
  left: RequirementBriefDomainEvent,
  right: RequirementBriefDomainEvent,
): number {
  return (
    left.sequence - right.sequence ||
    left.occurredAt - right.occurredAt ||
    left.eventId.localeCompare(right.eventId)
  );
}
