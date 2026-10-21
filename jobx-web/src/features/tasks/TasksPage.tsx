import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { Activity, CircleSlash, RefreshCw, Search } from "lucide-react";
import { PageHeader } from "@/components/page";
import { Panel, PanelState, Skeleton } from "@/components/ui/Panel";
import { Button, IconButton } from "@/components/ui/Button";
import { Input, Select } from "@/components/ui/Field";
import { Segmented, Switch } from "@/components/ui/Switch";
import { StatusBadge, ExecutorChip } from "@/components/domain";
import { useToast } from "@/components/ui/Toast";
import { useTaskDetail, useTasksPage } from "@/hooks/use-tasks";
import { useAllJobs } from "@/hooks/use-jobs";
import { useDebounced } from "@/hooks/use-debounce";
import { useNow } from "@/hooks/use-now";
import {
  TASK_STATUS,
  TASK_TYPE,
  setDataMode,
  type Task,
  type TaskStatus,
  type TaskType,
} from "@/lib/api";
import { formatCompact, formatDuration, formatRelative, toMs } from "@/lib/time";
import { cn } from "@/lib/utils";
import { TaskDetailDrawer } from "./TaskDetailDrawer";

const PAGE_SIZE = 25;

export function TasksPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const toast = useToast();
  const now = useNow(1_000);

  const [keywordInput, setKeywordInput] = useState("");
  const keyword = useDebounced(keywordInput, 300);
  const [status, setStatus] = useState<TaskStatus | "all">(() => {
    if (!searchParams.has("status")) return "all";
    const raw = Number(searchParams.get("status"));
    return raw === 0 || raw === 1 || raw === 2 ? (raw as TaskStatus) : "all";
  });
  const [taskType, setTaskType] = useState<TaskType | "all">("all");
  const [executor, setExecutor] = useState("all");
  const [page, setPage] = useState(1);
  const [auto, setAuto] = useState(true);
  const [selected, setSelected] = useState<Task | null>(null);

  // 深链参数只用来初始化一次，随后清掉，避免地址栏与控件脱节
  useEffect(() => {
    if (searchParams.has("status")) setSearchParams({}, { replace: true });
  }, [searchParams, setSearchParams]);

  const query = useMemo(
    () => ({
      keyword: keyword || undefined,
      status: status === "all" ? undefined : status,
      taskType: taskType === "all" ? undefined : taskType,
      executorCode: executor === "all" ? undefined : executor,
      page,
      size: PAGE_SIZE,
      orderBy: "assign_ms:desc",
    }),
    [keyword, status, taskType, executor, page],
  );

  const { data, isLoading, isFetching, error, refetch, dataUpdatedAt } = useTasksPage(query, {
    poll: auto,
  });
  const allJobs = useAllJobs();

  const detail = useTaskDetail(selected?.id ?? null);
  const jobById = useMemo(
    () => new Map((allJobs.data ?? []).map((job) => [job.id, job])),
    [allJobs.data],
  );

  const executors = useMemo(
    () => [...new Set((allJobs.data ?? []).map((job) => job.executorCode))].sort(),
    [allJobs.data],
  );

  const total = Number(data?.total ?? 0);
  const pageNum = Number(data?.pageNum ?? 1);
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  const openDetail = (task: Task) => setSelected(task);

  return (
    <>
      <PageHeader
        title="执行记录"
        subtitle={
          <>
            每条 task 对应一次真实执行 · 共{" "}
            <span className="tnum font-mono text-ink-soft">{total}</span> 条记录
            {auto && (
              <span className="ml-2 inline-flex items-center gap-1.5 text-[11.5px] text-muted">
                <span className="relative flex size-1.5">
                  <span className="live-dot relative size-1.5 rounded-full bg-success" />
                </span>
                每 5s 刷新
              </span>
            )}
          </>
        }
        actions={
          <>
            <label className="flex items-center gap-2 rounded-lg border border-line bg-surface px-2.5 py-1.5">
              <span className="text-[11.5px] text-ink-soft">自动刷新</span>
              <Switch
                size="sm"
                checked={auto}
                onChange={(value) => {
                  setAuto(value);
                  if (value) toast.info("自动刷新已开启", "运行中的记录每 5 秒更新");
                }}
                label="自动刷新"
              />
            </label>
            <Button
              size="sm"
              variant="ghost"
              icon={<RefreshCw className={cn("size-3.5", isFetching && "animate-spin")} />}
              onClick={() => refetch()}
            >
              刷新
            </Button>
          </>
        }
      />

      <Panel className="overflow-hidden">
        <div className="flex flex-wrap items-center gap-2 border-b border-line px-3 py-2.5">
          <div className="relative min-w-[200px] flex-1">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted" />
            <Input
              value={keywordInput}
              onChange={(event) => {
                setKeywordInput(event.target.value);
                setPage(1);
              }}
              placeholder="搜索执行 ID、执行器、实例、详情…"
              className="h-8 pl-8 text-[12.5px]"
            />
          </div>

          <Segmented
            size="sm"
            value={status}
            onChange={(value) => {
              setStatus(value);
              setPage(1);
            }}
            options={[
              { value: "all", label: "全部" },
              { value: TASK_STATUS.running, label: "运行中" },
              { value: TASK_STATUS.success, label: "成功" },
              { value: TASK_STATUS.failed, label: "失败" },
            ]}
          />

          <Select
            value={taskType}
            onChange={(event) => {
              setTaskType(
                event.target.value === "all" ? "all" : (Number(event.target.value) as TaskType),
              );
              setPage(1);
            }}
            className="h-8 w-[118px] text-[12.5px]"
          >
            <option value="all">全部来源</option>
            <option value={TASK_TYPE.immediate}>立即执行</option>
            <option value={TASK_TYPE.scheduled}>计划执行</option>
          </Select>

          <Select
            value={executor}
            onChange={(event) => {
              setExecutor(event.target.value);
              setPage(1);
            }}
            className="h-8 w-[142px] font-mono text-[12px]"
          >
            <option value="all">全部执行器</option>
            {executors.map((code) => (
              <option key={code} value={code}>
                {code}
              </option>
            ))}
          </Select>
        </div>

        {error ? (
          <PanelState
            icon={<CircleSlash className="size-4" />}
            title="执行记录加载失败"
            description={error.message}
            action={
              <div className="flex gap-2">
                <Button size="sm" onClick={() => refetch()}>
                  重试
                </Button>
                <Button size="sm" variant="primary" onClick={() => setDataMode("mock")}>
                  切换到演示数据
                </Button>
              </div>
            }
          />
        ) : isLoading ? (
          <div className="space-y-2 p-4">
            {Array.from({ length: 8 }).map((_, index) => (
              <Skeleton key={index} className="h-11" />
            ))}
          </div>
        ) : !data || data.list.length === 0 ? (
          <PanelState
            icon={<Activity className="size-4" />}
            title="没有执行记录"
            description="调度器还没有分派过任务，或当前筛选条件下没有匹配项。"
            action={
              status !== "all" || keyword ? (
                <Button
                  size="sm"
                  onClick={() => {
                    setStatus("all");
                    setTaskType("all");
                    setExecutor("all");
                    setKeywordInput("");
                  }}
                >
                  清空筛选
                </Button>
              ) : undefined
            }
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[940px] border-collapse text-left">
              <thead>
                <tr className="border-b border-line bg-sunken/60 text-[11px] text-muted">
                  <th className="px-4 py-2 font-medium">状态</th>
                  <th className="px-3 py-2 font-medium">任务计划</th>
                  <th className="px-3 py-2 font-medium">执行器</th>
                  <th className="px-3 py-2 text-right font-medium">分派时间</th>
                  <th className="px-3 py-2 text-right font-medium">执行开始</th>
                  <th className="px-3 py-2 text-right font-medium">耗时</th>
                  <th className="px-4 py-2" />
                </tr>
              </thead>
              <tbody className="divide-y divide-line/70">
                {data.list.map((task) => {
                  const job = task.jobId ? jobById.get(task.jobId) : undefined;
                  const start = toMs(task.execStartMs);
                  const end = toMs(task.execEndMs);
                  const duration = start !== null && end !== null ? end - start : null;
                  const queued = task.status === TASK_STATUS.running && start === null;

                  return (
                    <tr
                      key={task.id}
                      onClick={() => openDetail(task)}
                      className="cursor-pointer transition-colors hover:bg-sunken/50"
                    >
                      <td className="px-4 py-2.5">
                        <StatusBadge task={task} />
                      </td>

                      <td className="max-w-[240px] px-3 py-2.5">
                        <div className="truncate text-[12.5px] font-medium text-ink">
                          {job?.name ?? (task.jobId ? `#${task.jobId}` : "— 未关联计划")}
                        </div>
                        <div className="mt-0.5 flex items-center gap-1.5 font-mono text-[10px] text-muted">
                          <span>#{task.id}</span>
                          <span className="text-line-strong">·</span>
                          <span>{task.taskType === TASK_TYPE.immediate ? "立即" : "计划"}</span>
                        </div>
                      </td>

                      <td className="px-3 py-2.5">
                        <ExecutorChip code={task.executorCode} />
                        {task.executorInstance && (
                          <div className="mt-1 max-w-[170px] truncate font-mono text-[10px] text-muted">
                            {task.executorInstance}
                          </div>
                        )}
                      </td>

                      <td className="px-3 py-2.5 text-right">
                        <div className="tnum text-[11.5px] text-ink-soft">
                          {formatCompact(task.assignMs)}
                        </div>
                        <div className="text-[10px] text-muted">
                          {formatRelative(task.assignMs, now)}
                        </div>
                      </td>

                      <td className="px-3 py-2.5 text-right">
                        <div className="tnum text-[11.5px] text-ink-soft">
                          {start === null ? "—" : formatCompact(start)}
                        </div>
                        {task.scheduledExecStartMs && (
                          <div className="text-[10px] text-muted">
                            计划 {formatCompact(task.scheduledExecStartMs)}
                          </div>
                        )}
                      </td>

                      <td className="px-3 py-2.5 text-right">
                        <span
                          className={cn(
                            "tnum font-mono text-[11.5px]",
                            duration === null ? "text-muted" : "text-ink",
                          )}
                        >
                          {duration !== null
                            ? formatDuration(duration)
                            : queued
                              ? "待执行"
                              : start !== null
                                ? formatDuration(now - start)
                                : "—"}
                        </span>
                      </td>

                      <td className="px-4 py-2.5 text-right">
                        <IconButton
                          label="查看详情"
                          onClick={(event) => {
                            event.stopPropagation();
                            openDetail(task);
                          }}
                        >
                          <Activity className="size-3.5" />
                        </IconButton>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}

        {data && Number(data.total) > 0 && (
          <div className="flex flex-wrap items-center justify-between gap-2 border-t border-line px-4 py-2.5 text-[11.5px] text-muted">
            <span className="tnum">
              第 {(pageNum - 1) * PAGE_SIZE + 1}–{Math.min(pageNum * PAGE_SIZE, total)} 条 / 共{" "}
              {total} 条
              {dataUpdatedAt ? (
                <span className="ml-2 text-[10.5px] text-muted/70">
                  · 更新于 {formatCompact(dataUpdatedAt)}
                </span>
              ) : null}
            </span>
            <div className="flex items-center gap-1.5">
              <Button size="xs" disabled={pageNum <= 1} onClick={() => setPage(pageNum - 1)}>
                上一页
              </Button>
              <span className="tnum px-1 font-mono text-[11px] text-ink-soft">
                {pageNum} / {totalPages}
              </span>
              <Button
                size="xs"
                disabled={pageNum >= totalPages}
                onClick={() => setPage(pageNum + 1)}
              >
                下一页
              </Button>
            </div>
          </div>
        )}
      </Panel>

      <TaskDetailDrawer
        task={detail.data ?? selected}
        job={
          (detail.data ?? selected)?.jobId
            ? jobById.get((detail.data ?? selected)?.jobId as string)
            : undefined
        }
        onClose={() => setSelected(null)}
      />
    </>
  );
}
