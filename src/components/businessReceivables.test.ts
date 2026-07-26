import { describe, expect, it } from "vitest";
import type { BusinessCustomerReceivableSummary } from "../generated/bsaigc/BusinessCustomerReceivableSummary";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import {
  businessCustomerDisplayName,
  formatBusinessAmount,
  latestCustomerWorkspace,
  summarizeBusinessReceivables,
} from "./businessReceivables";

function customer(
  overrides: Partial<BusinessCustomerReceivableSummary> = {},
): BusinessCustomerReceivableSummary {
  return {
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
    workspaceCount: 2,
    activeWorkspaceCount: 1,
    contractCents: 100_000,
    requestedCents: 80_000,
    receivedCents: 60_000,
    outstandingCents: 40_000,
    workspaceIds: ["workspace-old", "workspace-new"],
    updatedAt: 20,
    ...overrides,
  };
}

function workspace(
  id: string,
  projectId: string,
  updatedAt: number,
): BusinessWorkspaceRecord {
  return {
    id,
    projectId,
    customerId: "customer-1",
    customer: {
      id: "customer-1",
      displayName: "华邦",
      legalName: "华邦有限公司",
      taxId: "",
      billingAddress: "",
      primaryContactName: "",
      primaryPhone: "",
      primaryEmail: "",
      notes: "",
      status: "active",
      revision: 1,
      createdAt: updatedAt - 1,
      updatedAt,
      archivedAt: null,
      archivedBy: null,
    },
    requirementBriefId: null,
    requirementBriefRevision: null,
    prefillSourceWorkspaceId: null,
    profile: {} as BusinessWorkspaceRecord["profile"],
    documents: [],
    payments: [],
    quoteConfirmations: [],
    receipts: [],
    milestones: [],
    deliverySubmissions: [],
    invoices: [],
    archiveSnapshots: [],
    archiveIntegrityStatus: "notCaptured",
    status: "active",
    archivedAt: null,
    archivedBy: null,
    lifecycleStage: "draft",
    financialSummary: {} as BusinessWorkspaceRecord["financialSummary"],
    currentDocuments: {} as BusinessWorkspaceRecord["currentDocuments"],
    revision: 1,
    createdAt: updatedAt - 1,
    updatedAt,
  };
}

describe("business receivables presentation", () => {
  it("sums the four management metrics", () => {
    expect(
      summarizeBusinessReceivables([
        customer(),
        customer({
          customerId: "customer-2",
          customerKey: "customer-2",
          contractCents: 50_000,
          requestedCents: 20_000,
          receivedCents: 10_000,
          outstandingCents: 40_000,
        }),
      ]),
    ).toEqual({
      contractCents: 150_000,
      requestedCents: 100_000,
      receivedCents: 70_000,
      outstandingCents: 80_000,
    });
  });

  it("selects the latest workspace belonging to the customer", () => {
    const latest = latestCustomerWorkspace(customer(), [
      workspace("workspace-new", "project-new", 200),
      workspace("unrelated", "project-unrelated", 300),
      workspace("workspace-old", "project-old", 100),
    ]);

    expect(latest?.id).toBe("workspace-new");
    expect(latest?.projectId).toBe("project-new");
  });

  it("formats compact amounts and falls back to the legal name", () => {
    expect(formatBusinessAmount(12_340_000)).toBe("¥12.34万");
    expect(
      businessCustomerDisplayName(
        customer({ customerName: "", customerLegalName: "华邦有限公司" }),
      ),
    ).toBe("华邦有限公司");
  });
});

