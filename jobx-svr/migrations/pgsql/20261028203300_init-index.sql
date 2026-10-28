/*==============================================================*/
/* 初始化索引                                                    */
/*==============================================================*/
--
-- 本脚本只放**逻辑模型（ldm / pdm）表达不了、或模型里没有对应 Index 对象**的索引。
-- `model/create-table-pgsql.sql` 是 PowerDesigner 的导出产物，手改会在下次
-- 「重新导出 DDL」时被静默覆盖，所以按迁移版本管理，可追溯、可重放。
--
-- 每条索引写 `if not exists`，保证在已手工建过索引的环境上也能重复执行。

/*--------------------------------------------------------------*/
/* jobx_task：过期任务清理的索引                                  */
/*--------------------------------------------------------------*/
-- jobx-task-cleaner 每轮对每个「设了 task_expire_duration 的计划」执行：
--   delete from jobx_task where id in (
--     select id from jobx_task where job_id = $1 and create_ms < $2 limit $3
--   )
-- 已有索引只有主键与 job_id 的外键索引：只按 job_id 扫会把该计划的**全部**历史
-- 都读一遍再过滤 create_ms，任务多的计划每批都是全量扫描。补一个 (job_id, create_ms)
-- 复合索引，让子查询变成一次范围扫描，且顺序天然就是「最老的在前」。
create index if not exists idx_jobx_task_job_create on jobx_task (
   job_id,
   create_ms
);
