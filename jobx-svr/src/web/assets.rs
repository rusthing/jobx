//! # 前端静态资源
//!
//! 把 `jobx-web` 的构建产物（`dist/`）编译进 jobx-svr 二进制，由同一个进程对外提供页面，
//! 不需要额外部署 nginx 或静态托管，做到「一个二进制带着前端一起跑」。
//!
//! ## 工作方式
//!
//! - 用 `rust-embed` 在**编译期**把 `../jobx-web/dist` 下的所有文件读进二进制。
//!   开了 `debug-embed` 特性，debug 包同样是真嵌入，运行时不再依赖磁盘上的 `dist/`。
//! - 运行时只挂一个 `Router::fallback`：命中静态文件就按 MIME 返回文件内容，
//!   没命中（前端路由，如 `/`、`/job`、`/task`）则回落 `index.html`，
//!   交给前端 `BrowserRouter` 自己渲染。
//!
//! ## 注意事项
//!
//! 1. **前端必须先构建再编译**：`cd jobx-web && pnpm install && pnpm build`。
//!    `#[allow_missing = "true"]` 保证 `dist/` 不存在时 `cargo build` 不会失败，
//!    只是访问页面会得到「未构建」提示。
//! 2. **嵌入是编译期快照**：改完前端必须重新 `cargo build` 才生效。
//!    `build.rs` 已对 `../jobx-web/dist` 声明了 `rerun-if-changed`，
//!    重新 `cargo build` 会自动带上最新产物。
//! 3. **axum 规定同一层级只能有一个自定义 fallback**。本模块是全仓唯一设置
//!    `Router::fallback` 的地方。若将来别的模块也要设 fallback，`Router::merge`
//!    会直接 panic（`Cannot merge two Routers that both have a fallback`），
//!    届时需要把两处逻辑合并到同一台路由器上。

use axum::{
    Router,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use linkme::distributed_slice;
use robotech::web::ROUTER_SLICE;
use rust_embed::{EmbeddedFile, RustEmbed};

/// 内嵌的前端构建产物。
///
/// `folder` 只接受字符串字面量（rust-embed 的限制，常量不行），所以这里写死路径；
/// 它同时必须和 `build.rs` 里 `setup_web_frontend` 监听的目录一致
/// （`../jobx-web/dist`，相对本 crate 的 `CARGO_MANIFEST_DIR` 即 `jobx/jobx-svr/` 解析）。
///
/// `allow_missing` 让目录不存在时退化成「空资源表」而不是编译报错，
/// 这样没有装 node / 没跑 `pnpm build` 的环境照样能 `cargo build`。
#[derive(RustEmbed)]
#[folder = "../jobx-web/dist"]
#[allow_missing = true]
struct WebAssets;

/// 把静态资源路由器注册进 robotech 的全局路由切片。
///
/// 实现方式与 `#[router]` 宏生成的代码一致：让 `setup_web_server` 在遍历
/// `ROUTER_SLICE` 时把本路由器 merge 进主路由器。
#[distributed_slice(ROUTER_SLICE)]
static BUILD_WEB_ASSETS_ROUTER_FN: fn() -> Router = build_web_assets_router;

fn build_web_assets_router() -> Router {
    // 不注册任何显式路径，全部交给 fallback 分流：
    // 静态文件 → 返回文件；前端路由 → 回落 index.html；接口调用 → 保持 404。
    Router::new().fallback(web_assets_handler)
}

/// 静态资源 + SPA 路由回落处理器。
///
/// 由 `Router::fallback` 兜底，所有没被其它路由（`/jobx/**`、健康检查、swagger 等）
/// 命中的请求都会走到这里。
async fn web_assets_handler(uri: Uri, headers: axum::http::HeaderMap) -> Response {
    // `/assets/index-abc123.js` → `assets/index-abc123.js`
    let path = uri.path().trim_start_matches('/');

    // 1. 先当静态资源找，命中就直接返回文件内容
    if !path.is_empty()
        && let Some(file) = WebAssets::get(path)
    {
        return file_response(path, file);
    }

    // 2. 没命中再区分「浏览器导航」和「接口调用」：
    //    接口调用的 Accept 里没有 text/html，不能被前端页面冒充成 200，
    //    否则拼错接口路径时前端路由会吃掉错误、极难排查。
    if accepts_html(&headers) {
        if let Some(index) = WebAssets::get("index.html") {
            return file_response("index.html", index);
        }
        // dist/ 不存在或为空：给出可操作的提示，而不是一个莫名的白屏 404
        return not_built_hint();
    }

    (StatusCode::NOT_FOUND, "Not Found").into_response()
}

/// 判断请求方是否期望 HTML（即浏览器地址栏导航 / 刷新）。
fn accepts_html(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(',')
                .any(|part| part.trim().starts_with("text/html"))
        })
}

/// 组装静态文件响应：MIME + 缓存策略 + 内容。
fn file_response(path: &str, file: EmbeddedFile) -> Response {
    let headers = [
        (header::CONTENT_TYPE, content_type(path)),
        (header::CACHE_CONTROL, cache_control(path)),
    ];
    (StatusCode::OK, headers, file.data.into_owned()).into_response()
}

/// 按扩展名推断 MIME。
///
/// 只覆盖 Vite 产物里会出现的类型，其余一律按二进制流处理，
/// 避免为了几个类型把 `mime-guess` 拖进依赖树。
fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// 按路径推断缓存策略。
///
/// - `assets/` 下是 Vite 带内容哈希的产物，文件名变内容才变，可以长缓存。
/// - `index.html` 是入口，必须每次校验，否则前端发版后浏览器还会拿到旧页面。
/// - 其它散落文件（favicon 等）折中短缓存。
fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else if path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=3600"
    }
}

/// 前端没构建时的提示页。
fn not_built_hint() -> Response {
    const MSG: &str = "jobx-web 前端尚未构建。\n请先在 jobx-web 目录执行：pnpm install && pnpm build\n然后回到 jobx-svr 重新执行 cargo build。";
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        MSG,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(
            content_type("assets/index-abc123.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(content_type("assets/index-abc123.css"), "text/css; charset=utf-8");
        assert_eq!(content_type("vite.svg"), "image/svg+xml");
        assert_eq!(content_type("assets/font.woff2"), "font/woff2");
        assert_eq!(content_type("noext"), "application/octet-stream");
    }

    #[test]
    fn test_cache_control() {
        assert_eq!(
            cache_control("assets/index-abc123.js"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("favicon.ico"), "public, max-age=3600");
    }

    #[test]
    fn test_accepts_html() {
        let mut headers = axum::http::HeaderMap::new();
        assert!(!accepts_html(&headers));
        headers.insert(
            header::ACCEPT,
            "application/json, text/plain, */*".parse().unwrap(),
        );
        assert!(!accepts_html(&headers));
        headers.insert(
            header::ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
                .parse()
                .unwrap(),
        );
        assert!(accepts_html(&headers));
    }

    /// dist/ 未构建时不应 panic，只是拿不到文件。
    #[test]
    fn test_assets_missing_is_tolerated() {
        // 这个用例本身不校验内容（取决于 CI 是否构建了前端），只保证调用安全。
        let _ = WebAssets::get("index.html");
    }
}
