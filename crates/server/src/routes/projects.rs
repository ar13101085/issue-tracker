//! Projects: a table and a board over a repository's issues, with custom fields.
//! Reading needs repository access, working with items needs triage, changing
//! the project itself (name, fields, deleting) needs write.

use crate::access::{load_repo, RepoAccess};
use crate::auth::Viewer;
use crate::error::ApiError;
use crate::models::{to_chrono, IssueDoc, ProjectDoc, ProjectItemDoc};
use crate::notify::signal_repo;
use crate::routes::issues::{find_issue, issue_dto, IssueCtx, IssuePath};
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime, Document};
use mongodb::options::ReturnDocument;
use rand::RngCore;
use serde_json::Value;
use shared::{
    AddItem, CreateField, CreateProject, FieldKind, FieldOption, IssueProject, Project, ProjectField, ProjectItem,
    ProjectSummary, Role, UpdateField, UpdateItem, UpdateProject, OPTION_COLORS, STATUS_FIELD,
};
use std::collections::HashMap;

type RepoPath = web::Path<(String, String)>;
type ProjectPath = web::Path<(String, String, i64)>;

const MAX_FIELDS: usize = 20;
const MAX_OPTIONS: usize = 50;
const MAX_ITEMS: u64 = 1000;
/// Gap left between neighbours so most moves touch one row.
const STEP: f64 = 1024.0;

fn new_id() -> String {
    let mut bytes = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn default_fields() -> Vec<ProjectField> {
    let option = |name: &str, color: &str| FieldOption { id: new_id(), name: name.into(), color: color.into() };
    vec![ProjectField {
        id: STATUS_FIELD.into(),
        name: "Status".into(),
        kind: FieldKind::SingleSelect,
        options: vec![option("Todo", "green"), option("In Progress", "yellow"), option("Done", "purple")],
    }]
}

fn check_title(title: &str) -> Result<String, ApiError> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 255 {
        return Err(ApiError::field("title", "Title must be between 1 and 255 characters"));
    }
    Ok(title.to_string())
}

fn check_name(name: &str, field: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 50 {
        return Err(ApiError::field(field, "Name must be between 1 and 50 characters"));
    }
    Ok(name.to_string())
}

async fn summary(state: &AppState, p: &ProjectDoc) -> Result<ProjectSummary, ApiError> {
    Ok(ProjectSummary {
        number: p.number,
        title: p.title.clone(),
        description: p.description.clone(),
        closed: p.closed,
        item_count: state.db.project_items.count_documents(doc! {"project_id": p.id}).await?,
        created_at: to_chrono(p.created_at),
        updated_at: to_chrono(p.updated_at),
    })
}

fn to_json(values: &Document) -> serde_json::Map<String, Value> {
    values.iter().map(|(k, v)| (k.clone(), v.clone().into_relaxed_extjson())).collect()
}

async fn find(state: &AppState, access: &RepoAccess, number: i64) -> Result<ProjectDoc, ApiError> {
    state.db.projects.find_one(doc! {"repo_id": access.repo.id, "number": number}).await?.ok_or(ApiError::NotFound)
}

async fn full(state: &AppState, access: &RepoAccess, p: ProjectDoc) -> Result<Project, ApiError> {
    let rows: Vec<ProjectItemDoc> =
        state.db.project_items.find(doc! {"project_id": p.id}).sort(doc! {"position": 1}).await?.try_collect().await?;
    let ids: Vec<ObjectId> = rows.iter().map(|r| r.issue_id).collect();
    let issues: Vec<IssueDoc> = state.db.issues.find(doc! {"_id": {"$in": ids}}).await?.try_collect().await?;
    let ctx = IssueCtx::load(state, access, &issues, false).await?;
    let mut by_id: HashMap<ObjectId, IssueDoc> = issues.into_iter().map(|i| (i.id, i)).collect();
    let items = rows
        .into_iter()
        .filter_map(|r| {
            let issue = by_id.remove(&r.issue_id)?;
            Some(ProjectItem { id: r.id.to_hex(), issue: issue_dto(issue, access, &ctx), values: to_json(&r.values), position: r.position })
        })
        .collect();
    Ok(Project {
        summary: summary(state, &p).await?,
        fields: p.fields,
        items,
        viewer_can_admin: access.has(Role::Write),
        viewer_can_edit: access.has(Role::Triage),
    })
}

async fn respond(state: &AppState, access: &RepoAccess, number: i64) -> Result<HttpResponse, ApiError> {
    let project = find(state, access, number).await?;
    state.db.projects.update_one(doc! {"_id": project.id}, doc! {"$set": {"updated_at": DateTime::now()}}).await?;
    signal_repo(state, access.repo.id);
    Ok(HttpResponse::Ok().json(full(state, access, project).await?))
}

pub async fn list(state: web::Data<AppState>, viewer: Viewer, path: RepoPath) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let docs: Vec<ProjectDoc> =
        state.db.projects.find(doc! {"repo_id": access.repo.id}).sort(doc! {"number": -1}).await?.try_collect().await?;
    let mut out = Vec::with_capacity(docs.len());
    for p in &docs {
        out.push(summary(&state, p).await?);
    }
    Ok(HttpResponse::Ok().json(out))
}

pub async fn create(state: web::Data<AppState>, viewer: Viewer, path: RepoPath, body: web::Json<CreateProject>) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    access.require(Role::Write)?;
    let title = check_title(&body.title)?;
    let repo = state
        .db
        .repos
        .find_one_and_update(doc! {"_id": access.repo.id}, doc! {"$inc": {"project_seq": 1}})
        .return_document(ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    let now = DateTime::now();
    let project = ProjectDoc {
        id: ObjectId::new(),
        repo_id: repo.id,
        number: repo.project_seq,
        title,
        description: body.description.trim().to_string(),
        closed: false,
        fields: default_fields(),
        created_at: now,
        updated_at: now,
    };
    state.db.projects.insert_one(&project).await?;
    signal_repo(&state, access.repo.id);
    Ok(HttpResponse::Created().json(full(&state, &access, project).await?))
}

pub async fn get(state: web::Data<AppState>, viewer: Viewer, path: ProjectPath) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let project = find(&state, &access, number).await?;
    Ok(HttpResponse::Ok().json(full(&state, &access, project).await?))
}

pub async fn update(state: web::Data<AppState>, viewer: Viewer, path: ProjectPath, body: web::Json<UpdateProject>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Write)?;
    let project = find(&state, &access, number).await?;
    let mut set = doc! {};
    if let Some(t) = &body.title {
        set.insert("title", check_title(t)?);
    }
    if let Some(d) = &body.description {
        set.insert("description", d.trim());
    }
    if let Some(c) = body.closed {
        set.insert("closed", c);
    }
    if !set.is_empty() {
        state.db.projects.update_one(doc! {"_id": project.id}, doc! {"$set": set}).await?;
    }
    respond(&state, &access, number).await
}

pub async fn delete(state: web::Data<AppState>, viewer: Viewer, path: ProjectPath) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Write)?;
    let project = find(&state, &access, number).await?;
    state.db.project_items.delete_many(doc! {"project_id": project.id}).await?;
    state.db.projects.delete_one(doc! {"_id": project.id}).await?;
    signal_repo(&state, access.repo.id);
    Ok(HttpResponse::NoContent().finish())
}

fn color_or(color: Option<&str>, fallback: &str) -> Result<String, ApiError> {
    match color {
        None => Ok(fallback.to_string()),
        Some(c) if OPTION_COLORS.contains(&c) => Ok(c.to_string()),
        Some(_) => Err(ApiError::field("options", format!("Color must be one of {}", OPTION_COLORS.join(", ")))),
    }
}

async fn save_fields(state: &AppState, project: &ProjectDoc, fields: &[ProjectField]) -> Result<(), ApiError> {
    let stored = mongodb::bson::to_bson(fields).map_err(|e| ApiError::Internal(e.to_string()))?;
    state.db.projects.update_one(doc! {"_id": project.id}, doc! {"$set": {"fields": stored}}).await?;
    Ok(())
}

pub async fn add_field(state: web::Data<AppState>, viewer: Viewer, path: ProjectPath, body: web::Json<CreateField>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Write)?;
    let project = find(&state, &access, number).await?;
    if project.fields.len() >= MAX_FIELDS {
        return Err(ApiError::field("name", "A project can have at most 20 fields"));
    }
    let field_name = check_name(&body.name, "name")?;
    let reserved = ["title", "assignees", "labels", "milestone", "type"];
    if reserved.contains(&field_name.to_lowercase().as_str()) || project.fields.iter().any(|f| f.name.eq_ignore_ascii_case(&field_name)) {
        return Err(ApiError::field("name", "A field with that name already exists"));
    }
    let mut options = Vec::new();
    if body.kind == FieldKind::SingleSelect {
        if body.options.is_empty() || body.options.len() > MAX_OPTIONS {
            return Err(ApiError::field("options", "A single select field needs between 1 and 50 options"));
        }
        for (i, o) in body.options.iter().enumerate() {
            options.push(FieldOption { id: new_id(), name: check_name(o, "options")?, color: OPTION_COLORS[i % OPTION_COLORS.len()].into() });
        }
    }
    let mut fields = project.fields.clone();
    fields.push(ProjectField { id: new_id(), name: field_name, kind: body.kind, options });
    save_fields(&state, &project, &fields).await?;
    respond(&state, &access, number).await
}

pub async fn update_field(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: web::Path<(String, String, i64, String)>,
    body: web::Json<UpdateField>,
) -> Result<HttpResponse, ApiError> {
    let (owner, name, number, field_id) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Write)?;
    let project = find(&state, &access, number).await?;
    let mut fields = project.fields.clone();
    let others: Vec<String> = fields.iter().filter(|f| f.id != field_id).map(|f| f.name.to_lowercase()).collect();
    let field = fields.iter_mut().find(|f| f.id == field_id).ok_or(ApiError::NotFound)?;
    if let Some(n) = &body.name {
        let n = check_name(n, "name")?;
        if others.contains(&n.to_lowercase()) {
            return Err(ApiError::field("name", "A field with that name already exists"));
        }
        field.name = n;
    }
    let mut removed: Vec<String> = Vec::new();
    if let Some(inputs) = &body.options {
        if field.kind != FieldKind::SingleSelect {
            return Err(ApiError::field("options", "Only single select fields have options"));
        }
        if inputs.is_empty() || inputs.len() > MAX_OPTIONS {
            return Err(ApiError::field("options", "A single select field needs between 1 and 50 options"));
        }
        let mut next: Vec<FieldOption> = Vec::new();
        for (i, input) in inputs.iter().enumerate() {
            let existing = input.id.as_ref().and_then(|id| field.options.iter().find(|o| &o.id == id));
            let fallback = existing.map_or(OPTION_COLORS[i % OPTION_COLORS.len()], |o| o.color.as_str());
            next.push(FieldOption {
                id: existing.map_or_else(new_id, |o| o.id.clone()),
                name: check_name(&input.name, "options")?,
                color: color_or(input.color.as_deref(), fallback)?,
            });
        }
        removed = field.options.iter().filter(|o| !next.iter().any(|n| n.id == o.id)).map(|o| o.id.clone()).collect();
        field.options = next;
    }
    save_fields(&state, &project, &fields).await?;
    // Items pointing at a removed option fall back to "no value".
    if !removed.is_empty() {
        let key = format!("values.{field_id}");
        let mut unset = Document::new();
        unset.insert(key.as_str(), "");
        let mut filter = doc! {"project_id": project.id};
        filter.insert(key.as_str(), doc! {"$in": removed});
        state.db.project_items.update_many(filter, doc! {"$unset": unset}).await?;
    }
    respond(&state, &access, number).await
}

pub async fn delete_field(state: web::Data<AppState>, viewer: Viewer, path: web::Path<(String, String, i64, String)>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number, field_id) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Write)?;
    let project = find(&state, &access, number).await?;
    if field_id == STATUS_FIELD {
        return Err(ApiError::field("field", "The Status field cannot be deleted"));
    }
    if !project.fields.iter().any(|f| f.id == field_id) {
        return Err(ApiError::NotFound);
    }
    let fields: Vec<ProjectField> = project.fields.iter().filter(|f| f.id != field_id).cloned().collect();
    save_fields(&state, &project, &fields).await?;
    let mut unset = Document::new();
    unset.insert(format!("values.{field_id}"), "");
    state.db.project_items.update_many(doc! {"project_id": project.id}, doc! {"$unset": unset}).await?;
    respond(&state, &access, number).await
}

/// Position for a new last item of a status column (`None` is the "no status" column).
async fn end_of_column(state: &AppState, project: &ProjectDoc, status: Option<&str>) -> Result<f64, ApiError> {
    let mut filter = doc! {"project_id": project.id};
    filter.insert("values.status", status.map_or(Bson::Null, |s| Bson::String(s.into())));
    let last = state.db.project_items.find(filter).sort(doc! {"position": -1}).limit(1).await?.try_next().await?;
    Ok(last.map_or(STEP, |i| i.position + STEP))
}

pub async fn add_item(state: web::Data<AppState>, viewer: Viewer, path: ProjectPath, body: web::Json<AddItem>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Triage)?;
    let project = find(&state, &access, number).await?;
    let issue = match find_issue(&state, &access, body.issue_number).await {
        Ok(i) => i,
        Err(ApiError::NotFound) => return Err(ApiError::field("issue_number", "That issue does not exist in this repository")),
        Err(e) => return Err(e),
    };
    if state.db.project_items.find_one(doc! {"project_id": project.id, "issue_id": issue.id}).await?.is_some() {
        return Err(ApiError::field("issue_number", "That issue is already in this project"));
    }
    if state.db.project_items.count_documents(doc! {"project_id": project.id}).await? >= MAX_ITEMS {
        return Err(ApiError::field("issue_number", "A project can hold at most 1000 items"));
    }
    let item = ProjectItemDoc {
        id: ObjectId::new(),
        project_id: project.id,
        repo_id: access.repo.id,
        issue_id: issue.id,
        values: Document::new(),
        position: end_of_column(&state, &project, None).await?,
        created_at: DateTime::now(),
    };
    state.db.project_items.insert_one(item).await?;
    respond(&state, &access, number).await
}

/// Checks a value against its field and turns it into what is stored.
fn stored_value(field: &ProjectField, value: &Value) -> Result<Bson, ApiError> {
    let bad = |m: &str| ApiError::field(&field.id, format!("{}: {m}", field.name));
    match (field.kind, value) {
        (FieldKind::SingleSelect, Value::String(id)) => {
            if field.options.iter().any(|o| &o.id == id) {
                Ok(Bson::String(id.clone()))
            } else {
                Err(bad("that option does not exist"))
            }
        }
        (FieldKind::Text, Value::String(text)) => {
            if text.chars().count() > 1000 {
                return Err(bad("text is too long (maximum is 1000 characters)"));
            }
            Ok(Bson::String(text.clone()))
        }
        (FieldKind::Number, Value::Number(n)) => n.as_f64().filter(|f| f.is_finite()).map(Bson::Double).ok_or_else(|| bad("not a number")),
        (FieldKind::Date, Value::String(date)) => {
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| bad("use the form YYYY-MM-DD"))?;
            Ok(Bson::String(date.clone()))
        }
        _ => Err(bad("wrong kind of value")),
    }
}

pub async fn update_item(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: web::Path<(String, String, i64, String)>,
    body: web::Json<UpdateItem>,
) -> Result<HttpResponse, ApiError> {
    let (owner, name, number, item_id) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Triage)?;
    let project = find(&state, &access, number).await?;
    let id = ObjectId::parse_str(&item_id).map_err(|_| ApiError::NotFound)?;
    let item = state.db.project_items.find_one(doc! {"_id": id, "project_id": project.id}).await?.ok_or(ApiError::NotFound)?;

    let mut values = item.values.clone();
    if let Some(changes) = &body.values {
        for (field_id, value) in changes {
            let field = project.fields.iter().find(|f| &f.id == field_id).ok_or_else(|| ApiError::field(field_id, "That field does not exist"))?;
            let empty = value.is_null() || value.as_str().is_some_and(|s| s.trim().is_empty());
            if empty {
                values.remove(field_id);
            } else {
                values.insert(field_id.clone(), stored_value(field, value)?);
            }
        }
    }
    let status = values.get_str(STATUS_FIELD).ok().map(str::to_string);
    let status_changed = status.as_deref() != item.values.get_str(STATUS_FIELD).ok();

    let mut position = item.position;
    match &body.before {
        // Dropped in front of another card of the same column.
        Some(Some(before_id)) => {
            let before_oid = ObjectId::parse_str(before_id).map_err(|_| ApiError::field("before", "That item does not exist"))?;
            let mut filter = doc! {"project_id": project.id, "_id": {"$ne": item.id}};
            filter.insert("values.status", status.clone().map_or(Bson::Null, Bson::String));
            let column: Vec<ProjectItemDoc> = state.db.project_items.find(filter).sort(doc! {"position": 1}).await?.try_collect().await?;
            let index = column.iter().position(|i| i.id == before_oid).ok_or_else(|| ApiError::field("before", "That item is not in this column"))?;
            let next = column[index].position;
            let previous = if index == 0 { next - 2.0 * STEP } else { column[index - 1].position };
            position = (previous + next) / 2.0;
        }
        Some(None) => position = end_of_column(&state, &project, status.as_deref()).await?,
        None if status_changed => position = end_of_column(&state, &project, status.as_deref()).await?,
        None => {}
    }
    state.db.project_items.update_one(doc! {"_id": item.id}, doc! {"$set": {"values": values, "position": position}}).await?;
    respond(&state, &access, number).await
}

pub async fn remove_item(state: web::Data<AppState>, viewer: Viewer, path: web::Path<(String, String, i64, String)>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number, item_id) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Triage)?;
    let project = find(&state, &access, number).await?;
    let id = ObjectId::parse_str(&item_id).map_err(|_| ApiError::NotFound)?;
    let result = state.db.project_items.delete_one(doc! {"_id": id, "project_id": project.id}).await?;
    if result.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    respond(&state, &access, number).await
}

/// `GET /issues/{number}/projects`: the projects an issue is in, with its status in each.
pub async fn of_issue(state: web::Data<AppState>, viewer: Viewer, path: IssuePath) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let issue = find_issue(&state, &access, number).await?;
    let rows: Vec<ProjectItemDoc> = state.db.project_items.find(doc! {"issue_id": issue.id}).await?.try_collect().await?;
    let mut out = Vec::new();
    for row in rows {
        let Some(project) = state.db.projects.find_one(doc! {"_id": row.project_id}).await? else {
            continue;
        };
        let status = row
            .values
            .get_str(STATUS_FIELD)
            .ok()
            .and_then(|id| project.fields.iter().find(|f| f.id == STATUS_FIELD)?.options.iter().find(|o| o.id == id).cloned());
        out.push(IssueProject { number: project.number, title: project.title, item_id: row.id.to_hex(), status });
    }
    out.sort_by_key(|p| p.number);
    Ok(HttpResponse::Ok().json(out))
}
