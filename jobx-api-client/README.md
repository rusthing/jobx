# jobx-api-client

JobX 服务的 Rust API 客户端库，基于 [robotech](https://github.com/rusthing/robotech-rs) 的 Feign 客户端实现，支持通过服务发现或直连方式调用远程 JobX 服务。

## 功能

- 完整的 JobX RESTful API 客户端封装
- 支持 Consul 服务发现自动寻址
- 支持配置热更新，无需重启即可切换服务地址
- 提供 Job、Schedule、Task 三组 API 客户端

## 模块结构

```
src/
├── api_client/
│   ├── job_api_client.rs           # 任务 (Job) API 客户端
│   ├── schedule_api_client.rs      # 调度 (Schedule) API 客户端
│   ├── task_api_client.rs          # 任务执行 (Task) API 客户端
│   ├── jobx_api_client_utils.rs    # 客户端初始化与配置工具
│   └── mod.rs
└── lib.rs
```

## 使用

### 添加依赖

```toml
[dependencies]
jobx-api-client = "0.1.0"
```

### 初始化客户端

```rust
use jobx_api_client::api_client::{setup_jobx_api_client, get_jobx_api_client};
use robotech::api_client::ApiClientConfig;
use std::collections::HashMap;

// 配置 API 客户端
let mut apis_config = HashMap::new();
apis_config.insert(
"jobx".to_string(),
ApiClientConfig {
base_url: "http://localhost:8080".to_string(),
..Default::default()
},
);

// 初始化客户端
setup_jobx_api_client(apis_config, &None).await?;

// 获取客户端实例
let client = get_jobx_api_client()?;
```

### 调用 API

```rust
// Job API
client.job_client.list_jobs(...).await?;
client.job_client.get_job(...).await?;
client.job_client.create_job(...).await?;
client.job_client.update_job(...).await?;
client.job_client.delete_job(...).await?;

// Schedule API
client.schedule_client.list_schedules(...).await?;
client.schedule_client.get_schedule(...).await?;
client.schedule_client.create_schedule(...).await?;
client.schedule_client.update_schedule(...).await?;
client.schedule_client.delete_schedule(...).await?;

// Task API
client.task_client.list_tasks(...).await?;
client.task_client.get_task(...).await?;
client.task_client.create_task(...).await?;
client.task_client.update_task(...).await?;
client.task_client.delete_task(...).await?;
```

## API 客户端结构

```rust
pub struct JobxApiClient {
    pub job_client: JobApiClient,         // 任务管理
    pub task_client: TaskApiClient,       // 任务执行管理
    pub schedule_client: ScheduleApiClient, // 调度管理
}
```

## 技术栈

- [robotech](https://github.com/rusthing/robotech-rs) - Feign 客户端与服务发现
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP 客户端
- [serde](https://serde.rs/) / [serde_json](https://github.com/serde-rs/json) - 序列化
- [arc-swap](https://crates.io/crates/arc-swap) - 无锁配置热更新
- [config](https://github.com/mehcode/config-rs) - 配置管理

## 许可证

MIT License
