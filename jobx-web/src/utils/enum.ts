export const JOB_TYPE_MAP: Record<string, string> = {
  Manual: '手动分派',
  Cron: 'Cron 表达式',
  FixedDelay: '固定延迟',
  FixedRate: '固定频率',
};

export const TASK_STATUS_MAP: Record<string, string> = {
  Running: '运行中',
  Success: '成功',
  Failed: '失败',
};

export const TASK_TYPE_MAP: Record<string, string> = {
  Immediate: '立即执行',
  Scheduled: '计划执行',
};

export const JOB_TYPE_OPTIONS = Object.entries(JOB_TYPE_MAP).map(([value, label]) => ({ value, label }));
export const TASK_STATUS_OPTIONS = Object.entries(TASK_STATUS_MAP).map(([value, label]) => ({ value, label }));
export const TASK_TYPE_OPTIONS = Object.entries(TASK_TYPE_MAP).map(([value, label]) => ({ value, label }));
