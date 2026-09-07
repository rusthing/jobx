use utoipa::openapi::OpenApi;

pub fn task_api_doc() -> (utoipa_swagger_ui::Url<'static>, OpenApi) {
    let url = utoipa_swagger_ui::Url::new("Task API", "/api-docs/task-api.json");
    let openapi = OpenApi::default();
    (url, openapi)
}
