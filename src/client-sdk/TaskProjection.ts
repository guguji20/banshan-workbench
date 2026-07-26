import type { TaskDomainEvent } from "../generated/bsaigc/TaskDomainEvent";
import type { TaskRecord } from "../generated/bsaigc/TaskRecord";

const EVENT_LIMIT = 80;

export interface TaskProjectionSnapshot {
  readonly tasks: readonly TaskRecord[];
  readonly events: readonly TaskDomainEvent[];
  readonly lastSequence: number;
}

export class TaskProjection {
  private readonly tasksById = new Map<string, TaskRecord>();
  private readonly seenEventIds = new Set<string>();
  private recentEvents: TaskDomainEvent[] = [];
  private currentSnapshot: TaskProjectionSnapshot = {
    tasks: [],
    events: [],
    lastSequence: 0,
  };

  hydrate(tasks: readonly TaskRecord[]): boolean {
    let changed = false;
    for (const task of tasks) {
      const current = this.tasksById.get(task.id);
      if (!current || task.revision > current.revision) {
        this.tasksById.set(task.id, task);
        changed = true;
      }
    }
    if (changed) this.rebuild();
    return changed;
  }

  applyEvent(event: TaskDomainEvent): boolean {
    if (this.seenEventIds.has(event.eventId)) return false;
    this.seenEventIds.add(event.eventId);
    const current = this.tasksById.get(event.task.id);
    if (!current || event.task.revision > current.revision) {
      this.tasksById.set(event.task.id, event.task);
    }
    this.recentEvents.push(event);
    this.recentEvents.sort(
      (left, right) =>
        left.sequence - right.sequence || left.eventId.localeCompare(right.eventId),
    );
    if (this.recentEvents.length > EVENT_LIMIT) {
      this.recentEvents = this.recentEvents.slice(-EVENT_LIMIT);
    }
    this.rebuild(Math.max(this.currentSnapshot.lastSequence, event.sequence));
    return true;
  }

  snapshot(): TaskProjectionSnapshot {
    return this.currentSnapshot;
  }

  private rebuild(lastSequence = this.currentSnapshot.lastSequence): void {
    this.currentSnapshot = {
      tasks: [...this.tasksById.values()].sort(compareTasks),
      events: [...this.recentEvents],
      lastSequence,
    };
  }
}

const STATUS_ORDER: Record<TaskRecord["status"], number> = {
  running: 0,
  awaitingApproval: 1,
  queued: 2,
  failed: 3,
  canceled: 4,
  succeeded: 5,
};

const PRIORITY_ORDER: Record<TaskRecord["priority"], number> = {
  critical: 0,
  high: 1,
  normal: 2,
  low: 3,
};

function compareTasks(left: TaskRecord, right: TaskRecord): number {
  return (
    STATUS_ORDER[left.status] - STATUS_ORDER[right.status] ||
    PRIORITY_ORDER[left.priority] - PRIORITY_ORDER[right.priority] ||
    right.updatedAt - left.updatedAt ||
    left.id.localeCompare(right.id)
  );
}
