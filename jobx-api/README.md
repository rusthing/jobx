# jobx-api

JobX 项目的共享数据类型库，为 `jobx-svr`（服务端）和 `jobx-api-client`（客户端）提供统一的 DTO、VO 和 MO 类型定义。

## 功能

- **DTO** (Data Transfer Object) - 数据传输对象，用于 API 请求参数
- **VO** (View Object) - 视图对象，用于 API 响应数据
- **MO** (Model Object) - SeaORM 数据库实体模型 (需开启 `server` feature)

## 模块结构

```
src/
├── dto/          # 请求参数定义
│   ├── job_dto.rs
│   ├── schedule_dto.rs
│   └── task_dto.rs
├── vo/           # 响应数据定义
│   ├── job_vo.rs
│   ├── schedule_vo.rs
│   └── task_vo.rs
├── mo/           # 数据库实体 (仅 server feature)
│   ├── job.rs
│   ├── schedule.rs
│   ├── task.rs
│   └── prelude.rs
└── lib.rs
```

## Features

| Feature | 说明 | 引入的依赖 |
|---------|------|-----------|
| `server` | 启用服务端相关功能，包括 SeaORM 实体、对象映射和验证 | `sea-orm`, `o2o`, `validator` |

## 使用

### 作为客户端依赖 (默认)

```toml
[dependencies]
jobx-api = "0.1.0"
```

此时仅包含 `dto` 和 `vo` 模块，适用于 API 客户端场景。

### 作为服务端依赖

```toml
[dependencies]
jobx-api = { version = "0.1.0", features = ["server"] }
```

开启 `server` feature 后，额外包含 `mo` 模块（SeaORM 实体），支持完整的数据库操作和对象映射。

## 数据模型

### JobDto / JobVo

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `u64` | 主键 |
| `name` | `String` | 任务名称 |
| `group` | `String` | 任务分组 |
| `job_type` | `String` | 任务类型 |
| `params` | `Option<String>` | 任务参数 (JSON) |
| `remark` | `Option<String>` | 备注 |

### ScheduleDto / ScheduleVo

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `u64` | 主键 |
| `job_id` | `u64` | 关联任务 ID |
| `cron` | `String` | Cron 表达式 |
| `enabled` | `bool` | 是否启用 |
| `remark` | `Option<String>` | 备注 |

### TaskDto / TaskVo

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `u64` | 主键 |
| `job_id` | `u64` | 关联任务 ID |
| `status` | `String` | 执行状态 |
| `result` | `Option<String>` | 执行结果 |
| `remark` | `Option<String>` | 备注 |

## 技术栈

- [serde](https://serde.rs/) - 序列化/反序列化
- [utoipa](https://github.com/juhaku/utoipa) - OpenAPI 文档生成
- [sea-orm](https://www.sea-ql.org/SeaORM/) - ORM 实体 (可选)
- [o2o](https://crates.io/crates/o2o) - 对象间映射 (可选)
- [validator](https://github.com/Keats/validator) - 数据校验 (可选)

## 许可证

MIT License
