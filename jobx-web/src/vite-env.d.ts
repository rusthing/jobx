/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 后端地址：留空 = 不连后端（默认走演示数据）；`/api` = 走 Vite 反代 */
  readonly VITE_API_BASE?: string;
  /** 若设置，则忽略演示模式默认值，强制走真实接口 */
  readonly VITE_FORCE_LIVE?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
