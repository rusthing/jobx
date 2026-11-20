/**
 * 联邦入口：给 uiam 桌面用的页面级模块。
 *
 * jobx 独立入口把 Provider / Router 都放在 App 里；这里拆开复用：自带一套
 * QueryClientProvider + ToastProvider + MemoryRouter，路由表 AppRoutes 共用。
 * MemoryRouter 把路由关在窗口内部，不碰宿主地址栏。
 *
 * 主题通过 props 传入（React 单例后 props 是最可靠通道），宿主从 light 切到
 * dark 时这里直接重渲染根节点的 data-theme。
 */
import { useEffect } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, useLocation } from "react-router-dom";
import { AppRoutes } from "@/App";
import { ToastProvider } from "@/components/ui/Toast";
import "@/styles.css";
import "./federation.css";

export interface JobxFederatedProps {
  theme?: "light" | "dark";
  initialPath?: string;
  onRoute?: (path: string, title: string) => void;
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 5_000, refetchOnWindowFocus: true },
  },
});

/** 与 AppRoutes 的相对路径一一对应；未知路径回退到「调度总览」。 */
const PATH_TITLES: Record<string, string> = {
  "/": "调度总览",
  "/jobs": "执行实例",
  "/groups": "任务分组",
  "/tasks": "任务计划",
  "/executors": "执行器",
};

function titleOf(pathname: string): string {
  return PATH_TITLES[pathname] ?? "调度总览";
}

function RouteReporter({ onRoute }: { onRoute?: JobxFederatedProps["onRoute"] }) {
  const { pathname } = useLocation();
  useEffect(() => {
    if (pathname === "/login") return; // 登录页不改窗口标题
    onRoute?.(pathname, titleOf(pathname));
  }, [pathname, onRoute]);
  return null;
}

export default function JobxFederatedApp({
  theme = "light",
  initialPath = "/",
  onRoute,
}: JobxFederatedProps) {
  const entry = initialPath.startsWith("/") ? initialPath : "/";
  return (
    <div className="jobx-federated" data-theme={theme}>
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <MemoryRouter initialEntries={[entry]}>
            <RouteReporter onRoute={onRoute} />
            <AppRoutes />
          </MemoryRouter>
        </ToastProvider>
      </QueryClientProvider>
    </div>
  );
}
