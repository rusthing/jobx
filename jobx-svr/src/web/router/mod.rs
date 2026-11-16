mod jobx_group_router;
mod jobx_job_router;
mod jobx_task_router;

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use axum::Router;
    use robotech::web::{ROUTER_LAYER_SLICE, ROUTER_SLICE};
    use tower::ServiceExt;

    /// 框架 `#[router(crud)]` 生成的 23 条路由（路径后缀 + 已注册方法）。
    ///
    /// 方法只用于说明每条路由注册了什么；探测统一用 crud 不会注册的 `PATCH`，
    /// 命中 405 说明路径存在，404 说明缺失，且不会执行任何 handler。
    const CRUD_ROUTES: [(&str, Method); 23] = [
        ("", Method::POST),
        ("", Method::PUT),
        ("", Method::GET),
        ("", Method::DELETE),
        ("/{id}", Method::DELETE),
        ("/{id}", Method::GET),
        ("/save", Method::POST),
        ("/upsert", Method::POST),
        ("/sync", Method::POST),
        ("/ex", Method::GET),
        ("/ex/{id}", Method::GET),
        ("/list", Method::GET),
        ("/list-ex", Method::GET),
        ("/page", Method::GET),
        ("/page-ex", Method::GET),
        ("/search", Method::GET),
        ("/search/{id}", Method::GET),
        ("/search/ex", Method::GET),
        ("/search/ex/{id}", Method::GET),
        ("/search/list", Method::GET),
        ("/search/list-ex", Method::GET),
        ("/search/page", Method::GET),
        ("/search/page-ex", Method::GET),
    ];

    /// 按 `setup_web_server` 的顺序组装业务路由：先合并 `ROUTER_SLICE`，再应用 `ROUTER_LAYER_SLICE`
    fn app_router() -> Router {
        let mut router = Router::new();
        for build_router in ROUTER_SLICE.iter() {
            router = router.merge(build_router());
        }
        for apply_layer in ROUTER_LAYER_SLICE.iter() {
            router = apply_layer(router);
        }
        router
    }

    async fn status(router: Router, method: Method, uri: &str) -> StatusCode {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            // 生产口径要求身份头；这里只验证路由存在，带一个合法值越过鉴权
            .header("X-User-Id", "0")
            .body(Body::empty())
            .expect("request");
        router.oneshot(request).await.expect("response").status()
    }

    /// 69 条 crud 路由（23 条 × 3 个资源）都必须真实存在
    ///
    /// 之前这些路由是靠 jobx 手工复制框架路由表注册的，框架增删路由时只能人工同步；
    /// 恢复成 `#[router(crud)]` 后，这个用例负责证明「不再依赖手工副本」。
    #[tokio::test]
    async fn all_crud_routes_are_registered() {
        for prefix in ["/jobx/job", "/jobx/task", "/jobx/group"] {
            for (suffix, _registered) in CRUD_ROUTES {
                let uri = format!("{prefix}{suffix}");
                let code = status(app_router(), Method::PATCH, &uri).await;
                assert_eq!(
                    code,
                    StatusCode::METHOD_NOT_ALLOWED,
                    "{uri} 未注册（期望 405，实际 {code}）"
                );
            }
        }
    }

    /// 3 条自定义路由（触发、领取、总览）同样由框架宏注册
    #[tokio::test]
    async fn custom_routes_are_registered() {
        for uri in [
            "/jobx/job/trigger",
            "/jobx/task/take",
            "/jobx/overview/summary",
        ] {
            let code = status(app_router(), Method::PATCH, uri).await;
            assert_eq!(
                code,
                StatusCode::METHOD_NOT_ALLOWED,
                "{uri} 未注册（期望 405，实际 {code}）"
            );
        }
    }

    /// 身份中间件用 `route_layer` 注册：未匹配路径保持 404，不会被改写成 401
    ///
    /// 注意不能用 `/jobx/job/not-a-route` 这类路径——它会被 `/{id}` 路由匹配到，
    /// 属于已注册路由，理应进入鉴权。
    #[tokio::test]
    async fn unknown_path_stays_404() {
        let request = Request::builder()
            .uri("/not-a-jobx-route")
            .body(Body::empty())
            .expect("request");
        let response = app_router().oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
