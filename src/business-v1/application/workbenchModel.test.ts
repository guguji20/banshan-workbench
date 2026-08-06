import { describe, expect, test } from "vitest";
import type { BrainTurnRecord } from "../../generated/bsaigc/BrainTurnRecord";
import type { TaskRecord } from "../../generated/bsaigc/TaskRecord";
import { parseBusinessTaskKind, toWorkbenchMessages, toWorkbenchTask } from "./workbenchModel";

describe("business v1 workbench model", () => {
  test("maps one runtime turn into user and assistant messages", () => {
    const turn: BrainTurnRecord = {
      id: "turn-1",
      threadId: "thread-1",
      status: "completed",
      inputText: "生成报价",
      assistantText: "报价草稿已生成",
      error: null,
      createdAt: 10,
      updatedAt: 20,
    };
    expect(toWorkbenchMessages([turn])).toEqual([
      { id: "turn-1:user", role: "user", text: "生成报价", status: "complete", createdAt: 10 },
      { id: "turn-1:assistant", role: "assistant", text: "报价草稿已生成", status: "complete", createdAt: 20 },
    ]);
  });

  test("projects public assistant links into unconfirmed research sources", () => {
    const turn: BrainTurnRecord = {
      id: "turn-web",
      threadId: "thread-1",
      status: "completed",
      inputText: "联网查一下政策",
      assistantText: "可参考 [政策原文](https://gov.example.cn/policy/2026) 和 https://gov.example.cn/policy/2026。",
      error: null,
      createdAt: Date.UTC(2026, 6, 28, 15, 0, 0),
      updatedAt: Date.UTC(2026, 6, 28, 16, 0, 0),
    };

    expect(toWorkbenchMessages([turn])[1]).toMatchObject({
      role: "assistant",
      sources: [{
        title: "政策原文",
        domain: "gov.example.cn",
        accessedDate: "2026年7月29日",
        verificationLabel: "外部未确认",
      }],
    });
  });

  test("keeps unknown legacy tasks outside the new domain", () => {
    expect(parseBusinessTaskKind("media.generate")).toBe("general");
    expect(parseBusinessTaskKind("business.v1.acceptance")).toBe("acceptance");
  });

  test("clamps progress from durable task records", () => {
    const task = {
      id: "task-1", kind: "business.v1.quotation", projectId: null, input: {}, output: null,
      status: "running", priority: "normal", replayPolicy: "safe", progress: 150,
      attempt: 1, maxAttempts: 3, revision: 1, createdAt: 1, updatedAt: 1,
      startedAt: 1, finishedAt: null, lastError: null, dependencies: [],
    } satisfies TaskRecord;
    expect(toWorkbenchTask(task)).toMatchObject({ kind: "quotation", title: "报价", progress: 100 });
  });
});
