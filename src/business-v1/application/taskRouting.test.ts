import { describe, expect, test } from "vitest";
import {
  buildBusinessTurnInput,
  businessTaskKindFromThreadTitle,
  businessTaskThreadTitle,
  canReuseThreadForTask,
  detectBusinessTaskKind,
  routeBusinessTask,
  stripBusinessTurnContext,
  WEB_RESEARCH_SAFETY_CONTRACT,
} from "./taskRouting";

describe("business v1 task routing", () => {
  test.each([
    ["给白鹅潭项目生成报价，保留项目优惠", "quotation"],
    ["用合同和当前素材做本次验收", "acceptance"],
    ["重点审查这份合同的付款和版权条款", "contractReview"],
    ["生成本期请款资料并登记回款计划", "settlement"],
    ["整理项目并生成归档包", "archive"],
    ["查找这个客户最近成功使用的模板", "knowledgeSearch"],
  ] as const)("routes %s independently", (prompt, expected) => {
    expect(detectBusinessTaskKind(prompt)).toBe(expected);
  });

  test("creates an independent acceptance task without quotation state", () => {
    const task = routeBusinessTask({
      id: "task-acceptance-1",
      prompt: "直接从合同和素材开始验收",
      projectId: "project-1",
      attachments: [{ id: "asset-contract", name: "合同.pdf", kind: "file" }],
      now: 123,
    });

    expect(task).toMatchObject({
      id: "task-acceptance-1",
      kind: "acceptance",
      projectId: "project-1",
      attachmentIds: ["asset-contract"],
      requiresConfirmation: true,
      createdAt: 123,
    });
  });

  test("keeps knowledge searches read-only by default", () => {
    expect(routeBusinessTask({ prompt: "搜索历史报价" })).toMatchObject({
      kind: "knowledgeSearch",
      knowledgeScope: "local",
      requiresConfirmation: false,
    });
    expect(routeBusinessTask({ prompt: "查找历史报价案例" })).toMatchObject({
      kind: "knowledgeSearch",
      requiresConfirmation: false,
    });
  });

  test("rejects empty tasks", () => {
    expect(() => routeBusinessTask({ prompt: "   " })).toThrow("任务内容不能为空");
  });

  test("persists task metadata without exposing it in the user message", () => {
    const task = routeBusinessTask({
      id: "task-quote-1",
      prompt: "生成报价",
      projectId: "project-1",
      knowledgeScope: "shared",
      now: 1,
    });
    const input = buildBusinessTurnInput(task);
    expect(input).toContain('"taskId":"task-quote-1"');
    expect(input).toContain('"knowledgeScope":"shared"');
    expect(stripBusinessTurnContext(input)).toBe("生成报价");
  });

  test("adds the safety contract only to web task context", () => {
    const localInput = buildBusinessTurnInput(routeBusinessTask({
      prompt: "搜索历史报价",
      knowledgeScope: "local",
    }));
    const webInput = buildBusinessTurnInput(routeBusinessTask({
      prompt: "搜索公开行业报价",
      knowledgeScope: "web",
    }));

    expect(localInput).not.toContain("webResearchSafety");
    expect(webInput).toContain(JSON.stringify(WEB_RESEARCH_SAFETY_CONTRACT));
    expect(webInput).toContain("public-information-only");
    expect(webInput).toContain("attachment or contract source text");
    expect(webInput).toContain("bank account details or customer secrets");
    expect(webInput).toContain("local filesystem paths");
    expect(webInput).toContain("external-unconfirmed");
    expect(webInput).toContain("External results must not automatically overwrite formal business fields.");
    expect(stripBusinessTurnContext(webInput)).toBe("搜索公开行业报价");
  });

  test("reuses only the same task kind inside the same project", () => {
    const quotation = routeBusinessTask({ id: "q", prompt: "生成报价", projectId: "p1" });
    const acceptance = routeBusinessTask({ id: "a", prompt: "开始验收", projectId: "p1" });
    const thread = { projectId: "p1", title: businessTaskThreadTitle(quotation) };
    expect(businessTaskKindFromThreadTitle(thread.title)).toBe("quotation");
    expect(canReuseThreadForTask(thread, quotation)).toBe(true);
    expect(canReuseThreadForTask(thread, acceptance)).toBe(false);
    expect(canReuseThreadForTask({ ...thread, projectId: "p2" }, quotation)).toBe(false);
  });
});
