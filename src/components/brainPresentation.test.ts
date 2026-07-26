import { describe, expect, it } from "vitest";
import type { BrainTurnRecord } from "../generated/bsaigc/BrainTurnRecord";
import {
  friendlyBrainThreadTitle,
  presentBrainTurn,
  presentOrphanBrainStreaming,
} from "./brainPresentation";

function turn(overrides: Partial<BrainTurnRecord> = {}): BrainTurnRecord {
  return {
    id: "turn-1",
    threadId: "thread-1",
    status: "completed",
    inputText: "请审查合同",
    assistantText: "已完成审查。",
    error: null,
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}

describe("brain presentation", () => {
  it("hides contract prompts, source JSON, structured output and streaming", () => {
    const inputText = [
      "你是中国大陆影视制作与营销服务合同的高级商务审查 Agent。",
      "CONTRACT_INPUT_JSON:",
      JSON.stringify({ blocks: [{ text: "不得展示" }] }),
    ].join("\n");
    const result = presentBrainTurn(
      turn({
        status: "running",
        inputText,
        assistantText: '{"findings":[]}',
      }),
      '{"findings":[{"title":"内部结果"}]}',
    );

    expect(result.internal).toBe(true);
    expect(result.userText).toBeNull();
    expect(result.assistantText).toBeNull();
    expect(result.statusText).toBe("正在审查合同");
  });

  it("replaces raw assistant JSON with a friendly completed state", () => {
    const result = presentBrainTurn(
      turn({ assistantText: '{"result":"ok","payload":{"secret":"hidden"}}' }),
    );

    expect(result.internal).toBe(true);
    expect(result.assistantText).toBeNull();
    expect(result.statusText).toBe("任务已完成，结果已同步到工作台");
  });

  it("keeps ordinary conversation text visible", () => {
    const result = presentBrainTurn(
      turn({ inputText: "帮我整理付款节点", assistantText: "已整理为三期付款。" }),
    );

    expect(result.internal).toBe(false);
    expect(result.userText).toBe("帮我整理付款节点");
    expect(result.assistantText).toBe("已整理为三期付款。");
  });

  it("never exposes orphan internal streaming or broken internal titles", () => {
    expect(
      presentOrphanBrainStreaming('<tool_call>{"name":"review"}</tool_call>', true),
    ).toEqual({ text: null, statusText: "正在处理任务" });
    expect(friendlyBrainThreadTitle(`${"?".repeat(4)} · review-1`)).toBe(
      "商务任务",
    );
  });
});
