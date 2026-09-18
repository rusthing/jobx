# jobx-svr

JobX 服务端应用，基于 Rust 生态构建的任务调度服务系统。提供任务的创建、调度、执行和监控功能，支持 Cron 表达式定时调度，通过 RESTful API 接口提供服务。

## 功能特性

- **任务管理** - 任务的 CRUD 操作，支持分组、类型和 JSON 参数
- **调度管理** - 基于 Cron 表达式的定时调度，支持启用/禁用
- **任务执行** - 任务执行实例记录，状态跟踪与结果记录
- **RESTful API** - 基于 Axum 的 HTTP API，自动生成 CRUD 接口
- **Swagger UI** - 集成 OpenAPI 文档，方便调试
- **配置热更新** - 运行时自动检测并重载配置变更
- **信号控制** - 支持 start / restart / stop / kill 信号
- **服务注册** - 支持 Consul 服务注册与配置中心
- **多数据库** - 支持 PostgreSQL / MySQL / SQLite

## 项目结构

```
src/
├── config/              # 应用配置
│   ├── app_config.rs    # 应用配置结构体
│   ├── jobx_config.rs   # JobX 业务配置 (调度器线程数、超时、重试)
│   └── mod.rs
├── dao/                 # 数据访问层 (基于 SeaORM)
│   ├── job_dao.rs       # 任务数据访问
│   ├── schedule_dao.rs  # 调度数据访问
│   ├── task_dao.rs      # 任务执行数据访问
│   └── mod.rs
├── svc/                 # 业务逻辑层
│   ├── job_svc.rs       # 任务服务
│   ├── schedule_svc.rs  # 调度服务
│   ├── task_svc.rs      # 任务执行服务
│   └── mod.rs
├── web/                 # Web 层
│   ├── api_doc/         # Swagger 文档定义
│   │   ├── job_api_doc.rs
│   │   ├── schedule_api_doc.rs
│   │   └── task_api_doc.rs
│   ├── ctrl/            # 控制器 (请求处理)
│   │   ├── job_ctrl.rs
│   │   ├── schedule_ctrl.rs
│   │   └── task_ctrl.rs
│   ├── router/          # 路由定义
│   │   ├── job_router.rs
│   │   ├── schedule_router.rs
│   │   └── task_router.rs
│   └── mod.rs
├── lib.rs               # 库入口
└── main.rs              # 应用入口
```

## 架构设计

项目采用分层架构，基于 [robotech](https://github.com/rusthing/robotech-rs) 宏驱动开发：

```
main.rs  ──→ config (配置加载与热更新)
         ──→ dao (SeaORM 数据访问)
         ──→ svc (业务逻辑)
         ──→ web/ctrl (请求处理)
         ──→ web/router (路由注册)
         ──→ web/api_doc (OpenAPI 文档)
```

通过 `#[svc]`、`#[dao]`、`#[ctrl]`、`#[router(crud)]` 等宏，自动生成 CRUD 接口、数据访问层和路由注册代码。

## 快速开始

### 前置条件

- Rust 1.80+ (edition 2024)
- PostgreSQL (或 MySQL / SQLite)

### 数据库准备

```sql
CREATE USER jobx WITH PASSWORD 'jobx';
CREATE DATABASE jobx OWNER jobx;
```

### 配置

编辑 `jobx-svr.toml`：

```toml
profile = "dev"

[log]
level = "debug"

[db]
url = "postgres://jobx:jobx@127.0.0.1:5432/jobx"

[jobx]
scheduler-workers = 4           # 调度器线程数，默认 4
task-timeout-seconds = 3600     # 任务执行超时(秒)，默认 3600
max-retry-count = 3             # 最大重试次数，默认 3
```

配置 `.env` 文件用于 sqlx 数据库迁移：

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

```bash
cargo run -- --signal start      # 启动 (默认)
cargo run -- --signal restart    # 优雅重启
cargo run -- --signal stop       # 优雅停止
cargo run -- --signal kill       # 强制终止
```

## API 接口

服务启动后，访问 Swagger UI：`http://localhost:8080/swagger-ui`

### Job API

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/job/list` | 分页查询任务列表 |
| `GET` | `/job/:id` | 查询单个任务 |
| `POST` | `/job` | 创建任务 |
| `PUT` | `/job/:id` | 更新任务 |
| `DELETE` | `/job/:id` | 删除任务 |

### Schedule API

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/schedule/list` | 分页查询调度列表 |
| `GET` | `/schedule/:id` | 查询单个调度 |
| `POST` | `/schedule` | 创建调度 |
| `PUT` | `/schedule/:id` | 更新调度 |
| `DELETE` | `/schedule/:id` | 删除调度 |

### Task API

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/task/list` | 分页查询执行列表 |
| `GET` | `/task/:id` | 查询单个执行 |
| `POST` | `/task` | 创建任务执行 |
| `PUT` | `/task/:id` | 更新任务执行 |
| `DELETE` | `/task/:id` | 删除任务执行 |

## 配置说明

### JobxConfig

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `scheduler-workers` | `usize` | 4 | 调度器工作线程数 |
| `task-timeout-seconds` | `u64` | 3600 | 任务执行超时时间 |
| `max-retry-count` | `u32` | 3 | 最大重试次数 |

### AppConfig

| 配置项 | 说明 |
|--------|------|
| `jobx` | JobX 业务配置 |
| `db` | 数据库连接配置 |
| `web` | Web 服务器配置 (端口、地址等) |
| `id_worker` | 分布式 ID 生成器配置 |

## 技术栈

| 类别 | 技术 |
|------|------|
| Web 框架 | [Axum](https://github.com/tokio-rs/axum) 0.8 |
| 异步运行时 | [Tokio](https://tokio.rs/) 1.x |
| ORM | [SeaORM](https://www.sea-ql.org/SeaORM/) 2.0 |
| SQL 驱动 | [sqlx](https://github.com/launchbadge/sqlx) 0.9 |
| API 文档 | [utoipa](https://github.com/juhaku/utoipa) + Swagger UI |
| 微服务框架 | [robotech](https://github.com/rusthing/robotech-rs) |
| 命令行 | [clap](https://docs.rs/clap/latest/clap/) |
| 日志 | [tracing](https://github.com/tokio-rs/tracing) |
| 配置管理 | [config](https://github.com/mehcode/config-rs) |
| 分布式 ID | [idworker](https://crates.io/crates/idworker) |
| 数据校验 | [validator](https://github.com/Keats/validator) |

## 许可证

MIT License
