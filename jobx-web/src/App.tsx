import { useEffect } from "react";
import { QueryClient, QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "@/components/layout/AppShell";
import { ToastProvider } from "@/components/ui/Toast";
import { subscribeDataMode } from "@/lib/api";
import { OverviewPage } from "@/features/overview/OverviewPage";
import { JobsPage } from "@/features/jobs/JobsPage";
import { TasksPage } from "@/features/tasks/TasksPage";
import { ExecutorsPage } from "@/features/executors/ExecutorsPage";

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
        <BrowserRouter>
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
        </BrowserRouter>
      </ToastProvider>
    </QueryClientProvider>
  );
}
