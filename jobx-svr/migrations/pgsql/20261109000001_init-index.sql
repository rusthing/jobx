-- jobx 数据库迁移基线：init-index（普通索引）
--
-- 约束型索引随建表语句定义；此处只放独立 CREATE INDEX（含部分/表达式索引）。

CREATE INDEX idx_jobx_task_job_create ON public.jobx_task USING btree (job_id, create_ms);

CREATE INDEX idx_jobx_task_running ON public.jobx_task USING btree (exec_start_ms, scheduled_exec_start_ms) WHERE ((status = 0) AND (exec_end_ms IS NULL));

CREATE INDEX relationship_1_fk ON public.jobx_task USING btree (job_id);

CREATE INDEX relationship_2_fk ON public.jobx_job USING btree (group_id);
