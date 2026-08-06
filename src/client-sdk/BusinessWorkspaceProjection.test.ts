import { describe, expect, it } from "vitest";
import type { BusinessProfile } from "../generated/bsaigc/BusinessProfile";
import type { BusinessSettlementBatchRecord } from "../generated/bsaigc/BusinessSettlementBatchRecord";
import type { BusinessWorkspaceDomainEvent } from "../generated/bsaigc/BusinessWorkspaceDomainEvent";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";
import { BusinessWorkspaceProjection } from "./BusinessWorkspaceProjection";

const PROFILE: BusinessProfile = {
  projectTitle: "Launch campaign",
  projectCode: "LAUNCH-001",
  customerName: "Client",
  customerLegalName: "Client Limited",
  customerTaxId: "customer-tax-id",
  customerAddress: "Customer address",
  customerContact: "Customer contact",
  customerPhone: "10000",
  customerEmail: "client@example.com",
  supplierLegalName: "Studio Limited",
  supplierTaxId: "supplier-tax-id",
  supplierAddress: "Supplier address",
  supplierContact: "Supplier contact",
  supplierPhone: "10001",
  supplierBankName: "Business Bank",
  supplierBankAccount: "1000000001",
  currency: "CNY",
  defaultTaxRateBps: 600,
  taxMode: "taxExclusive",
  projectDiscountCents: 0,
  quotationTotals: null,
  serviceStartAt: null,
  serviceEndAt: null,
  deliverySummary: "Launch film",
  paymentTerms: "Net 30",
  acceptanceTerms: "Written approval",
  notes: "",
  lineItems: [],
};

function businessWorkspace(
  id: string,
  revision: number,
  updatedAt = revision,
): BusinessWorkspaceRecord {
  return {
    id,
    projectId: `project-${id}`,
    customerId: `customer-${id}`,
    customer: {
      id: `customer-${id}`,
      displayName: PROFILE.customerName,
      legalName: PROFILE.customerLegalName,
      taxId: PROFILE.customerTaxId,
      billingAddress: PROFILE.customerAddress,
      primaryContactName: PROFILE.customerContact,
      primaryPhone: PROFILE.customerPhone,
      primaryEmail: PROFILE.customerEmail,
      notes: "",
      status: "active",
      revision: 1,
      createdAt: 1,
      updatedAt,
      archivedAt: null,
      archivedBy: null,
    },
    requirementBriefId: null,
    requirementBriefRevision: null,
    prefillSourceWorkspaceId: null,
    profile: {
      ...PROFILE,
      projectTitle: `${PROFILE.projectTitle} r${revision}`,
    },
    documents: [],
    payments: [],
    quoteConfirmations: [],
    receipts: [],
    milestones: [],
    settlementBatches: [],
    acceptanceBatches: [],
    templateVersions: [],
    deliverySubmissions: [],
    invoices: [],
    archiveSnapshots: [],
    archiveIntegrityStatus: "notCaptured",
    status: revision > 1 ? "archived" : "active",
    archivedAt: revision > 1 ? revision : null,
    archivedBy: revision > 1 ? "operator-1" : null,
    lifecycleStage: revision > 1 ? "archived" : "draft",
    financialSummary: {
      quotedCents: 0,
      contractCents: 0,
      scheduledCents: 0,
      requestedCents: 0,
      receivedCents: 0,
      outstandingCents: 0,
    },
    currentDocuments: {
      quoteDocumentId: null,
      contractDocumentId: null,
      paymentRequestDocumentId: null,
      acceptanceDocumentId: null,
    },
    revision,
    createdAt: 1,
    updatedAt,
  };
}

function event(
  sequence: number,
  revision = sequence,
  id = "workspace-1",
): BusinessWorkspaceDomainEvent {
  return {
    sequence,
    eventId: `event-${sequence}-${id}`,
    eventType:
      revision === 1
        ? "businessWorkspace.created"
        : "businessWorkspace.profileUpdated",
    aggregateId: id,
    revision,
    occurredAt: sequence,
    traceId: `trace-${sequence}`,
    actorId: "actor-1",
    commandId: `business-command-${sequence}`,
    reason: "测试商务操作",
    businessWorkspace: businessWorkspace(id, revision),
  };
}

describe("BusinessWorkspaceProjection", () => {
  it("hydrates and upserts only higher revisions with stable sorting", () => {
    const projection = new BusinessWorkspaceProjection();

    expect(
      projection.hydrate([
        businessWorkspace("workspace-c", 1, 10),
        businessWorkspace("workspace-b", 1, 20),
        businessWorkspace("workspace-a", 1, 20),
      ]),
    ).toBe(true);
    expect(projection.upsert(businessWorkspace("workspace-b", 2, 30))).toBe(
      true,
    );
    expect(projection.upsert(businessWorkspace("workspace-b", 1, 99))).toBe(
      false,
    );

    const snapshot = projection.snapshot();
    expect(snapshot.businessWorkspaces.map(({ id }) => id)).toEqual([
      "workspace-b",
      "workspace-a",
      "workspace-c",
    ]);
    expect(snapshot.businessWorkspaces[0]).toMatchObject({
      revision: 2,
      updatedAt: 30,
    });
  });

  it("holds future events until their sequence gap closes", () => {
    const projection = new BusinessWorkspaceProjection();

    expect(projection.applyEvent(event(3))).toBe(true);
    expect(projection.applyEvent(event(2))).toBe(true);
    expect(projection.snapshot()).toEqual({
      businessWorkspaces: [],
      events: [],
      lastSequence: 0,
    });

    expect(projection.applyEvent(event(1))).toBe(true);
    expect(projection.snapshot().lastSequence).toBe(3);
    expect(projection.snapshot().businessWorkspaces[0]?.revision).toBe(3);
    expect(projection.snapshot().events.map(({ sequence }) => sequence)).toEqual([
      1, 2, 3,
    ]);
  });

  it("deduplicates applied and pending sequences", () => {
    const projection = new BusinessWorkspaceProjection();
    const future = event(3, 3, "future-workspace");

    expect(projection.applyEvent(event(1))).toBe(true);
    expect(projection.applyEvent(event(1, 99, "duplicate-workspace"))).toBe(
      false,
    );
    expect(projection.applyEvent(future)).toBe(true);
    expect(projection.applyEvent({ ...future, eventId: "duplicate" })).toBe(
      false,
    );

    projection.applyEvent(event(2));
    expect(projection.snapshot().lastSequence).toBe(3);
    expect(projection.snapshot().events).toHaveLength(3);
    expect(
      projection
        .snapshot()
        .businessWorkspaces.some(({ id }) => id === "duplicate-workspace"),
    ).toBe(false);
  });

  it("projects the complete 1.6 customer, delivery, invoice, and archive aggregate", () => {
    const projection = new BusinessWorkspaceProjection();
    const artifact = {
      role: "deliverable",
      assetId: "asset-master-v1",
      sha256: "a".repeat(64),
      sizeBytes: 4096,
      originalName: "master-v1.mp4",
    };
    const closureWorkspace: BusinessWorkspaceRecord = {
      ...businessWorkspace("workspace-closure", 2, 20),
      customerId: "customer-stable-1",
      customer: {
        ...businessWorkspace("workspace-closure", 2, 20).customer,
        id: "customer-stable-1",
        revision: 4,
      },
      milestones: [
        {
          id: "milestone-1",
          sequenceNumber: 1,
          title: "Final delivery",
          description: "Master package",
          dueAt: 30,
          acceptanceCriteria: "Written signoff",
          required: true,
          status: "accepted",
          deliverables: [
            {
              id: "deliverable-1",
              milestoneId: "milestone-1",
              name: "Master film",
              required: true,
              versions: [
                {
                  id: "version-1",
                  deliverableId: "deliverable-1",
                  milestoneId: "milestone-1",
                  name: "Master film",
                  required: true,
                  versionNumber: 1,
                  artifact,
                  status: "accepted",
                  notes: "Final",
                  createdBy: "operator-1",
                  createdAt: 10,
                },
              ],
            },
          ],
          revision: 2,
          createdAt: 5,
          updatedAt: 15,
        },
      ],
      deliverySubmissions: [
        {
          id: "submission-1",
          milestoneId: "milestone-1",
          submissionNumber: 1,
          versionIds: ["version-1"],
          recipient: "client@example.com",
          channel: "email",
          note: "Final delivery",
          sentAt: 12,
          sentBy: "operator-1",
          status: "accepted",
          signoffs: [
            {
              id: "signoff-1",
              submissionId: "submission-1",
              acceptedVersionIds: ["version-1"],
              rejectedVersionIds: [],
              customerRepresentative: "Customer contact",
              evidence: null,
              note: "Accepted",
              occurredAt: 14,
              recordedBy: "operator-1",
              recordedAt: 14,
            },
          ],
        },
      ],
      invoices: [
        {
          id: "invoice-1",
          paymentId: null,
          kind: "issued",
          status: "issued",
          invoiceCode: "INV-CODE",
          invoiceNumber: "INV-001",
          issuerTaxId: PROFILE.supplierTaxId,
          buyerTaxId: PROFILE.customerTaxId,
          currency: "CNY",
          amountCents: 100_000,
          taxCents: 6_000,
          issuedAt: 16,
          originalInvoiceId: null,
          reversalReason: "",
          artifacts: [{ ...artifact, role: "invoice", assetId: "asset-invoice" }],
          recordedBy: "operator-1",
          createdAt: 16,
        },
      ],
      archiveSnapshots: [
        {
          id: "archive-1",
          capturedWorkspaceRevision: 2,
          capturedCustomerRevision: 4,
          manifestSha256: "b".repeat(64),
          manifestAssetId: null,
          packageAssetId: null,
          entries: [
            {
              logicalPath: "delivery/master-v1.mp4",
              role: "deliverable",
              sourceEntityType: "deliverableVersion",
              sourceEntityId: "version-1",
              artifact,
            },
          ],
          generatedBy: "operator-1",
          generatedAt: 20,
        },
      ],
      archiveIntegrityStatus: "ready",
    };

    expect(
      projection.applyEvent({
        ...event(1, 2, "workspace-closure"),
        eventType: "businessWorkspace.archiveSnapshotPrepared",
        businessWorkspace: closureWorkspace,
      }),
    ).toBe(true);
    expect(projection.snapshot().businessWorkspaces[0]).toMatchObject({
      customerId: "customer-stable-1",
      customer: { revision: 4 },
      milestones: [
        { deliverables: [{ versions: [{ id: "version-1", status: "accepted" }] }] },
      ],
      deliverySubmissions: [{ id: "submission-1", status: "accepted" }],
      invoices: [{ id: "invoice-1", kind: "issued" }],
      archiveSnapshots: [{ id: "archive-1", manifestAssetId: null }],
      archiveIntegrityStatus: "ready",
      revision: 2,
    });
  });

  it("projects settlement batch upserts and voids from workspace events", () => {
    const projection = new BusinessWorkspaceProjection();
    const settlementBatch: BusinessSettlementBatchRecord = {
      id: "settlement-q1",
      workspaceId: "workspace-settlement",
      contractNumber: "ANNUAL-2026-001",
      settlementPeriod: "2026-Q1",
      cadence: "quarterly",
      status: "confirmed",
      lines: [
        {
          deliverableId: "deliverable-q1",
          milestoneId: "milestone-q1",
          deliverableName: "Quarter one delivery",
          contractQuantityMillis: 12_000,
          cumulativeExecutedMillis: 3_000,
          currentExecutedMillis: 3_000,
          cumulativeAcceptedMillis: 3_000,
          currentAcceptedMillis: 3_000,
          cumulativeSettledMillis: 3_000,
          currentSettlementMillis: 3_000,
          remainingQuantityMillis: 9_000,
          unit: "item",
          notes: "Accepted Q1 output",
        },
      ],
      notes: "Quarterly settlement",
      revision: 1,
      createdAt: 10,
      updatedAt: 10,
      voidedAt: null,
      voidedBy: null,
      voidReason: "",
    };
    const upsertedWorkspace = {
      ...businessWorkspace("workspace-settlement", 2, 10),
      settlementBatches: [settlementBatch],
    };

    expect(
      projection.applyEvent({
        ...event(1, 2, "workspace-settlement"),
        eventType: "businessWorkspace.settlementBatchUpserted",
        businessWorkspace: upsertedWorkspace,
      }),
    ).toBe(true);
    expect(projection.snapshot().businessWorkspaces[0]?.settlementBatches).toEqual([
      settlementBatch,
    ]);

    const voidedBatch: BusinessSettlementBatchRecord = {
      ...settlementBatch,
      status: "voided",
      revision: 2,
      updatedAt: 20,
      voidedAt: 20,
      voidedBy: "operator-1",
      voidReason: "Incorrect accepted quantity",
    };
    expect(
      projection.applyEvent({
        ...event(2, 3, "workspace-settlement"),
        eventType: "businessWorkspace.settlementBatchVoided",
        businessWorkspace: {
          ...upsertedWorkspace,
          settlementBatches: [voidedBatch],
          revision: 3,
          updatedAt: 20,
        },
      }),
    ).toBe(true);
    expect(projection.snapshot()).toMatchObject({
      lastSequence: 2,
      businessWorkspaces: [
        {
          id: "workspace-settlement",
          revision: 3,
          settlementBatches: [
            {
              id: "settlement-q1",
              status: "voided",
              revision: 2,
              voidedBy: "operator-1",
              voidReason: "Incorrect accepted quantity",
            },
          ],
        },
      ],
    });
  });

  it("bounds recent events and resets records, pending events, and cursor", () => {
    const projection = new BusinessWorkspaceProjection(3);

    projection.applyEvent(event(1));
    projection.applyEvent(event(2));
    projection.applyEvent(event(3));
    projection.applyEvent(event(4));
    projection.applyEvent(event(6));
    expect(projection.snapshot().events.map(({ sequence }) => sequence)).toEqual([
      2, 3, 4,
    ]);

    expect(projection.reset()).toBe(true);
    expect(projection.snapshot()).toEqual({
      businessWorkspaces: [],
      events: [],
      lastSequence: 0,
    });
    expect(projection.reset()).toBe(false);

    projection.applyEvent(event(1, 10, "after-reset"));
    expect(projection.snapshot().businessWorkspaces[0]).toMatchObject({
      id: "after-reset",
      revision: 10,
    });
  });
});
