/*==============================================================*/
/* DBMS name:      PostgreSQL 9.x                               */
/* Created on:     2026/9/11 14:51:47                           */
/*==============================================================*/


/*==============================================================*/
/* Table: jobx_job                                              */
/*==============================================================*/
create table jobx_job (
   id                   INT8                 not null,
   code                 VARCHAR(50)          not null,
   name                 VARCHAR(50)          not null,
   params               VARCHAR(800)         null,
   job_type             INT2                 not null,
   high_freq            BOOL                 not null,
   cron                 VARCHAR(30)          null,
   interval_duration    VARCHAR(10)          null,
   valid_begin_ts       INT8                 null,
   valid_end_ts         INT8                 null,
   pre_assign_duration  VARCHAR(10)          null,
   next_assign_ts       INT8                 not null,
   remark               VARCHAR(50)          null,
   enabled              BOOL                 not null default true,
   creator_id           INT8                 not null,
   create_ts            INT8                 not null,
   updator_id           INT8                 not null,
   update_ts            INT8                 not null,
   constraint PK_JOBX_JOB primary key (id),
   constraint AK_CODE_JOBX_JOB unique (code),
   constraint AK_NAME_JOBX_JOB unique (name)
);

comment on table jobx_job is
'任务计划';

comment on column jobx_job.id is
'ID';

comment on column jobx_job.code is
'编码';

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
'是否高频任务';

comment on column jobx_job.cron is
'cron表达式';

comment on column jobx_job.interval_duration is
'固定间隔时间';

comment on column jobx_job.valid_begin_ts is
'有效开始时间戳';

comment on column jobx_job.valid_end_ts is
'有效结束时间戳';

comment on column jobx_job.pre_assign_duration is
'提前分派时间';

comment on column jobx_job.next_assign_ts is
'下次分派时间戳';

comment on column jobx_job.remark is
'备注';

comment on column jobx_job.enabled is
'启用';

comment on column jobx_job.creator_id is
'创建人的用户ID';

comment on column jobx_job.create_ts is
'创建时间戳';

comment on column jobx_job.updator_id is
'修改人的用户ID';

comment on column jobx_job.update_ts is
'修改时间戳';

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
   assign_ts            INT8                 not null,
   scheduled_assign_ts  INT8                 null,
   exec_detail          VARCHAR(800)         null,
   exec_start_ts        INT8                 null,
   exec_end_ts          INT8                 null,
   creator_id           INT8                 not null,
   create_ts            INT8                 not null,
   updator_id           INT8                 not null,
   update_ts            INT8                 not null,
   constraint PK_JOBX_TASK primary key (id),
   constraint AK_JOB_ID_AND_SCHEDULED_ASSIGN_TS unique (job_id, assign_ts)
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

comment on column jobx_task.assign_ts is
'分派时间戳';

comment on column jobx_task.scheduled_assign_ts is
'预定分派时间戳
为计划分派任务当时的下次分派时间戳';

comment on column jobx_task.exec_detail is
'执行详情';

comment on column jobx_task.exec_start_ts is
'开始执行时间戳';

comment on column jobx_task.exec_end_ts is
'结束执行时间戳';

comment on column jobx_task.creator_id is
'创建人的用户ID';

comment on column jobx_task.create_ts is
'创建时间戳';

comment on column jobx_task.updator_id is
'修改人的用户ID';

comment on column jobx_task.update_ts is
'修改时间戳';

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

