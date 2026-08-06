import { describe, expect, it } from "vitest";
import type { HistoryDataSnapshot } from "./historyDataView";
import {
  buildHistoryRecords,
  filterHistoryRecords,
  historyCategoryCounts,
} from "./historyDataView";

const UPDATED_AT = Date.UTC(2026, 7, 2, 8, 30);

function historySnapshot(): HistoryDataSnapshot {
  return {
    projects: [{
      id: "project-1",
      name: "白鹅潭项目",
      clientName: "客户甲",
      brief: {
        objective: "",
        audience: "",
        deliverables: [],
        styleKeywords: [],
        mandatoryItems: [],
        constraints: [],
        risks: [],
        referenceNotes: "",
      },
      stage: "delivery",
      revision: 1,
      createdAt: UPDATED_AT - 20_000,
      updatedAt: UPDATED_AT,
    }],
    tasks: [{
      id: "task-1",
      kind: "quotation",
      projectId: "project-1",
      input: {},
      output: null,
      status: "succeeded",
      priority: "normal",
      replayPolicy: "manual",
      progress: 100,
      attempt: 1,
      maxAttempts: 3,
      revision: 1,
      createdAt: UPDATED_AT - 1_000,
      updatedAt: UPDATED_AT,
      startedAt: UPDATED_AT - 800,
      finishedAt: UPDATED_AT,
      lastError: null,
      dependencies: [],
    }] as HistoryDataSnapshot["tasks"],
    cases: [{
      id: "case-1",
      assetId: "asset-case",
      projectId: "project-1",
      title: "白鹅潭品牌片",
      clientName: "客户甲",
      contentType: "brand",
      presentation: "mixedMedia",
      hasActors: true,
      isAigc: false,
      qualityTier: "premium",
      tags: ["品牌", "视频"],
      notes: "已授权案例",
      revision: 1,
      createdAt: UPDATED_AT - 3_000,
      updatedAt: UPDATED_AT - 1_000,
    }],
    requirementBriefs: [{
      id: "requirement-1",
      projectId: "project-1",
      questionSetVersion: "1",
      answers: [],
      content: {
        objective: "完成年度品牌内容",
        audience: "公众",
        keyMessage: "品牌焕新",
        deliverables: ["品牌片", "海报"],
        channels: [],
        styleKeywords: [],
        mandatoryItems: [],
        constraints: [],
        acceptanceCriteria: [],
        risks: [],
        deadlineAt: null,
        budgetNotes: "",
        referenceCaseIds: [],
        referenceNotes: "",
      },
      status: "confirmed",
      confirmedAt: UPDATED_AT,
      confirmedBy: "admin",
      revision: 1,
      createdAt: UPDATED_AT - 5_000,
      updatedAt: UPDATED_AT - 2_000,
    }],
    executionBriefs: [{
      id: "execution-1",
      projectId: "project-1",
      content: {
        shootAt: null,
        clientGoal: "完成首批拍摄",
        visualStyle: "纪实",
        primaryShots: [],
        secondaryShots: [],
        requiredShots: ["园区全景"],
        fallbackShots: [],
        riskPoints: [],
        waitingTimeActions: [],
        equipmentNotes: "",
        postShootHighlights: [],
      },
      status: "ready",
      revision: 1,
      createdAt: UPDATED_AT - 7_000,
      updatedAt: UPDATED_AT - 3_000,
    }],
    assets: [{
      id: "asset-1",
      projectId: null,
      originalName: "历史报价.xlsx",
      kind: "document",
      mimeType: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
      sizeBytes: 2048,
      sha256: "a".repeat(64),
      status: "ready",
      revision: 1,
      createdAt: UPDATED_AT - 9_000,
      updatedAt: UPDATED_AT - 4_000,
      previewAvailable: true,
    }],
    brainThreads: [{
      id: "thread-archived",
      projectId: "project-1",
      title: "已归档报价讨论",
      model: "default",
      status: "archived",
      createdAt: UPDATED_AT - 11_000,
      updatedAt: UPDATED_AT - 5_000,
    }, {
      id: "thread-active",
      projectId: "project-1",
      title: "当前对话",
      model: "default",
      status: "ready",
      createdAt: UPDATED_AT - 10_000,
      updatedAt: UPDATED_AT - 6_000,
    }],
  };
}

describe("historyDataView", () => {
  it("maps every required legacy data class without restoring legacy UI logic", () => {
    const records = buildHistoryRecords(historySnapshot());

    expect(records.map((record) => record.category)).toEqual([
      "tasks",
      "cases",
      "requirementBriefs",
      "executionBriefs",
      "assets",
      "archivedThreads",
    ]);
    expect(records.find((record) => record.category === "cases")).toMatchObject({
      title: "白鹅潭品牌片",
      assetId: "asset-case",
      projectLabel: "白鹅潭项目",
    });
    expect(records.find((record) => record.category === "archivedThreads")).toMatchObject({
      title: "已归档报价讨论",
      restoreThreadId: "thread-archived",
    });
    expect(records.some((record) => record.id === "thread-active")).toBe(false);
  });

  it("filters by category, current project, and free text", () => {
    const records = buildHistoryRecords(historySnapshot());

    expect(filterHistoryRecords(records, "assets", "报价", null)).toHaveLength(1);
    expect(filterHistoryRecords(records, "assets", "报价", "project-1")).toHaveLength(0);
    expect(filterHistoryRecords(records, "cases", "品牌", "project-1")[0]?.id).toBe("case-1");
  });

  it("reports category counts for the selected project scope", () => {
    const records = buildHistoryRecords(historySnapshot());

    expect(historyCategoryCounts(records, null)).toMatchObject({
      tasks: 1,
      cases: 1,
      requirementBriefs: 1,
      executionBriefs: 1,
      assets: 1,
      archivedThreads: 1,
    });
    expect(historyCategoryCounts(records, "project-1").assets).toBe(0);
  });
});
