# JobX

基于 Rust 生态的任务调度服务系统，提供任务的创建、调度、执行和监控功能。支持 Cron 表达式定时调度，通过 RESTful API 接口提供服务。

## 项目结构

```
jobx/
├── Cargo.toml              # 工作区配置
├── jobx-api/               # 共享数据类型 (DTO / VO / MO)
│   └── src/
│       ├── dto/            # 数据传输对象 (Data Transfer Object)
│       ├── vo/             # 视图对象 (View Object)
│       └── mo/             # 模型对象 (Model Object, SeaORM 实体)
├── jobx-api-client/        # API 客户端
│   └── src/
│       └── api_client/     # Job / Schedule / Task 的 API 客户端
└── jobx-svr/               # 服务端应用
    ├── migrations/         # 数据库迁移文件
    │   ├── mysql/
    │   ├── pgsql/
    │   └── sqlite/
    └── src/
        ├── config/         # 应用配置 (JobX 配置、Web 配置等)
        ├── dao/            # 数据访问层 (基于 SeaORM)
        ├── svc/            # 业务逻辑层
        ├── web/            # Web 层
        │   ├── api_doc/    # Swagger API 文档
        │   ├── ctrl/       # 控制器
        │   └── router/     # 路由
        ├── lib.rs
        └── main.rs         # 应用入口
```

## 核心功能

### 任务管理 (Job)
- 创建、更新、删除、查询任务定义
- 支持任务分组 (`group`) 和任务类型 (`job_type`)
- 支持 JSON 格式的任务参数 (`params`)

### 调度管理 (Schedule)
- 基于 Cron 表达式的定时调度
- 支持启用/禁用调度
- 调度关联到具体任务

### 任务执行 (Task)
- 任务执行实例记录
- 执行状态跟踪
- 执行结果记录

## 技术栈

| 类别 | 技术 |
|------|------|
| Web 框架 | [Axum](https://github.com/tokio-rs/axum) 0.8 |
| 异步运行时 | [Tokio](https://tokio.rs/) 1.x |
| ORM | [SeaORM](https://www.sea-ql.org/SeaORM/) 2.0 |
| API 文档 | [utoipa](https://github.com/juhaku/utoipa) + Swagger UI |
| 微服务框架 | [robotech](https://github.com/rusthing/robotech-rs) |
| 序列化 | [serde](https://serde.rs/) + [serde_json](https://github.com/serde-rs/json) |
| 配置管理 | [config](https://github.com/mehcode/config-rs) |
| 命令行 | [clap](https://docs.rs/clap/latest/clap/) |
| 日志 | [tracing](https://github.com/tokio-rs/tracing) |
| 数据库 | PostgreSQL (也支持 SQLite / MySQL) |
| 服务注册 | Consul |

## 快速开始

### 前置条件

- [Rust](https://www.rust-lang.org/) 1.80+ (edition 2024)
- PostgreSQL 数据库
- (可选) Consul 用于服务注册与配置中心

### 配置

1. 创建 PostgreSQL 数据库：

```sql
CREATE USER jobx WITH PASSWORD 'jobx';
CREATE DATABASE jobx OWNER jobx;
```

2. 编辑配置文件 `jobx-svr/jobx-svr.toml`：

```toml
profile = "dev"

[log]
level = "debug"

[db]
url = "postgres://jobx:jobx@127.0.0.1:5432/jobx"

[jobx]
scheduler-workers = 4           # 调度器线程数
task-timeout-seconds = 3600     # 任务执行超时时间(秒)
max-retry-count = 3             # 最大重试次数
```

3. 配置环境变量 (`.env`)：

```
DATABASE_URL=postgres://jobx:jobx@127.0.0.1/jobx
```

### 编译与运行

```bash
# 编译
cargo build --release --all-features

# 运行
cargo run --release --all-features

# 指定配置文件
cargo run --release --all-features -- --config-file /path/to/config.toml

# 指定端口
cargo run --release --all-features -- --port 8080
```

### 信号控制

支持通过信号控制服务生命周期：

```bash
# 启动 (默认)
cargo run -- --signal start

# 优雅重启 (先发送 SIGTERM 停止旧进程，再启动新进程)
cargo run -- --signal restart

# 优雅停止 (发送 SIGTERM)
cargo run -- --signal stop

# 强制终止 (发送 SIGKILL)
cargo run -- --signal kill
```

## API 接口

服务启动后，Swagger UI 文档地址：`http://localhost:8080/swagger-ui`

### 任务 (Job) API

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/job/list` | 分页查询任务列表 |
| `GET` | `/job/:id` | 查询单个任务 |
| `POST` | `/job` | 创建任务 |
| `PUT` | `/job/:id` | 更新任务 |
| `DELETE` | `/job/:id` | 删除任务 |

### 调度 (Schedule) API

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/schedule/list` | 分页查询调度列表 |
| `GET` | `/schedule/:id` | 查询单个调度 |
| `POST` | `/schedule` | 创建调度 |
| `PUT` | `/schedule/:id` | 更新调度 |
| `DELETE` | `/schedule/:id` | 删除调度 |

### 任务执行 (Task) API

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/task/list` | 分页查询任务执行列表 |
| `GET` | `/task/:id` | 查询单个任务执行 |
| `POST` | `/task` | 创建任务执行 |
| `PUT` | `/task/:id` | 更新任务执行 |
| `DELETE` | `/task/:id` | 删除任务执行 |

## 使用 API 客户端

`jobx-api-client` 提供了 Rust 语言的 API 客户端，支持通过服务发现调用远程 JobX 服务：

```rust
use jobx_api_client::api_client::{JobxApiClient, get_jobx_api_client};

// 获取客户端实例
let client = get_jobx_api_client()?;

// 调用 Job API
// client.job_client.create_job(...).await?;
// client.task_client.list_tasks(...).await?;
// client.schedule_client.create_schedule(...).await?;
```

## 数据模型

### Job (任务定义)

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `i64` | 主键 |
| `name` | `String` | 任务名称 (唯一) |
| `group` | `String` | 任务分组 |
| `job_type` | `String` | 任务类型 |
| `params` | `Option<String>` | 任务参数 (JSON) |
| `remark` | `Option<String>` | 备注 |

### Schedule (调度)

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `i64` | 主键 |
| `job_id` | `i64` | 关联任务 ID |
| `cron` | `String` | Cron 表达式 |
| `enabled` | `bool` | 是否启用 |
| `remark` | `Option<String>` | 备注 |

### Task (任务执行)

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `i64` | 主键 |
| `job_id` | `i64` | 关联任务 ID |
| `status` | `String` | 执行状态 |
| `result` | `Option<String>` | 执行结果 |
| `remark` | `Option<String>` | 备注 |

> 所有实体均包含 `creator_id`、`create_ts`、`updator_id`、`update_ts` 审计字段。

## 配置热更新

JobX 支持运行时配置热更新，无需重启服务即可使配置变更生效。当配置文件发生变化时，服务会自动检测并重新加载配置。

## 许可证

MIT License
