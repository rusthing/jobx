import { useMemo } from "react";
import { computeOverview } from "@/lib/metrics";
import { useAllJobs } from "./use-jobs";
import { useRecentTasks } from "./use-tasks";
import { useNow } from "./use-now";

/**
 * 总览指标。数据来自两个列表接口（计划全量 + 最近执行记录），
 * 聚合在浏览器侧完成 —— 后端没有专门的统计接口，这里不去假装有。
 */
export function useOverview() {
  const jobsQuery = useAllJobs();
  const tasksQuery = useRecentTasks(1_200);
  const now = useNow(5_000);

  const data = useMemo(() => {
    if (!jobsQuery.data || !tasksQuery.data) return null;
    return computeOverview(jobsQuery.data, tasksQuery.data, now);
  }, [jobsQuery.data, tasksQuery.data, now]);

  return {
    data,
    jobs: jobsQuery.data ?? [],
    tasks: tasksQuery.data ?? [],
    isLoading: jobsQuery.isLoading || tasksQuery.isLoading,
    error: (jobsQuery.error ?? tasksQuery.error) as Error | null,
    refetch: () => {
      void jobsQuery.refetch();
      void tasksQuery.refetch();
    },
  };
}
