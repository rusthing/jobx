/*==============================================================*/
/* DBMS name:      MySQL 5.0                                    */
/* Created on:     2026/9/17 12:41:11                           */
/*==============================================================*/


/*==============================================================*/
/* Table: jobx_job                                              */
/*==============================================================*/
create table jobx_job
(
    id                   bigint not null  comment 'ID',
    name                 varchar(50) not null  comment '名称',
    params               varchar(800)  comment '参数',
    job_type             tinyint not null  comment '计划类型
             0: 手动分派
             1: cron表达式
             2: 固定延迟
             3: 固定频率',
    high_freq            bit(1) default false  comment '是否高频任务',
    cron                 varchar(30)  comment 'cron表达式',
    interval_duration    varchar(10)  comment '固定间隔时间',
    valid_begin_ts       bigint  comment '有效开始时间戳',
    valid_end_ts         bigint  comment '有效结束时间戳',
    executor_code        varchar(50) not null  comment '执行器编码
             发布任务消息时的key将以此编码结尾，只有相同编码的执行器才订阅此key',
    assign_lead_duration varchar(10)  comment '分派提前时间',
    next_assign_ts       bigint  comment '下次分派时间戳',
    remark               varchar(50)  comment '备注',
    enabled              bit(1) not null default true  comment '启用',
    creator_id           bigint not null  comment '创建人的用户ID',
    create_ts            bigint not null  comment '创建时间戳',
    updator_id           bigint not null  comment '修改人的用户ID',
    update_ts            bigint not null  comment '修改时间戳',
    primary key (id),
    unique key AK_name (name)
);

alter table jobx_job comment '任务计划';

/*==============================================================*/
/* Table: jobx_task                                             */
/*==============================================================*/
create table jobx_task
(
    id                   bigint not null  comment 'ID',
    task_type            tinyint not null  comment '任务类型
             1: 立即执行
             2: 计划执行',
    job_id               bigint  comment '任务计划ID',
    status               tinyint not null default 0  comment '任务状态
             0: 运行中
             1: 成功
             2: 失败
             ',
    assign_ts            bigint not null  comment '分派时间戳',
    scheduled_assign_ts  bigint  comment '预定分派时间戳
             为计划分派任务当时的下次分派时间戳',
    exec_detail          varchar(800)  comment '执行详情',
    exec_start_ts        bigint  comment '开始执行时间戳',
    exec_end_ts          bigint  comment '结束执行时间戳',
    creator_id           bigint not null  comment '创建人的用户ID',
    create_ts            bigint not null  comment '创建时间戳',
    updator_id           bigint not null  comment '修改人的用户ID',
    update_ts            bigint not null  comment '修改时间戳',
    primary key (id),
    unique key AK_job_id_and_scheduled_assign_ts (job_id, scheduled_assign_ts)
);

alter table jobx_task comment '任务记录';

alter table jobx_task add constraint fk_job_id__from__jobx_job foreign key (job_id)
    references jobx_job (id) on delete restrict on update restrict;



