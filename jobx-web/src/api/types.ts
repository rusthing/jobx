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

export type JobType = 'Manual' | 'Cron' | 'FixedDelay' | 'FixedRate';
export type TaskStatus = 'Running' | 'Success' | 'Failed';
export type TaskType = 'Immediate' | 'Scheduled';

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
