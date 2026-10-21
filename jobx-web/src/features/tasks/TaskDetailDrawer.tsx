import { memo, useMemo } from "react";
import { Link } from "react-router-dom";
import { Braces, Clock3, ExternalLink, FileText, Hash } from "lucide-react";
import { Drawer } from "@/components/ui/Drawer";
import { Badge } from "@/components/ui/Badge";
import { CopyButton } from "@/components/ui/CopyButton";
import { ExecutorChip, JobTypeBadge, StatusBadge } from "@/components/domain";
import { TASK_STATUS, TASK_TYPE, TASK_TYPE_META, type Job, type Task } from "@/lib/api";
import {
  formatCompact,
  formatDateTime,
  formatDuration,
  formatRelative,
  toMs,
} from "@/lib/time";
import { cn, prettyJson } from "@/lib/utils";

interface Step {
  label: string;
  at: string | null;
  hint?: string;
}

function Timeline({ task }: { task: Task }) {
  const steps: Step[] = [
    { label: "分派", at: task.assignMs, hint: "进入 Redis Stream" },
    {
      label: "预计开始",
      at: task.scheduledExecStartMs,
      hint: task.taskType === TASK_TYPE.scheduled ? "按计划开始时间" : "立即执行，无计划时间",
    },
    { label: "开始执行", at: task.execStartMs, hint: "执行器接手" },
    { label: "结束", at: task.execEndMs, hint: "上报结果" },
  ];

  return (
    <ol className="relative space-y-3 pl-1">
      {steps.map((step, index) => {
        const ms = toMs(step.at);
        const previous = index > 0 ? toMs(steps[index - 1].at) : null;
        const isLast = index === steps.length - 1;
        const pending = ms === null;
        const running = !pending && index === 2 && task.status === TASK_STATUS.running;

        return (
          <li key={step.label} className="relative flex gap-3">
            <div className="relative flex w-3 shrink-0 justify-center">
              {!isLast && (
                <span
                  className={cn(
                    "absolute top-3.5 h-[calc(100%+2px)] w-px",
                    pending ? "bg-line" : "bg-line-strong",
                  )}
                />
              )}
              <span
                className={cn(
                  "relative mt-[3px] size-2 rounded-full border",
                  pending
                    ? "border-line-strong bg-surface"
                    : task.status === TASK_STATUS.failed && isLast
                      ? "border-danger bg-danger"
                      : "border-accent bg-accent",
                )}
              />
            </div>

            <div className="min-w-0 flex-1 pb-0.5">
              <div className="flex items-baseline justify-between gap-2">
                <span
                  className={cn(
                    "text-[12px] font-medium",
                    pending ? "text-muted" : "text-ink-soft",
                  )}
                >
                  {step.label}
                </span>
                <span className="tnum font-mono text-[11px] text-ink">
                  {pending ? "—" : formatDateTime(ms)}
                </span>
              </div>
              <div className="mt-0.5 flex items-center justify-between gap-2 text-[10.5px] text-muted">
                <span>{step.hint}</span>
                {!pending && previous !== null && ms > previous && (
                  <span className="font-mono">+{formatDuration(ms - previous)}</span>
                )}
                {running && <span className="text-info">进行中</span>}
              </div>
            </div>
          </li>
        );
      })}
    </ol>
  );
}

function CodeBlock({
  value,
  empty = "—",
  rows = 6,
}: {
  value: string | null;
  empty?: string;
  rows?: number;
}) {
  // exec_detail 可能是 KB 级 JSON，格式化不该在每次渲染里重跑
  const text = useMemo(() => (value ? prettyJson(value) : empty), [value, empty]);
  return (
    <pre
      className="overflow-auto rounded-lg border border-line bg-sunken/70 px-3 py-2.5 font-mono text-[11px] leading-relaxed text-ink-soft"
      style={{ maxHeight: rows * 20 }}
    >
      {text}
    </pre>
  );
}

function TaskDetailDrawerImpl({
  task,
  job,
  onClose,
}: {
  task: Task | null;
  job: Job | undefined;
  onClose: () => void;
}) {
  const duration = useMemo(() => {
    const start = toMs(task?.execStartMs);
    const end = toMs(task?.execEndMs);
    if (start === null || end === null) return null;
    return end - start;
  }, [task]);

  if (!task) return null;

  return (
    <Drawer
      open
      onClose={onClose}
      width={620}
      title={job?.name ?? `执行记录`}
      badge={<StatusBadge task={task} />}
      subtitle={
        <span className="flex flex-wrap items-center gap-2">
          <span className="font-mono text-[11px]">#{task.id}</span>
          <CopyButton value={task.id} label="复制执行 ID" />
          <span className="text-line-strong">·</span>
          <span className="text-[11px] text-muted">
            {TASK_TYPE_META[task.taskType].label} · {formatRelative(task.assignMs)}
          </span>
        </span>
      }
    >
      <div className="space-y-5">
        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-lg border border-line bg-sunken/50 px-3 py-2">
            <p className="text-[10.5px] text-muted">执行器</p>
            <div className="mt-1 flex items-center gap-1.5">
              <ExecutorChip code={task.executorCode} />
              <JobTypeBadge type={task.jobType} short />
            </div>
            <p className="mt-1.5 truncate font-mono text-[10.5px] text-muted" title={task.executorInstance ?? ""}>
              {task.executorInstance ?? "尚未分配到实例"}
            </p>
          </div>

          <div className="rounded-lg border border-line bg-sunken/50 px-3 py-2">
            <p className="text-[10.5px] text-muted">执行耗时</p>
            <p className="tnum mt-0.5 font-mono text-[15px] text-ink">
              {duration === null ? (task.status === TASK_STATUS.running ? "运行中" : "—") : formatDuration(duration)}
            </p>
            <p className="mt-1 text-[10.5px] text-muted">
              {task.scheduledExecStartMs
                ? `计划开始 ${formatCompact(task.scheduledExecStartMs)}`
                : "立即执行，无计划时间"}
            </p>
          </div>
        </div>

        {job && (
          <Link
            to={`/jobs?job=${job.id}`}
            className="flex items-center justify-between rounded-lg border border-line px-3 py-2 text-[12px] text-ink-soft transition-colors hover:border-line-strong hover:bg-sunken"
          >
            <span className="flex items-center gap-2">
              <Clock3 className="size-3.5 text-muted" />
              任务计划
              <span className="font-medium text-ink">{job.name}</span>
            </span>
            <span className="flex items-center gap-1 text-[11px] text-muted">
              打开 <ExternalLink className="size-3" />
            </span>
          </Link>
        )}

        <section>
          <h3 className="mb-2 flex items-center gap-1.5 text-[11.5px] font-semibold text-ink-soft">
            <Clock3 className="size-3.5 text-muted" />
            执行时间线
          </h3>
          <div className="rounded-xl border border-line px-3 py-3">
            <Timeline task={task} />
          </div>
          <div className="mt-1.5 flex flex-wrap gap-x-3 gap-y-1 text-[10.5px] text-muted">
            <span>创建 {formatDateTime(task.createMs)}</span>
            <span>更新 {formatDateTime(task.updateMs)}</span>
            <span className="font-mono">creater #{task.createrId}</span>
          </div>
        </section>

        <section>
          <h3 className="mb-2 flex items-center justify-between text-[11.5px] font-semibold text-ink-soft">
            <span className="flex items-center gap-1.5">
              <Braces className="size-3.5 text-muted" />
              执行参数
            </span>
            <span className="font-mono text-[10px] font-normal text-muted">exec_params</span>
          </h3>
          <CodeBlock value={task.execParams} empty="未传参数" rows={5} />
        </section>

        <section>
          <h3 className="mb-2 flex items-center justify-between text-[11.5px] font-semibold text-ink-soft">
            <span className="flex items-center gap-1.5">
              <FileText className="size-3.5 text-muted" />
              执行详情
            </span>
            <span className="font-mono text-[10px] font-normal text-muted">exec_detail</span>
          </h3>
          <CodeBlock
            value={task.execDetail}
            empty={task.status === TASK_STATUS.running ? "执行器尚未上报结果" : "无详情"}
            rows={8}
          />
        </section>

        <section className="rounded-lg border border-line bg-sunken/50 px-3 py-2">
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-1.5 text-[10.5px] text-muted">
              <Hash className="size-3" />
              job_id
            </span>
            <span className="font-mono text-[10.5px] text-ink-soft">
              {task.jobId ?? "—"}
              {task.jobId && <CopyButton value={task.jobId} label="复制计划 ID" />}
            </span>
          </div>
          <div className="mt-1 flex items-center justify-between">
            <span className="text-[10.5px] text-muted">调度快照</span>
            <span className="flex items-center gap-1.5">
              <JobTypeBadge type={task.jobType} short />
              {task.highFreq && <Badge tone="violet">高频</Badge>}
              {task.cron && (
                <code className="font-mono text-[10.5px] text-ink-soft">{task.cron}</code>
              )}
              {task.intervalDuration ? (
                <span className="font-mono text-[10.5px] text-ink-soft">
                  {formatDuration(task.intervalDuration)}
                </span>
              ) : null}
            </span>
          </div>
          {task.assignLeadMs !== null && (
            <div className="mt-1 flex items-center justify-between">
              <span className="text-[10.5px] text-muted">分派提前量</span>
              <span className="font-mono text-[10.5px] text-ink-soft">{task.assignLeadMs} ms</span>
            </div>
          )}
        </section>
      </div>
    </Drawer>
  );
}

/**
 * 列表页有个每秒跳动的 `now`（相对时间列），每次 tick 都会让整页重渲染。
 * memo 之后只要 `task` / `job` / `onClose` 的引用不变，抽屉就不再跟着重渲染。
 */
export const TaskDetailDrawer = memo(TaskDetailDrawerImpl);
