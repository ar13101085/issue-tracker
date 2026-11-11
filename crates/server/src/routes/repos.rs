use crate::access::{load_repo, role_in};
use crate::auth::Viewer;
use crate::error::{is_duplicate, ApiError};
use crate::models::{to_chrono, CollaboratorDoc, RepoDoc, UserDoc};
use crate::routes::users::find_user;
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use shared::{Collaborator, CreateRepo, Repo, Role, SetCollaborator, Template, UpdateRepo, UserRef, ISSUE_TYPES};

type RepoPath = web::Path<(String, String)>;

fn name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() || name.len() > 100 {
        return Some("Repository name must be between 1 and 100 characters");
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) || name == "." || name == ".." {
        return Some("Repository name may only contain letters, digits, hyphens, underscores and periods");
    }
    None
}

pub async fn repo_dto(state: &AppState, repo: &RepoDoc, role: Option<Role>) -> Result<Repo, ApiError> {
    let open = state.db.issues.count_documents(doc! {"repo_id": repo.id, "state": "open"}).await?;
    Ok(Repo {
        owner: repo.owner_login.clone(),
        name: repo.name.clone(),
        description: repo.description.clone(),
        private: repo.private,
        created_at: to_chrono(repo.created_at),
        open_issues_count: open,
        viewer_role: role,
    })
}

/// Turns repositories into DTOs, dropping private ones the viewer has no role in.
async fn visible(state: &AppState, repos: Vec<RepoDoc>, viewer: Option<&UserDoc>) -> Result<Vec<Repo>, ApiError> {
    let mut out = Vec::new();
    for repo in repos {
        let role = role_in(state, &repo, viewer).await?;
        if repo.private && role.is_none() {
            continue;
        }
        out.push(repo_dto(state, &repo, role).await?);
    }
    Ok(out)
}

/// `GET /user/repos`: repositories the caller owns or collaborates on.
pub async fn mine(state: web::Data<AppState>, viewer: Viewer) -> Result<HttpResponse, ApiError> {
    let user = viewer.require()?;
    let collabs: Vec<CollaboratorDoc> = state.db.collaborators.find(doc! {"user_id": user.id}).await?.try_collect().await?;
    let ids: Vec<ObjectId> = collabs.iter().map(|c| c.repo_id).collect();
    let filter = doc! {"$or": [{"owner_id": user.id}, {"_id": {"$in": ids}}]};
    let repos: Vec<RepoDoc> = state.db.repos.find(filter).sort(doc! {"created_at": -1}).await?.try_collect().await?;
    Ok(HttpResponse::Ok().json(visible(&state, repos, Some(&user)).await?))
}

/// `GET /users/{login}/repos`
pub async fn of_owner(state: web::Data<AppState>, viewer: Viewer, path: web::Path<String>) -> Result<HttpResponse, ApiError> {
    let owner = find_user(&state, &path).await?;
    let repos: Vec<RepoDoc> =
        state.db.repos.find(doc! {"owner_id": owner.id}).sort(doc! {"created_at": -1}).await?.try_collect().await?;
    Ok(HttpResponse::Ok().json(visible(&state, repos, viewer.0.as_ref()).await?))
}

pub async fn create(state: web::Data<AppState>, viewer: Viewer, body: web::Json<CreateRepo>) -> Result<HttpResponse, ApiError> {
    let user = viewer.require()?;
    let body = body.into_inner();
    let name = body.name.trim().to_string();
    if let Some(p) = name_problem(&name) {
        return Err(ApiError::field("name", p));
    }
    if body.description.len() > 350 {
        return Err(ApiError::field("description", "Description is too long (maximum is 350 characters)"));
    }
    let repo = RepoDoc {
        id: ObjectId::new(),
        owner_id: user.id,
        owner_login: user.login.clone(),
        owner_lower: user.login_lower.clone(),
        name_lower: name.to_lowercase(),
        name,
        description: body.description.trim().to_string(),
        private: body.private,
        issue_seq: 0,
        milestone_seq: 0,
        project_seq: 0,
        templates: Vec::new(),
        created_at: DateTime::now(),
    };
    match state.db.repos.insert_one(&repo).await {
        Ok(_) => {}
        Err(e) if is_duplicate(&e) => return Err(ApiError::field("name", "The repository already exists on this account")),
        Err(e) => return Err(e.into()),
    }
    crate::routes::labels::create_defaults(&state, &repo).await?;
    Ok(HttpResponse::Created().json(repo_dto(&state, &repo, Some(Role::Admin)).await?))
}

pub async fn get(state: web::Data<AppState>, viewer: Viewer, path: RepoPath) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    Ok(HttpResponse::Ok().json(repo_dto(&state, &access.repo, access.role).await?))
}

pub async fn update(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: RepoPath,
    body: web::Json<UpdateRepo>,
) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    access.require(Role::Admin)?;
    let body = body.into_inner();
    let mut set = doc! {};
    if let Some(name) = body.name.as_deref().map(str::trim) {
        if let Some(p) = name_problem(name) {
            return Err(ApiError::field("name", p));
        }
        set.insert("name", name);
        set.insert("name_lower", name.to_lowercase());
    }
    if let Some(d) = body.description.as_deref().map(str::trim) {
        if d.len() > 350 {
            return Err(ApiError::field("description", "Description is too long (maximum is 350 characters)"));
        }
        set.insert("description", d);
    }
    if let Some(p) = body.private {
        set.insert("private", p);
    }
    if !set.is_empty() {
        match state.db.repos.update_one(doc! {"_id": access.repo.id}, doc! {"$set": set}).await {
            Ok(_) => {}
            Err(e) if is_duplicate(&e) => return Err(ApiError::field("name", "The repository already exists on this account")),
            Err(e) => return Err(e.into()),
        }
    }
    let repo = state.db.repos.find_one(doc! {"_id": access.repo.id}).await?.ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(repo_dto(&state, &repo, access.role).await?))
}

pub async fn delete(state: web::Data<AppState>, viewer: Viewer, path: RepoPath) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    access.require(Role::Admin)?;
    let id = access.repo.id;
    state.db.comments.delete_many(doc! {"repo_id": id}).await?;
    state.db.events.delete_many(doc! {"repo_id": id}).await?;
    state.db.labels.delete_many(doc! {"repo_id": id}).await?;
    state.db.projects.delete_many(doc! {"repo_id": id}).await?;
    state.db.project_items.delete_many(doc! {"repo_id": id}).await?;
    state.db.milestones.delete_many(doc! {"repo_id": id}).await?;
    state.db.reactions.delete_many(doc! {"repo_id": id}).await?;
    state.db.notifications.delete_many(doc! {"repo_id": id}).await?;
    let issue_ids: Vec<ObjectId> = state.db.issues.distinct("_id", doc! {"repo_id": id}).await?.into_iter().filter_map(|b| b.as_object_id()).collect();
    state.db.subscriptions.delete_many(doc! {"issue_id": {"$in": issue_ids}}).await?;
    state.db.issues.delete_many(doc! {"repo_id": id}).await?;
    state.db.collaborators.delete_many(doc! {"repo_id": id}).await?;
    state.db.repos.delete_one(doc! {"_id": id}).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub async fn collaborators(state: web::Data<AppState>, viewer: Viewer, path: RepoPath) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    access.require(Role::Admin)?;
    let docs: Vec<CollaboratorDoc> = state
        .db
        .collaborators
        .find(doc! {"repo_id": access.repo.id})
        .sort(doc! {"user_login": 1})
        .await?
        .try_collect()
        .await?;
    let list: Vec<Collaborator> = docs.into_iter().map(|c| Collaborator { login: c.user_login, role: c.role }).collect();
    Ok(HttpResponse::Ok().json(list))
}

pub async fn set_collaborator(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: web::Path<(String, String, String)>,
    body: web::Json<SetCollaborator>,
) -> Result<HttpResponse, ApiError> {
    let (owner, name, login) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Admin)?;
    let user = match find_user(&state, &login).await {
        Ok(u) => u,
        Err(ApiError::NotFound) => return Err(ApiError::field("login", "Could not find that user")),
        Err(e) => return Err(e),
    };
    if user.id == access.repo.owner_id {
        return Err(ApiError::field("login", "The repository owner already has full access"));
    }
    let role = mongodb::bson::to_bson(&body.role).map_err(|e| ApiError::Internal(e.to_string()))?;
    state
        .db
        .collaborators
        .update_one(
            doc! {"repo_id": access.repo.id, "user_id": user.id},
            doc! {"$set": {"role": role, "user_login": &user.login}, "$setOnInsert": {"_id": ObjectId::new()}},
        )
        .upsert(true)
        .await?;
    Ok(HttpResponse::Ok().json(Collaborator { login: user.login, role: body.role }))
}

pub async fn remove_collaborator(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse, ApiError> {
    let (owner, name, login) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Admin)?;
    let user = find_user(&state, &login).await?;
    state.db.collaborators.delete_one(doc! {"repo_id": access.repo.id, "user_id": user.id}).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// `GET /assignees`: who can be assigned, the owner first.
pub async fn assignees(state: web::Data<AppState>, viewer: Viewer, path: RepoPath) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let docs: Vec<CollaboratorDoc> = state
        .db
        .collaborators
        .find(doc! {"repo_id": access.repo.id})
        .sort(doc! {"user_login": 1})
        .await?
        .try_collect()
        .await?;
    let out: Vec<UserRef> = std::iter::once(access.repo.owner_login.clone())
        .chain(docs.into_iter().map(|c| c.user_login))
        .map(|login| UserRef { login })
        .collect();
    Ok(HttpResponse::Ok().json(out))
}

pub async fn templates(state: web::Data<AppState>, viewer: Viewer, path: RepoPath) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    Ok(HttpResponse::Ok().json(access.repo.templates))
}

/// `PUT /issue-templates`: replaces the whole list.
pub async fn set_templates(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: RepoPath,
    body: web::Json<Vec<Template>>,
) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    access.require(Role::Admin)?;
    let mut list = body.into_inner();
    if list.len() > 20 {
        return Err(ApiError::field("templates", "A repository can have at most 20 templates"));
    }
    let mut names: Vec<String> = Vec::new();
    for t in &mut list {
        t.name = t.name.trim().to_string();
        if t.name.is_empty() || t.name.chars().count() > 100 {
            return Err(ApiError::field("name", "Template name must be between 1 and 100 characters"));
        }
        if names.contains(&t.name.to_lowercase()) {
            return Err(ApiError::field("name", format!("There are two templates named \"{}\"", t.name)));
        }
        names.push(t.name.to_lowercase());
        if t.body.chars().count() > 65_536 || t.title.chars().count() > 256 || t.about.chars().count() > 200 {
            return Err(ApiError::field("body", format!("Template \"{}\" is too long", t.name)));
        }
        if let Some(kind) = &t.issue_type {
            t.issue_type = Some(
                ISSUE_TYPES
                    .iter()
                    .find(|k| k.eq_ignore_ascii_case(kind))
                    .map(|k| k.to_string())
                    .ok_or_else(|| ApiError::field("issue_type", format!("Issue type must be one of {}", ISSUE_TYPES.join(", "))))?,
            );
        }
        // Unknown label names would make every issue from the template fail.
        crate::routes::issues::resolve_labels(&state, &access.repo, &t.labels).await?;
    }
    let stored = mongodb::bson::to_bson(&list).map_err(|e| ApiError::Internal(e.to_string()))?;
    state.db.repos.update_one(doc! {"_id": access.repo.id}, doc! {"$set": {"templates": stored}}).await?;
    Ok(HttpResponse::Ok().json(list))
}
