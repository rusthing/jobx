import { useState, useEffect, useCallback } from 'react';
import { Table, Input, Select, Space, Drawer } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { getTaskPage } from '../../api/task';
import type { JobxTaskVo, TaskQuery, TaskStatus, TaskType } from '../../api/types';
import {
  JOB_TYPE_MAP,
  TASK_STATUS_MAP,
  TASK_TYPE_MAP,
  TASK_STATUS_OPTIONS,
  TASK_TYPE_OPTIONS,
  labelOf,
} from '../../utils/enum';
import { formatTimestamp, calcDuration } from '../../utils/format';

export default function TaskPage() {
  const [data, setData] = useState<JobxTaskVo[]>([]);
  const [loading, setLoading] = useState(false);
  const [pagination, setPagination] = useState({ current: 1, pageSize: 10, total: 0 });
  const [filters, setFilters] = useState<TaskQuery>({});
  const [detailVisible, setDetailVisible] = useState(false);
  const [detailRecord, setDetailRecord] = useState<JobxTaskVo | null>(null);

  const fetchData = useCallback(async (page = pagination.current, size = pagination.pageSize) => {
    setLoading(true);
    try {
      const query: TaskQuery = {
        ...filters,
        _page: String(page),
        _size: String(size),
        _orderBy: 'create_ms:desc',
      };
      const res = await getTaskPage(query);
      if (res) {
        setData(res.list);
        setPagination({ current: page, pageSize: size, total: parseInt(res.total, 10) });
      }
    } catch {
      // 错误提示已由 http 层统一处理，这里只需收尾
    } finally {
      setLoading(false);
    }
  }, [filters, pagination.current, pagination.pageSize]);

  useEffect(() => {
    fetchData(1);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filters]);

  const showDetail = (record: JobxTaskVo) => {
    setDetailRecord(record);
    setDetailVisible(true);
  };

  const columns: ColumnsType<JobxTaskVo> = [
    { title: 'ID', dataIndex: 'id', ellipsis: true, width: 160 },
    { title: '任务类型', dataIndex: 'taskType', width: 100, render: (v) => labelOf(TASK_TYPE_MAP, v) },
    { title: '计划ID', dataIndex: 'jobId', width: 160, ellipsis: true },
    { title: '状态', dataIndex: 'status', width: 80, render: (v) => labelOf(TASK_STATUS_MAP, v) },
    { title: '执行器编码', dataIndex: 'executorCode', width: 140 },
    { title: '计划类型', dataIndex: 'jobType', width: 100, render: (v) => labelOf(JOB_TYPE_MAP, v) },
    { title: '执行详情', dataIndex: 'execDetail', ellipsis: true, width: 200 },
    { title: '耗时', width: 100, render: (_, r) => calcDuration(r.execStartMs, r.execEndMs) },
    { title: '分派时间', dataIndex: 'assignMs', width: 160, render: (v) => formatTimestamp(v) },
    { title: '创建时间', dataIndex: 'createMs', width: 160, render: (v) => formatTimestamp(v) },
    {
      title: '操作',
      fixed: 'right',
      width: 100,
      render: (_, record) => (
        <Space>
          <a onClick={() => showDetail(record)}>详情</a>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <Space style={{ marginBottom: 16 }} wrap>
        <Input
          placeholder="执行器编码"
          value={filters.executorCode}
          onChange={(e) => setFilters({ ...filters, executorCode: e.target.value })}
          allowClear
        />
        <Select
          placeholder="任务状态"
          options={TASK_STATUS_OPTIONS}
          value={filters.status}
          onChange={(v) => setFilters({ ...filters, status: v as TaskStatus | undefined })}
          style={{ width: 120 }}
          allowClear
        />
        <Select
          placeholder="任务类型"
          options={TASK_TYPE_OPTIONS}
          value={filters.taskType}
          onChange={(v) => setFilters({ ...filters, taskType: v as TaskType | undefined })}
          style={{ width: 120 }}
          allowClear
        />
        <Input
          placeholder="计划ID"
          value={filters.jobId}
          onChange={(e) => setFilters({ ...filters, jobId: e.target.value })}
          allowClear
        />
      </Space>
      <Table
        rowKey="id"
        columns={columns}
        dataSource={data}
        loading={loading}
        pagination={{
          ...pagination,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 条`,
          onChange: (page, pageSize) => fetchData(page, pageSize),
        }}
        scroll={{ x: 1200 }}
      />
      <Drawer
        title="任务详情"
        width={480}
        open={detailVisible}
        onClose={() => setDetailVisible(false)}
      >
        {detailRecord && (
          <Space direction="vertical" style={{ width: '100%' }}>
            <p><b>ID：</b>{detailRecord.id}</p>
            <p><b>任务类型：</b>{labelOf(TASK_TYPE_MAP, detailRecord.taskType)}</p>
            <p><b>计划ID：</b>{detailRecord.jobId}</p>
            <p><b>状态：</b>{labelOf(TASK_STATUS_MAP, detailRecord.status)}</p>
            <p><b>执行器编码：</b>{detailRecord.executorCode}</p>
            <p><b>执行器实例：</b>{detailRecord.executorInstance || '-'}</p>
            <p><b>执行参数：</b><pre style={{ whiteSpace: 'pre-wrap' }}>{detailRecord.execParams || '-'}</pre></p>
            <p><b>计划类型：</b>{labelOf(JOB_TYPE_MAP, detailRecord.jobType)}</p>
            <p><b>cron：</b>{detailRecord.cron || '-'}</p>
            <p><b>固定间隔：</b>{detailRecord.intervalDuration != null ? `${detailRecord.intervalDuration}ms` : '-'}</p>
            <p><b>分派提前：</b>{detailRecord.assignLeadMs != null ? `${detailRecord.assignLeadMs}ms` : '-'}</p>
            <p><b>执行详情：</b><pre style={{ whiteSpace: 'pre-wrap' }}>{detailRecord.execDetail || '-'}</pre></p>
            <p><b>分派时间：</b>{formatTimestamp(detailRecord.assignMs)}</p>
            <p><b>预定开始：</b>{formatTimestamp(detailRecord.scheduledExecStartMs)}</p>
            <p><b>实际开始：</b>{formatTimestamp(detailRecord.execStartMs)}</p>
            <p><b>实际结束：</b>{formatTimestamp(detailRecord.execEndMs)}</p>
            <p><b>耗时：</b>{calcDuration(detailRecord.execStartMs, detailRecord.execEndMs)}</p>
            <p><b>创建时间：</b>{formatTimestamp(detailRecord.createMs)}</p>
          </Space>
        )}
      </Drawer>
    </div>
  );
}
