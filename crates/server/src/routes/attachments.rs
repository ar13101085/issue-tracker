//! Files dropped into comments, stored in MongoDB GridFS so the server stays a single binary.
//!
//! Uploads need a signed-in account with access to the repository. Downloads
//! check the repository again, so a private repository's images stay private.
//! Only real PNG, JPEG, GIF and WebP files are shown inline; everything else
//! is offered as a download and never rendered by the browser.

use crate::access::{load_repo, role_in};
use crate::auth::Viewer;
use crate::error::ApiError;
use crate::state::AppState;
use actix_web::http::header;
use actix_web::{web, HttpRequest, HttpResponse};
use futures::{AsyncReadExt, AsyncWriteExt};
use mongodb::bson::{doc, oid::ObjectId, Bson};
use serde::Deserialize;
use shared::{Attachment, Role};

pub const MAX_BYTES: usize = 10 * 1024 * 1024;

/// The image type the bytes really are, whatever the client claimed.
fn sniff_image(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn clean_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '-' }).collect();
    let cleaned = cleaned.trim_matches(|c| c == '.' || c == '-').to_string();
    if cleaned.is_empty() {
        "file".into()
    } else {
        cleaned.chars().take(100).collect()
    }
}

#[derive(Deserialize)]
pub struct UploadQuery {
    name: String,
}

/// `POST /repos/{owner}/{repo}/attachments?name=shot.png` with the file as the body.
pub async fn upload(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: web::Path<(String, String)>,
    query: web::Query<UploadQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let user = access.require(Role::Read)?;
    if body.is_empty() {
        return Err(ApiError::field("file", "The file is empty"));
    }
    if body.len() > MAX_BYTES {
        return Err(ApiError::field("file", "The file is larger than 10 MB"));
    }
    let name = clean_name(&query.name);
    let image = sniff_image(&body);
    let content_type = image.unwrap_or("application/octet-stream");

    let bucket = state.db.database.gridfs_bucket(None);
    let meta = doc! {"repo_id": access.repo.id, "user_id": user.id, "content_type": content_type};
    let mut stream = bucket.open_upload_stream(&name).metadata(meta).await?;
    let id = stream.id().as_object_id().ok_or_else(|| ApiError::Internal("GridFS id is not an ObjectId".into()))?;
    stream.write_all(&body).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    stream.close().await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let url = format!("/attachments/{}/{}", id.to_hex(), name);
    let markdown = if image.is_some() { format!("![{name}]({url})") } else { format!("[{name}]({url})") };
    Ok(HttpResponse::Created().json(Attachment { url, name, content_type: content_type.into(), size: body.len() as u64, markdown }))
}

/// `GET /attachments/{id}/{name}`
pub async fn download(state: web::Data<AppState>, viewer: Viewer, path: web::Path<(String, String)>, req: HttpRequest) -> Result<HttpResponse, ApiError> {
    let id = ObjectId::parse_str(&path.0).map_err(|_| ApiError::NotFound)?;
    let bucket = state.db.database.gridfs_bucket(None);
    let file = bucket.find_one(doc! {"_id": id}).await?.ok_or(ApiError::NotFound)?;
    let meta = file.metadata.unwrap_or_default();
    let repo_id = meta.get_object_id("repo_id").map_err(|_| ApiError::NotFound)?;
    let repo = state.db.repos.find_one(doc! {"_id": repo_id}).await?.ok_or(ApiError::NotFound)?;
    if repo.private && role_in(&state, &repo, viewer.0.as_ref()).await?.is_none() {
        return Err(ApiError::NotFound);
    }
    let etag = format!("\"{}\"", id.to_hex());
    if req.headers().get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(etag.as_str()) {
        return Ok(HttpResponse::NotModified().finish());
    }
    let mut data = Vec::with_capacity(file.length as usize);
    let mut stream = bucket.open_download_stream(Bson::ObjectId(id)).await?;
    stream.read_to_end(&mut data).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let content_type = meta.get_str("content_type").unwrap_or("application/octet-stream").to_string();
    let inline = content_type.starts_with("image/");
    let name = clean_name(file.filename.as_deref().unwrap_or("file"));
    let disposition = if inline { "inline".to_string() } else { format!("attachment; filename=\"{name}\"") };
    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, content_type))
        .insert_header((header::CONTENT_DISPOSITION, disposition))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("Content-Security-Policy", "default-src 'none'; sandbox"))
        .insert_header((header::CACHE_CONTROL, "private, max-age=31536000, immutable"))
        .insert_header((header::ETAG, etag))
        .body(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_made_safe() {
        assert_eq!(clean_name("../../etc/passwd"), "passwd");
        assert_eq!(clean_name("my shot (1).PNG"), "my-shot--1-.PNG");
        assert_eq!(clean_name("..\\..\\x\".html"), "x-.html");
        assert_eq!(clean_name("///"), "file");
    }

    #[test]
    fn only_real_images_are_images() {
        assert_eq!(sniff_image(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(sniff_image(b"GIF89a..."), Some("image/gif"));
        assert_eq!(sniff_image(b"<svg onload=alert(1)>"), None);
        assert_eq!(sniff_image(b"<html>"), None);
    }
}
