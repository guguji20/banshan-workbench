import type { AuditedField } from "./audit";
import { validateFieldAudit } from "./audit";
import { assertValid, requiredText, type ValidationIssue } from "./validation";

export interface Company {
  id: string;
  legalName: AuditedField<string>;
  displayName: AuditedField<string>;
  taxId?: AuditedField<string>;
  status: "active" | "archived";
}

export function validateCompany(company: Company): void {
  const issues: ValidationIssue[] = [];
  requiredText(company.id, "company.id", issues);
  requiredText(company.legalName.value, "company.legalName", issues);
  requiredText(company.displayName.value, "company.displayName", issues);
  assertValid(issues);
  validateFieldAudit(company.legalName.audit);
  validateFieldAudit(company.displayName.audit);
  if (company.taxId) validateFieldAudit(company.taxId.audit);
}
