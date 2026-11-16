//! # 身份透传（不做鉴权）
//!
//! jobx-svr 不校验令牌、不判断权限点：登录与授权统一由 gatex 网关负责，服务只关心
//! 「这条请求的操作人是谁」。网关校验通过后把用户 ID 写进 `X-User-Id`，
//! 控制器继续用 `get_current_user_id(&headers)` 读取，业务层不需要改动。
//!
//! ## 两种运行环境
//!
//! * `profile = dev`（`APP_PROFILE=dev`）：本地开发免登录，操作人固定为系统账号 `0`。
//!   客户端带来的 `X-User-Id` 一律被覆盖，避免有人借开发模式冒充任意账号。
//! * 其它环境（缺省 `prod`）：要求请求头带 `X-User-Id`，缺失或不是合法 `u64`
//!   直接返回 401。没有这个头说明请求没经过网关，不能放行。
//!
//! ## 为什么是中间件而不是控制器里逐个校验
//!
//! 读取类接口（列表、详情、统计）本来不读 `X-User-Id`，只在写接口校验等于
//! 「未登录也能看全量数据与执行详情」。这里通过框架的 `ROUTER_LAYER_SLICE`
//! 统一包住全部业务路由，行为才能一致。
//!
//! ## 覆盖范围与放置位置
//!
//! 框架在合并完业务路由、注册健康检查与 Swagger 之前应用这个钩子，
//! 因此健康检查（启动自检 / k8s 探针）与 Swagger UI 不会被拦，CORS 预检也在更外层
//! 短路。jobx 用 `route_layer` 而不是 `layer`：只包已注册路由，未知路径仍是 404。

use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use linkme::distributed_slice;
use robotech::api::Ro;
use robotech::cst::user_id_cst::USER_ID_HEADER_NAME;
use robotech::web::ROUTER_LAYER_SLICE;

/// # 把身份透传注册为框架级业务路由中间件
///
/// 只在启动合并路由时被调用一次（每个注册项一个函数指针），不涉及请求路径。
#[distributed_slice(ROUTER_LAYER_SLICE)]
static REQUIRE_IDENTITY_LAYER: fn(Router) -> Router = require_identity_layer;

/// 用 `route_layer` 而不是 `layer`：不包 fallback，未匹配路径仍返回 404 而不是 401。
fn require_identity_layer(router: Router) -> Router {
    router.route_layer(axum::middleware::from_fn(require_identity))
}

/// # 身份透传中间件
///
/// 校验/补齐 `X-User-Id` 后交给下游处理器；失败时返回 401 + `Ro.result = -2`。
pub async fn require_identity(mut request: Request, next: Next) -> Response {
    match identity_header(request.headers(), crate::config::is_dev_profile()) {
        Ok(value) => {
            request.headers_mut().insert(USER_ID_HEADER_NAME, value);
            next.run(request).await
        }
        Err(detail) => unauthorized(&detail),
    }
}

/// # 解析当前请求的操作人
///
/// ## 参数
/// * `headers` - 原始请求头
/// * `dev` - 是否开发环境（免登录）
///
/// ## 返回值
/// * `Ok(HeaderValue)` - 要写回请求头的用户 ID
/// * `Err(String)` - 拒绝原因（写进 `Ro.detail`）
fn identity_header(headers: &HeaderMap, dev: bool) -> Result<HeaderValue, String> {
    if dev {
        return Ok(HeaderValue::from_static("0"));
    }

    let value = headers.get(USER_ID_HEADER_NAME).ok_or_else(|| {
        format!("请求头缺少 {USER_ID_HEADER_NAME}：jobx 只接受网关注入的身份")
    })?;
    let text = value
        .to_str()
        .map_err(|_| format!("请求头 {USER_ID_HEADER_NAME} 不是合法的 ASCII 文本"))?;
    // 只做形状校验：真正解析用户还会由 `get_current_user_id()` 再做一次，
    // 这里提前拦是为了让读取类接口也能在身份缺失时统一 401。
    text.trim()
        .parse::<u64>()
        .map_err(|_| format!("请求头 {USER_ID_HEADER_NAME} 应为 u64，实际收到 {text:?}"))?;

    Ok(value.clone())
}

/// 统一 401 响应
fn unauthorized(detail: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(Ro::<()>::warn("未登录或身份缺失".to_string()).detail(Some(detail.to_string()))),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(user_id: Option<&str>) -> HeaderMap {
        let mut map = HeaderMap::new();
        if let Some(user_id) = user_id {
            map.insert(USER_ID_HEADER_NAME, HeaderValue::from_str(user_id).expect("header"));
        }
        map
    }

    #[test]
    fn dev_overrides_client_supplied_identity() {
        let value = identity_header(&headers(Some("12345")), true).unwrap();
        assert_eq!(value, "0");
        assert_eq!(identity_header(&HeaderMap::new(), true).unwrap(), "0");
    }

    #[test]
    fn prod_requires_identity_header() {
        let error = identity_header(&HeaderMap::new(), false).unwrap_err();
        assert!(error.contains(USER_ID_HEADER_NAME), "{error}");
    }

    #[test]
    fn prod_passes_through_gateway_identity() {
        assert_eq!(identity_header(&headers(Some("10086")), false).unwrap(), "10086");
    }

    #[test]
    fn prod_rejects_malformed_identity() {
        assert!(identity_header(&headers(Some("abc")), false).is_err());
        assert!(identity_header(&headers(Some("")), false).is_err());
    }
}
