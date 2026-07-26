import type { BrainTurnRecord } from "../generated/bsaigc/BrainTurnRecord";
import type { BrainTurnStatus } from "../generated/bsaigc/BrainTurnStatus";

const CONTRACT_MARKERS = [
  "CONTRACT_INPUT_JSON",
  "你是中国大陆影视制作与营销服务合同的高级商务审查 Agent",
  "只审查输入 JSON 中的合同",
  "deterministicFindings",
  "evidenceBlockIds",
  "missingEvidenceReason",
];

const INTERNAL_MARKERS = [
  ...CONTRACT_MARKERS,
  "SYSTEM_PROMPT",
  "INTERNAL_AGENT_INSTRUCTION",
  "output_schema",
  "response_format",
  "tool_payload",
  "toolPayload",
  "tool_call",
  "toolCall",
  "function_call",
  "functionCall",
  "<tool_call",
  "<tool_result",
  "<tool_use",
];

const TOOL_LINE = /^\s*(?:tool|function|mcp)[ _-]?(?:call|result|payload|output)\s*[:=]/i;
const TOOL_TAG_BLOCK = /<(?:tool|function|mcp)[_-]?(?:call|result|use|output)\b[^>]*>[\s\S]*?<\/(?:tool|function|mcp)[_-]?(?:call|result|use|output)>/gi;
const INTERNAL_FENCE = /```(?:json|tool|function|mcp)?\s*([\s\S]*?)```/gi;

export interface BrainTurnPresentation {
  readonly internal: boolean;
  readonly contractReview: boolean;
  readonly userText: string | null;
  readonly assistantText: string | null;
  readonly statusText: string | null;
  readonly errorText: string | null;
  readonly isStreaming: boolean;
}

export function friendlyBrainThreadTitle(title: string | null | undefined): string {
  const normalized = title?.trim() ?? "";
  if (!normalized) return "未命名对话";
  if (/\?{2,}/.test(normalized)) return "商务任务";
  if (containsContractMarker(normalized) || normalized.includes("合同审查")) {
    return "合同审查";
  }
  return normalized;
}

export function isInternalBrainText(value: string | null | undefined): boolean {
  if (!value) return false;
  return INTERNAL_MARKERS.some((marker) => value.includes(marker)) || TOOL_LINE.test(value);
}

export function isContractReviewBrainText(value: string | null | undefined): boolean {
  if (!value) return false;
  return containsContractMarker(value);
}

export function presentBrainTurn(
  turn: BrainTurnRecord,
  streamingDelta = "",
): BrainTurnPresentation {
  const contractReview =
    isContractReviewBrainText(turn.inputText) ||
    isContractReviewBrainText(turn.assistantText) ||
    isContractReviewBrainText(streamingDelta);
  const internal =
    contractReview ||
    isInternalBrainText(turn.inputText) ||
    isInternalBrainText(turn.assistantText) ||
    isInternalBrainText(streamingDelta) ||
    isStructuredPayload(turn.assistantText) ||
    isStructuredPayload(streamingDelta);
  const isStreaming = turn.status === "running" || streamingDelta.length > 0;

  if (internal) {
    return {
      internal: true,
      contractReview,
      userText: null,
      assistantText: null,
      statusText: internalStatus(turn.status, contractReview),
      errorText: friendlyBrainError(turn.error),
      isStreaming,
    };
  }

  const userText = sanitizeVisibleText(turn.inputText, "user");
  const assistantText = sanitizeVisibleText(
    `${turn.assistantText}${streamingDelta}`,
    "assistant",
  );
  const hiddenStructuredOutput =
    Boolean(turn.assistantText || streamingDelta) && !assistantText;

  return {
    internal: false,
    contractReview: false,
    userText,
    assistantText,
    statusText: hiddenStructuredOutput
      ? turn.status === "running"
        ? "正在处理"
        : "任务已完成，结果已同步到工作台"
      : null,
    errorText: friendlyBrainError(turn.error),
    isStreaming,
  };
}

export function presentOrphanBrainStreaming(
  streamingDelta: string,
  internalThread: boolean,
): { readonly text: string | null; readonly statusText: string } | null {
  if (!streamingDelta) return null;
  if (internalThread || isInternalBrainText(streamingDelta)) {
    return { text: null, statusText: "正在处理任务" };
  }
  const text = sanitizeVisibleText(streamingDelta, "assistant");
  return text
    ? { text, statusText: "正在回复" }
    : { text: null, statusText: "正在处理任务" };
}

function sanitizeVisibleText(
  value: string | null | undefined,
  role: "user" | "assistant",
): string | null {
  let normalized = value?.replace(/\u0000/g, "").trim() ?? "";
  if (!normalized || isInternalBrainText(normalized)) return null;

  normalized = normalized.replace(TOOL_TAG_BLOCK, "").trim();
  normalized = normalized
    .replace(INTERNAL_FENCE, (whole, body: string) =>
      isStructuredPayload(body) || isInternalBrainText(body) ? "" : whole,
    )
    .split(/\r?\n/)
    .filter((line) => !TOOL_LINE.test(line))
    .join("\n")
    .trim();

  if (!normalized) return null;
  if (role === "assistant" && isStructuredPayload(normalized)) return null;
  return normalized;
}

function isStructuredPayload(value: string): boolean {
  const normalized = value.trim();
  if (!(normalized.startsWith("{") || normalized.startsWith("["))) return false;
  try {
    JSON.parse(normalized);
    return true;
  } catch {
    return false;
  }
}

function containsContractMarker(value: string): boolean {
  return CONTRACT_MARKERS.some((marker) => value.includes(marker));
}

function internalStatus(status: BrainTurnStatus, contractReview: boolean): string {
  if (contractReview) {
    if (status === "running") return "正在审查合同";
    if (status === "completed") return "合同审查已完成，结果已同步到审查中心";
    if (status === "interrupted") return "合同审查已停止";
    return "合同审查未完成，可在审查中心重试";
  }
  if (status === "running") return "正在处理任务";
  if (status === "completed") return "任务已完成，结果已同步到工作台";
  if (status === "interrupted") return "任务已停止";
  return "任务未完成，可稍后重试";
}

function friendlyBrainError(value: string | null | undefined): string | null {
  const normalized = value?.trim() ?? "";
  if (!normalized) return null;
  if (
    isInternalBrainText(normalized) ||
    isStructuredPayload(normalized) ||
    /(?:stack trace|backtrace|panic|stderr|stdout|payload)/i.test(normalized)
  ) {
    return "任务执行未完成，请稍后重试。";
  }
  return normalized.length > 240
    ? "任务执行未完成，请稍后重试。"
    : normalized;
}
