import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { BusinessCustomerReceivableSummary } from "../generated/bsaigc/BusinessCustomerReceivableSummary";
import { BusinessReceivablesDrawer } from "./BusinessReceivablesDrawer";

const CUSTOMER: BusinessCustomerReceivableSummary = {
  customerId: "customer-1",
  customerKey: "customer-1",
  customerName: "华邦",
  customerLegalName: "华邦有限公司",
  customerTaxId: "",
  customerContact: "",
  customerPhone: "",
  customerEmail: "",
  customerStatus: "active",
  customerRevision: 1,
  workspaceCount: 3,
  activeWorkspaceCount: 2,
  contractCents: 1_200_000,
  requestedCents: 900_000,
  receivedCents: 700_000,
  outstandingCents: 500_000,
  workspaceIds: ["workspace-1"],
  updatedAt: 100,
};

describe("BusinessReceivablesDrawer", () => {
  it("renders the compact overview and customer receivable columns", () => {
    const html = renderToStaticMarkup(
      <BusinessReceivablesDrawer
        open
        customers={[CUSTOMER]}
        loading={false}
        error={null}
        query=""
        selectedWorkspaceId="workspace-1"
        onClose={vi.fn()}
        onQueryChange={vi.fn()}
        onRetry={vi.fn()}
        onSelectCustomer={vi.fn()}
      />,
    );

    expect(html).toContain("经营概览");
    expect(html).toContain("总合同额");
    expect(html).toContain("已请款");
    expect(html).toContain("已到账");
    expect(html).toContain("待回款");
    expect(html).toContain("华邦");
    expect(html).toContain("项目");
  });

  it("does not render when closed", () => {
    const html = renderToStaticMarkup(
      <BusinessReceivablesDrawer
        open={false}
        customers={[]}
        loading={false}
        error={null}
        query=""
        selectedWorkspaceId={null}
        onClose={vi.fn()}
        onQueryChange={vi.fn()}
        onRetry={vi.fn()}
        onSelectCustomer={vi.fn()}
      />,
    );

    expect(html).toBe("");
  });
});
