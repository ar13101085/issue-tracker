//! One error type for every handler; always answered as JSON.

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use shared::{ErrorBody, FieldError};
use std::fmt;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized,
    Forbidden(String),
    NotFound,
    /// 422 with per-field messages.
    Validation(Vec<FieldError>),
    /// 429 with the seconds to wait.
    TooMany(u64),
    Internal(String),
}

impl ApiError {
    pub fn field(field: &str, message: impl Into<String>) -> Self {
        Self::Validation(vec![FieldError { field: field.into(), message: message.into() }])
    }
    pub fn forbidden(message: &str) -> Self {
        Self::Forbidden(message.into())
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRequest(m) | Self::Forbidden(m) => f.write_str(m),
            Self::Unauthorized => f.write_str("Requires authentication"),
            Self::NotFound => f.write_str("Not Found"),
            Self::Validation(_) => f.write_str("Validation Failed"),
            Self::TooMany(seconds) => write!(f, "Too many failed attempts. Try again in {} minutes.", seconds.div_ceil(60)),
            Self::Internal(_) => f.write_str("Internal Server Error"),
        }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::TooMany(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        if let Self::Internal(detail) = self {
            tracing::error!("internal error: {detail}");
        }
        let errors = match self {
            Self::Validation(e) => e.clone(),
            _ => Vec::new(),
        };
        HttpResponse::build(self.status_code()).json(ErrorBody { message: self.to_string(), errors })
    }
}

impl From<mongodb::error::Error> for ApiError {
    fn from(e: mongodb::error::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

/// True when an insert or update hit a unique index.
pub fn is_duplicate(e: &mongodb::error::Error) -> bool {
    use mongodb::error::{ErrorKind, WriteFailure};
    matches!(&*e.kind, ErrorKind::Write(WriteFailure::WriteError(w)) if w.code == 11000)
        || matches!(&*e.kind, ErrorKind::Command(c) if c.code == 11000)
}
