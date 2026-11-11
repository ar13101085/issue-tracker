use crate::access::{load_repo, RepoAccess};
use crate::auth::Viewer;
use crate::error::{is_duplicate, ApiError};
use crate::models::{to_chrono, MilestoneDoc};
use crate::routes::issues::bson;
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime};
use mongodb::options::ReturnDocument;
use serde::Deserialize;
use shared::{CreateMilestone, IssueState, Milestone, Role, UpdateMilestone};

type RepoPath = web::Path<(String, String)>;
type MilestonePath = web::Path<(String, String, i64)>;

async fn dto(state: &AppState, m: MilestoneDoc) -> Result<Milestone, ApiError> {
    let open_issues = state.db.issues.count_documents(doc! {"milestone_id": m.id, "state": "open"}).await?;
    let closed_issues = state.db.issues.count_documents(doc! {"milestone_id": m.id, "state": "closed"}).await?;
    Ok(Milestone {
        number: m.number,
        title: m.title,
        description: m.description,
        state: m.state,
        due_on: m.due_on.map(to_chrono),
        open_issues,
        closed_issues,
        created_at: to_chrono(m.created_at),
        updated_at: to_chrono(m.updated_at),
        closed_at: m.closed_at.map(to_chrono),
    })
}

fn check_title(title: &str) -> Result<String, ApiError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ApiError::field("title", "Title can't be blank"));
    }
    if title.chars().count() > 255 {
        return Err(ApiError::field("title", "Title is too long (maximum is 255 characters)"));
    }
    if title.contains('"') {
        return Err(ApiError::field("title", "Title can't contain a double quote"));
    }
    Ok(title.to_string())
}

fn to_bson_date(d: chrono::DateTime<chrono::Utc>) -> DateTime {
    DateTime::from_millis(d.timestamp_millis())
}

#[derive(Deserialize)]
pub struct ListQuery {
    /// `open` (default), `closed` or `all`.
    state: Option<String>,
}

pub async fn list(state: web::Data<AppState>, viewer: Viewer, path: RepoPath, query: web::Query<ListQuery>) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let mut filter = doc! {"repo_id": access.repo.id};
    match query.state.as_deref().unwrap_or("open") {
        "all" => {}
        s @ ("open" | "closed") => {
            filter.insert("state", s);
        }
        other => return Err(ApiError::BadRequest(format!("state must be open, closed or all, got {other}"))),
    }
    // Soonest due date first, undated last.
    let mut docs: Vec<MilestoneDoc> = state.db.milestones.find(filter).await?.try_collect().await?;
    docs.sort_by_key(|m| (m.due_on.is_none(), m.due_on, m.number));
    let mut out = Vec::with_capacity(docs.len());
    for m in docs {
        out.push(dto(&state, m).await?);
    }
    Ok(HttpResponse::Ok().json(out))
}

pub async fn create(state: web::Data<AppState>, viewer: Viewer, path: RepoPath, body: web::Json<CreateMilestone>) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    access.require(Role::Write)?;
    let title = check_title(&body.title)?;
    let repo = state
        .db
        .repos
        .find_one_and_update(doc! {"_id": access.repo.id}, doc! {"$inc": {"milestone_seq": 1}})
        .return_document(ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    let now = DateTime::now();
    let m = MilestoneDoc {
        id: ObjectId::new(),
        repo_id: repo.id,
        number: repo.milestone_seq,
        title_lower: title.to_lowercase(),
        title,
        description: body.description.trim().to_string(),
        state: IssueState::Open,
        due_on: body.due_on.map(to_bson_date),
        created_at: now,
        updated_at: now,
        closed_at: None,
    };
    match state.db.milestones.insert_one(&m).await {
        Ok(_) => Ok(HttpResponse::Created().json(dto(&state, m).await?)),
        Err(e) if is_duplicate(&e) => Err(ApiError::field("title", "Title has already been taken")),
        Err(e) => Err(e.into()),
    }
}

async fn find(state: &AppState, access: &RepoAccess, number: i64) -> Result<MilestoneDoc, ApiError> {
    state.db.milestones.find_one(doc! {"repo_id": access.repo.id, "number": number}).await?.ok_or(ApiError::NotFound)
}

pub async fn get(state: web::Data<AppState>, viewer: Viewer, path: MilestonePath) -> Result<HttpResponse, ApiError> {
    let (owner, repo, number) = path.into_inner();
    let access = load_repo(&state, &owner, &repo, viewer.0).await?;
    let m = find(&state, &access, number).await?;
    Ok(HttpResponse::Ok().json(dto(&state, m).await?))
}

pub async fn update(state: web::Data<AppState>, viewer: Viewer, path: MilestonePath, body: web::Json<UpdateMilestone>) -> Result<HttpResponse, ApiError> {
    let (owner, repo, number) = path.into_inner();
    let access = load_repo(&state, &owner, &repo, viewer.0).await?;
    access.require(Role::Write)?;
    let m = find(&state, &access, number).await?;
    let mut set = doc! {"updated_at": DateTime::now()};
    if let Some(t) = &body.title {
        let t = check_title(t)?;
        set.insert("title_lower", t.to_lowercase());
        set.insert("title", t);
    }
    if let Some(d) = &body.description {
        set.insert("description", d.trim());
    }
    if let Some(due) = body.due_on {
        set.insert("due_on", due.map_or(Bson::Null, |d| Bson::DateTime(to_bson_date(d))));
    }
    if let Some(s) = body.state.filter(|s| *s != m.state) {
        set.insert("state", bson(&s)?);
        set.insert("closed_at", if s == IssueState::Closed { Bson::DateTime(DateTime::now()) } else { Bson::Null });
    }
    match state.db.milestones.update_one(doc! {"_id": m.id}, doc! {"$set": set}).await {
        Ok(_) => {}
        Err(e) if is_duplicate(&e) => return Err(ApiError::field("title", "Title has already been taken")),
        Err(e) => return Err(e.into()),
    }
    let m = find(&state, &access, number).await?;
    crate::notify::signal_repo(&state, access.repo.id);
    Ok(HttpResponse::Ok().json(dto(&state, m).await?))
}

pub async fn delete(state: web::Data<AppState>, viewer: Viewer, path: MilestonePath) -> Result<HttpResponse, ApiError> {
    let (owner, repo, number) = path.into_inner();
    let access = load_repo(&state, &owner, &repo, viewer.0).await?;
    access.require(Role::Write)?;
    let m = find(&state, &access, number).await?;
    state.db.issues.update_many(doc! {"milestone_id": m.id}, doc! {"$set": {"milestone_id": Bson::Null}}).await?;
    state.db.milestones.delete_one(doc! {"_id": m.id}).await?;
    crate::notify::signal_repo(&state, access.repo.id);
    Ok(HttpResponse::NoContent().finish())
}
