import type { BusinessWorkspaceDomainEvent } from "../generated/bsaigc/BusinessWorkspaceDomainEvent";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";

const DEFAULT_EVENT_LIMIT = 80;

export interface BusinessWorkspaceProjectionSnapshot {
  readonly businessWorkspaces: readonly BusinessWorkspaceRecord[];
  readonly events: readonly BusinessWorkspaceDomainEvent[];
  readonly lastSequence: number;
}

/** In-memory read model. Durable business workspace state remains owned by the host. */
export class BusinessWorkspaceProjection {
  private readonly businessWorkspacesById = new Map<
    string,
    BusinessWorkspaceRecord
  >();
  private readonly pendingEvents = new Map<
    number,
    BusinessWorkspaceDomainEvent
  >();
  private readonly eventLimit: number;
  private recentEvents: BusinessWorkspaceDomainEvent[] = [];
  private currentSnapshot: BusinessWorkspaceProjectionSnapshot =
    emptySnapshot();

  constructor(eventLimit = DEFAULT_EVENT_LIMIT) {
    if (!Number.isInteger(eventLimit) || eventLimit < 1) {
      throw new RangeError("eventLimit must be a positive integer");
    }
    this.eventLimit = eventLimit;
  }

  hydrate(records: readonly BusinessWorkspaceRecord[]): boolean {
    let changed = false;
    for (const record of records) {
      changed = this.upsertRecord(record) || changed;
    }
    if (changed) this.rebuild();
    return changed;
  }

  upsert(record: BusinessWorkspaceRecord): boolean {
    if (!this.upsertRecord(record)) return false;
    this.rebuild();
    return true;
  }

  applyEvent(event: BusinessWorkspaceDomainEvent): boolean {
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
      this.upsertRecord(contiguousEvent.businessWorkspace);
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

  reset(): boolean {
    const changed =
      this.businessWorkspacesById.size > 0 ||
      this.pendingEvents.size > 0 ||
      this.recentEvents.length > 0 ||
      this.currentSnapshot.lastSequence > 0;
    this.businessWorkspacesById.clear();
    this.pendingEvents.clear();
    this.recentEvents = [];
    this.currentSnapshot = emptySnapshot();
    return changed;
  }

  snapshot(): BusinessWorkspaceProjectionSnapshot {
    return this.currentSnapshot;
  }

  private upsertRecord(record: BusinessWorkspaceRecord): boolean {
    const current = this.businessWorkspacesById.get(record.id);
    if (current && record.revision <= current.revision) return false;
    this.businessWorkspacesById.set(record.id, record);
    return true;
  }

  private rebuild(lastSequence = this.currentSnapshot.lastSequence): void {
    this.currentSnapshot = {
      businessWorkspaces: [...this.businessWorkspacesById.values()].sort(
        compareBusinessWorkspaces,
      ),
      events: [...this.recentEvents],
      lastSequence,
    };
  }
}

function emptySnapshot(): BusinessWorkspaceProjectionSnapshot {
  return {
    businessWorkspaces: [],
    events: [],
    lastSequence: 0,
  };
}

function compareBusinessWorkspaces(
  left: BusinessWorkspaceRecord,
  right: BusinessWorkspaceRecord,
): number {
  return right.updatedAt - left.updatedAt || left.id.localeCompare(right.id);
}

function compareEvents(
  left: BusinessWorkspaceDomainEvent,
  right: BusinessWorkspaceDomainEvent,
): number {
  return (
    left.sequence - right.sequence ||
    left.occurredAt - right.occurredAt ||
    left.eventId.localeCompare(right.eventId)
  );
}
