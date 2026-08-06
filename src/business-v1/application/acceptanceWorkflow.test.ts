import { describe, expect, test } from "vitest";
import {
  createAcceptanceWorkflow,
  type CreateAcceptanceWorkflowInput,
} from "./acceptanceWorkflow";

function acceptanceInput(): CreateAcceptanceWorkflowInput {
  return {
    batch: { id: "acceptance-baietan-1", status: "collecting" },
    requirements: [{
      id: "video-series",
      label: "Series videos",
      requiredGroupCount: 4,
    }],
    materials: [
      { id: "video-1", requirementId: "video-series", groupId: "group-1", confirmed: true },
      { id: "video-2", requirementId: "video-series", groupId: "group-2", confirmed: true },
      { id: "video-3", requirementId: "video-series", groupId: "group-3", confirmed: true },
    ],
  };
}

describe("business v1 acceptance workflow", () => {
  test("reports required 4, provided 3, and missing 1 as a structured blocker", () => {
    const result = createAcceptanceWorkflow(acceptanceInput());

    expect(result.isReady).toBe(false);
    expect(result.materialMatrix[0]).toMatchObject({
      requiredGroupCount: 4,
      providedGroupCount: 3,
      missingGroupCount: 1,
      status: "missing",
    });
    expect(result.blockers).toEqual([{
      code: "missingMaterialGroups",
      requirementId: "video-series",
      requirementLabel: "Series videos",
      requiredGroupCount: 4,
      providedGroupCount: 3,
      missingGroupCount: 1,
    }]);
    expect(result.stages.find((stage) => stage.id === "validation")?.status).toBe("blocked");
  });

  test("does not count duplicate or unconfirmed materials", () => {
    const input = acceptanceInput();
    input.materials = [
      ...input.materials,
      {
        id: "video-3-copy",
        requirementId: "video-series",
        groupId: "group-3",
        confirmed: true,
        duplicateOfMaterialId: "video-3",
      },
      {
        id: "video-4-pending",
        requirementId: "video-series",
        groupId: "group-4",
        confirmed: false,
      },
    ];

    const result = createAcceptanceWorkflow(input);

    expect(result.materialMatrix[0].providedGroupCount).toBe(3);
    expect(result.materialMatrix[0].materials.slice(-2)).toEqual([
      {
        id: "video-3-copy",
        groupId: "group-3",
        counted: false,
        exclusionReason: "duplicate",
      },
      {
        id: "video-4-pending",
        groupId: "group-4",
        counted: false,
        exclusionReason: "unconfirmed",
      },
    ]);
  });

  test("becomes ready after the fourth confirmed unique group is supplied", () => {
    const input = acceptanceInput();
    input.materials = [
      ...input.materials,
      { id: "video-4", requirementId: "video-series", groupId: "group-4", confirmed: true },
    ];

    const result = createAcceptanceWorkflow(input);

    expect(result.isReady).toBe(true);
    expect(result.materialMatrix[0]).toMatchObject({
      providedGroupCount: 4,
      missingGroupCount: 0,
      status: "ready",
    });
    expect(result.blockers).toEqual([]);
    expect(result.stages.find((stage) => stage.id === "validation")?.status).toBe("completed");
    expect(result.stages.find((stage) => stage.id === "documents")?.status).toBe("active");
  });
});
