//! Saved views: a named search of a repository's issues.
//!
//! A private view belongs to the person who made it and nobody else can see it
//! (others get 404). A shared view is visible to everyone who can see the
//! repository; write and up may create, change and delete shared views.

use crate::access::{load_repo, RepoAccess};
use crate::auth::Viewer;
use crate::error::ApiError;
use crate::models::{to_chrono, ViewDoc};
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use shared::{Role, SaveView, SavedView, UpdateView};

type RepoPath = web::Path<(String, String)>;
type ViewPath = web::Path<(String, String, String)>;

const MAX_VIEWS: u64 = 100;

fn may_edit(view: &ViewDoc, access: &RepoAccess) -> bool {
    if view.shared {
        access.has(Role::Write)
    } else {
        access.viewer.as_ref().is_some_and(|u| u.id == view.owner_id)
    }
}

fn dto(view: ViewDoc, access: &RepoAccess) -> SavedView {
    let viewer_can_edit = may_edit(&view, access);
    SavedView {
        id: view.id.to_hex(),
        name: view.name,
        description: view.description,
        query: view.query,
        shared: view.shared,
        viewer_can_edit,
        created_at: to_chrono(view.created_at),
    }
}

fn check_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::field("name", "Title can't be blank"));
    }
    if name.chars().count() > 100 {
        return Err(ApiError::field("name", "Title is too long (maximum is 100 characters)"));
    }
    Ok(name.to_string())
}

fn check_query(query: &str) -> Result<String, ApiError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(ApiError::field("query", "Query can't be blank"));
    }
    if query.chars().count() > 1000 {
        return Err(ApiError::field("query", "Query is too long (maximum is 1000 characters)"));
    }
    Ok(query.to_string())
}

fn check_description(description: &str) -> Result<String, ApiError> {
    let description = description.trim();
    if description.chars().count() > 500 {
        return Err(ApiError::field("description", "Description is too long (maximum is 500 characters)"));
    }
    Ok(description.to_string())
}

fn require_share(access: &RepoAccess) -> Result<(), ApiError> {
    if access.has(Role::Write) {
        Ok(())
    } else {
        Err(ApiError::forbidden("Only people with write access can manage shared views"))
    }
}

/// `GET /views`: shared views, then the caller's private ones.
pub async fn list(state: web::Data<AppState>, viewer: Viewer, path: RepoPath) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let mine = access.viewer.as_ref().map(|u| u.id);
    let filter = match mine {
        Some(id) => doc! {"repo_id": access.repo.id, "$or": [{"shared": true}, {"owner_id": id}]},
        None => doc! {"repo_id": access.repo.id, "shared": true},
    };
    let docs: Vec<ViewDoc> = state.db.views.find(filter).sort(doc! {"name_lower": 1}).await?.try_collect().await?;
    let items: Vec<SavedView> = docs.into_iter().map(|v| dto(v, &access)).collect();
    Ok(HttpResponse::Ok().json(items))
}

async fn find(state: &AppState, access: &RepoAccess, id: &str) -> Result<ViewDoc, ApiError> {
    let id = ObjectId::parse_str(id).map_err(|_| ApiError::NotFound)?;
    let view = state.db.views.find_one(doc! {"_id": id, "repo_id": access.repo.id}).await?.ok_or(ApiError::NotFound)?;
    // Someone else's private view does not exist as far as the caller can tell.
    if !view.shared && access.viewer.as_ref().map(|u| u.id) != Some(view.owner_id) {
        return Err(ApiError::NotFound);
    }
    Ok(view)
}

pub async fn get(state: web::Data<AppState>, viewer: Viewer, path: ViewPath) -> Result<HttpResponse, ApiError> {
    let (owner, name, id) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let view = find(&state, &access, &id).await?;
    Ok(HttpResponse::Ok().json(dto(view, &access)))
}

pub async fn create(state: web::Data<AppState>, viewer: Viewer, path: RepoPath, body: web::Json<SaveView>) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let user = access.require(Role::Read)?;
    if body.shared {
        require_share(&access)?;
    }
    let name = check_name(&body.name)?;
    let query = check_query(&body.query)?;
    let description = check_description(&body.description)?;
    let owned = doc! {"repo_id": access.repo.id, "owner_id": user.id};
    if state.db.views.count_documents(owned).await? >= MAX_VIEWS {
        return Err(ApiError::field("name", "You can save at most 100 views in a repository"));
    }
    let view = ViewDoc {
        id: ObjectId::new(),
        repo_id: access.repo.id,
        owner_id: user.id,
        name_lower: name.to_lowercase(),
        name,
        description,
        query,
        shared: body.shared,
        created_at: DateTime::now(),
    };
    state.db.views.insert_one(&view).await?;
    Ok(HttpResponse::Created().json(dto(view, &access)))
}

pub async fn update(state: web::Data<AppState>, viewer: Viewer, path: ViewPath, body: web::Json<UpdateView>) -> Result<HttpResponse, ApiError> {
    let (owner, name, id) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.user()?;
    let view = find(&state, &access, &id).await?;
    if !may_edit(&view, &access) {
        return Err(ApiError::forbidden("You do not have permission to change this view"));
    }
    let mut set = doc! {};
    if let Some(n) = &body.name {
        let n = check_name(n)?;
        set.insert("name_lower", n.to_lowercase());
        set.insert("name", n);
    }
    if let Some(q) = &body.query {
        set.insert("query", check_query(q)?);
    }
    if let Some(d) = &body.description {
        set.insert("description", check_description(d)?);
    }
    if let Some(shared) = body.shared {
        // Sharing a private view, or taking a shared one private, both touch what others see.
        if shared != view.shared {
            require_share(&access)?;
            set.insert("shared", shared);
            if !shared {
                // It becomes the private view of whoever took it out of the shared list.
                set.insert("owner_id", access.user()?.id);
            }
        }
    }
    if !set.is_empty() {
        state.db.views.update_one(doc! {"_id": view.id}, doc! {"$set": set}).await?;
    }
    let view = state.db.views.find_one(doc! {"_id": view.id}).await?.ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(dto(view, &access)))
}

pub async fn delete(state: web::Data<AppState>, viewer: Viewer, path: ViewPath) -> Result<HttpResponse, ApiError> {
    let (owner, name, id) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.user()?;
    let view = find(&state, &access, &id).await?;
    if !may_edit(&view, &access) {
        return Err(ApiError::forbidden("You do not have permission to delete this view"));
    }
    state.db.views.delete_one(doc! {"_id": view.id}).await?;
    Ok(HttpResponse::NoContent().finish())
}
