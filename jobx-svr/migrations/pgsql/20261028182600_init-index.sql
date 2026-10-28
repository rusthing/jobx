/*==============================================================*/
/* 初始化索引                                                    */
/*==============================================================*/
--
-- ## 这个脚本放什么
--
-- 只放**逻辑模型（ldm / pdm）表达不了的索引**：部分索引（带 where 谓词）、
-- 表达式索引、以及其它 PD 的 Index 定义覆盖不到的形态。
--
-- ## 为什么不写进 model/create-table-pgsql.sql
--
-- 那份文件是 PowerDesigner 的导出产物，手改会在下次「重新导出 DDL」时被静默覆盖，
-- 于是索引在开发库上有、在新环境上莫名其妙没有。能在 pdm 里定义的索引一律写进模型，
-- 由导出带出；模型表达不了的收敛到这里，按迁移版本管理，可追溯、可重放。
--
-- ## 约定
--
-- 每条索引写 `if not exists`，保证在已手工建过索引的环境上也能重复执行。

/*--------------------------------------------------------------*/
/* jobx_task：僵尸任务巡检的部分索引                              */
/*--------------------------------------------------------------*/
-- jobx-task-reaper 的三条 UPDATE 共用 `status = 0 and exec_end_ms is null`
-- 两个谓词，分别再按 exec_start_ms / scheduled_exec_start_ms 卡超时阈值：
--   规则 1（已分派未开始）：exec_start_ms is null and scheduled_exec_start_ms < now - grace
--   规则 2/3（执行超时，普通/高频）：exec_start_ms < now - timeout
-- 用部分索引把扫描范围压到「仍在运行」的行（健康状态下只有个位数），
-- 避免全表扫描随任务历史线性变慢。
--
-- 列顺序按选择性由强到弱：exec_start_ms 服务于两档超时规则（规则 2/3），
-- scheduled_exec_start_ms 服务于规则 1（其 exec_start_ms 恒为 null，落在索引最左端）。
create index if not exists idx_jobx_task_running on jobx_task (
   exec_start_ms,
   scheduled_exec_start_ms
)
where status = 0 and exec_end_ms is null;
