/** 真实后端接口（路径与 `#[router(crud)]` 生成的完全一致）。 */

import { request } from "./http";
import type {
  Group,
  GroupInput,
  GroupQuery,
  Job,
  JobInput,
  JobQuery,
  Page,
  Task,
  TaskQuery,
} from "./types";

type QueryRecord = Record<string, unknown> & {
  page?: number;
  size?: number;
  keyword?: string;
  orderBy?: string;
};

/** 后端的查询参数是 `_page` / `_size` / `_keyword` / `_orderBy`，其余按 DTO 字段名直传 */
function pageQuery(query: JobQuery | TaskQuery | GroupQuery) {
  const { page = 1, size = 20, keyword, orderBy, ...rest } = query as QueryRecord;
  return { ...rest, _page: page, _size: size, _keyword: keyword, _orderBy: orderBy };
}

/**
 * 时间范围用 Bean Searcher 的字段条件表达：`create_ms-op=ge&create_ms=<ms>`。
 * 前端的 `createdAfterMs` / `assignAfterMs` 是语义化命名，不能原样发给后端。
 */
function withTimeFilters(
  query: JobQuery | TaskQuery,
): { rest: Record<string, unknown>; page: number; size: number; orderBy?: string } {
  const { page = 1, size = 20, orderBy, createdAfterMs, assignAfterMs, ...rest } =
    query as QueryRecord & { createdAfterMs?: string; assignAfterMs?: string };
  const filters: Record<string, unknown> = { ...rest };
  if (createdAfterMs) {
    filters["create_ms-op"] = "ge";
    filters.create_ms = createdAfterMs;
  }
  if (assignAfterMs) {
    filters["assign_ms-op"] = "ge";
    filters.assign_ms = assignAfterMs;
  }
  return { rest: filters, page, size, orderBy };
}

function jobQuery(query: JobQuery) {
  const { rest, page, size, orderBy } = withTimeFilters(query);
  const { keyword: rawKeyword, ...filters } = rest;
  const keyword = typeof rawKeyword === "string" ? rawKeyword : undefined;
  return { ...filters, _page: page, _size: size, _keyword: keyword, _orderBy: orderBy };
}

/**
 * jobx_task 的 DAO 没有配置 `like_columns`，所以 `_keyword` 对任务列表是空操作
 * （服务端不会报错，但也不会过滤）。在前端把「执行 ID / 执行器」的关键字翻译成
 * 真实可用的字段条件：纯数字按 ID 精确匹配，其余按执行器编码模糊匹配。
 * 后端补上 `like_columns` 后，这里可以退回 `_keyword`。
 */
function taskQuery(query: TaskQuery) {
  const { rest, page, size, orderBy } = withTimeFilters(query);
  const { keyword, ...filters } = rest;
  const needle = typeof keyword === "string" ? keyword.trim() : "";
  if (needle) {
    if (/^\d+$/.test(needle)) filters.id = needle;
    else {
      filters["executor_code-op"] = "ct";
      filters.executor_code = needle;
    }
  }
  return { ...filters, _page: page, _size: size, _orderBy: orderBy };
}

export const liveJobs = {
  page: (query: JobQuery) => request<Page<Job>>("/jobx/job/page", { query: jobQuery(query) }),

  list: async (query: JobQuery = {}): Promise<Job[]> =>
    (await request<Job[]>("/jobx/job/list", { query: jobQuery(query) })) ?? [],

  get: (id: string) => request<Job>(`/jobx/job/${id}`),

  create: (input: JobInput) => request<Job>("/jobx/job", { method: "POST", body: input }),

  update: (input: JobInput) =>
    request<Job>("/jobx/job", { method: "PUT", body: { ...input, id: input.id } }),

  remove: (id: string) => request<null>(`/jobx/job/${id}`, { method: "DELETE" }),

  /** 手动分派：服务端立刻建一条执行记录并投递 Redis Stream */
  trigger: (id: string) =>
    request<Task>("/jobx/job/trigger", { method: "POST", body: { jobId: id } }),
};

export const liveTasks = {
  page: (query: TaskQuery) => request<Page<Task>>("/jobx/task/page", { query: taskQuery(query) }),

  list: async (query: TaskQuery = {}): Promise<Task[]> =>
    (await request<Task[]>("/jobx/task/list", { query: taskQuery(query) })) ?? [],

  get: (id: string) => request<Task>(`/jobx/task/${id}`),

  remove: (id: string) => request<null>(`/jobx/task/${id}`, { method: "DELETE" }),
};

export const liveGroups = {
  page: (query: GroupQuery) =>
    request<Page<Group>>("/jobx/group/page", { query: pageQuery(query) }),

  /** 下拉与分组列都要用全量，一次 200 条足够（分组由人工维护，不会有几千个） */
  list: async (query: GroupQuery = {}): Promise<Group[]> => {
    const { size = 200, orderBy = "name:asc", ...rest } = query;
    return (
      (await request<Group[]>("/jobx/group/list", {
        query: pageQuery({ ...rest, size, orderBy }),
      })) ?? []
    );
  },

  get: (id: string) => request<Group>(`/jobx/group/${id}`),

  create: (input: GroupInput) => request<Group>("/jobx/group", { method: "POST", body: input }),

  update: (input: GroupInput) =>
    request<Group>("/jobx/group", { method: "PUT", body: { ...input, id: input.id } }),

  remove: (id: string) => request<null>(`/jobx/group/${id}`, { method: "DELETE" }),
};
