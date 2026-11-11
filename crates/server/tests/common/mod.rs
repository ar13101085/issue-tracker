//! Test harness: each test gets its own MongoDB database (`it_test_*`) on the dev
//! container and drops it at the end.
#![allow(dead_code)]

use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::http::{Method, StatusCode};
use actix_web::test::TestRequest;
use actix_web::web;
use issue_tracker::config::Config;
use issue_tracker::state::AppState;
use serde_json::Value;

pub async fn state() -> web::Data<AppState> {
    let mut cfg = Config::from_env().expect("config");
    cfg.mongodb_db = format!("it_test_{:016x}", rand::random::<u64>());
    web::Data::new(issue_tracker::state::init(cfg).await.expect("MongoDB must be running (container it-mongo)"))
}

/// Like [`state`], with the outgoing mail queue handed to the test instead of an SMTP server.
pub async fn state_with_mail() -> (web::Data<AppState>, tokio::sync::mpsc::UnboundedReceiver<issue_tracker::mail::Mail>) {
    let mut cfg = Config::from_env().expect("config");
    cfg.mongodb_db = format!("it_test_{:016x}", rand::random::<u64>());
    let mut st = issue_tracker::state::init(cfg).await.expect("MongoDB must be running (container it-mongo)");
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    st.mail = Some(tx);
    (web::Data::new(st), rx)
}

pub async fn drop_db(state: &AppState) {
    state.db.database.drop().await.expect("drop test database");
}

#[macro_export]
macro_rules! app {
    ($state:expr) => {
        actix_web::test::init_service(
            actix_web::App::new().app_data($state.clone()).configure(issue_tracker::configure),
        )
        .await
    };
}

pub struct Reply {
    pub status: StatusCode,
    pub json: Value,
    /// Value of the session cookie, when the response set one.
    pub cookie: Option<String>,
}

pub async fn send<S, B>(app: &S, method: Method, path: &str, cookie: Option<&str>, body: Option<Value>) -> Reply
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let mut req = TestRequest::default().method(method).uri(path);
    if let Some(c) = cookie {
        req = req.insert_header(("cookie", format!("it_session={c}")));
    }
    if let Some(b) = body {
        req = req.set_json(b);
    }
    let res = actix_web::test::call_service(app, req.to_request()).await;
    let status = res.status();
    let cookie = res
        .response()
        .cookies()
        .find(|c| c.name() == "it_session")
        .map(|c| c.value().to_string());
    let bytes = actix_web::test::read_body(res).await;
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Reply { status, json, cookie }
}

/// Signs up `login` and returns its session cookie.
pub async fn signup<S, B>(app: &S, login: &str) -> String
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let body = serde_json::json!({"login": login, "email": format!("{login}@example.test"), "password": "correct horse"});
    let r = send(app, Method::POST, "/api/auth/signup", None, Some(body)).await;
    assert_eq!(r.status, StatusCode::CREATED, "signup {login}: {}", r.json);
    r.cookie.expect("session cookie")
}
