use utoipa::openapi::OpenApi;

pub fn job_api_doc() -> (utoipa_swagger_ui::Url<'static>, OpenApi) {
    let url = utoipa_swagger_ui::Url::new("Job API", "/api-docs/job-api.json");
    let openapi = OpenApi::default();
    (url, openapi)
}
