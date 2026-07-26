import type { BusinessMilestoneStatus } from "../generated/bsaigc/BusinessMilestoneStatus";
import type { BusinessPaymentStatus } from "../generated/bsaigc/BusinessPaymentStatus";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import type { RequirementBriefRecord } from "../generated/bsaigc/RequirementBriefRecord";
import type { ReviewFindingRecord } from "../generated/bsaigc/ReviewFindingRecord";
import type { ReviewSeverity } from "../generated/bsaigc/ReviewSeverity";

/**
 * 商务智能体（V1）：一组带"办公技能"的角色预设。
 * 只负责组装提示词与项目上下文——草稿由现有 AI 会话生成，
 * 正式生效必须回到单据流程，智能体不落章、不改台账。
 */

export interface BusinessAgentSkill {
  id: string;
  label: string;
  hint: string;
  instruction: string;
}

export interface BusinessAgentDefinition {
  id: string;
  name: string;
  tagline: string;
  preamble: string;
  skills: readonly BusinessAgentSkill[];
}

export const BUSINESS_AGENTS: readonly BusinessAgentDefinition[] = [
  {
    id: "negotiation",
    name: "洽谈助手",
    tagline: "需求确认、追问与 Brief 整理",
    preamble:
      "你是一名资深商务策划，擅长把模糊的客户需求变成可执行的结构化 Brief。语气专业、克制，不堆砌形容词。",
    skills: [
      {
        id: "follow-up-questions",
        label: "生成追问清单",
        hint: "根据已有需求信息，列出还必须问清楚的问题",
        instruction:
          "根据上下文中的需求 Brief 与项目信息，列出目前仍不明确、必须向客户追问的问题清单。按「影响报价的」「影响拍摄执行的」「影响验收的」三组输出，每个问题一句话，并注明为什么要问。",
      },
      {
        id: "chat-to-brief",
        label: "聊天记录整理成 Brief",
        hint: "把零散沟通记录整理成结构化需求",
        instruction:
          "我会在下方粘贴与客户的零散沟通记录。请把它整理成结构化需求 Brief：项目目标、目标受众、核心信息、交付物、发布渠道、风格关键词、必须出现的元素、约束条件、验收标准、风险。原文没有提到的字段标注【待补充】，不要臆造。\n\n（在此粘贴聊天记录：）\n",
      },
      {
        id: "vague-feedback",
        label: "模糊反馈追问话术",
        hint: "客户说'不够高级/感觉不对'时怎么追问",
        instruction:
          "客户对方案或成片给出了模糊反馈（例如'不够高级''调性不对''再打磨一下'）。请生成一段可以直接发给客户的追问话术：礼貌地把模糊评价拆解成可选项（例如：高级感是指材质质感、构图留白、节奏快慢、色彩倾向，还是文案表达？），让客户做选择题而不是简答题。最后附一句确认改稿范围与轮次的话。",
      },
    ],
  },
  {
    id: "quotation",
    name: "报价助手",
    tagline: "行项起草、措辞润色、报价说明",
    preamble:
      "你是一名影视制作报价专家，熟悉视频制作各环节成本构成。输出的行项命名规范、客户可读，不虚报不漏项。",
    skills: [
      {
        id: "draft-line-items",
        label: "按需求起草报价行项",
        hint: "根据 Brief 建议报价结构（不含单价）",
        instruction:
          "根据上下文中的需求 Brief（交付物、渠道、风格、约束），起草一份报价行项建议：行项名称、简要说明、数量与单位。只搭结构不填单价，单价留给我决定。如某些交付物信息不足以判断工作量，单独列出需要确认的点。",
      },
      {
        id: "polish-line-items",
        label: "行项说明润色",
        hint: "把内部叫法改写成客户可读的服务说明",
        instruction:
          "把上下文中商务档案的服务明细行项，逐条润色成客户可读的服务说明：说清楚这一项包含什么、交付到什么程度（例如修改轮次、输出规格）。保持行项数量与顺序不变，不改金额，输出成「行项名称：说明」的列表。",
      },
      {
        id: "quote-cover-letter",
        label: "报价说明函",
        hint: "随报价单发给客户的一段说明",
        instruction:
          "根据上下文中的项目信息与服务明细，写一段随报价单发送给客户的说明函（150-250 字）：概述服务范围与交付物、说明报价有效期与不含税/含税口径（按上下文税率）、注明修改轮次约定，结尾礼貌推动确认。语气专业友好，不卑不亢。",
      },
    ],
  },
  {
    id: "contract-review",
    name: "合同审核助手",
    tagline: "风险转译、修改意见函、审查纪要",
    preamble:
      "你是一名熟悉影视服务合同的商务法务助理。表达严谨，引用条款时忠于原文，绝不编造合同内容。",
    skills: [
      {
        id: "revision-letter",
        label: "起草合同修改意见函",
        hint: "把审查发现整理成发给客户的修改意见",
        instruction:
          "根据上下文中的合同审查发现（以及我补充粘贴的条款原文），起草一份发给客户的合同修改意见函：逐条列出「原条款要点 → 我方修改建议 → 理由」，语气合作而坚定；结尾说明其余条款无异议、盼复时间。若上下文中没有审查发现，请列出影视服务合同最常见的 5 个风险点核对项并注明【请先在合同审查页完成审查】。",
      },
      {
        id: "review-minutes",
        label: "汇总审查纪要",
        hint: "把逐条决策整理成一页纪要",
        instruction:
          "把上下文中的合同审查发现与人工决策，整理成一页审查纪要：合同基本信息、发现总数与分布（严重/高/中/低）、已确认风险及处理决定、要求修改项、接受风险项及理由。格式适合存档与向上汇报。",
      },
      {
        id: "explain-clause",
        label: "大白话解释条款",
        hint: "把风险条款讲给非法务同事听",
        instruction:
          "我会在下方粘贴一段合同条款。请用大白话解释：这条约定了什么、对我方（乙方/服务方）最不利的情形是什么、谈判时可以怎么改。不超过 200 字，最后给一句一句话结论：能接受 / 建议改 / 必须改。\n\n（在此粘贴条款：）\n",
      },
    ],
  },
  {
    id: "acceptance-billing",
    name: "验收请款助手",
    tagline: "验收摘要、请款说明、回款跟进",
    preamble:
      "你是一名商务执行专员，负责验收与请款文书。所有金额、日期、节点必须以上下文提供的数据为准，缺什么标注什么，绝不编数。",
    skills: [
      {
        id: "acceptance-summary",
        label: "起草验收单交付摘要",
        hint: "根据里程碑与交付版本写交付摘要",
        instruction:
          "根据上下文中的里程碑与交付情况，起草验收单用的「交付摘要」与「验收结论」段落：列出已完成的交付物及版本、达成的验收标准、双方确认方式。可直接粘贴进验收单的交付摘要字段。未完成或未签收的项目单独列出，不要写进已交付部分。",
      },
      {
        id: "payment-request-note",
        label: "起草请款说明",
        hint: "引用合同条款与节点金额写请款函",
        instruction:
          "根据上下文中的合同金额、付款条款与付款节点，起草一份请款说明（发给客户对接人）：本次请款对应的节点与金额（大写+小写）、合同依据（付款条款原文要点）、已完成的交付/验收事实、收款账户信息占位【以请款单为准】、期望付款时间。语气专业、留有余地但明确。",
      },
      {
        id: "collection-reminder",
        label: "回款催收话术",
        hint: "按逾期程度分级的催收提醒",
        instruction:
          "根据上下文中的待收金额与付款节点状态，生成三档回款跟进话术：①未到期的温和提醒（顺带同步项目进展）；②刚到期的正式提醒（附请款信息要点）；③逾期较久的升级沟通（提及合同约定与后续安排，但保持合作姿态）。每档 80 字以内，可直接微信/邮件发送。",
      },
    ],
  },
];

export interface BusinessAgentContextInput {
  projectName: string | null;
  customerName: string | null;
  brief: RequirementBriefRecord | null;
  workspace: BusinessWorkspaceRecord | null;
  contractFindings: readonly ReviewFindingRecord[];
}

const PAYMENT_STATUS_TEXT: Record<BusinessPaymentStatus, string> = {
  planned: "计划中",
  requested: "已请款",
  partiallyReceived: "部分到账",
  received: "已到账",
  canceled: "已取消",
};

const MILESTONE_STATUS_TEXT: Record<BusinessMilestoneStatus, string> = {
  planned: "计划中",
  inProgress: "进行中",
  delivered: "已交付",
  accepted: "已签收",
  canceled: "已取消",
};

const SEVERITY_TEXT: Record<ReviewSeverity, string> = {
  info: "提示",
  low: "低",
  medium: "中",
  high: "高",
  critical: "严重",
};

const DECISION_TEXT: Record<ReviewFindingRecord["decision"], string> = {
  unreviewed: "未决策",
  confirmed: "已确认风险",
  rejected: "已驳回",
  acceptedRisk: "接受风险",
  needsRevision: "要求修改",
};

function formatCents(cents: number, currency: string): string {
  const amount = (cents / 100).toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  return `${currency === "CNY" || !currency ? "¥" : currency + " "}${amount}`;
}

function formatDay(timestamp: number | null): string {
  if (!timestamp) return "未定";
  const milliseconds = timestamp < 10_000_000_000 ? timestamp * 1000 : timestamp;
  const date = new Date(milliseconds);
  if (Number.isNaN(date.getTime())) return "未定";
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

function pushList(lines: string[], label: string, values: readonly string[]): void {
  const cleaned = values.map((value) => value.trim()).filter(Boolean);
  if (cleaned.length > 0) lines.push(`${label}：${cleaned.join("；")}`);
}

export function buildBusinessAgentContext(
  input: BusinessAgentContextInput,
): string {
  const lines: string[] = [];
  if (input.projectName) lines.push(`项目：${input.projectName}`);
  if (input.customerName) lines.push(`客户：${input.customerName}`);

  const brief = input.brief;
  if (brief) {
    const content = brief.content;
    if (content.objective.trim()) lines.push(`项目目标：${content.objective.trim()}`);
    if (content.keyMessage.trim()) lines.push(`核心信息：${content.keyMessage.trim()}`);
    pushList(lines, "交付物", content.deliverables);
    pushList(lines, "发布渠道", content.channels);
    pushList(lines, "风格关键词", content.styleKeywords);
    pushList(lines, "必须出现", content.mandatoryItems);
    pushList(lines, "验收标准", content.acceptanceCriteria);
    if (content.deadlineAt) lines.push(`交付时间：${formatDay(content.deadlineAt)}`);
    if (content.budgetNotes.trim()) lines.push(`预算说明：${content.budgetNotes.trim()}`);
  }

  const workspace = input.workspace;
  if (workspace) {
    const profile = workspace.profile;
    const summary = workspace.financialSummary;
    const currency = profile.currency || "CNY";
    if (summary.contractCents > 0) {
      lines.push(
        `合同金额：${formatCents(summary.contractCents, currency)}（已到账 ${formatCents(summary.receivedCents, currency)}，待收 ${formatCents(summary.outstandingCents, currency)}）`,
      );
    } else if (summary.quotedCents > 0) {
      lines.push(`报价金额：${formatCents(summary.quotedCents, currency)}（合同未生效）`);
    }
    if (profile.paymentTerms.trim()) lines.push(`付款条款：${profile.paymentTerms.trim()}`);
    if (profile.acceptanceTerms.trim()) lines.push(`验收条款：${profile.acceptanceTerms.trim()}`);
    if (profile.deliverySummary.trim()) lines.push(`交付摘要：${profile.deliverySummary.trim()}`);

    if (profile.lineItems.length > 0) {
      const items = profile.lineItems
        .slice(0, 12)
        .map((item) => {
          const quantity = item.quantityMillis % 1000 === 0
            ? String(item.quantityMillis / 1000)
            : (item.quantityMillis / 1000).toFixed(3);
          const price = item.unitPriceCents > 0
            ? `×${formatCents(item.unitPriceCents, currency)}`
            : "";
          return `${item.name}（${quantity}${item.unit}${price}）`;
        })
        .join("；");
      lines.push(`服务明细：${items}`);
    }

    if (workspace.payments.length > 0) {
      const payments = workspace.payments
        .slice(0, 8)
        .map(
          (payment) =>
            `${payment.label} ${formatCents(payment.amountCents, currency)}（${PAYMENT_STATUS_TEXT[payment.status]}${payment.dueAt ? `，约定 ${formatDay(payment.dueAt)}` : ""}）`,
        )
        .join("；");
      lines.push(`付款节点：${payments}`);
    }

    if (workspace.milestones.length > 0) {
      const milestones = workspace.milestones
        .slice(0, 8)
        .map((milestone) => {
          const versions = milestone.deliverables.flatMap(
            (deliverable) => deliverable.versions,
          );
          const accepted = versions.filter(
            (version) => version.status === "accepted",
          ).length;
          const versionNote = versions.length > 0 ? `，版本 ${accepted}/${versions.length} 已签收` : "";
          return `${milestone.title}（${MILESTONE_STATUS_TEXT[milestone.status]}${versionNote}）`;
        })
        .join("；");
      lines.push(`里程碑：${milestones}`);
    }

    const contract = workspace.documents.find(
      (document) => document.id === workspace.currentDocuments.contractDocumentId,
    );
    if (contract) {
      lines.push(`当前合同：${contract.documentNumber}（${contract.title}）`);
    }
  }

  if (input.contractFindings.length > 0) {
    const findings = input.contractFindings
      .slice(0, 8)
      .map(
        (finding) =>
          `[${SEVERITY_TEXT[finding.severity]}] ${finding.title}（${DECISION_TEXT[finding.decision]}）`,
      )
      .join("；");
    lines.push(`合同审查发现：${findings}`);
  }

  return lines.join("\n");
}

export function buildBusinessAgentPrompt(
  agent: BusinessAgentDefinition,
  skill: BusinessAgentSkill,
  contextText: string,
): string {
  const context = contextText.trim().length > 0
    ? contextText.trim()
    : "（这个项目还没有商务资料，我会在下面自己补充背景。）";
  return [
    `请你作为「${agent.name}」帮我做一件事。${agent.preamble}`,
    `要做的事：${skill.instruction}`,
    `——下面是这个项目的资料——\n${context}`,
    "写好后直接给我能用的中文成稿。金额和日期必须按上面资料来，资料里没有的就写【待补充】，不要自己编。",
  ].join("\n\n");
}
