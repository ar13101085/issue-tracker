pub mod access;
pub mod auth;
pub mod config;
pub mod error;
pub mod mail;
pub mod markdown;
pub mod models;
pub mod notify;
pub mod ratelimit;
pub mod routes;
pub mod state;
pub mod static_files;

use actix_web::web;

/// Everything the server answers: `/api` plus the embedded frontend.
/// The caller adds `web::Data<state::AppState>`.
pub fn configure(cfg: &mut web::ServiceConfig) {
    let json = web::JsonConfig::default()
        .limit(1024 * 1024)
        .error_handler(|err, _| error::ApiError::BadRequest(format!("invalid JSON body: {err}")).into());
    let query = web::QueryConfig::default()
        .error_handler(|err, _| error::ApiError::BadRequest(format!("invalid query: {err}")).into());
    cfg.app_data(json)
        .app_data(query)
        .service(web::scope("/api").configure(routes::configure).default_service(web::to(routes::not_found)))
        .route("/attachments/{id}/{name}", web::get().to(routes::attachments::download))
        .default_service(web::to(static_files::serve));
}
