import { useState, type FormEvent } from "react";
import {
  Building2,
  Eye,
  EyeOff,
  LoaderCircle,
  LogIn,
  ShieldCheck,
} from "lucide-react";
import type { AuthCredentials } from "../generated/bsaigc/AuthCredentials";
import type { AuthStatus } from "../generated/bsaigc/AuthStatus";
import { AUTH_SYNC_LABELS } from "./authText";
import "./AuthGate.css";

export interface AuthGateProps {
  status: AuthStatus;
  busy: boolean;
  error: string | null;
  initialCredentials: AuthCredentials | null;
  onInitialize: (username: string, password: string, remember: boolean) => void;
  onLogin: (username: string, password: string, remember: boolean) => void;
  onForgetSaved: () => void;
}

/**
 * Full-screen login gate. Two modes:
 * - first run (no accounts anywhere): create the administrator;
 * - normal: sign in.
 */
export function AuthGate({
  status,
  busy,
  error,
  initialCredentials,
  onInitialize,
  onLogin,
  onForgetSaved,
}: AuthGateProps) {
  const [username, setUsername] = useState(initialCredentials?.username ?? "");
  const [password, setPassword] = useState(initialCredentials?.password ?? "");
  const [confirm, setConfirm] = useState("");
  const [remember, setRemember] = useState(Boolean(initialCredentials));
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const initializing = !status.initialized;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (busy) return;
    setLocalError(null);
    const name = username.trim();
    if (!name || !password) {
      setLocalError("用户名和密码都要填");
      return;
    }
    if (initializing) {
      if (password !== confirm) {
        setLocalError("两次输入的密码不一样");
        return;
      }
      onInitialize(name, password, remember);
    } else {
      onLogin(name, password, remember);
    }
  };

  const message = localError ?? error;
  const syncLine =
    status.registrySync === "degraded" && status.registryMessage
      ? status.registryMessage
      : AUTH_SYNC_LABELS[status.registrySync];

  return (
    <div className="auth-gate">
      <form className="auth-gate__card" onSubmit={submit}>
        <div className="auth-gate__brand">
          <span className="auth-gate__logo">
            <Building2 size={22} />
          </span>
          <strong>半山商务工作台</strong>
          <small>
            {initializing
              ? "第一次使用：先创建管理员账号，之后由管理员在设置里给同事开账号"
              : "请登录后使用"}
          </small>
        </div>

        <label className="auth-gate__field">
          <span>用户名</span>
          <input
            value={username}
            onChange={(event) => setUsername(event.currentTarget.value)}
            placeholder={initializing ? "例如：老板、王总" : "输入用户名"}
            autoFocus
            disabled={busy}
            autoComplete="username"
          />
        </label>
        <label className="auth-gate__field">
          <span>密码</span>
          <div className="auth-gate__password">
            <input
              type={showPassword ? "text" : "password"}
              value={password}
              onChange={(event) => setPassword(event.currentTarget.value)}
              placeholder={initializing ? "至少 6 位" : "输入密码"}
              disabled={busy}
              autoComplete={initializing ? "new-password" : "current-password"}
            />
            <button
              type="button"
              onClick={() => setShowPassword((current) => !current)}
              disabled={busy}
              aria-label={showPassword ? "隐藏密码" : "显示密码"}
              title={showPassword ? "隐藏密码" : "显示密码"}
            >
              {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
            </button>
          </div>
        </label>
        {initializing && (
          <label className="auth-gate__field">
            <span>再输一遍密码</span>
            <div className="auth-gate__password">
              <input
                type={showConfirm ? "text" : "password"}
                value={confirm}
                onChange={(event) => setConfirm(event.currentTarget.value)}
                placeholder="确认密码"
                disabled={busy}
                autoComplete="new-password"
              />
              <button
                type="button"
                onClick={() => setShowConfirm((current) => !current)}
                disabled={busy}
                aria-label={showConfirm ? "隐藏确认密码" : "显示确认密码"}
                title={showConfirm ? "隐藏密码" : "显示密码"}
              >
                {showConfirm ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
          </label>
        )}

        <label className="auth-gate__remember">
          <input
            type="checkbox"
            checked={remember}
            onChange={(event) => {
              const checked = event.currentTarget.checked;
              setRemember(checked);
              if (!checked && initialCredentials) onForgetSaved();
            }}
            disabled={busy}
          />
          <span>记住账号密码</span>
        </label>

        {message && (
          <div className="auth-gate__error" role="alert">
            {message}
          </div>
        )}

        <button type="submit" className="auth-gate__submit" disabled={busy}>
          {busy ? (
            <LoaderCircle size={15} className="auth-gate__spin" />
          ) : initializing ? (
            <ShieldCheck size={15} />
          ) : (
            <LogIn size={15} />
          )}
          {busy ? "请稍候…" : initializing ? "创建管理员并进入" : "登录"}
        </button>

        <small className="auth-gate__sync">{syncLine}</small>
      </form>
    </div>
  );
}
