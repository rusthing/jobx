export type RoResult = 1 | -1 | -2 | -3;

export interface Ro<T> {
  result: RoResult;
  msg: string;
  timestamp: string;
  extra?: T;
  detail?: string;
  code?: string;
}

export interface PageRx<T> {
  pageNum: string;
  total: string;
  list: T[];
}

/**
 * 字典枚举统一走**整数**（后端 `#[dic]` 宏把 serde 表示固定成变体判别式）。
 * 传变体名字符串会被后端拒绝，所以这里用数字字面量联合类型，编译期就能挡住。
 */
export type JobType = 0 | 1 | 2 | 3; // 0=手动分派 1=cron表达式 2=固定延迟 3=固定频率
export type TaskStatus = 0 | 1 | 2; // 0=运行中 1=成功 2=失败
export type TaskType = 1 | 2; // 1=立即执行 2=计划执行

export interface JobxJobVo {
  id: string;
  executorCode: string;
  name: string;
  params?: string;
  jobType: JobType;
  highFreq?: boolean;
  cron?: string;
  intervalDuration?: number;
  validBeginMs?: string;
  validEndMs?: string;
  assignLeadMs?: number;
  nextAssignMs?: string;
  streamMaxLen?: number;
  remark?: string;
  enabled: boolean;
  createrId: string;
  createMs: string;
  updaterId: string;
  updateMs: string;
}

export interface JobxTaskVo {
  id: string;
  taskType: TaskType;
  jobId?: string;
  status: TaskStatus;
  assignMs: string;
  scheduledExecStartMs?: string;
  executorCode: string;
  executorInstance?: string;
  execParams?: string;
  jobType: JobType;
  highFreq?: boolean;
  cron?: string;
  intervalDuration?: number;
  validBeginMs?: string;
  validEndMs?: string;
  assignLeadMs?: number;
  execDetail?: string;
  execStartMs?: string;
  execEndMs?: string;
  createrId: string;
  createMs: string;
  updaterId: string;
  updateMs: string;
}

export interface JobQuery {
  id?: string;
  executorCode?: string;
  name?: string;
  params?: string;
  jobType?: JobType;
  highFreq?: boolean;
  cron?: string;
  intervalDuration?: number;
  validBeginMs?: string;
  validEndMs?: string;
  assignLeadMs?: number;
  nextAssignMs?: string;
  streamMaxLen?: number;
  remark?: string;
  enabled?: boolean;
  _keyword?: string;
  _orderBy?: string;
  _page?: string;
  _size?: string;
}

export interface TaskQuery {
  id?: string;
  taskType?: TaskType;
  jobId?: string;
  status?: TaskStatus;
  assignMs?: string;
  scheduledExecStartMs?: string;
  executorCode?: string;
  executorInstance?: string;
  execParams?: string;
  jobType?: JobType;
  highFreq?: boolean;
  cron?: string;
  intervalDuration?: number;
  validBeginMs?: string;
  validEndMs?: string;
  assignLeadMs?: number;
  execDetail?: string;
  execStartMs?: string;
  execEndMs?: string;
  _keyword?: string;
  _orderBy?: string;
  _page?: string;
  _size?: string;
}

export interface JobxJobAddDto {
  id?: string;
  executorCode: string;
  name: string;
  params?: string;
  jobType: JobType;
  highFreq?: boolean;
  cron?: string;
  intervalDuration?: number;
  validBeginMs?: string;
  validEndMs?: string;
  assignLeadMs?: number;
  nextAssignMs?: string;
  streamMaxLen?: number;
  remark?: string;
  enabled?: boolean;
}

export interface JobxJobModifyDto {
  id: string;
  executorCode?: string;
  name?: string;
  params?: string;
  jobType?: JobType;
  highFreq?: boolean;
  cron?: string;
  intervalDuration?: number;
  validBeginMs?: string;
  validEndMs?: string;
  assignLeadMs?: number;
  nextAssignMs?: string;
  streamMaxLen?: number;
  remark?: string;
  enabled?: boolean;
}

export interface JobxJobTriggerDto {
  jobId: string;
}
