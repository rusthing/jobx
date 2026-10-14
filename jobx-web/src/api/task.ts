import { request } from './http';
import type { JobxTaskVo, PageRx, TaskQuery } from './types';

export function getTaskPage(query: TaskQuery) {
  return request<PageRx<JobxTaskVo>>({ method: 'GET', url: '/jobx/task/page', params: query });
}

export function getTaskList(query: TaskQuery) {
  return request<JobxTaskVo[]>({ method: 'GET', url: '/jobx/task/list', params: query });
}

export function getTaskById(id: string) {
  return request<JobxTaskVo>({ method: 'GET', url: `/jobx/task/${id}` });
}
