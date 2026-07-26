import type { ExecutionBriefContent } from "./generated/bsaigc/ExecutionBriefContent";

export interface ExecutionBriefDraftEntry {
  content: ExecutionBriefContent;
  sourceKey: string;
  dirty: boolean;
}

export type ExecutionBriefDrafts = Record<string, ExecutionBriefDraftEntry>;

export function executionBriefSourceKey(
  projectRevision: number,
  briefRevision?: number,
  requirementBriefRevision?: number,
): string {
  if (briefRevision !== undefined) return `brief:${briefRevision}`;
  return requirementBriefRevision === undefined
    ? `project:${projectRevision}`
    : `project:${projectRevision}:requirement-brief:${requirementBriefRevision}`;
}

export function syncExecutionBriefDraft(
  drafts: ExecutionBriefDrafts,
  projectId: string,
  sourceKey: string,
  content: ExecutionBriefContent,
): ExecutionBriefDrafts {
  const current = drafts[projectId];
  if (current && (current.dirty || current.sourceKey === sourceKey)) {
    return drafts;
  }
  return {
    ...drafts,
    [projectId]: {
      content: cloneExecutionBriefContent(content),
      sourceKey,
      dirty: false,
    },
  };
}

export function editExecutionBriefDraft(
  drafts: ExecutionBriefDrafts,
  projectId: string,
  content: ExecutionBriefContent,
): ExecutionBriefDrafts {
  const current = drafts[projectId];
  if (!current) return drafts;
  return {
    ...drafts,
    [projectId]: {
      ...current,
      content: cloneExecutionBriefContent(content),
      dirty: true,
    },
  };
}

export function settleExecutionBriefDraft(
  drafts: ExecutionBriefDrafts,
  projectId: string,
  sourceKey: string,
  submittedContent: ExecutionBriefContent,
  persistedContent: ExecutionBriefContent,
): ExecutionBriefDrafts {
  const current = drafts[projectId];
  const changedSinceSubmit =
    current && !sameExecutionBriefContent(current.content, submittedContent);
  return {
    ...drafts,
    [projectId]: changedSinceSubmit
      ? {
          ...current,
          sourceKey,
          dirty: true,
        }
      : {
          content: cloneExecutionBriefContent(persistedContent),
          sourceKey,
          dirty: false,
        },
  };
}

export function cloneExecutionBriefContent(
  content: ExecutionBriefContent,
): ExecutionBriefContent {
  return {
    ...content,
    primaryShots: [...content.primaryShots],
    secondaryShots: [...content.secondaryShots],
    requiredShots: [...content.requiredShots],
    fallbackShots: [...content.fallbackShots],
    riskPoints: [...content.riskPoints],
    waitingTimeActions: [...content.waitingTimeActions],
    postShootHighlights: [...content.postShootHighlights],
  };
}

export function sameExecutionBriefContent(
  left: ExecutionBriefContent,
  right: ExecutionBriefContent,
): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}
