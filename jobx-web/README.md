# JobX Web 管理平台

JobX 任务调度系统的后台管理前端，对接 `jobx-svr` 的 RESTful API。
前端**独立运行**（开发用 Vite dev server，生产由静态托管 / 网关自行部署），不再编译进 `jobx-svr` 二进制。

## 技术栈

- React 19 + TypeScript + Vite
- Ant Design 5
- React Router 6（`BrowserRouter`）
- Axios
- pnpm（corepack）

## 开发

```bash
# 安装依赖（首次）
corepack pnpm install

# 启动开发服务器（默认 5173）
corepack pnpm run dev
```

开发服务器通过 Vite proxy 转发接口请求，绕开 CORS。

约定：**`VITE_API_BASE` 与 `vite.config.ts` 里 proxy 的 key 必须一致**。
接口路径本身不带前缀（如 `/jobx/job/page`），靠 `baseURL` 加上前缀后才会命中 proxy：

| 环境 | `VITE_API_BASE` | proxy key | 最终请求 |
|------|-----------------|-----------|----------|
| dev | `/jobx-svr` | `/jobx-svr` | `http://127.0.0.1:9001/jobx/job/page` |

若本地 `jobx-svr` 不在 9001，改 `vite.config.ts` 里的 `target`。

> ⚠️ 如果 `VITE_API_BASE` 为空、而 proxy 只匹配 `/jobx-svr`，请求 `/jobx/job/page` **不会**被转发，
> Vite 会按 SPA 规则回落 `index.html`（HTTP 200 + `text/html`）。axios 拿到的是 HTML 字符串，
> `Ro` 判定通过、`extra` 为 `undefined`，最终 `request()` 返回 `null` ——
> 表现为**页面数据静默为空且不报任何错**，极难排查。

## 构建

```bash
corepack pnpm install
corepack pnpm build      # 等价于 tsc -b && vite build，产物在 dist/
```

产物是纯静态文件，交给任意静态托管 / 网关即可，`jobx-svr` 不再内嵌前端。

### 页面路由

前端用 `BrowserRouter`，静态托管需要把未命中的路径回落 `index.html`：

| 路径 | 说明 |
|------|------|
| `/` | 页面入口，默认落在「计划管理」 |
| `/job` | 计划管理 |
| `/task` | 任务监控 |
| `/assets/*` | 带内容哈希的静态资源，可长缓存（`max-age=31536000, immutable`） |
| `/index.html` | 入口文件，需 `no-cache` |

注意**别让静态托管的回落规则吃掉接口路径**：`/jobx/**` 必须转发给 `jobx-svr`，
否则拼错接口路径时会拿到一个 200 的 HTML 页面而不是 404。

## 功能

- **计划管理**：增删改查任务计划（Job），支持 Cron / 固定延迟 / 固定频率 / 手动分派四种类型，可启用/停用、手动触发。
- **任务监控**：查看任务执行记录（Task），按状态/类型/执行器筛选，查看执行详情与耗时。
- **顶栏可配置 X-User-Id**：写操作必填，存在 `localStorage` 的 `jobx_user_id`，默认 `1`。

## 关键约定

- **ID 与时间戳全是字符串**：后端使用 19 位雪花 ID，超出 JS 安全整数范围，前端全程以 `string` 处理。
- **枚举值是变体名字符串**：如 `jobType: "FixedRate"`、`status: "Success"`、`taskType: "Immediate"`。
- **`_orderBy` 使用 snake_case 数据库列名**：如 `create_ms:desc`。
- **错误提示只在一个地方弹**：`src/api/http.ts` 统一判定后端 `Ro.result !== 1` 并 `message.error(msg)`，页面的 `catch` 只用于收尾（关 loading、关弹窗），不要再各自调 `message.error`。需要页面自己定制的传 `silent: true`。

## 目录结构

```
src/
├── api/
│   ├── http.ts        # axios 实例 + 全局错误处理（Ro.result 判定）
│   ├── job.ts         # 计划相关接口
│   ├── task.ts        # 任务相关接口
│   └── types.ts       # 手写的 TS 类型（Ro / VO / Query / DTO）
├── layouts/
│   └── BasicLayout.tsx  # 侧边菜单 + 顶栏用户 ID 配置
├── pages/
│   ├── job/index.tsx    # 计划管理
│   └── task/index.tsx   # 任务监控
├── router/index.tsx     # 路由表
├── utils/               # 枚举映射、时间/耗时格式化
└── main.tsx / App.tsx
```

## 已知限制

- 产物目前是单个 `index-*.js` chunk（约 1.2 MB / gzip 396 KB），antd 全量打包。要做首屏优化可在 `vite.config.ts` 里配 `build.rollupOptions.output.manualChunks`，把 antd / react 拆出去。
- 没有登录鉴权，`X-User-Id` 由顶栏手工指定，仅适用于内网/开发环境。
