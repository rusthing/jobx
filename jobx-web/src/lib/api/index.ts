/**
 * 数据入口。上层永远只 import 这里的 `jobs` / `tasks`，不关心背后是真服务还是演示数据。
 *
 * 模式选择：
 * - 配了 `VITE_API_BASE`（例如 `/api` 走 Vite 反代）→ 默认连真实服务；
 * - 没配 → 默认演示数据（没有可连的地址，硬着头皮请求只会白屏）。
 * 用户可在顶栏手动切换，选择存在 localStorage，优先级最高。
 */

import { API_BASE } from "./http";
import { liveJobs, liveTasks } from "./live";
import { mockJobs, mockTasks } from "./mock";
import type { Job, JobInput, JobQuery, Page, Task, TaskQuery } from "./types";

export type DataMode = "mock" | "live";

const MODE_KEY = "jobx.data-mode";
const MODE_EVENT = "jobx:data-mode";

const configuredBase = (import.meta.env.VITE_API_BASE ?? "").trim();

export const DEFAULT_MODE: DataMode = configuredBase ? "live" : "mock";

export function getDataMode(): DataMode {
  try {
    const stored = localStorage.getItem(MODE_KEY);
    if (stored === "mock" || stored === "live") return stored;
  } catch {
    /* ignore */
  }
  return DEFAULT_MODE;
}

export function setDataMode(mode: DataMode) {
  try {
    localStorage.setItem(MODE_KEY, mode);
  } catch {
    /* ignore */
  }
  window.dispatchEvent(new CustomEvent<DataMode>(MODE_EVENT, { detail: mode }));
}

export function subscribeDataMode(listener: (mode: DataMode) => void): () => void {
  const handler = (event: Event) => listener((event as CustomEvent<DataMode>).detail);
  window.addEventListener(MODE_EVENT, handler);
  return () => window.removeEventListener(MODE_EVENT, handler);
}

export function endpointLabel(): string {
  if (getDataMode() === "mock") return "内存演示数据";
  return configuredBase || API_BASE;
}

const isMock = () => getDataMode() === "mock";

export const jobs = {
  page: (query: JobQuery = {}): Promise<Page<Job>> =>
    isMock() ? mockJobs.page(query) : liveJobs.page(query),
  list: (query: JobQuery = {}): Promise<Job[]> =>
    isMock() ? mockJobs.list(query) : liveJobs.list(query),
  get: (id: string): Promise<Job> => (isMock() ? mockJobs.get(id) : liveJobs.get(id)),
  create: (input: JobInput): Promise<Job> =>
    isMock() ? mockJobs.create(input) : liveJobs.create(input),
  update: (input: JobInput): Promise<Job> =>
    isMock() ? mockJobs.update(input) : liveJobs.update(input),
  remove: (id: string): Promise<null> => (isMock() ? mockJobs.remove(id) : liveJobs.remove(id)),
  trigger: (id: string): Promise<Task> =>
    isMock() ? mockJobs.trigger(id) : liveJobs.trigger(id),
};

export const tasks = {
  page: (query: TaskQuery = {}): Promise<Page<Task>> =>
    isMock() ? mockTasks.page(query) : liveTasks.page(query),
  list: (query: TaskQuery = {}): Promise<Task[]> =>
    isMock() ? mockTasks.list(query) : liveTasks.list(query),
  get: (id: string): Promise<Task> => (isMock() ? mockTasks.get(id) : liveTasks.get(id)),
  remove: (id: string): Promise<null> => (isMock() ? mockTasks.remove(id) : liveTasks.remove(id)),
};

export { ApiError } from "./http";
export * from "./types";
