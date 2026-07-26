import { describe, expect, it } from "vitest";
import type { ExecutionBriefContent } from "./generated/bsaigc/ExecutionBriefContent";
import {
  editExecutionBriefDraft,
  executionBriefSourceKey,
  settleExecutionBriefDraft,
  syncExecutionBriefDraft,
  type ExecutionBriefDrafts,
} from "./executionBriefDrafts";

function content(clientGoal: string): ExecutionBriefContent {
  return {
    shootAt: null,
    clientGoal,
    visualStyle: "natural",
    primaryShots: [],
    secondaryShots: [],
    requiredShots: [],
    fallbackShots: [],
    riskPoints: [],
    waitingTimeActions: [],
    equipmentNotes: "",
    postShootHighlights: [],
  };
}

describe("execution brief drafts", () => {
  it("includes a confirmed requirement brief revision when no execution brief exists", () => {
    expect(executionBriefSourceKey(4, undefined, 7)).toBe(
      "project:4:requirement-brief:7",
    );
    expect(executionBriefSourceKey(4, undefined, 8)).not.toBe(
      executionBriefSourceKey(4, undefined, 7),
    );
  });

  it("uses the execution brief revision as the authoritative source", () => {
    expect(executionBriefSourceKey(4, 9, 7)).toBe("brief:9");
    expect(executionBriefSourceKey(5, 9, 8)).toBe("brief:9");
  });

  it("refreshes clean drafts but preserves dirty drafts", () => {
    const initial = syncExecutionBriefDraft({}, "a", "brief:1", content("one"));
    const refreshed = syncExecutionBriefDraft(initial, "a", "brief:2", content("two"));
    expect(refreshed.a.content.clientGoal).toBe("two");

    const dirty = editExecutionBriefDraft(refreshed, "a", content("local"));
    const preserved = syncExecutionBriefDraft(dirty, "a", "brief:3", content("remote"));
    expect(preserved).toBe(dirty);
    expect(preserved.a.content.clientGoal).toBe("local");
  });

  it("settles only the submitted project", () => {
    let drafts: ExecutionBriefDrafts = {};
    drafts = syncExecutionBriefDraft(drafts, "a", "brief:1", content("a"));
    drafts = syncExecutionBriefDraft(drafts, "b", "brief:4", content("b"));

    const settled = settleExecutionBriefDraft(
      drafts,
      "a",
      "brief:2",
      content("a"),
      content("saved a"),
    );
    expect(settled.a.content.clientGoal).toBe("saved a");
    expect(settled.a.dirty).toBe(false);
    expect(settled.b).toBe(drafts.b);
  });

  it("keeps edits made while a save is in flight", () => {
    const initial = syncExecutionBriefDraft({}, "a", "brief:1", content("before"));
    const submitted = content("submitted");
    const saving = editExecutionBriefDraft(initial, "a", submitted);
    const edited = editExecutionBriefDraft(saving, "a", content("typed later"));

    const settled = settleExecutionBriefDraft(
      edited,
      "a",
      "brief:2",
      submitted,
      submitted,
    );
    expect(settled.a.content.clientGoal).toBe("typed later");
    expect(settled.a.sourceKey).toBe("brief:2");
    expect(settled.a.dirty).toBe(true);
  });
});
