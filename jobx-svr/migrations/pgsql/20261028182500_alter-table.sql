/*==============================================================*/
/* Alter: jobx_task.exec_deadline_ms                            */
/*==============================================================*/

-- 高频任务「本段执行截止时刻」：
--   min(计划有效结束时间, 预定开始执行时刻 + 高频分派间隔)
-- 由 jobx-svr 的 JobxTaskSvc::before_add 在 take 建单时算出并落库，执行器据此结束
-- 高频循环。此前执行器只能拿计划整体的 valid_end_ms 当终点，为空（很常见）时会
-- 退化成 u64::MAX，任务永不结束、永远挂在「运行中」。
--
-- 用 if not exists：部分环境（含本地开发库）已通过手工 DDL 加过该列，
-- 迁移需要能重复执行而不报错。
alter table jobx_task
   add column if not exists exec_deadline_ms INT8 null;

comment on column jobx_task.exec_deadline_ms is
'执行截止时间戳
高频任务本次分派的执行时段终点（min(计划有效结束时间, 预定开始执行时间 + 高频分派间隔)）；为空表示不限，仅对高频任务有意义';
