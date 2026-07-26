import type { BusinessCustomerReceivableSummary } from "../generated/bsaigc/BusinessCustomerReceivableSummary";
import type { BusinessWorkspaceRecord } from "../generated/bsaigc/BusinessWorkspaceRecord";

export interface BusinessReceivableTotals {
  contractCents: number;
  requestedCents: number;
  receivedCents: number;
  outstandingCents: number;
}

const compactAmountFormatter = new Intl.NumberFormat("zh-CN", {
  minimumFractionDigits: 0,
  maximumFractionDigits: 2,
});

export function summarizeBusinessReceivables(
  customers: readonly BusinessCustomerReceivableSummary[],
): BusinessReceivableTotals {
  return customers.reduce<BusinessReceivableTotals>(
    (totals, customer) => ({
      contractCents: totals.contractCents + customer.contractCents,
      requestedCents: totals.requestedCents + customer.requestedCents,
      receivedCents: totals.receivedCents + customer.receivedCents,
      outstandingCents: totals.outstandingCents + customer.outstandingCents,
    }),
    {
      contractCents: 0,
      requestedCents: 0,
      receivedCents: 0,
      outstandingCents: 0,
    },
  );
}

export function latestCustomerWorkspace(
  customer: BusinessCustomerReceivableSummary,
  workspaces: readonly BusinessWorkspaceRecord[],
): BusinessWorkspaceRecord | null {
  const workspaceIds = new Set(customer.workspaceIds);
  return (
    workspaces
      .filter((workspace) => workspaceIds.has(workspace.id))
      .sort(
        (left, right) =>
          right.updatedAt - left.updatedAt ||
          right.createdAt - left.createdAt ||
          right.revision - left.revision,
      )[0] ?? null
  );
}

export function formatBusinessAmount(cents: number): string {
  const yuan = Number.isFinite(cents) ? cents / 100 : 0;
  const absolute = Math.abs(yuan);

  if (absolute >= 100_000_000) {
    return `¥${compactAmountFormatter.format(yuan / 100_000_000)}亿`;
  }
  if (absolute >= 10_000) {
    return `¥${compactAmountFormatter.format(yuan / 10_000)}万`;
  }
  return `¥${compactAmountFormatter.format(yuan)}`;
}

export function businessCustomerDisplayName(
  customer: BusinessCustomerReceivableSummary,
): string {
  return (
    customer.customerName.trim() ||
    customer.customerLegalName.trim() ||
    "未命名客户"
  );
}
