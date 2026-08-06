import type { AuditedField } from "./audit";
import { validateFieldAudit } from "./audit";
import { assertValid, requiredText, type ValidationIssue } from "./validation";

export type CustomerProjectKind = "singleProject" | "annualFramework";

export interface CustomerProject {
  id: string;
  companyId: string;
  customerLegalName: AuditedField<string>;
  projectName: AuditedField<string>;
  kind: CustomerProjectKind;
  contactName?: AuditedField<string>;
  status: "active" | "completed" | "archived";
}

export function validateCustomerProject(project: CustomerProject): void {
  const issues: ValidationIssue[] = [];
  requiredText(project.id, "customerProject.id", issues);
  requiredText(project.companyId, "customerProject.companyId", issues);
  requiredText(project.customerLegalName.value, "customerProject.customerLegalName", issues);
  requiredText(project.projectName.value, "customerProject.projectName", issues);
  assertValid(issues);
  validateFieldAudit(project.customerLegalName.audit);
  validateFieldAudit(project.projectName.audit);
  if (project.contactName) validateFieldAudit(project.contactName.audit);
}
