export type BusinessTaskKind =
  | "quotation"
  | "acceptance"
  | "contractReview"
  | "settlement"
  | "archive"
  | "knowledgeSearch"
  | "general";

export type KnowledgeScope = "local" | "shared" | "web";

export interface BusinessTaskAttachment {
  id: string;
  name: string;
  kind: "file" | "folder" | "image";
}

export interface BusinessTaskDraft {
  id: string;
  kind: BusinessTaskKind;
  projectId: string | null;
  prompt: string;
  attachmentIds: string[];
  knowledgeScope: KnowledgeScope;
  requiresConfirmation: boolean;
  createdAt: number;
}

const TASK_CONTEXT_PREFIX = "<!--BSAIGC_BUSINESS_TASK:";
const TASK_CONTEXT_SUFFIX = "-->";

export interface RouteBusinessTaskInput {
  prompt: string;
  projectId?: string | null;
  attachments?: readonly BusinessTaskAttachment[];
  knowledgeScope?: KnowledgeScope;
  now?: number;
  id?: string;
}

export const WEB_RESEARCH_SAFETY_CONTRACT = {
  informationScope: "public-information-only",
  queryRestrictions: [
    "Do not send attachment or contract source text as a search query.",
    "Do not send bank account details or customer secrets as a search query.",
    "Do not send local filesystem paths as a search query.",
  ],
  sourceRequirements: ["url", "access-date", "external-unconfirmed"],
  formalDataPolicy: "External results must not automatically overwrite formal business fields.",
} as const;

const INTENT_RULES: ReadonlyArray<{
  kind: Exclude<BusinessTaskKind, "general">;
  terms: readonly string[];
}> = [
  { kind: "knowledgeSearch", terms: ["检索", "查找", "搜索", "search"] },
  { kind: "acceptance", terms: ["验收", "签收", "成果确认", "acceptance"] },
  { kind: "quotation", terms: ["报价", "询价", "价格", "优惠", "quote"] },
  { kind: "contractReview", terms: ["合同", "条款", "法务", "contract"] },
  { kind: "settlement", terms: ["结算", "请款", "回款", "发票", "settlement"] },
  { kind: "archive", terms: ["归档", "打包", "archive"] },
];

const CONFIRMATION_TASKS = new Set<BusinessTaskKind>([
  "quotation",
  "acceptance",
  "contractReview",
  "settlement",
  "archive",
]);

export function detectBusinessTaskKind(prompt: string): BusinessTaskKind {
  const normalized = prompt.trim().toLocaleLowerCase();
  if (!normalized) return "general";

  for (const rule of INTENT_RULES) {
    if (rule.terms.some((term) => normalized.includes(term))) return rule.kind;
  }
  return "general";
}

export function routeBusinessTask(input: RouteBusinessTaskInput): BusinessTaskDraft {
  const prompt = input.prompt.trim();
  if (!prompt) throw new Error("任务内容不能为空");

  const kind = detectBusinessTaskKind(prompt);
  const now = input.now ?? Date.now();
  const generatedId = input.id ?? `business-task-${now}`;

  return {
    id: generatedId,
    kind,
    projectId: input.projectId ?? null,
    prompt,
    attachmentIds: (input.attachments ?? []).map((attachment) => attachment.id),
    knowledgeScope: input.knowledgeScope ?? "local",
    requiresConfirmation: CONFIRMATION_TASKS.has(kind),
    createdAt: now,
  };
}

export function taskKindLabel(kind: BusinessTaskKind): string {
  const labels: Record<BusinessTaskKind, string> = {
    quotation: "报价",
    acceptance: "验收",
    contractReview: "合同审查",
    settlement: "结算与收款",
    archive: "归档",
    knowledgeSearch: "资料检索",
    general: "商务任务",
  };
  return labels[kind];
}

export function businessTaskThreadTitle(task: BusinessTaskDraft): string {
  return `【${taskKindLabel(task.kind)}】${task.prompt.slice(0, 32)}`;
}

export function businessTaskKindFromThreadTitle(title: string | null): BusinessTaskKind | null {
  if (!title) return null;
  const match = /^【([^】]+)】/.exec(title.trim());
  if (!match) return null;
  const entry = (Object.entries({
    quotation: "报价",
    acceptance: "验收",
    contractReview: "合同审查",
    settlement: "结算与收款",
    archive: "归档",
    knowledgeSearch: "资料检索",
    general: "商务任务",
  }) as Array<[BusinessTaskKind, string]>).find(([, label]) => label === match[1]);
  return entry?.[0] ?? null;
}

export function canReuseThreadForTask(
  thread: { projectId: string | null; title: string | null } | null,
  task: BusinessTaskDraft,
): boolean {
  if (!thread || thread.projectId !== task.projectId) return false;
  const threadKind = businessTaskKindFromThreadTitle(thread.title);
  return task.kind === "general" ? threadKind === "general" : threadKind === task.kind;
}

export function buildBusinessTurnInput(task: BusinessTaskDraft): string {
  const context = JSON.stringify({
    taskId: task.id,
    kind: task.kind,
    knowledgeScope: task.knowledgeScope,
    requiresConfirmation: task.requiresConfirmation,
    ...(task.knowledgeScope === "web" ? { webResearchSafety: WEB_RESEARCH_SAFETY_CONTRACT } : {}),
  });
  return `${TASK_CONTEXT_PREFIX}${context}${TASK_CONTEXT_SUFFIX}\n${task.prompt}`;
}

export function stripBusinessTurnContext(input: string): string {
  if (!input.startsWith(TASK_CONTEXT_PREFIX)) return input;
  const end = input.indexOf(TASK_CONTEXT_SUFFIX);
  return end < 0 ? input : input.slice(end + TASK_CONTEXT_SUFFIX.length).replace(/^\r?\n/, "");
}
