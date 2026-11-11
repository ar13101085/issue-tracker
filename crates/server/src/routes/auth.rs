use crate::auth::{end_session, hash_password, new_token, start_session, verify_password, Viewer};
use crate::error::{is_duplicate, ApiError};
use crate::models::{to_chrono, TokenDoc, UserDoc};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use futures::TryStreamExt;
use shared::{AccessToken, CreateToken, CurrentUser, FieldError, LoginRequest, NewToken, SignupRequest, TokenLogin, TokenLoginResult, UpdateUser};

/// First path segments used by the frontend; they cannot be user names.
pub const RESERVED: &[&str] = &["login", "signup", "logout", "notifications", "new", "settings", "api", "_next"];

pub fn current_user(u: &UserDoc) -> CurrentUser {
    CurrentUser {
        login: u.login.clone(),
        name: u.name.clone(),
        email: u.email.clone(),
        is_admin: u.is_admin,
        email_notifications: u.email_notifications,
    }
}

/// GitHub's rule: letters, digits and single hyphens, not at either end, at most 39.
fn login_problem(login: &str) -> Option<&'static str> {
    if login.is_empty() || login.len() > 39 {
        return Some("Username must be between 1 and 39 characters");
    }
    if !login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        || login.starts_with('-')
        || login.ends_with('-')
        || login.contains("--")
    {
        return Some("Username may only contain alphanumeric characters or single hyphens, and cannot begin or end with a hyphen");
    }
    if RESERVED.contains(&login.to_lowercase().as_str()) {
        return Some("Username is reserved");
    }
    None
}

fn fail(errors: &mut Vec<FieldError>, field: &str, message: &str) {
    errors.push(FieldError { field: field.into(), message: message.into() });
}

pub async fn signup(state: web::Data<AppState>, body: web::Json<SignupRequest>) -> Result<HttpResponse, ApiError> {
    let body = body.into_inner();
    let first = state.db.users.count_documents(doc! {}).await? == 0;
    if !first && !state.cfg.allow_signup {
        return Err(ApiError::forbidden("Sign up is disabled on this server"));
    }
    let login = body.login.trim().to_string();
    let email = body.email.trim().to_lowercase();
    let mut errors: Vec<FieldError> = Vec::new();
    if let Some(p) = login_problem(&login) {
        fail(&mut errors, "login", p);
    }
    let email_ok = email.len() <= 254 && email.split_once('@').is_some_and(|(a, b)| !a.is_empty() && b.contains('.'));
    if !email_ok {
        fail(&mut errors, "email", "Email is invalid");
    }
    if body.password.len() < 8 || body.password.len() > 200 {
        fail(&mut errors, "password", "Password must be between 8 and 200 characters");
    }
    if errors.is_empty() {
        if state.db.users.find_one(doc! {"login_lower": login.to_lowercase()}).await?.is_some() {
            fail(&mut errors, "login", "Username is already taken");
        }
        if state.db.users.find_one(doc! {"email": &email}).await?.is_some() {
            fail(&mut errors, "email", "Email is already taken");
        }
    }
    if !errors.is_empty() {
        return Err(ApiError::Validation(errors));
    }
    let user = UserDoc {
        id: ObjectId::new(),
        login_lower: login.to_lowercase(),
        login,
        email,
        password_hash: hash_password(body.password).await?,
        name: None,
        is_admin: first,
        email_notifications: true,
        created_at: DateTime::now(),
    };
    match state.db.users.insert_one(&user).await {
        Ok(_) => {}
        Err(e) if is_duplicate(&e) => return Err(ApiError::field("login", "Username or email is already taken")),
        Err(e) => return Err(e.into()),
    }
    let cookie = start_session(&state, user.id).await?;
    Ok(HttpResponse::Created().cookie(cookie).json(current_user(&user)))
}

/// The account for a username-or-email and password, or the same error for either being wrong.
async fn check_password(state: &AppState, req: &HttpRequest, login: &str, password: String) -> Result<UserDoc, ApiError> {
    let id = login.trim().to_lowercase();
    // Guessing is counted per account and per address, so neither one account
    // nor one address can be hammered.
    let address = req.connection_info().realip_remote_addr().unwrap_or("unknown").to_string();
    let keys = [format!("login:{id}"), format!("ip:{address}")];
    if let Some(wait) = keys.iter().filter_map(|k| state.limiter.blocked(k)).max() {
        return Err(ApiError::TooMany(wait));
    }
    let user = state.db.users.find_one(doc! {"$or": [{"login_lower": &id}, {"email": &id}]}).await?;
    let ok = match &user {
        Some(u) => verify_password(password, u.password_hash.clone()).await?,
        None => false,
    };
    match user.filter(|_| ok) {
        Some(user) => {
            keys.iter().for_each(|k| state.limiter.succeeded(k));
            Ok(user)
        }
        None => {
            keys.iter().for_each(|k| state.limiter.failed(k));
            Err(ApiError::BadRequest("Incorrect username or password.".into()))
        }
    }
}

pub async fn login(state: web::Data<AppState>, req: HttpRequest, body: web::Json<LoginRequest>) -> Result<HttpResponse, ApiError> {
    let body = body.into_inner();
    let user = check_password(&state, &req, &body.login, body.password).await?;
    let cookie = start_session(&state, user.id).await?;
    Ok(HttpResponse::Ok().cookie(cookie).json(current_user(&user)))
}

pub async fn logout(state: web::Data<AppState>, req: HttpRequest, _viewer: Viewer) -> Result<HttpResponse, ApiError> {
    let cookie = end_session(&state, &req).await?;
    Ok(HttpResponse::NoContent().cookie(cookie).finish())
}

pub async fn current(viewer: Viewer) -> Result<HttpResponse, ApiError> {
    Ok(HttpResponse::Ok().json(current_user(&viewer.require()?)))
}

/// `PATCH /user`: display name and email preference.
pub async fn update(state: web::Data<AppState>, viewer: Viewer, body: web::Json<UpdateUser>) -> Result<HttpResponse, ApiError> {
    let user = viewer.require()?;
    let mut set = doc! {};
    if let Some(name) = &body.name {
        let name = name.trim();
        if name.chars().count() > 255 {
            return Err(ApiError::field("name", "Name is too long (maximum is 255 characters)"));
        }
        set.insert("name", if name.is_empty() { mongodb::bson::Bson::Null } else { name.into() });
    }
    if let Some(on) = body.email_notifications {
        set.insert("email_notifications", on);
    }
    if !set.is_empty() {
        state.db.users.update_one(doc! {"_id": user.id}, doc! {"$set": set}).await?;
    }
    let user = state.db.users.find_one(doc! {"_id": user.id}).await?.ok_or(ApiError::Unauthorized)?;
    Ok(HttpResponse::Ok().json(current_user(&user)))
}

fn token_dto(t: &TokenDoc) -> AccessToken {
    AccessToken {
        id: t.id.to_hex(),
        name: t.name.clone(),
        prefix: t.prefix.clone(),
        created_at: to_chrono(t.created_at),
        last_used_at: t.last_used_at.map(to_chrono),
    }
}

const MAX_TOKENS: u64 = 50;

async fn mint(state: &AppState, user: &UserDoc, name: &str) -> Result<NewToken, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(ApiError::field("name", "Name must be between 1 and 100 characters"));
    }
    if state.db.tokens.count_documents(doc! {"user_id": user.id}).await? >= MAX_TOKENS {
        return Err(ApiError::field("name", "You have 50 tokens already. Revoke one you no longer use."));
    }
    let (secret, hash, prefix) = new_token();
    let token = TokenDoc { id: ObjectId::new(), user_id: user.id, name: name.to_string(), hash, prefix, created_at: DateTime::now(), last_used_at: None };
    state.db.tokens.insert_one(&token).await?;
    Ok(NewToken { info: token_dto(&token), token: secret })
}

pub async fn tokens(state: web::Data<AppState>, viewer: Viewer) -> Result<HttpResponse, ApiError> {
    let user = viewer.require_session()?;
    let docs: Vec<TokenDoc> = state.db.tokens.find(doc! {"user_id": user.id}).sort(doc! {"created_at": -1}).await?.try_collect().await?;
    let list: Vec<AccessToken> = docs.iter().map(token_dto).collect();
    Ok(HttpResponse::Ok().json(list))
}

pub async fn create_token(state: web::Data<AppState>, viewer: Viewer, body: web::Json<CreateToken>) -> Result<HttpResponse, ApiError> {
    let user = viewer.require_session()?;
    Ok(HttpResponse::Created().json(mint(&state, &user, &body.name).await?))
}

/// `DELETE /user/tokens/{id}`. A token may revoke itself (`current`), which is how `it auth logout` works.
pub async fn revoke_token(state: web::Data<AppState>, viewer: Viewer, path: web::Path<String>, req: HttpRequest) -> Result<HttpResponse, ApiError> {
    let filter = if path.as_str() == "current" {
        let secret = req.headers().get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.split_once(' ')).map(|(_, t)| t.trim().to_string());
        let secret = secret.ok_or_else(|| ApiError::BadRequest("This request was not made with a token".into()))?;
        let user = viewer.require()?;
        doc! {"hash": crate::auth::token_id(&secret), "user_id": user.id}
    } else {
        let user = viewer.require_session()?;
        doc! {"_id": ObjectId::parse_str(path.as_str()).map_err(|_| ApiError::NotFound)?, "user_id": user.id}
    };
    if state.db.tokens.delete_one(filter).await?.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(HttpResponse::NoContent().finish())
}

/// `POST /auth/token`: username and password in, a new token out. No cookie is set.
pub async fn token_login(state: web::Data<AppState>, req: HttpRequest, body: web::Json<TokenLogin>) -> Result<HttpResponse, ApiError> {
    let body = body.into_inner();
    let user = check_password(&state, &req, &body.login, body.password).await?;
    let made = mint(&state, &user, &body.name).await?;
    Ok(HttpResponse::Created().json(TokenLoginResult { login: user.login, token: made.token }))
}
