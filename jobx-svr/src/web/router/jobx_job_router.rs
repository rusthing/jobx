use robotech::macros::router;

#[router(crud, routes [
    ("/jobx/job/trigger", post(trigger)),
])]
struct JobxJobRouter;