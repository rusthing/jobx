// # 执行记录路由
//
// `/jobx/task` 下的一整套接口，外加 worker 领取任务的 `take`。
// 全局身份透传由 `crate::web::auth` 统一注册；worker 的 feign 自带 `X-User-Id`，
// 与网关注入的身份头约定一致。
use robotech::macros::router;

#[router(crud, routes [
    ("/jobx/task/take", post(take)),
])]
struct JobxTaskRouter;
