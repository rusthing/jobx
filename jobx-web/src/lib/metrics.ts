/**
 * 执行侧的时间判读辅助。
 *
 * 总览 / 执行器的指标已由服务端 `GET /jobx/overview/summary` 聚合（见 `OverviewSummary`），
 * 这里只保留「某条执行记录是不是卡住了」这类纯前端的提示判断。
 */

import { TASK_STATUS, type Task } from "./api/types";
import { toMs } from "./time";

/**
 * 「已分派待执行」的宽限期。
 *
 * 执行器 take 之后要等预定执行时刻才真正开始（等待 = 调度器发布提前量 + assign-lead，
 * 默认最多约 2 分钟），所以几分钟内停在「待执行」是正常的。后端巡检的 start_grace
 * 默认 5 分钟，这里留一倍余量再提示。
 */
export const STALE_QUEUED_MS = 10 * 60_000;

/** 「执行中」的超时提示阈值，与后端巡检的 exec_timeout 默认值一致 */
export const STALE_RUNNING_MS = 30 * 60_000;

/**
 * 状态是否长时间没有推进（疑似僵尸任务）。
 *
 * 只做**提示**，不做过滤：判定依据只有时间，分不清「进程死了」和「任务本来就慢」。
 * 真正的回收由服务端巡检（`jobx-task-reaper`）负责。
 */
export function isStaleTask(task: Task, now = Date.now()): boolean {
  if (task.status !== TASK_STATUS.running) return false;
  const started = toMs(task.execStartMs);
  if (started !== null) return now - started > STALE_RUNNING_MS;
  const anchor = toMs(task.scheduledExecStartMs) ?? toMs(task.assignMs);
  return anchor !== null && now - anchor > STALE_QUEUED_MS;
}
