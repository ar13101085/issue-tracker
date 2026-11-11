//! MongoDB documents.

use mongodb::bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};
use shared::{EventKind, IssueState, LabelRef, Rename, Role, StateReason};

pub fn to_chrono(d: DateTime) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_millis(d.timestamp_millis()).unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub login: String,
    pub login_lower: String,
    /// Stored lowercased.
    pub email: String,
    pub password_hash: String,
    pub name: Option<String>,
    pub is_admin: bool,
    #[serde(default = "yes")]
    pub email_notifications: bool,
    pub created_at: DateTime,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDoc {
    /// SHA-256 of the cookie value; the raw token is never stored.
    #[serde(rename = "_id")]
    pub id: String,
    pub user_id: ObjectId,
    pub expires_at: DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub owner_id: ObjectId,
    pub owner_login: String,
    pub owner_lower: String,
    pub name: String,
    pub name_lower: String,
    pub description: String,
    pub private: bool,
    /// Last issue number handed out.
    pub issue_seq: i64,
    #[serde(default)]
    pub milestone_seq: i64,
    #[serde(default)]
    pub project_seq: i64,
    #[serde(default)]
    pub templates: Vec<shared::Template>,
    pub created_at: DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaboratorDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub repo_id: ObjectId,
    pub user_id: ObjectId,
    pub user_login: String,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub repo_id: ObjectId,
    pub number: i64,
    pub title: String,
    pub body: String,
    /// Rendered when the body is written, so reads do not render.
    #[serde(default)]
    pub body_html: String,
    #[serde(default)]
    pub edits: Vec<EditDoc>,
    pub state: IssueState,
    pub state_reason: Option<StateReason>,
    pub author_id: ObjectId,
    pub author_login: String,
    pub comments: i64,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub closed_at: Option<DateTime>,
    #[serde(default)]
    pub label_ids: Vec<ObjectId>,
    #[serde(default)]
    pub assignee_ids: Vec<ObjectId>,
    #[serde(default)]
    pub milestone_id: Option<ObjectId>,
    #[serde(default)]
    pub issue_type: Option<String>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub lock_reason: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    /// Set on a sub-issue.
    #[serde(default)]
    pub parent_id: Option<ObjectId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub repo_id: ObjectId,
    pub name: String,
    pub name_lower: String,
    pub color: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub repo_id: ObjectId,
    pub number: i64,
    pub title: String,
    pub title_lower: String,
    pub description: String,
    pub state: IssueState,
    pub due_on: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub closed_at: Option<DateTime>,
}

/// One emoji from one user on an issue body or a comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    /// The issue or comment reacted to.
    pub subject_id: ObjectId,
    pub repo_id: ObjectId,
    pub user_id: ObjectId,
    pub user_login: String,
    pub content: String,
    pub created_at: DateTime,
}

/// A replaced version of a body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditDoc {
    pub editor_login: String,
    pub edited_at: DateTime,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub repo_id: ObjectId,
    pub issue_id: ObjectId,
    pub author_id: ObjectId,
    pub author_login: String,
    pub body: String,
    pub body_html: String,
    #[serde(default)]
    pub edits: Vec<EditDoc>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

/// Timeline entry that is not a comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub repo_id: ObjectId,
    pub issue_id: ObjectId,
    pub kind: EventKind,
    pub actor_id: ObjectId,
    pub actor_login: String,
    pub created_at: DateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_reason: Option<StateReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename: Option<Rename>,
    /// The other issue: the one that mentioned this one, a sub-issue, a parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_issue_id: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<LabelRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee_login: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl EventDoc {
    pub fn new(issue: &IssueDoc, actor: &UserDoc, kind: EventKind) -> Self {
        Self {
            id: ObjectId::new(),
            repo_id: issue.repo_id,
            issue_id: issue.id,
            kind,
            actor_id: actor.id,
            actor_login: actor.login.clone(),
            created_at: DateTime::now(),
            state_reason: None,
            rename: None,
            source_issue_id: None,
            label: None,
            assignee_login: None,
            milestone: None,
            detail: None,
        }
    }
}

/// Whether a user follows an issue. A row with `subscribed: false` is an explicit
/// unsubscribe and stops later activity from subscribing them again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub issue_id: ObjectId,
    pub user_id: ObjectId,
    pub subscribed: bool,
    pub reason: String,
}

/// Inbox row: one per user and issue, refreshed by each new activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub user_id: ObjectId,
    pub repo_id: ObjectId,
    pub issue_id: ObjectId,
    pub reason: String,
    pub unread: bool,
    pub done: bool,
    pub last_actor_login: String,
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub repo_id: ObjectId,
    pub number: i64,
    pub title: String,
    pub description: String,
    pub closed: bool,
    pub fields: Vec<shared::ProjectField>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectItemDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub project_id: ObjectId,
    pub repo_id: ObjectId,
    pub issue_id: ObjectId,
    /// Field id to value.
    pub values: mongodb::bson::Document,
    pub position: f64,
    pub created_at: DateTime,
}

/// Personal access token: what the CLI signs in with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub user_id: ObjectId,
    pub name: String,
    /// SHA-256 of the secret; the secret itself is never stored.
    pub hash: String,
    pub prefix: String,
    pub created_at: DateTime,
    pub last_used_at: Option<DateTime>,
}
