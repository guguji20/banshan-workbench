import type { AppUserRole } from "../generated/bsaigc/AppUserRole";
import type { AuthRegistrySync } from "../generated/bsaigc/AuthRegistrySync";

/** 登录相关文案：全部大白话，供登录页与设置页共用。 */

export const AUTH_ROLE_LABELS: Record<AppUserRole, string> = {
  admin: "管理员",
  member: "员工",
};

export const AUTH_SYNC_LABELS: Record<AuthRegistrySync, string> = {
  localOnly: "账号只存在本机（未配置云备份）",
  synced: "账号已与云端同步，全公司通用",
  degraded: "云端同步待处理",
};

const AUTH_ERROR_TEXT: Record<string, string> = {
  AUTH_INVALID_CREDENTIALS: "用户名或密码不对",
  AUTH_ALREADY_INITIALIZED: "公司已经有账号了，请直接登录",
  AUTH_NOT_INITIALIZED: "还没有任何账号，请先创建管理员",
  AUTH_NOT_LOGGED_IN: "请先登录",
  AUTH_FORBIDDEN: "只有管理员能管理用户",
  AUTH_USER_EXISTS: "这个用户名已经有人用了",
  AUTH_USER_NOT_FOUND: "这个账号不存在（可能已被删除）",
  AUTH_LAST_ADMIN: "最后一个管理员不能删除",
  AUTH_SELF_DELETE: "不能删除自己正在用的账号",
  AUTH_WEAK_PASSWORD: "密码至少要 6 位",
  AUTH_INVALID_USERNAME: "用户名要 2-32 个字，不能带特殊符号",
  AUTH_USER_DISABLED: "这个账号已被停用，请联系管理员",
  AUTH_TOO_MANY_USERS: "账号数量已达上限",
};

interface HostErrorLike {
  code?: string;
  message?: string;
}

export function localizeAuthError(error: unknown): string {
  const candidate = error as HostErrorLike | null;
  const code = candidate?.code;
  if (code && AUTH_ERROR_TEXT[code]) return AUTH_ERROR_TEXT[code];
  if (candidate?.message) return candidate.message;
  if (error instanceof Error) return error.message;
  return String(error);
}
