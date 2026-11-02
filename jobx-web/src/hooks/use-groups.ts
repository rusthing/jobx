import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { groups, type Group, type GroupInput } from "@/lib/api";

export const groupKeys = {
  all: ["groups"] as const,
  every: ["groups", "list", "every"] as const,
};

const POLL_MS = 15_000;

/**
 * 分组全量列表：计划列表的「分组」下拉与表格里的分组列共用这一份数据。
 *
 * 分组是低频变动的维表（几十条量级），单独拉一次比让计划接口带上关联对象更省：
 * 后者会让每条计划都内联一份分组信息，改名后列表里立刻出现新旧两种叫法。
 */
export function useAllGroups() {
  return useQuery<Group[]>({
    queryKey: groupKeys.every,
    queryFn: () => groups.list(),
    refetchInterval: POLL_MS,
    staleTime: 5_000,
  });
}

export function useGroupMutations() {
  const queryClient = useQueryClient();

  /*
   * 分组改名会让计划列表里的分组列跟着变，但那是通过 groups 缓存映射出来的，
   * 失效 groups 一条就够；jobs 侧没有冗余分组信息，不用跟着失效。
   */
  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: groupKeys.all });
  };

  const create = useMutation({
    mutationFn: (input: GroupInput) => groups.create(input),
    onSuccess: invalidate,
  });

  const update = useMutation({
    mutationFn: (input: GroupInput) => groups.update(input),
    onSuccess: invalidate,
  });

  const remove = useMutation({
    mutationFn: (id: string) => groups.remove(id),
    onSuccess: invalidate,
  });

  return { create, update, remove };
}
