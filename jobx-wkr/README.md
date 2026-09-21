# jobx-wkr

JobX 任务执行器（Executor）客户端库，为下游 Worker 应用提供简化的任务执行工具。通过 Redis Stream 订阅任务消息，自动拉取待执行任务并通过 HTTP API 上报执行结果。

## 功能

- **Redis Stream 订阅** - 通过 Redis Stream 接收服务端分发的任务消息
- **自动任务拉取** - 从服务端拉取待执行任务并自动创建任务记录
- **执行状态上报** - 支持启动任务、上报成功/失败结果
- **配置热更新** - 运行时自动检测并重载配置变更
- **消费者组管理** - 自动创建和管理 Redis 消费者组

## 模块结构

```
src/
├── config/
│   ├── jobx_executor_config.rs   # Executor 配置（Redis Stream、扫描间隔、重试策略等）
│   └── mod.rs
├── utils/
│   ├── jobx_executor_utils.rs    # Executor 核心工具函数
│   └── mod.rs
└── lib.rs                        # 库入口
```

## 配置说明

```toml
[jobx.executor]
# Redis Stream 的 key 前缀，后面跟着任务执行器的编码
executor-key = "jobx:stream:"
# 任务执行器的编码
executor-code = "my-executor"
# 任务执行器订阅消息分组
executor-group = "my-group"
# 扫描间隔
scan-interval = "5s"
# 扫描阻塞时间
scan-block-duration = "2s"
# 单次接收消息的最大数量
max-messages = 10
# 上报执行结果失败时最大重试次数
report-retry-count = 3
# 上报执行结果重试间隔
report-retry-interval = "1s"
```

## 使用

### 添加依赖

```toml
[dependencies]
jobx-wkr = "1.0.0"
```

### 基本用法

```rust
use jobx_wkr::{create_client, fetch_pending_tasks, start_task, report_success, JobxExecutorConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = JobxExecutorConfig {
        base_url: "http://127.0.0.1:8080".to_string(),
        user_id: 1,
    };
    let client = create_client(&config).await?;

    // 拉取待执行任务
    let tasks = fetch_pending_tasks(&client, config.user_id, "my-job").await?;

    for task in tasks {
        let task_id: u64 = task.id.into();
        start_task(&client, config.user_id, task_id).await?;

        // 执行业务逻辑...

        report_success(&client, config.user_id, task_id, Some("执行成功")).await?;
    }

    Ok(())
}
```

### 实现自定义消息处理器

```rust
use async_trait::async_trait;
use jobx_api::vo::JobxTaskVo;
use jobx_wkr::JobxMessageHandler;

struct MyHandler;

#[async_trait]
impl JobxMessageHandler for MyHandler {
    async fn handle(&self, task: JobxTaskVo) -> Result<(), Box<dyn std::error::Error>> {
        // 处理任务逻辑
        println!("Processing task: {:?}", task.id);
        Ok(())
    }
}
```

### 启动 Executor 监听

```rust
use jobx_wkr::setup_jobx_executor;
use std::sync::Arc;

let handler = Arc::new(MyHandler);
setup_jobx_executor(handler).await?;
```