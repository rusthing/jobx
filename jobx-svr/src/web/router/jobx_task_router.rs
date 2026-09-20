use robotech::macros::router;

#[router(crud, routes [
    ("/jobx/task/take", post(take)),
])]
struct JobxTaskRouter;