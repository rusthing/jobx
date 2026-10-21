# jobx-web

JobX 调度控制台的浏览器端。面向的是**正在值班的人**：他需要一眼看出「哪些计划还活着、下一次分派在几秒后、刚才那条为什么失败」，而不是先读一份 API 文档。

它只跟 `jobx-svr` 的 REST 接口说话，不引入任何中间层。

```
jobx-svr (:9001)  ──HTTP──▶  jobx-web
   /jobx/job/*        计划 CRUD + 手动触发
   /jobx/task/*       执行记录
```

## 快速开始

```bash
pnpm install
pnpm dev          # http://localhost:5180
```

**默认是演示数据**（内存里的 14 条计划 + 24 小时执行历史），因为没配置后端地址时硬发请求只会白屏。
要连本地 `jobx-svr`：

```bash
VITE_API_BASE=/api pnpm dev          # 走 Vite 反代 → 127.0.0.1:9001
# 或指定远端
VITE_API_BASE=http://10.0.0.7:9001 pnpm dev
PROXY 目标不同时：VITE_PROXY_TARGET=http://10.0.0.7:9001 pnpm dev
```

顶栏右侧的数据源开关随时可以切回演示数据；选择记在 `localStorage`，切换时会清空查询缓存，不会把两边的数据混在一起。

```bash
pnpm build        # 产出 dist/
pnpm preview      # 本地预览构建产物
pnpm test         # cron 解析 / 预览 / 描述的单元测试
pnpm typecheck
```

部署时把 `dist/` 交给任意静态服务器，并将 `/jobx/*` 反代到服务端即可；`BrowserRouter` 需要 history fallback。

## 这个前端理解到的 JobX

三张表，一条链路：

| 概念 | 说明 |
| --- | --- |
| **计划（jobx_job）** | 「什么任务、谁来执行、多久一次」。`executor_code` 决定消息发到哪个 Redis Stream（`jobx:stream:{code}`）；`job_type` 是手动 / cron / 固定延迟 / 固定频率；`next_assign_ms` 与 `high_freq` 由服务端算，前端只读。 |
| **执行（jobx_task）** | 一次真实执行。调度器会**提前**建单（提前量 = `assign_lead_ms`），所以会看到「已分派但还没开始」的待执行态。 |
| **执行器（executor）** | 不落库，是编码约定；因此「执行器」页面展示的是按 `executor_code` 聚合的执行侧事实，而不是一份注册表。 |

界面按这条链路组织：

- **调度总览**：启用计划数、正在执行、24h 成功率、P95 耗时；按小时的成/败趋势；计划类型构成；即将分派（带倒计时，可直接触发）；最近失败（错误原文来自 `exec_detail`）。
- **任务计划**：搜索 + 状态/类型/执行器过滤，行内启停、触发、编辑、删除；抽屉里按类型给出不同的调度表单，cron 会实时算「接下来 4 次」。
- **执行记录**：状态/来源/执行器过滤，5 秒轮询（可关）；详情抽屉给出「分派 → 预计开始 → 开始执行 → 结束」的时间线，每步的间隔都算好了。
- **执行器**：每个编码一张卡，计划数、24h 执行量、成功率、平均耗时、24h 活跃度条。

## 接口约定（照抄后端，不猜）

- 响应统一是 `Ro<T>`：`result` 为 `1` 成功、`-1` 参数错、`-2` 警告、`-3` 失败，业务数据在 `extra`。`http.ts` 统一拆包，非 1 一律抛 `ApiError`。
- 每个请求必须带 `X-User-Id`（服务端 `get_current_user_id` 缺了直接 400）。顶栏「操作人」就是改这个头，写入 `creater_id` / `updater_id`。
- 所有 `u64`（雪花 ID、毫秒时间戳）在 JSON 里是**字符串**——19 位超过 `Number.MAX_SAFE_INTEGER`，前端全程按字符串处理，只在展示时格式化。
- 分页参数是 `_page`（从 1 起）/ `_size` / `_keyword` / `_orderBy`（`字段:asc|desc`，字段用数据库列名，如 `next_assign_ms:asc`）；响应是 `PageRx`：`{ pageNum, total, list }`。
- CRUD 是宏生成的：`GET /jobx/job/page`、`GET /jobx/job/list`、`GET /jobx/job/{id}`、`POST /jobx/job`、`PUT /jobx/job`、`DELETE /jobx/job/{id}`、`POST /jobx/job/trigger`。任务侧同构。
- 删除计划受外键 `on delete restrict` 保护：有执行记录就删不掉。界面照实提示，并建议改用「停用」，不假装成功。

## Cron 处理

后端 `cron` crate 是 **6 段（秒在前）**，和运维习惯的 5 段 crontab 不同。`src/lib/cron.ts` 自己实现了解析与预览：

- 接受 5 / 6 段，5 段自动补 `0` 秒，落库前 `normalizeCron()` 统一成 6 段；
- 支持 `*` `?` `a` `a-b` `*/n` `a-b/n` `a/n`、列表、`JAN`/`MON` 缩写、`7`=周日；
- 「接下来 N 次」按天推进 + 命中日展开时/分/秒，不是逐秒试探；
- 人话描述拿不准就返回 `null`（界面显示「自定义节奏」），宁可不说也不猜错；
- 不支持的 Quartz 语法（`L` `W` `#`）直接报错——后端也不支持，预览不该比服务端更乐观。

## 结构

```
src/
├── lib/
│   ├── api/         # types（契约）/ http（Ro + X-User-Id）/ live / mock / index（模式切换）
│   ├── cron.ts      # 解析、next runs、中文描述
│   ├── metrics.ts   # 总览聚合（纯函数）
│   ├── time.ts      # 时间与时长格式化
│   └── utils.ts
├── hooks/           # react-query 封装、useNow、防抖
├── components/
│   ├── ui/          # Button / Field / Switch / Badge / Panel / Drawer / Toast…
│   ├── layout/      # AppShell / Sidebar / Topbar / CommandPalette
│   ├── charts.tsx   # Sparkline / TrendChart / ActivityStrip（手写 SVG，无图表库）
│   └── domain.tsx   # 类型徽标、执行器 chip、调度节奏
└── features/        # overview / jobs / tasks / executors
```

技术栈：React 19 + TypeScript + Vite + Tailwind CSS v4 + TanStack Query。没有 UI 组件库和图表库——面板、抽屉、堆叠柱、命令面板都是本项目自己的代码，样式令牌集中在 `styles.css` 的 `@theme` 里。

## 已知边界

- 统计是**浏览器侧聚合**（最近 400 条执行记录）。后端没有统计接口，界面如实标注口径，不把样本当全量。
- 没有鉴权：`X-User-Id` 由使用者自己填，适合内网运维台；接入统一登录时替换 `getOperatorId()` 即可。
- 轮询而非推送。执行记录页 5s、其余 15s，页面隐藏时 `useNow` 停表、React Query 自动暂停。
