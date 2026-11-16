// # 任务分组路由
//
// `/jobx/group` 下的一整套接口；全局身份透传由 `crate::web::auth` 统一注册。
use robotech::macros::router;

#[router(crud)]
struct JobxGroupRouter;
