import { useEffect, useMemo, useState } from "react";
import { AlertCircle, Braces, CalendarClock, Play, Sparkles, Wand2 } from "lucide-react";
import { Drawer } from "@/components/ui/Drawer";
import { Button } from "@/components/ui/Button";
import { Field, Input, Select, Textarea } from "@/components/ui/Field";
import { Segmented, Switch } from "@/components/ui/Switch";
import { Badge } from "@/components/ui/Badge";
import { CRON_PRESETS, describeCron, nextRuns, normalizeCron, validateCron } from "@/lib/cron";
import { JOB_TYPE, type Job, type JobInput, type JobType } from "@/lib/api";
import {
  DURATION_UNIT_MS,
  formatCompact,
  formatDuration,
  formatRelative,
  fromDateTimeLocal,
  splitDuration,
  toDateTimeLocal,
  type DurationUnit,
} from "@/lib/time";
import { cn, jsonError, prettyJson } from "@/lib/utils";

interface FormState {
  name: string;
  executorCode: string;
  remark: string;
  jobType: JobType;
  cron: string;
  intervalAmount: string;
  intervalUnit: DurationUnit;
  assignLeadSec: string;
  streamMaxLen: string;
  validBegin: string;
  validEnd: string;
  params: string;
  enabled: boolean;
}

function initialState(job: Job | null): FormState {
  if (!job) {
    return {
      name: "",
      executorCode: "",
      remark: "",
      jobType: JOB_TYPE.cron,
      cron: "0 */5 * * * *",
      intervalAmount: "5",
      intervalUnit: "m",
      assignLeadSec: "5",
      streamMaxLen: "",
      validBegin: "",
      validEnd: "",
      params: "",
      enabled: true,
    };
  }

  const duration = splitDuration(job.intervalDuration);
  return {
    name: job.name,
    executorCode: job.executorCode,
    remark: job.remark ?? "",
    jobType: job.jobType,
    cron: job.cron ?? "",
    intervalAmount: String(duration.amount),
    intervalUnit: duration.unit,
    assignLeadSec: job.assignLeadMs === null ? "" : String(Math.round(job.assignLeadMs / 1_000)),
    streamMaxLen: job.streamMaxLen === null ? "" : String(job.streamMaxLen),
    validBegin: toDateTimeLocal(job.validBeginMs),
    validEnd: toDateTimeLocal(job.validEndMs),
    params: job.params ? prettyJson(job.params) : "",
    enabled: job.enabled,
  };
}

const TYPE_OPTIONS = [
  { value: JOB_TYPE.cron, label: "Cron" },
  { value: JOB_TYPE.fixedDelay, label: "固定延迟" },
  { value: JOB_TYPE.fixedRate, label: "固定频率" },
  { value: JOB_TYPE.manual, label: "手动" },
];

export interface JobFormDrawerProps {
  open: boolean;
  job: Job | null;
  executors: string[];
  submitting: boolean;
  onClose: () => void;
  onSubmit: (input: JobInput) => void;
  onTrigger?: () => void;
}

export function JobFormDrawer({
  open,
  job,
  executors,
  submitting,
  onClose,
  onSubmit,
  onTrigger,
}: JobFormDrawerProps) {
  const [form, setForm] = useState<FormState>(() => initialState(job));
  const [touched, setTouched] = useState<Record<string, boolean>>({});
  const [submitted, setSubmitted] = useState(false);

  useEffect(() => {
    if (open) {
      setForm(initialState(job));
      setTouched({});
      setSubmitted(false);
    }
  }, [open, job]);

  const markTouched = (key: string) => () =>
    setTouched((current) => ({ ...current, [key]: true }));

  const set = <K extends keyof FormState>(key: K, value: FormState[K]) =>
    setForm((current) => ({ ...current, [key]: value }));

  const cronError = useMemo(() => {
    if (form.jobType !== JOB_TYPE.cron) return null;
    if (!form.cron.trim()) return "Cron 类型必须提供表达式";
    return validateCron(form.cron);
  }, [form.cron, form.jobType]);

  const paramsError = useMemo(() => jsonError(form.params), [form.params]);

  const intervalMs =
    form.jobType === JOB_TYPE.fixedDelay || form.jobType === JOB_TYPE.fixedRate
      ? Number(form.intervalAmount) * DURATION_UNIT_MS[form.intervalUnit]
      : null;

  const upcoming = useMemo(() => {
    if (form.jobType !== JOB_TYPE.cron || cronError) return [];
    try {
      return nextRuns(form.cron, Date.now(), 4);
    } catch {
      return [];
    }
  }, [form.cron, cronError, form.jobType]);

  const highFreqHint = useMemo(() => {
    if (form.jobType === JOB_TYPE.cron && upcoming.length >= 2) {
      return upcoming[1] - upcoming[0] < 60_000;
    }
    if (intervalMs) return intervalMs < 60_000;
    return false;
  }, [upcoming, intervalMs, form.jobType]);

  const errors = {
    name: form.name.trim() ? null : "名称不能为空",
    executorCode: form.executorCode.trim() ? null : "执行器编码不能为空",
    interval:
      (form.jobType === JOB_TYPE.fixedDelay || form.jobType === JOB_TYPE.fixedRate) &&
      (!Number.isFinite(intervalMs) || (intervalMs ?? 0) <= 0)
        ? "间隔必须大于 0"
        : null,
    validRange:
      form.validBegin && form.validEnd && form.validEnd < form.validBegin
        ? "结束时间不能早于开始时间"
        : null,
  };

  const invalid =
    Boolean(errors.name || errors.executorCode || errors.interval || errors.validRange) ||
    Boolean(cronError) ||
    Boolean(paramsError);

  const submit = () => {
    setSubmitted(true);
    if (invalid) return;
    onSubmit({
      ...(job ? { id: job.id } : {}),
      name: form.name.trim(),
      executorCode: form.executorCode.trim(),
      jobType: form.jobType,
      cron: form.jobType === JOB_TYPE.cron ? normalizeCron(form.cron) : null,
      intervalDuration: intervalMs,
      assignLeadMs: form.assignLeadSec ? Math.round(Number(form.assignLeadSec) * 1_000) : null,
      streamMaxLen: form.streamMaxLen ? Number(form.streamMaxLen) : null,
      validBeginMs: fromDateTimeLocal(form.validBegin),
      validEndMs: fromDateTimeLocal(form.validEnd),
      params: form.params.trim() ? form.params.trim() : null,
      remark: form.remark.trim() ? form.remark.trim() : null,
      enabled: form.enabled,
    });
  };

  const showError = (key: keyof typeof errors) => (submitted || touched[key] ? errors[key] : null);

  return (
    <Drawer
      open={open}
      onClose={onClose}
      title={job ? "编辑任务计划" : "新建任务计划"}
      badge={
        job ? (
          <Badge tone={job.enabled ? "success" : "muted"} dot>
            {job.enabled ? "启用中" : "已停用"}
          </Badge>
        ) : (
          <Badge tone="accent">草稿</Badge>
        )
      }
      subtitle={
        job ? (
          <span className="font-mono text-[11px] text-muted">#{job.id}</span>
        ) : (
          "保存后由调度器计算首次分派时间"
        )
      }
      width={640}
      footer={
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            {job && onTrigger && (
              <Button size="sm" variant="ghost" icon={<Play className="size-3.5" />} onClick={onTrigger}>
                立即触发
              </Button>
            )}
          </div>
          <div className="flex items-center gap-2">
            <Button size="sm" variant="ghost" onClick={onClose}>
              取消
            </Button>
            <Button size="sm" variant="primary" loading={submitting} onClick={submit}>
              {job ? "保存修改" : "创建计划"}
            </Button>
          </div>
        </div>
      }
    >
      <div className="space-y-5">
        <section className="space-y-3.5">
          <div className="flex items-center gap-2">
            <Sparkles className="size-3.5 text-muted" />
            <h3 className="text-[12px] font-semibold tracking-[0.02em] text-ink-soft">基本信息</h3>
          </div>

          <Field label="计划名称" required error={showError("name")}>
            <Input
              value={form.name}
              onChange={(event) => set("name", event.target.value)}
              onBlur={markTouched("name")}
              placeholder="例如：订单增量同步"
              autoFocus={!job}
            />
          </Field>

          <div className="grid grid-cols-2 gap-3">
            <Field
              label="执行器编码"
              required
              error={showError("executorCode")}
              hint="消息发到 jobx:stream:{编码}，只有同编码执行器会订阅"
            >
              <Input
                value={form.executorCode}
                onChange={(event) => set("executorCode", event.target.value)}
                onBlur={markTouched("executorCode")}
                placeholder="order-sync"
                list="jobx-executors"
                className="font-mono"
              />
              <datalist id="jobx-executors">
                {executors.map((code) => (
                  <option key={code} value={code} />
                ))}
              </datalist>
            </Field>

            <Field label="备注" hint="给人看的上下文，可为空">
              <Input
                value={form.remark}
                onChange={(event) => set("remark", event.target.value)}
                placeholder="例：大促期间暂停"
              />
            </Field>
          </div>
        </section>

        <section className="space-y-3.5 border-t border-line pt-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <CalendarClock className="size-3.5 text-muted" />
              <h3 className="text-[12px] font-semibold tracking-[0.02em] text-ink-soft">调度策略</h3>
            </div>
            <Segmented options={TYPE_OPTIONS} value={form.jobType} onChange={(value) => set("jobType", value)} />
          </div>

          {form.jobType === JOB_TYPE.cron && (
            <div className="rounded-xl border border-line bg-sunken/60 p-3">
              <Field
                label="Cron 表达式"
                required
                error={cronError}
                hint={
                  <span className="flex flex-wrap items-center gap-x-1.5">
                    <span>秒 分 时 日 月 星期 · 5 段会自动补 0 秒</span>
                    {!cronError && form.cron && (
                      <>
                        <span className="text-line-strong">·</span>
                        <span className="font-medium text-accent">
                          {describeCron(form.cron) ?? "自定义节奏"}
                        </span>
                      </>
                    )}
                  </span>
                }
                extra={
                  <span className="flex items-center gap-1.5">
                    <Wand2 className="size-3 text-muted" />
                    <select
                      value=""
                      onChange={(event) => {
                        if (event.target.value) set("cron", event.target.value);
                      }}
                      className="h-6 rounded border border-line bg-surface px-1 text-[11px] text-ink-soft focus:outline-none"
                    >
                      <option value="">插入模板…</option>
                      {CRON_PRESETS.map((preset) => (
                        <option key={preset.expression} value={preset.expression}>
                          {preset.label}
                        </option>
                      ))}
                    </select>
                  </span>
                }
              >
                <Input
                  value={form.cron}
                  onChange={(event) => set("cron", event.target.value)}
                  className="font-mono"
                  spellCheck={false}
                  placeholder="0 */5 * * * *"
                />
              </Field>

              <div className="mt-2.5 flex items-start gap-2">
                <span className="mt-[3px] text-[10.5px] font-medium tracking-[0.06em] text-muted">接下来</span>
                <ul className="flex-1 space-y-1">
                  {upcoming.length === 0 ? (
                    <li className="text-[11px] text-muted">暂无触发时间</li>
                  ) : (
                    upcoming.map((time) => (
                      <li key={time} className="flex items-center gap-2 font-mono text-[11px]">
                        <span className="tnum text-ink-soft">{formatCompact(time)}</span>
                        <span className="text-muted/70">{formatRelative(time)}</span>
                      </li>
                    ))
                  )}
                </ul>
              </div>
            </div>
          )}

          {(form.jobType === JOB_TYPE.fixedDelay || form.jobType === JOB_TYPE.fixedRate) && (
            <Field
              label={form.jobType === JOB_TYPE.fixedDelay ? "延迟间隔" : "执行频率"}
              required
              error={showError("interval")}
              hint={
                form.jobType === JOB_TYPE.fixedDelay
                  ? "上一次执行结束后，间隔该时长再分派下一次"
                  : "按固定频率周期性分派，不等待上一次结束"
              }
            >
              <div className="flex gap-2">
                <Input
                  value={form.intervalAmount}
                  onChange={(event) => set("intervalAmount", event.target.value)}
                  onBlur={markTouched("interval")}
                  inputMode="decimal"
                  className="w-32 font-mono"
                />
                <Select
                  value={form.intervalUnit}
                  onChange={(event) => set("intervalUnit", event.target.value as DurationUnit)}
                  className="w-24"
                >
                  <option value="ms">毫秒</option>
                  <option value="s">秒</option>
                  <option value="m">分钟</option>
                  <option value="h">小时</option>
                </Select>
                {intervalMs ? (
                  <span
                    className="self-center whitespace-nowrap font-mono text-[11px] text-muted"
                    title={`${intervalMs} ms`}
                  >
                    = {formatDuration(intervalMs)}
                  </span>
                ) : null}
              </div>
            </Field>
          )}

          {form.jobType === JOB_TYPE.manual && (
            <p className="rounded-lg border border-line bg-sunken/60 px-3 py-2 text-[11.5px] leading-relaxed text-muted">
              手动分派不会生成 <code className="font-mono">next_assign_ms</code>，调度器也不会扫描它；
              只有调用 <code className="font-mono">POST /jobx/job/trigger</code> 或点击「立即触发」才会执行。
            </p>
          )}

          <div className="grid grid-cols-2 gap-3">
            <Field
              label="分派提前量（秒）"
              hint={`留空则用调度器默认值；当前 ${
                form.assignLeadSec ? `${form.assignLeadSec} 秒` : "默认"
              }`}
            >
              <Input
                value={form.assignLeadSec}
                onChange={(event) => set("assignLeadSec", event.target.value)}
                inputMode="numeric"
                className="font-mono"
                placeholder="5"
              />
            </Field>

            <Field label="Stream 长度上限" hint="MAXLEN ~ N，留空用全局默认（10000）">
              <Input
                value={form.streamMaxLen}
                onChange={(event) => set("streamMaxLen", event.target.value)}
                inputMode="numeric"
                className="font-mono"
                placeholder="10000"
              />
            </Field>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <Field label="有效开始" hint="留空表示立即生效">
              <Input
                type="datetime-local"
                value={form.validBegin}
                onChange={(event) => set("validBegin", event.target.value)}
                className="font-mono text-[12.5px]"
              />
            </Field>
            <Field label="有效结束" error={showError("validRange")} hint="超出后不再分派">
              <Input
                type="datetime-local"
                value={form.validEnd}
                onChange={(event) => set("validEnd", event.target.value)}
                className="font-mono text-[12.5px]"
              />
            </Field>
          </div>

          {highFreqHint && (
            <div className="flex items-start gap-1.5 rounded-lg border border-violet/20 bg-violet-soft/70 px-3 py-2">
              <Sparkles className="mt-[1px] size-3 shrink-0 text-violet" />
              <p className="text-[11px] leading-relaxed text-violet">
                触发间隔小于 60 秒阈值，保存后服务端会把该计划标记为
                <span className="font-medium"> 高频任务</span>，下次分派按高频间隔推进。
              </p>
            </div>
          )}
        </section>

        <section className="space-y-3 border-t border-line pt-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Braces className="size-3.5 text-muted" />
              <h3 className="text-[12px] font-semibold tracking-[0.02em] text-ink-soft">执行参数</h3>
            </div>
            <div className="flex items-center gap-1.5">
              {paramsError ? (
                <span className="flex items-center gap-1 text-[10.5px] text-danger">
                  <AlertCircle className="size-3" />
                  JSON 非法
                </span>
              ) : (
                <span className="text-[10.5px] text-muted">JSON 对象，可留空</span>
              )}
              <button
                type="button"
                disabled={Boolean(paramsError) || !form.params.trim()}
                onClick={() => set("params", prettyJson(form.params))}
                className="h-6 rounded border border-line bg-surface px-1.5 text-[11px] text-ink-soft hover:border-line-strong disabled:opacity-40"
              >
                格式化
              </button>
            </div>
          </div>

          <Textarea
            value={form.params}
            onChange={(event) => set("params", event.target.value)}
            rows={6}
            spellCheck={false}
            placeholder={'{\n  "batchSize": 500\n}'}
            className={cn("font-mono text-[12px]", paramsError && "border-danger/50 focus:border-danger/60 focus:ring-danger/10")}
          />
          {paramsError && (
            <p className="font-mono text-[10.5px] leading-snug text-danger/90">{paramsError}</p>
          )}
        </section>

        {job && (
          <section className="space-y-2 border-t border-line pt-4">
            <div className="flex items-center justify-between rounded-lg border border-line bg-sunken/60 px-3 py-2">
              <div>
                <p className="text-[11px] text-muted">启用状态</p>
                <p className="text-[11.5px] text-ink-soft">
                  停用后调度器跳过该计划，执行历史仍保留
                </p>
              </div>
              <Switch
                checked={form.enabled}
                onChange={(value) => set("enabled", value)}
                label="启用计划"
              />
            </div>

            <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 pt-1 text-[11px]">
              <div className="flex justify-between gap-2">
                <dt className="text-muted">下次分派</dt>
                <dd className="tnum font-mono text-ink-soft">
                  {job.nextAssignMs ? formatCompact(job.nextAssignMs) : "—"}
                </dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-muted">高频标记</dt>
                <dd className="text-ink-soft">{job.highFreq ? "是" : "否"}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-muted">创建</dt>
                <dd className="tnum font-mono text-ink-soft">{formatCompact(job.createMs)}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-muted">更新</dt>
                <dd className="tnum font-mono text-ink-soft">{formatCompact(job.updateMs)}</dd>
              </div>
            </dl>
          </section>
        )}

        {!job && (
          <div className="flex items-center justify-between rounded-lg border border-line bg-sunken/60 px-3 py-2">
            <div>
              <p className="text-[11.5px] font-medium text-ink-soft">创建后立即启用</p>
              <p className="text-[10.5px] text-muted">关闭则先存为停用状态，确认无误后再开启</p>
            </div>
            <Switch checked={form.enabled} onChange={(value) => set("enabled", value)} label="启用计划" />
          </div>
        )}
      </div>
    </Drawer>
  );
}
