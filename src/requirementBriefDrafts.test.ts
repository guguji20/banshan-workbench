import { describe, expect, it } from "vitest";
import type { RequirementBriefRecord } from "./generated/bsaigc/RequirementBriefRecord";
import {
  editRequirementBriefDraft,
  hasFollowUp,
  hasRequirementBriefConflict,
  rebaseRequirementBriefDraft,
  reloadRequirementBriefDraft,
  requirementExpectedRevision,
  reviewMissing,
  settleRequirementBriefDraft,
  syncRequirementBriefDraft,
} from "./requirementBriefDrafts";

function record(projectId: string, revision: number): RequirementBriefRecord {
  return {
    id: `brief-${projectId}`,
    projectId,
    questionSetVersion: "v1",
    status: "interviewing",
    confirmedAt: null,
    confirmedBy: null,
    revision,
    createdAt: 1,
    updatedAt: revision,
    answers: [
      {
        questionId: "goal",
        prompt: "目标是什么？",
        required: true,
        answer: "增长认知",
        disposition: "answered",
      },
    ],
    content: {
      objective: "增长认知",
      audience: "改善型家庭",
      keyMessage: "生活方式",
      deliverables: ["90 秒主片"],
      channels: ["视频号"],
      styleKeywords: [],
      mandatoryItems: [],
      constraints: [],
      acceptanceCriteria: ["客户品牌负责人确认"],
      risks: [],
      deadlineAt: null,
      budgetNotes: "",
      referenceCaseIds: [],
      referenceNotes: "",
    },
  };
}

describe("requirement brief drafts", () => {
  it("refreshes clean state and preserves dirty project drafts", () => {
    const first = syncRequirementBriefDraft({}, record("a", 1));
    const editedDraft = {
      ...first.a.draft,
      content: { ...first.a.draft.content, objective: "本地编辑" },
    };
    const dirty = editRequirementBriefDraft(first, "a", editedDraft);
    const preserved = syncRequirementBriefDraft(dirty, {
      ...record("a", 2),
      content: { ...record("a", 2).content, objective: "远端编辑" },
    });
    expect(preserved).toBe(dirty);
    expect(preserved.a.draft.content.objective).toBe("本地编辑");
    expect(requirementExpectedRevision(preserved, "a", 2)).toBe(1);
  });

  it("replaces stale dirty content when the host confirms a newer record", () => {
    const first = syncRequirementBriefDraft({}, record("a", 1));
    const dirty = editRequirementBriefDraft(first, "a", {
      ...first.a.draft,
      content: { ...first.a.draft.content, objective: "未保存内容" },
    });
    const confirmed = {
      ...record("a", 2),
      status: "confirmed" as const,
      confirmedAt: 2,
      confirmedBy: "reviewer",
      content: { ...record("a", 2).content, objective: "确认版本" },
    };
    const synchronized = syncRequirementBriefDraft(dirty, confirmed);
    expect(synchronized.a.dirty).toBe(false);
    expect(synchronized.a.sourceRevision).toBe(2);
    expect(synchronized.a.draft.content.objective).toBe("确认版本");
  });

  it("does not let one project response overwrite another project", () => {
    let drafts = syncRequirementBriefDraft({}, record("a", 1));
    drafts = syncRequirementBriefDraft(drafts, record("b", 1));
    const settled = settleRequirementBriefDraft(
      drafts,
      "a",
      drafts.a.draft,
      record("a", 2),
    );
    expect(settled.a.sourceRevision).toBe(2);
    expect(settled.b).toBe(drafts.b);
  });

  it("preserves input typed while save is in flight", () => {
    const initial = syncRequirementBriefDraft({}, record("a", 1));
    const submitted = initial.a.draft;
    const later = {
      ...submitted,
      content: { ...submitted.content, objective: "保存后继续输入" },
    };
    const dirty = editRequirementBriefDraft(initial, "a", later);
    const settled = settleRequirementBriefDraft(
      dirty,
      "a",
      submitted,
      record("a", 2),
    );
    expect(settled.a.draft.content.objective).toBe("保存后继续输入");
    expect(settled.a.dirty).toBe(true);
  });

  it("exposes stale dirty drafts and resolves them only by explicit choice", () => {
    const first = syncRequirementBriefDraft({}, record("a", 1));
    const dirty = editRequirementBriefDraft(first, "a", {
      ...first.a.draft,
      content: { ...first.a.draft.content, objective: "本地版本" },
    });
    const remote = {
      ...record("a", 2),
      content: { ...record("a", 2).content, objective: "远端版本" },
    };
    const conflicted = syncRequirementBriefDraft(dirty, remote);

    expect(hasRequirementBriefConflict(conflicted, remote)).toBe(true);
    const rebased = rebaseRequirementBriefDraft(conflicted, remote);
    expect(rebased.a.draft.content.objective).toBe("本地版本");
    expect(rebased.a.sourceRevision).toBe(2);
    expect(hasRequirementBriefConflict(rebased, remote)).toBe(false);

    const reloaded = reloadRequirementBriefDraft(conflicted, remote);
    expect(reloaded.a.draft.content.objective).toBe("远端版本");
    expect(reloaded.a.sourceRevision).toBe(2);
    expect(reloaded.a.dirty).toBe(false);
  });

  it("calculates review and confirmation gates", () => {
    const complete = syncRequirementBriefDraft({}, record("a", 1)).a.draft;
    expect(reviewMissing(complete)).toEqual([]);
    expect(hasFollowUp(complete)).toBe(false);
    const followUp = {
      ...complete,
      answers: [
        { ...complete.answers[0], disposition: "followUp" as const },
      ],
    };
    expect(hasFollowUp(followUp)).toBe(true);
  });
});
