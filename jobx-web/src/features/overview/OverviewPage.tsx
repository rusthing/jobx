import { useMemo } from "react";
import { Link } from "react-router-dom";
import {
  AlertTriangle,
  ArrowUpRight,
  CalendarClock,
  CircleSlash,
  Gauge,
  Play,
  Timer,
  Zap,
} from "lucide-react";
import { useOverview } from "@/hooks/use-overview";
import { useJobMutations } from "@/hooks/use-jobs";
import { useNow } from "@/hooks/use-now";
import { useToast } from "@/components/ui/Toast";
import { PageHeader, StatCard } from "@/components/page";
import { Panel, PanelHeader, PanelState, Skeleton } from "@/components/ui/Panel";
import { Badge } from "@/components/ui/Badge";
import { Button, IconButton } from "@/components/ui/Button";
import { TrendChart } from "@/components/charts";
import { ExecutorChip, JobTypeBadge } from "@/components/domain";
import { JOB_TYPE, JOB_TYPE_META, setDataMode, type JobType } from "@/lib/api";
import { formatCompact, formatCountdown, formatDuration, formatRelative } from "@/lib/time";
import { percent } from "@/lib/utils";
import { describeCron } from "@/lib/cron";

const TYPE_COLORS: Record<JobType, string> = {
  [JOB_TYPE.cron]: "#146b57",
  [JOB_TYPE.fixedDelay]: "#2a5bd7",
  [JOB_TYPE.fixedRate]: "#6344c6",
  [JOB_TYPE.manual]: "#b8bdb2",
};

function failureText(detail: string | null): string {
  if (!detail) return "执行器未上报详情";
  try {
    const parsed = JSON.parse(detail) as { error?: string; message?: string };
    return parsed.error ?? parsed.message ?? "执行失败";
  } catch {
    return detail.slice(0, 120);
  }
}

export function OverviewPage() {
  const { data, jobs, isLoading, error, refetch } = useOverview();
  const now = useNow(1_000);
  const toast = useToast();
  const { trigger } = useJobMutations();

  const jobById = useMemo(() => new Map(jobs.map((job) => [job.id, job])), [jobs]);

  const fire = (id: string, name: string) => {
    trigger.mutate(id, {
      onSuccess: (task) => toast.success(`已分派「${name}」`, `执行记录 #${task.id}`),
      onError: (mutationError) => toast.error("触发失败", mutationError.message),
    });
  };

  if (error && !data) {
    return (
      <>
        <PageHeader title="调度总览" subtitle="任务计划、执行实例与执行器的实时态势。" />
        <Panel>
          <PanelState
            icon={<CircleSlash className="size-4" />}
            title="连不上 JobX 服务"
            description={
              <>
                {error.message}
                <br />
                检查 jobx-svr 是否已启动（默认 127.0.0.1:9001），或先用演示数据浏览界面。
              </>
            }
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
        <PageHeader title="调度总览" subtitle="任务计划、执行实例与执行器的实时态势。" />
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {[0, 1, 2, 3].map((index) => (
            <Skeleton key={index} className="h-[104px] rounded-[14px]" />
          ))}
        </div>
        <div className="mt-3 grid gap-3 lg:grid-cols-12">
          <Skeleton className="h-[300px] rounded-[14px] lg:col-span-8" />
          <Skeleton className="h-[300px] rounded-[14px] lg:col-span-4" />
        </div>
      </>
    );
  }

  const typeEntries = ([
    JOB_TYPE.cron,
    JOB_TYPE.fixedDelay,
    JOB_TYPE.fixedRate,
    JOB_TYPE.manual,
  ] as JobType[]).map((type) => ({ type, count: data.typeCount[type] }));

  const successTotal = data.hourly.reduce((acc, bucket) => acc + bucket.success, 0);
  const failedTotal = data.hourly.reduce((acc, bucket) => acc + bucket.failed, 0);

  return (
    <>
      <PageHeader
        title="调度总览"
        subtitle={
          <>
            最近 24 小时 · {data.completed24h} 条执行完成，{data.running} 条运行中 ·
            指标由最近 1200 条执行记录聚合
          </>
        }
        actions={
          <>
            {data.highFreqJobs > 0 && (
              <Badge tone="violet">
                <Zap className="size-2.5" />
                {data.highFreqJobs} 个高频计划
              </Badge>
            )}
            <Link
              to="/jobs"
              className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-line bg-surface px-2.5 text-[13px] font-medium text-ink transition-colors hover:border-line-strong hover:bg-sunken"
            >
              <CalendarClock className="size-3.5 text-muted" />
              管理计划
            </Link>
          </>
        }
      />

      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard
          label="启用的任务计划"
          value={data.enabledJobs}
          suffix={`/ ${data.totalJobs}`}
          icon={<CalendarClock className="size-3.5" />}
          footer={
            <span className="flex items-center gap-2">
              <span className="text-ink-soft">{data.pausedJobs} 个已停用</span>
              <span className="text-line-strong">·</span>
              <span className="font-mono text-[10.5px]">{data.highFreqJobs} 高频</span>
            </span>
          }
        />
        <StatCard
          label="正在执行"
          value={data.running}
          suffix="条"
          accent={data.running > 0 ? "warn" : "default"}
          icon={<Timer className="size-3.5" />}
          footer={
            <span className="flex items-center justify-between">
              <span>已分派待执行 {data.queued} 条</span>
              <span className="inline-flex items-center gap-1 text-warn">
                <span className="relative flex size-1.5">
                  <span className="live-dot relative size-1.5 rounded-full bg-warn" />
                </span>
                实时
              </span>
            </span>
          }
        />
        <StatCard
          label="24h 成功率"
          value={data.successRate}
          suffix="%"
          accent={data.successRate >= 98 ? "success" : data.successRate >= 90 ? "warn" : "danger"}
          icon={<Gauge className="size-3.5" />}
          footer={
            <span className="flex items-center justify-between gap-2">
              <span>
                成功 {data.success24h} · 失败 <span className="text-danger">{data.failed24h}</span>
              </span>
              <span className="font-mono text-[10.5px]">{data.completed24h} 完成</span>
            </span>
          }
        />
        <StatCard
          label="执行耗时 P95"
          value={formatDuration(data.p95DurationMs).replace(/\s/g, "")}
          icon={<Timer className="size-3.5" />}
          footer={
            <span className="flex items-center justify-between">
              <span>均值 {formatDuration(data.avgDurationMs).replace(/\s/g, "")}</span>
              <span className="font-mono text-[10.5px]">exec_end − exec_start</span>
            </span>
          }
        />
      </div>

      <div className="mt-3 grid gap-3 lg:grid-cols-12">
        <Panel className="lg:col-span-8">
          <PanelHeader
            title="执行趋势"
            subtitle="按小时聚合，堆叠展示成功与失败"
            actions={
              <span className="tnum text-[11px] text-muted">
                {successTotal} 成功 · {failedTotal} 失败
              </span>
            }
          />
          <div className="px-4 py-4">
            <TrendChart buckets={data.hourly} />
          </div>
        </Panel>

        <Panel className="lg:col-span-4">
          <PanelHeader title="计划构成" subtitle="按调度方式分组" />
          <div className="px-4 py-4">
            <div className="flex h-2.5 w-full overflow-hidden rounded-full bg-sunken">
              {typeEntries.map(({ type, count }) =>
                count > 0 ? (
                  <div
                    key={type}
                    style={{ width: `${percent(count, data.totalJobs)}%`, background: TYPE_COLORS[type] }}
                    className="h-full first:rounded-l-full last:rounded-r-full"
                  />
                ) : null,
              )}
            </div>

            <ul className="mt-4 space-y-2.5">
              {typeEntries.map(({ type, count }) => (
                <li key={type} className="flex items-center gap-2">
                  <span
                    className="size-2 rounded-[2px]"
                    style={{ background: TYPE_COLORS[type] }}
                  />
                  <span className="text-[12.5px] text-ink-soft">{JOB_TYPE_META[type].label}</span>
                  <span className="ml-auto tnum font-mono text-[11.5px] text-muted">
                    {count} · {percent(count, data.totalJobs)}%
                  </span>
                </li>
              ))}
            </ul>

            <div className="mt-4 rounded-lg border border-line bg-sunken/70 px-3 py-2.5">
              <div className="flex items-center justify-between text-[11.5px]">
                <span className="text-muted">启用率</span>
                <span className="tnum font-mono text-ink">{percent(data.enabledJobs, data.totalJobs)}%</span>
              </div>
              <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-line">
                <div
                  className="h-full rounded-full bg-accent"
                  style={{ width: `${percent(data.enabledJobs, data.totalJobs)}%` }}
                />
              </div>
              <p className="mt-2 text-[10.5px] leading-relaxed text-muted">
                停用的计划不会进入分派队列，但执行历史与下次分派时间会保留。
              </p>
            </div>
          </div>
        </Panel>
      </div>

      <div className="mt-3 grid gap-3 lg:grid-cols-12">
        <Panel className="lg:col-span-7">
          <PanelHeader
            title="即将分派"
            subtitle="按 next_assign_ms 排序，已扣减分派提前量"
            actions={
              <Link
                to="/jobs"
                className="inline-flex items-center gap-1 text-[11.5px] text-muted hover:text-accent"
              >
                全部计划 <ArrowUpRight className="size-3" />
              </Link>
            }
          />
          {data.upcoming.length === 0 ? (
            <PanelState compact title="没有待分派的计划" description="所有计划都已停用，或调度已超出有效窗口。" />
          ) : (
            <ul className="divide-y divide-line/70">
              {data.upcoming.map((job) => {
                const countdown = formatCountdown(job.nextAssignMs, now);
                const urgent = (Number(job.nextAssignMs) - now) < 60_000;
                return (
                  <li
                    key={job.id}
                    className="flex items-center gap-3 px-4 py-2.5 transition-colors hover:bg-sunken/60"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <Link
                          to={`/jobs?job=${job.id}`}
                          className="truncate text-[13px] font-medium text-ink hover:text-accent"
                        >
                          {job.name}
                        </Link>
                        <JobTypeBadge type={job.jobType} short />
                      </div>
                      <div className="mt-1 flex items-center gap-2">
                        <ExecutorChip code={job.executorCode} />
                        <span className="truncate text-[10.5px] text-muted">
                          {job.cron ? (describeCron(job.cron) ?? job.cron) : "固定间隔"}
                        </span>
                      </div>
                    </div>

                    <div className="w-[104px] shrink-0 text-right">
                      <div
                        className={`tnum font-mono text-[13px] ${urgent ? "text-warn" : "text-ink"}`}
                      >
                        {countdown}
                      </div>
                      <div className="text-[10.5px] text-muted">{formatCompact(job.nextAssignMs)}</div>
                    </div>

                    <IconButton
                      label="立即触发"
                      onClick={() => fire(job.id, job.name)}
                      disabled={trigger.isPending}
                    >
                      <Play className="size-3.5" />
                    </IconButton>
                  </li>
                );
              })}
            </ul>
          )}
        </Panel>

        <Panel className="lg:col-span-5">
          <PanelHeader
            title="最近失败"
            subtitle={`24h 内 ${data.failed24h} 条`}
            actions={
              <Link
                to="/tasks?status=2"
                className="inline-flex items-center gap-1 text-[11.5px] text-muted hover:text-accent"
              >
                全部 <ArrowUpRight className="size-3" />
              </Link>
            }
          />
          {data.recentFailures.length === 0 ? (
            <PanelState compact title="没有失败记录" description="最近 24 小时内的执行都成功了。" />
          ) : (
            <ul className="divide-y divide-line/70">
              {data.recentFailures.map((task) => {
                const job = task.jobId ? jobById.get(task.jobId) : undefined;
                return (
                  <li key={task.id} className="px-4 py-2.5">
                    <div className="flex items-center gap-2">
                      <AlertTriangle className="size-3.5 shrink-0 text-danger" />
                      <span className="truncate text-[12.5px] font-medium text-ink">
                        {job?.name ?? `计划 #${task.jobId ?? "—"}`}
                      </span>
                      <span className="ml-auto shrink-0 text-[10.5px] text-muted">
                        {formatRelative(task.execEndMs ?? task.assignMs, now)}
                      </span>
                    </div>
                    <p className="mt-1 truncate pl-[22px] font-mono text-[10.5px] text-danger/90">
                      {failureText(task.execDetail)}
                    </p>
                  </li>
                );
              })}
            </ul>
          )}
        </Panel>
      </div>

      <p className="mt-4 text-[10.5px] leading-relaxed text-muted/80">
        数据口径：计划来自 <code className="font-mono">/jobx/job/list</code>，执行记录来自{" "}
        <code className="font-mono">/jobx/task/list</code>（最近 1200 条）。失败详情取自{" "}
        <code className="font-mono">exec_detail</code>，原样展示、不做美化。
      </p>
    </>
  );
}
