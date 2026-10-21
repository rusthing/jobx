import { Boxes, Clock3, Zap } from "lucide-react";
import {
  JOB_TYPE,
  JOB_TYPE_META,
  TASK_STATUS,
  TASK_STATUS_META,
  type Job,
  type JobType,
  type Task,
} from "@/lib/api";
import { cn } from "@/lib/utils";
import { describeCron } from "@/lib/cron";
import { formatDuration } from "@/lib/time";
import { Badge } from "@/components/ui/Badge";

export function JobTypeBadge({ type, short }: { type: JobType; short?: boolean }) {
  const meta = JOB_TYPE_META[type];
  return (
    <Badge tone={meta.tone} mono={false}>
      {short ? meta.short : meta.label}
    </Badge>
  );
}

export function StatusBadge({ task, compact }: { task: Task; compact?: boolean }) {
  if (task.status === TASK_STATUS.running && task.execStartMs === null) {
    return (
      <Badge tone="warn" dot pulse={!compact}>
        待执行
      </Badge>
    );
  }
  const meta = TASK_STATUS_META[task.status];
  return (
    <Badge tone={meta.tone} dot pulse={task.status === TASK_STATUS.running && !compact}>
      {meta.label}
    </Badge>
  );
}

export function HighFreqBadge() {
  return (
    <Badge tone="violet">
      <Zap className="size-2.5" />
      高频
    </Badge>
  );
}

const EXECUTOR_TONES = [
  "text-accent-strong bg-accent-soft border-accent/15",
  "text-info bg-info-soft border-info/15",
  "text-violet bg-violet-soft border-violet/15",
  "text-warn bg-warn-soft border-warn/15",
  "text-success bg-success-soft border-success/15",
  "text-ink-soft bg-ink/[0.05] border-line",
];

function hash(value: string): number {
  let result = 0;
  for (let index = 0; index < value.length; index += 1) {
    result = (result * 31 + value.charCodeAt(index)) >>> 0;
  }
  return result;
}

export function ExecutorChip({ code, className }: { code: string; className?: string }) {
  const tone = EXECUTOR_TONES[hash(code) % EXECUTOR_TONES.length];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-md border px-1.5 py-[1px] font-mono text-[11px]",
        tone,
        className,
      )}
    >
      <Boxes className="size-2.5 opacity-70" />
      {code}
    </span>
  );
}

/** 调度节奏：cron 原文 + 人话解释；固定间隔直接给时长 */
export function ScheduleText({ job, dense }: { job: Job; dense?: boolean }) {
  if (job.jobType === JOB_TYPE.manual) {
    return <span className="text-[12px] text-muted/70">—</span>;
  }

  if (job.intervalDuration) {
    return (
      <Badge tone="muted" mono>
        <Clock3 className="size-2.5" />
        {formatDuration(job.intervalDuration)}
      </Badge>
    );
  }

  if (!job.cron) return <span className="text-[12px] text-muted">—</span>;

  const hint = describeCron(job.cron);
  return (
    <div className="min-w-0">
      <code
        title={job.cron}
        className="inline-block max-w-full truncate rounded border border-line bg-sunken px-1.5 py-[1px] font-mono text-[11.5px] text-ink-soft"
      >
        {job.cron}
      </code>
      {!dense && (
        <div className="mt-1 truncate text-[11px] text-muted">{hint ?? "自定义节奏"}</div>
      )}
    </div>
  );
}
