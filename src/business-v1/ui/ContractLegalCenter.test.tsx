import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import type { BsaigcClient } from "../../client-sdk";
import type { BusinessWorkspaceRecord } from "../../generated/bsaigc/BusinessWorkspaceRecord";
import type { ContractReviewRecord } from "../../generated/bsaigc/ContractReviewRecord";
import type { EvidenceContext } from "../../generated/bsaigc/EvidenceContext";
import type { ReviewFindingRecord } from "../../generated/bsaigc/ReviewFindingRecord";
import type { ContractLegalCenterProps } from "./ContractLegalCenter";

const WORKSPACE = { id: "workspace-1" } as BusinessWorkspaceRecord;

const OPEN_FINDING: ReviewFindingRecord = {
  id: "finding-open",
  reviewId: "review-1",
  source: "rule",
  ruleId: "payment-penalty",
  ruleVersion: "1.0.0",
  agentRunId: null,
  category: "违约责任",
  severity: "high",
  title: "违约金比例过高",
  description: "合同约定每日 5% 违约金，明显高于常用范围。",
  recommendation: "建议改为每日 0.05%，并设置累计上限。",
  evidenceIds: ["evidence-1"],
  missingEvidenceReason: null,
  status: "open",
  decision: "unreviewed",
  revision: 1,
  createdAt: 100,
  updatedAt: 100,
};

const DECIDED_FINDING: ReviewFindingRecord = {
  ...OPEN_FINDING,
  id: "finding-decided",
  title: "验收口径不清晰",
  severity: "critical",
  status: "decided",
  decision: "needsRevision",
  evidenceIds: [],
  missingEvidenceReason: "原文未找到对应验收条款",
};

const REVIEW: ContractReviewRecord = {
  session: {
    id: "review-1",
    workspaceId: WORKSPACE.id,
    sourceAssetId: "asset-contract",
    sourceAssetSha256: "a".repeat(64),
    sourceFileName: "华邦-年度框架合同.pdf",
    status: "awaitingConfirmation",
    stage: "awaitingConfirmation",
    extractionId: "extraction-1",
    reportAssetId: "asset-report",
    revision: 4,
    createdAt: 100,
    updatedAt: 200,
    completedAt: null,
    failure: null,
  },
  extraction: null,
  evidence: [],
  findings: [OPEN_FINDING, DECIDED_FINDING],
  ruleEvaluations: [],
  decisions: [{
    id: "decision-1",
    reviewId: "review-1",
    findingId: DECIDED_FINDING.id,
    decision: "needsRevision",
    comment: "要求法务重拟验收条款。",
    actorId: "admin",
    findingRevision: 1,
    createdAt: 180,
  }],
  reports: [{
    id: "report-1",
    reviewId: "review-1",
    reviewRevision: 3,
    sourceAssetId: "asset-contract",
    sourceAssetSha256: "a".repeat(64),
    extractionId: "extraction-1",
    ruleSetVersion: "1.0.0",
    agentRunIds: [],
    format: "docx",
    reportAssetId: "asset-report",
    reportAssetSha256: "b".repeat(64),
    generatedAt: 190,
  }],
};

const EVIDENCE: EvidenceContext = {
  evidence: {
    id: "evidence-1",
    extractionId: "extraction-1",
    sourceAssetId: "asset-contract",
    pageIndex: 4,
    blockId: "block-1",
    charStart: 0,
    charEnd: 18,
    bbox: null,
    quotedText: "乙方每逾期一日，应支付合同金额 5% 的违约金。",
    quotedTextSha256: "c".repeat(64),
    contextBefore: "违约责任：",
    contextAfter: "甲方有权解除合同。",
  },
  page: {
    id: "page-5",
    extractionId: "extraction-1",
    pageIndex: 4,
    text: "违约责任：乙方每逾期一日，应支付合同金额 5% 的违约金。甲方有权解除合同。",
    textSha256: "d".repeat(64),
    width: 595,
    height: 842,
    previewAssetId: "asset-page-5",
  },
  block: {
    id: "block-1",
    extractionId: "extraction-1",
    pageId: "page-5",
    pageIndex: 4,
    orderIndex: 0,
    kind: "paragraph",
    text: "乙方每逾期一日，应支付合同金额 5% 的违约金。",
    charStart: 0,
    charEnd: 18,
    bbox: null,
  },
};

afterEach(() => {
  vi.doUnmock("react");
  vi.resetModules();
});

describe("ContractLegalCenter", () => {
  it("loads review findings and keeps the DOCX report gated while one finding is open", async () => {
    const harness = await createHarness(REVIEW);
    await harness.loadWorkspaceReview();
    const html = renderToStaticMarkup(harness.render());

    expect(html).toContain("华邦-年度框架合同.pdf");
    expect(html).toContain("待人工决策");
    expect(html).toContain("违约金比例过高");
    expect(html).toContain("建议改为每日 0.05%");
    expect(html).toContain("仍有 1 项风险等待人工决策");
    expect(html).toContain("打开报告");
    expect(buttonOpening(html, "生成 DOCX 报告")).toContain("disabled");
  });

  it("renders precise page evidence and unlocks DOCX only after every finding is decided", async () => {
    const allDecided: ContractReviewRecord = {
      ...REVIEW,
      findings: REVIEW.findings.map((finding) => ({
        ...finding,
        status: "decided",
        decision: finding.decision === "unreviewed" ? "confirmed" : finding.decision,
      })),
    };
    const harness = await createHarness(allDecided);
    await harness.loadWorkspaceReview();
    harness.runEffect(3);
    await flushPromises();
    const html = renderToStaticMarkup(harness.render());

    expect(harness.getEvidenceContext).toHaveBeenCalledWith("evidence-1");
    expect(html).toContain("第 5 页");
    expect(html).toContain("合同金额 5% 的违约金");
    expect(html).toContain("所有风险已完成人工决策");
    expect(buttonOpening(html, "生成 DOCX 报告")).not.toContain("disabled");
  });

  it("shows an explicit WebHost downgrade instead of fake write controls", async () => {
    const harness = await createHarness(REVIEW, {
      listContractReviews: vi.fn().mockRejectedValue({ code: "NOT_CONFIGURED", message: "not configured" }),
    });
    harness.render();
    harness.runEffect(1);
    await flushPromises();
    const html = renderToStaticMarkup(harness.render());

    expect(html).toContain("当前运行环境不支持合同审查");
    expect(html).toContain("WebHost 不会伪造创建、决策或报告成功");
    expect(html).not.toContain("新建并启动审查");
    expect(html).not.toContain("生成 DOCX 报告");
  });
});

async function createHarness(
  review: ContractReviewRecord,
  overrides: {
    workspace?: BusinessWorkspaceRecord | null;
    listContractReviews?: ReturnType<typeof vi.fn>;
  } = {},
) {
  const states: unknown[] = [];
  let effects: Array<() => void | (() => void)> = [];
  let cursor = 0;
  const useState = <Value,>(initialValue: Value | (() => Value)) => {
    const stateIndex = cursor++;
    if (!(stateIndex in states)) {
      states[stateIndex] = typeof initialValue === "function" ? (initialValue as () => Value)() : initialValue;
    }
    const setState = (nextValue: Value | ((current: Value) => Value)) => {
      const currentValue = states[stateIndex] as Value;
      states[stateIndex] = typeof nextValue === "function"
        ? (nextValue as (current: Value) => Value)(currentValue)
        : nextValue;
    };
    return [states[stateIndex] as Value, setState] as const;
  };
  const useEffect = (effect: () => void | (() => void)) => {
    effects.push(effect);
  };
  const useMemo = <Value,>(factory: () => Value) => factory();
  const useCallback = <Value extends (...args: never[]) => unknown>(callback: Value) => callback;

  vi.resetModules();
  vi.doMock("react", async () => {
    const actual = await vi.importActual<typeof import("react")>("react");
    return { ...actual, useState, useEffect, useMemo, useCallback };
  });
  const { ContractLegalCenter } = await import("./ContractLegalCenter");
  const getEvidenceContext = vi.fn().mockResolvedValue(EVIDENCE);
  const listContractReviews = overrides.listContractReviews ?? vi.fn().mockResolvedValue([review]);
  const client = {
    listContractReviews,
    getContractReview: vi.fn().mockResolvedValue(review),
    listReviewFindings: vi.fn().mockResolvedValue(review.findings),
    getEvidenceContext,
    subscribeContractReviewEvents: vi.fn().mockResolvedValue(() => undefined),
  } as unknown as BsaigcClient;
  const props: ContractLegalCenterProps = {
    client,
    projectId: "project-1",
    workspace: overrides.workspace === undefined ? WORKSPACE : overrides.workspace,
    attachmentCandidates: [{ id: "asset-contract", name: "华邦-年度框架合同.pdf", status: "ready" }],
    onClose: vi.fn(),
    onOpenAsset: vi.fn(),
  };
  const render = (): ReactNode => {
    cursor = 0;
    effects = [];
    return ContractLegalCenter(props);
  };

  return {
    getEvidenceContext,
    render,
    runEffect(index: number) {
      const effect = effects[index];
      if (!effect) throw new Error("missing effect " + index);
      effect();
    },
    async loadWorkspaceReview() {
      render();
      const loadEffect = effects[1];
      if (!loadEffect) throw new Error("missing review load effect");
      loadEffect();
      await flushPromises();
      render();
    },
  };
}

function buttonOpening(html: string, label: string): string {
  const labelIndex = html.indexOf(label);
  if (labelIndex < 0) throw new Error("missing button label: " + label);
  const buttonIndex = html.lastIndexOf("<button", labelIndex);
  const openingEnd = html.indexOf(">", buttonIndex);
  return html.slice(buttonIndex, openingEnd + 1);
}

async function flushPromises(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}
