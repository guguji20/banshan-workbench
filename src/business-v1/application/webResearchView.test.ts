import { describe, expect, it } from "vitest";
import {
  extractWebResearchSources,
  isPublicHttpUrl,
  WEB_RESEARCH_DATA_POLICY,
  WEB_RESEARCH_VERIFICATION_LABEL,
} from "./webResearchView";

describe("web research source projection", () => {
  it("extracts, normalizes, labels, and deduplicates public links", () => {
    const accessedAt = Date.UTC(2026, 6, 28, 16, 0, 0);
    const sources = extractWebResearchSources(
      "参考 [官方采购规则](https://Example.com/rules/latest?type=public)，也可打开 https://example.com/rules/latest?type=public。补充：https://docs.example.org/guides/contract-review.html",
      accessedAt,
    );

    expect(sources).toHaveLength(2);
    expect(sources[0]).toMatchObject({
      title: "官方采购规则",
      domain: "example.com",
      url: "https://example.com/rules/latest?type=public",
      accessedAt,
      accessedDate: "2026年7月29日",
      verificationLabel: WEB_RESEARCH_VERIFICATION_LABEL,
    });
    expect(sources[1]).toMatchObject({
      title: "contract review",
      domain: "docs.example.org",
    });
    expect(WEB_RESEARCH_DATA_POLICY).toBe("仅供参考，不覆盖正式业务数据");
  });

  it("rejects local and private network URLs", () => {
    const sources = extractWebResearchSources(
      "https://localhost/admin http://127.0.0.1:1420 https://10.0.0.2/a https://192.168.1.5/a https://public.example.com/a",
      1,
    );

    expect(sources.map((source) => source.domain)).toEqual(["public.example.com"]);
    expect(isPublicHttpUrl("javascript:alert(1)")).toBe(false);
    expect(isPublicHttpUrl("https://[::1]/private")).toBe(false);
    expect(isPublicHttpUrl("https://www.example.com/public")).toBe(true);
  });
});
