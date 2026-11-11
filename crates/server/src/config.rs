//! Settings read from the environment.

#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub mongodb_uri: String,
    pub mongodb_db: String,
    /// Send the session cookie only over HTTPS. Turn on behind TLS.
    pub cookie_secure: bool,
    /// Whether new accounts can be created after the first one.
    pub allow_signup: bool,
    /// Address people reach the site at; used for links in emails.
    pub public_url: String,
    /// Outgoing mail. Email notifications are off when `SMTP_HOST` is not set.
    pub smtp: Option<Smtp>,
}

#[derive(Clone)]
pub struct Smtp {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    /// The From header, e.g. `Issue Tracker <issues@example.com>`.
    pub from: String,
    /// `starttls` (default), `tls` or `none`.
    pub security: String,
}

fn var(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

fn flag(name: &str, default: bool) -> Result<bool, String> {
    match std::env::var(name) {
        Err(_) => Ok(default),
        Ok(v) => match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Ok(true),
            "0" | "false" | "no" => Ok(false),
            _ => Err(format!("{name} must be true or false, got {v}")),
        },
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let port = var("PORT", &shared::DEFAULT_PORT.to_string());
        let host = var("HOST", "127.0.0.1");
        let smtp = match std::env::var("SMTP_HOST").ok().filter(|h| !h.is_empty()) {
            None => None,
            Some(smtp_host) => {
                let security = var("SMTP_SECURITY", "starttls").to_lowercase();
                if !matches!(security.as_str(), "starttls" | "tls" | "none") {
                    return Err(format!("SMTP_SECURITY must be starttls, tls or none, got {security}"));
                }
                let default_port = if security == "tls" { "465" } else { "587" };
                let smtp_port = var("SMTP_PORT", default_port);
                Some(Smtp {
                    host: smtp_host,
                    port: smtp_port.parse().map_err(|_| format!("SMTP_PORT is not a port number: {smtp_port}"))?,
                    username: std::env::var("SMTP_USERNAME").ok().filter(|v| !v.is_empty()),
                    password: std::env::var("SMTP_PASSWORD").ok().filter(|v| !v.is_empty()),
                    from: std::env::var("SMTP_FROM").map_err(|_| "SMTP_FROM is required when SMTP_HOST is set".to_string())?,
                    security,
                })
            }
        };
        Ok(Self {
            public_url: var("PUBLIC_URL", &format!("http://{host}:{port}")).trim_end_matches('/').to_string(),
            smtp,
            host,
            port: port.parse().map_err(|_| format!("PORT is not a port number: {port}"))?,
            mongodb_uri: var("MONGODB_URI", "mongodb://127.0.0.1:27020"),
            mongodb_db: var("MONGODB_DB", "issue_tracker"),
            cookie_secure: flag("COOKIE_SECURE", false)?,
            allow_signup: flag("ALLOW_SIGNUP", true)?,
        })
    }
}
