import type { RequirementBriefContent } from "./generated/bsaigc/RequirementBriefContent";
import type { RequirementBriefRecord } from "./generated/bsaigc/RequirementBriefRecord";
import type { RequirementQuestionAnswer } from "./generated/bsaigc/RequirementQuestionAnswer";

export interface RequirementBriefDraft {
  answers: RequirementQuestionAnswer[];
  content: RequirementBriefContent;
}

export interface RequirementBriefDraftEntry {
  draft: RequirementBriefDraft;
  sourceRevision: number;
  dirty: boolean;
}

export type RequirementBriefDrafts = Record<
  string,
  RequirementBriefDraftEntry
>;

export function syncRequirementBriefDraft(
  drafts: RequirementBriefDrafts,
  record: RequirementBriefRecord,
): RequirementBriefDrafts {
  const current = drafts[record.projectId];
  if (
    current &&
    (current.sourceRevision === record.revision ||
      (current.dirty && record.status !== "confirmed"))
  ) {
    return drafts;
  }
  return {
    ...drafts,
    [record.projectId]: {
      draft: draftFromRecord(record),
      sourceRevision: record.revision,
      dirty: false,
    },
  };
}

export function requirementExpectedRevision(
  drafts: RequirementBriefDrafts,
  projectId: string,
  fallbackRevision: number,
): number {
  return drafts[projectId]?.sourceRevision ?? fallbackRevision;
}

export function hasRequirementBriefConflict(
  drafts: RequirementBriefDrafts,
  record: RequirementBriefRecord,
): boolean {
  const current = drafts[record.projectId];
  return Boolean(
    current?.dirty && current.sourceRevision !== record.revision,
  );
}

export function reloadRequirementBriefDraft(
  drafts: RequirementBriefDrafts,
  record: RequirementBriefRecord,
): RequirementBriefDrafts {
  return {
    ...drafts,
    [record.projectId]: entryFromRecord(record),
  };
}

export function rebaseRequirementBriefDraft(
  drafts: RequirementBriefDrafts,
  record: RequirementBriefRecord,
): RequirementBriefDrafts {
  const current = drafts[record.projectId];
  if (!current?.dirty) return reloadRequirementBriefDraft(drafts, record);
  return {
    ...drafts,
    [record.projectId]: {
      ...current,
      sourceRevision: record.revision,
    },
  };
}

export function editRequirementBriefDraft(
  drafts: RequirementBriefDrafts,
  projectId: string,
  draft: RequirementBriefDraft,
): RequirementBriefDrafts {
  const current = drafts[projectId];
  if (!current) return drafts;
  return {
    ...drafts,
    [projectId]: {
      ...current,
      draft: cloneRequirementBriefDraft(draft),
      dirty: true,
    },
  };
}

export function settleRequirementBriefDraft(
  drafts: RequirementBriefDrafts,
  projectId: string,
  submitted: RequirementBriefDraft | null,
  record: RequirementBriefRecord,
): RequirementBriefDrafts {
  const current = drafts[projectId];
  const changedSinceSubmit =
    submitted !== null &&
    current !== undefined &&
    !sameRequirementBriefDraft(current.draft, submitted);
  return {
    ...drafts,
    [projectId]: changedSinceSubmit
      ? {
          ...current,
          sourceRevision: record.revision,
          dirty: true,
        }
      : {
          draft: draftFromRecord(record),
          sourceRevision: record.revision,
          dirty: false,
        },
  };
}

export function reviewMissing(draft: RequirementBriefDraft): string[] {
  const missing: string[] = [];
  const content = draft.content;
  if (!content.objective.trim()) missing.push("项目目标");
  if (!content.audience.trim()) missing.push("目标受众");
  if (!content.keyMessage.trim()) missing.push("核心信息");
  if (content.deliverables.length === 0) missing.push("交付物");
  if (content.channels.length === 0) missing.push("发布渠道");
  if (content.acceptanceCriteria.length === 0) missing.push("验收标准");
  for (const answer of draft.answers) {
    if (!answer.required) continue;
    if (answer.disposition === "unanswered") {
      missing.push(answer.prompt);
    } else if (
      answer.disposition === "answered" &&
      !answer.answer.trim()
    ) {
      missing.push(answer.prompt);
    }
  }
  return missing;
}

export function hasFollowUp(draft: RequirementBriefDraft): boolean {
  return draft.answers.some((answer) => answer.disposition === "followUp");
}

export function cloneRequirementBriefDraft(
  draft: RequirementBriefDraft,
): RequirementBriefDraft {
  return {
    answers: draft.answers.map((answer) => ({ ...answer })),
    content: {
      ...draft.content,
      deliverables: [...draft.content.deliverables],
      channels: [...draft.content.channels],
      styleKeywords: [...draft.content.styleKeywords],
      mandatoryItems: [...draft.content.mandatoryItems],
      constraints: [...draft.content.constraints],
      acceptanceCriteria: [...draft.content.acceptanceCriteria],
      risks: [...draft.content.risks],
      referenceCaseIds: [...draft.content.referenceCaseIds],
    },
  };
}

export function sameRequirementBriefDraft(
  left: RequirementBriefDraft,
  right: RequirementBriefDraft,
): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function draftFromRecord(record: RequirementBriefRecord): RequirementBriefDraft {
  return cloneRequirementBriefDraft({
    answers: record.answers,
    content: record.content,
  });
}

function entryFromRecord(record: RequirementBriefRecord): RequirementBriefDraftEntry {
  return {
    draft: draftFromRecord(record),
    sourceRevision: record.revision,
    dirty: false,
  };
}
