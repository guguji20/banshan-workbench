import type { BriefRecord } from "./generated/bsaigc/BriefRecord";
import type { ExecutionBriefContent } from "./generated/bsaigc/ExecutionBriefContent";
import type { RequirementBriefContent } from "./generated/bsaigc/RequirementBriefContent";

export function prefillExecutionBrief(
  projectBrief: BriefRecord,
  confirmedRequirement: RequirementBriefContent | null,
): ExecutionBriefContent {
  const source = confirmedRequirement;
  return {
    shootAt: null,
    clientGoal: source?.objective ?? projectBrief.objective,
    visualStyle:
      source?.styleKeywords.join("\n") ?? projectBrief.styleKeywords.join("\n"),
    primaryShots: [],
    secondaryShots: [],
    requiredShots: [
      ...(source?.mandatoryItems ?? projectBrief.mandatoryItems),
    ],
    fallbackShots: [],
    riskPoints: [...(source?.risks ?? projectBrief.risks)],
    waitingTimeActions: [
      "看景与备选机位",
      "检查灯位与构图",
      "沟通演员动作与状态",
    ],
    equipmentNotes:
      source?.constraints.join("\n") ?? projectBrief.constraints.join("\n"),
    postShootHighlights: [],
  };
}
