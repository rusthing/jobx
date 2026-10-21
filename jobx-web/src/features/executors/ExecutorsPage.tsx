import { useMemo } from "react";
import { Link } from "react-router-dom";
import { ArrowUpRight, Boxes, CircleSlash, Cpu, Gauge, Timer, Zap } from "lucide-react";
import { PageHeader, StatCard } from "@/components/page";
import { Panel, PanelState, Skeleton } from "@/components/ui/Panel";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { ActivityStrip } from "@/components/charts";
import { useOverview } from "@/hooks/use-overview";
import { useNow } from "@/hooks/use-now";
import { formatDuration, formatRelative } from "@/lib/time";
import { cn } from "@/lib/utils";
import { setDataMode } from "@/lib/api";

export function ExecutorsPage() {
  const { data, tasks, isLoading, error, refetch } = useOverview();
  const now = useNow(5_000);

  const tasksByExecutor = useMemo(() => {
    const map = new Map<string, typeof tasks>();
    for (const task of tasks) {
      const list = map.get(task.executorCode);
      if (list) list.push(task);
      else map.set(task.executorCode, [task]);
    }
    return map;
  }, [tasks]);

  if (error && !data) {
    return (
      <>
        <PageHeader title="执行器" subtitle="按 executor_code 聚合的执行侧视图。" />
        <Panel>
          <PanelState
            icon={<CircleSlash className="size-4" />}
            title="连不上 JobX 服务"
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
        </Panel>
      </>
    );
  }

  if (isLoading || !data) {
    return (
      <>
        <PageHeader title="执行器" subtitle="按 executor_code 聚合的执行侧视图。" />
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {[0, 1, 2, 3].map((index) => (
            <Skeleton key={index} className="h-[200px] rounded-[14px]" />
          ))}
        </div>
      </>
    );
  }

  const executors = data.executors;
  const totalTasks = executors.reduce((acc, item) => acc + item.tasks24h, 0);
  const weightedRate = executors.length
    ? Math.round(
        executors.reduce((acc, item) => acc + item.successRate * item.tasks24h, 0) /
          Math.max(1, totalTasks),
      )
    : 100;

  return (
    <>
      <PageHeader
        title="执行器"
        subtitle="每条计划通过 executor_code 绑定一个执行器族群 · 统计来自最近 24h 执行记录"
        actions={
          <Badge tone="muted" mono>
            jobx:stream:{"{executor_code}"}
          </Badge>
        }
      />

      <div className="grid gap-3 sm:grid-cols-3">
        <StatCard
          label="执行器族群"
          value={executors.length}
          suffix="个"
          icon={<Boxes className="size-3.5" />}
          footer={`${executors.reduce((acc, item) => acc + item.jobCount, 0)} 条计划分布其上`}
        />
        <StatCard
          label="24h 执行总量"
          value={totalTasks}
          suffix="条"
          icon={<Cpu className="size-3.5" />}
          footer={`${data.running} 条正在运行`}
        />
        <StatCard
          label="整体成功率"
          value={weightedRate}
          suffix="%"
          accent={weightedRate >= 98 ? "success" : weightedRate >= 90 ? "warn" : "danger"}
          icon={<Gauge className="size-3.5" />}
          footer={`按执行量加权 · 失败 ${data.failed24h} 条`}
        />
      </div>

      {executors.length === 0 ? (
        <Panel className="mt-3">
          <PanelState
            icon={<Boxes className="size-4" />}
            title="还没有执行器"
            description="创建一条任务计划并填写执行器编码后，这里会按编码聚合出执行侧视图。"
            action={
              <Link
                to="/jobs?new=1"
                className="inline-flex h-8 items-center rounded-lg bg-accent px-3 text-[13px] font-medium text-white hover:bg-accent-strong"
              >
                新建计划
              </Link>
            }
          />
        </Panel>
      ) : (
        <div className="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {executors.map((executor) => {
            const active =
              executor.lastActivityMs !== null && now - executor.lastActivityMs < 10 * 60_000;
            const rateTone =
              executor.successRate >= 95
                ? "bg-success"
                : executor.successRate >= 90
                  ? "bg-warn"
                  : "bg-danger";

            return (
              <Panel key={executor.code} className="flex flex-col">
                <div className="flex items-start justify-between gap-2 px-4 pt-3.5">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span
                        className={cn(
                          "relative flex size-1.5",
                          active ? "text-success" : "text-muted/50",
                        )}
                      >
                        <span
                          className={cn(
                            "relative size-1.5 rounded-full",
                            active ? "bg-success live-dot" : "bg-muted/40",
                          )}
                        />
                      </span>
                      <h2 className="truncate font-mono text-[13.5px] font-semibold text-ink">
                        {executor.code}
                      </h2>
                      {executor.highFreqCount > 0 && (
                        <Badge tone="violet">
                          <Zap className="size-2.5" />
                          {executor.highFreqCount}
                        </Badge>
                      )}
                    </div>
                    <p className="mt-1 text-[10.5px] text-muted">
                      {executor.lastActivityMs
                        ? `最后活动 ${formatRelative(executor.lastActivityMs, now)}`
                        : "24h 内没有执行"}
                    </p>
                  </div>

                  <Link
                    to={`/jobs?executor=${encodeURIComponent(executor.code)}`}
                    className="inline-flex items-center gap-1 rounded-md border border-line px-1.5 py-1 text-[10.5px] text-muted transition-colors hover:border-line-strong hover:text-ink"
                  >
                    计划 <ArrowUpRight className="size-2.5" />
                  </Link>
                </div>

                <div className="mt-3 grid grid-cols-3 gap-px overflow-hidden border-y border-line bg-line/60 text-center">
                  {[
                    { label: "计划", value: `${executor.enabledCount}/${executor.jobCount}` },
                    { label: "24h 执行", value: String(executor.tasks24h) },
                    { label: "运行中", value: String(executor.running) },
                  ].map((item) => (
                    <div key={item.label} className="bg-surface px-2 py-2">
                      <div className="tnum font-mono text-[13px] text-ink">{item.value}</div>
                      <div className="mt-0.5 text-[10px] text-muted">{item.label}</div>
                    </div>
                  ))}
                </div>

                <div className="px-4 py-3">
                  <div className="flex items-baseline justify-between">
                    <span className="text-[10.5px] text-muted">成功率</span>
                    <span className="tnum font-mono text-[11.5px] text-ink">
                      {executor.successRate}%
                      {executor.failed24h > 0 && (
                        <span className="ml-1 text-danger">· {executor.failed24h} 失败</span>
                      )}
                    </span>
                  </div>
                  <div className="mt-1.5 flex h-1.5 overflow-hidden rounded-full bg-sunken">
                    <div
                      className={cn("h-full", rateTone)}
                      style={{ width: `${executor.successRate}%` }}
                    />
                    {executor.successRate < 100 && (
                      <div
                        className="h-full bg-danger/70"
                        style={{ width: `${100 - executor.successRate}%` }}
                      />
                    )}
                  </div>

                  <div className="mt-3 flex items-end justify-between gap-3">
                    <ActivityStrip tasks={tasksByExecutor.get(executor.code) ?? []} now={now} />
                    <span className="shrink-0 text-[10px] text-muted">24h 活跃度</span>
                  </div>

                  <div className="mt-2.5 flex items-center gap-1.5 text-[10.5px] text-muted">
                    <Timer className="size-3" />
                    平均耗时 {formatDuration(executor.avgDurationMs)}
                  </div>
                </div>
              </Panel>
            );
          })}
        </div>
      )}

      <p className="mt-4 text-[10.5px] leading-relaxed text-muted/80">
        执行器本身不落库：它是计划上的 <code className="font-mono">executor_code</code> 与
        Redis Stream key 的约定。因此这里展示的是「按编码聚合的执行侧事实」，而不是一份执行器注册表。
      </p>
    </>
  );
}
