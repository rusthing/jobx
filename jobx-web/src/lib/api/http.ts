/**
 * HTTP 基座：JobX 的响应一律包在 `Ro<T>` 里（`result` 1 成功、-1 参数错、-2 警告、-3 失败），
 * 请求一律要求 `X-User-Id` 头（服务端 `get_current_user_id` 缺了直接 400）。
 * 这两件事在这里一次性处理掉，业务层只拿 `extra`。
 */

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


const USER_KEY = "jobx.operator-id";
const DEFAULT_OPERATOR = "1";

export function getOperatorId(): string {
  try {
    return localStorage.getItem(USER_KEY) ?? DEFAULT_OPERATOR;
  } catch {
    return DEFAULT_OPERATOR;
  }
}

export function setOperatorId(id: string) {
  try {
    localStorage.setItem(USER_KEY, id);
  } catch {
    /* 隐私模式下 localStorage 不可写，忽略即可 */
  }
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
        "X-User-Id": getOperatorId(),
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
    // 静态服务器 / 网关把请求兜底成了 HTML：通常是反代没配好，不是业务错误
    throw new ApiError(
      `服务返回了非 JSON 响应（HTTP ${response.status}），检查 ${API_BASE}/jobx/* 是否被反代到 jobx-svr`,
      response.status,
    );
  }

  if (!response.ok) {
    throw new ApiError(payload.msg || `服务返回 ${response.status}`, response.status, payload.result, payload.detail);
  }

  if (payload.result !== 1) {
    throw new ApiError(
      payload.detail || payload.msg || "请求失败",
      response.status,
      payload.result,
      payload.detail,
    );
  }

  return payload.extra as T;
}
