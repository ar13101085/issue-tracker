//! Comments, the issue timeline, markdown preview and `@` suggestions.

use crate::access::{load_repo, RepoAccess};
use crate::auth::Viewer;
use crate::error::ApiError;
use crate::markdown;
use crate::models::{to_chrono, CollaboratorDoc, CommentDoc, EditDoc, EventDoc, IssueDoc, UserDoc};
use crate::models::ReactionDoc;
use crate::routes::issues::{
    bson, can_comment, check_body, edits_dto, find_issue, issue_ref, load_reactions, reaction_groups, record_references,
    Associations, IssuePath,
};
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use serde::Deserialize;
use shared::{Comment, CommentBody, Event, MarkdownRequest, MarkdownResponse, Role, TimelineItem, UserRef};
use std::collections::HashMap;

type CommentPath = web::Path<(String, String, String)>;

fn comment_dto(doc: CommentDoc, access: &RepoAccess, assoc: &Associations, reactions: Option<&Vec<ReactionDoc>>) -> Comment {
    let own = access.viewer.as_ref().is_some_and(|u| u.id == doc.author_id);
    let can = own || access.has(Role::Write);
    Comment {
        id: doc.id.to_hex(),
        author: UserRef { login: doc.author_login },
        author_association: assoc.of(doc.author_id),
        body: doc.body,
        body_html: doc.body_html,
        created_at: to_chrono(doc.created_at),
        updated_at: to_chrono(doc.updated_at),
        edit_count: doc.edits.len(),
        reactions: reaction_groups(reactions, access.viewer.as_ref()),
        viewer_can_edit: can,
        viewer_can_delete: can,
    }
}

fn check_comment(body: &str) -> Result<(), ApiError> {
    if body.trim().is_empty() {
        return Err(ApiError::field("body", "Comment can't be blank"));
    }
    check_body(body)
}

/// `GET /issues/{number}/timeline`: comments and events, oldest first.
pub async fn timeline(state: web::Data<AppState>, viewer: Viewer, path: IssuePath) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let issue = find_issue(&state, &access, number).await?;
    let assoc = Associations::load(&state, &access.repo).await?;
    let comments: Vec<CommentDoc> = state.db.comments.find(doc! {"issue_id": issue.id}).await?.try_collect().await?;
    let events: Vec<EventDoc> = state.db.events.find(doc! {"issue_id": issue.id}).await?.try_collect().await?;

    // The other issues named by events, fetched together.
    let ids: Vec<ObjectId> = events.iter().filter_map(|e| e.source_issue_id).collect();
    let mut sources: HashMap<ObjectId, IssueDoc> = HashMap::new();
    if !ids.is_empty() {
        let docs: Vec<IssueDoc> = state.db.issues.find(doc! {"_id": {"$in": ids}}).await?.try_collect().await?;
        sources = docs.into_iter().map(|i| (i.id, i)).collect();
    }

    let reactions = load_reactions(&state, comments.iter().map(|c| c.id).collect()).await?;
    let mut items: Vec<(DateTime, TimelineItem)> = Vec::with_capacity(comments.len() + events.len());
    for c in comments {
        let mine = reactions.get(&c.id);
        items.push((c.created_at, TimelineItem::Comment(comment_dto(c, &access, &assoc, mine))));
    }
    for e in events {
        let source = match e.source_issue_id {
            // The other issue was deleted: the entry no longer means anything.
            Some(id) => match sources.get(&id) {
                Some(i) => Some(issue_ref(i)),
                None => continue,
            },
            None => None,
        };
        let event = Event {
            id: e.id.to_hex(),
            event: e.kind,
            actor: UserRef { login: e.actor_login },
            created_at: to_chrono(e.created_at),
            state_reason: e.state_reason,
            rename: e.rename,
            source,
            label: e.label,
            assignee: e.assignee_login.map(|login| UserRef { login }),
            milestone: e.milestone,
            detail: e.detail,
        };
        items.push((e.created_at, TimelineItem::Event(event)));
    }
    items.sort_by_key(|(at, _)| *at);
    let items: Vec<TimelineItem> = items.into_iter().map(|(_, item)| item).collect();
    Ok(HttpResponse::Ok().json(items))
}

pub async fn create(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: IssuePath,
    body: web::Json<CommentBody>,
) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Read)?;
    let issue = find_issue(&state, &access, number).await?;
    if !can_comment(&issue, &access) {
        return Err(ApiError::forbidden("This conversation is locked"));
    }
    check_comment(&body.body)?;
    let rendered = markdown::render(&state, &body.body, Some(&access.repo)).await?;
    let now = DateTime::now();
    let comment = CommentDoc {
        id: ObjectId::new(),
        repo_id: issue.repo_id,
        issue_id: issue.id,
        author_id: user.id,
        author_login: user.login.clone(),
        body: body.body.clone(),
        body_html: rendered.html,
        edits: Vec::new(),
        created_at: now,
        updated_at: now,
    };
    state.db.comments.insert_one(&comment).await?;
    state
        .db
        .issues
        .update_one(doc! {"_id": issue.id}, doc! {"$inc": {"comments": 1}, "$set": {"updated_at": now}})
        .await?;
    record_references(&state, &issue, user, &rendered.issue_refs).await?;
    crate::notify::commented(&state, &access.repo, &issue, user, &comment, &rendered.mentions).await;
    let assoc = Associations::load(&state, &access.repo).await?;
    Ok(HttpResponse::Created().json(comment_dto(comment, &access, &assoc, None)))
}

/// Loads a comment of this repository and checks the caller may change it.
async fn own_comment(state: &AppState, access: &RepoAccess, id: &str) -> Result<(CommentDoc, UserDoc), ApiError> {
    let user = access.user()?.clone();
    let id = ObjectId::parse_str(id).map_err(|_| ApiError::NotFound)?;
    let comment = state.db.comments.find_one(doc! {"_id": id, "repo_id": access.repo.id}).await?.ok_or(ApiError::NotFound)?;
    if comment.author_id != user.id && !access.has(Role::Write) {
        return Err(ApiError::forbidden("You do not have permission to change this comment"));
    }
    Ok((comment, user))
}

pub async fn update(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: CommentPath,
    body: web::Json<CommentBody>,
) -> Result<HttpResponse, ApiError> {
    let (owner, name, id) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let (comment, user) = own_comment(&state, &access, &id).await?;
    check_comment(&body.body)?;
    if body.body != comment.body {
        let rendered = markdown::render(&state, &body.body, Some(&access.repo)).await?;
        let now = DateTime::now();
        let old = EditDoc { editor_login: user.login.clone(), edited_at: now, body: comment.body.clone() };
        state
            .db
            .comments
            .update_one(
                doc! {"_id": comment.id},
                doc! {"$set": {"body": &body.body, "body_html": rendered.html, "updated_at": now}, "$push": {"edits": bson(&old)?}},
            )
            .await?;
        if let Some(issue) = state.db.issues.find_one(doc! {"_id": comment.issue_id}).await? {
            record_references(&state, &issue, &user, &rendered.issue_refs).await?;
            crate::notify::signal_issue(&state, access.repo.id, issue.number);
        }
    }
    let comment = state.db.comments.find_one(doc! {"_id": comment.id}).await?.ok_or(ApiError::NotFound)?;
    let assoc = Associations::load(&state, &access.repo).await?;
    let reactions = load_reactions(&state, vec![comment.id]).await?;
    let mine = reactions.get(&comment.id);
    Ok(HttpResponse::Ok().json(comment_dto(comment, &access, &assoc, mine)))
}

pub async fn delete(state: web::Data<AppState>, viewer: Viewer, path: CommentPath) -> Result<HttpResponse, ApiError> {
    let (owner, name, id) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let (comment, _) = own_comment(&state, &access, &id).await?;
    state.db.comments.delete_one(doc! {"_id": comment.id}).await?;
    state.db.reactions.delete_many(doc! {"subject_id": comment.id}).await?;
    state.db.issues.update_one(doc! {"_id": comment.issue_id}, doc! {"$inc": {"comments": -1}}).await?;
    if let Some(issue) = state.db.issues.find_one(doc! {"_id": comment.issue_id}).await? {
        crate::notify::signal_issue(&state, access.repo.id, issue.number);
    }
    Ok(HttpResponse::NoContent().finish())
}

/// `GET /issues/comments/{id}/edits`
pub async fn edits(state: web::Data<AppState>, viewer: Viewer, path: CommentPath) -> Result<HttpResponse, ApiError> {
    let (owner, name, id) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let id = ObjectId::parse_str(&id).map_err(|_| ApiError::NotFound)?;
    let comment = state.db.comments.find_one(doc! {"_id": id, "repo_id": access.repo.id}).await?.ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(edits_dto(comment.edits)))
}

/// `POST /markdown`: what the text will look like once posted.
pub async fn preview(
    state: web::Data<AppState>,
    viewer: Viewer,
    body: web::Json<MarkdownRequest>,
) -> Result<HttpResponse, ApiError> {
    check_body(&body.text)?;
    let repo = match body.repo.as_deref().and_then(|r| r.split_once('/')) {
        Some((owner, name)) => Some(load_repo(&state, owner, name, viewer.0).await?.repo),
        None => None,
    };
    let rendered = markdown::render(&state, &body.text, repo.as_ref()).await?;
    Ok(HttpResponse::Ok().json(MarkdownResponse { html: rendered.html }))
}

#[derive(Deserialize)]
pub struct Prefix {
    #[serde(default)]
    q: String,
}

/// `GET /repos/{owner}/{repo}/mentionables?q=al`: logins to suggest after `@`.
/// A private repository only suggests people who can see it.
pub async fn mentionables(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: web::Path<(String, String)>,
    query: web::Query<Prefix>,
) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let prefix = query.q.to_lowercase();
    if !prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Ok(HttpResponse::Ok().json(Vec::<UserRef>::new()));
    }
    let collabs: Vec<CollaboratorDoc> =
        state.db.collaborators.find(doc! {"repo_id": access.repo.id}).await?.try_collect().await?;
    let mut logins: Vec<String> = std::iter::once(access.repo.owner_login.clone())
        .chain(collabs.into_iter().map(|c| c.user_login))
        .filter(|l| l.to_lowercase().starts_with(&prefix))
        .collect();
    if !access.repo.private {
        let filter = doc! {"login_lower": {"$regex": format!("^{prefix}")}};
        let users: Vec<UserDoc> = state.db.users.find(filter).sort(doc! {"login_lower": 1}).limit(10).await?.try_collect().await?;
        for u in users {
            if !logins.contains(&u.login) {
                logins.push(u.login);
            }
        }
    }
    logins.truncate(10);
    let out: Vec<UserRef> = logins.into_iter().map(|login| UserRef { login }).collect();
    Ok(HttpResponse::Ok().json(out))
}
