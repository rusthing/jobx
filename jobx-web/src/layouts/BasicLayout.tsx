import { useState, useEffect } from 'react';
import { Layout, Menu, Input, Space, Typography } from 'antd';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { ScheduleOutlined, UnorderedListOutlined } from '@ant-design/icons';

const { Header, Sider, Content } = Layout;
const { Text } = Typography;

export default function BasicLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const [userId, setUserId] = useState(() => localStorage.getItem('jobx_user_id') || '1');

  useEffect(() => {
    localStorage.setItem('jobx_user_id', userId);
  }, [userId]);

  const menuItems = [
    { key: '/job', icon: <ScheduleOutlined />, label: '计划管理' },
    { key: '/task', icon: <UnorderedListOutlined />, label: '任务监控' },
  ];

  const selectedKey = location.pathname === '/' ? '/job' : location.pathname;

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Sider theme="light">
        <div style={{ padding: 16, fontWeight: 500, fontSize: 16 }}>JobX 管理平台</div>
        <Menu
          mode="inline"
          selectedKeys={[selectedKey]}
          items={menuItems}
          onClick={({ key }) => navigate(key)}
        />
      </Sider>
      <Layout>
        <Header style={{ background: '#fff', padding: '0 24px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Text type="secondary">任务调度后台管理系统</Text>
          <Space>
            <Text>当前用户ID：</Text>
            <Input
              value={userId}
              onChange={(e) => setUserId(e.target.value)}
              style={{ width: 120 }}
              size="small"
            />
          </Space>
        </Header>
        <Content style={{ margin: 24, padding: 24, background: '#fff', borderRadius: 8 }}>
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  );
}
