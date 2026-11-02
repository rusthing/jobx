/*==============================================================*/
/* 新增任务分组                                                  */
/*==============================================================*/
--
-- ## 为什么单独建表，而不是在 jobx_job 上加一个字符串列
--
-- 计划原本只有两个区分维度：执行器编码（谁跑）与名称（跑什么）。分组回答的是
-- 「这批计划归谁管」，与执行器正交，所以不能复用 executor_code。
-- 用字符串列（jobx_job.group_name）看似更省事，代价是：改名要批量刷计划、
-- 排序无从表达、错别字会把同一组悄悄裂成两三组且无从收敛。
--
-- ## group_id 可为空
--
-- 空 = 未分组。存量计划不在迁移里补值，保持原样即可。
--
-- ## 外键 on delete restrict
--
-- 组里还挂着计划时不允许删组，得先把计划挪到别的组或置空 —— 否则删一个分组
-- 会连带影响一批计划的归属，而调度语义（跑不跑、多久跑）又完全没变，
-- 排查时很难把「某个计划突然不见了」和「有人清了分组」联系起来。

create table jobx_group (
   id                   INT8                 not null,
   name                 VARCHAR(50)          not null,
   remark               VARCHAR(50)          null,
   creater_id           INT8                 not null,
   create_ms            INT8                 not null,
   updater_id           INT8                 not null,
   update_ms            INT8                 not null,
   constraint PK_JOBX_GROUP primary key (id),
   constraint AK_NAME_JOBX_GROUP unique (name)
);

comment on table jobx_group is
'任务分组';

comment on column jobx_group.id is
'ID';

comment on column jobx_group.name is
'名称
@len 1..=50';

comment on column jobx_group.remark is
'备注
@len 0..=50';

comment on column jobx_group.creater_id is
'创建者ID';

comment on column jobx_group.create_ms is
'创建时间戳';

comment on column jobx_group.updater_id is
'更新者ID';

comment on column jobx_group.update_ms is
'更新时间戳';

create unique index jobx_group_PK on jobx_group (
id
);

/*--------------------------------------------------------------*/
/* jobx_job：分组归属                                            */
/*--------------------------------------------------------------*/
alter table jobx_job
   add column group_id INT8 null;

comment on column jobx_job.group_id is
'任务分组ID
为空表示未分组';

-- 外键索引：按分组筛计划是列表页的常规查询，没有它会退化成全表扫描
create index Relationship_2_FK on jobx_job (
group_id
);

alter table jobx_job
   add constraint fk_group_id__from__jobx_group foreign key (group_id)
      references jobx_group (id)
      on delete restrict on update restrict;
