import { useQuery } from "@tanstack/react-query";
import { overview, type OverviewSummary } from "@/lib/api";
import { useAllJobs } from "./use-jobs";
import { useRecentFailures } from "./use-tasks";

export const overviewKeys = {
  summary: ["overview", "summary"] as const,
};

/**
 * 总览指标。
 *
 * 计划 / 24h 执行 / 小时趋势 / 执行器维度都来自一次 `GET /jobx/overview/summary`，
 * 聚合在服务端做（不再受「前 500 条计划 / 1200 条记录」的采样上限影响）。
 * 「即将分派」与「最近失败」仍走列表接口：前者只需要最早的一批计划，
 * 后者只需要几条失败记录，都不值得进统计接口。
 */
export function useOverview() {
  const summaryQuery = useQuery<OverviewSummary>({
    queryKey: overviewKeys.summary,
    queryFn: () => overview.summary(),
    refetchInterval: 15_000,
    staleTime: 5_000,
  });

  const jobsQuery = useAllJobs();
  const failuresQuery = useRecentFailures(summaryQuery.data?.windowStartMs ?? null);

  return {
    data: summaryQuery.data ?? null,
    /** 用于「即将分派」列表与执行记录上的计划名 */
    jobs: jobsQuery.data ?? [],
    /** 24h 内最近几条失败，取自执行列表接口 */
    failures: failuresQuery.data ?? [],
    isLoading: summaryQuery.isLoading,
    error: (summaryQuery.error ?? jobsQuery.error) as Error | null,
    refetch: () => {
      void summaryQuery.refetch();
      void jobsQuery.refetch();
      void failuresQuery.refetch();
    },
  };
}
