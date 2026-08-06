export interface ValidationIssue {
  field: string;
  code: string;
  message: string;
}

export class DomainValidationError extends Error {
  readonly issues: readonly ValidationIssue[];

  constructor(issues: readonly ValidationIssue[]) {
    super(issues.map((issue) => `${issue.field}: ${issue.message}`).join("; "));
    this.name = "DomainValidationError";
    this.issues = [...issues];
  }
}

export function assertValid(issues: readonly ValidationIssue[]): void {
  if (issues.length > 0) {
    throw new DomainValidationError(issues);
  }
}

export function requiredText(
  value: string,
  field: string,
  issues: ValidationIssue[],
): void {
  if (value.trim().length === 0) {
    issues.push({ field, code: "required", message: "is required" });
  }
}

export function nonNegativeInteger(
  value: number,
  field: string,
  issues: ValidationIssue[],
): void {
  if (!Number.isSafeInteger(value) || value < 0) {
    issues.push({
      field,
      code: "non_negative_safe_integer",
      message: "must be a non-negative safe integer",
    });
  }
}

export function positiveInteger(
  value: number,
  field: string,
  issues: ValidationIssue[],
): void {
  if (!Number.isSafeInteger(value) || value <= 0) {
    issues.push({
      field,
      code: "positive_safe_integer",
      message: "must be a positive safe integer",
    });
  }
}

export function assertSafeArithmetic(
  value: number,
  field: string,
): number {
  if (!Number.isSafeInteger(value)) {
    throw new DomainValidationError([
      {
        field,
        code: "unsafe_arithmetic",
        message: "exceeds deterministic integer range",
      },
    ]);
  }
  return value;
}
