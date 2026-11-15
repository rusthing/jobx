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

/** 后端的查询参数是 `_page` / `_size` / `_keyword` / `_orderBy`，其余按 DTO 字段名直传 */
function pageQuery(query: GroupQuery) {
  const { page = 1, size = 20, keyword, orderBy, ...rest } = query;
  return { ...rest, _page: page, _size: size, _keyword: keyword, _orderBy: orderBy };
}

/**
 * 计划 / 执行列表统一走 `search/page`（Bean Searcher 风格），而不是 `page`：
 *
 * - `page` 只认 QueryDto 里的字段名，`create_ms-op=ge` 这类条件会被静默丢掉，
 *   「最近 24 小时」之类的筛选看起来选中了、实际没有任何过滤效果；
 * - `page` 也不处理 `_size`，`list` 会把整张表拉回来（执行记录尤其致命）。
 *
 * `search/page` 的条件按**列名**（snake_case）传，支持 `字段-op` 运算符、
 * `_page` / `_size` / `_keyword` / `_orderBy`，与后端 `#[dao]` 的搜索约定一致。
 */
function jobQuery(query: JobQuery) {
  const {
    page = 1,
    size = 20,
    orderBy,
    keyword,
    jobType,
    enabled,
    executorCode,
    groupId,
    highFreq,
    createdAfterMs,
  } = query;

  const filters: Record<string, unknown> = {};
  if (jobType !== undefined) filters.job_type = jobType;
  if (enabled !== undefined) filters.enabled = enabled;
  if (executorCode) filters.executor_code = executorCode;
  if (groupId) filters.group_id = groupId;
  if (highFreq !== undefined) filters.high_freq = highFreq;
  if (createdAfterMs) {
    filters["create_ms-op"] = "ge";
    filters.create_ms = createdAfterMs;
  }

  return { ...filters, _page: page, _size: size, _keyword: keyword, _orderBy: orderBy };
}

/**
 * jobx_task 的 DAO 没有配置 `like_columns`，所以 `_keyword` 对任务列表是空操作
 * （服务端不会报错，但也不会过滤）。在前端把「执行 ID / 执行器」的关键字翻译成
 * 真实可用的字段条件：纯数字按 ID 精确匹配，其余按执行器编码模糊匹配。
 * 后端补上 `like_columns` 后，这里可以退回 `_keyword`。
 */
function taskQuery(query: TaskQuery) {
  const {
    page = 1,
    size = 20,
    orderBy,
    keyword,
    status,
    taskType,
    executorCode,
    jobId,
    assignAfterMs,
  } = query;

  const filters: Record<string, unknown> = {};
  if (status !== undefined) filters.status = status;
  if (taskType !== undefined) filters.task_type = taskType;
  if (executorCode) filters.executor_code = executorCode;
  if (jobId) filters.job_id = jobId;
  if (assignAfterMs) {
    filters["assign_ms-op"] = "ge";
    filters.assign_ms = assignAfterMs;
  }

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
  page: (query: JobQuery) =>
    request<Page<Job>>("/jobx/job/search/page", { query: jobQuery(query) }),

  list: async (query: JobQuery = {}): Promise<Job[]> =>
    (await request<Page<Job>>("/jobx/job/search/page", { query: jobQuery(query) }))?.list ?? [],

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
  page: (query: TaskQuery) =>
    request<Page<Task>>("/jobx/task/search/page", { query: taskQuery(query) }),

  list: async (query: TaskQuery = {}): Promise<Task[]> =>
    (await request<Page<Task>>("/jobx/task/search/page", { query: taskQuery(query) }))?.list ?? [],

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
