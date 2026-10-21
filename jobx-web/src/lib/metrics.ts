/** 总览 / 执行器的聚合计算。纯函数，好在测试里直接喂假数据。 */

import { JOB_TYPE, TASK_STATUS, type Job, type JobType, type Task } from "./api/types";
import { floorToHour, toMs } from "./time";

const HOUR = 3_600_000;

export interface HourBucket {
  hourMs: number;
  success: number;
  failed: number;
}

export interface ExecutorStat {
  code: string;
  jobCount: number;
  enabledCount: number;
  highFreqCount: number;
  tasks24h: number;
  running: number;
  failed24h: number;
  successRate: number;
  lastActivityMs: number | null;
  avgDurationMs: number | null;
}

export interface Overview {
  totalJobs: number;
  enabledJobs: number;
  pausedJobs: number;
  highFreqJobs: number;
  running: number;
  /** 已分派但执行器还没接手的（execStartMs 为空） */
  queued: number;
  success24h: number;
  failed24h: number;
  completed24h: number;
  successRate: number;
  avgDurationMs: number | null;
  p95DurationMs: number | null;
  upcoming: Job[];
  hourly: HourBucket[];
  typeCount: Record<JobType, number>;
  executors: ExecutorStat[];
  recentFailures: Task[];
  windowMs: number;
}

function percentile(sorted: number[], ratio: number): number | null {
  if (sorted.length === 0) return null;
  const index = Math.min(sorted.length - 1, Math.floor(sorted.length * ratio));
  return sorted[index];
}

function completionMs(task: Task): number | null {
  const start = toMs(task.execStartMs);
  const end = toMs(task.execEndMs);
  if (start === null || end === null) return null;
  const duration = end - start;
  return duration >= 0 ? duration : null;
}

export function computeOverview(jobs: Job[], tasks: Task[], now = Date.now()): Overview {
  // 统计窗口与趋势图对齐：24 个整点桶，避免「卡片 363 条 / 图里 309 条」这种对不上
  const hourlyStart = floorToHour(now) - 23 * HOUR;
  const windowStart = hourlyStart;

  const tasks24 = tasks.filter((task) => {
    const time = toMs(task.execStartMs) ?? toMs(task.assignMs) ?? 0;
    return time >= windowStart;
  });

  const completed = tasks24.filter((task) => task.status !== TASK_STATUS.running);
  const success24h = completed.filter((task) => task.status === TASK_STATUS.success).length;
  const failed24h = completed.filter((task) => task.status === TASK_STATUS.failed).length;
  const runningTasks = tasks24.filter(
    (task) => task.status === TASK_STATUS.running && toMs(task.execStartMs) !== null,
  );
  const queued = tasks24.filter(
    (task) => task.status === TASK_STATUS.running && toMs(task.execStartMs) === null,
  ).length;

  const durations = completed
    .map(completionMs)
    .filter((value): value is number => value !== null)
    .sort((a, b) => a - b);

  const buckets: HourBucket[] = Array.from({ length: 24 }, (_, index) => ({
    hourMs: hourlyStart + index * HOUR,
    success: 0,
    failed: 0,
  }));

  for (const task of tasks24) {
    if (task.status === TASK_STATUS.running) continue;
    const time = toMs(task.execStartMs) ?? toMs(task.assignMs);
    if (time === null) continue;
    const index = Math.floor((floorToHour(time) - hourlyStart) / HOUR);
    if (index < 0 || index >= buckets.length) continue;
    if (task.status === TASK_STATUS.success) buckets[index].success += 1;
    else buckets[index].failed += 1;
  }

  const typeCount = {
    [JOB_TYPE.manual]: 0,
    [JOB_TYPE.cron]: 0,
    [JOB_TYPE.fixedDelay]: 0,
    [JOB_TYPE.fixedRate]: 0,
  } as Record<JobType, number>;
  for (const job of jobs) typeCount[job.jobType] += 1;

  const upcoming = jobs
    .filter((job) => job.enabled)
    .map((job) => ({ job, at: toMs(job.nextAssignMs) }))
    .filter((item): item is { job: Job; at: number } => item.at !== null && item.at > now - 60_000)
    .sort((a, b) => a.at - b.at)
    .slice(0, 6)
    .map((item) => item.job);

  const executorMap = new Map<string, ExecutorStat>();

  const ensure = (code: string): ExecutorStat => {
    let stat = executorMap.get(code);
    if (!stat) {
      stat = {
        code,
        jobCount: 0,
        enabledCount: 0,
        highFreqCount: 0,
        tasks24h: 0,
        running: 0,
        failed24h: 0,
        successRate: 0,
        lastActivityMs: null,
        avgDurationMs: null,
      };
      executorMap.set(code, stat);
    }
    return stat;
  };

  for (const job of jobs) {
    const stat = ensure(job.executorCode);
    stat.jobCount += 1;
    if (job.enabled) stat.enabledCount += 1;
    if (job.highFreq) stat.highFreqCount += 1;
  }

  const durationsByExecutor = new Map<string, number[]>();

  for (const task of tasks24) {
    const stat = ensure(task.executorCode);
    stat.tasks24h += 1;
    if (task.status === TASK_STATUS.running) stat.running += 1;
    if (task.status === TASK_STATUS.failed) stat.failed24h += 1;

    const activity = toMs(task.execStartMs) ?? toMs(task.assignMs);
    if (activity !== null && (stat.lastActivityMs === null || activity > stat.lastActivityMs)) {
      stat.lastActivityMs = activity;
    }

    const duration = completionMs(task);
    if (duration !== null) {
      const list = durationsByExecutor.get(task.executorCode) ?? [];
      list.push(duration);
      durationsByExecutor.set(task.executorCode, list);
    }
  }

  for (const stat of executorMap.values()) {
    const done = stat.tasks24h - stat.running;
    stat.successRate = done > 0 ? Math.round(((done - stat.failed24h) / done) * 100) : 100;
    const list = durationsByExecutor.get(stat.code) ?? [];
    stat.avgDurationMs = list.length
      ? Math.round(list.reduce((acc, value) => acc + value, 0) / list.length)
      : null;
  }

  const recentFailures = tasks24
    .filter((task) => task.status === TASK_STATUS.failed)
    .sort((a, b) => Number(b.execEndMs ?? b.assignMs) - Number(a.execEndMs ?? a.assignMs))
    .slice(0, 5);

  return {
    totalJobs: jobs.length,
    enabledJobs: jobs.filter((job) => job.enabled).length,
    pausedJobs: jobs.filter((job) => !job.enabled).length,
    highFreqJobs: jobs.filter((job) => job.highFreq).length,
    running: runningTasks.length,
    queued,
    success24h,
    failed24h,
    completed24h: completed.length,
    successRate: completed.length > 0 ? Math.round((success24h / completed.length) * 100) : 100,
    avgDurationMs: durations.length
      ? Math.round(durations.reduce((acc, value) => acc + value, 0) / durations.length)
      : null,
    p95DurationMs: percentile(durations, 0.95),
    upcoming,
    hourly: buckets,
    typeCount,
    executors: [...executorMap.values()].sort(
      (a, b) => (b.lastActivityMs ?? 0) - (a.lastActivityMs ?? 0) || b.jobCount - a.jobCount,
    ),
    recentFailures,
    windowMs: 24 * HOUR,
  };
}
