import { useEffect } from "react";
import { QueryClient, QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { BrowserRouter, HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "@/components/layout/AppShell";
import { ToastProvider } from "@/components/ui/Toast";
import { subscribeDataMode } from "@/lib/api";
import { OverviewPage } from "@/features/overview/OverviewPage";
import { JobsPage } from "@/features/jobs/JobsPage";
import { TasksPage } from "@/features/tasks/TasksPage";
import { ExecutorsPage } from "@/features/executors/ExecutorsPage";

/**
 * 静态服务器不方便配 SPA 回退（`try_files $uri /jobx-web/index.html`）时，
 * 用 `VITE_ROUTER=hash` 退化成 hash 路由，刷新不会 404 —— 路径永远停在目录首页，
 * 路由挂在 `#` 后面。
 */
const USE_HASH = import.meta.env.VITE_ROUTER === "hash";

/**
 * 部署在子路径时（PUBLIC_BASE=/jobx-web/），路由必须跟着挂同样的前缀：
 * 否则 `/jobx-web/jobs` 会被当成外部路径，所有 Link 都指回站点根。
 * `BASE_URL` 由 Vite 按 `base` 注入，默认 `/`，去掉末尾斜杠后 `""` 即站点根。
 *
 * hash 模式例外：react-router 会拿 basename 去匹配 hash 里的路径，而 hash 里只有 `/jobs`
 * 没有 `/jobx-web`，带上去就整页空白，所以这里必须留空。
 */
const BASENAME = USE_HASH ? "" : import.meta.env.BASE_URL.replace(/\/+$/, "");

const Router = USE_HASH ? HashRouter : BrowserRouter;

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 5_000,
      refetchOnWindowFocus: true,
    },
  },
});

/** 切换数据源后必须清缓存，否则演示数据会混进实时视图 */
function DataModeBridge() {
  const client = useQueryClient();
  useEffect(
    () =>
      subscribeDataMode(() => {
        client.clear();
      }),
    [client],
  );
  return null;
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <Router basename={BASENAME || undefined}>
          <DataModeBridge />
          <Routes>
            <Route element={<AppShell />}>
              <Route index element={<OverviewPage />} />
              <Route path="jobs" element={<JobsPage />} />
              <Route path="tasks" element={<TasksPage />} />
              <Route path="executors" element={<ExecutorsPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Route>
          </Routes>
        </Router>
      </ToastProvider>
    </QueryClientProvider>
  );
}
