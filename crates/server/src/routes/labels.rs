use crate::access::load_repo;
use crate::auth::Viewer;
use crate::error::{is_duplicate, ApiError};
use crate::models::{LabelDoc, RepoDoc};
use crate::routes::issues::label_dto;
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use shared::{CreateLabel, LabelWithCount, Role, UpdateLabel};

type RepoPath = web::Path<(String, String)>;
type LabelPath = web::Path<(String, String, String)>;

/// What every new repository starts with, as on GitHub.
const DEFAULTS: &[(&str, &str, &str)] = &[
    ("bug", "d73a4a", "Something isn't working"),
    ("documentation", "0075ca", "Improvements or additions to documentation"),
    ("duplicate", "cfd3d7", "This issue already exists"),
    ("enhancement", "a2eeef", "New feature or request"),
    ("good first issue", "7057ff", "Good for newcomers"),
    ("help wanted", "008672", "Extra attention is needed"),
    ("invalid", "e4e669", "This doesn't seem right"),
    ("question", "d876e3", "Further information is requested"),
    ("wontfix", "ffffff", "This will not be worked on"),
];

pub async fn create_defaults(state: &AppState, repo: &RepoDoc) -> Result<(), ApiError> {
    let docs: Vec<LabelDoc> = DEFAULTS
        .iter()
        .map(|(name, color, description)| LabelDoc {
            id: ObjectId::new(),
            repo_id: repo.id,
            name: name.to_string(),
            name_lower: name.to_lowercase(),
            color: color.to_string(),
            description: description.to_string(),
        })
        .collect();
    state.db.labels.insert_many(docs).await?;
    Ok(())
}

fn check_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::field("name", "Name can't be blank"));
    }
    if name.chars().count() > 50 {
        return Err(ApiError::field("name", "Name is too long (maximum is 50 characters)"));
    }
    if name.contains('"') {
        return Err(ApiError::field("name", "Name can't contain a double quote"));
    }
    Ok(name.to_string())
}

fn check_color(color: &str) -> Result<String, ApiError> {
    let color = color.trim().trim_start_matches('#').to_lowercase();
    if color.len() == 6 && color.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(color)
    } else {
        Err(ApiError::field("color", "Color must be a 6 character hex code"))
    }
}

fn check_description(text: &str) -> Result<String, ApiError> {
    let text = text.trim();
    if text.chars().count() > 100 {
        return Err(ApiError::field("description", "Description is too long (maximum is 100 characters)"));
    }
    Ok(text.to_string())
}

pub async fn list(state: web::Data<AppState>, viewer: Viewer, path: RepoPath) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let docs: Vec<LabelDoc> =
        state.db.labels.find(doc! {"repo_id": access.repo.id}).sort(doc! {"name_lower": 1}).await?.try_collect().await?;
    let mut out = Vec::with_capacity(docs.len());
    for l in docs {
        let open_issues = state.db.issues.count_documents(doc! {"repo_id": access.repo.id, "state": "open", "label_ids": l.id}).await?;
        out.push(LabelWithCount { label: label_dto(&l), open_issues });
    }
    Ok(HttpResponse::Ok().json(out))
}

pub async fn create(state: web::Data<AppState>, viewer: Viewer, path: RepoPath, body: web::Json<CreateLabel>) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    access.require(Role::Write)?;
    let name = check_name(&body.name)?;
    let label = LabelDoc {
        id: ObjectId::new(),
        repo_id: access.repo.id,
        name_lower: name.to_lowercase(),
        name,
        color: check_color(&body.color)?,
        description: check_description(&body.description)?,
    };
    match state.db.labels.insert_one(&label).await {
        Ok(_) => Ok(HttpResponse::Created().json(label_dto(&label))),
        Err(e) if is_duplicate(&e) => Err(ApiError::field("name", "Name has already been taken")),
        Err(e) => Err(e.into()),
    }
}

async fn find(state: &AppState, repo: &RepoDoc, name: &str) -> Result<LabelDoc, ApiError> {
    state.db.labels.find_one(doc! {"repo_id": repo.id, "name_lower": name.to_lowercase()}).await?.ok_or(ApiError::NotFound)
}

pub async fn update(state: web::Data<AppState>, viewer: Viewer, path: LabelPath, body: web::Json<UpdateLabel>) -> Result<HttpResponse, ApiError> {
    let (owner, repo, name) = path.into_inner();
    let access = load_repo(&state, &owner, &repo, viewer.0).await?;
    access.require(Role::Write)?;
    let label = find(&state, &access.repo, &name).await?;
    let mut set = doc! {};
    if let Some(n) = &body.new_name {
        let n = check_name(n)?;
        set.insert("name_lower", n.to_lowercase());
        set.insert("name", n);
    }
    if let Some(c) = &body.color {
        set.insert("color", check_color(c)?);
    }
    if let Some(d) = &body.description {
        set.insert("description", check_description(d)?);
    }
    if !set.is_empty() {
        match state.db.labels.update_one(doc! {"_id": label.id}, doc! {"$set": set}).await {
            Ok(_) => {}
            Err(e) if is_duplicate(&e) => return Err(ApiError::field("name", "Name has already been taken")),
            Err(e) => return Err(e.into()),
        }
    }
    let label = state.db.labels.find_one(doc! {"_id": label.id}).await?.ok_or(ApiError::NotFound)?;
    crate::notify::signal_repo(&state, access.repo.id);
    Ok(HttpResponse::Ok().json(label_dto(&label)))
}

pub async fn delete(state: web::Data<AppState>, viewer: Viewer, path: LabelPath) -> Result<HttpResponse, ApiError> {
    let (owner, repo, name) = path.into_inner();
    let access = load_repo(&state, &owner, &repo, viewer.0).await?;
    access.require(Role::Write)?;
    let label = find(&state, &access.repo, &name).await?;
    state.db.issues.update_many(doc! {"repo_id": access.repo.id}, doc! {"$pull": {"label_ids": label.id}}).await?;
    state.db.labels.delete_one(doc! {"_id": label.id}).await?;
    crate::notify::signal_repo(&state, access.repo.id);
    Ok(HttpResponse::NoContent().finish())
}
