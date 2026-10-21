import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import {
  Activity,
  ArrowRight,
  Boxes,
  CalendarClock,
  Database,
  LayoutDashboard,
  Plus,
  RotateCw,
  Search,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { getDataMode, setDataMode } from "@/lib/api";
import { useAllJobs } from "@/hooks/use-jobs";
import { useBodyScrollLock } from "@/hooks/use-body-scroll-lock";
import { useToast } from "@/components/ui/Toast";

interface Action {
  id: string;
  label: string;
  hint?: string;
  icon: typeof Search;
  keywords: string;
  run: () => void;
}

export function CommandPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const navigate = useNavigate();
  const toast = useToast();
  const queryClient = useQueryClient();
  const { data: jobs } = useAllJobs();

  const actions = useMemo<Action[]>(() => {
    const go = (to: string) => () => {
      navigate(to);
      onClose();
    };

    const base: Action[] = [
      { id: "overview", label: "调度总览", hint: "总览", icon: LayoutDashboard, keywords: "overview dashboard 总览 首页", run: go("/") },
      { id: "jobs", label: "任务计划", hint: "列表", icon: CalendarClock, keywords: "jobs 计划 任务 cron", run: go("/jobs") },
      { id: "new-job", label: "新建任务计划", hint: "动作", icon: Plus, keywords: "new create 新建 创建 计划", run: go("/jobs?new=1") },
      { id: "tasks", label: "执行记录", hint: "列表", icon: Activity, keywords: "tasks 执行 记录 日志", run: go("/tasks") },
      { id: "executors", label: "执行器", hint: "列表", icon: Boxes, keywords: "executor 执行器 worker", run: go("/executors") },
      {
        id: "refresh",
        label: "刷新全部数据",
        hint: "动作",
        icon: RotateCw,
        keywords: "refresh 刷新 重载",
        run: () => {
          void queryClient.invalidateQueries();
          toast.success("已触发刷新");
          onClose();
        },
      },
      {
        id: "mode",
        label: getDataMode() === "mock" ? "切换到实时接口" : "切换到演示数据",
        hint: "数据源",
        icon: Database,
        keywords: "mock live 数据源 切换 演示",
        run: () => {
          setDataMode(getDataMode() === "mock" ? "live" : "mock");
          onClose();
        },
      },
    ];

    const jobActions: Action[] = (jobs ?? []).map((job) => ({
      id: `job-${job.id}`,
      label: job.name,
      hint: job.executorCode,
      icon: CalendarClock,
      keywords: `${job.name} ${job.executorCode} ${job.cron ?? ""}`,
      run: go(`/jobs?job=${job.id}`),
    }));

    return [...base, ...jobActions];
  }, [jobs, navigate, onClose, queryClient, toast]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return actions.slice(0, 8);
    return actions
      .filter((action) => `${action.label} ${action.keywords}`.toLowerCase().includes(needle))
      .slice(0, 9);
  }, [actions, query]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setCursor(0);
    const timer = window.setTimeout(() => inputRef.current?.focus(), 30);
    return () => window.clearTimeout(timer);
  }, [open]);

  useEffect(() => {
    setCursor(0);
  }, [query]);

  useBodyScrollLock(open);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const runAt = (index: number) => {
    const action = filtered[index];
    if (action) action.run();
  };

  return createPortal(
    <div className="fixed inset-0 z-[70] flex items-start justify-center p-4 pt-[12vh]">
      <div className="absolute inset-0 animate-fade bg-ink/25" onClick={onClose} aria-hidden />
      <div
        role="dialog"
        aria-modal="true"
        aria-label="命令面板"
        className="relative w-full max-w-[560px] animate-rise overflow-hidden rounded-[14px] border border-line bg-surface shadow-float"
      >
        <div className="flex items-center gap-2.5 border-b border-line px-3.5">
          <Search className="size-4 text-muted" />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setCursor((current) => Math.min(filtered.length - 1, current + 1));
              } else if (event.key === "ArrowUp") {
                event.preventDefault();
                setCursor((current) => Math.max(0, current - 1));
              } else if (event.key === "Enter") {
                event.preventDefault();
                runAt(cursor);
              }
            }}
            placeholder="跳转页面、打开计划、切换数据源…"
            className="h-11 flex-1 bg-transparent text-[13.5px] text-ink placeholder:text-muted/70 focus:outline-none"
          />
          <kbd className="rounded border border-line bg-sunken px-1.5 py-0.5 font-mono text-[10px] text-muted">
            ESC
          </kbd>
        </div>

        <div className="max-h-[360px] overflow-y-auto p-1.5">
          {filtered.length === 0 ? (
            <p className="px-3 py-8 text-center text-[12.5px] text-muted">没有匹配项</p>
          ) : (
            filtered.map((action, index) => (
              <button
                key={action.id}
                type="button"
                onMouseEnter={() => setCursor(index)}
                onClick={() => runAt(index)}
                className={cn(
                  "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left transition-colors",
                  index === cursor ? "bg-accent-soft text-accent-strong" : "text-ink-soft",
                )}
              >
                <action.icon
                  className={cn("size-4 shrink-0", index === cursor ? "text-accent" : "text-muted")}
                  strokeWidth={1.8}
                />
                <span className="min-w-0 flex-1 truncate text-[13px]">{action.label}</span>
                {action.hint && (
                  <span className="shrink-0 font-mono text-[10.5px] text-muted">{action.hint}</span>
                )}
                {index === cursor && <ArrowRight className="size-3 text-accent/70" />}
              </button>
            ))
          )}
        </div>

        <div className="flex items-center gap-3 border-t border-line bg-sunken/60 px-3.5 py-1.5 text-[10.5px] text-muted">
          <span>↑↓ 选择</span>
          <span>↵ 执行</span>
          <span className="ml-auto font-mono">JobX</span>
        </div>
      </div>
    </div>,
    document.body,
  );
}
