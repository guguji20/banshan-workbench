import { describe, expect, test } from "vitest";
import type { ContractReviewRecord } from "../../generated/bsaigc/ContractReviewRecord";
import type { EvidenceAnchor } from "../../generated/bsaigc/EvidenceAnchor";
import type { EvidenceContext } from "../../generated/bsaigc/EvidenceContext";
import type { ReviewFindingRecord } from "../../generated/bsaigc/ReviewFindingRecord";
import type { ReviewReportRecord } from "../../generated/bsaigc/ReviewReportRecord";
import {
  CONTRACT_LEGAL_DECISION_LABELS,
  CONTRACT_LEGAL_SEVERITY_LABELS,
  CONTRACT_REVIEW_STAGE_LABELS,
  evaluateContractReportGate,
  evaluateContractReviewRetry,
  preferredContractReviewReport,
  projectContractLegalCapabilities,
  projectContractLegalEvidence,
  projectContractLegalFinding,
  projectContractLegalReview,
} from "./contractLegalView";

describe("contract legal application projection", () => {
  test("projects stable Chinese labels", () => {
    expect(CONTRACT_REVIEW_STAGE_LABELS).toMatchObject({
      created: "等待开始",
      awaitingOcr: "等待 OCR",
      reviewingAgent: "智能审查",
      awaitingConfirmation: "人工确认",
      completed: "本地完成",
    });
    expect(CONTRACT_LEGAL_SEVERITY_LABELS).toEqual({
      info: "提示",
      low: "低风险",
      medium: "中风险",
      high: "高风险",
      critical: "严重",
    });
    expect(CONTRACT_LEGAL_DECISION_LABELS).toEqual({
      unreviewed: "未决策",
      confirmed: "已确认风险",
      rejected: "已驳回",
      acceptedRisk: "接受风险",
      needsRevision: "要求修改",
    });
  });

  test("projects findings without leaking mutable protocol arrays", () => {
    const finding = createFinding({
      severity: "critical",
      evidenceIds: ["evidence-1"],
      missingEvidenceReason: "不应显示",
    });
    const view = projectContractLegalFinding(finding);

    expect(view).toMatchObject({
      severityLabel: "严重",
      decisionLabel: "未决策",
      needsDecision: true,
      evidenceCount: 1,
      hasEvidence: true,
      missingEvidenceReason: null,
    });
    view.evidenceIds.push("evidence-2");
    expect(finding.evidenceIds).toEqual(["evidence-1"]);
  });

  test("projects evidence page and optional loaded context", () => {
    const evidence = createEvidence();
    const context: EvidenceContext = {
      evidence,
      page: {
        id: "page-1",
        extractionId: "extraction-1",
        pageIndex: 1,
        text: "付款应在验收后十日内完成。",
        textSha256: hash("p"),
        width: 100,
        height: 200,
        previewAssetId: "preview-1",
      },
      block: {
        id: "block-1",
        extractionId: "extraction-1",
        pageId: "page-1",
        pageIndex: 1,
        orderIndex: 0,
        kind: "paragraph",
        text: "付款应在验收后十日内完成。",
        charStart: 4,
        charEnd: 14,
        bbox: null,
      },
    };

    expect(projectContractLegalEvidence(evidence, context)).toMatchObject({
      id: "evidence-1",
      pageIndex: 1,
      pageNumber: 2,
      quote: "验收后十日内付款",
      blockId: "block-1",
      blockText: "付款应在验收后十日内完成。",
      previewAssetId: "preview-1",
      hasPreciseLocation: true,
    });
  });

  test("gates reports on extraction, review state, capability and open findings", () => {
    const review = createReview({
      findings: [createFinding(), createFinding({ id: "finding-2", status: "decided", decision: "confirmed" })],
    });
    const blocked = evaluateContractReportGate(review);

    expect(blocked.canGenerate).toBe(false);
    expect(blocked.openFindingCount).toBe(1);
    expect(blocked.blockers).toContainEqual({
      code: "findingsAwaitingDecision",
      message: "仍有 1 项风险等待人工决策。",
    });

    const ready = createReview({
      findings: [createFinding({ status: "decided", decision: "needsRevision" })],
    });
    expect(evaluateContractReportGate(ready)).toEqual({
      canGenerate: true,
      openFindingCount: 0,
      blockers: [],
    });

    const degraded = projectContractLegalCapabilities({ isDesktopRuntime: false });
    expect(evaluateContractReportGate(ready, degraded).blockers[0]?.code).toBe(
      "capabilityUnavailable",
    );
  });

  test("selects retry stage for failed and degraded Agent reviews", () => {
    const failed = createReview({
      status: "failed",
      stage: "extracting",
      failure: {
        code: "CONTRACT_EXTRACTION_FAILED",
        message: "解析失败",
        retryable: true,
        stage: "extracting",
      },
    });
    expect(evaluateContractReviewRetry(failed)).toMatchObject({
      canRetry: true,
      stage: "extracting",
      mode: "failedStage",
      blockerCode: null,
    });

    const degradedAgent = createReview({
      failure: {
        code: "CONTRACT_AGENT_UNAVAILABLE",
        message: "智能审查暂不可用",
        retryable: true,
        stage: "mergingFindings",
      },
    });
    expect(evaluateContractReviewRetry(degradedAgent)).toMatchObject({
      canRetry: true,
      stage: "reviewingAgent",
      mode: "degradedAgent",
    });
  });

  test("protects human decisions from retry and reports capability degradation", () => {
    const decided = createReview({
      failure: {
        code: "CONTRACT_AGENT_UNAVAILABLE",
        message: "智能审查暂不可用",
        retryable: true,
        stage: "reviewingAgent",
      },
      decisions: [{
        id: "decision-1",
        reviewId: "review-1",
        findingId: "finding-1",
        decision: "acceptedRisk",
        comment: "业务接受",
        actorId: "user-1",
        findingRevision: 1,
        createdAt: 20,
      }],
    });
    expect(evaluateContractReviewRetry(decided)).toMatchObject({
      canRetry: false,
      blockerCode: "humanDecisionExists",
    });

    expect(projectContractLegalCapabilities({ isDesktopRuntime: false })).toEqual({
      reviewCommands: false,
      evidenceRead: false,
      reportGeneration: false,
      stageRetry: false,
      isDegraded: true,
      unavailable: ["reviewCommands", "evidenceRead", "reportGeneration", "stageRetry"],
      message: "当前运行环境不支持本地合同审查，仅可展示已加载的法务信息。",
    });
    expect(projectContractLegalCapabilities({
      isDesktopRuntime: true,
      evidenceRead: false,
    })).toMatchObject({
      reviewCommands: true,
      evidenceRead: false,
      reportGeneration: true,
      stageRetry: true,
      isDegraded: true,
      unavailable: ["evidenceRead"],
    });
  });

  test("builds the lightweight review summary and prefers newest DOCX report", () => {
    const reports = [
      createReport("json", 30),
      createReport("docx", 10),
      createReport("docx", 40),
      createReport("html", 50),
    ];
    const review = createReview({
      findings: [
        createFinding({ severity: "high", status: "decided", decision: "confirmed" }),
        createFinding({ id: "finding-2", severity: "low", status: "superseded" }),
      ],
      reports,
    });
    const view = projectContractLegalReview(review);

    expect(view).toMatchObject({
      id: "review-1",
      sourceFileName: "服务合同.docx",
      statusLabel: "待确认",
      stageLabel: "人工确认",
      findingCounts: {
        total: 2,
        awaitingDecision: 0,
        decided: 1,
        superseded: 1,
        bySeverity: { info: 0, low: 1, medium: 0, high: 1, critical: 0 },
      },
      reportGate: { canGenerate: true, openFindingCount: 0, blockers: [] },
    });
    expect(view.preferredReport?.generatedAt).toBe(40);
    expect(preferredContractReviewReport(reports)?.format).toBe("docx");
  });
});

function createReview(overrides: {
  status?: ContractReviewRecord["session"]["status"];
  stage?: ContractReviewRecord["session"]["stage"];
  failure?: ContractReviewRecord["session"]["failure"];
  findings?: ReviewFindingRecord[];
  decisions?: ContractReviewRecord["decisions"];
  reports?: ReviewReportRecord[];
} = {}): ContractReviewRecord {
  const evidence = createEvidence();
  return {
    session: {
      id: "review-1",
      workspaceId: "workspace-1",
      sourceAssetId: "asset-1",
      sourceAssetSha256: hash("a"),
      sourceFileName: "服务合同.docx",
      status: overrides.status ?? "awaitingConfirmation",
      stage: overrides.stage ?? "awaitingConfirmation",
      extractionId: "extraction-1",
      reportAssetId: null,
      revision: 3,
      createdAt: 1,
      updatedAt: 2,
      completedAt: null,
      failure: overrides.failure ?? null,
    },
    extraction: null,
    evidence: [evidence],
    findings: overrides.findings ?? [createFinding()],
    ruleEvaluations: [],
    decisions: overrides.decisions ?? [],
    reports: overrides.reports ?? [],
  };
}

function createFinding(
  overrides: Partial<ReviewFindingRecord> = {},
): ReviewFindingRecord {
  return {
    id: "finding-1",
    reviewId: "review-1",
    source: "rule",
    ruleId: "payment-term",
    ruleVersion: "1",
    agentRunId: null,
    category: "付款",
    severity: "high",
    title: "付款条件不完整",
    description: "缺少明确的付款触发条件。",
    recommendation: "补充验收后十日内付款。",
    evidenceIds: ["evidence-1"],
    missingEvidenceReason: null,
    status: "open",
    decision: "unreviewed",
    revision: 1,
    createdAt: 10,
    updatedAt: 10,
    ...overrides,
  };
}

function createEvidence(): EvidenceAnchor {
  return {
    id: "evidence-1",
    extractionId: "extraction-1",
    sourceAssetId: "asset-1",
    pageIndex: 1,
    blockId: "block-1",
    charStart: 4,
    charEnd: 14,
    bbox: null,
    quotedText: " 验收后十日内付款 ",
    quotedTextSha256: hash("e"),
    contextBefore: "付款条件：",
    contextAfter: "，乙方应开具发票。",
  };
}

function createReport(
  format: ReviewReportRecord["format"],
  generatedAt: number,
): ReviewReportRecord {
  return {
    id: `report-${format}-${generatedAt}`,
    reviewId: "review-1",
    reviewRevision: 3,
    sourceAssetId: "asset-1",
    sourceAssetSha256: hash("a"),
    extractionId: "extraction-1",
    ruleSetVersion: "business-contract-cn-1",
    agentRunIds: [],
    format,
    reportAssetId: `asset-report-${format}-${generatedAt}`,
    reportAssetSha256: hash("r"),
    generatedAt,
  };
}

function hash(value: string): string {
  return value.repeat(64).slice(0, 64);
}
