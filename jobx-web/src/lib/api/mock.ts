/**
 * 演示数据（内存版 JobX）。
 *
 * 目的很明确：后端没起、或还没接上时，控制台依然能完整走通「列表 → 过滤 → 编辑 → 触发 →
 * 看执行记录」的每条路径，且分页/排序/外键约束的行为与真实服务端一致（包括删不掉有执行
 * 记录的计划）。它只在显式开启演示模式时生效，顶栏一直挂着标记，不会假装成真实数据。
 *
 * 数据用固定种子生成：每次刷新看到的是同一批计划与执行历史，便于截图与对照。
 */

import { nextRuns } from "../cron";
import { ApiError } from "./http";
import {
  JOB_TYPE,
  TASK_STATUS,
  TASK_TYPE,
  type Job,
  type JobInput,
  type JobQuery,
  type Page,
  type Task,
  type TaskQuery,
} from "./types";

const HOUR = 3_600_000;
const DAY = 24 * HOUR;

function mulberry32(seed: number) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

const random = mulberry32(20_261_021);

function pick<T>(items: T[]): T {
  return items[Math.floor(random() * items.length)];
}

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

let idCursor = 0;
const ID_BASE = 1_895_243_000_000_000_000n;
function newId(): string {
  idCursor += 1;
  return String(ID_BASE + BigInt(idCursor) + BigInt(Math.floor(random() * 9_000)));
}

const SEED_NOW = Date.now();

interface JobSeed {
  name: string;
  executorCode: string;
  jobType: number;
  cron?: string;
  intervalDuration?: number;
  params?: string;
  remark?: string;
  enabled: boolean;
  assignLeadMs: number;
  streamMaxLen?: number;
  /** 任务保留时长（毫秒），不填表示永久保留 */
  taskExpireDuration?: number;
  validEndOffsetDays?: number;
  createdDaysAgo: number;
}

const JOB_SEEDS: JobSeed[] = [
  {
    name: "订单增量同步",
    executorCode: "order-sync",
    jobType: JOB_TYPE.cron,
    cron: "0 */5 * * * *",
    params: '{"source":"mysql","batchSize":500,"retry":2}',
    remark: "准实时增量，5 分钟一批",
    enabled: true,
    assignLeadMs: 2_000,
    streamMaxLen: 20_000,
    taskExpireDuration: 7 * DAY,
    createdDaysAgo: 46,
  },
  {
    name: "订单状态回补",
    executorCode: "order-sync",
    jobType: JOB_TYPE.cron,
    cron: "0 */15 * * * *",
    params: '{"windowMinutes":30,"dryRun":false}',
    remark: "补偿上游状态机漏推",
    enabled: true,
    assignLeadMs: 5_000,
    taskExpireDuration: 30 * DAY,
    createdDaysAgo: 41,
  },
  {
    name: "库存快照",
    executorCode: "order-sync",
    jobType: JOB_TYPE.cron,
    cron: "0 0 */6 * * *",
    params: '{"warehouse":"ALL"}',
    enabled: true,
    assignLeadMs: 30_000,
    createdDaysAgo: 33,
  },
  {
    name: "每日对账报表",
    executorCode: "report-render",
    jobType: JOB_TYPE.cron,
    cron: "0 0 2 * * *",
    params: '{"template":"daily-reconcile","notify":["finance@corp"]}',
    remark: "凌晨出对账差异表",
    enabled: true,
    assignLeadMs: 60_000,
    createdDaysAgo: 60,
  },
  {
    name: "经营日报 PDF",
    executorCode: "report-render",
    jobType: JOB_TYPE.cron,
    cron: "0 30 7 * * MON-FRI",
    params: '{"template":"ops-daily","paper":"A4"}',
    enabled: true,
    assignLeadMs: 60_000,
    createdDaysAgo: 60,
  },
  {
    name: "月度账单汇总",
    executorCode: "billing-rollup",
    jobType: JOB_TYPE.cron,
    cron: "0 30 0 1 * *",
    params: '{"period":"last-month","currency":"CNY"}',
    remark: "月初 00:30 汇总上月账单",
    enabled: true,
    assignLeadMs: 120_000,
    createdDaysAgo: 120,
  },
  {
    name: "账单异常重算",
    executorCode: "billing-rollup",
    jobType: JOB_TYPE.fixedDelay,
    intervalDuration: 20 * 60_000,
    params: '{"onlyDisputed":true}',
    enabled: true,
    assignLeadMs: 10_000,
    createdDaysAgo: 22,
  },
  {
    name: "营销邮件派发",
    executorCode: "mail-dispatch",
    jobType: JOB_TYPE.cron,
    cron: "0 0 9,14,18 * * *",
    params: '{"campaign":"autumn-2026","throttle":200}',
    enabled: true,
    assignLeadMs: 60_000,
    createdDaysAgo: 15,
  },
  {
    name: "邮件退信回执处理",
    executorCode: "mail-dispatch",
    jobType: JOB_TYPE.fixedRate,
    intervalDuration: 8 * 60_000,
    params: '{"imap":"bounce@corp"}',
    enabled: true,
    assignLeadMs: 5_000,
    createdDaysAgo: 15,
  },
  {
    name: "商品索引重建",
    executorCode: "search-index",
    jobType: JOB_TYPE.cron,
    cron: "*/40 * * * * *",
    params: '{"shards":8,"force":false}',
    remark: "40 秒一轮，大促前暂停避免抢 IO",
    enabled: false,
    assignLeadMs: 30_000,
    createdDaysAgo: 90,
  },
  {
    name: "索引热词统计",
    executorCode: "search-index",
    jobType: JOB_TYPE.cron,
    cron: "0 */15 * * * *",
    params: '{"topN":200}',
    enabled: true,
    assignLeadMs: 5_000,
    createdDaysAgo: 90,
  },
  {
    name: "日志归档",
    executorCode: "log-archive",
    jobType: JOB_TYPE.cron,
    cron: "0 0 3 * * *",
    params: '{"keepDays":30,"compress":true}',
    enabled: true,
    assignLeadMs: 60_000,
    validEndOffsetDays: 30,
    createdDaysAgo: 75,
  },
  {
    name: "冷数据清理",
    executorCode: "log-archive",
    jobType: JOB_TYPE.manual,
    params: '{"olderThanDays":180}',
    remark: "只在存储告警时手动触发",
    enabled: true,
    assignLeadMs: 0,
    createdDaysAgo: 12,
  },
  {
    name: "结算文件推送",
    executorCode: "billing-rollup",
    jobType: JOB_TYPE.cron,
    cron: "0 0 6 * * *",
    params: '{"target":"sftp://settle","sign":true}',
    remark: "上游 SFTP 偶发超时，失败率偏高",
    enabled: true,
    assignLeadMs: 60_000,
    createdDaysAgo: 28,
  },
];

function buildJob(seed: JobSeed, index: number): Job {
  const createMs = SEED_NOW - seed.createdDaysAgo * DAY;
  const job: Job = {
    id: newId(),
    executorCode: seed.executorCode,
    name: seed.name,
    params: seed.params ?? null,
    jobType: seed.jobType as Job["jobType"],
    highFreq: null,
    cron: seed.cron ?? null,
    intervalDuration: seed.intervalDuration ?? null,
    validBeginMs: null,
    validEndMs:
      seed.validEndOffsetDays === undefined
        ? null
        : String(SEED_NOW + seed.validEndOffsetDays * DAY),
    assignLeadMs: seed.assignLeadMs,
    nextAssignMs: null,
    streamMaxLen: seed.streamMaxLen ?? null,
    taskExpireDuration: seed.taskExpireDuration ?? null,
    remark: seed.remark ?? null,
    enabled: seed.enabled,
    createrId: "1",
    createMs: String(createMs + index * 1_337),
    updaterId: "1",
    updateMs: String(createMs + index * 1_337 + HOUR),
  };
  job.highFreq = isHighFrequency(job);
  job.nextAssignMs = nextAssign(job);
  return job;
}

const HIGH_FREQ_THRESHOLD_MS = 60_000;

function isHighFrequency(job: Job): boolean {
  if (job.jobType === JOB_TYPE.cron && job.cron) {
    try {
      const runs = nextRuns(job.cron, SEED_NOW, 2);
      if (runs.length === 2) return runs[1] - runs[0] < HIGH_FREQ_THRESHOLD_MS;
    } catch {
      return false;
    }
  }
  if (job.intervalDuration) return job.intervalDuration < HIGH_FREQ_THRESHOLD_MS;
  return false;
}

function nextAssign(job: Job, from = Date.now()): string | null {
  if (!job.enabled || job.jobType === JOB_TYPE.manual) return null;
  if (job.jobType === JOB_TYPE.cron && job.cron) {
    try {
      const run = nextRuns(job.cron, from, 1)[0];
      if (!run) return null;
      return String(run - (job.assignLeadMs ?? 0));
    } catch {
      return null;
    }
  }
  if (job.intervalDuration) return String(from + job.intervalDuration);
  return null;
}

const INSTANCE_SUFFIX = ["7d9f4c8b6d-x2v4k", "7d9f4c8b6d-h9m2p", "5c7b1a2e4f-q8t3n", "5c7b1a2e4f-w4r6s"];

function instanceOf(executorCode: string) {
  return `${executorCode}-${pick(INSTANCE_SUFFIX)}`;
}

function execDetailFor(failed: boolean, index: number): string {
  if (failed) {
    const reasons = [
      "upstream connect timeout after 30000ms",
      "sftp: authentication failed (key rotated)",
      "deadlock detected while writing jobx_task",
      "redis stream XADD failed: NOGROUP",
    ];
    return JSON.stringify(
      { ok: false, error: reasons[index % reasons.length], attempt: 1 + (index % 3) },
      null,
      2,
    );
  }
  return JSON.stringify(
    {
      ok: true,
      affected: 120 + Math.floor(random() * 8_800),
      elapsedMs: 200 + Math.floor(random() * 4_000),
    },
    null,
    2,
  );
}

/** 按计划推导出过去 24h 的触发时刻，比随机撒点更像真实调度 */
function runTimesOf(job: Job): number[] {
  const since = SEED_NOW - DAY;
  const createdMs = Number(job.createMs);

  if (job.jobType === JOB_TYPE.manual) {
    return [SEED_NOW - 8 * HOUR, SEED_NOW - 27 * HOUR].filter((time) => time >= createdMs);
  }

  // 停用的计划不再产生新记录，只留几笔“停用之前”的历史
  if (!job.enabled) {
    return [SEED_NOW - 9 * DAY, SEED_NOW - 7 * DAY, SEED_NOW - 5 * DAY].filter(
      (time) => time >= createdMs,
    );
  }

  if (job.jobType === JOB_TYPE.cron && job.cron) {
    try {
      return nextRuns(job.cron, since, 2_000).filter(
        (time) => time <= SEED_NOW && time >= createdMs,
      );
    } catch {
      return [];
    }
  }

  const interval = job.intervalDuration ?? 15 * 60_000;
  const times: number[] = [];
  for (let time = SEED_NOW - interval; time > since && times.length < 2_000; time -= interval) {
    if (time >= createdMs) times.push(time);
  }
  return times;
}

function buildTasks(jobs: Job[]): Task[] {
  const tasks: Task[] = [];
  let index = 0;

  for (const job of jobs) {
    const times = runTimesOf(job);
    const flaky = job.name.includes("结算");
    times.forEach((scheduledExecStartMs, position) => {
      index += 1;
      const failed = flaky ? random() < 0.2 : random() < 0.03;
      const isLatest = position === times.length - 1;
      // 让主链路的最后一轮停在“执行中”，好让总览有活的运行态可看
      const running =
        job.enabled &&
        isLatest &&
        (job.executorCode === "mail-dispatch" || job.executorCode === "order-sync");

      const assignMs = scheduledExecStartMs - (job.assignLeadMs ?? 0);
      const execStartMs = running
        ? SEED_NOW - Math.floor(random() * 25_000)
        : scheduledExecStartMs + Math.floor(random() * 1_200);
      const execEndMs = running ? null : execStartMs + Math.floor(180 + random() * 12_000);

      tasks.push({
        id: newId(),
        taskType: job.jobType === JOB_TYPE.manual ? TASK_TYPE.immediate : TASK_TYPE.scheduled,
        jobId: job.id,
        status: running
          ? TASK_STATUS.running
          : failed
            ? TASK_STATUS.failed
            : TASK_STATUS.success,
        assignMs: String(assignMs),
        scheduledExecStartMs: job.jobType === JOB_TYPE.manual ? null : String(scheduledExecStartMs),
        executorCode: job.executorCode,
        executorInstance: instanceOf(job.executorCode),
        execParams: job.params,
        jobType: job.jobType,
        highFreq: job.highFreq,
        cron: job.cron,
        intervalDuration: job.intervalDuration,
        validBeginMs: job.validBeginMs,
        validEndMs: job.validEndMs,
        assignLeadMs: job.assignLeadMs,
        execDetail: running ? null : execDetailFor(failed, index),
        execStartMs: String(execStartMs),
        execEndMs: execEndMs === null ? null : String(execEndMs),
        createrId: "1",
        createMs: String(assignMs),
        updaterId: "1",
        updateMs: String(execEndMs ?? execStartMs),
      });
    });
  }

  // 两条“计划执行但还没到点”的未来记录，让任务列表也能看到待执行态（后端会提前建单）
  const scheduledJobs = jobs.filter((job) => job.enabled && job.nextAssignMs);
  scheduledJobs.slice(0, 3).forEach((job) => {
    const start = Number(job.nextAssignMs) + (job.assignLeadMs ?? 0);
    tasks.push({
      id: newId(),
      taskType: TASK_TYPE.scheduled,
      jobId: job.id,
      status: TASK_STATUS.running,
      assignMs: String(SEED_NOW - 20_000),
      scheduledExecStartMs: String(start),
      executorCode: job.executorCode,
      executorInstance: null,
      execParams: job.params,
      jobType: job.jobType,
      highFreq: job.highFreq,
      cron: job.cron,
      intervalDuration: job.intervalDuration,
      validBeginMs: job.validBeginMs,
      validEndMs: job.validEndMs,
      assignLeadMs: job.assignLeadMs,
      execDetail: null,
      execStartMs: null,
      execEndMs: null,
      createrId: "1",
      createMs: String(SEED_NOW - 20_000),
      updaterId: "1",
      updateMs: String(SEED_NOW - 20_000),
    });
  });

  return tasks.sort((a, b) => Number(b.assignMs) - Number(a.assignMs));
}

const state = {
  jobs: JOB_SEEDS.map(buildJob),
  tasks: [] as Task[],
};
state.tasks = buildTasks(state.jobs);

// ---------------------------------------------------------------------------
// 查询 / 变更
// ---------------------------------------------------------------------------

const SORT_FIELD: Record<string, keyof Job | keyof Task> = {
  next_assign_ms: "nextAssignMs",
  assign_ms: "assignMs",
  create_ms: "createMs",
  update_ms: "updateMs",
  exec_start_ms: "execStartMs",
  exec_end_ms: "execEndMs",
  name: "name",
  status: "status",
  id: "id",
};

function sortItems<T>(items: T[], orderBy?: string): T[] {
  if (!orderBy) return items;
  const [rawField, direction = "asc"] = orderBy.split(":");
  const field = SORT_FIELD[rawField] ?? rawField;
  const sign = direction === "desc" ? -1 : 1;
  return [...items].sort((a, b) => {
    const left = (a as Record<string, unknown>)[field as string];
    const right = (b as Record<string, unknown>)[field as string];
    if (left === right) return 0;
    if (left === null || left === undefined) return 1;
    if (right === null || right === undefined) return -1;
    const numeric = typeof left === "string" && typeof right === "string" && /^\d+$/.test(left);
    if (numeric) return (Number(left) - Number(right)) * sign;
    return String(left).localeCompare(String(right), "zh-CN") * sign;
  });
}

function paginate<T>(items: T[], page = 1, size = 20, orderBy?: string): Page<T> {
  const sorted = sortItems(items, orderBy);
  const total = sorted.length;
  const safeSize = Math.max(1, size);
  const pageNum = Math.min(Math.max(1, page), Math.max(1, Math.ceil(total / safeSize)));
  const list = sorted.slice((pageNum - 1) * safeSize, pageNum * safeSize);
  return { pageNum: String(pageNum), total: String(total), list };
}

function matchesKeyword(haystack: Array<string | null | undefined>, keyword?: string) {
  if (!keyword) return true;
  const needle = keyword.trim().toLowerCase();
  if (!needle) return true;
  return haystack.some((value) => value?.toLowerCase().includes(needle));
}

function ensureUniqueName(name: string, selfId?: string) {
  const exists = state.jobs.some((job) => job.name === name && job.id !== selfId);
  if (exists) throw new ApiError(`计划名称「${name}」已存在`, 200, -2);
}

function validateInput(input: JobInput) {
  if (!input.name.trim()) throw new ApiError("名称不能为空", 400, -1);
  if (!input.executorCode.trim()) throw new ApiError("执行器编码不能为空", 400, -1);
  if (input.jobType === JOB_TYPE.cron && !input.cron?.trim()) {
    throw new ApiError("Cron 类型必须提供表达式", 400, -1);
  }
  if (
    (input.jobType === JOB_TYPE.fixedDelay || input.jobType === JOB_TYPE.fixedRate) &&
    !input.intervalDuration
  ) {
    throw new ApiError("固定间隔类型必须提供间隔时长", 400, -1);
  }
}

function normalizeInput(input: JobInput) {
  const value = { ...input };
  if (value.jobType !== JOB_TYPE.cron) value.cron = null;
  if (value.jobType !== JOB_TYPE.fixedDelay && value.jobType !== JOB_TYPE.fixedRate) {
    value.intervalDuration = null;
  }
  return value;
}

export const mockJobs = {
  async page(query: JobQuery = {}): Promise<Page<Job>> {
    await delay(90 + random() * 120);
    const filtered = state.jobs.filter(
      (job) =>
        matchesKeyword([job.name, job.executorCode, job.remark, job.cron], query.keyword) &&
        (query.jobType === undefined || job.jobType === query.jobType) &&
        (query.enabled === undefined || job.enabled === query.enabled) &&
        (query.highFreq === undefined || job.highFreq === query.highFreq) &&
        (!query.executorCode || job.executorCode === query.executorCode),
    );
    return paginate(filtered, query.page, query.size, query.orderBy);
  },

  async list(query: JobQuery = {}): Promise<Job[]> {
    const page = await mockJobs.page({ ...query, page: 1, size: 5_000 });
    return page.list;
  },

  async get(id: string): Promise<Job> {
    await delay(60);
    const job = state.jobs.find((item) => item.id === id);
    if (!job) throw new ApiError("计划不存在", 404, -2);
    return job;
  },

  async create(input: JobInput): Promise<Job> {
    await delay(160);
    validateInput(input);
    ensureUniqueName(input.name);
    const now = Date.now();
    const payload = normalizeInput(input);
    const job: Job = {
      ...payload,
      id: newId(),
      highFreq: null,
      params: payload.params ?? null,
      cron: payload.cron ?? null,
      intervalDuration: payload.intervalDuration ?? null,
      validBeginMs: payload.validBeginMs ?? null,
      validEndMs: payload.validEndMs ?? null,
      assignLeadMs: payload.assignLeadMs ?? null,
      nextAssignMs: null,
      streamMaxLen: payload.streamMaxLen ?? null,
      taskExpireDuration: payload.taskExpireDuration ?? null,
      remark: payload.remark ?? null,
      createrId: "1",
      createMs: String(now),
      updaterId: "1",
      updateMs: String(now),
    };
    job.highFreq = isHighFrequency(job);
    job.nextAssignMs = nextAssign(job);
    state.jobs.unshift(job);
    return job;
  },

  async update(input: JobInput): Promise<Job> {
    await delay(160);
    if (!input.id) throw new ApiError("缺少 id", 400, -1);
    validateInput(input);
    ensureUniqueName(input.name, input.id);
    const index = state.jobs.findIndex((item) => item.id === input.id);
    if (index < 0) throw new ApiError("计划不存在", 404, -2);
    const payload = normalizeInput(input);
    const updated: Job = {
      ...state.jobs[index],
      ...payload,
      id: input.id,
      highFreq: null,
      updateMs: String(Date.now()),
    };
    updated.highFreq = isHighFrequency(updated);
    updated.nextAssignMs = nextAssign(updated);
    state.jobs[index] = updated;
    return updated;
  },

  async remove(id: string): Promise<null> {
    await delay(140);
    const used = state.tasks.some((task) => task.jobId === id);
    if (used) {
      throw new ApiError("该计划下存在执行记录，外键 on delete restrict 拒绝删除", 200, -2);
    }
    state.jobs = state.jobs.filter((job) => job.id !== id);
    return null;
  },

  async trigger(id: string): Promise<Task> {
    await delay(180);
    const job = state.jobs.find((item) => item.id === id);
    if (!job) throw new ApiError("计划不存在", 404, -2);
    if (!job.enabled) throw new ApiError("计划已停用，无法手动触发", 200, -2);

    const now = Date.now();
    const task: Task = {
      id: newId(),
      taskType: TASK_TYPE.immediate,
      jobId: job.id,
      status: TASK_STATUS.running,
      assignMs: String(now),
      scheduledExecStartMs: null,
      executorCode: job.executorCode,
      executorInstance: instanceOf(job.executorCode),
      execParams: job.params,
      jobType: job.jobType,
      highFreq: job.highFreq,
      cron: job.cron,
      intervalDuration: job.intervalDuration,
      validBeginMs: job.validBeginMs,
      validEndMs: job.validEndMs,
      assignLeadMs: job.assignLeadMs,
      execDetail: null,
      execStartMs: String(now + 220),
      execEndMs: null,
      createrId: "1",
      createMs: String(now),
      updaterId: "1",
      updateMs: String(now),
    };
    state.tasks.unshift(task);
    return task;
  },
};

export const mockTasks = {
  async page(query: TaskQuery = {}): Promise<Page<Task>> {
    await delay(90 + random() * 120);
    const filtered = state.tasks.filter(
      (task) =>
        matchesKeyword(
          [task.id, task.executorCode, task.executorInstance, task.execDetail],
          query.keyword,
        ) &&
        (query.status === undefined || task.status === query.status) &&
        (query.taskType === undefined || task.taskType === query.taskType) &&
        (!query.executorCode || task.executorCode === query.executorCode) &&
        (!query.jobId || task.jobId === query.jobId),
    );
    return paginate(filtered, query.page, query.size, query.orderBy);
  },

  async list(query: TaskQuery = {}): Promise<Task[]> {
    const page = await mockTasks.page({ ...query, page: 1, size: 5_000 });
    return page.list;
  },

  async get(id: string): Promise<Task> {
    await delay(60);
    const task = state.tasks.find((item) => item.id === id);
    if (!task) throw new ApiError("执行记录不存在", 404, -2);
    return task;
  },

  async remove(id: string): Promise<null> {
    await delay(120);
    state.tasks = state.tasks.filter((task) => task.id !== id);
    return null;
  },
};
