import { useState, useEffect, useCallback } from 'react';
import { Table, Button, Form, Input, Select, Switch, Space, Drawer, Popconfirm, message, InputNumber, DatePicker } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import dayjs from 'dayjs';
import { getJobPage, addJob, modifyJob, deleteJob, triggerJob } from '../../api/job';
import type { JobxJobVo, JobQuery, JobxJobAddDto, JobxJobModifyDto, JobType } from '../../api/types';
import { JOB_TYPE_MAP, JOB_TYPE_OPTIONS } from '../../utils/enum';
import { formatTimestamp } from '../../utils/format';

export default function JobPage() {
  const [data, setData] = useState<JobxJobVo[]>([]);
  const [loading, setLoading] = useState(false);
  const [pagination, setPagination] = useState({ current: 1, pageSize: 10, total: 0 });
  const [filters, setFilters] = useState<JobQuery>({});
  const [drawerVisible, setDrawerVisible] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [form] = Form.useForm();

  const fetchData = useCallback(async (page = pagination.current, size = pagination.pageSize) => {
    setLoading(true);
    try {
      const query: JobQuery = {
        ...filters,
        _page: String(page),
        _size: String(size),
        _orderBy: 'create_ms:desc',
      };
      const res = await getJobPage(query);
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

  const handleAdd = () => {
    setEditingId(null);
    form.resetFields();
    setDrawerVisible(true);
  };

  const handleEdit = (record: JobxJobVo) => {
    setEditingId(record.id);
    form.setFieldsValue({
      ...record,
      validBeginMs: record.validBeginMs ? dayjs(Number(record.validBeginMs)) : null,
      validEndMs: record.validEndMs ? dayjs(Number(record.validEndMs)) : null,
    });
    setDrawerVisible(true);
  };

  const handleSubmit = async () => {
    let values;
    try {
      values = await form.validateFields();
    } catch {
      // 表单校验失败由 AntD 自行标红，请求根本没发出去
      return;
    }
    const dto: JobxJobAddDto | JobxJobModifyDto = {
      ...values,
      validBeginMs: values.validBeginMs ? String(values.validBeginMs.valueOf()) : undefined,
      validEndMs: values.validEndMs ? String(values.validEndMs.valueOf()) : undefined,
    };
    try {
      if (editingId) {
        await modifyJob({ ...dto, id: editingId });
        message.success('修改成功');
      } else {
        await addJob(dto as JobxJobAddDto);
        message.success('新增成功');
      }
      setDrawerVisible(false);
      fetchData();
    } catch {
      // 错误提示已由 http 层统一处理，这里只需收尾
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteJob(id);
      message.success('删除成功');
      fetchData();
    } catch {
      // 错误提示已由 http 层统一处理，这里只需收尾
    }
  };

  const handleTrigger = async (record: JobxJobVo) => {
    try {
      await triggerJob({ jobId: record.id });
      message.success('触发成功');
    } catch {
      // 错误提示已由 http 层统一处理，这里只需收尾
    }
  };

  const columns: ColumnsType<JobxJobVo> = [
    { title: 'ID', dataIndex: 'id', ellipsis: true, width: 160 },
    { title: '名称', dataIndex: 'name', width: 180 },
    { title: '执行器编码', dataIndex: 'executorCode', width: 140 },
    { title: '计划类型', dataIndex: 'jobType', width: 100, render: (v) => JOB_TYPE_MAP[v as string] || v },
    { title: 'cron', dataIndex: 'cron', width: 140 },
    { title: '间隔(ms)', dataIndex: 'intervalDuration', width: 100 },
    { title: '提前(ms)', dataIndex: 'assignLeadMs', width: 100 },
    { title: 'Stream上限', dataIndex: 'streamMaxLen', width: 100 },
    { title: '启用', dataIndex: 'enabled', width: 80, render: (v) => (v ? '是' : '否') },
    { title: '下次分派', dataIndex: 'nextAssignMs', width: 160, render: (v) => formatTimestamp(v) },
    { title: '创建时间', dataIndex: 'createMs', width: 160, render: (v) => formatTimestamp(v) },
    {
      title: '操作',
      fixed: 'right',
      width: 200,
      render: (_, record) => (
        <Space>
          <Button size="small" onClick={() => handleEdit(record)}>编辑</Button>
          <Button size="small" onClick={() => handleTrigger(record)}>触发</Button>
          <Popconfirm title="确认删除？" onConfirm={() => handleDelete(record.id)}>
            <Button size="small" danger>删除</Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <Space style={{ marginBottom: 16 }} wrap>
        <Input
          placeholder="名称关键字"
          value={filters.name}
          onChange={(e) => setFilters({ ...filters, name: e.target.value })}
          allowClear
        />
        <Input
          placeholder="执行器编码"
          value={filters.executorCode}
          onChange={(e) => setFilters({ ...filters, executorCode: e.target.value })}
          allowClear
        />
        <Select
          placeholder="计划类型"
          options={[{ value: '', label: '全部' }, ...JOB_TYPE_OPTIONS]}
          value={filters.jobType || ''}
          onChange={(v) => setFilters({ ...filters, jobType: (v || undefined) as JobType | undefined })}
          style={{ width: 120 }}
          allowClear
        />
        <Select
          placeholder="启用状态"
          options={[
            { value: '', label: '全部' },
            { value: 'true', label: '启用' },
            { value: 'false', label: '停用' },
          ]}
          value={filters.enabled === undefined ? '' : String(filters.enabled)}
          onChange={(v) => setFilters({ ...filters, enabled: v === '' ? undefined : v === 'true' })}
          style={{ width: 100 }}
        />
        <Button type="primary" onClick={handleAdd}>新增计划</Button>
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
        title={editingId ? '编辑计划' : '新增计划'}
        width={560}
        open={drawerVisible}
        onClose={() => setDrawerVisible(false)}
        footer={
          <Space>
            <Button type="primary" onClick={handleSubmit}>保存</Button>
            <Button onClick={() => setDrawerVisible(false)}>取消</Button>
          </Space>
        }
      >
        <Form form={form} layout="vertical">
          <Form.Item name="executorCode" label="执行器编码" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="name" label="名称" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="jobType" label="计划类型" rules={[{ required: true }]}>
            <Select options={JOB_TYPE_OPTIONS} />
          </Form.Item>
          <Form.Item noStyle shouldUpdate={(prev, cur) => prev.jobType !== cur.jobType}>
            {({ getFieldValue }) => {
              const jobType = getFieldValue('jobType');
              return jobType === 'Cron' ? (
                <Form.Item name="cron" label="Cron 表达式" rules={[{ required: true }]}>
                  <Input />
                </Form.Item>
              ) : (jobType === 'FixedDelay' || jobType === 'FixedRate') ? (
                <Form.Item name="intervalDuration" label="固定间隔（毫秒）" rules={[{ required: true }]}>
                  <InputNumber style={{ width: '100%' }} min={0} />
                </Form.Item>
              ) : null;
            }}
          </Form.Item>
          <Form.Item name="params" label="参数（JSON）">
            <Input.TextArea rows={3} />
          </Form.Item>
          <Form.Item name="validBeginMs" label="有效开始时间">
            <DatePicker showTime style={{ width: '100%' }} format="YYYY-MM-DD HH:mm:ss" />
          </Form.Item>
          <Form.Item name="validEndMs" label="有效结束时间">
            <DatePicker showTime style={{ width: '100%' }} format="YYYY-MM-DD HH:mm:ss" />
          </Form.Item>
          <Form.Item name="assignLeadMs" label="分派提前毫秒数">
            <InputNumber style={{ width: '100%' }} min={0} />
          </Form.Item>
          <Form.Item name="streamMaxLen" label="Stream 长度上限">
            <InputNumber style={{ width: '100%' }} min={0} />
          </Form.Item>
          <Form.Item name="remark" label="备注">
            <Input.TextArea rows={2} />
          </Form.Item>
          <Form.Item name="enabled" label="启用" valuePropName="checked" initialValue={true}>
            <Switch />
          </Form.Item>
        </Form>
      </Drawer>
    </div>
  );
}
