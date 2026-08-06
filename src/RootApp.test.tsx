import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import RootApp from "./RootApp";

describe("RootApp", () => {
  it("renders the real Business Workbench 1.0 entry chain", () => {
    const html = renderToStaticMarkup(<RootApp />);

    expect(html).toContain('data-product-shell="business-v1"');
    expect(html).toContain("华邦互娱商务系统");
    expect(html).toContain("选择项目后开始任务");
    expect(html).not.toContain("legacy");
  });
});
