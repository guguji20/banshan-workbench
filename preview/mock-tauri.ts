/**
 * 预览专用：在浏览器中伪装 Tauri 宿主，用演示数据渲染完整界面。
 * 仅用于云端截图预览，不参与正式构建（vite/tauri 构建不引用本目录）。
 */

const T = 1_784_700_000_000; // 2026-07 前后的毫秒时间戳基准
const day = 86_400_000;

const receipt = (commandType: string) => ({
  commandId: crypto.randomUUID(),
  idempotencyKey: crypto.randomUUID(),
  commandType,
  aggregateId: "demo",
  revision: 1,
  lastEventSequence: 0,
  completedAt: T,
});

const brief = {
  objective: "让华邦的年度品牌片在发布会前上线",
  audience: "行业客户与投资人",
  deliverables: ["90 秒主片", "15 秒切条"],
  styleKeywords: ["克制", "真实质感"],
  mandatoryItems: ["品牌 Logo", "核心产品"],
  constraints: ["9 月发布会前交付"],
  risks: ["档期紧张"],
  referenceNotes: "参考去年展会开场片",
};

const projects = [
  {
    id: "project-1",
    name: "华邦年度品牌视频",
    clientName: "华邦",
    brief,
    stage: "postProduction",
    revision: 6,
    createdAt: T - 30 * day,
    updatedAt: T - day,
  },
  {
    id: "project-2",
    name: "蓝谷科技宣传片",
    clientName: "蓝谷科技",
    brief: { ...brief, objective: "新园区招商宣传", deliverables: ["3 分钟宣传片"] },
    stage: "briefing",
    revision: 2,
    createdAt: T - 6 * day,
    updatedAt: T - 2 * day,
  },
];

const tasks = [
  {
    id: "task-1",
    kind: "business.document.generate",
    projectId: "project-1",
    input: { document: "请款单" },
    output: null,
    status: "running",
    priority: "normal",
    replayPolicy: "safe",
    progress: 62,
    attempt: 1,
    maxAttempts: 3,
    revision: 3,
    createdAt: T - 3600_000,
    updatedAt: T - 60_000,
    startedAt: T - 3500_000,
    finishedAt: null,
    lastError: null,
    dependencies: [],
  },
  {
    id: "task-2",
    kind: "contract.review",
    projectId: "project-1",
    input: { file: "华邦-年度框架合同.pdf" },
    output: { findings: 3 },
    status: "succeeded",
    priority: "high",
    replayPolicy: "safe",
    progress: 100,
    attempt: 1,
    maxAttempts: 3,
    revision: 5,
    createdAt: T - 2 * day,
    updatedAt: T - 2 * day + 1800_000,
    startedAt: T - 2 * day,
    finishedAt: T - 2 * day + 1800_000,
    lastError: null,
    dependencies: [],
  },
];

const assets = [
  {
    id: "asset-contract",
    projectId: "project-1",
    originalName: "华邦-年度框架合同.pdf",
    kind: "document",
    mimeType: "application/pdf",
    sizeBytes: 1_284_301,
    sha256: "a".repeat(64),
    status: "ready",
    revision: 2,
    createdAt: T - 2 * day,
    updatedAt: T - 2 * day,
    previewAvailable: false,
  },
  {
    id: "asset-quote",
    projectId: "project-1",
    originalName: "华邦-报价单-Q1.xlsx",
    kind: "document",
    mimeType: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    sizeBytes: 48_231,
    sha256: "b".repeat(64),
    status: "ready",
    revision: 1,
    createdAt: T - 12 * day,
    updatedAt: T - 12 * day,
    previewAvailable: false,
  },
  {
    id: "asset-film",
    projectId: "project-1",
    originalName: "华邦主片-v2.mp4",
    kind: "video",
    mimeType: "video/mp4",
    sizeBytes: 812_002_113,
    sha256: "c".repeat(64),
    status: "ready",
    revision: 1,
    createdAt: T - day,
    updatedAt: T - day,
    previewAvailable: true,
  },
  {
    id: "asset-invoice",
    projectId: "project-1",
    originalName: "华邦-预付款发票.pdf",
    kind: "document",
    mimeType: "application/pdf",
    sizeBytes: 220_113,
    sha256: "d".repeat(64),
    status: "ready",
    revision: 1,
    createdAt: T - 5 * day,
    updatedAt: T - 5 * day,
    previewAvailable: false,
  },
  {
    id: "asset-signature",
    projectId: "project-1",
    originalName: "合同签署页-盖章.pdf",
    kind: "document",
    mimeType: "application/pdf",
    sizeBytes: 90_113,
    sha256: "e".repeat(64),
    status: "ready",
    revision: 1,
    createdAt: T - 10 * day,
    updatedAt: T - 10 * day,
    previewAvailable: false,
  },
  {
    id: "asset-report",
    projectId: "project-1",
    originalName: "华邦合同审查报告.docx",
    kind: "document",
    mimeType: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    sizeBytes: 96_004,
    sha256: "f".repeat(64),
    status: "ready",
    revision: 1,
    createdAt: T - 2 * day + 1800_000,
    updatedAt: T - 2 * day + 1800_000,
    previewAvailable: false,
  },
];

const brainThreads = [
  {
    id: "thread-1",
    projectId: "project-1",
    title: "帮我梳理华邦项目的回款进度",
    model: "gpt-5.6-sol",
    status: "ready",
    createdAt: T - day,
    updatedAt: T - 3600_000,
  },
];

const brainTurns = [
  {
    id: "turn-1",
    threadId: "thread-1",
    status: "completed",
    inputText: "帮我梳理华邦项目的回款进度，还有多少尾款没有到账？",
    assistantText:
      "华邦年度品牌视频项目：合同总额 ¥106,000，已到账预付款 ¥53,000（50%），尾款 ¥53,000 已发起请款、等待客户付款。建议本周跟进验收单签署，验收生效后即可催收尾款。",
    error: null,
    createdAt: T - 3700_000,
    updatedAt: T - 3600_000,
  },
];

const requirementBriefs = [
  {
    id: "req-1",
    projectId: "project-1",
    questionSetVersion: "v1",
    answers: [
      {
        questionId: "q-goal",
        prompt: "这次投放最想改变什么业务结果？",
        required: true,
        answer: "发布会现场播放并沉淀官网素材",
        disposition: "answered",
      },
      {
        questionId: "q-approver",
        prompt: "最终验收人是谁？",
        required: true,
        answer: "市场部王总监",
        disposition: "answered",
      },
    ],
    content: {
      objective: "年度品牌片在 9 月发布会前上线",
      audience: "行业客户与投资人",
      keyMessage: "华邦十年，可信赖的行业伙伴",
      deliverables: ["90 秒主片", "15 秒切条"],
      channels: ["发布会大屏", "视频号"],
      styleKeywords: ["克制", "真实质感"],
      mandatoryItems: ["品牌 Logo", "核心产品"],
      constraints: ["9 月发布会前交付"],
      acceptanceCriteria: ["王总监确认成片", "出具书面验收单"],
      risks: ["档期紧张"],
      deadlineAt: T + 30 * day,
      budgetNotes: "含税 10.6 万",
      referenceCaseIds: [],
      referenceNotes: "",
    },
    status: "confirmed",
    confirmedAt: T - 14 * day,
    confirmedBy: "operator-local",
    revision: 4,
    createdAt: T - 20 * day,
    updatedAt: T - 14 * day,
  },
];

const executionBriefs = [
  {
    id: "exec-1",
    projectId: "project-1",
    content: {
      shootAt: T + 3 * day,
      clientGoal: "发布会开场 90 秒主片",
      visualStyle: "自然光为主，克制的品牌色点缀",
      primaryShots: ["园区空镜航拍", "创始人访谈"],
      secondaryShots: ["产线细节", "团队协作"],
      requiredShots: ["品牌 Logo 墙", "核心产品特写"],
      fallbackShots: ["雨天改室内访谈"],
      riskPoints: ["天气", "创始人档期"],
      waitingTimeActions: ["补拍产品静物"],
      equipmentNotes: "FX6 双机位 + 无人机",
      postShootHighlights: ["航拍开场", "访谈金句"],
    },
    status: "ready",
    revision: 2,
    createdAt: T - 8 * day,
    updatedAt: T - 6 * day,
  },
];

const cases = [
  {
    id: "case-1",
    assetId: "asset-film",
    projectId: "project-1",
    title: "华邦主片 v2（发布会开场）",
    clientName: "华邦",
    contentType: "brand",
    presentation: "liveAction",
    hasActors: true,
    isAigc: false,
    qualityTier: "premium",
    tags: ["品牌片", "发布会", "航拍"],
    notes: "适合作为制造业品牌片提案参考",
    revision: 1,
    createdAt: T - day,
    updatedAt: T - day,
  },
];

const customer = {
  id: "customer-1",
  displayName: "华邦",
  legalName: "华邦精密制造有限公司",
  taxId: "91330100MA27XW1234",
  billingAddress: "杭州市滨江区江陵路 88 号",
  primaryContactName: "王总监",
  primaryPhone: "138-0000-0000",
  primaryEmail: "wang@huabang.example",
  notes: "",
  status: "active",
  revision: 3,
  createdAt: T - 30 * day,
  updatedAt: T - 5 * day,
};

const profile = {
  projectTitle: "华邦年度品牌视频",
  projectCode: "HB-2026",
  customerName: "华邦",
  customerLegalName: "华邦精密制造有限公司",
  customerTaxId: "91330100MA27XW1234",
  customerAddress: "杭州市滨江区江陵路 88 号",
  customerContact: "王总监",
  customerPhone: "138-0000-0000",
  customerEmail: "wang@huabang.example",
  supplierLegalName: "半山文化传媒有限公司",
  supplierTaxId: "91330100MA28YT5678",
  supplierAddress: "杭州市西湖区文三路 100 号",
  supplierContact: "陈经理",
  supplierPhone: "139-0000-0000",
  supplierBankName: "招商银行杭州分行",
  supplierBankAccount: "571908888810001",
  currency: "CNY",
  defaultTaxRateBps: 600,
  serviceStartAt: T - 20 * day,
  serviceEndAt: T + 30 * day,
  deliverySummary: "90 秒主片 1 条 + 15 秒切条 2 条",
  paymentTerms: "签约付 50%，验收合格后 7 日内付尾款 50%",
  acceptanceTerms: "客户书面确认成片并出具验收单",
  notes: "",
  lineItems: [
    {
      id: "line-1",
      name: "品牌视频策划与制作",
      description: "含策划、拍摄、后期",
      quantityMillis: 1_000,
      unit: "项",
      unitPriceCents: 10_000_000,
      taxRateBps: 600,
      amountCents: 10_600_000,
    },
  ],
};

const snapshotBase = {
  workspaceRevision: 5,
  customerId: customer.id,
  customer,
  profile,
};

const quoteDoc = {
  id: "doc-quote",
  kind: "quote",
  sequenceNumber: 1,
  documentNumber: "Q-HB-2026-01",
  title: "华邦年度品牌视频报价单",
  templateKey: "builtin.quote.standard.v1",
  status: "generated",
  snapshot: { ...snapshotBase, payment: null },
  outputAssetId: "asset-quote",
  outputFormat: "xlsx",
  sourceAssetId: null,
  reviewId: null,
  reportAssetId: null,
  evidence: null,
  manualWaiver: null,
  voidedAt: null,
  voidedBy: null,
  voidReason: "",
  approvedAt: T - 13 * day,
  approvedBy: "operator-local",
  generatedAt: T - 12 * day,
  revision: 4,
  createdAt: T - 14 * day,
  updatedAt: T - 12 * day,
};

const contractDoc = {
  ...quoteDoc,
  id: "doc-contract",
  kind: "contract",
  documentNumber: "C-HB-2026-01",
  title: "华邦年度品牌视频合同",
  templateKey: "builtin.contract.standard.v1",
  status: "effective",
  outputAssetId: "asset-contract",
  outputFormat: "docx",
  reviewId: "review-1",
  evidence: {
    kind: "contractSignature",
    assetId: "asset-signature",
    sha256: "e".repeat(64),
    occurredAt: T - 10 * day,
    note: "双方盖章版",
    recordedBy: "operator-local",
    recordedAt: T - 10 * day,
  },
  approvedAt: T - 11 * day,
  generatedAt: T - 11 * day,
  revision: 6,
  createdAt: T - 11 * day,
  updatedAt: T - 10 * day,
};

const payment1 = {
  id: "pay-1",
  label: "预付款 50%",
  amountCents: 5_300_000,
  dueAt: T - 9 * day,
  occurredAt: T - 8 * day,
  status: "received",
  reference: "CMB-20260718-0001",
  notes: "",
  revision: 4,
  createdAt: T - 11 * day,
  updatedAt: T - 8 * day,
};

const payment2 = {
  id: "pay-2",
  label: "尾款 50%",
  amountCents: 5_300_000,
  dueAt: T + 7 * day,
  occurredAt: null,
  status: "requested",
  reference: "",
  notes: "验收合格后 7 日内支付",
  revision: 3,
  createdAt: T - 11 * day,
  updatedAt: T - 3 * day,
};

const paymentRequestDoc = {
  ...quoteDoc,
  id: "doc-payreq",
  kind: "paymentRequest",
  documentNumber: "PR-HB-2026-02",
  title: "华邦尾款请款单",
  templateKey: "builtin.payment-request.standard.v1",
  status: "generated",
  snapshot: { ...snapshotBase, payment: payment2 },
  outputAssetId: "asset-quote",
  outputFormat: "docx",
  reviewId: null,
  evidence: null,
  approvedAt: T - 3 * day,
  generatedAt: T - 3 * day,
  revision: 3,
  createdAt: T - 3 * day,
  updatedAt: T - 3 * day,
};

const acceptanceDoc = {
  ...quoteDoc,
  id: "doc-acceptance",
  kind: "acceptance",
  documentNumber: "A-HB-2026-01",
  title: "华邦年度品牌视频验收单",
  templateKey: "builtin.acceptance.standard.v1",
  status: "approved",
  snapshot: { ...snapshotBase, payment: null },
  outputAssetId: null,
  outputFormat: null,
  reviewId: null,
  evidence: null,
  approvedAt: T - day,
  generatedAt: null,
  revision: 2,
  createdAt: T - 2 * day,
  updatedAt: T - day,
};

const workspace = {
  id: "workspace-1",
  projectId: "project-1",
  customerId: customer.id,
  customer,
  requirementBriefId: "req-1",
  requirementBriefRevision: 4,
  prefillSourceWorkspaceId: null,
  profile,
  documents: [quoteDoc, contractDoc, paymentRequestDoc, acceptanceDoc],
  payments: [payment1, payment2],
  quoteConfirmations: [
    {
      id: "qc-1",
      quoteDocumentId: quoteDoc.id,
      quoteDocumentRevision: quoteDoc.revision,
      quoteAssetId: "asset-quote",
      quoteSha256: "b".repeat(64),
      confirmationVersion: "Q-HB-2026-01-R4",
      customerRepresentative: "王总监",
      evidence: {
        kind: "quoteConfirmation",
        assetId: "asset-signature",
        sha256: "e".repeat(64),
        occurredAt: T - 12 * day,
        note: "邮件确认截图",
        recordedBy: "operator-local",
        recordedAt: T - 12 * day,
      },
      notes: "",
      confirmedBy: "operator-local",
      confirmedAt: T - 12 * day,
    },
  ],
  receipts: [
    {
      id: "receipt-1",
      paymentId: payment1.id,
      kind: "receipt",
      amountCents: 5_300_000,
      occurredAt: T - 8 * day,
      reference: "CMB-20260718-0001",
      notes: "预付款到账",
      reversesReceiptId: null,
      evidence: {
        kind: "receiptProof",
        assetId: "asset-invoice",
        sha256: "d".repeat(64),
        occurredAt: T - 8 * day,
        note: "银行回单",
        recordedBy: "operator-local",
        recordedAt: T - 8 * day,
      },
      recordedBy: "operator-local",
      createdAt: T - 8 * day,
    },
  ],
  milestones: [
    {
      id: "ms-1",
      sequenceNumber: 1,
      title: "成片交付",
      description: "主片 + 切条全部交付",
      dueAt: T + 2 * day,
      acceptanceCriteria: "王总监书面确认成片",
      required: true,
      status: "delivered",
      deliverables: [
        {
          id: "dlv-1",
          milestoneId: "ms-1",
          name: "90 秒主片",
          required: true,
          versions: [
            {
              id: "ver-1",
              deliverableId: "dlv-1",
              milestoneId: "ms-1",
              name: "90 秒主片",
              required: true,
              versionNumber: 2,
              artifact: {
                role: "deliverable",
                assetId: "asset-film",
                sha256: "c".repeat(64),
                sizeBytes: 812_002_113,
                originalName: "华邦主片-v2.mp4",
              },
              status: "sent",
              notes: "按王总监意见调整片尾",
              createdBy: "operator-local",
              createdAt: T - day,
            },
          ],
        },
      ],
      revision: 4,
      createdAt: T - 9 * day,
      updatedAt: T - day,
    },
  ],
  deliverySubmissions: [
    {
      id: "sub-1",
      milestoneId: "ms-1",
      submissionNumber: 1,
      versionIds: ["ver-1"],
      recipient: "王总监",
      channel: "企业网盘",
      note: "v2 修改版，请查收",
      sentAt: T - day,
      sentBy: "operator-local",
      status: "sent",
      signoffs: [],
    },
  ],
  invoices: [
    {
      id: "inv-1",
      paymentId: payment1.id,
      kind: "issued",
      status: "issued",
      invoiceCode: "033001900211",
      invoiceNumber: "25317888",
      issuerTaxId: "91330100MA28YT5678",
      buyerTaxId: "91330100MA27XW1234",
      currency: "CNY",
      amountCents: 5_300_000,
      taxCents: 300_000,
      issuedAt: T - 7 * day,
      originalInvoiceId: null,
      reversalReason: "",
      artifacts: [
        {
          role: "invoice",
          assetId: "asset-invoice",
          sha256: "d".repeat(64),
          sizeBytes: 220_113,
          originalName: "华邦-预付款发票.pdf",
        },
      ],
      recordedBy: "operator-local",
      createdAt: T - 7 * day,
    },
  ],
  archiveSnapshots: [],
  archiveIntegrityStatus: "notCaptured",
  status: "active",
  archivedAt: null,
  archivedBy: null,
  lifecycleStage: "paymentRequested",
  financialSummary: {
    quotedCents: 10_600_000,
    contractCents: 10_600_000,
    scheduledCents: 10_600_000,
    requestedCents: 5_300_000,
    receivedCents: 5_300_000,
    outstandingCents: 5_300_000,
  },
  currentDocuments: {
    quoteDocumentId: quoteDoc.id,
    contractDocumentId: contractDoc.id,
    paymentRequestDocumentId: paymentRequestDoc.id,
    acceptanceDocumentId: null,
  },
  revision: 18,
  createdAt: T - 20 * day,
  updatedAt: T - day,
};

const contractReview = {
  session: {
    id: "review-1",
    workspaceId: workspace.id,
    sourceAssetId: "asset-contract",
    sourceAssetSha256: "a".repeat(64),
    sourceFileName: "华邦-年度框架合同.pdf",
    status: "awaitingConfirmation",
    stage: "awaitingConfirmation",
    extractionId: "ext-1",
    reportAssetId: null,
    revision: 8,
    createdAt: T - 2 * day,
    updatedAt: T - 2 * day + 1800_000,
    completedAt: null,
    failure: null,
  },
  extraction: {
    id: "ext-1",
    reviewId: "review-1",
    sourceAssetId: "asset-contract",
    sourceAssetSha256: "a".repeat(64),
    parser: { name: "pdfium", version: "1.0", mode: "native" },
    ocr: null,
    status: "completed",
    pageCount: 12,
    contentSha256: "9".repeat(64),
    snapshotAssetId: null,
    pages: [],
    blocks: [],
    tables: [],
    createdAt: T - 2 * day,
    completedAt: T - 2 * day + 600_000,
    failure: null,
  },
  evidence: [
    {
      id: "ev-1",
      extractionId: "ext-1",
      sourceAssetId: "asset-contract",
      pageIndex: 4,
      blockId: "b-12",
      charStart: 120,
      charEnd: 210,
      bbox: null,
      quotedText: "乙方逾期交付的，每逾期一日按合同总额的 5% 支付违约金。",
      quotedTextSha256: "1".repeat(64),
      contextBefore: "第七条 违约责任：",
      contextAfter: "累计不超过合同总额的 50%。",
    },
  ],
  findings: [
    {
      id: "finding-1",
      reviewId: "review-1",
      source: "rule",
      ruleId: "RULE-PENALTY-RATE",
      ruleVersion: "1.2",
      agentRunId: null,
      category: "违约责任",
      severity: "critical",
      title: "逾期违约金比例过高（日 5%）",
      description: "合同第七条约定每日 5% 的逾期违约金，远高于行业惯例（日 0.05%–0.1%），累计上限 50% 也明显过高。",
      recommendation: "建议改为每日 0.05%，累计不超过合同总额的 10%。",
      evidenceIds: ["ev-1"],
      missingEvidenceReason: null,
      status: "decided",
      decision: "needsRevision",
      revision: 3,
      createdAt: T - 2 * day + 700_000,
      updatedAt: T - 2 * day + 1700_000,
    },
    {
      id: "finding-2",
      reviewId: "review-1",
      source: "agent",
      ruleId: null,
      ruleVersion: null,
      agentRunId: "run-1",
      category: "知识产权",
      severity: "high",
      title: "成片著作权归属未约定分镜与素材",
      description: "合同仅约定成片著作权归甲方，未明确工程文件与拍摄素材的归属及二次使用边界。",
      recommendation: "补充条款：工程文件与素材归乙方，甲方享有成片使用权。",
      evidenceIds: [],
      missingEvidenceReason: "原文未找到对应条款",
      status: "open",
      decision: "unreviewed",
      revision: 1,
      createdAt: T - 2 * day + 800_000,
      updatedAt: T - 2 * day + 800_000,
    },
    {
      id: "finding-3",
      reviewId: "review-1",
      source: "rule",
      ruleId: "RULE-PAYMENT-TERMS",
      ruleVersion: "1.2",
      agentRunId: null,
      category: "付款条款",
      severity: "medium",
      title: "尾款支付条件缺少明确期限",
      description: "验收合格后付款未约定具体天数。",
      recommendation: "补充「验收合格后 7 个工作日内支付」。",
      evidenceIds: [],
      missingEvidenceReason: null,
      status: "decided",
      decision: "confirmed",
      revision: 2,
      createdAt: T - 2 * day + 750_000,
      updatedAt: T - 2 * day + 1600_000,
    },
  ],
  ruleEvaluations: [],
  decisions: [
    {
      id: "dec-1",
      reviewId: "review-1",
      findingId: "finding-3",
      decision: "confirmed",
      comment: "已与客户口头确认补充 7 个工作日条款",
      actorId: "operator-local",
      findingRevision: 1,
      createdAt: T - 2 * day + 1600_000,
    },
    {
      id: "dec-2",
      reviewId: "review-1",
      findingId: "finding-1",
      decision: "needsRevision",
      comment: "要求法务按 0.05%/日 重拟",
      actorId: "operator-local",
      findingRevision: 2,
      createdAt: T - 2 * day + 1700_000,
    },
  ],
  reports: [
    {
      id: "report-1",
      reviewId: "review-1",
      reviewRevision: 7,
      sourceAssetId: "asset-contract",
      sourceAssetSha256: "a".repeat(64),
      extractionId: "ext-1",
      ruleSetVersion: "1.2",
      agentRunIds: ["run-1"],
      format: "docx",
      reportAssetId: "asset-report",
      reportAssetSha256: "f".repeat(64),
      generatedAt: T - 2 * day + 1800_000,
    },
  ],
};

const aiStatus = {
  provider: "bsaigc",
  configured: true,
  persisted: true,
  protection: "windowsDpapiCurrentUser",
  revision: 3,
  updatedAt: T - 4 * day,
  appliesOnNextRuntimeStart: false,
  defaultProviderId: "provider-banshan",
  defaultModel: "gpt-5.6-sol",
  providers: [
    {
      id: "provider-banshan",
      name: "半山 AIGC",
      kind: "openAiCompatible",
      baseUrl: "https://ai.banshan.example/v1",
      apiKeyConfigured: true,
      apiKeyHint: "••••1b18",
      models: ["gpt-5.6-sol", "gpt-5.6-sol-mini"],
      defaultModel: "gpt-5.6-sol",
      isDefault: true,
      enabled: true,
      connection: {
        state: "ready",
        message: "连接正常",
        latencyMs: 412,
        testedAt: T - 4 * day,
        discoveredModels: ["gpt-5.6-sol", "gpt-5.6-sol-mini"],
      },
      createdAt: T - 20 * day,
      updatedAt: T - 4 * day,
    },
  ],
};

const desktopSettings = {
  storage: {
    dataRoot: "D:\\BSAIGC\\data",
    totalBytes: 96_215_113_002,
    cacheBytes: 1_204_113_000,
    locations: [
      { target: "database", label: "业务数据库", path: "D:\\BSAIGC\\data\\ledger", sizeBytes: 88_211_002, exists: true, authoritative: true, clearable: false },
      { target: "vault", label: "Local Vault", path: "D:\\BSAIGC\\data\\vault", sizeBytes: 94_002_113_000, exists: true, authoritative: true, clearable: false },
      { target: "cache", label: "可再生缓存", path: "D:\\BSAIGC\\data\\cache", sizeBytes: 1_204_113_000, exists: true, authoritative: false, clearable: true },
    ],
  },
  channelAdapters: [
    { id: "feishu-cli", name: "飞书 CLI", state: "planned", configured: false, capabilities: [], message: "" },
  ],
  cloudBackup: {
    provider: "cloudflare-r2",
    mode: "async",
    configured: true,
    ready: true,
    state: "ready",
    message: "备份队列空闲",
    pendingItems: 0,
  },
  update: {
    currentVersion: "1.2.2",
    buildChannel: "internalPreview",
    buildVersion: "1.2.2+preview",
    codexRuntimeVersion: "codex-cli 0.144.5",
    updateSourceConfigured: false,
    automaticInstallAllowed: false,
    state: "idle",
    message: "",
    lastCheckedAt: null,
  },
  revision: 2,
};

const businessCustomers = [
  {
    customerId: customer.id,
    customerKey: customer.id,
    customerName: "华邦",
    customerLegalName: "华邦精密制造有限公司",
    customerTaxId: "91330100MA27XW1234",
    customerContact: "王总监",
    customerPhone: "138-0000-0000",
    customerEmail: "wang@huabang.example",
    customerStatus: "active",
    customerRevision: 3,
    workspaceCount: 1,
    activeWorkspaceCount: 1,
    contractCents: 10_600_000,
    requestedCents: 5_300_000,
    receivedCents: 5_300_000,
    outstandingCents: 5_300_000,
    workspaceIds: ["workspace-1"],
    updatedAt: T - day,
  },
];

const backups = [
  {
    assetId: "asset-contract",
    contentSha256: "a".repeat(64),
    state: "backedUp",
    attemptCount: 1,
    nextAttemptAt: null,
    lastError: null,
    remoteObjectKey: "vault/asset-contract",
    remoteEtag: "etag-1",
    revision: 2,
    createdAt: T - 2 * day,
    updatedAt: T - 2 * day + 400_000,
    backedUpAt: T - 2 * day + 400_000,
  },
  {
    assetId: "asset-report",
    contentSha256: "f".repeat(64),
    state: "queued",
    attemptCount: 0,
    nextAttemptAt: T + 600_000,
    lastError: null,
    remoteObjectKey: null,
    remoteEtag: null,
    revision: 1,
    createdAt: T - 1800_000,
    updatedAt: T - 1800_000,
    backedUpAt: null,
  },
];

const hostStatus = {
  protocolVersion: "1.5",
  databaseReady: true,
  vaultReady: true,
  projectCount: projects.length,
  taskCount: tasks.length,
  assetCount: assets.length,
  lastEventSequence: 128,
  runtime: "preview-mock",
  modules: [],
};

const codexStatus = {
  available: true,
  runtime: "codex-cli 0.144.5",
  transport: "app-server",
  userAgent: "bsaigc-desktop/1.2.2",
  platformFamily: "windows",
  platformOs: "windows 11",
  codexHomeReady: true,
  source: "bundled",
  handshakeAt: T - 3600_000,
  error: null,
};

const authState = {
  initialized: true,
  currentUser: null as null | { username: string; role: string; status: string; updatedAt: number },
  registrySync: "synced",
  registryMessage: null as string | null,
  registryRevision: 12,
  users: [
    { username: "老板", role: "admin", status: "active", updatedAt: T - 30 * day },
    { username: "市场部小李", role: "member", status: "active", updatedAt: T - 6 * day },
    { username: "市场部小王", role: "member", status: "active", updatedAt: T - 2 * day },
  ],
};

function authStatusPayload() {
  return {
    initialized: authState.initialized,
    currentUser: authState.currentUser,
    registrySync: authState.registrySync,
    registryMessage: authState.registryMessage,
    registryRevision: authState.registryRevision,
    userCount: authState.users.length,
  };
}

function authUsersPayload() {
  return {
    users: authState.users.map(({ username, role, status, updatedAt }) => ({ username, role, status, updatedAt })),
    registrySync: authState.registrySync,
    registryMessage: authState.registryMessage,
    registryRevision: authState.registryRevision,
  };
}

async function handleInvoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "brain_thread_archive":
      return { id: (args as { threadId?: string })?.threadId ?? "thread-1", projectId: "project-1", title: "已归档对话", model: "gpt-5.6-sol", status: "archived", createdAt: T - day, updatedAt: T };
    case "brain_thread_delete":
      return null;
    case "auth_status":
      return authStatusPayload();
    case "auth_login": {
      const credentials = (args as { credentials?: { username?: string } })?.credentials;
      authState.currentUser = {
        username: credentials?.username || "老板",
        role: credentials?.username === "老板" || !credentials?.username ? "admin" : "member",
        status: "active",
        updatedAt: T - day,
      };
      return authStatusPayload();
    }
    case "auth_initialize_admin": {
      const credentials = (args as { credentials?: { username?: string } })?.credentials;
      authState.initialized = true;
      authState.currentUser = { username: credentials?.username || "老板", role: "admin", status: "active", updatedAt: T };
      return authStatusPayload();
    }
    case "auth_logout":
      authState.currentUser = null;
      return authStatusPayload();
    case "auth_change_password":
    case "auth_refresh_registry":
      return authStatusPayload();
    case "auth_list_users":
      return authUsersPayload();
    case "auth_create_user": {
      const payload = (args as { payload?: { username?: string; role?: string } })?.payload;
      if (payload?.username) {
        authState.users.push({ username: payload.username, role: payload.role || "member", status: "active", updatedAt: T });
        authState.registryRevision += 1;
      }
      return authUsersPayload();
    }
    case "auth_reset_password":
      authState.registryRevision += 1;
      return authUsersPayload();
    case "auth_delete_user": {
      const payload = (args as { payload?: { username?: string } })?.payload;
      authState.users = authState.users.filter((user) => user.username !== payload?.username);
      authState.registryRevision += 1;
      return authUsersPayload();
    }
    case "plugin:event|listen":
      return 1;
    case "plugin:event|unlisten":
      return null;
    case "list_projects":
      return projects;
    case "list_tasks":
      return tasks;
    case "list_assets":
      return assets;
    case "list_cases":
      return cases;
    case "list_execution_briefs":
      return executionBriefs;
    case "list_requirement_briefs":
      return requirementBriefs;
    case "list_business_workspaces":
      return [workspace];
    case "list_business_customers":
      return businessCustomers;
    case "list_business_workspace_prefill_candidates":
      if ((args as { request?: { targetProjectId?: string } })?.request?.targetProjectId === "project-2") {
        return [
          {
            sourceWorkspaceId: "ws-lg-2025",
            sourceProjectId: "project-lg-2025",
            sourceProjectTitle: "蓝谷科技园区招商片（2025）",
            customerName: "蓝谷科技",
            customerLegalName: "蓝谷科技（深圳）有限公司",
            supplierLegalName: "半山影像（深圳）有限公司",
            matchKind: "customerName",
            populatedFields: [
              "customerLegalName",
              "customerTaxId",
              "customerAddress",
              "customerContact",
              "customerPhone",
              "supplierLegalName",
              "supplierTaxId",
              "supplierBankName",
              "supplierBankAccount",
              "currency",
              "defaultTaxRateBps",
            ],
            status: "archived",
            sourceRevision: 18,
            sourceUpdatedAt: T - 200 * day,
          },
        ];
      }
      return [];
    case "preview_business_workspace_prefill":
      return {
        targetProjectId: "project-2",
        targetProjectTitle: "蓝谷科技宣传片",
        targetCustomerName: "蓝谷科技",
        targetRequirementBriefId: null,
        sourceWorkspaceId: "ws-lg-2025",
        sourceProjectId: "project-lg-2025",
        sourceProjectTitle: "蓝谷科技园区招商片（2025）",
        matchKind: "customerName",
        sourceRevision: 18,
        sourceUpdatedAt: T - 200 * day,
        changes: [
          { field: "customerLegalName", targetValue: "", sourceValue: "蓝谷科技（深圳）有限公司", resultValue: "蓝谷科技（深圳）有限公司", decision: "filled" },
          { field: "customerTaxId", targetValue: "", sourceValue: "91440300MA5LGT2K8Q", resultValue: "91440300MA5LGT2K8Q", decision: "filled" },
          { field: "customerAddress", targetValue: "", sourceValue: "深圳市南山区蓝谷科技园 A 座 12 层", resultValue: "深圳市南山区蓝谷科技园 A 座 12 层", decision: "filled" },
          { field: "customerContact", targetValue: "", sourceValue: "陈明", resultValue: "陈明", decision: "filled" },
          { field: "customerPhone", targetValue: "", sourceValue: "13800001234", resultValue: "13800001234", decision: "filled" },
          { field: "customerEmail", targetValue: "", sourceValue: "", resultValue: "", decision: "unchanged" },
          { field: "supplierLegalName", targetValue: "", sourceValue: "半山影像（深圳）有限公司", resultValue: "半山影像（深圳）有限公司", decision: "filled" },
          { field: "supplierTaxId", targetValue: "", sourceValue: "91440300MA5FYW9P3T", resultValue: "91440300MA5FYW9P3T", decision: "filled" },
          { field: "supplierAddress", targetValue: "", sourceValue: "", resultValue: "", decision: "unchanged" },
          { field: "supplierContact", targetValue: "", sourceValue: "", resultValue: "", decision: "unchanged" },
          { field: "supplierPhone", targetValue: "", sourceValue: "", resultValue: "", decision: "unchanged" },
          { field: "supplierBankName", targetValue: "", sourceValue: "招商银行深圳分行科技园支行", resultValue: "招商银行深圳分行科技园支行", decision: "filled" },
          { field: "supplierBankAccount", targetValue: "", sourceValue: "755936182210801", resultValue: "755936182210801", decision: "filled" },
          { field: "currency", targetValue: "CNY", sourceValue: "CNY", resultValue: "CNY", decision: "unchanged" },
          { field: "defaultTaxRateBps", targetValue: "", sourceValue: "600", resultValue: "600", decision: "filled" },
        ],
      };
    case "list_contract_reviews":
      return [contractReview];
    case "get_contract_review":
      return contractReview;
    case "list_review_findings":
      return contractReview.findings;
    case "get_evidence_context":
      return {
        evidence: contractReview.evidence[0],
        page: {
          id: "page-5",
          extractionId: "ext-1",
          pageIndex: 4,
          text: "第七条 违约责任：乙方逾期交付的，每逾期一日按合同总额的 5% 支付违约金。累计不超过合同总额的 50%。甲方逾期付款的，按同等标准承担违约责任。",
          charCount: 66,
        },
        block: null,
      };
    case "list_asset_backups":
      return backups;
    case "list_pending_approvals":
      return [];
    case "brain_list_local_threads":
      return brainThreads;
    case "brain_list_local_turns":
      return brainTurns;
    case "get_brain_health":
      return {
        state: "ready",
        running: true,
        initialized: true,
        pendingRequests: 0,
        subscribers: 1,
        startedAt: T - 7200_000,
        lastMessageAt: T - 3600_000,
        lastErrorCode: null,
      };
    case "get_native_media_health":
      return { state: "ready", ffmpegAvailable: true, ffprobeAvailable: true, ffmpegSource: "bundled", ffprobeSource: "bundled" };
    case "get_host_status":
      return hostStatus;
    case "probe_codex":
      return codexStatus;
    case "get_asset_action_capabilities":
      return { assetId: (args as { assetId?: string })?.assetId ?? "", canOpen: true, canExport: true, reason: null };
    case "replay_events":
    case "replay_task_events":
    case "replay_asset_events":
    case "replay_case_events":
    case "replay_execution_brief_events":
    case "replay_requirement_brief_events":
    case "replay_business_workspace_events":
    case "replay_contract_review_events":
    case "replay_backup_events":
      return [];
    case "execute_ai_credential_command":
      return { receipt: receipt("aiCredentials.status"), status: aiStatus, connectionTest: null, replayed: false };
    case "execute_desktop_settings_command":
      return { receipt: receipt("settings.status"), snapshot: desktopSettings, cacheClear: null, replayed: false };
    case "execute_business_workspace_command":
      return { receipt: receipt("businessWorkspace.demo"), businessWorkspace: workspace, replayed: false };
    case "select_asset_source":
      return null;
    case "open_asset":
      return null;
    case "export_asset":
      return true;
    default:
      throw { code: "PREVIEW_READONLY", message: `预览模式不支持该操作（${cmd}）`, retryable: false };
  }
}

let callbackId = 100;
(window as unknown as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
  unregisterListener: () => undefined,
};
(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
  invoke: (cmd: string, args?: Record<string, unknown>) => handleInvoke(cmd, args),
  transformCallback: () => ++callbackId,
  unregisterCallback: () => undefined,
  convertFileSrc: (path: string) => path,
};

export {};
