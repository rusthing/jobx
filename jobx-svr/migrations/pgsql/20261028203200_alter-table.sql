/*==============================================================*/
/* Alter: jobx_job.task_expire_duration                         */
/*==============================================================*/

-- 计划级的「任务保留时长」：
--   该计划产生的 jobx_task 记录超过这个时长后，由 jobx-svr 的 jobx-task-cleaner
--   循环删除。基准是任务的创建时刻（create_ms），与任务是否执行完无关。
--
-- 为什么是计划级而不是全局：保留多久是业务判断。健康检查这种 60s 一次的计划
-- 每天长 1440 行、一年 50 万行，留 7 天足够；人工触发、低频的审计型计划可以留
-- 一年甚至永久。
--
-- 为什么默认 NULL：NULL = 永久保留、不清理。升级后不会因为「有了这个功能」
-- 就突然删掉任何历史数据，必须显式给计划设上保留时长才会开始清理。
--
-- 为什么是 VARCHAR 而不是 INT8 毫秒：与 jobx_job.interval_duration 保持一致的
-- 表示方式 —— 由 robotech 的 Duration 包装类型在 String <-> Duration 之间用
-- humantime 解析/格式化（如 "7days"、"30m"）。长度给 30 是因为 humantime 会把
-- 365 天格式化成 "11months 29days 23h 59m 59s" 这种多单位串。
alter table jobx_job
   add column if not exists task_expire_duration VARCHAR(30) null;

comment on column jobx_job.task_expire_duration is
'任务保留时长
该计划产生的任务记录超过这个时长后会被服务端自动清理
基准是任务的创建时刻（create_ms），与任务是否执行完无关
为空表示永久保留，不参与清理';
