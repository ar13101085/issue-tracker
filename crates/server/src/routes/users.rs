use crate::error::ApiError;
use crate::models::{to_chrono, UserDoc};
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use mongodb::bson::doc;
use shared::User;

pub async fn find_user(state: &AppState, login: &str) -> Result<UserDoc, ApiError> {
    state.db.users.find_one(doc! {"login_lower": login.to_lowercase()}).await?.ok_or(ApiError::NotFound)
}

pub async fn get(state: web::Data<AppState>, path: web::Path<String>) -> Result<HttpResponse, ApiError> {
    let u = find_user(&state, &path).await?;
    Ok(HttpResponse::Ok().json(User {
        login: u.login,
        name: u.name,
        is_admin: u.is_admin,
        created_at: to_chrono(u.created_at),
    }))
}
