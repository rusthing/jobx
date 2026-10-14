import axios, { type AxiosError, type AxiosRequestConfig } from 'axios';
import { message } from 'antd';
import type { Ro } from './types';

/** 请求配置：在 axios 基础上加一个「是否吞掉全局错误提示」的开关。 */
export interface RequestConfig extends AxiosRequestConfig {
  /** 为 true 时 http 层不弹全局错误提示，由页面自行处理；默认 false。 */
  silent?: boolean;
}

export const raw = axios.create({
  baseURL: import.meta.env.VITE_API_BASE || '/api',
  headers: { 'Content-Type': 'application/json' },
});

raw.interceptors.request.use((config) => {
  const userId = localStorage.getItem('jobx_user_id') || '1';
  config.headers['X-User-Id'] = userId;
  return config;
});

/**
 * 把后端响应体里的 `Ro.result` 判定转换成 axios 层面的失败。
 *
 * 后端统一响应体是 `Ro<T>`：只有 `result === 1` 算业务成功，其余（-1 参数错误 /
 * -2 警告 / -3 失败）都是失败，错误说明就在 `msg` 字段里。在这里统一判定并 reject，
 * 业务侧拿到的就是一个正常的 Promise 异常，不需要在每个页面重复写
 * `if (result !== 1) ...`，也不需要每处都去解 `msg`。
 */
function assertRoOk(res: { data: unknown }): void {
  const body = res.data as Ro<unknown> | undefined;
  if (body && typeof body === 'object' && typeof body.result === 'number' && body.result !== 1) {
    throw new Error(body.msg || '请求失败');
  }
}

/** 网络层 / HTTP 层错误统一转成可读的 Error，避免页面里到处 `error?.response?.data?.msg`。 */
function normalizeError(error: AxiosError): Error {
  if (error.code === 'ERR_NETWORK') {
    return new Error('网络异常，请确认 jobx-svr 已启动');
  }
  const data = error.response?.data as Ro<unknown> | undefined;
  if (data && typeof data === 'object' && typeof data.result === 'number' && data.result !== 1) {
    return new Error(data.msg || '请求失败');
  }
  return error instanceof Error ? error : new Error('请求失败');
}

raw.interceptors.response.use(
  (response) => {
    // 业务失败在这里统一 reject，错误提示仍然只由 request() 弹一次
    try {
      assertRoOk(response);
    } catch (e) {
      return Promise.reject(e);
    }
    return response;
  },
  (error: AxiosError) => Promise.reject(normalizeError(error)),
);

/**
 * 发请求并解包后端响应体。
 *
 * - 成功：返回 `Ro.extra`（没有 extra 字段时返回 null）。
 * - 失败：**全局弹一次错误提示**后把异常抛给调用方。
 *   页面方的 `catch` 只用来收尾（关 loading、关弹窗、重置表单），
 *   **不要再各自调用 `message.error()`**，否则同一个错误会弹两次。
 *   需要页面自己定制提示的请求，传 `silent: true`。
 */
export async function request<T>(config: RequestConfig): Promise<T | null> {
  try {
    const res = await raw.request<Ro<T>>(config);
    return res.data.extra ?? null;
  } catch (e) {
    if (config.silent !== true) {
      message.error(e instanceof Error ? e.message : '请求失败');
    }
    throw e;
  }
}
