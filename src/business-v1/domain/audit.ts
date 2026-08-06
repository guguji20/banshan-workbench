import {
  assertValid,
  nonNegativeInteger,
  requiredText,
  type ValidationIssue,
} from "./validation";

export type FieldSourceKind =
  | "userInput"
  | "sourceFile"
  | "template"
  | "historicalQuotation"
  | "serviceCatalog"
  | "sharedCase"
  | "calculation"
  | "migration";

export interface FieldSource {
  kind: FieldSourceKind;
  referenceId: string;
  label: string;
  page?: number;
  sheet?: string;
  cell?: string;
  capturedAt: string;
}

export interface FieldConfirmation {
  confirmedBy: string;
  confirmedAt: string;
  note?: string;
}

export interface FieldAudit {
  version: number;
  sources: readonly FieldSource[];
  confirmation?: FieldConfirmation;
}

export interface AuditedField<T> {
  value: T;
  audit: FieldAudit;
}

export interface VersionAuditEntry {
  version: number;
  action: "created" | "revised" | "submitted" | "confirmed" | "withdrawn";
  actorId: string;
  occurredAt: string;
  note?: string;
}

export function createAuditedField<T>(
  value: T,
  audit: FieldAudit,
): AuditedField<T> {
  validateFieldAudit(audit);
  return {
    value,
    audit: {
      ...audit,
      sources: audit.sources.map((source) => ({ ...source })),
      confirmation: audit.confirmation ? { ...audit.confirmation } : undefined,
    },
  };
}

export function validateFieldAudit(audit: FieldAudit): void {
  const issues: ValidationIssue[] = [];
  nonNegativeInteger(audit.version, "audit.version", issues);
  if (audit.version === 0) {
    issues.push({
      field: "audit.version",
      code: "positive_version",
      message: "must be at least 1",
    });
  }
  if (audit.sources.length === 0) {
    issues.push({
      field: "audit.sources",
      code: "source_required",
      message: "must contain at least one field source",
    });
  }
  audit.sources.forEach((source, index) => {
    requiredText(source.referenceId, `audit.sources[${index}].referenceId`, issues);
    requiredText(source.label, `audit.sources[${index}].label`, issues);
    requiredText(source.capturedAt, `audit.sources[${index}].capturedAt`, issues);
    if (source.page !== undefined && (!Number.isSafeInteger(source.page) || source.page < 1)) {
      issues.push({
        field: `audit.sources[${index}].page`,
        code: "positive_page",
        message: "must be a positive integer",
      });
    }
  });
  if (audit.confirmation) {
    requiredText(audit.confirmation.confirmedBy, "audit.confirmation.confirmedBy", issues);
    requiredText(audit.confirmation.confirmedAt, "audit.confirmation.confirmedAt", issues);
  }
  assertValid(issues);
}

export function validateVersionAudit(
  entries: readonly VersionAuditEntry[],
  currentVersion: number,
): void {
  const issues: ValidationIssue[] = [];
  if (entries.length === 0) {
    issues.push({ field: "versionAudit", code: "audit_required", message: "is required" });
  }
  let previousVersion = 0;
  entries.forEach((entry, index) => {
    requiredText(entry.actorId, `versionAudit[${index}].actorId`, issues);
    requiredText(entry.occurredAt, `versionAudit[${index}].occurredAt`, issues);
    if (!Number.isSafeInteger(entry.version) || entry.version < 1) {
      issues.push({
        field: `versionAudit[${index}].version`,
        code: "positive_version",
        message: "must be a positive integer",
      });
    }
    if (entry.version < previousVersion) {
      issues.push({
        field: `versionAudit[${index}].version`,
        code: "version_order",
        message: "must not move backwards",
      });
    }
    previousVersion = entry.version;
  });
  if (entries.length > 0 && entries[entries.length - 1].version !== currentVersion) {
    issues.push({
      field: "versionAudit",
      code: "current_version_missing",
      message: "must end at the current version",
    });
  }
  assertValid(issues);
}
