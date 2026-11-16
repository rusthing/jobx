CREATE INDEX idx_jobx_task_job_create
    ON jobx_task USING btree (job_id, create_ms);

-- 部分索引：只覆盖「已开始但没结束」的运行中任务，供巡检与执行器侧查询使用
CREATE INDEX idx_jobx_task_running
    ON jobx_task USING btree (exec_start_ms, scheduled_exec_start_ms)
    WHERE ((status = 0) AND (exec_end_ms IS NULL));

-- 总览统计的 24 小时窗口按 assign_ms 过滤（select ... where assign_ms >= $1）
CREATE INDEX idx_jobx_task_assign_ms
    ON jobx_task USING btree (assign_ms);
