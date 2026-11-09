CREATE INDEX idx_jobx_task_job_create
    ON jobx_task USING btree (job_id, create_ms);

CREATE INDEX idx_jobx_task_running
    ON jobx_task USING btree (exec_start_ms, scheduled_exec_start_ms)
    WHERE ((status = 0) AND (exec_end_ms IS NULL));