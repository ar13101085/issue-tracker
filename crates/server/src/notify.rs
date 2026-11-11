//! What happens after a change: subscriptions, inbox rows, emails and the live
//! update signal. Nothing here may fail the request that caused it, so errors
//! are logged and swallowed.

use crate::access::role_in;
use crate::error::ApiError;
use crate::mail::Mail;
use crate::models::{CommentDoc, IssueDoc, NotificationDoc, RepoDoc, SubscriptionDoc, UserDoc};
use crate::state::AppState;
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime};

/// Something changed; pages showing it should refetch.
#[derive(Debug, Clone)]
pub struct Signal {
    /// Set for repository changes: an issue, its timeline, or the list.
    pub repo_id: Option<ObjectId>,
    pub number: Option<i64>,
    /// Set when this user's inbox changed.
    pub user_id: Option<ObjectId>,
}

pub fn signal_issue(state: &AppState, repo_id: ObjectId, number: i64) {
    let _ = state.hub.send(Signal { repo_id: Some(repo_id), number: Some(number), user_id: None });
}

pub fn signal_repo(state: &AppState, repo_id: ObjectId) {
    let _ = state.hub.send(Signal { repo_id: Some(repo_id), number: None, user_id: None });
}

pub fn signal_user(state: &AppState, user_id: ObjectId) {
    let _ = state.hub.send(Signal { repo_id: None, number: None, user_id: Some(user_id) });
}

fn log(context: &str, result: Result<(), ApiError>) {
    if let Err(e) = result {
        tracing::warn!("notify ({context}): {e:?}");
    }
}

/// Subscribes unless the user already has a row (including an explicit unsubscribe).
async fn subscribe(state: &AppState, issue_id: ObjectId, user_id: ObjectId, reason: &str) -> Result<(), ApiError> {
    state
        .db
        .subscriptions
        .update_one(
            doc! {"issue_id": issue_id, "user_id": user_id},
            doc! {"$setOnInsert": {"_id": ObjectId::new(), "subscribed": true, "reason": reason}},
        )
        .upsert(true)
        .await?;
    Ok(())
}

async fn unsubscribed(state: &AppState, issue_id: ObjectId, user_id: ObjectId) -> Result<bool, ApiError> {
    let row = state.db.subscriptions.find_one(doc! {"issue_id": issue_id, "user_id": user_id}).await?;
    Ok(row.is_some_and(|s| !s.subscribed))
}

struct Activity<'a> {
    repo: &'a RepoDoc,
    issue: &'a IssueDoc,
    actor: &'a UserDoc,
    /// "commented", "closed this", ...
    verb: &'a str,
    /// Rendered comment, when there is one.
    html: Option<&'a str>,
    text: Option<&'a str>,
}

fn reason_text(reason: &str) -> &'static str {
    match reason {
        "mention" => "you were mentioned",
        "assign" => "you were assigned",
        "author" => "you authored the thread",
        "comment" => "you commented",
        "state_change" => "you changed the thread state",
        _ => "you are subscribed to this thread",
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Puts the activity in each recipient's inbox, and their mailbox if they want that.
async fn deliver(state: &AppState, a: &Activity<'_>, recipients: Vec<(ObjectId, String)>) -> Result<(), ApiError> {
    let mut seen: Vec<ObjectId> = Vec::new();
    for (user_id, reason) in recipients {
        if user_id == a.actor.id || seen.contains(&user_id) {
            continue;
        }
        seen.push(user_id);
        let Some(user) = state.db.users.find_one(doc! {"_id": user_id}).await? else {
            continue;
        };
        // Never tell someone about a repository they cannot open.
        if a.repo.private && role_in(state, a.repo, Some(&user)).await?.is_none() {
            continue;
        }
        if unsubscribed(state, a.issue.id, user_id).await? && reason != "mention" {
            continue;
        }
        state
            .db
            .notifications
            .update_one(
                doc! {"user_id": user_id, "issue_id": a.issue.id},
                doc! {
                    "$set": {"reason": &reason, "unread": true, "done": false, "updated_at": DateTime::now(), "last_actor_login": &a.actor.login, "repo_id": a.repo.id},
                    "$setOnInsert": {"_id": ObjectId::new()},
                },
            )
            .upsert(true)
            .await?;
        signal_user(state, user_id);

        if let (Some(queue), true) = (&state.mail, user.email_notifications) {
            let full = format!("{}/{}", a.repo.owner_login, a.repo.name);
            let link = format!("{}/{}/issues/{}", state.cfg.public_url, full, a.issue.number);
            let why = reason_text(&reason);
            let html = format!(
                "<p><strong>{actor}</strong> {verb} <a href=\"{link}\">{full}#{n}</a></p>{body}<hr>\
                 <p style=\"color:#59636e;font-size:12px\">You are receiving this because {why}.<br>\
                 <a href=\"{link}\">View it on Issue Tracker</a> or change email notifications in your settings.</p>",
                actor = escape(&a.actor.login),
                verb = a.verb,
                n = a.issue.number,
                body = a.html.unwrap_or(""),
            );
            let text = format!(
                "{} {} {}#{}\n\n{}\n\n--\nYou are receiving this because {}.\n{}\n",
                a.actor.login,
                a.verb,
                full,
                a.issue.number,
                a.text.unwrap_or(""),
                why,
                link
            );
            let subject = format!("Re: [{full}] {} (#{})", a.issue.title, a.issue.number);
            let _ = queue.send(Mail { to: user.email, subject, html, text });
        }
    }
    Ok(())
}

async fn subscribers(state: &AppState, issue_id: ObjectId) -> Result<Vec<(ObjectId, String)>, ApiError> {
    let rows: Vec<SubscriptionDoc> =
        state.db.subscriptions.find(doc! {"issue_id": issue_id, "subscribed": true}).await?.try_collect().await?;
    Ok(rows.into_iter().map(|s| (s.user_id, s.reason)).collect())
}

pub async fn issue_opened(
    state: &AppState,
    repo: &RepoDoc,
    issue: &IssueDoc,
    actor: &UserDoc,
    mentions: &[ObjectId],
    assignees: &[UserDoc],
) {
    let work = async {
        subscribe(state, issue.id, actor.id, "author").await?;
        let mut to: Vec<(ObjectId, String)> = Vec::new();
        for &id in mentions {
            subscribe(state, issue.id, id, "mention").await?;
            to.push((id, "mention".into()));
        }
        for u in assignees {
            subscribe(state, issue.id, u.id, "assign").await?;
            to.push((u.id, "assign".into()));
        }
        // The owner hears about every new issue in their repository.
        subscribe(state, issue.id, repo.owner_id, "subscribed").await?;
        to.push((repo.owner_id, "subscribed".into()));
        let a = Activity { repo, issue, actor, verb: "opened", html: Some(&issue.body_html), text: Some(&issue.body) };
        deliver(state, &a, to).await
    };
    log("issue opened", work.await);
    signal_repo(state, repo.id);
}

#[allow(clippy::too_many_arguments)]
pub async fn issue_changed(
    state: &AppState,
    repo: &RepoDoc,
    issue: &IssueDoc,
    actor: &UserDoc,
    closed: bool,
    reopened: bool,
    mentions: &[ObjectId],
    newly_assigned: &[UserDoc],
) {
    let work = async {
        let mut to: Vec<(ObjectId, String)> = Vec::new();
        for &id in mentions {
            subscribe(state, issue.id, id, "mention").await?;
            to.push((id, "mention".into()));
        }
        for u in newly_assigned {
            subscribe(state, issue.id, u.id, "assign").await?;
            to.push((u.id, "assign".into()));
        }
        if !to.is_empty() {
            let verb = if newly_assigned.is_empty() { "mentioned you in" } else { "assigned you to" };
            deliver(state, &Activity { repo, issue, actor, verb, html: None, text: None }, to).await?;
        }
        if closed || reopened {
            subscribe(state, issue.id, actor.id, "state_change").await?;
            let verb = if closed { "closed" } else { "reopened" };
            let all = subscribers(state, issue.id).await?;
            deliver(state, &Activity { repo, issue, actor, verb, html: None, text: None }, all).await?;
        }
        Ok(())
    };
    log("issue changed", work.await);
    signal_issue(state, repo.id, issue.number);
}

pub async fn commented(
    state: &AppState,
    repo: &RepoDoc,
    issue: &IssueDoc,
    actor: &UserDoc,
    comment: &CommentDoc,
    mentions: &[ObjectId],
) {
    let work = async {
        subscribe(state, issue.id, actor.id, "comment").await?;
        let mut to: Vec<(ObjectId, String)> = Vec::new();
        for &id in mentions {
            subscribe(state, issue.id, id, "mention").await?;
            to.push((id, "mention".into()));
        }
        to.extend(subscribers(state, issue.id).await?);
        let a = Activity { repo, issue, actor, verb: "commented on", html: Some(&comment.body_html), text: Some(&comment.body) };
        deliver(state, &a, to).await
    };
    log("commented", work.await);
    signal_issue(state, repo.id, issue.number);
}

pub async fn issue_deleted(state: &AppState, issue: &IssueDoc) {
    let work = async {
        let rows: Vec<NotificationDoc> = state.db.notifications.find(doc! {"issue_id": issue.id}).await?.try_collect().await?;
        state.db.notifications.delete_many(doc! {"issue_id": issue.id}).await?;
        state.db.subscriptions.delete_many(doc! {"issue_id": issue.id}).await?;
        for n in rows {
            signal_user(state, n.user_id);
        }
        Ok(())
    };
    log("issue deleted", work.await);
    signal_repo(state, issue.repo_id);
}
