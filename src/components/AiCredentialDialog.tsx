import { useEffect, useRef, useState, type FormEvent } from "react";
import { KeyRound, LoaderCircle, RefreshCw, ShieldCheck, Trash2, X } from "lucide-react";
import type { AiCredentialStatus } from "../generated/bsaigc/AiCredentialStatus";
import "./AiCredentialDialog.css";

export interface AiCredentialDialogProps {
  open: boolean;
  status: AiCredentialStatus | null;
  busy: boolean;
  error: string | null;
  onClose: () => void;
  onRefresh: () => Promise<void>;
  onSave: (apiKey: string) => Promise<void>;
  onClear: () => Promise<void>;
}

export function AiCredentialDialog({
  open,
  status,
  busy,
  error,
  onClose,
  onRefresh,
  onSave,
  onClear,
}: AiCredentialDialogProps) {
  const [apiKey, setApiKey] = useState("");
  const [confirmingClear, setConfirmingClear] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    setApiKey("");
    setConfirmingClear(false);
    void onRefresh().catch(() => undefined);
    const timer = window.setTimeout(() => inputRef.current?.focus(), 40);
    return () => window.clearTimeout(timer);
  }, [open, onRefresh]);

  useEffect(() => {
    if (!open) return;
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [busy, onClose, open]);

  if (!open) return null;

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const nextKey = apiKey.trim();
    if (!nextKey || busy) return;
    await onSave(nextKey);
    setApiKey("");
    setConfirmingClear(false);
  };

  const handleClear = async () => {
    if (!status?.configured || busy) return;
    if (!confirmingClear) {
      setConfirmingClear(true);
      return;
    }
    await onClear();
    setApiKey("");
    setConfirmingClear(false);
  };

  return (
    <div
      className="ai-credential-dialog__backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) onClose();
      }}
    >
      <section
        className="ai-credential-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="ai-credential-title"
      >
        <header className="ai-credential-dialog__header">
          <div>
            <span className="ai-credential-dialog__icon"><KeyRound size={16} /></span>
            <div>
              <h2 id="ai-credential-title">配置 AI</h2>
              <p>用于合同审查和商务助手</p>
            </div>
          </div>
          <button type="button" onClick={onClose} disabled={busy} aria-label="关闭 AI 配置">
            <X size={16} />
          </button>
        </header>

        <div className="ai-credential-dialog__status">
          <span className={status?.configured ? "is-ready" : "is-empty"} />
          <div>
            <strong>{status?.configured ? "已配置" : "未配置"}</strong>
            <small>
              {status?.provider ?? "OpenAI API"}
              {status?.updatedAt ? ` · ${formatUpdatedAt(status.updatedAt)}` : ""}
            </small>
          </div>
          <button
            type="button"
            className="ai-credential-dialog__icon-button"
            onClick={() => void onRefresh().catch(() => undefined)}
            disabled={busy}
            aria-label="刷新 AI 配置状态"
            title="刷新状态"
          >
            {busy ? <LoaderCircle size={14} className="ai-credential-dialog__spin" /> : <RefreshCw size={14} />}
          </button>
        </div>

        <form onSubmit={(event) => void handleSubmit(event)}>
          <label htmlFor="bsaigc-api-key">API Key</label>
          <input
            ref={inputRef}
            id="bsaigc-api-key"
            type="password"
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
            placeholder={status?.configured ? "输入新 Key 以替换" : "粘贴 API Key"}
            autoComplete="new-password"
            spellCheck={false}
            disabled={busy}
          />
          <div className="ai-credential-dialog__security-note">
            <ShieldCheck size={14} />
            <span>仅保存在当前 Windows 账户的加密存储中，不进入项目资料或操作记录。</span>
          </div>

          {status?.appliesOnNextRuntimeStart && (
            <p className="ai-credential-dialog__notice">下次智能任务将使用新的凭据。</p>
          )}
          {error && <p className="ai-credential-dialog__error" role="alert">{error}</p>}

          <footer className="ai-credential-dialog__actions">
            <div>
              {status?.configured && (
                confirmingClear ? (
                  <span className="ai-credential-dialog__clear-confirm">
                    <button type="button" onClick={() => setConfirmingClear(false)} disabled={busy}>取消</button>
                    <button type="button" className="is-danger" onClick={() => void handleClear()} disabled={busy}>
                      确认清除
                    </button>
                  </span>
                ) : (
                  <button type="button" className="ai-credential-dialog__clear" onClick={() => void handleClear()} disabled={busy}>
                    <Trash2 size={14} /> 清除
                  </button>
                )
              )}
            </div>
            <button type="submit" className="ai-credential-dialog__save" disabled={busy || apiKey.trim().length === 0}>
              {busy && <LoaderCircle size={14} className="ai-credential-dialog__spin" />}
              {status?.configured ? "保存并替换" : "保存"}
            </button>
          </footer>
        </form>
      </section>
    </div>
  );
}

function formatUpdatedAt(value: number): string {
  const timestamp = value < 10_000_000_000 ? value * 1000 : value;
  return new Intl.DateTimeFormat("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp));
}