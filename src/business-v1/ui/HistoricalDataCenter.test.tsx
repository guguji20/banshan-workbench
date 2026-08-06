import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { HistoricalDataCenterProps } from "./HistoricalDataCenter";
import { HistoricalDataCenter } from "./HistoricalDataCenter";

function props(overrides: Partial<HistoricalDataCenterProps> = {}): HistoricalDataCenterProps {
  return {
    open: true,
    snapshot: {
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
        createdAt: Date.UTC(2026, 7, 2, 8),
        updatedAt: Date.UTC(2026, 7, 2, 9),
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
        maxAttempts: 1,
        revision: 1,
        createdAt: Date.UTC(2026, 7, 2, 8),
        updatedAt: Date.UTC(2026, 7, 2, 9),
        startedAt: Date.UTC(2026, 7, 2, 8, 30),
        finishedAt: Date.UTC(2026, 7, 2, 9),
        lastError: null,
        dependencies: [],
      }],
      cases: [],
      requirementBriefs: [],
      executionBriefs: [],
      assets: [],
      brainThreads: [{
        id: "thread-archived",
        projectId: "project-1",
        title: "已归档报价讨论",
        model: "default",
        status: "archived",
        createdAt: Date.UTC(2026, 7, 2, 8),
        updatedAt: Date.UTC(2026, 7, 2, 9),
      }],
    },
    activeProjectId: "project-1",
    onClose: vi.fn(),
    onRefresh: vi.fn().mockResolvedValue(undefined),
    onOpenAsset: vi.fn().mockResolvedValue(undefined),
    onRestoreThread: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe("HistoricalDataCenter", () => {
  it("renders the read-only history center and required categories", () => {
    const html = renderToStaticMarkup(<HistoricalDataCenter {...props()} />);

    expect(html).toContain("历史资料");
    expect(html).toContain("原件和历史版本保持不变");
    expect(html).toContain("历史任务");
    expect(html).toContain("历史案例");
    expect(html).toContain("需求简报");
    expect(html).toContain("执行简报");
    expect(html).toContain("通用资产");
    expect(html).toContain("归档会话");
    expect(html).toContain("报价任务");
    expect(html).toContain("原件和历史版本不会被覆盖");
  });

  it("renders nothing while closed", () => {
    expect(renderToStaticMarkup(<HistoricalDataCenter {...props({ open: false })} />)).toBe("");
  });
});
