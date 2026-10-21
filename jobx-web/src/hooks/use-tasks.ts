import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { tasks, type Page, type Task, type TaskQuery } from "@/lib/api";

export const taskKeys = {
  all: ["tasks"] as const,
  page: (query: TaskQuery) => ["tasks", "page", query] as const,
  recent: (limit: number) => ["tasks", "recent", limit] as const,
  detail: (id: string) => ["tasks", "detail", id] as const,
};

export function useTasksPage(query: TaskQuery, options?: { poll?: boolean }) {
  const poll = options?.poll ?? false;
  return useQuery<Page<Task>>({
    queryKey: taskKeys.page(query),
    queryFn: () => tasks.page(query),
    placeholderData: keepPreviousData,
    // 有运行中的任务时轮询才有意义；页面上由开关控制
    refetchInterval: poll ? 5_000 : false,
    staleTime: 2_000,
  });
}

/** 总览 / 执行器聚合用：最近一批执行记录 */
export function useRecentTasks(limit = 400) {
  return useQuery<Task[]>({
    queryKey: taskKeys.recent(limit),
    queryFn: () => tasks.list({ size: limit, orderBy: "assign_ms:desc" }),
    refetchInterval: 15_000,
    staleTime: 5_000,
  });
}

export function useTaskDetail(id: string | null) {
  return useQuery<Task>({
    queryKey: taskKeys.detail(id ?? ""),
    queryFn: () => tasks.get(id as string),
    enabled: Boolean(id),
  });
}

export function useTaskMutations() {
  const queryClient = useQueryClient();
  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: taskKeys.all });
  };

  const remove = useMutation({
    mutationFn: (id: string) => tasks.remove(id),
    onSuccess: invalidate,
  });

  return { remove };
}
