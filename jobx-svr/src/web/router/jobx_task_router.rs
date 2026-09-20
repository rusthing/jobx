use robotech::macros::router;

#[router(crud, routes [
    ("/jobx/task/dispatch", post(dispatch)),
])]
struct JobxTaskRouter;