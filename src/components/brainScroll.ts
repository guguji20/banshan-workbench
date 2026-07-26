export const BRAIN_SCROLL_FOLLOW_THRESHOLD_PX = 96;

export interface BrainScrollMetrics {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
}

export type BrainScrollReason =
  | "thread-switch"
  | "user-send"
  | "content-update";

export function brainScrollDistanceFromBottom({
  scrollTop,
  scrollHeight,
  clientHeight,
}: BrainScrollMetrics): number {
  return Math.max(0, scrollHeight - clientHeight - scrollTop);
}

export function isBrainScrollNearBottom(
  metrics: BrainScrollMetrics,
  threshold = BRAIN_SCROLL_FOLLOW_THRESHOLD_PX,
): boolean {
  return brainScrollDistanceFromBottom(metrics) <= Math.max(0, threshold);
}

export function shouldFollowBrainScroll(
  reason: BrainScrollReason,
  wasNearBottom: boolean,
): boolean {
  return reason === "thread-switch" || reason === "user-send" || wasNearBottom;
}
