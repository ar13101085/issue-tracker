//! Lock, pin, sub-issues, blocking, transfer, bulk edits and reactions.

use crate::access::{load_repo, RepoAccess};
use crate::auth::Viewer;
use crate::error::ApiError;
use crate::models::{CommentDoc, EventDoc, IssueDoc, LabelDoc, ReactionDoc, UserDoc};
use crate::notify::{signal_issue, signal_repo};
use crate::routes::issues::{apply_update, can_comment, find_issue, issue_dto, next_number, one, IssueCtx, IssuePath};
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime};
use shared::{BulkUpdate, EventKind, IssueLocation, LockRequest, Role, SubIssueRequest, TransferRequest, UpdateIssue, LOCK_REASONS, REACTIONS};

const MAX_PINNED: u64 = 3;
const MAX_SUB_ISSUES: u64 = 100;
const MAX_BLOCKERS: usize = 50;
const MAX_RELATED: usize = 50;
const MAX_BULK: usize = 100;

async fn event(state: &AppState, issue: &IssueDoc, actor: &UserDoc, kind: EventKind, edit: impl FnOnce(&mut EventDoc)) -> Result<(), ApiError> {
    let mut e = EventDoc::new(issue, actor, kind);
    edit(&mut e);
    state.db.events.insert_one(e).await?;
    Ok(())
}

async fn respond(state: &AppState, access: &RepoAccess, number: i64) -> Result<HttpResponse, ApiError> {
    let issue = find_issue(state, access, number).await?;
    signal_issue(state, access.repo.id, number);
    Ok(HttpResponse::Ok().json(one(state, access, issue).await?))
}

pub async fn lock(state: web::Data<AppState>, viewer: Viewer, path: IssuePath, body: Option<web::Json<LockRequest>>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Triage)?;
    let issue = find_issue(&state, &access, number).await?;
    let reason = body.and_then(|b| b.into_inner().lock_reason).filter(|r| !r.is_empty());
    if let Some(r) = &reason {
        if !LOCK_REASONS.contains(&r.as_str()) {
            return Err(ApiError::field("lock_reason", format!("Reason must be one of {}", LOCK_REASONS.join(", "))));
        }
    }
    if !issue.locked {
        let stored = reason.clone().map_or(Bson::Null, Bson::String);
        state.db.issues.update_one(doc! {"_id": issue.id}, doc! {"$set": {"locked": true, "lock_reason": stored}}).await?;
        event(&state, &issue, user, EventKind::Locked, |e| e.detail = reason).await?;
    }
    respond(&state, &access, number).await
}

pub async fn unlock(state: web::Data<AppState>, viewer: Viewer, path: IssuePath) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Triage)?;
    let issue = find_issue(&state, &access, number).await?;
    if issue.locked {
        state.db.issues.update_one(doc! {"_id": issue.id}, doc! {"$set": {"locked": false, "lock_reason": Bson::Null}}).await?;
        event(&state, &issue, user, EventKind::Unlocked, |_| {}).await?;
    }
    respond(&state, &access, number).await
}

pub async fn pin(state: web::Data<AppState>, viewer: Viewer, path: IssuePath) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Write)?;
    let issue = find_issue(&state, &access, number).await?;
    if !issue.pinned {
        if state.db.issues.count_documents(doc! {"repo_id": access.repo.id, "pinned": true}).await? >= MAX_PINNED {
            return Err(ApiError::field("pinned", "You can pin up to 3 issues. Unpin one first."));
        }
        state.db.issues.update_one(doc! {"_id": issue.id}, doc! {"$set": {"pinned": true}}).await?;
        event(&state, &issue, user, EventKind::Pinned, |_| {}).await?;
    }
    signal_repo(&state, access.repo.id);
    respond(&state, &access, number).await
}

pub async fn unpin(state: web::Data<AppState>, viewer: Viewer, path: IssuePath) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Write)?;
    let issue = find_issue(&state, &access, number).await?;
    if issue.pinned {
        state.db.issues.update_one(doc! {"_id": issue.id}, doc! {"$set": {"pinned": false}}).await?;
        event(&state, &issue, user, EventKind::Unpinned, |_| {}).await?;
    }
    signal_repo(&state, access.repo.id);
    respond(&state, &access, number).await
}

/// `GET /issues/pinned`
pub async fn pinned(state: web::Data<AppState>, viewer: Viewer, path: web::Path<(String, String)>) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let docs: Vec<IssueDoc> =
        state.db.issues.find(doc! {"repo_id": access.repo.id, "pinned": true}).sort(doc! {"number": 1}).await?.try_collect().await?;
    let ctx = IssueCtx::load(&state, &access, &docs, false).await?;
    let items: Vec<shared::Issue> = docs.into_iter().map(|d| issue_dto(d, &access, &ctx)).collect();
    Ok(HttpResponse::Ok().json(items))
}

pub async fn add_sub_issue(state: web::Data<AppState>, viewer: Viewer, path: IssuePath, body: web::Json<SubIssueRequest>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Triage)?;
    let parent = find_issue(&state, &access, number).await?;
    let fail = |m: &str| ApiError::field("number", m);
    let child = match find_issue(&state, &access, body.number).await {
        Ok(c) => c,
        Err(ApiError::NotFound) => return Err(fail("That issue does not exist in this repository")),
        Err(e) => return Err(e),
    };
    if child.id == parent.id {
        return Err(fail("An issue cannot be its own sub-issue"));
    }
    if child.parent_id.is_some() {
        return Err(fail("That issue already has a parent"));
    }
    // Walking up from the parent must never reach the child.
    let mut cursor = parent.parent_id;
    let mut hops = 0;
    while let Some(id) = cursor {
        if id == child.id {
            return Err(fail("That would make an issue its own ancestor"));
        }
        hops += 1;
        if hops > 50 {
            break;
        }
        cursor = state.db.issues.find_one(doc! {"_id": id}).await?.and_then(|i| i.parent_id);
    }
    if state.db.issues.count_documents(doc! {"parent_id": parent.id}).await? >= MAX_SUB_ISSUES {
        return Err(fail("An issue can have at most 100 sub-issues"));
    }
    state.db.issues.update_one(doc! {"_id": child.id}, doc! {"$set": {"parent_id": parent.id}}).await?;
    event(&state, &parent, user, EventKind::SubIssueAdded, |e| e.source_issue_id = Some(child.id)).await?;
    event(&state, &child, user, EventKind::ParentAdded, |e| e.source_issue_id = Some(parent.id)).await?;
    signal_issue(&state, access.repo.id, child.number);
    respond(&state, &access, number).await
}

pub async fn remove_sub_issue(state: web::Data<AppState>, viewer: Viewer, path: web::Path<(String, String, i64, i64)>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number, child_number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Triage)?;
    let parent = find_issue(&state, &access, number).await?;
    let child = find_issue(&state, &access, child_number).await?;
    if child.parent_id != Some(parent.id) {
        return Err(ApiError::NotFound);
    }
    state.db.issues.update_one(doc! {"_id": child.id}, doc! {"$set": {"parent_id": Bson::Null}}).await?;
    event(&state, &parent, user, EventKind::SubIssueRemoved, |e| e.source_issue_id = Some(child.id)).await?;
    event(&state, &child, user, EventKind::ParentRemoved, |e| e.source_issue_id = Some(parent.id)).await?;
    signal_issue(&state, access.repo.id, child.number);
    respond(&state, &access, number).await
}

/// `POST /issues/{number}/dependencies/blocked_by`: `number` waits on the issue in the body.
pub async fn add_blocked_by(state: web::Data<AppState>, viewer: Viewer, path: IssuePath, body: web::Json<SubIssueRequest>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Triage)?;
    let issue = find_issue(&state, &access, number).await?;
    let fail = |m: &str| ApiError::field("number", m);
    let blocker = match find_issue(&state, &access, body.number).await {
        Ok(b) => b,
        Err(ApiError::NotFound) => return Err(fail("That issue does not exist in this repository")),
        Err(e) => return Err(e),
    };
    if blocker.id == issue.id {
        return Err(fail("An issue cannot block itself"));
    }
    if issue.blocked_by_ids.contains(&blocker.id) {
        return Err(fail("That issue already blocks this one"));
    }
    if issue.blocked_by_ids.len() >= MAX_BLOCKERS {
        return Err(fail("An issue can be blocked by at most 50 issues"));
    }
    // Everything the blocker waits on, directly or not, must not include this issue.
    let mut pending = blocker.blocked_by_ids.clone();
    let mut seen: std::collections::HashSet<ObjectId> = std::collections::HashSet::new();
    while let Some(id) = pending.pop() {
        if id == issue.id {
            return Err(fail("Those two issues would block each other"));
        }
        if !seen.insert(id) || seen.len() > 500 {
            continue;
        }
        if let Some(next) = state.db.issues.find_one(doc! {"_id": id}).await? {
            pending.extend(next.blocked_by_ids);
        }
    }
    state.db.issues.update_one(doc! {"_id": issue.id}, doc! {"$addToSet": {"blocked_by_ids": blocker.id}}).await?;
    event(&state, &issue, user, EventKind::BlockedByAdded, |e| e.source_issue_id = Some(blocker.id)).await?;
    event(&state, &blocker, user, EventKind::BlockingAdded, |e| e.source_issue_id = Some(issue.id)).await?;
    signal_issue(&state, access.repo.id, blocker.number);
    respond(&state, &access, number).await
}

/// `DELETE /issues/{number}/dependencies/blocked_by/{blocker}`
pub async fn remove_blocked_by(state: web::Data<AppState>, viewer: Viewer, path: web::Path<(String, String, i64, i64)>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number, blocker_number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Triage)?;
    let issue = find_issue(&state, &access, number).await?;
    let blocker = find_issue(&state, &access, blocker_number).await?;
    if !issue.blocked_by_ids.contains(&blocker.id) {
        return Err(ApiError::NotFound);
    }
    state.db.issues.update_one(doc! {"_id": issue.id}, doc! {"$pull": {"blocked_by_ids": blocker.id}}).await?;
    event(&state, &issue, user, EventKind::BlockedByRemoved, |e| e.source_issue_id = Some(blocker.id)).await?;
    event(&state, &blocker, user, EventKind::BlockingRemoved, |e| e.source_issue_id = Some(issue.id)).await?;
    signal_issue(&state, access.repo.id, blocker.number);
    respond(&state, &access, number).await
}

/// `POST /issues/{number}/relations`: links two issues as related. The link has no direction.
pub async fn add_related(state: web::Data<AppState>, viewer: Viewer, path: IssuePath, body: web::Json<SubIssueRequest>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Triage)?;
    let issue = find_issue(&state, &access, number).await?;
    let fail = |m: &str| ApiError::field("number", m);
    let other = match find_issue(&state, &access, body.number).await {
        Ok(o) => o,
        Err(ApiError::NotFound) => return Err(fail("That issue does not exist in this repository")),
        Err(e) => return Err(e),
    };
    if other.id == issue.id {
        return Err(fail("An issue cannot relate to itself"));
    }
    if issue.related_ids.contains(&other.id) {
        return Err(fail("Those issues are already related"));
    }
    if issue.related_ids.len() >= MAX_RELATED || other.related_ids.len() >= MAX_RELATED {
        return Err(fail("An issue can relate to at most 50 issues"));
    }
    state.db.issues.update_one(doc! {"_id": issue.id}, doc! {"$addToSet": {"related_ids": other.id}}).await?;
    state.db.issues.update_one(doc! {"_id": other.id}, doc! {"$addToSet": {"related_ids": issue.id}}).await?;
    event(&state, &issue, user, EventKind::RelatedAdded, |e| e.source_issue_id = Some(other.id)).await?;
    event(&state, &other, user, EventKind::RelatedAdded, |e| e.source_issue_id = Some(issue.id)).await?;
    signal_issue(&state, access.repo.id, other.number);
    respond(&state, &access, number).await
}

/// `DELETE /issues/{number}/relations/{other}`
pub async fn remove_related(state: web::Data<AppState>, viewer: Viewer, path: web::Path<(String, String, i64, i64)>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number, other_number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.require(Role::Triage)?;
    let issue = find_issue(&state, &access, number).await?;
    let other = find_issue(&state, &access, other_number).await?;
    if !issue.related_ids.contains(&other.id) {
        return Err(ApiError::NotFound);
    }
    state.db.issues.update_one(doc! {"_id": issue.id}, doc! {"$pull": {"related_ids": other.id}}).await?;
    state.db.issues.update_one(doc! {"_id": other.id}, doc! {"$pull": {"related_ids": issue.id}}).await?;
    event(&state, &issue, user, EventKind::RelatedRemoved, |e| e.source_issue_id = Some(other.id)).await?;
    event(&state, &other, user, EventKind::RelatedRemoved, |e| e.source_issue_id = Some(issue.id)).await?;
    signal_issue(&state, access.repo.id, other.number);
    respond(&state, &access, number).await
}

/// Moves an issue, with its comments and history, to another repository.
pub async fn transfer(state: web::Data<AppState>, viewer: Viewer, path: IssuePath, body: web::Json<TransferRequest>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0.clone()).await?;
    let user = access.require(Role::Write)?.clone();
    let issue = find_issue(&state, &access, number).await?;
    let fail = |m: &str| ApiError::field("repo", m);
    let (to_owner, to_name) = body.repo.trim().split_once('/').ok_or_else(|| fail("Enter the destination as owner/name"))?;
    let target = match load_repo(&state, to_owner, to_name, viewer.0).await {
        Ok(t) => t,
        Err(ApiError::NotFound) => return Err(fail("That repository does not exist")),
        Err(e) => return Err(e),
    };
    if target.repo.id == access.repo.id {
        return Err(fail("The issue is already in that repository"));
    }
    if !target.has(Role::Write) {
        return Err(fail("You need write access to the destination repository"));
    }

    // Labels carry over by name where the destination has them.
    let old: Vec<LabelDoc> = state.db.labels.find(doc! {"_id": {"$in": issue.label_ids.clone()}}).await?.try_collect().await?;
    let mut label_ids = Vec::new();
    for l in old {
        if let Some(m) = state.db.labels.find_one(doc! {"repo_id": target.repo.id, "name_lower": &l.name_lower}).await? {
            label_ids.push(m.id);
        }
    }
    let new_number = next_number(&state, target.repo.id).await?;
    state.db.issues.update_many(doc! {"parent_id": issue.id}, doc! {"$set": {"parent_id": Bson::Null}}).await?;
    // Relationships stay inside one repository.
    state.db.issues.update_many(doc! {"blocked_by_ids": issue.id}, doc! {"$pull": {"blocked_by_ids": issue.id}}).await?;
    state.db.issues.update_many(doc! {"related_ids": issue.id}, doc! {"$pull": {"related_ids": issue.id}}).await?;
    state
        .db
        .issues
        .update_one(
            doc! {"_id": issue.id},
            doc! {"$set": {
                "repo_id": target.repo.id, "number": new_number, "label_ids": label_ids,
                "milestone_id": Bson::Null, "parent_id": Bson::Null, "blocked_by_ids": [], "related_ids": [], "pinned": false, "updated_at": DateTime::now(),
            }},
        )
        .await?;
    let moved = doc! {"$set": {"repo_id": target.repo.id}};
    state.db.comments.update_many(doc! {"issue_id": issue.id}, moved.clone()).await?;
    state.db.events.update_many(doc! {"issue_id": issue.id}, moved.clone()).await?;
    state.db.notifications.update_many(doc! {"issue_id": issue.id}, moved).await?;
    // Projects belong to a repository; the issue leaves them when it leaves.
    state.db.project_items.delete_many(doc! {"issue_id": issue.id}).await?;
    // Mentions from the old repository's issues would now point across repositories.
    state.db.events.delete_many(doc! {"source_issue_id": issue.id}).await?;
    let from = format!("{}/{}", access.repo.owner_login, access.repo.name);
    let mut e = EventDoc::new(&issue, &user, EventKind::Transferred);
    e.repo_id = target.repo.id;
    e.detail = Some(from);
    state.db.events.insert_one(e).await?;
    signal_repo(&state, access.repo.id);
    signal_repo(&state, target.repo.id);
    Ok(HttpResponse::Ok().json(IssueLocation { owner: target.repo.owner_login, repo: target.repo.name, number: new_number }))
}

/// `POST /issues/bulk`: the list page's "mark as", "label" and "assign".
pub async fn bulk(state: web::Data<AppState>, viewer: Viewer, path: web::Path<(String, String)>, body: web::Json<BulkUpdate>) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    access.require(Role::Triage)?;
    let body = body.into_inner();
    if body.numbers.is_empty() || body.numbers.len() > MAX_BULK {
        return Err(ApiError::field("numbers", "Select between 1 and 100 issues"));
    }
    let lower = |names: &[String]| names.iter().map(|n| n.trim().to_lowercase()).collect::<Vec<_>>();
    let (add_l, del_l, add_a, del_a) = (lower(&body.add_labels), lower(&body.remove_labels), lower(&body.add_assignees), lower(&body.remove_assignees));
    let mut updated = 0;
    for number in &body.numbers {
        let issue = find_issue(&state, &access, *number).await?;
        let mut change = UpdateIssue { state: body.state, state_reason: body.state_reason, milestone: body.milestone, ..Default::default() };
        if !add_l.is_empty() || !del_l.is_empty() {
            let current: Vec<LabelDoc> = state.db.labels.find(doc! {"_id": {"$in": issue.label_ids.clone()}}).await?.try_collect().await?;
            let mut names: Vec<String> = current.into_iter().map(|l| l.name_lower).filter(|n| !del_l.contains(n)).collect();
            names.extend(add_l.iter().filter(|n| !names.contains(n)).cloned().collect::<Vec<_>>());
            change.labels = Some(names);
        }
        if !add_a.is_empty() || !del_a.is_empty() {
            let current: Vec<UserDoc> = state.db.users.find(doc! {"_id": {"$in": issue.assignee_ids.clone()}}).await?.try_collect().await?;
            let mut logins: Vec<String> = current.into_iter().map(|u| u.login_lower).filter(|n| !del_a.contains(n)).collect();
            logins.extend(add_a.iter().filter(|n| !logins.contains(n)).cloned().collect::<Vec<_>>());
            change.assignees = Some(logins);
        }
        apply_update(&state, &access, &issue, change).await?;
        updated += 1;
    }
    signal_repo(&state, access.repo.id);
    Ok(HttpResponse::Ok().json(serde_json::json!({ "updated": updated })))
}

/// The issue or comment a reaction is put on, with the issue it belongs to.
async fn reaction_target(state: &AppState, access: &RepoAccess, issue_number: Option<i64>, comment_id: Option<&str>) -> Result<(ObjectId, IssueDoc), ApiError> {
    match (issue_number, comment_id) {
        (Some(n), _) => {
            let issue = find_issue(state, access, n).await?;
            Ok((issue.id, issue))
        }
        (None, Some(id)) => {
            let id = ObjectId::parse_str(id).map_err(|_| ApiError::NotFound)?;
            let comment: CommentDoc = state.db.comments.find_one(doc! {"_id": id, "repo_id": access.repo.id}).await?.ok_or(ApiError::NotFound)?;
            let issue = state.db.issues.find_one(doc! {"_id": comment.issue_id}).await?.ok_or(ApiError::NotFound)?;
            Ok((comment.id, issue))
        }
        _ => Err(ApiError::NotFound),
    }
}

async fn react(state: &AppState, access: &RepoAccess, subject: ObjectId, issue: &IssueDoc, content: &str, add: bool) -> Result<HttpResponse, ApiError> {
    let user = access.require(Role::Read)?;
    if !REACTIONS.contains(&content) {
        return Err(ApiError::field("content", format!("Reaction must be one of {}", REACTIONS.join(", "))));
    }
    if !can_comment(issue, access) {
        return Err(ApiError::forbidden("This conversation is locked"));
    }
    let key = doc! {"subject_id": subject, "user_id": user.id, "content": content};
    if add {
        let row = ReactionDoc {
            id: ObjectId::new(),
            subject_id: subject,
            repo_id: access.repo.id,
            user_id: user.id,
            user_login: user.login.clone(),
            content: content.to_string(),
            created_at: DateTime::now(),
        };
        match state.db.reactions.insert_one(row).await {
            Ok(_) => {}
            Err(e) if crate::error::is_duplicate(&e) => {}
            Err(e) => return Err(e.into()),
        }
    } else {
        state.db.reactions.delete_one(key).await?;
    }
    signal_issue(state, access.repo.id, issue.number);
    let map = crate::routes::issues::load_reactions(state, vec![subject]).await?;
    Ok(HttpResponse::Ok().json(crate::routes::issues::reaction_groups(map.get(&subject), access.viewer.as_ref())))
}

pub async fn react_issue(state: web::Data<AppState>, viewer: Viewer, path: web::Path<(String, String, i64, String)>, req: actix_web::HttpRequest) -> Result<HttpResponse, ApiError> {
    let (owner, name, number, content) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let (subject, issue) = reaction_target(&state, &access, Some(number), None).await?;
    react(&state, &access, subject, &issue, &content, req.method() == actix_web::http::Method::PUT).await
}

pub async fn react_comment(state: web::Data<AppState>, viewer: Viewer, path: web::Path<(String, String, String, String)>, req: actix_web::HttpRequest) -> Result<HttpResponse, ApiError> {
    let (owner, name, id, content) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let (subject, issue) = reaction_target(&state, &access, None, Some(&id)).await?;
    react(&state, &access, subject, &issue, &content, req.method() == actix_web::http::Method::PUT).await
}
