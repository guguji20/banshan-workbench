import type { BsaigcClientSnapshot } from "../../client-sdk";

export type HistoryCategory =
  | "tasks"
  | "cases"
  | "requirementBriefs"
  | "executionBriefs"
  | "assets"
  | "archivedThreads";

export type HistoryStatusTone = "neutral" | "success" | "warning" | "danger";

export interface HistoryRecordView {
  id: string;
  category: HistoryCategory;
  projectId: string | null;
  projectLabel: string;
  title: string;
  detail: string;
  statusLabel: string;
  statusTone: HistoryStatusTone;
  updatedAt: number;
  assetId?: string;
  restoreThreadId?: string;
}

export type HistoryDataSnapshot = Pick<
  BsaigcClientSnapshot,
  | "projects"
  | "tasks"
  | "cases"
  | "requirementBriefs"
  | "executionBriefs"
  | "assets"
  | "brainThreads"
>;

export const HISTORY_CATEGORY_LABELS: Record<HistoryCategory, string> = {
  tasks: "历史任务",
  cases: "历史案例",
  requirementBriefs: "需求简报",
  executionBriefs: "执行简报",
  assets: "通用资产",
  archivedThreads: "归档会话",
};

const TASK_STATUS_LABELS: Record<string, string> = {
  queued: "排队中",
  running: "执行中",
  succeeded: "已完成",
  failed: "失败",
  canceled: "已取消",
  awaitingApproval: "待确认",
};

const TASK_KIND_LABELS: Record<string, string> = {
  quotation: "报价",
  acceptance: "验收",
  settlement: "结算",
  archive: "归档",
  contractReview: "合同审查",
  "contract-review": "合同审查",
};

const CASE_TYPE_LABELS: Record<string, string> = {
  brand: "品牌",
  property: "地产",
  interview: "访谈",
  lifestyle: "生活方式",
  product: "产品",
  event: "活动",
  documentary: "纪录",
  narrative: "叙事",
  other: "其他",
};

const ASSET_KIND_LABELS: Record<string, string> = {
  image: "图片",
  video: "视频",
  audio: "音频",
  document: "文档",
  other: "其他",
};

export function buildHistoryRecords(snapshot: HistoryDataSnapshot): HistoryRecordView[] {
  const projectNames = new Map(
    snapshot.projects.map((project) => [project.id, project.name] as const),
  );
  const projectLabel = (projectId: string | null) =>
    projectId ? projectNames.get(projectId) ?? "历史项目" : "未归属项目";

  return [
    ...snapshot.tasks.map<HistoryRecordView>((task) => ({
      id: task.id,
      category: "tasks",
      projectId: task.projectId,
      projectLabel: projectLabel(task.projectId),
      title: (TASK_KIND_LABELS[task.kind] ?? task.kind) + "任务",
      detail: task.lastError
        ? task.lastError
        : "进度 " + Math.max(0, Math.min(100, task.progress)) + "% · 第 " + task.attempt + "/" + task.maxAttempts + " 次执行",
      statusLabel: TASK_STATUS_LABELS[task.status] ?? task.status,
      statusTone: taskStatusTone(task.status),
      updatedAt: task.updatedAt,
    })),
    ...snapshot.cases.map<HistoryRecordView>((caseRecord) => ({
      id: caseRecord.id,
      category: "cases",
      projectId: caseRecord.projectId,
      projectLabel: projectLabel(caseRecord.projectId),
      title: caseRecord.title,
      detail: [
        caseRecord.clientName,
        CASE_TYPE_LABELS[caseRecord.contentType] ?? caseRecord.contentType,
        ...caseRecord.tags.slice(0, 3),
      ].filter(Boolean).join(" · "),
      statusLabel: caseRecord.qualityTier === "premium"
        ? "精品"
        : caseRecord.qualityTier === "featured"
          ? "精选"
          : "参考",
      statusTone: caseRecord.qualityTier === "premium" ? "success" : "neutral",
      updatedAt: caseRecord.updatedAt,
      assetId: caseRecord.assetId,
    })),
    ...snapshot.requirementBriefs.map<HistoryRecordView>((brief) => ({
      id: brief.id,
      category: "requirementBriefs",
      projectId: brief.projectId,
      projectLabel: projectLabel(brief.projectId),
      title: brief.content.objective.trim() || "需求简报 · " + projectLabel(brief.projectId),
      detail: [
        brief.content.deliverables.length ? brief.content.deliverables.length + " 项交付物" : "未填写交付物",
        brief.content.deadlineAt ? "截止 " + formatHistoryDate(brief.content.deadlineAt) : "未填写截止时间",
      ].join(" · "),
      statusLabel: brief.status === "confirmed" ? "已确认" : brief.status === "review" ? "待审阅" : "访谈中",
      statusTone: brief.status === "confirmed" ? "success" : "warning",
      updatedAt: brief.updatedAt,
    })),
    ...snapshot.executionBriefs.map<HistoryRecordView>((brief) => ({
      id: brief.id,
      category: "executionBriefs",
      projectId: brief.projectId,
      projectLabel: projectLabel(brief.projectId),
      title: brief.content.clientGoal.trim() || "执行简报 · " + projectLabel(brief.projectId),
      detail: [
        brief.content.visualStyle.trim() || "未填写视觉风格",
        brief.content.requiredShots.length ? brief.content.requiredShots.length + " 个必拍镜头" : "未填写必拍镜头",
      ].join(" · "),
      statusLabel: brief.status === "ready" ? "已就绪" : "草稿",
      statusTone: brief.status === "ready" ? "success" : "neutral",
      updatedAt: brief.updatedAt,
    })),
    ...snapshot.assets.map<HistoryRecordView>((asset) => ({
      id: asset.id,
      category: "assets",
      projectId: asset.projectId,
      projectLabel: projectLabel(asset.projectId),
      title: asset.originalName,
      detail: (ASSET_KIND_LABELS[asset.kind] ?? asset.kind) + " · " + formatHistorySize(asset.sizeBytes) + (asset.previewAvailable ? " · 可预览" : ""),
      statusLabel: asset.status === "ready" ? "可用" : "失败",
      statusTone: asset.status === "ready" ? "success" : "danger",
      updatedAt: asset.updatedAt,
      assetId: asset.id,
    })),
    ...snapshot.brainThreads
      .filter((thread) => thread.status === "archived")
      .map<HistoryRecordView>((thread) => ({
        id: thread.id,
        category: "archivedThreads",
        projectId: thread.projectId,
        projectLabel: projectLabel(thread.projectId),
        title: thread.title?.trim() || "未命名归档会话",
        detail: thread.model ? "模型 " + thread.model : "保留原会话记录，可恢复到当前项目对话",
        statusLabel: "已归档",
        statusTone: "neutral",
        updatedAt: thread.updatedAt,
        restoreThreadId: thread.id,
      })),
  ].sort((left, right) => right.updatedAt - left.updatedAt || left.id.localeCompare(right.id));
}

export function filterHistoryRecords(
  records: readonly HistoryRecordView[],
  category: HistoryCategory,
  query: string,
  projectId: string | null,
): HistoryRecordView[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  return records.filter((record) =>
    record.category === category
    && (!projectId || record.projectId === projectId)
    && (!normalizedQuery || (record.title + " " + record.detail + " " + record.projectLabel + " " + record.statusLabel)
      .toLocaleLowerCase()
      .includes(normalizedQuery)),
  );
}

export function historyCategoryCounts(
  records: readonly HistoryRecordView[],
  projectId: string | null,
): Record<HistoryCategory, number> {
  const counts = Object.fromEntries(
    Object.keys(HISTORY_CATEGORY_LABELS).map((category) => [category, 0]),
  ) as Record<HistoryCategory, number>;
  for (const record of records) {
    if (!projectId || record.projectId === projectId) counts[record.category] += 1;
  }
  return counts;
}

export function formatHistoryTimestamp(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(value);
}

function formatHistoryDate(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(value);
}

function formatHistorySize(bytes: number): string {
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return Math.round(bytes / 1024) + " KB";
  return (bytes / 1024 / 1024).toFixed(1) + " MB";
}

function taskStatusTone(status: string): HistoryStatusTone {
  if (status === "succeeded") return "success";
  if (status === "failed" || status === "canceled") return "danger";
  if (status === "running" || status === "awaitingApproval") return "warning";
  return "neutral";
}
