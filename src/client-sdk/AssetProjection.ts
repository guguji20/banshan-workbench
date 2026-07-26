import type { AssetDomainEvent } from "../generated/bsaigc/AssetDomainEvent";
import type { AssetRecord } from "../generated/bsaigc/AssetRecord";

const EVENT_LIMIT = 80;

export interface AssetProjectionSnapshot {
  readonly assets: readonly AssetRecord[];
  readonly events: readonly AssetDomainEvent[];
  readonly lastSequence: number;
}

export class AssetProjection {
  private readonly assetsById = new Map<string, AssetRecord>();
  private readonly seenEventIds = new Set<string>();
  private recentEvents: AssetDomainEvent[] = [];
  private currentSnapshot: AssetProjectionSnapshot = {
    assets: [],
    events: [],
    lastSequence: 0,
  };

  hydrate(assets: readonly AssetRecord[]): boolean {
    let changed = false;
    for (const asset of assets) {
      const current = this.assetsById.get(asset.id);
      if (!current || asset.revision > current.revision) {
        this.assetsById.set(asset.id, asset);
        changed = true;
      }
    }
    if (changed) this.rebuild();
    return changed;
  }

  applyEvent(event: AssetDomainEvent): boolean {
    if (this.seenEventIds.has(event.eventId)) return false;
    this.seenEventIds.add(event.eventId);
    const current = this.assetsById.get(event.asset.id);
    if (!current || event.asset.revision > current.revision) {
      this.assetsById.set(event.asset.id, event.asset);
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

  snapshot(): AssetProjectionSnapshot {
    return this.currentSnapshot;
  }

  private rebuild(lastSequence = this.currentSnapshot.lastSequence): void {
    this.currentSnapshot = {
      assets: [...this.assetsById.values()].sort(
        (left, right) =>
          right.updatedAt - left.updatedAt || left.id.localeCompare(right.id),
      ),
      events: [...this.recentEvents],
      lastSequence,
    };
  }
}
