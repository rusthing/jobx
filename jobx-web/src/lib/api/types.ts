/**
 * JobX 前端领域模型。
 *
 * 契约来自 `jobx-api`（`#[vo]` / `#[crud_dto]`）：字段名一律 camelCase，
 * 所有 `u64`（雪花 ID、毫秒时间戳）在 JSON 里都是**字符串**——19 位 ID 超出
 * JS 安全整数范围，这是后端的刻意约定，前端不许 `Number()` 它们。
 */

export type Id = string;

/** 计划类型：0 手动分派 / 1 cron / 2 固定延迟 / 3 固定频率 */
export const JOB_TYPE = {
  manual: 0,
  cron: 1,
  fixedDelay: 2,
  fixedRate: 3,
} as const;

export type JobType = (typeof JOB_TYPE)[keyof typeof JOB_TYPE];

/** 执行状态：0 运行中 / 1 成功 / 2 失败 */
export const TASK_STATUS = {
  running: 0,
  success: 1,
  failed: 2,
} as const;

export type TaskStatus = (typeof TASK_STATUS)[keyof typeof TASK_STATUS];

/** 任务类型：1 立即执行 / 2 计划执行 */
export const TASK_TYPE = {
  immediate: 1,
  scheduled: 2,
} as const;

export type TaskType = (typeof TASK_TYPE)[keyof typeof TASK_TYPE];

export interface Job {
  id: Id;
  executorCode: string;
  name: string;
  params: string | null;
  jobType: JobType;
  /** 由调度器按高频阈值算出来的派生标记，不可手改 */
  highFreq: boolean | null;
  cron: string | null;
  /** 固定间隔，毫秒 */
  intervalDuration: number | null;
  validBeginMs: string | null;
  validEndMs: string | null;
  /** 分派提前量，毫秒 */
  assignLeadMs: number | null;
  nextAssignMs: string | null;
  /** 发布到 Redis Stream 时的 MAXLEN 近似上限 */
  streamMaxLen: number | null;
  remark: string | null;
  enabled: boolean;
  createrId: string;
  createMs: string;
  updaterId: string;
  updateMs: string;
}

export interface Task {
  id: Id;
  taskType: TaskType;
  jobId: Id | null;
  status: TaskStatus;
  assignMs: string;
  scheduledExecStartMs: string | null;
  executorCode: string;
  executorInstance: string | null;
  execParams: string | null;
  jobType: JobType;
  highFreq: boolean | null;
  cron: string | null;
  intervalDuration: number | null;
  validBeginMs: string | null;
  validEndMs: string | null;
  assignLeadMs: number | null;
  execDetail: string | null;
  execStartMs: string | null;
  execEndMs: string | null;
  createrId: string;
  createMs: string;
  updaterId: string;
  updateMs: string;
}

/** 分页响应体（`PageRx`）：pageNum 从 1 开始 */
export interface Page<T> {
  pageNum: string;
  total: string;
  list: T[];
}

/** 新增/修改计划的请求体（`JobxJobAddDto` / `JobxJobModifyDto`） */
export interface JobInput {
  id?: Id;
  executorCode: string;
  name: string;
  params?: string | null;
  jobType: JobType;
  cron?: string | null;
  intervalDuration?: number | null;
  validBeginMs?: string | null;
  validEndMs?: string | null;
  assignLeadMs?: number | null;
  streamMaxLen?: number | null;
  remark?: string | null;
  enabled: boolean;
}

export interface JobQuery {
  keyword?: string;
  jobType?: JobType;
  enabled?: boolean;
  executorCode?: string;
  highFreq?: boolean;
  orderBy?: string;
  page?: number;
  size?: number;
}

export interface TaskQuery {
  keyword?: string;
  status?: TaskStatus;
  taskType?: TaskType;
  executorCode?: string;
  jobId?: Id;
  orderBy?: string;
  page?: number;
  size?: number;
}

export const JOB_TYPE_META: Record<
  JobType,
  { label: string; short: string; desc: string; tone: "accent" | "info" | "violet" | "muted" }
> = {
  0: { label: "手动分派", short: "手动", desc: "仅在手动触发时执行", tone: "muted" },
  1: { label: "Cron 表达式", short: "Cron", desc: "按 cron 表达式定时执行", tone: "accent" },
  2: { label: "固定延迟", short: "延迟", desc: "上一次结束后间隔固定时长再执行", tone: "info" },
  3: { label: "固定频率", short: "频率", desc: "按固定频率周期性执行", tone: "violet" },
};

export const TASK_STATUS_META: Record<
  TaskStatus,
  { label: string; tone: "info" | "success" | "danger" }
> = {
  0: { label: "运行中", tone: "info" },
  1: { label: "成功", tone: "success" },
  2: { label: "失败", tone: "danger" },
};

export const TASK_TYPE_META: Record<TaskType, { label: string }> = {
  1: { label: "立即执行" },
  2: { label: "计划执行" },
};
