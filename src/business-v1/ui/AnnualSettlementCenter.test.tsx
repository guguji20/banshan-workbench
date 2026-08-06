import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import {
  AnnualSettlementCenter,
  eligibleAnnualSettlementDeliverables,
  settlementAvailableQuantity,
  validateAnnualSettlementInput,
  type AnnualSettlementBatch,
  type AnnualSettlementBatchInput,
  type AnnualSettlementCenterProps,
  type AnnualSettlementWorkspace,
} from "./AnnualSettlementCenter";

const WORKSPACE: AnnualSettlementWorkspace = {
  id: "workspace-annual-1",
  projectTitle: "2026 年品牌内容年框",
  projectCode: "AF-2026-001",
  customerName: "客户甲",
  deliverables: [
    { id: "deliverable-q1", milestoneId: "milestone-q1", milestoneTitle: "第一季度", name: "Q1 品牌短片", unit: "条", contractQuantity: 4, executedQuantity: 4, acceptedQuantity: 4, settledQuantity: 0 },
    { id: "deliverable-q2", milestoneId: "milestone-q2", milestoneTitle: "第二季度", name: "Q2 品牌短片", unit: "条", contractQuantity: 5, executedQuantity: 4, acceptedQuantity: 3, settledQuantity: 1 },
    { id: "deliverable-pending", milestoneId: "milestone-q3", milestoneTitle: "第三季度", name: "待验收海报", unit: "张", contractQuantity: 8, executedQuantity: 5, acceptedQuantity: 0, settledQuantity: 0 },
  ],
};

const ACTIVE_BATCH: AnnualSettlementBatch = {
  id: "settlement-q1",
  workspaceId: WORKSPACE.id,
  period: "2026 年第一季度",
  cadence: "quarterly",
  status: "confirmed",
  lines: [{ deliverableId: "deliverable-q1", deliverableName: "Q1 品牌短片", milestoneTitle: "第一季度", unit: "条", quantity: 4 }],
  note: "第一季度已确认",
  createdAt: 100,
  updatedAt: 200,
};

const VOIDED_BATCH: AnnualSettlementBatch = {
  ...ACTIVE_BATCH,
  id: "settlement-voided",
  period: "作废批次",
  status: "voided",
  lines: [{ deliverableId: "deliverable-q2", deliverableName: "Q2 品牌短片", milestoneTitle: "第二季度", unit: "条", quantity: 2 }],
  updatedAt: 300,
  voidedAt: 300,
};

function props(overrides: Partial<AnnualSettlementCenterProps> = {}): AnnualSettlementCenterProps {
  return {
    workspace: WORKSPACE,
    settlementBatches: [ACTIVE_BATCH, VOIDED_BATCH],
    onUpsert: vi.fn(),
    onVoid: vi.fn(),
    ...overrides,
  };
}

describe("annual settlement rules", () => {
  it("uses accepted and contract remainder as the quantity ceiling", () => {
    expect(settlementAvailableQuantity(WORKSPACE.deliverables[1])).toBe(2);
    expect(settlementAvailableQuantity(WORKSPACE.deliverables[2])).toBe(0);
  });

  it("excludes deliverables reserved by active batches and restores voided items", () => {
    expect(eligibleAnnualSettlementDeliverables(WORKSPACE, [ACTIVE_BATCH, VOIDED_BATCH]).map((item) => item.id)).toEqual(["deliverable-q2"]);
    expect(eligibleAnnualSettlementDeliverables(WORKSPACE, [ACTIVE_BATCH], ACTIVE_BATCH.id).map((item) => item.id)).toEqual(["deliverable-q1", "deliverable-q2"]);
  });

  it("rejects missing periods, duplicate references, and excessive quantities", () => {
    const input: AnnualSettlementBatchInput = {
      id: null,
      workspaceId: WORKSPACE.id,
      period: " ",
      cadence: "quarterly",
      lines: [
        { deliverableId: "deliverable-q2", deliverableName: "Q2 品牌短片", milestoneTitle: "第二季度", unit: "条", quantity: 3 },
        { deliverableId: "deliverable-q2", deliverableName: "Q2 品牌短片", milestoneTitle: "第二季度", unit: "条", quantity: 1 },
      ],
      note: "",
    };

    expect(validateAnnualSettlementInput(input, WORKSPACE, [ACTIVE_BATCH, VOIDED_BATCH])).toEqual([
      "请填写结算期间",
      "交付项“Q2 品牌短片”最多可结算 2 条",
      "交付项“Q2 品牌短片”不能重复选择",
    ]);
  });

  it("rejects an item already held by another effective batch", () => {
    const input: AnnualSettlementBatchInput = {
      id: null,
      workspaceId: WORKSPACE.id,
      period: "2026 年第二季度",
      cadence: "quarterly",
      lines: ACTIVE_BATCH.lines.map((line) => ({ ...line })),
      note: "",
    };
    expect(validateAnnualSettlementInput(input, WORKSPACE, [ACTIVE_BATCH])).toContain("交付项“Q1 品牌短片”已结算或不可结算");
  });
});

describe("AnnualSettlementCenter", () => {
  it("renders period, cadence, quantity metrics, batch list, and void controls", () => {
    const html = renderToStaticMarkup(<AnnualSettlementCenter {...props()} />);

    expect(html).toContain("年框结算中心");
    expect(html).toContain("结算期间");
    expect(html).toContain("季度结算");
    expect(html).toContain("Q2 品牌短片");
    expect(html).toContain("剩余可结算");
    expect(html).not.toContain("待验收海报");
    expect(html).toContain("2026 年第一季度");
    expect(html).toContain("作废批次");
    expect(html).toContain("作废");
  });

  it("shows a clear empty state when every accepted item is reserved", () => {
    const q2Batch: AnnualSettlementBatch = {
      ...ACTIVE_BATCH,
      id: "settlement-q2",
      period: "2026 年第二季度",
      status: "draft",
      lines: [{ deliverableId: "deliverable-q2", deliverableName: "Q2 品牌短片", milestoneTitle: "第二季度", unit: "条", quantity: 2 }],
    };
    const html = renderToStaticMarkup(<AnnualSettlementCenter {...props({ settlementBatches: [ACTIVE_BATCH, q2Batch] })} />);

    expect(html).toContain("当前没有可结算交付项");
    expect(html).toContain("disabled");
  });

  it("keeps destructive controls out of voided batch cards", () => {
    const html = renderToStaticMarkup(<AnnualSettlementCenter {...props({ settlementBatches: [VOIDED_BATCH] })} />);
    const voidedCard = html.slice(html.indexOf("作废批次"));

    expect(voidedCard).toContain("已作废");
    expect(voidedCard).not.toContain(">编辑<");
    expect(voidedCard).not.toContain(">作废<");
  });
});
