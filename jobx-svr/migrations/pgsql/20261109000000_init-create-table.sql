-- jobx 数据库迁移基线：init-create-table（最终态建表）
--
-- 清库后按 000000 → 000001 → 000002 顺序执行一次建成。
-- 表、列、主键、唯一键、外键、检查约束与注释；普通索引在 init-index.sql。
-- 与线上库 pg_dump -s 归一化后逐行一致。

CREATE TABLE public.jobx_group (
    id bigint NOT NULL,
    name character varying(50) NOT NULL,
    remark character varying(50),
    creater_id bigint NOT NULL,
    create_ms bigint NOT NULL,
    updater_id bigint NOT NULL,
    update_ms bigint NOT NULL
);

COMMENT ON TABLE public.jobx_group IS '任务分组';

COMMENT ON COLUMN public.jobx_group.id IS 'ID';

COMMENT ON COLUMN public.jobx_group.name IS '名称
@len 1..=50';

COMMENT ON COLUMN public.jobx_group.remark IS '备注
@len 0..=50';

COMMENT ON COLUMN public.jobx_group.creater_id IS '创建者ID';

COMMENT ON COLUMN public.jobx_group.create_ms IS '创建时间戳';

COMMENT ON COLUMN public.jobx_group.updater_id IS '更新者ID';

COMMENT ON COLUMN public.jobx_group.update_ms IS '更新时间戳';

CREATE TABLE public.jobx_job (
    id bigint NOT NULL,
    name character varying(50) NOT NULL,
    group_id bigint,
    params character varying(800),
    job_type smallint NOT NULL,
    high_freq boolean DEFAULT false,
    cron character varying(30),
    interval_duration character varying(10),
    valid_begin_ms bigint,
    valid_end_ms bigint,
    executor_code character varying(50) NOT NULL,
    assign_lead_ms integer,
    next_assign_ms bigint,
    stream_max_len integer,
    task_expire_duration character varying(30),
    remark character varying(50),
    enabled boolean DEFAULT true NOT NULL,
    creater_id bigint NOT NULL,
    create_ms bigint NOT NULL,
    updater_id bigint NOT NULL,
    update_ms bigint NOT NULL
);

COMMENT ON TABLE public.jobx_job IS '任务计划';

COMMENT ON COLUMN public.jobx_job.id IS 'ID';

COMMENT ON COLUMN public.jobx_job.name IS '名称
@len 1..=50';

COMMENT ON COLUMN public.jobx_job.group_id IS '任务分组ID
为空表示未分组';

COMMENT ON COLUMN public.jobx_job.params IS '参数
@len 0..=800';

COMMENT ON COLUMN public.jobx_job.job_type IS '计划类型
@enum JobType default=Manual
  0=Manual      手动分派
  1=Cron        cron 表达式
  2=FixedDelay  固定延迟
  3=FixedRate   固定频率';

COMMENT ON COLUMN public.jobx_job.high_freq IS '是否高频任务
这个由计划类型、cron表达式、固定时间间隔与配置中的高频任务时长阈值计算得出';

COMMENT ON COLUMN public.jobx_job.cron IS 'cron表达式
当 job_type 为 cron表达式时必填
@len 0..=30';

COMMENT ON COLUMN public.jobx_job.interval_duration IS '固定间隔时间
当job_type为固定延迟或固定频率时必填';

COMMENT ON COLUMN public.jobx_job.valid_begin_ms IS '有效开始时间戳';

COMMENT ON COLUMN public.jobx_job.valid_end_ms IS '有效结束时间戳';

COMMENT ON COLUMN public.jobx_job.executor_code IS '执行器编码
发布任务消息时的key将以此编码结尾，只有相同编码的执行器才订阅此key
@len 1..=50';

COMMENT ON COLUMN public.jobx_job.assign_lead_ms IS '分派提前毫秒数';

COMMENT ON COLUMN public.jobx_job.next_assign_ms IS '下次分派时间戳';

COMMENT ON COLUMN public.jobx_job.stream_max_len IS '发布该计划的消息时使用的 Stream 长度上限（MAXLEN ~ N）
为空时采用配置文件中 jobx.scheduler.stream-max-len 的默认值';

COMMENT ON COLUMN public.jobx_job.task_expire_duration IS '任务保留时长
该计划产生的任务记录超过这个时长后会被服务端自动清理
基准是任务的创建时刻（create_ms），与任务是否执行完无关
为空表示永久保留，不参与清理';

COMMENT ON COLUMN public.jobx_job.remark IS '备注
@len 0..=50';

COMMENT ON COLUMN public.jobx_job.enabled IS '启用';

COMMENT ON COLUMN public.jobx_job.creater_id IS '创建者ID';

COMMENT ON COLUMN public.jobx_job.create_ms IS '创建时间戳';

COMMENT ON COLUMN public.jobx_job.updater_id IS '更新者ID';

COMMENT ON COLUMN public.jobx_job.update_ms IS '更新时间戳';

CREATE TABLE public.jobx_task (
    id bigint NOT NULL,
    task_type smallint NOT NULL,
    job_id bigint,
    status smallint DEFAULT 0 NOT NULL,
    assign_ms bigint NOT NULL,
    scheduled_exec_start_ms bigint,
    exec_detail character varying(800),
    exec_start_ms bigint,
    exec_end_ms bigint,
    executor_code character varying(50) NOT NULL,
    executor_instance character varying(100),
    exec_params character varying(800),
    job_type smallint NOT NULL,
    high_freq boolean DEFAULT false,
    cron character varying(30),
    interval_duration character varying(10),
    valid_begin_ms bigint,
    valid_end_ms bigint,
    exec_deadline_ms bigint,
    assign_lead_ms integer,
    creater_id bigint NOT NULL,
    create_ms bigint NOT NULL,
    updater_id bigint NOT NULL,
    update_ms bigint NOT NULL
);

COMMENT ON TABLE public.jobx_task IS '任务记录';

COMMENT ON COLUMN public.jobx_task.id IS 'ID';

COMMENT ON COLUMN public.jobx_task.task_type IS '任务类型
@enum TaskType default=Immediate
  1=Immediate  立即执行
  2=Scheduled  计划执行';

COMMENT ON COLUMN public.jobx_task.job_id IS '任务计划ID';

COMMENT ON COLUMN public.jobx_task.status IS '任务状态
@enum TaskStatus default=Running
  0=Running  运行中
  1=Success  成功
  2=Failed   失败';

COMMENT ON COLUMN public.jobx_task.assign_ms IS '分派时间戳';

COMMENT ON COLUMN public.jobx_task.scheduled_exec_start_ms IS '预定开始执行时间戳';

COMMENT ON COLUMN public.jobx_task.exec_detail IS '执行详情
@len 0..=800';

COMMENT ON COLUMN public.jobx_task.exec_start_ms IS '开始执行时间戳';

COMMENT ON COLUMN public.jobx_task.exec_end_ms IS '结束执行时间戳';

COMMENT ON COLUMN public.jobx_task.executor_code IS '执行器编码
@len 1..=50';

COMMENT ON COLUMN public.jobx_task.executor_instance IS '执行器实例
@len 0..=100';

COMMENT ON COLUMN public.jobx_task.exec_params IS '执行参数
@len 0..=800';

COMMENT ON COLUMN public.jobx_task.job_type IS '计划类型
@enum JobType default=Manual
  0=Manual      手动分派
  1=Cron        cron 表达式
  2=FixedDelay  固定延迟
  3=FixedRate   固定频率';

COMMENT ON COLUMN public.jobx_task.high_freq IS '是否高频任务';

COMMENT ON COLUMN public.jobx_task.cron IS 'cron表达式
@len 0..=30';

COMMENT ON COLUMN public.jobx_task.interval_duration IS '固定间隔时间';

COMMENT ON COLUMN public.jobx_task.valid_begin_ms IS '有效开始时间戳';

COMMENT ON COLUMN public.jobx_task.valid_end_ms IS '有效结束时间戳';

COMMENT ON COLUMN public.jobx_task.exec_deadline_ms IS '执行截止时间戳
高频任务本次分派的执行时段终点（min(计划有效结束时间, 预定开始执行时间 + 高频分派间隔)）；为空表示不限，仅对高频任务有意义';

COMMENT ON COLUMN public.jobx_task.assign_lead_ms IS '分派提前毫秒数';

COMMENT ON COLUMN public.jobx_task.creater_id IS '创建者ID';

COMMENT ON COLUMN public.jobx_task.create_ms IS '创建时间戳';

COMMENT ON COLUMN public.jobx_task.updater_id IS '更新者ID';

COMMENT ON COLUMN public.jobx_task.update_ms IS '更新时间戳';

ALTER TABLE ONLY public.jobx_task
    ADD CONSTRAINT ak_job_id_and_scheduled_exec_start_ms_jobx_task UNIQUE (job_id, scheduled_exec_start_ms);

ALTER TABLE ONLY public.jobx_group
    ADD CONSTRAINT ak_name_jobx_group UNIQUE (name);

ALTER TABLE ONLY public.jobx_job
    ADD CONSTRAINT ak_name_jobx_job UNIQUE (name);

ALTER TABLE ONLY public.jobx_group
    ADD CONSTRAINT pk_jobx_group PRIMARY KEY (id);

ALTER TABLE ONLY public.jobx_job
    ADD CONSTRAINT pk_jobx_job PRIMARY KEY (id);

ALTER TABLE ONLY public.jobx_task
    ADD CONSTRAINT pk_jobx_task PRIMARY KEY (id);

CREATE UNIQUE INDEX jobx_group_pk ON public.jobx_group USING btree (id);

CREATE UNIQUE INDEX jobx_job_pk ON public.jobx_job USING btree (id);

CREATE UNIQUE INDEX jobx_task_pk ON public.jobx_task USING btree (id);

ALTER TABLE ONLY public.jobx_job
    ADD CONSTRAINT fk_group_id__from__jobx_group FOREIGN KEY (group_id) REFERENCES public.jobx_group(id) ON UPDATE RESTRICT ON DELETE RESTRICT;

ALTER TABLE ONLY public.jobx_task
    ADD CONSTRAINT fk_job_id__from__jobx_job FOREIGN KEY (job_id) REFERENCES public.jobx_job(id) ON UPDATE RESTRICT ON DELETE RESTRICT;
