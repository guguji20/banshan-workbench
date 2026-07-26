import { describe, expect, it } from "vitest";
import {
  BRAIN_SCROLL_FOLLOW_THRESHOLD_PX,
  brainScrollDistanceFromBottom,
  isBrainScrollNearBottom,
  shouldFollowBrainScroll,
} from "./brainScroll";

describe("brain scroll position", () => {
  it("treats the exact bottom and short content as near the bottom", () => {
    expect(
      isBrainScrollNearBottom({
        scrollTop: 400,
        scrollHeight: 1_000,
        clientHeight: 600,
      }),
    ).toBe(true);
    expect(
      isBrainScrollNearBottom({
        scrollTop: 0,
        scrollHeight: 500,
        clientHeight: 600,
      }),
    ).toBe(true);
  });

  it("follows at the threshold but stops after the user scrolls farther away", () => {
    const atThreshold = {
      scrollTop: 304,
      scrollHeight: 1_000,
      clientHeight: 600,
    };
    const beyondThreshold = { ...atThreshold, scrollTop: 303 };

    expect(brainScrollDistanceFromBottom(atThreshold)).toBe(
      BRAIN_SCROLL_FOLLOW_THRESHOLD_PX,
    );
    expect(isBrainScrollNearBottom(atThreshold)).toBe(true);
    expect(isBrainScrollNearBottom(beyondThreshold)).toBe(false);
  });
});

describe("brain scroll follow decisions", () => {
  it("always moves to the latest content for thread switches and user sends", () => {
    expect(shouldFollowBrainScroll("thread-switch", false)).toBe(true);
    expect(shouldFollowBrainScroll("user-send", false)).toBe(true);
  });

  it("follows streaming content only while the user remains near the bottom", () => {
    expect(shouldFollowBrainScroll("content-update", true)).toBe(true);
    expect(shouldFollowBrainScroll("content-update", false)).toBe(false);
  });
});
