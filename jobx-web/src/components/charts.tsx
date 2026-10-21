import { useMemo } from "react";
import { cn } from "@/lib/utils";
import { formatTime } from "@/lib/time";
import type { HourBucket } from "@/lib/metrics";
import type { Task } from "@/lib/api";
import { TASK_STATUS } from "@/lib/api";

/** 迷你趋势线：面板右上角的“最近还好吗” */
export function Sparkline({
  values,
  tone = "accent",
  width = 96,
  height = 28,
  className,
}: {
  values: number[];
  tone?: "accent" | "success" | "danger" | "info";
  width?: number;
  height?: number;
  className?: string;
}) {
  const colors = {
    accent: "#146b57",
    success: "#17794a",
    danger: "#b3392e",
    info: "#2a5bd7",
  } as const;

  const path = useMemo(() => {
    const data = values.length ? values : [0, 0];
    const max = Math.max(...data, 1);
    const step = width / Math.max(1, data.length - 1);
    const points = data.map((value, index) => {
      const x = index * step;
      const y = height - (value / max) * (height - 4) - 2;
      return [x, y] as const;
    });
    const line = points
      .map(([x, y], index) => `${index === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`)
      .join(" ");
    const area = `${line} L${width},${height} L0,${height} Z`;
    return { line, area, last: points[points.length - 1] };
  }, [values, width, height]);

  const color = colors[tone];
  const gradientId = `spark-${tone}-${width}`;

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={className}
      aria-hidden
    >
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.18" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      <path d={path.area} fill={`url(#${gradientId})`} />
      <path d={path.line} fill="none" stroke={color} strokeWidth="1.4" strokeLinecap="round" />
      <circle cx={path.last[0]} cy={path.last[1]} r="2" fill={color} />
    </svg>
  );
}

function niceStep(value: number): number {
  const exponent = Math.floor(Math.log10(Math.max(1, value)));
  const base = value / 10 ** exponent;
  const multiplier = base <= 1 ? 1 : base <= 2 ? 2 : base <= 5 ? 5 : 10;
  return multiplier * 10 ** exponent;
}

/** 24 小时执行趋势：成功/失败堆叠柱，悬停出明细 */
export function TrendChart({ buckets, className }: { buckets: HourBucket[]; className?: string }) {
  const CHART_H = 152;
  const rawMax = Math.max(1, ...buckets.map((bucket) => bucket.success + bucket.failed));
  const step = niceStep(rawMax / 4);
  const niceMax = Math.ceil(rawMax / step) * step;
  const tickCount = Math.round(niceMax / step);
  const ticks = Array.from({ length: tickCount + 1 }, (_, index) => niceMax - index * step);
  const total = buckets.reduce((acc, bucket) => acc + bucket.success + bucket.failed, 0);

  return (
    <div className={cn("relative", className)}>
      <div className="flex items-stretch gap-3">
        <div className="flex w-7 flex-col justify-between pb-[1px] text-right font-mono text-[10px] leading-none text-muted/80">
          {ticks.map((tick, index) => (
            <span key={index}>{tick}</span>
          ))}
        </div>

        <div className="min-w-0 flex-1">
          <div className="relative" style={{ height: CHART_H }}>
            <div className="pointer-events-none absolute inset-0 flex flex-col justify-between">
              {ticks.map((_, index) => (
                <div
                  key={index}
                  className={cn(
                    "h-px w-full",
                    index === ticks.length - 1 ? "bg-line-strong" : "bg-line/70",
                  )}
                />
              ))}
            </div>

            <div className="relative flex h-full items-end gap-[2px]">
              {buckets.map((bucket) => {
                const sum = bucket.success + bucket.failed;
                const failedPx = (bucket.failed / niceMax) * CHART_H;
                const successPx = (bucket.success / niceMax) * CHART_H;
                return (
                  <div
                    key={bucket.hourMs}
                    className="group relative flex h-full flex-1 flex-col justify-end"
                  >
                    <div className="pointer-events-none absolute bottom-[calc(100%+8px)] left-1/2 z-20 hidden -translate-x-1/2 whitespace-nowrap rounded-lg border border-line bg-surface px-2.5 py-1.5 text-left shadow-float group-hover:block">
                      <div className="font-mono text-[11px] text-ink">
                        {formatTime(bucket.hourMs, false)} – {formatTime(bucket.hourMs + 3_600_000, false)}
                      </div>
                      <div className="mt-1 flex items-center gap-2 text-[11px]">
                        <span className="text-success">成功 {bucket.success}</span>
                        <span className="text-danger">失败 {bucket.failed}</span>
                      </div>
                    </div>

                    <div className="flex w-full flex-col justify-end overflow-hidden rounded-[2px]">
                      {bucket.failed > 0 && (
                        <div
                          className="w-full rounded-t-[2px] bg-danger/70 transition-colors group-hover:bg-danger"
                          style={{ height: Math.max(2, failedPx) }}
                        />
                      )}
                      {bucket.success > 0 && (
                        <div
                          className={cn(
                            "w-full bg-accent/60 transition-colors group-hover:bg-accent",
                            bucket.failed === 0 && "rounded-t-[2px]",
                          )}
                          style={{ height: Math.max(2, successPx) }}
                        />
                      )}
                      {sum === 0 && <div className="h-px w-full bg-line-strong/40" />}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          <div className="mt-1.5 flex justify-between font-mono text-[10px] text-muted/80">
            {[0, 6, 12, 18, 23].map((index) => (
              <span key={index}>{formatTime(buckets[index]?.hourMs ?? 0, false)}</span>
            ))}
          </div>
        </div>
      </div>

      <div className="mt-3 flex items-center gap-4 border-t border-line/70 pt-2.5 text-[11px] text-muted">
        <span className="inline-flex items-center gap-1.5">
          <span className="size-2 rounded-[2px] bg-accent/70" /> 成功
        </span>
        <span className="inline-flex items-center gap-1.5">
          <span className="size-2 rounded-[2px] bg-danger/70" /> 失败
        </span>
        <span className="ml-auto tnum">24h 共 {total} 条执行</span>
      </div>
    </div>
  );
}

/** 执行器活跃度条：24 格，每格里叠当小时的成功/失败密度 */
export function ActivityStrip({ tasks, now }: { tasks: Task[]; now: number }) {
  const HOUR = 3_600_000;
  const slots = 24;
  const start = Math.floor((now - (slots - 1) * HOUR) / HOUR) * HOUR;

  const cells = Array.from({ length: slots }, (_, index) => {
    const from = start + index * HOUR;
    const to = from + HOUR;
    const bucket = tasks.filter((task) => {
      const time = Number(task.execStartMs ?? task.assignMs);
      return time >= from && time < to;
    });
    const failed = bucket.filter((task) => task.status === TASK_STATUS.failed).length;
    return { from, count: bucket.length, failed };
  });

  const max = Math.max(1, ...cells.map((cell) => cell.count));

  return (
    <div className="flex items-end gap-[3px]">
      {cells.map((cell) => {
        const height = cell.count === 0 ? 3 : 4 + (cell.count / max) * 22;
        return (
          <div
            key={cell.from}
            title={`${formatTime(cell.from, false)} · ${cell.count} 次${cell.failed ? ` · ${cell.failed} 失败` : ""}`}
            className={cn(
              "w-[8px] rounded-[2px] transition-colors",
              cell.count === 0
                ? "bg-line"
                : cell.failed > 0
                  ? "bg-danger/55 hover:bg-danger"
                  : "bg-accent/45 hover:bg-accent",
            )}
            style={{ height }}
          />
        );
      })}
    </div>
  );
}
