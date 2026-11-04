import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { jobs, type Job, type JobInput, type JobQuery, type Page } from "@/lib/api";

export const jobKeys = {
  all: ["jobs"] as const,
  page: (query: JobQuery) => ["jobs", "page", query] as const,
  every: ["jobs", "list", "every"] as const,
  detail: (id: string) => ["jobs", "detail", id] as const,
};

const POLL_MS = 15_000;

export function useJobsPage(query: JobQuery) {
  return useQuery<Page<Job>>({
    queryKey: jobKeys.page(query),
    queryFn: () => jobs.page(query),
    placeholderData: keepPreviousData,
    refetchInterval: POLL_MS,
    staleTime: 5_000,
  });
}

/** 总览 / 执行器页用：一次拉全量计划，聚合在浏览器侧做 */
export function useAllJobs() {
  return useQuery<Job[]>({
    queryKey: jobKeys.every,
    queryFn: () => jobs.list({ size: 500, orderBy: "next_assign_ms:asc" }),
    refetchInterval: POLL_MS,
    staleTime: 5_000,
  });
}

export function useJobDetail(id: string | null) {
  return useQuery<Job>({
    queryKey: jobKeys.detail(id ?? ""),
    queryFn: () => jobs.get(id as string),
    enabled: Boolean(id),
  });
}

export function useJobMutations() {
  const queryClient = useQueryClient();

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: jobKeys.all });
    void queryClient.invalidateQueries({ queryKey: ["tasks"] });
  };

  const create = useMutation({
    mutationFn: (input: JobInput) => jobs.create(input),
    onSuccess: invalidate,
  });

  const update = useMutation({
    mutationFn: (input: JobInput) => jobs.update(input),
    onSuccess: invalidate,
  });

  const remove = useMutation({
    mutationFn: (id: string) => jobs.remove(id),
    onSuccess: invalidate,
  });

  const trigger = useMutation({
    mutationFn: (id: string) => jobs.trigger(id),
    onSuccess: invalidate,
  });

  const setEnabled = useMutation({
    mutationFn: (job: Job) =>
      jobs.update({
        id: job.id,
        executorCode: job.executorCode,
        name: job.name,
        groupId: job.groupId,
        params: job.params,
        jobType: job.jobType,
        cron: job.cron,
        intervalDuration: job.intervalDuration,
        validBeginMs: job.validBeginMs,
        validEndMs: job.validEndMs,
        assignLeadMs: job.assignLeadMs,
        streamMaxLen: job.streamMaxLen,
        taskExpireDuration: job.taskExpireDuration,
        remark: job.remark,
        enabled: !job.enabled,
      }),
    /*
     * 乐观更新：行内开关是最高频操作，等一个 PUT + 列表重拉再翻面会有明显「点了没反应」。
     * 先把所有已缓存的列表/详情改成目标状态，失败再整体回滚，成功以服务端为准重拉。
     */
    onMutate: async (job) => {
      await queryClient.cancelQueries({ queryKey: jobKeys.all });
      const snapshot = queryClient.getQueriesData<Page<Job> | Job[] | Job>({
        queryKey: jobKeys.all,
      });

      const flip = <T extends Job>(item: T): T =>
        item.id === job.id ? { ...item, enabled: !item.enabled } : item;

      queryClient.setQueriesData<Page<Job> | Job[] | Job>({ queryKey: jobKeys.all }, (old) => {
        if (!old) return old;
        if (Array.isArray(old)) return old.map(flip);
        if ("list" in old) return { ...old, list: old.list.map(flip) };
        return flip(old);
      });

      return { snapshot };
    },
    onError: (_error, _job, context) => {
      for (const [key, data] of context?.snapshot ?? []) queryClient.setQueryData(key, data);
    },
    onSettled: invalidate,
  });

  return { create, update, remove, trigger, setEnabled };
}
