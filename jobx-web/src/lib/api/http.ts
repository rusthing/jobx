/**
 * HTTP 基座：JobX 的响应一律包在 `Ro<T>` 里（`result` 1 成功、-1 参数错、-2 警告、-3 失败），
 * 请求一律要求 `X-User-Id` 头（服务端 `get_current_user_id` 缺了直接 400）。
 * 这两件事在这里一次性处理掉，业务层只拿 `extra`。
 */

import { authHeaders, handleUnauthorized, readSession } from "../auth-core";

export interface Ro<T> {
  result: 1 | -1 | -2 | -3;
  msg: string;
  timestamp: string;
  extra?: T;
  detail?: string;
  code?: string;
}

/**
 * 接口基址。
 * - 开发期默认 `/api`，由 Vite 反代到 jobx-svr 并剥掉前缀；
 * - 生产环境由 `VITE_API_BASE` 给出，通常是同源的 nginx 前缀（如 `/jobx-svr`），
 *   也可以是跨域的绝对地址（此时后端要开 `[web.cors]`）。
 * 末尾斜杠统一去掉，避免拼成 `/jobx-svr//jobx/job/page`。
 */
const RAW_API_BASE = (import.meta.env.VITE_API_BASE ?? "").trim();

export const API_BASE = (RAW_API_BASE || "/api").replace(/\/+$/, "");

/**
 * 当前操作人固定来自统一登录态：required 模式是令牌里的用户，
 * disabled 模式是系统账号 0；界面不再允许手工改 X-User-Id。
 */
export function getOperatorId(): string {
  const session = readSession();
  return session ? String(session.userId) : "0";
}

export class ApiError extends Error {
  readonly status: number;
  readonly result?: number;
  readonly detail?: string;

  constructor(message: string, status: number, result?: number, detail?: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.result = result;
    this.detail = detail;
  }

  /** 网络层完全没通（区别于服务端返回的业务错误） */
  get isNetworkError() {
    return this.status === 0;
  }

  /** 会话失效或无权限：值不值得让用户重试，要区别对待 */
  get isAuthError() {
    return this.status === 401 || this.status === 403;
  }

  /** `result = -2` 是服务端的「业务拒绝」，不是系统故障 */
  get isWarning() {
    return this.result === -2;
  }
}

const RESULT_PREFIX: Record<number, string> = {
  [-1]: "参数不合法：",
  [-2]: "",
  [-3]: "服务处理失败：",
};

/** 把 HTTP 状态码 / 业务码翻成值班同学能直接行动的提示 */
export function describeApiError(
  status: number,
  result: number | undefined,
  msg: string | undefined,
  detail: string | undefined,
): string {
  if (status === 401) return "登录状态已失效，请重新登录后再试";
  if (status === 403) return "当前操作人没有权限执行该操作";
  if (status === 404) return msg || detail || "请求的资源不存在";
  if (status === 429) return "请求过于频繁，请稍后重试";
  if (status >= 500) return msg || detail || `服务端异常（HTTP ${status}）`;

  const text = msg || detail || "请求失败";
  return `${RESULT_PREFIX[result ?? 0] ?? ""}${text}`;
}

type QueryValue = string | number | boolean | null | undefined;

export interface RequestOptions {
  method?: "GET" | "POST" | "PUT" | "DELETE";
  query?: Record<string, QueryValue>;
  body?: unknown;
  signal?: AbortSignal;
  timeoutMs?: number;
}

function buildUrl(path: string, query?: Record<string, QueryValue>) {
  const url = `${API_BASE}${path}`;
  if (!query) return url;
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null || value === "") continue;
    params.set(key, String(value));
  }
  const qs = params.toString();
  return qs ? `${url}?${qs}` : url;
}

export async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const { method = "GET", query, body, signal, timeoutMs = 15_000 } = options;

  const timeout = AbortSignal.timeout(timeoutMs);
  const composed = signal ? AbortSignal.any([signal, timeout]) : timeout;

  let response: Response;
  try {
    response = await fetch(buildUrl(path, query), {
      method,
      signal: composed,
      headers: {
        Accept: "application/json",
        ...authHeaders(),
        ...(body === undefined ? {} : { "Content-Type": "application/json" }),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") {
      throw error;
    }
    const reason = timeout.aborted ? "请求超时" : "无法连接到 JobX 服务";
    throw new ApiError(reason, 0);
  }

  if (response.status === 401) handleUnauthorized();

  // 204 或网关吞掉响应体时没有 JSON 可拆，此时只能按 HTTP 状态判断
  const text = await response.text();
  let payload: Ro<T> | null = null;
  if (text) {
    try {
      payload = JSON.parse(text) as Ro<T>;
    } catch {
      payload = null;
    }
  }

  if (!payload) {
    if (response.ok && response.status === 204) return undefined as T;
    // 静态服务器 / 网关把请求兜底成了 HTML：通常是反代没配好，不是业务错误
    throw new ApiError(
      `服务返回了非 JSON 响应（HTTP ${response.status}），检查 ${API_BASE}/jobx/* 是否被反代到 jobx-svr`,
      response.status,
    );
  }

  if (!response.ok) {
    throw new ApiError(
      describeApiError(response.status, payload.result, payload.msg, payload.detail),
      response.status,
      payload.result,
      payload.detail,
    );
  }

  if (payload.result !== 1) {
    throw new ApiError(
      describeApiError(response.status, payload.result, payload.msg, payload.detail),
      response.status,
      payload.result,
      payload.detail,
    );
  }

  return payload.extra as T;
}
