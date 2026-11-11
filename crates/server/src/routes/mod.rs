//! `/api` routes. Shapes follow GitHub's REST API where one exists.

pub mod attachments;
pub mod auth;
pub mod comments;
pub mod issue_actions;
pub mod issues;
pub mod labels;
pub mod milestones;
pub mod notifications;
pub mod projects;
pub mod repos;
pub mod users;

use actix_web::{web, HttpResponse};
use shared::Health;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health))
        .route("/auth/signup", web::post().to(auth::signup))
        .route("/auth/login", web::post().to(auth::login))
        .route("/auth/logout", web::post().to(auth::logout))
        .route("/auth/token", web::post().to(auth::token_login))
        .route("/user/tokens", web::get().to(auth::tokens))
        .route("/user/tokens", web::post().to(auth::create_token))
        .route("/user/tokens/{id}", web::delete().to(auth::revoke_token))
        .route("/user", web::get().to(auth::current))
        .route("/user", web::patch().to(auth::update))
        .route("/stream", web::get().to(notifications::stream))
        .route("/notifications", web::get().to(notifications::list))
        .route("/notifications/read", web::post().to(notifications::read_all))
        .route("/notifications/{id}", web::patch().to(notifications::update))
        .route("/markdown", web::post().to(comments::preview))
        .route("/user/repos", web::get().to(repos::mine))
        .route("/user/repos", web::post().to(repos::create))
        .route("/users/{login}", web::get().to(users::get))
        .route("/users/{login}/repos", web::get().to(repos::of_owner))
        .service(
            web::scope("/repos/{owner}/{repo}")
                .route("", web::get().to(repos::get))
                .route("", web::patch().to(repos::update))
                .route("", web::delete().to(repos::delete))
                .route("/collaborators", web::get().to(repos::collaborators))
                .route("/collaborators/{login}", web::put().to(repos::set_collaborator))
                .route("/collaborators/{login}", web::delete().to(repos::remove_collaborator))
                .route("/mentionables", web::get().to(comments::mentionables))
                .route("/assignees", web::get().to(repos::assignees))
                .route("/issue-templates", web::get().to(repos::templates))
                .route("/issue-templates", web::put().to(repos::set_templates))
                .service(
                    web::resource("/attachments")
                        .app_data(web::PayloadConfig::new(attachments::MAX_BYTES + 1024))
                        .route(web::post().to(attachments::upload)),
                )
                .route("/labels", web::get().to(labels::list))
                .route("/labels", web::post().to(labels::create))
                .route("/labels/{name}", web::patch().to(labels::update))
                .route("/labels/{name}", web::delete().to(labels::delete))
                .route("/milestones", web::get().to(milestones::list))
                .route("/milestones", web::post().to(milestones::create))
                .route("/milestones/{number}", web::get().to(milestones::get))
                .route("/milestones/{number}", web::patch().to(milestones::update))
                .route("/milestones/{number}", web::delete().to(milestones::delete))
                .route("/projects", web::get().to(projects::list))
                .route("/projects", web::post().to(projects::create))
                .route("/projects/{number}", web::get().to(projects::get))
                .route("/projects/{number}", web::patch().to(projects::update))
                .route("/projects/{number}", web::delete().to(projects::delete))
                .route("/projects/{number}/fields", web::post().to(projects::add_field))
                .route("/projects/{number}/fields/{field}", web::patch().to(projects::update_field))
                .route("/projects/{number}/fields/{field}", web::delete().to(projects::delete_field))
                .route("/projects/{number}/items", web::post().to(projects::add_item))
                .route("/projects/{number}/items/{item}", web::patch().to(projects::update_item))
                .route("/projects/{number}/items/{item}", web::delete().to(projects::remove_item))
                .route("/issues/pinned", web::get().to(issue_actions::pinned))
                .route("/issues/bulk", web::post().to(issue_actions::bulk))
                .route("/issues/comments/{id}/reactions/{content}", web::put().to(issue_actions::react_comment))
                .route("/issues/comments/{id}/reactions/{content}", web::delete().to(issue_actions::react_comment))
                .route("/issues/comments/{id}", web::patch().to(comments::update))
                .route("/issues/comments/{id}", web::delete().to(comments::delete))
                .route("/issues/comments/{id}/edits", web::get().to(comments::edits))
                .route("/issues", web::get().to(issues::list))
                .route("/issues", web::post().to(issues::create))
                .route("/issues/{number}", web::get().to(issues::get))
                .route("/issues/{number}", web::patch().to(issues::update))
                .route("/issues/{number}", web::delete().to(issues::delete))
                .route("/issues/{number}/edits", web::get().to(issues::edits))
                .route("/issues/{number}/timeline", web::get().to(comments::timeline))
                .route("/issues/{number}/comments", web::post().to(comments::create))
                .route("/issues/{number}/lock", web::put().to(issue_actions::lock))
                .route("/issues/{number}/lock", web::delete().to(issue_actions::unlock))
                .route("/issues/{number}/pin", web::put().to(issue_actions::pin))
                .route("/issues/{number}/pin", web::delete().to(issue_actions::unpin))
                .route("/issues/{number}/sub_issues", web::post().to(issue_actions::add_sub_issue))
                .route("/issues/{number}/sub_issues/{child}", web::delete().to(issue_actions::remove_sub_issue))
                .route("/issues/{number}/dependencies/blocked_by", web::post().to(issue_actions::add_blocked_by))
                .route("/issues/{number}/dependencies/blocked_by/{blocker}", web::delete().to(issue_actions::remove_blocked_by))
                .route("/issues/{number}/transfer", web::post().to(issue_actions::transfer))
                .route("/issues/{number}/projects", web::get().to(projects::of_issue))
                .route("/issues/{number}/subscription", web::put().to(notifications::set_subscription))
                .route("/issues/{number}/reactions/{content}", web::put().to(issue_actions::react_issue))
                .route("/issues/{number}/reactions/{content}", web::delete().to(issue_actions::react_issue)),
        );
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(Health { status: "ok".into(), version: env!("CARGO_PKG_VERSION").into() })
}

/// Unknown `/api` paths answer with JSON, never with the frontend shell.
pub async fn not_found() -> HttpResponse {
    HttpResponse::NotFound().json(serde_json::json!({ "message": "Not Found" }))
}
