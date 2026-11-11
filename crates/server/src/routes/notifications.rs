//! The inbox, per-issue subscriptions and the live update stream.

use crate::access::load_repo;
use crate::auth::Viewer;
use crate::error::ApiError;
use crate::models::{to_chrono, IssueDoc, NotificationDoc, RepoDoc};
use crate::notify::{signal_user, Signal};
use crate::routes::issues::{find_issue, issue_ref, IssuePath};
use crate::state::AppState;
use actix_web::web::Bytes;
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use serde::Deserialize;
use shared::{Notification, NotificationList, Subscription, UpdateNotification, UserRef};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Deserialize)]
pub struct ListQuery {
    /// `inbox` (default), `unread` or `done`.
    filter: Option<String>,
    page: Option<u64>,
}

const PER_PAGE: u64 = 25;

pub async fn list(state: web::Data<AppState>, viewer: Viewer, query: web::Query<ListQuery>) -> Result<HttpResponse, ApiError> {
    let user = viewer.require()?;
    let mut filter = doc! {"user_id": user.id};
    match query.filter.as_deref().unwrap_or("inbox") {
        "inbox" => {
            filter.insert("done", false);
        }
        "unread" => {
            filter.insert("done", false);
            filter.insert("unread", true);
        }
        "done" => {
            filter.insert("done", true);
        }
        other => return Err(ApiError::BadRequest(format!("filter must be inbox, unread or done, got {other}"))),
    }
    let total_count = state.db.notifications.count_documents(filter.clone()).await?;
    let unread_count = state.db.notifications.count_documents(doc! {"user_id": user.id, "unread": true, "done": false}).await?;
    let page = query.page.unwrap_or(1).max(1);
    let rows: Vec<NotificationDoc> = state
        .db
        .notifications
        .find(filter)
        .sort(doc! {"updated_at": -1})
        .skip((page - 1) * PER_PAGE)
        .limit(PER_PAGE as i64)
        .await?
        .try_collect()
        .await?;

    let issue_ids: Vec<ObjectId> = rows.iter().map(|n| n.issue_id).collect();
    let repo_ids: Vec<ObjectId> = rows.iter().map(|n| n.repo_id).collect();
    let issues: Vec<IssueDoc> = state.db.issues.find(doc! {"_id": {"$in": issue_ids}}).await?.try_collect().await?;
    let repos: Vec<RepoDoc> = state.db.repos.find(doc! {"_id": {"$in": repo_ids}}).await?.try_collect().await?;
    let issues: HashMap<ObjectId, IssueDoc> = issues.into_iter().map(|i| (i.id, i)).collect();
    let repos: HashMap<ObjectId, RepoDoc> = repos.into_iter().map(|r| (r.id, r)).collect();

    let items = rows
        .into_iter()
        .filter_map(|n| {
            let issue = issues.get(&n.issue_id)?;
            // The issue may have moved since the row was written.
            let repo = repos.get(&issue.repo_id).or_else(|| repos.get(&n.repo_id))?;
            Some(Notification {
                id: n.id.to_hex(),
                repo: format!("{}/{}", repo.owner_login, repo.name),
                issue: issue_ref(issue),
                reason: n.reason,
                unread: n.unread,
                done: n.done,
                last_actor: UserRef { login: n.last_actor_login },
                updated_at: to_chrono(n.updated_at),
            })
        })
        .collect();
    Ok(HttpResponse::Ok().json(NotificationList { unread_count, total_count, items }))
}

pub async fn update(state: web::Data<AppState>, viewer: Viewer, path: web::Path<String>, body: web::Json<UpdateNotification>) -> Result<HttpResponse, ApiError> {
    let user = viewer.require()?;
    let id = ObjectId::parse_str(path.as_str()).map_err(|_| ApiError::NotFound)?;
    let mut set = doc! {};
    if let Some(unread) = body.unread {
        set.insert("unread", unread);
    }
    if let Some(done) = body.done {
        set.insert("done", done);
        if done {
            set.insert("unread", false);
        }
    }
    if set.is_empty() {
        return Err(ApiError::BadRequest("Nothing to change".into()));
    }
    let result = state.db.notifications.update_one(doc! {"_id": id, "user_id": user.id}, doc! {"$set": set}).await?;
    if result.matched_count == 0 {
        return Err(ApiError::NotFound);
    }
    signal_user(&state, user.id);
    Ok(HttpResponse::NoContent().finish())
}

/// `POST /notifications/read`: mark the whole inbox read.
pub async fn read_all(state: web::Data<AppState>, viewer: Viewer) -> Result<HttpResponse, ApiError> {
    let user = viewer.require()?;
    state.db.notifications.update_many(doc! {"user_id": user.id, "unread": true}, doc! {"$set": {"unread": false}}).await?;
    signal_user(&state, user.id);
    Ok(HttpResponse::NoContent().finish())
}

pub async fn set_subscription(state: web::Data<AppState>, viewer: Viewer, path: IssuePath, body: web::Json<Subscription>) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let user = access.user()?;
    let issue = find_issue(&state, &access, number).await?;
    state
        .db
        .subscriptions
        .update_one(
            doc! {"issue_id": issue.id, "user_id": user.id},
            doc! {"$set": {"subscribed": body.subscribed, "reason": "subscribed"}, "$setOnInsert": {"_id": ObjectId::new()}},
        )
        .upsert(true)
        .await?;
    Ok(HttpResponse::Ok().json(Subscription { subscribed: body.subscribed }))
}

#[derive(Deserialize)]
pub struct StreamQuery {
    /// `owner/name`: also receive that repository's changes.
    repo: Option<String>,
}

/// `GET /stream`: server-sent events. `issue` carries the number that changed,
/// `repo` means the list changed, `notifications` means the viewer's inbox changed.
/// The events only say what to refetch, so nothing private travels on the stream.
pub async fn stream(state: web::Data<AppState>, viewer: Viewer, query: web::Query<StreamQuery>) -> Result<HttpResponse, ApiError> {
    let viewer = viewer.0;
    let repo_id = match query.repo.as_deref().and_then(|r| r.split_once('/')) {
        Some((owner, name)) => Some(load_repo(&state, owner, name, viewer.clone()).await?.repo.id),
        None => None,
    };
    let user_id = viewer.map(|u| u.id);
    let rx = state.hub.subscribe();

    let events = futures::stream::unfold((rx, true), move |(mut rx, first)| async move {
        if first {
            return Some((Ok::<_, actix_web::Error>(Bytes::from_static(b"retry: 3000\n\n")), (rx, false)));
        }
        loop {
            let next = tokio::time::timeout(Duration::from_secs(25), rx.recv()).await;
            let frame = match next {
                // Keeps proxies from closing an idle connection.
                Err(_) => Bytes::from_static(b": ping\n\n"),
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => Bytes::from_static(b"event: repo\ndata: {}\n\n"),
                Ok(Err(_)) => return None,
                Ok(Ok(Signal { repo_id: Some(r), number, .. })) if Some(r) == repo_id => match number {
                    Some(n) => Bytes::from(format!("event: issue\ndata: {{\"number\":{n}}}\n\n")),
                    None => Bytes::from_static(b"event: repo\ndata: {}\n\n"),
                },
                Ok(Ok(Signal { user_id: Some(u), .. })) if Some(u) == user_id => Bytes::from_static(b"event: notifications\ndata: {}\n\n"),
                Ok(Ok(_)) => continue,
            };
            return Some((Ok(frame), (rx, false)));
        }
    });
    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        // Stops the compression middleware from buffering the stream.
        .insert_header(("Content-Encoding", "identity"))
        .streaming(events))
}
