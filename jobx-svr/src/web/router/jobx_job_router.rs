// # 任务计划路由
//
// `/jobx/job` 下的一整套接口，外加手动触发与总览统计。
// 全局身份透传由 `crate::web::auth` 注册到框架的 `ROUTER_LAYER_SLICE`，
// 这里只声明路由，不再手工复制框架的 crud 路由表。
use robotech::macros::router;

#[router(crud, routes [
    ("/jobx/job/trigger", post(trigger)),
    ("/jobx/overview/summary", get(overview_summary)),
])]
struct JobxJobRouter;
