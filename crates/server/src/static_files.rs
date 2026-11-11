//! Serves the Next.js static export embedded into the binary.
//!
//! The frontend is a single-page app: any path that is not a real file gets
//! `index.html`, and the client router (frontend/lib/router.tsx) picks the view.

use actix_web::{http::header, HttpRequest, HttpResponse};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../frontend/out/"]
struct Assets;

fn respond(path: &str, data: Vec<u8>) -> HttpResponse {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Hashed Next.js assets are immutable; HTML must always revalidate.
    let cache = if path.starts_with("_next/static/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, mime.as_ref()))
        .insert_header((header::CACHE_CONTROL, cache))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("Referrer-Policy", "strict-origin-when-cross-origin"))
        .insert_header(("X-Frame-Options", "DENY"))
        .body(data)
}

fn file(path: &str) -> Option<Vec<u8>> {
    Assets::get(path).map(|f| f.data.into_owned())
}

pub async fn serve(req: HttpRequest) -> HttpResponse {
    let path = req.path().trim_start_matches('/');
    if path.contains("..") {
        return HttpResponse::BadRequest().finish();
    }
    if !path.is_empty() {
        if let Some(data) = file(path) {
            return respond(path, data);
        }
        // A missing build asset is a real 404; the shell would only hide the error.
        if path.starts_with("_next/") {
            return HttpResponse::NotFound().finish();
        }
    }
    match file("index.html") {
        Some(data) => respond("index.html", data),
        None => HttpResponse::InternalServerError().body("frontend is not built: run scripts/build-frontend.sh"),
    }
}
