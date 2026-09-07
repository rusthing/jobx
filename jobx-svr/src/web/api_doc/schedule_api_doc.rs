use utoipa::openapi::OpenApi;

pub fn schedule_api_doc() -> (utoipa_swagger_ui::Url<'static>, OpenApi) {
    let url = utoipa_swagger_ui::Url::new("Schedule API", "/api-docs/schedule-api.json");
    let openapi = OpenApi::default();
    (url, openapi)
}
