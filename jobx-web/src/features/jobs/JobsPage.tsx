import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  ArrowDownUp,
  CircleSlash,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Search,
  Trash2,
} from "lucide-react";
import { PageHeader } from "@/components/page";
import { Panel, PanelState, Skeleton } from "@/components/ui/Panel";
import { Button, IconButton } from "@/components/ui/Button";
import { Input, Select } from "@/components/ui/Field";
import { Segmented, Switch } from "@/components/ui/Switch";
import { Badge } from "@/components/ui/Badge";
import { CopyButton } from "@/components/ui/CopyButton";
import { ConfirmDialog } from "@/components/ui/Drawer";
import { useToast } from "@/components/ui/Toast";
import { ExecutorChip, HighFreqBadge, JobTypeBadge, ScheduleText } from "@/components/domain";
import {
  useAllJobs,
  useJobDetail,
  useJobMutations,
  useJobsPage,
} from "@/hooks/use-jobs";
import { useDebounced } from "@/hooks/use-debounce";
import { useNow } from "@/hooks/use-now";
import {
  JOB_TYPE,
  JOB_TYPE_META,
  setDataMode,
  type Job,
  type JobType,
} from "@/lib/api";
import { formatCompact, formatRelative } from "@/lib/time";
import { cn } from "@/lib/utils";
import { JobFormDrawer } from "./JobFormDrawer";

type Editor = { mode: "new" } | { mode: "edit"; job: Job } | null;
type EnabledFilter = "all" | "enabled" | "disabled";

export function JobsPage() {
  const toast = useToast();
  const now = useNow(1_000);
  const [searchParams, setSearchParams] = useSearchParams();

  const [keywordInput, setKeywordInput] = useState("");
  const keyword = useDebounced(keywordInput, 300);
  const [type, setType] = useState<JobType | "all">("all");
  const [enabled, setEnabled] = useState<EnabledFilter>("all");
  const [executor, setExecutor] = useState(() => searchParams.get("executor") ?? "all");
  const [page, setPage] = useState(1);
  const [sortNext, setSortNext] = useState(false);

  const [editor, setEditor] = useState<Editor>(null);
  const [deleting, setDeleting] = useState<Job | null>(null);

  const query = useMemo(
    () => ({
      keyword: keyword || undefined,
      jobType: type === "all" ? undefined : type,
      enabled: enabled === "all" ? undefined : enabled === "enabled",
      executorCode: executor === "all" ? undefined : executor,
      page,
      size: 20,
      orderBy: sortNext ? "next_assign_ms:asc" : "update_ms:desc",
    }),
    [keyword, type, enabled, executor, page, sortNext],
  );

  const { data, isLoading, isFetching, error, refetch } = useJobsPage(query);
  const allJobs = useAllJobs();
  const { create, update, remove, trigger, setEnabled: toggleEnabledMutation } = useJobMutations();

  const urlJobId = searchParams.get("job");
  const urlNew = searchParams.get("new");
  const detail = useJobDetail(urlJobId);

  // 深链：/jobs?new=1 打开新建；/jobs?job=<id> 打开编辑
  useEffect(() => {
    if (urlNew === "1") {
      setEditor({ mode: "new" });
      setSearchParams({}, { replace: true });
    }
  }, [urlNew, setSearchParams]);

  useEffect(() => {
    if (searchParams.has("executor")) setSearchParams({}, { replace: true });
  }, [searchParams, setSearchParams]);

  useEffect(() => {
    if (urlJobId && detail.data) {
      setEditor({ mode: "edit", job: detail.data });
      setSearchParams({}, { replace: true });
    }
  }, [urlJobId, detail.data, setSearchParams]);

  const executors = useMemo(
    () => [...new Set((allJobs.data ?? []).map((job) => job.executorCode))].sort(),
    [allJobs.data],
  );

  const total = Number(data?.total ?? 0);
  const pageNum = Number(data?.pageNum ?? 1);
  const pageSize = 20;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  const resetPage = () => setPage(1);

  const openNew = () => setEditor({ mode: "new" });
  const openEdit = (job: Job) => setEditor({ mode: "edit", job });

  const handleSubmit = (input: Parameters<typeof create.mutate>[0]) => {
    const isEdit = "id" in input && Boolean(input.id);
    const mutation = isEdit ? update : create;
    mutation.mutate(input, {
      onSuccess: (saved) => {
        toast.success(isEdit ? "计划已更新" : "计划已创建", `${saved.name} · #${saved.id}`);
        setEditor(null);
      },
      onError: (mutationError) => toast.error("保存失败", mutationError.message),
    });
  };

  const handleTrigger = (job: Job) => {
    trigger.mutate(job.id, {
      onSuccess: (task) => toast.success(`已分派「${job.name}」`, `执行记录 #${task.id}`),
      onError: (mutationError) => toast.error("触发失败", mutationError.message),
    });
  };

  const toggleEnabled = (job: Job) => {
    toggleEnabledMutation.mutate(job, {
      onSuccess: (saved) =>
        toast.success(saved.enabled ? "已启用" : "已停用", saved.name),
      onError: (mutationError) => toast.error("切换失败", mutationError.message),
    });
  };

  const confirmDelete = () => {
    if (!deleting) return;
    remove.mutate(deleting.id, {
      onSuccess: () => {
        toast.success("计划已删除", deleting.name);
        setDeleting(null);
      },
      onError: (mutationError) => {
        toast.error("删除失败", mutationError.message);
        setDeleting(null);
      },
    });
  };

  const filtered = keyword || type !== "all" || enabled !== "all" || executor !== "all";

  /*
   * JobFormDrawer 是 memo 化的（页面上有一个每秒 tick 的 now），所以传给它的回调必须引用
   * 稳定。`create` / `update` / `trigger` 这些 mutation 对象每次渲染都是新引用，直接
   * useCallback 会白搭 —— 用 ref 转发到最新的那个处理函数。
   */
  const editorRef = useRef<Editor>(null);
  const submitRef = useRef(handleSubmit);
  const triggerRef = useRef(handleTrigger);
  // 渲染期不写 ref，提交后再同步，避免并发渲染下读到中间态
  useEffect(() => {
    editorRef.current = editor;
    submitRef.current = handleSubmit;
    triggerRef.current = handleTrigger;
  });

  const closeEditor = useCallback(() => setEditor(null), []);
  const submitEditor = useCallback(
    (input: Parameters<typeof handleSubmit>[0]) => submitRef.current(input),
    [],
  );
  const isEditMode = editor?.mode === "edit";
  const triggerEditor = useMemo(() => {
    if (!isEditMode) return undefined;
    return () => {
      const current = editorRef.current;
      if (current?.mode === "edit") triggerRef.current(current.job);
    };
  }, [isEditMode]);

  return (
    <>
      <PageHeader
        title="任务计划"
        subtitle={
          <>
            每条计划绑定一个执行器编码与一种调度策略 · 共{" "}
            <span className="tnum font-mono text-ink-soft">{total}</span> 条
            {filtered ? "（已过滤）" : ""}
          </>
        }
        actions={
          <>
            <Button
              size="sm"
              variant="ghost"
              icon={<RefreshCw className={cn("size-3.5", isFetching && "animate-spin")} />}
              onClick={() => refetch()}
            >
              刷新
            </Button>
            <Button size="sm" variant="primary" icon={<Plus className="size-3.5" />} onClick={openNew}>
              新建计划
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
                resetPage();
              }}
              placeholder="搜索名称、执行器、备注、cron…"
              className="h-8 pl-8 text-[12.5px]"
            />
          </div>

          <Segmented
            size="sm"
            value={enabled}
            onChange={(value) => {
              setEnabled(value);
              resetPage();
            }}
            options={[
              { value: "all", label: "全部" },
              { value: "enabled", label: "启用" },
              { value: "disabled", label: "停用" },
            ]}
          />

          <Select
            value={type}
            onChange={(event) => {
              setType(event.target.value === "all" ? "all" : (Number(event.target.value) as JobType));
              resetPage();
            }}
            className="h-8 w-[118px] text-[12.5px]"
          >
            <option value="all">全部类型</option>
            {Object.entries(JOB_TYPE_META).map(([value, meta]) => (
              <option key={value} value={value}>
                {meta.label}
              </option>
            ))}
          </Select>

          <Select
            value={executor}
            onChange={(event) => {
              setExecutor(event.target.value);
              resetPage();
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

          <button
            type="button"
            onClick={() => {
              setSortNext((current) => !current);
              resetPage();
            }}
            className={cn(
              "ml-auto flex h-8 items-center gap-1.5 rounded-lg border px-2 text-[12px] transition-colors",
              sortNext
                ? "border-accent/30 bg-accent-soft text-accent-strong"
                : "border-line bg-surface text-muted hover:text-ink",
            )}
            title="按下次分派时间排序"
          >
            <ArrowDownUp className="size-3" />
            {sortNext ? "按分派时间" : "按更新时间"}
          </button>
        </div>

        {error ? (
          <PanelState
            icon={<CircleSlash className="size-4" />}
            title="计划列表加载失败"
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
            {Array.from({ length: 6 }).map((_, index) => (
              <Skeleton key={index} className="h-12" />
            ))}
          </div>
        ) : !data || data.list.length === 0 ? (
          <PanelState
            icon={<Search className="size-4" />}
            title={filtered ? "没有符合条件的计划" : "还没有任务计划"}
            description={
              filtered
                ? "换个关键词或清空筛选条件再试。"
                : "创建第一条计划：选定执行器编码，再用 cron 或固定间隔描述它该多久跑一次。"
            }
            action={
              filtered ? (
                <Button
                  size="sm"
                  onClick={() => {
                    setKeywordInput("");
                    setType("all");
                    setEnabled("all");
                    setExecutor("all");
                    setPage(1);
                  }}
                >
                  清空筛选
                </Button>
              ) : (
                <Button size="sm" variant="primary" icon={<Plus className="size-3.5" />} onClick={openNew}>
                  新建计划
                </Button>
              )
            }
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[960px] border-collapse text-left">
              <thead>
                <tr className="border-b border-line bg-sunken/60 text-[11px] text-muted">
                  <th className="px-4 py-2 font-medium">计划</th>
                  <th className="px-3 py-2 font-medium">执行器</th>
                  <th className="px-3 py-2 font-medium">调度</th>
                  <th className="px-3 py-2 font-medium">状态</th>
                  <th className="px-3 py-2 text-right font-medium">下次分派</th>
                  <th className="px-4 py-2 text-right font-medium">操作</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-line/70">
                {data.list.map((job) => {
                  const next = job.nextAssignMs;
                  const disabled = !job.enabled;
                  return (
                    <tr
                      key={job.id}
                      className={cn(
                        "group transition-colors hover:bg-sunken/50",
                        disabled && "opacity-[0.72]",
                      )}
                    >
                      <td className="max-w-[300px] px-4 py-2.5">
                        <div className="flex items-center gap-1.5">
                          <button
                            type="button"
                            onClick={() => openEdit(job)}
                            className="truncate text-[13px] font-medium text-ink hover:text-accent"
                          >
                            {job.name}
                          </button>
                          {job.highFreq && <HighFreqBadge />}
                        </div>
                        <div className="mt-0.5 flex items-center gap-1">
                          <span className="font-mono text-[10px] text-muted/80">#{job.id}</span>
                          <CopyButton value={job.id} label="复制 ID" />
                          {job.remark && (
                            <span className="truncate text-[10.5px] text-muted/80">
                              · {job.remark}
                            </span>
                          )}
                        </div>
                      </td>

                      <td className="px-3 py-2.5">
                        <ExecutorChip code={job.executorCode} />
                      </td>

                      <td className="max-w-[220px] px-3 py-2.5">
                        <div className="flex items-start gap-2">
                          <JobTypeBadge type={job.jobType} short />
                          <ScheduleText job={job} dense />
                        </div>
                      </td>

                      <td className="px-3 py-2.5">
                        <div className="flex items-center gap-2">
                          <Switch
                            size="sm"
                            checked={job.enabled}
                            onChange={() => toggleEnabled(job)}
                            disabled={toggleEnabledMutation.isPending}
                            label={`${job.enabled ? "停用" : "启用"} ${job.name}`}
                          />
                          <span className="text-[11px] text-muted">
                            {job.enabled ? "启用" : "停用"}
                          </span>
                        </div>
                      </td>

                      <td className="px-3 py-2.5 text-right">
                        {next ? (
                          <>
                            <div className="tnum font-mono text-[12px] text-ink">
                              {formatRelative(next, now)}
                            </div>
                            <div className="text-[10px] text-muted">{formatCompact(next)}</div>
                          </>
                        ) : (
                          <span className="text-[11.5px] text-muted">
                            {job.jobType === JOB_TYPE.manual ? "手动" : "—"}
                          </span>
                        )}
                      </td>

                      <td className="px-4 py-2.5">
                        <div className="flex items-center justify-end gap-0.5 opacity-70 transition-opacity group-hover:opacity-100">
                          <IconButton
                            label="立即触发"
                            disabled={!job.enabled || trigger.isPending}
                            onClick={() => handleTrigger(job)}
                          >
                            <Play className="size-3.5" />
                          </IconButton>
                          <IconButton label="编辑" onClick={() => openEdit(job)}>
                            <Pencil className="size-3.5" />
                          </IconButton>
                          <IconButton label="删除" tone="danger" onClick={() => setDeleting(job)}>
                            <Trash2 className="size-3.5" />
                          </IconButton>
                        </div>
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
              第 {(pageNum - 1) * pageSize + 1}–{Math.min(pageNum * pageSize, total)} 条 / 共 {total}{" "}
              条
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

      <p className="mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 text-[10.5px] leading-relaxed text-muted/80">
        <Badge tone="muted" mono>
          next_assign_ms
        </Badge>
        由调度器按 cron / 间隔与有效窗口计算；停用或手动分派的计划该字段为空。
      </p>

      <JobFormDrawer
        open={editor !== null}
        job={editor?.mode === "edit" ? editor.job : null}
        executors={executors}
        submitting={create.isPending || update.isPending}
        onClose={closeEditor}
        onSubmit={submitEditor}
        onTrigger={triggerEditor}
      />

      <ConfirmDialog
        open={deleting !== null}
        title={`删除计划「${deleting?.name ?? ""}」？`}
        description={
          <>
            服务端外键为 <code className="font-mono text-[11px]">on delete restrict</code>，
            若该计划已产生执行记录，删除会被拒绝 —— 那种情况下更合适的做法是停用。
          </>
        }
        confirmText="删除"
        loading={remove.isPending}
        onConfirm={confirmDelete}
        onCancel={() => setDeleting(null)}
      />
    </>
  );
}
