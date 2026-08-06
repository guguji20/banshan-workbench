import { describe, expect, it } from "vitest";
import {
  DomainValidationError,
  calculateQuotation,
  createAuditedField,
  money,
  transitionQuotation,
  validateCompany,
  validateCustomerProject,
  type AuditedField,
  type Company,
  type CustomerProject,
  type FieldAudit,
  type Quotation,
} from "./index";

const now = "2026-07-28T10:00:00.000Z";

function audit(referenceId = "user-brief", version = 1): FieldAudit {
  return {
    version,
    sources: [
      {
        kind: "userInput",
        referenceId,
        label: "Confirmed user brief",
        capturedAt: now,
      },
    ],
  };
}

function field<T>(value: T, referenceId?: string): AuditedField<T> {
  return createAuditedField(value, audit(referenceId));
}

function quotation(overrides: Partial<Quotation> = {}): Quotation {
  return {
    id: "quotation-1",
    companyId: "company-1",
    customerProjectId: "project-1",
    title: field("Baietan service quotation"),
    status: "draft",
    version: {
      number: 1,
      createdBy: "operator-1",
      createdAt: now,
    },
    items: [
      {
        id: "service-1",
        description: field("Production service", "service-catalog-1"),
        quantity: field(4, "user-quantity"),
        unitPrice: field(money(2_120_000), "approved-unit-price"),
      },
    ],
    discounts: [
      {
        id: "discount-1",
        label: field("Project discount", "approved-discount"),
        amount: field(money(490_000), "approved-discount"),
      },
    ],
    tax: {
      basisPoints: field(0, "tax-rule"),
      mode: "taxExclusive",
    },
    versionAudit: [
      {
        version: 1,
        action: "created",
        actorId: "operator-1",
        occurredAt: now,
      },
    ],
    ...overrides,
  };
}

describe("quotation deterministic calculation", () => {
  it("calculates 21200 x 4, applies 4900 discount, and returns 79900", () => {
    const source = quotation();
    const originalUnitPrice = source.items[0].unitPrice.value;

    const result = calculateQuotation(source);

    expect(result.items[0].lineTotal.cents).toBe(8_480_000);
    expect(result.subtotal.cents).toBe(8_480_000);
    expect(result.discountTotal.cents).toBe(490_000);
    expect(result.finalTotal.cents).toBe(7_990_000);
    expect(source.items[0].unitPrice.value).toBe(originalUnitPrice);
    expect(source.items[0].unitPrice.value.cents).toBe(2_120_000);
  });

  it("uses basis points and deterministic half-up cent rounding for tax", () => {
    const source = quotation({
      discounts: [],
      items: [
        {
          id: "item-tax",
          description: field("Taxed item"),
          quantity: field(1),
          unitPrice: field(money(10_001)),
        },
      ],
      tax: { basisPoints: field(600), mode: "taxExclusive" },
    });

    const result = calculateQuotation(source);

    expect(result.taxAmount.cents).toBe(600);
    expect(result.finalTotal.cents).toBe(10_601);
  });

  it("rejects fractional cents and discounts exceeding subtotal", () => {
    expect(() => money(10.5)).toThrow(DomainValidationError);
    expect(() =>
      calculateQuotation(
        quotation({
          discounts: [
            { id: "too-much", label: field("Invalid"), amount: field(money(8_480_001)) },
          ],
        }),
      ),
    ).toThrowError(/must not exceed subtotal/);
  });

  it("rejects duplicate lines and missing field provenance", () => {
    const duplicate = quotation();
    expect(() =>
      calculateQuotation({
        ...duplicate,
        items: [...duplicate.items, duplicate.items[0]],
      }),
    ).toThrowError(/must be unique/);

    expect(() =>
      createAuditedField("unknown", { version: 1, sources: [] }),
    ).toThrowError(/must contain at least one field source/);
  });
});

describe("quotation state machine and version audit", () => {
  it("requires review before confirmation and records every transition", () => {
    const draft = quotation();
    expect(() =>
      transitionQuotation(draft, "confirm", { actorId: "approver", occurredAt: now }),
    ).toThrowError(/Cannot confirm quotation from draft/);

    const review = transitionQuotation(draft, "submitForReview", {
      actorId: "operator-1",
      occurredAt: "2026-07-28T11:00:00.000Z",
    });
    const confirmed = transitionQuotation(review, "confirm", {
      actorId: "approver-1",
      occurredAt: "2026-07-28T12:00:00.000Z",
    });

    expect(review.status).toBe("readyForReview");
    expect(confirmed.status).toBe("confirmed");
    expect(confirmed.versionAudit.map((entry) => entry.action)).toEqual([
      "created",
      "submitted",
      "confirmed",
    ]);
    expect(draft.status).toBe("draft");
  });

  it("creates a new version on revision without mutating the confirmed version", () => {
    const review = transitionQuotation(quotation(), "submitForReview", {
      actorId: "operator-1",
      occurredAt: now,
    });
    const confirmed = transitionQuotation(review, "confirm", {
      actorId: "approver-1",
      occurredAt: now,
    });
    const revised = transitionQuotation(confirmed, "revise", {
      actorId: "operator-2",
      occurredAt: "2026-07-29T09:00:00.000Z",
      note: "Customer requested a new scope",
    });

    expect(revised.status).toBe("draft");
    expect(revised.version.number).toBe(2);
    expect(revised.version.parentVersion).toBe(1);
    expect(revised.items).toBe(confirmed.items);
    expect(revised.items[0].unitPrice.value.cents).toBe(2_120_000);
    expect(confirmed.version.number).toBe(1);
    expect(confirmed.status).toBe("confirmed");
  });
});

describe("company and customer project validation", () => {
  it("validates auditable company and single/annual customer projects", () => {
    const company: Company = {
      id: "company-1",
      legalName: field("Banshan Culture Co., Ltd."),
      displayName: field("Banshan"),
      status: "active",
    };
    const project: CustomerProject = {
      id: "project-1",
      companyId: company.id,
      customerLegalName: field("Customer Co., Ltd."),
      projectName: field("2026 annual content framework"),
      kind: "annualFramework",
      status: "active",
    };

    expect(() => validateCompany(company)).not.toThrow();
    expect(() => validateCustomerProject(project)).not.toThrow();
  });
});
