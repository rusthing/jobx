import { request } from './http';
import type { JobxJobVo, PageRx, JobQuery, JobxJobAddDto, JobxJobModifyDto, JobxJobTriggerDto } from './types';

export function getJobPage(query: JobQuery) {
  return request<PageRx<JobxJobVo>>({ method: 'GET', url: '/jobx/job/page', params: query });
}

export function getJobList(query: JobQuery) {
  return request<JobxJobVo[]>({ method: 'GET', url: '/jobx/job/list', params: query });
}

export function getJobById(id: string) {
  return request<JobxJobVo>({ method: 'GET', url: `/jobx/job/${id}` });
}

export function addJob(dto: JobxJobAddDto) {
  return request<JobxJobVo>({ method: 'POST', url: '/jobx/job', data: dto });
}

export function modifyJob(dto: JobxJobModifyDto) {
  return request<JobxJobVo>({ method: 'PUT', url: '/jobx/job', data: dto });
}

export function deleteJob(id: string) {
  return request<void>({ method: 'DELETE', url: `/jobx/job/${id}` });
}

export function triggerJob(dto: JobxJobTriggerDto) {
  return request<JobxJobVo>({ method: 'POST', url: '/jobx/job/trigger', data: dto });
}
