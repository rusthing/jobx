# JobX Web 管理平台

JobX 任务调度系统的后台管理前端，对接 `jobx-svr` 的 RESTful API。
构建产物由 `jobx-svr` 用 `rust-embed` **编译进二进制**，一个进程同时提供接口和页面，不需要额外部署 nginx 或静态托管。

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

开发服务器通过 Vite proxy 把 `/api` 转发到 `http://127.0.0.1:9001`，绕开 CORS。若本地 `jobx-svr` 不在 9001，改 `vite.config.ts` 里的 `target`。

## 构建与打包进 jobx-svr

```bash
corepack pnpm install
corepack pnpm build      # 等价于 tsc -b && vite build，产物在 dist/
cd ../jobx-svr && cargo build
```

`jobx-svr` 的 `build.rs` 已经把 `../jobx-web/dist` 纳入 `rerun-if-changed`：
- `dist/` 已存在 → 直接编译进二进制；
- `dist/` 不存在但 `node_modules/.bin/vite` 存在 → 自动跑一次 `vite build`；
- 两者都没有 → 只发 `cargo:warning` 并嵌入空资源表，**`cargo build` 照样成功**，访问页面会返回「未构建」提示。

想跳过自动构建：`JOBX_WEB_SKIP_BUILD=1 cargo build`。

注意 `rust-embed` 是**编译期快照**：改了前端必须重新 `cargo build` 才会生效。

### 页面访问方式

| 路径 | 说明 |
|------|------|
| `/` | 页面入口，默认落在「计划管理」 |
| `/job` | 计划管理（前端路由，服务端回落 `index.html`） |
| `/task` | 任务监控（同上） |
| `/assets/*` | 带内容哈希的静态资源，`Cache-Control: max-age=31536000, immutable` |
| `/index.html` | 入口文件，`Cache-Control: no-cache` |

未命中前端路由的**接口调用**不会被前端页面冒充：`Accept` 里没有 `text/html` 时统一返回 404，拼错接口路径不会拿到一个 200 的 HTML。

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
