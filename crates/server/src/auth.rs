//! Sessions, passwords and the `Viewer` extractor.

use crate::error::ApiError;
use crate::models::{SessionDoc, UserDoc};
use crate::state::AppState;
use actix_web::cookie::{time::Duration as CookieDuration, Cookie, SameSite};
use actix_web::http::Method;
use actix_web::{dev::Payload, web, FromRequest, HttpRequest};
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::pin::Pin;

pub const COOKIE: &str = "it_session";
const SESSION_DAYS: i64 = 30;

/// The account making the request, if any. The second field is true when it
/// signed in with a personal access token rather than the browser session.
pub struct Viewer(pub Option<UserDoc>, pub bool);

impl Viewer {
    pub fn require(self) -> Result<UserDoc, ApiError> {
        self.0.ok_or(ApiError::Unauthorized)
    }

    /// For account security settings: a token must not be able to mint more tokens.
    pub fn require_session(self) -> Result<UserDoc, ApiError> {
        if self.1 {
            return Err(ApiError::forbidden("This needs a browser session, not an access token"));
        }
        self.require()
    }
}

pub const TOKEN_PREFIX: &str = "itp_";

/// A new token secret and what is stored for it: (secret, hash, display prefix).
pub fn new_token() -> (String, String, String) {
    let mut bytes = [0u8; 20];
    rand::thread_rng().fill_bytes(&mut bytes);
    let secret = format!("{TOKEN_PREFIX}{}", hex::encode(bytes));
    let prefix = secret.chars().take(TOKEN_PREFIX.len() + 4).collect();
    (secret.clone(), token_id(&secret), prefix)
}

/// The token from `Authorization: Bearer itp_...` (or GitHub's older `token itp_...`).
fn bearer(req: &HttpRequest) -> Option<&str> {
    let value = req.headers().get("authorization")?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    (scheme.eq_ignore_ascii_case("bearer") || scheme.eq_ignore_ascii_case("token")).then_some(token.trim())
}

pub fn token_id(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Cookie-authenticated writes must come from our own pages. `SameSite=Lax`
/// already keeps the cookie off cross-site POSTs; this also rejects a foreign Origin.
fn check_origin(req: &HttpRequest) -> Result<(), ApiError> {
    if matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return Ok(());
    }
    let Some(origin) = req.headers().get("origin").and_then(|v| v.to_str().ok()) else {
        return Ok(());
    };
    let origin_host = origin.split("://").nth(1).unwrap_or(origin);
    if origin_host.eq_ignore_ascii_case(req.connection_info().host()) {
        Ok(())
    } else {
        Err(ApiError::forbidden("Cross-origin request refused"))
    }
}

impl FromRequest for Viewer {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, ApiError>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            check_origin(&req)?;
            let state = req
                .app_data::<web::Data<AppState>>()
                .ok_or_else(|| ApiError::Internal("app state missing".into()))?;
            if let Some(secret) = bearer(&req) {
                // A token that is sent but wrong is an error, not an anonymous visit.
                let token = state.db.tokens.find_one(doc! {"hash": token_id(secret)}).await?.ok_or(ApiError::Unauthorized)?;
                let user = state.db.users.find_one(doc! {"_id": token.user_id}).await?.ok_or(ApiError::Unauthorized)?;
                state.db.tokens.update_one(doc! {"_id": token.id}, doc! {"$set": {"last_used_at": DateTime::now()}}).await?;
                return Ok(Viewer(Some(user), true));
            }
            let Some(cookie) = req.cookie(COOKIE) else {
                return Ok(Viewer(None, false));
            };
            let Some(session) = state.db.sessions.find_one(doc! {"_id": token_id(cookie.value())}).await? else {
                return Ok(Viewer(None, false));
            };
            if session.expires_at < DateTime::now() {
                return Ok(Viewer(None, false));
            }
            Ok(Viewer(state.db.users.find_one(doc! {"_id": session.user_id}).await?, false))
        })
    }
}

/// Creates a session and returns the cookie that carries it.
pub async fn start_session(state: &AppState, user_id: ObjectId) -> Result<Cookie<'static>, ApiError> {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    let expires = DateTime::from_millis(DateTime::now().timestamp_millis() + SESSION_DAYS * 86_400_000);
    state.db.sessions.insert_one(SessionDoc { id: token_id(&token), user_id, expires_at: expires }).await?;
    Ok(session_cookie(state, token, CookieDuration::days(SESSION_DAYS)))
}

pub async fn end_session(state: &AppState, req: &HttpRequest) -> Result<Cookie<'static>, ApiError> {
    if let Some(cookie) = req.cookie(COOKIE) {
        state.db.sessions.delete_one(doc! {"_id": token_id(cookie.value())}).await?;
    }
    Ok(session_cookie(state, String::new(), CookieDuration::ZERO))
}

fn session_cookie(state: &AppState, value: String, max_age: CookieDuration) -> Cookie<'static> {
    Cookie::build(COOKIE, value)
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.cfg.cookie_secure)
        .max_age(max_age)
        .finish()
}

pub async fn hash_password(password: String) -> Result<String, ApiError> {
    web::block(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default().hash_password(password.as_bytes(), &salt).map(|h| h.to_string())
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn verify_password(password: String, hash: String) -> Result<bool, ApiError> {
    web::block(move || match PasswordHash::new(&hash) {
        Ok(parsed) => Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}
