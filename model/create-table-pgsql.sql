/*==============================================================*/
/* DBMS name:      PostgreSQL 9.x                               */
/* Created on:     2026/10/13 15:04:40                          */
/*==============================================================*/


/*==============================================================*/
/* Table: jobx_job                                              */
/*==============================================================*/
create table jobx_job (
   id                   INT8                 not null,
   name                 VARCHAR(50)          not null,
   params               VARCHAR(800)         null,
   job_type             INT2                 not null,
   high_freq            BOOL                 null default false,
   cron                 VARCHAR(30)          null,
   interval_duration    VARCHAR(10)          null,
   valid_begin_ms       INT8                 null,
   valid_end_ms         INT8                 null,
   executor_code        VARCHAR(50)          not null,
   assign_lead_ms       INT4                 null,
   next_assign_ms       INT8                 null,
   remark               VARCHAR(50)          null,
   enabled              BOOL                 not null default true,
   creater_id           INT8                 not null,
   create_ms            INT8                 not null,
   updater_id           INT8                 not null,
   update_ms            INT8                 not null,
   constraint PK_JOBX_JOB primary key (id),
   constraint AK_NAME_JOBX_JOB unique (name)
);

comment on table jobx_job is
'任务计划';

comment on column jobx_job.id is
'ID';

comment on column jobx_job.name is
'名称';

comment on column jobx_job.params is
'参数';

comment on column jobx_job.job_type is
'计划类型
0: 手动分派
1: cron表达式
2: 固定延迟
3: 固定频率';

comment on column jobx_job.high_freq is
'是否高频任务
这个由计划类型、cron表达式、固定时间间隔与配置中的高频任务时长阈值计算得出';

comment on column jobx_job.cron is
'cron表达式
当 job_type 为 cron表达式时必填';

comment on column jobx_job.interval_duration is
'固定间隔时间
当job_type为固定延迟或固定频率时必填';

comment on column jobx_job.valid_begin_ms is
'有效开始时间戳';

comment on column jobx_job.valid_end_ms is
'有效结束时间戳';

comment on column jobx_job.executor_code is
'执行器编码
发布任务消息时的key将以此编码结尾，只有相同编码的执行器才订阅此key';

comment on column jobx_job.assign_lead_ms is
'分派提前毫秒数';

comment on column jobx_job.next_assign_ms is
'下次分派时间戳';

comment on column jobx_job.remark is
'备注';

comment on column jobx_job.enabled is
'启用';

comment on column jobx_job.creater_id is
'创建者ID';

comment on column jobx_job.create_ms is
'创建时间戳';

comment on column jobx_job.updater_id is
'更新者ID';

comment on column jobx_job.update_ms is
'更新时间戳';

/*==============================================================*/
/* Index: jobx_job_PK                                           */
/*==============================================================*/
create unique index jobx_job_PK on jobx_job (
id
);

/*==============================================================*/
/* Table: jobx_task                                             */
/*==============================================================*/
create table jobx_task (
   id                   INT8                 not null,
   task_type            INT2                 not null,
   job_id               INT8                 null,
   status               INT2                 not null default 0,
   assign_ms            INT8                 not null,
   scheduled_exec_start_ms INT8                 null,
   exec_detail          VARCHAR(800)         null,
   exec_start_ms        INT8                 null,
   exec_end_ms          INT8                 null,
   executor_code        VARCHAR(50)          not null,
   executor_instance    VARCHAR(100)         null,
   exec_params          VARCHAR(800)         null,
   job_type             INT2                 not null,
   high_freq            BOOL                 null default false,
   cron                 VARCHAR(30)          null,
   interval_duration    VARCHAR(10)          null,
   valid_begin_ms       INT8                 null,
   valid_end_ms         INT8                 null,
   assign_lead_ms       INT4                 null,
   creater_id           INT8                 not null,
   create_ms            INT8                 not null,
   updater_id           INT8                 not null,
   update_ms            INT8                 not null,
   constraint PK_JOBX_TASK primary key (id),
   constraint AK_JOB_ID_AND_SCHEDULED_EXEC_STAR_JOBX_TASK unique (job_id, scheduled_exec_start_ms)
);

comment on table jobx_task is
'任务记录';

comment on column jobx_task.id is
'ID';

comment on column jobx_task.task_type is
'任务类型
1: 立即执行
2: 计划执行';

comment on column jobx_task.job_id is
'任务计划ID';

comment on column jobx_task.status is
'任务状态
0: 运行中
1: 成功
2: 失败
';

comment on column jobx_task.assign_ms is
'分派时间戳';

comment on column jobx_task.scheduled_exec_start_ms is
'预定开始执行时间戳';

comment on column jobx_task.exec_detail is
'执行详情';

comment on column jobx_task.exec_start_ms is
'开始执行时间戳';

comment on column jobx_task.exec_end_ms is
'结束执行时间戳';

comment on column jobx_task.executor_code is
'执行器编码';

comment on column jobx_task.executor_instance is
'执行器实例';

comment on column jobx_task.exec_params is
'执行参数';

comment on column jobx_task.job_type is
'计划类型
0: 手动分派
1: cron表达式
2: 固定延迟
3: 固定频率';

comment on column jobx_task.high_freq is
'是否高频任务';

comment on column jobx_task.cron is
'cron表达式';

comment on column jobx_task.interval_duration is
'固定间隔时间';

comment on column jobx_task.valid_begin_ms is
'有效开始时间戳';

comment on column jobx_task.valid_end_ms is
'有效结束时间戳';

comment on column jobx_task.assign_lead_ms is
'分派提前毫秒数';

comment on column jobx_task.creater_id is
'创建者ID';

comment on column jobx_task.create_ms is
'创建时间戳';

comment on column jobx_task.updater_id is
'更新者ID';

comment on column jobx_task.update_ms is
'更新时间戳';

/*==============================================================*/
/* Index: jobx_task_PK                                          */
/*==============================================================*/
create unique index jobx_task_PK on jobx_task (
id
);

/*==============================================================*/
/* Index: Relationship_1_FK                                     */
/*==============================================================*/
create  index Relationship_1_FK on jobx_task (
job_id
);

alter table jobx_task
   add constraint fk_job_id__from__jobx_job foreign key (job_id)
      references jobx_job (id)
      on delete restrict on update restrict;

