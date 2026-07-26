import { describe, expect, it } from "vitest";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import {
  BUSINESS_AGENTS,
  buildBusinessAgentContext,
  buildBusinessAgentPrompt,
} from "./businessAgents";

describe("business agents", () => {
  it("ships four business roles, each with three skills", () => {
    expect(BUSINESS_AGENTS.map((agent) => agent.id)).toEqual([
      "negotiation",
      "quotation",
      "contract-review",
      "acceptance-billing",
    ]);
    for (const agent of BUSINESS_AGENTS) {
      expect(agent.skills).toHaveLength(3);
      for (const skill of agent.skills) {
        expect(skill.label.length).toBeGreaterThan(0);
        expect(skill.instruction.length).toBeGreaterThan(10);
      }
    }
  });

  it("builds a readable context from real workspace numbers", () => {
    const workspace = {
      profile: {
        currency: "CNY",
        paymentTerms: "预付 50%，验收后付尾款",
        acceptanceTerms: "",
        deliverySummary: "",
        lineItems: [
          { name: "品牌视频", description: "", quantityMillis: 1000, unit: "条", unitPriceCents: 10_600_000, taxRateBps: 600 },
        ],
      },
      financialSummary: {
        quotedCents: 10_600_000,
        contractCents: 10_600_000,
        scheduledCents: 10_600_000,
        requestedCents: 5_300_000,
        receivedCents: 5_300_000,
        outstandingCents: 5_300_000,
      },
      payments: [
        { label: "预付款", amountCents: 5_300_000, status: "received", dueAt: null },
        { label: "尾款", amountCents: 5_300_000, status: "requested", dueAt: null },
      ],
      milestones: [],
      documents: [],
      currentDocuments: {
        quoteDocumentId: null,
        contractDocumentId: null,
        paymentRequestDocumentId: null,
        acceptanceDocumentId: null,
      },
    } as unknown as BusinessWorkspaceRecord;

    const context = buildBusinessAgentContext({
      projectName: "华邦年度品牌视频",
      customerName: "华邦",
      brief: null,
      workspace,
      contractFindings: [],
    });
    expect(context).toContain("项目：华邦年度品牌视频");
    expect(context).toContain("合同金额：¥106,000.00");
    expect(context).toContain("待收 ¥53,000.00");
    expect(context).toContain("尾款 ¥53,000.00（已请款）");
    expect(context).toContain("付款条款：预付 50%");
  });

  it("assembles a plain-language prompt with data-first guardrails", () => {
    const agent = BUSINESS_AGENTS[3];
    const prompt = buildBusinessAgentPrompt(agent, agent.skills[1], "项目：测试");
    expect(prompt).toContain("请你作为「验收请款助手」");
    expect(prompt).toContain("项目：测试");
    expect(prompt).toContain("不要自己编");
  });

  it("falls back gracefully when no project data exists", () => {
    const agent = BUSINESS_AGENTS[0];
    const prompt = buildBusinessAgentPrompt(agent, agent.skills[0], "");
    expect(prompt).toContain("还没有商务资料");
  });
});
