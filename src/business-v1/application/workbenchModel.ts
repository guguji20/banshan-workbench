import type { BrainThreadRecord } from "../../generated/bsaigc/BrainThreadRecord";
import type { BrainTurnRecord } from "../../generated/bsaigc/BrainTurnRecord";
import type { ProjectRecord } from "../../generated/bsaigc/ProjectRecord";
import type { TaskRecord } from "../../generated/bsaigc/TaskRecord";
import { stripBusinessTurnContext, taskKindLabel, type BusinessTaskKind } from "./taskRouting";
import { extractWebResearchSources, type WebResearchSource } from "./webResearchView";

export interface WorkbenchProject {
  id: string;
  name: string;
  customerName: string;
  stage: string;
  updatedAt: number;
}

export interface WorkbenchConversation {
  id: string;
  projectId: string | null;
  title: string;
  status: string;
  updatedAt: number;
}

export interface WorkbenchMessage {
  id: string;
  role: "user" | "assistant" | "system";
  text: string;
  status: "pending" | "complete" | "failed";
  createdAt: number;
  sources?: WebResearchSource[];
}

export interface WorkbenchTask {
  id: string;
  kind: BusinessTaskKind;
  title: string;
  status: TaskRecord["status"];
  progress: number;
  error: string | null;
}

const TASK_KIND_PREFIX = "business.v1.";

export function toWorkbenchProject(project: ProjectRecord): WorkbenchProject {
  return {
    id: project.id,
    name: project.name,
    customerName: project.clientName,
    stage: project.stage,
    updatedAt: project.updatedAt,
  };
}

export function toWorkbenchConversation(thread: BrainThreadRecord): WorkbenchConversation {
  return {
    id: thread.id,
    projectId: thread.projectId,
    title: thread.title?.trim() || "新商务任务",
    status: thread.status,
    updatedAt: thread.updatedAt,
  };
}

export function toWorkbenchMessages(turns: readonly BrainTurnRecord[]): WorkbenchMessage[] {
  return turns.flatMap((turn) => {
    const status = turn.status === "failed" ? "failed" : turn.status === "completed" ? "complete" : "pending";
    const messages: WorkbenchMessage[] = [
      {
        id: `${turn.id}:user`,
        role: "user",
        text: stripBusinessTurnContext(turn.inputText),
        status: "complete",
        createdAt: turn.createdAt,
      },
    ];
    if (turn.assistantText || turn.error) {
      const sources = turn.error ? [] : extractWebResearchSources(turn.assistantText, turn.updatedAt);
      messages.push({
        id: `${turn.id}:assistant`,
        role: turn.error ? "system" : "assistant",
        text: turn.assistantText || turn.error || "任务执行失败",
        status,
        createdAt: turn.updatedAt,
        ...(sources.length ? { sources } : {}),
      });
    }
    return messages;
  });
}

export function parseBusinessTaskKind(kind: string): BusinessTaskKind {
  const candidate = kind.startsWith(TASK_KIND_PREFIX) ? kind.slice(TASK_KIND_PREFIX.length) : kind;
  switch (candidate) {
    case "quotation":
    case "acceptance":
    case "contractReview":
    case "settlement":
    case "archive":
    case "knowledgeSearch":
      return candidate;
    default:
      return "general";
  }
}

export function toWorkbenchTask(task: TaskRecord): WorkbenchTask {
  const kind = parseBusinessTaskKind(task.kind);
  return {
    id: task.id,
    kind,
    title: taskKindLabel(kind),
    status: task.status,
    progress: Math.max(0, Math.min(100, task.progress)),
    error: task.lastError,
  };
}
