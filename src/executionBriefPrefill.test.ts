import { describe, expect, it } from "vitest";
import type { BriefRecord } from "./generated/bsaigc/BriefRecord";
import type { RequirementBriefContent } from "./generated/bsaigc/RequirementBriefContent";
import { prefillExecutionBrief } from "./executionBriefPrefill";

const projectBrief: BriefRecord = {
  objective: "旧目标",
  audience: "旧受众",
  deliverables: ["旧交付物"],
  styleKeywords: ["旧风格"],
  mandatoryItems: ["旧必拍"],
  constraints: ["旧约束"],
  risks: ["旧风险"],
  referenceNotes: "",
};

const requirement: RequirementBriefContent = {
  objective: "已确认目标",
  audience: "已确认受众",
  keyMessage: "已确认信息",
  deliverables: ["主片"],
  channels: ["视频号"],
  styleKeywords: ["克制", "真实"],
  mandatoryItems: ["品牌标识"],
  constraints: ["室内拍摄", "合规边界"],
  acceptanceCriteria: ["品牌负责人确认"],
  risks: ["档期风险"],
  deadlineAt: null,
  budgetNotes: "",
  referenceCaseIds: [],
  referenceNotes: "",
};

describe("execution brief prefill", () => {
  it("uses confirmed requirement content as the authoritative source", () => {
    const result = prefillExecutionBrief(projectBrief, requirement);
    expect(result.clientGoal).toBe("已确认目标");
    expect(result.visualStyle).toBe("克制\n真实");
    expect(result.requiredShots).toEqual(["品牌标识"]);
    expect(result.riskPoints).toEqual(["档期风险"]);
    expect(result.equipmentNotes).toBe("室内拍摄\n合规边界");
  });

  it("falls back to the legacy project brief when no requirement is confirmed", () => {
    const result = prefillExecutionBrief(projectBrief, null);
    expect(result.clientGoal).toBe("旧目标");
    expect(result.visualStyle).toBe("旧风格");
    expect(result.requiredShots).toEqual(["旧必拍"]);
    expect(result.riskPoints).toEqual(["旧风险"]);
    expect(result.equipmentNotes).toBe("旧约束");
  });
});
