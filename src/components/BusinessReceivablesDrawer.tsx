import { AlertCircle, ChevronRight, LoaderCircle, Search, X } from "lucide-react";
import type { BusinessCustomerReceivableSummary } from "../generated/bsaigc/BusinessCustomerReceivableSummary";
import {
  businessCustomerDisplayName,
  formatBusinessAmount,
  summarizeBusinessReceivables,
} from "./businessReceivables";

export interface BusinessReceivablesDrawerProps {
  open: boolean;
  customers: readonly BusinessCustomerReceivableSummary[];
  loading: boolean;
  error: string | null;
  query: string;
  selectedWorkspaceId: string | null;
  onClose: () => void;
  onQueryChange: (query: string) => void;
  onRetry: () => void;
  onSelectCustomer: (customer: BusinessCustomerReceivableSummary) => void;
}

const METRICS = [
  ["contractCents", "总合同额"],
  ["requestedCents", "已请款"],
  ["receivedCents", "已到账"],
  ["outstandingCents", "待回款"],
] as const;

export function BusinessReceivablesDrawer({
  open,
  customers,
  loading,
  error,
  query,
  selectedWorkspaceId,
  onClose,
  onQueryChange,
  onRetry,
  onSelectCustomer,
}: BusinessReceivablesDrawerProps) {
  if (!open) return null;

  const totals = summarizeBusinessReceivables(customers);

  return (
    <>
      <button
        type="button"
        className="business-workbench__receivables-backdrop"
        aria-label="关闭经营概览"
        onClick={onClose}
      />
      <aside
        className="business-workbench__receivables-drawer"
        aria-label="经营概览"
      >
        <header className="business-workbench__receivables-header">
          <div>
            <strong>经营概览</strong>
            <span>{customers.length} 个客户</span>
          </div>
          <button type="button" onClick={onClose} aria-label="关闭经营概览">
            <X size={16} />
          </button>
        </header>

        <section
          className="business-workbench__receivables-metrics"
          aria-label="应收汇总"
        >
          {METRICS.map(([key, label]) => (
            <article key={key}>
              <span>{label}</span>
              <strong>{formatBusinessAmount(totals[key])}</strong>
            </article>
          ))}
        </section>

        <label className="business-workbench__receivables-search">
          <Search size={14} />
          <input
            type="search"
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="搜索客户"
            aria-label="搜索客户"
            autoComplete="off"
          />
          {query && (
            <button
              type="button"
              onClick={() => onQueryChange("")}
              aria-label="清空客户搜索"
            >
              <X size={13} />
            </button>
          )}
        </label>

        <section className="business-workbench__receivables-list" aria-live="polite">
          <div className="business-workbench__receivables-columns" aria-hidden="true">
            <span>客户</span>
            <span>项目</span>
            <span>合同额</span>
            <span>到账</span>
            <span>待回款</span>
            <span />
          </div>

          {loading ? (
            <div className="business-workbench__receivables-state">
              <LoaderCircle className="is-spinning" size={17} />
              <span>正在汇总</span>
            </div>
          ) : error ? (
            <div className="business-workbench__receivables-state is-error">
              <AlertCircle size={17} />
              <span>{error}</span>
              <button type="button" onClick={onRetry}>重试</button>
            </div>
          ) : customers.length === 0 ? (
            <div className="business-workbench__receivables-state">
              <span>{query ? "没有匹配的客户" : "暂无应收数据"}</span>
            </div>
          ) : (
            customers.map((customer) => {
              const selected = customer.workspaceIds.includes(
                selectedWorkspaceId ?? "",
              );
              return (
                <button
                  type="button"
                  key={customer.customerKey}
                  className={`business-workbench__receivables-row ${selected ? "is-selected" : ""}`}
                  onClick={() => onSelectCustomer(customer)}
                >
                  <span className="business-workbench__receivables-customer">
                    <strong>{businessCustomerDisplayName(customer)}</strong>
                    <small>{customer.activeWorkspaceCount} 个进行中</small>
                  </span>
                  <ReceivableValue label="项目" value={`${customer.workspaceCount}`} />
                  <ReceivableValue label="合同额" value={formatBusinessAmount(customer.contractCents)} />
                  <ReceivableValue label="到账" value={formatBusinessAmount(customer.receivedCents)} />
                  <ReceivableValue label="待回款" value={formatBusinessAmount(customer.outstandingCents)} />
                  <ChevronRight size={15} />
                </button>
              );
            })
          )}
        </section>
      </aside>
    </>
  );
}

function ReceivableValue({ label, value }: { label: string; value: string }) {
  return (
    <span className="business-workbench__receivables-value">
      <small>{label}</small>
      <strong>{value}</strong>
    </span>
  );
}
