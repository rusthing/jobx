/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 后端地址：留空 = 不连后端（默认走演示数据）；`/api` = 走 Vite 反代；生产通常是同源前缀 `/jobx-svr` */
  readonly VITE_API_BASE?: string;
  /** 页面挂载路径前缀（构建期 PUBLIC_BASE 的产物），由 Vite 注入，形如 `/jobx-web/` 或 `/` */
  readonly BASE_URL: string;
  /** `hash` = 用 hash 路由，省掉静态服务器的 SPA 回退配置 */
  readonly VITE_ROUTER?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
