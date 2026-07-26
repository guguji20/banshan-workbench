import type { DomainEvent } from "../generated/bsaigc/DomainEvent";
import type { ProjectRecord } from "../generated/bsaigc/ProjectRecord";

const DEFAULT_EVENT_LIMIT = 80;

export interface EventProjectionSnapshot {
  readonly projects: readonly ProjectRecord[];
  readonly events: readonly DomainEvent[];
  readonly lastSequence: number;
}

/**
 * In-memory read model only. Durable state remains owned by the Rust host.
 */
export class EventProjection {
  private readonly projectsById = new Map<string, ProjectRecord>();
  private readonly seenEventIds = new Set<string>();
  private readonly eventLimit: number;
  private recentEvents: DomainEvent[] = [];
  private lastSequence = 0;
  private currentSnapshot: EventProjectionSnapshot = {
    projects: [],
    events: [],
    lastSequence: 0,
  };

  constructor(eventLimit = DEFAULT_EVENT_LIMIT) {
    if (!Number.isInteger(eventLimit) || eventLimit < 1) {
      throw new RangeError("eventLimit must be a positive integer");
    }
    this.eventLimit = eventLimit;
  }

  hydrateProjects(projects: readonly ProjectRecord[]): boolean {
    let changed = false;

    for (const project of projects) {
      const current = this.projectsById.get(project.id);
      if (!current || project.revision > current.revision) {
        this.projectsById.set(project.id, project);
        changed = true;
      }
    }

    if (changed) {
      this.rebuildSnapshot();
    }
    return changed;
  }

  applyEvent(event: DomainEvent): boolean {
    if (this.seenEventIds.has(event.eventId)) {
      return false;
    }

    this.seenEventIds.add(event.eventId);
    this.lastSequence = Math.max(this.lastSequence, event.sequence);

    const current = this.projectsById.get(event.project.id);
    if (!current || event.project.revision > current.revision) {
      this.projectsById.set(event.project.id, event.project);
    }

    this.recentEvents.push(event);
    this.recentEvents.sort(compareEvents);
    if (this.recentEvents.length > this.eventLimit) {
      this.recentEvents = this.recentEvents.slice(-this.eventLimit);
    }

    this.rebuildSnapshot();
    return true;
  }

  snapshot(): EventProjectionSnapshot {
    return this.currentSnapshot;
  }

  private rebuildSnapshot(): void {
    const projects = [...this.projectsById.values()].sort(compareProjects);
    this.currentSnapshot = {
      projects,
      events: [...this.recentEvents],
      lastSequence: this.lastSequence,
    };
  }
}

function compareProjects(left: ProjectRecord, right: ProjectRecord): number {
  return right.updatedAt - left.updatedAt || left.id.localeCompare(right.id);
}

function compareEvents(left: DomainEvent, right: DomainEvent): number {
  return (
    left.sequence - right.sequence ||
    left.occurredAt - right.occurredAt ||
    left.eventId.localeCompare(right.eventId)
  );
}
