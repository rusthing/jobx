import { NavLink, useLocation } from "react-router-dom";
import { useIsFetching, useQueryClient } from "@tanstack/react-query";
import { CircleUser, Command, LogOut, RotateCw, Search } from "lucide-react";
import { cn } from "@/lib/utils";
import { formatTime } from "@/lib/time";
import { getOperatorId } from "@/lib/api/http";
import { logoutRemote, notifySessionExpired } from "@/lib/auth-core";
import { useNow } from "@/hooks/use-now";
import { Wordmark } from "./Logo";
import { NAV } from "./Sidebar";

const TITLES: Record<string, string> = {
  "/": "调度总览",
  "/jobs": "任务计划",
  "/groups": "计划分组",
  "/tasks": "执行记录",
  "/executors": "执行器",
};

/** 操作人只读展示 + 退出登录；身份来自统一会话，不再允许手改 X-User-Id */
function OperatorBadge() {
  const operatorId = getOperatorId();
  return (
    <div className="flex items-center gap-1">
      <span
        className="flex h-9 items-center gap-1.5 rounded-lg border border-line bg-surface pl-1.5 pr-2 text-[12px] font-medium text-ink-soft md:h-8"
        title="当前登录用户（来自统一登录态）"
      >
        <CircleUser className="size-4 text-muted" strokeWidth={1.7} />
        <span className="tnum">操作人 #{operatorId}</span>
      </span>
      <button
        type="button"
        onClick={() => {
          void logoutRemote().finally(notifySessionExpired);
        }}
        title="退出登录"
        aria-label="退出登录"
        className="flex h-9 items-center gap-1 rounded-lg border border-line bg-surface px-2 text-[12px] font-medium text-ink-soft transition-colors hover:border-line-strong hover:text-ink md:h-8"
      >
        <LogOut className="size-3.5 text-muted" strokeWidth={1.7} />
        <span className="hidden sm:inline">退出</span>
      </button>
    </div>
  );
}

export function Topbar({ onOpenPalette }: { onOpenPalette: () => void }) {
  const location = useLocation();
  const fetching = useIsFetching();
  const queryClient = useQueryClient();
  const now = useNow(1_000);

  const title = TITLES[location.pathname] ?? "调度总览";

  return (
    <>
    <header className="sticky top-0 z-30 flex h-14 items-center gap-3 border-b border-line bg-canvas/85 px-4 backdrop-blur-md lg:px-8">
      <div className="md:hidden">
        <Wordmark />
      </div>

      <div className="hidden items-center gap-2 text-[12px] text-muted md:flex">
        <span className="font-mono text-[11px] tracking-[0.04em] text-muted/80">JOBX</span>
        <span className="text-line-strong">/</span>
        <span className="text-[12.5px] font-medium text-ink">{title}</span>
      </div>

      <div className="ml-auto flex items-center gap-2">
        <span className="hidden items-center gap-1.5 text-[11px] text-muted lg:flex">
          <span className="relative flex size-1.5">
            <span className="live-dot relative size-1.5 rounded-full bg-success/80" />
          </span>
          <span className="tnum font-mono">{formatTime(now)}</span>
          <span className="text-muted/70">· 15s 自动刷新</span>
        </span>

        <button
          type="button"
          onClick={onOpenPalette}
          aria-label="搜索或跳转页面"
          className="flex h-9 items-center gap-2 rounded-lg border border-line bg-surface pl-2 pr-1.5 text-[12px] text-muted transition-colors hover:border-line-strong hover:text-ink-soft md:h-8"
        >
          <Search className="size-3.5" />
          <span className="hidden sm:inline">搜索 / 跳转</span>
          <kbd className="ml-1 hidden items-center gap-0.5 rounded border border-line bg-sunken px-1 py-[1px] font-mono text-[10px] text-muted sm:inline-flex">
            <Command className="size-2.5" />K
          </kbd>
        </button>

        <button
          type="button"
          onClick={() => {
            void queryClient.invalidateQueries();
          }}
          title="立即刷新"
          aria-label="立即刷新"
          className="flex size-9 items-center justify-center rounded-lg border border-line bg-surface text-muted transition-colors hover:border-line-strong hover:text-ink md:size-8"
        >
          <RotateCw className={cn("size-3.5", fetching > 0 && "animate-spin text-accent")} />
        </button>

        <OperatorBadge />
      </div>
    </header>

    {/* 移动端没有侧边栏，用一条可横向滚动的导航补上，触控高度 44px */}
    <nav
      aria-label="页面导航"
      className="sticky top-14 z-20 flex overflow-x-auto border-b border-line bg-canvas/95 px-2 backdrop-blur md:hidden"
    >
      {NAV.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={"end" in item ? item.end : false}
          className={({ isActive }) =>
            cn(
              "inline-flex min-h-11 shrink-0 items-center gap-1.5 whitespace-nowrap px-3 text-[12.5px] transition-colors",
              isActive
                ? "border-b-2 border-accent font-medium text-accent-strong"
                : "border-b-2 border-transparent text-ink-soft",
            )
          }
        >
          <item.icon className="size-4 text-muted" strokeWidth={1.8} />
          {item.label}
        </NavLink>
      ))}
    </nav>
    </>
  );
}
