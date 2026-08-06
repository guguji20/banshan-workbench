export const WEB_RESEARCH_VERIFICATION_LABEL = "外部未确认" as const;
export const WEB_RESEARCH_DATA_POLICY = "仅供参考，不覆盖正式业务数据" as const;

export interface WebResearchSource {
  id: string;
  url: string;
  title: string;
  domain: string;
  accessedAt: number;
  accessedDate: string;
  verificationLabel: typeof WEB_RESEARCH_VERIFICATION_LABEL;
}

interface SourceCandidate {
  url: string;
  title?: string;
  position: number;
}

const MARKDOWN_LINK_PATTERN = /\[([^\]\n]{1,160})\]\((https?:\/\/[^\s<>"']+)\)/giu;
const HTTP_URL_PATTERN = /https?:\/\/[^\s<>"'`，。；：！？、]+/giu;
const TRAILING_PUNCTUATION_PATTERN = /[.,;:!?，。；：！？、]+$/u;

export function extractWebResearchSources(
  assistantText: string,
  accessedAt = Date.now(),
): WebResearchSource[] {
  if (!assistantText.trim()) return [];

  const candidates: SourceCandidate[] = [];
  for (const match of assistantText.matchAll(MARKDOWN_LINK_PATTERN)) {
    candidates.push({
      url: match[2],
      title: cleanMarkdownTitle(match[1]),
      position: match.index ?? 0,
    });
  }
  for (const match of assistantText.matchAll(HTTP_URL_PATTERN)) {
    candidates.push({ url: match[0], position: match.index ?? 0 });
  }

  const sources = new Map<string, WebResearchSource>();
  for (const candidate of candidates.sort((left, right) => left.position - right.position)) {
    const normalizedUrl = normalizePublicHttpUrl(candidate.url);
    if (!normalizedUrl || sources.has(normalizedUrl)) continue;

    const parsed = new URL(normalizedUrl);
    sources.set(normalizedUrl, {
      id: `web-source:${normalizedUrl}`,
      url: normalizedUrl,
      title: candidate.title || deriveSourceTitle(parsed),
      domain: parsed.hostname.toLowerCase(),
      accessedAt,
      accessedDate: formatAccessDate(accessedAt),
      verificationLabel: WEB_RESEARCH_VERIFICATION_LABEL,
    });
  }
  return [...sources.values()];
}

export function isPublicHttpUrl(value: string): boolean {
  return normalizePublicHttpUrl(value) !== null;
}

function normalizePublicHttpUrl(candidate: string): string | null {
  const cleaned = trimUrlSuffix(candidate.trim());
  try {
    const parsed = new URL(cleaned);
    if ((parsed.protocol !== "http:" && parsed.protocol !== "https:") || !isPublicHostname(parsed.hostname)) {
      return null;
    }
    parsed.hash = "";
    return parsed.href;
  } catch {
    return null;
  }
}

function trimUrlSuffix(value: string): string {
  let result = value.replace(TRAILING_PUNCTUATION_PATTERN, "");
  while (result.endsWith(")") && countCharacter(result, ")") > countCharacter(result, "(")) {
    result = result.slice(0, -1);
  }
  while (/[\]}]$/u.test(result)) result = result.slice(0, -1);
  return result.replace(TRAILING_PUNCTUATION_PATTERN, "");
}

function countCharacter(value: string, character: string): number {
  return [...value].filter((item) => item === character).length;
}

function isPublicHostname(hostname: string): boolean {
  const normalized = hostname.toLowerCase().replace(/^\[|\]$/gu, "").replace(/\.$/u, "");
  if (!normalized || normalized === "localhost" || normalized.endsWith(".localhost") || normalized.endsWith(".local")) {
    return false;
  }
  if (normalized.includes(":")) {
    return !(
      normalized === "::" ||
      normalized === "::1" ||
      normalized.startsWith("fc") ||
      normalized.startsWith("fd") ||
      /^fe[89ab]/u.test(normalized)
    );
  }

  const octets = normalized.split(".").map(Number);
  if (octets.length !== 4 || octets.some((octet) => !Number.isInteger(octet) || octet < 0 || octet > 255)) {
    return true;
  }
  const [first, second] = octets;
  return !(
    first === 0 ||
    first === 10 ||
    first === 127 ||
    first >= 224 ||
    (first === 100 && second >= 64 && second <= 127) ||
    (first === 169 && second === 254) ||
    (first === 172 && second >= 16 && second <= 31) ||
    (first === 192 && second === 168) ||
    (first === 198 && (second === 18 || second === 19))
  );
}

function cleanMarkdownTitle(value: string): string | undefined {
  const cleaned = value.replace(/[*_~`]/gu, "").replace(/\s+/gu, " ").trim();
  return cleaned && !/^https?:\/\//iu.test(cleaned) ? truncate(cleaned, 80) : undefined;
}

function deriveSourceTitle(url: URL): string {
  const pathSegments = url.pathname.split("/").filter(Boolean);
  const lastSegment = pathSegments[pathSegments.length - 1];
  if (!lastSegment) return url.hostname.toLowerCase();
  try {
    const decoded = decodeURIComponent(lastSegment)
      .replace(/\.[a-z0-9]{1,8}$/iu, "")
      .replace(/[-_]+/gu, " ")
      .replace(/\s+/gu, " ")
      .trim();
    return truncate(decoded || url.hostname.toLowerCase(), 80);
  } catch {
    return truncate(lastSegment, 80);
  }
}

function truncate(value: string, limit: number): string {
  return value.length > limit ? `${value.slice(0, limit - 1)}…` : value;
}

function formatAccessDate(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "long",
    day: "numeric",
    timeZone: "Asia/Shanghai",
  }).format(value);
}
