import type { JobType, TaskStatus, TaskType } from '../api/types';

/**
 * 字典枚举的「值 → 中文」映射。
 *
 * 值与后端枚举的判别式一一对应（后端 `#[dic]` 宏把 serde 表示固定成整数），
 * 是**稳定契约**：新增取值只能往后追加，不能改动已有数字。
 */

/** 计划类型 */
export const JOB_TYPE = {
  /** 手动分派 */
  Manual: 0,
  /** cron 表达式 */
  Cron: 1,
  /** 固定延迟 */
  FixedDelay: 2,
  /** 固定频率 */
  FixedRate: 3,
} as const;

/** 任务状态 */
export const TASK_STATUS = {
  /** 运行中 */
  Running: 0,
  /** 成功 */
  Success: 1,
  /** 失败 */
  Failed: 2,
} as const;

/** 任务类型 */
export const TASK_TYPE = {
  /** 立即执行 */
  Immediate: 1,
  /** 计划执行 */
  Scheduled: 2,
} as const;

export const JOB_TYPE_MAP: Record<number, string> = {
  [JOB_TYPE.Manual]: '手动分派',
  [JOB_TYPE.Cron]: 'Cron 表达式',
  [JOB_TYPE.FixedDelay]: '固定延迟',
  [JOB_TYPE.FixedRate]: '固定频率',
};

export const TASK_STATUS_MAP: Record<number, string> = {
  [TASK_STATUS.Running]: '运行中',
  [TASK_STATUS.Success]: '成功',
  [TASK_STATUS.Failed]: '失败',
};

export const TASK_TYPE_MAP: Record<number, string> = {
  [TASK_TYPE.Immediate]: '立即执行',
  [TASK_TYPE.Scheduled]: '计划执行',
};

/** `Object.entries` 的 key 是字符串，这里统一转回数字，避免 Select 的 value 类型不一致 */
function toOptions(map: Record<number, string>) {
  return Object.entries(map).map(([value, label]) => ({ value: Number(value), label }));
}

export const JOB_TYPE_OPTIONS = toOptions(JOB_TYPE_MAP);
export const TASK_STATUS_OPTIONS = toOptions(TASK_STATUS_MAP);
export const TASK_TYPE_OPTIONS = toOptions(TASK_TYPE_MAP);

/** 把后端返回值安全地渲染成中文；未知取值原样显示，便于发现契约漂移 */
export function labelOf(map: Record<number, string>, value: number | undefined | null): string {
  if (value == null) return '-';
  return map[value] ?? String(value);
}

export type { JobType, TaskStatus, TaskType };
