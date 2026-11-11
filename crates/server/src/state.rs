//! Database handles and indexes.

use crate::config::Config;
use crate::models::*;
use mongodb::bson::{doc, Document};
use mongodb::options::IndexOptions;
use mongodb::{Client, Collection, Database, IndexModel};
use std::time::Duration;

pub struct Db {
    pub database: Database,
    pub users: Collection<UserDoc>,
    pub sessions: Collection<SessionDoc>,
    pub repos: Collection<RepoDoc>,
    pub collaborators: Collection<CollaboratorDoc>,
    pub issues: Collection<IssueDoc>,
    pub comments: Collection<CommentDoc>,
    pub events: Collection<EventDoc>,
    pub labels: Collection<LabelDoc>,
    pub milestones: Collection<MilestoneDoc>,
    pub reactions: Collection<ReactionDoc>,
    pub subscriptions: Collection<SubscriptionDoc>,
    pub notifications: Collection<NotificationDoc>,
    pub projects: Collection<ProjectDoc>,
    pub project_items: Collection<ProjectItemDoc>,
    pub tokens: Collection<TokenDoc>,
}

pub struct AppState {
    pub db: Db,
    pub cfg: Config,
    /// Live updates: every change is announced here and fanned out to open pages.
    pub hub: tokio::sync::broadcast::Sender<crate::notify::Signal>,
    /// Outgoing email queue; `None` when SMTP is not configured.
    pub mail: Option<tokio::sync::mpsc::UnboundedSender<crate::mail::Mail>>,
    /// Failed sign-in counter.
    pub limiter: crate::ratelimit::Limiter,
}

fn unique(keys: Document) -> IndexModel {
    IndexModel::builder().keys(keys).options(IndexOptions::builder().unique(true).build()).build()
}

fn plain(keys: Document) -> IndexModel {
    IndexModel::builder().keys(keys).build()
}

pub async fn init(cfg: Config) -> Result<AppState, String> {
    let client = Client::with_uri_str(&cfg.mongodb_uri).await.map_err(|e| format!("MongoDB: {e}"))?;
    let database = client.database(&cfg.mongodb_db);
    let db = Db {
        users: database.collection("users"),
        sessions: database.collection("sessions"),
        repos: database.collection("repos"),
        collaborators: database.collection("collaborators"),
        issues: database.collection("issues"),
        comments: database.collection("comments"),
        events: database.collection("events"),
        labels: database.collection("labels"),
        milestones: database.collection("milestones"),
        reactions: database.collection("reactions"),
        subscriptions: database.collection("subscriptions"),
        notifications: database.collection("notifications"),
        projects: database.collection("projects"),
        project_items: database.collection("project_items"),
        tokens: database.collection("tokens"),
        database,
    };
    let err = |e: mongodb::error::Error| format!("MongoDB at {}: {e}", cfg.mongodb_uri);
    db.users.create_index(unique(doc! {"login_lower": 1})).await.map_err(err)?;
    db.users.create_index(unique(doc! {"email": 1})).await.map_err(err)?;
    let ttl = IndexOptions::builder().expire_after(Duration::from_secs(0)).build();
    db.sessions
        .create_index(IndexModel::builder().keys(doc! {"expires_at": 1}).options(ttl).build())
        .await
        .map_err(err)?;
    db.repos.create_index(unique(doc! {"owner_lower": 1, "name_lower": 1})).await.map_err(err)?;
    db.collaborators.create_index(unique(doc! {"repo_id": 1, "user_id": 1})).await.map_err(err)?;
    db.collaborators.create_index(plain(doc! {"user_id": 1})).await.map_err(err)?;
    db.issues.create_index(unique(doc! {"repo_id": 1, "number": 1})).await.map_err(err)?;
    db.issues.create_index(plain(doc! {"repo_id": 1, "state": 1, "created_at": -1})).await.map_err(err)?;
    db.comments.create_index(plain(doc! {"issue_id": 1, "created_at": 1})).await.map_err(err)?;
    db.events.create_index(plain(doc! {"issue_id": 1, "created_at": 1})).await.map_err(err)?;
    db.labels.create_index(unique(doc! {"repo_id": 1, "name_lower": 1})).await.map_err(err)?;
    db.milestones.create_index(unique(doc! {"repo_id": 1, "number": 1})).await.map_err(err)?;
    db.milestones.create_index(unique(doc! {"repo_id": 1, "title_lower": 1})).await.map_err(err)?;
    db.reactions.create_index(unique(doc! {"subject_id": 1, "user_id": 1, "content": 1})).await.map_err(err)?;
    db.issues.create_index(plain(doc! {"parent_id": 1})).await.map_err(err)?;
    db.subscriptions.create_index(unique(doc! {"issue_id": 1, "user_id": 1})).await.map_err(err)?;
    db.notifications.create_index(unique(doc! {"user_id": 1, "issue_id": 1})).await.map_err(err)?;
    db.notifications.create_index(plain(doc! {"user_id": 1, "done": 1, "updated_at": -1})).await.map_err(err)?;
    db.projects.create_index(unique(doc! {"repo_id": 1, "number": 1})).await.map_err(err)?;
    db.project_items.create_index(unique(doc! {"project_id": 1, "issue_id": 1})).await.map_err(err)?;
    db.project_items.create_index(plain(doc! {"issue_id": 1})).await.map_err(err)?;
    db.tokens.create_index(unique(doc! {"hash": 1})).await.map_err(err)?;
    db.tokens.create_index(plain(doc! {"user_id": 1})).await.map_err(err)?;
    let (hub, _) = tokio::sync::broadcast::channel(256);
    let mail = crate::mail::start(&cfg)?;
    Ok(AppState { db, cfg, hub, mail, limiter: Default::default() })
}
