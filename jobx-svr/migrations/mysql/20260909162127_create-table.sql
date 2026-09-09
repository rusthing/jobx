/*==============================================================*/
/* DBMS name:      MySQL 5.0                                    */
/* Created on:     2026/9/9 16:21:27                            */
/*==============================================================*/


/*==============================================================*/
/* Table: jobx_job                                              */
/*==============================================================*/
create table jobx_job
(
    id                   bigint not null  comment 'ID',
    code                 varchar(50) not null  comment '编码',
    name                 varchar(50) not null  comment '名称',
    params               varchar(800)  comment '参数',
    scheduling_type      tinyint not null  comment '计划类型
             1: cron表达式
             2: 固定延迟
             3: 固定频率',
    cron                 varchar(30)  comment 'cron表达式',
    interval_duration    varchar(10)  comment '固定间隔时间',
    valid_begin_ts       bigint  comment '有效开始时间戳',
    valid_end_ts         bigint  comment '有效结束时间戳',
    next_trigger_ts      bigint not null  comment '下次触发时间戳',
    remark               varchar(50)  comment '备注',
    enabled              bit(1) not null default true  comment '启用',
    creator_id           bigint not null  comment '创建人的用户ID',
    create_ts            bigint not null  comment '建立时间戳',
    updator_id           bigint not null  comment '修改人的用户ID',
    update_ts            bigint not null  comment '修改时间戳',
    primary key (id),
    unique key AK_code (code),
    unique key AK_name (name)
);

alter table jobx_job comment '任务计划';

/*==============================================================*/
/* Table: jobx_task                                             */
/*==============================================================*/
create table jobx_task
(
    id                   bigint not null  comment 'ID',
    job_id               bigint  comment '任务计划ID',
    status               tinyint not null default 0  comment '任务状态
             0: 运行中
             1: 成功
             2: 失败
             ',
    scheduled_ts         bigint not null  comment '预定执行时间戳',
    start_ts             bigint  comment '开始执行时间戳',
    end_ts               bigint  comment '结束执行时间戳',
    creator_id           bigint not null  comment '创建人的用户ID',
    create_ts            bigint not null  comment '建立时间戳',
    updator_id           bigint not null  comment '修改人的用户ID',
    update_ts            bigint not null  comment '修改时间戳',
    primary key (id),
    unique key AK_job_id_and_scheduled_ts (job_id, scheduled_ts)
);

alter table jobx_task comment '任务记录';

alter table jobx_task add constraint fk_job_id__from__jobx_job foreign key (job_id)
    references jobx_job (id) on delete restrict on update restrict;

