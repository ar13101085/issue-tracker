//! HTTP client for the tracker API.

use serde::de::DeserializeOwned;
use serde::Serialize;
use shared::ErrorBody;

pub struct Client {
    http: reqwest::Client,
    pub host: String,
    token: Option<String>,
}

impl Client {
    pub fn new(host: String, token: Option<String>) -> Self {
        let http = reqwest::Client::builder().user_agent(concat!("it/", env!("CARGO_PKG_VERSION"))).build().expect("HTTP client");
        Self { http, host, token }
    }

    async fn send(&self, method: reqwest::Method, path: &str, body: Option<serde_json::Value>) -> Result<reqwest::Response, String> {
        let url = format!("{}/api{path}", self.host);
        let mut req = self.http.request(method, &url);
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        if let Some(body) = body {
            req = req.json(&body);
        }
        let res = req.send().await.map_err(|e| format!("cannot reach {}: {}", self.host, root_cause(&e)))?;
        if res.status().is_success() {
            return Ok(res);
        }
        let status = res.status();
        let body: Option<ErrorBody> = res.json().await.ok();
        Err(match (status.as_u16(), body) {
            (401, _) if self.token.is_none() => "you are not signed in. Run `it auth login`".to_string(),
            (401, _) => "your token was not accepted. Run `it auth login` again".to_string(),
            (404, _) => "not found (or you do not have access to it)".to_string(),
            (_, Some(b)) if !b.errors.is_empty() => b.errors.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("; "),
            (_, Some(b)) => b.message,
            (code, None) => format!("the server answered {code}"),
        })
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let res = self.send(reqwest::Method::GET, path, None).await?;
        res.json().await.map_err(|e| format!("unexpected answer from the server: {e}"))
    }

    /// Sends a JSON body and reads a JSON answer.
    pub async fn call<T: DeserializeOwned>(&self, method: reqwest::Method, path: &str, body: &impl Serialize) -> Result<T, String> {
        let body = serde_json::to_value(body).map_err(|e| e.to_string())?;
        let res = self.send(method, path, Some(body)).await?;
        res.json().await.map_err(|e| format!("unexpected answer from the server: {e}"))
    }

    /// For requests whose answer does not matter (204, or a body we ignore).
    pub async fn run(&self, method: reqwest::Method, path: &str, body: Option<serde_json::Value>) -> Result<(), String> {
        self.send(method, path, body).await.map(|_| ())
    }
}

fn root_cause(e: &reqwest::Error) -> String {
    let mut source: &dyn std::error::Error = e;
    while let Some(next) = source.source() {
        source = next;
    }
    source.to_string()
}

/// Percent-encodes one path segment or query value.
pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}
