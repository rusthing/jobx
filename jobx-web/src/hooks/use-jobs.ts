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
        params: job.params,
        jobType: job.jobType,
        cron: job.cron,
        intervalDuration: job.intervalDuration,
        validBeginMs: job.validBeginMs,
        validEndMs: job.validEndMs,
        assignLeadMs: job.assignLeadMs,
        streamMaxLen: job.streamMaxLen,
        remark: job.remark,
        enabled: !job.enabled,
      }),
    onSuccess: invalidate,
  });

  return { create, update, remove, trigger, setEnabled };
}
