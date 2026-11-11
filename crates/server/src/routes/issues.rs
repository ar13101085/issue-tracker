use crate::access::{load_repo, RepoAccess};
use crate::auth::Viewer;
use crate::error::ApiError;
use crate::markdown;
use crate::models::{
    to_chrono, CollaboratorDoc, EditDoc, EventDoc, IssueDoc, LabelDoc, MilestoneDoc, ReactionDoc, RepoDoc, UserDoc,
};
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, to_bson, Bson, DateTime, Document};
use mongodb::options::ReturnDocument;
use serde::Deserialize;
use shared::query::{self, IssueQuery, Missing, SortField};
use shared::{
    Association, CreateIssue, Edit, EventKind, Issue, IssueList, IssueRef, IssueState, Label, LabelRef, MilestoneRef,
    ReactionGroup, Rename, Role, StateReason, UpdateIssue, UserRef, ISSUE_TYPES, REACTIONS,
};
use std::collections::{HashMap, HashSet};

type RepoPath = web::Path<(String, String)>;
pub type IssuePath = web::Path<(String, String, i64)>;

const TITLE_MAX: usize = 256;
pub const BODY_MAX: usize = 65_536;
const MAX_ASSIGNEES: usize = 10;

/// Who is the owner and who collaborates, loaded once per request for the author badges.
pub struct Associations {
    owner: ObjectId,
    collaborators: HashSet<ObjectId>,
}

impl Associations {
    pub async fn load(state: &AppState, repo: &RepoDoc) -> Result<Self, ApiError> {
        let docs: Vec<CollaboratorDoc> = state.db.collaborators.find(doc! {"repo_id": repo.id}).await?.try_collect().await?;
        Ok(Self { owner: repo.owner_id, collaborators: docs.into_iter().map(|c| c.user_id).collect() })
    }

    pub fn of(&self, user: ObjectId) -> Association {
        if user == self.owner {
            Association::Owner
        } else if self.collaborators.contains(&user) {
            Association::Collaborator
        } else {
            Association::None
        }
    }
}

pub fn issue_ref(i: &IssueDoc) -> IssueRef {
    IssueRef { number: i.number, title: i.title.clone(), state: i.state, state_reason: i.state_reason }
}

pub fn reaction_groups(list: Option<&Vec<ReactionDoc>>, viewer: Option<&UserDoc>) -> Vec<ReactionGroup> {
    let Some(list) = list else {
        return Vec::new();
    };
    REACTIONS
        .iter()
        .filter_map(|content| {
            let of_kind: Vec<&ReactionDoc> = list.iter().filter(|r| r.content == *content).collect();
            if of_kind.is_empty() {
                return None;
            }
            Some(ReactionGroup {
                content: content.to_string(),
                count: of_kind.len(),
                viewer_reacted: viewer.is_some_and(|v| of_kind.iter().any(|r| r.user_id == v.id)),
                users: of_kind.iter().take(10).map(|r| r.user_login.clone()).collect(),
            })
        })
        .collect()
}

pub async fn load_reactions(state: &AppState, subjects: Vec<ObjectId>) -> Result<HashMap<ObjectId, Vec<ReactionDoc>>, ApiError> {
    let mut map: HashMap<ObjectId, Vec<ReactionDoc>> = HashMap::new();
    if subjects.is_empty() {
        return Ok(map);
    }
    let docs: Vec<ReactionDoc> = state
        .db
        .reactions
        .find(doc! {"subject_id": {"$in": subjects}})
        .sort(doc! {"created_at": 1})
        .await?
        .try_collect()
        .await?;
    for r in docs {
        map.entry(r.subject_id).or_default().push(r);
    }
    Ok(map)
}

/// Everything needed to turn issue documents into API issues, fetched in a few
/// queries for the whole batch instead of per issue.
pub struct IssueCtx {
    pub assoc: Associations,
    labels: HashMap<ObjectId, LabelDoc>,
    milestones: HashMap<ObjectId, MilestoneDoc>,
    users: HashMap<ObjectId, String>,
    parents: HashMap<ObjectId, IssueRef>,
    children: HashMap<ObjectId, Vec<IssueRef>>,
    /// Every issue named in a `blocked_by_ids` or `related_ids` of the loaded issues.
    blockers: HashMap<ObjectId, IssueRef>,
    /// For each loaded issue, the issues it blocks.
    blocking: HashMap<ObjectId, Vec<IssueRef>>,
    reactions: HashMap<ObjectId, Vec<ReactionDoc>>,
    subscribed: HashSet<ObjectId>,
}

impl IssueCtx {
    /// `detail` also loads reactions and the viewer's subscription, which lists do not show.
    pub async fn load(state: &AppState, access: &RepoAccess, issues: &[IssueDoc], detail: bool) -> Result<Self, ApiError> {
        let repo = &access.repo;
        let with_reactions = detail;
        let assoc = Associations::load(state, repo).await?;
        let labels: Vec<LabelDoc> = state.db.labels.find(doc! {"repo_id": repo.id}).await?.try_collect().await?;
        let milestones: Vec<MilestoneDoc> = state.db.milestones.find(doc! {"repo_id": repo.id}).await?.try_collect().await?;

        let user_ids: HashSet<ObjectId> = issues.iter().flat_map(|i| i.assignee_ids.iter().copied()).collect();
        let mut users = HashMap::new();
        if !user_ids.is_empty() {
            let ids: Vec<ObjectId> = user_ids.into_iter().collect();
            let docs: Vec<UserDoc> = state.db.users.find(doc! {"_id": {"$in": ids}}).await?.try_collect().await?;
            users = docs.into_iter().map(|u| (u.id, u.login)).collect();
        }

        let ids: Vec<ObjectId> = issues.iter().map(|i| i.id).collect();
        let parent_ids: Vec<ObjectId> = issues.iter().filter_map(|i| i.parent_id).collect();
        let mut parents = HashMap::new();
        if !parent_ids.is_empty() {
            let docs: Vec<IssueDoc> = state.db.issues.find(doc! {"_id": {"$in": parent_ids}}).await?.try_collect().await?;
            parents = docs.iter().map(|i| (i.id, issue_ref(i))).collect();
        }
        let mut children: HashMap<ObjectId, Vec<IssueRef>> = HashMap::new();
        if !ids.is_empty() {
            let docs: Vec<IssueDoc> =
                state.db.issues.find(doc! {"parent_id": {"$in": ids.clone()}}).sort(doc! {"number": 1}).await?.try_collect().await?;
            for c in docs {
                if let Some(p) = c.parent_id {
                    children.entry(p).or_default().push(issue_ref(&c));
                }
            }
        }
        let blocker_ids: Vec<ObjectId> = issues.iter().flat_map(|i| i.blocked_by_ids.iter().chain(i.related_ids.iter()).copied()).collect();
        let mut blockers = HashMap::new();
        if !blocker_ids.is_empty() {
            let docs: Vec<IssueDoc> = state.db.issues.find(doc! {"_id": {"$in": blocker_ids}}).await?.try_collect().await?;
            blockers = docs.iter().map(|i| (i.id, issue_ref(i))).collect();
        }
        let mut blocking: HashMap<ObjectId, Vec<IssueRef>> = HashMap::new();
        if !ids.is_empty() {
            let docs: Vec<IssueDoc> =
                state.db.issues.find(doc! {"blocked_by_ids": {"$in": ids.clone()}}).sort(doc! {"number": 1}).await?.try_collect().await?;
            for waiting in docs {
                for id in &waiting.blocked_by_ids {
                    blocking.entry(*id).or_default().push(issue_ref(&waiting));
                }
            }
        }
        let reactions = if with_reactions { load_reactions(state, ids.clone()).await? } else { HashMap::new() };
        let mut subscribed = HashSet::new();
        if let (true, Some(viewer)) = (detail, &access.viewer) {
            let rows: Vec<crate::models::SubscriptionDoc> = state
                .db
                .subscriptions
                .find(doc! {"issue_id": {"$in": ids}, "user_id": viewer.id, "subscribed": true})
                .await?
                .try_collect()
                .await?;
            subscribed = rows.into_iter().map(|s| s.issue_id).collect();
        }
        Ok(Self {
            subscribed,
            assoc,
            labels: labels.into_iter().map(|l| (l.id, l)).collect(),
            milestones: milestones.into_iter().map(|m| (m.id, m)).collect(),
            users,
            parents,
            children,
            blockers,
            blocking,
            reactions,
        })
    }
}

pub fn label_dto(l: &LabelDoc) -> Label {
    Label { name: l.name.clone(), color: l.color.clone(), description: l.description.clone() }
}

/// A locked conversation only takes comments from the owner and collaborators with triage or more.
pub fn can_comment(issue: &IssueDoc, access: &RepoAccess) -> bool {
    access.viewer.is_some() && (!issue.locked || access.has(Role::Triage))
}

pub fn issue_dto(doc: IssueDoc, access: &RepoAccess, ctx: &IssueCtx) -> Issue {
    let is_author = access.viewer.as_ref().is_some_and(|u| u.id == doc.author_id);
    let mut labels: Vec<Label> = doc.label_ids.iter().filter_map(|id| ctx.labels.get(id)).map(label_dto).collect();
    labels.sort_by_key(|l| l.name.to_lowercase());
    let viewer_can_comment = can_comment(&doc, access);
    let viewer_subscribed = ctx.subscribed.contains(&doc.id);
    Issue {
        number: doc.number,
        title: doc.title,
        body: doc.body,
        body_html: doc.body_html,
        state: doc.state,
        state_reason: doc.state_reason,
        author: UserRef { login: doc.author_login },
        author_association: ctx.assoc.of(doc.author_id),
        edit_count: doc.edits.len(),
        comments: doc.comments,
        created_at: to_chrono(doc.created_at),
        updated_at: to_chrono(doc.updated_at),
        closed_at: doc.closed_at.map(to_chrono),
        labels,
        assignees: doc.assignee_ids.iter().filter_map(|id| ctx.users.get(id)).map(|l| UserRef { login: l.clone() }).collect(),
        milestone: doc
            .milestone_id
            .and_then(|id| ctx.milestones.get(&id))
            .map(|m| MilestoneRef { number: m.number, title: m.title.clone() }),
        issue_type: doc.issue_type,
        locked: doc.locked,
        lock_reason: doc.lock_reason,
        pinned: doc.pinned,
        parent: doc.parent_id.and_then(|id| ctx.parents.get(&id).cloned()),
        sub_issues: ctx.children.get(&doc.id).cloned().unwrap_or_default(),
        blocked_by: doc.blocked_by_ids.iter().filter_map(|id| ctx.blockers.get(id).cloned()).collect(),
        blocking: ctx.blocking.get(&doc.id).cloned().unwrap_or_default(),
        related: doc.related_ids.iter().filter_map(|id| ctx.blockers.get(id).cloned()).collect(),
        reactions: reaction_groups(ctx.reactions.get(&doc.id), access.viewer.as_ref()),
        viewer_can_edit: is_author || access.has(Role::Write),
        viewer_can_close: is_author || access.has(Role::Triage),
        viewer_can_triage: access.has(Role::Triage),
        viewer_can_comment,
        viewer_subscribed,
    }
}

/// One issue as the API returns it.
pub async fn one(state: &AppState, access: &RepoAccess, issue: IssueDoc) -> Result<Issue, ApiError> {
    let ctx = IssueCtx::load(state, access, std::slice::from_ref(&issue), true).await?;
    Ok(issue_dto(issue, access, &ctx))
}

fn check_title(title: &str) -> Result<String, ApiError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ApiError::field("title", "Title can't be blank"));
    }
    if title.chars().count() > TITLE_MAX {
        return Err(ApiError::field("title", "Title is too long (maximum is 256 characters)"));
    }
    Ok(title.to_string())
}

pub fn check_body(body: &str) -> Result<(), ApiError> {
    if body.chars().count() > BODY_MAX {
        return Err(ApiError::field("body", "Body is too long (maximum is 65536 characters)"));
    }
    Ok(())
}

pub fn bson<T: serde::Serialize>(value: &T) -> Result<Bson, ApiError> {
    to_bson(value).map_err(|e| ApiError::Internal(e.to_string()))
}

pub fn edits_dto(edits: Vec<EditDoc>) -> Vec<Edit> {
    edits
        .into_iter()
        .rev()
        .map(|e| Edit { editor: UserRef { login: e.editor_login }, edited_at: to_chrono(e.edited_at), body: e.body })
        .collect()
}

/// `source` mentioned `#n` for each of `numbers`: leave one "mentioned this" entry on each target.
pub async fn record_references(
    state: &AppState,
    source: &IssueDoc,
    actor: &UserDoc,
    numbers: &[i64],
) -> Result<(), ApiError> {
    for &n in numbers.iter().filter(|&&n| n != source.number) {
        let Some(target) = state.db.issues.find_one(doc! {"repo_id": source.repo_id, "number": n}).await? else {
            continue;
        };
        let seen = doc! {"issue_id": target.id, "kind": bson(&EventKind::CrossReferenced)?, "source_issue_id": source.id};
        if state.db.events.find_one(seen).await?.is_none() {
            let mut event = EventDoc::new(&target, actor, EventKind::CrossReferenced);
            event.source_issue_id = Some(source.id);
            state.db.events.insert_one(event).await?;
        }
    }
    Ok(())
}

fn escape_regex(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        if r"\.+*?()|[]{}^$-/".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Label names to documents; an unknown name is a 422 on `field`.
pub async fn resolve_labels(state: &AppState, repo: &RepoDoc, names: &[String]) -> Result<Vec<LabelDoc>, ApiError> {
    let mut out: Vec<LabelDoc> = Vec::new();
    for name in names {
        let found = state.db.labels.find_one(doc! {"repo_id": repo.id, "name_lower": name.trim().to_lowercase()}).await?;
        let label = found.ok_or_else(|| ApiError::field("labels", format!("Label \"{name}\" does not exist")))?;
        if !out.iter().any(|l| l.id == label.id) {
            out.push(label);
        }
    }
    Ok(out)
}

/// Logins to users who can be assigned here: the owner and collaborators.
pub async fn resolve_assignees(state: &AppState, repo: &RepoDoc, logins: &[String]) -> Result<Vec<UserDoc>, ApiError> {
    let assoc = Associations::load(state, repo).await?;
    let mut out: Vec<UserDoc> = Vec::new();
    for login in logins {
        let user = state.db.users.find_one(doc! {"login_lower": login.trim().to_lowercase()}).await?;
        let user = user
            .filter(|u| assoc.of(u.id) != Association::None)
            .ok_or_else(|| ApiError::field("assignees", format!("{login} cannot be assigned to issues in this repository")))?;
        if !out.iter().any(|u| u.id == user.id) {
            out.push(user);
        }
    }
    if out.len() > MAX_ASSIGNEES {
        return Err(ApiError::field("assignees", "An issue can have at most 10 assignees"));
    }
    Ok(out)
}

pub async fn resolve_milestone(state: &AppState, repo: &RepoDoc, number: i64) -> Result<MilestoneDoc, ApiError> {
    state
        .db
        .milestones
        .find_one(doc! {"repo_id": repo.id, "number": number})
        .await?
        .ok_or_else(|| ApiError::field("milestone", "Milestone does not exist"))
}

pub fn resolve_type(name: &str) -> Result<String, ApiError> {
    ISSUE_TYPES
        .iter()
        .find(|t| t.eq_ignore_ascii_case(name.trim()))
        .map(|t| t.to_string())
        .ok_or_else(|| ApiError::field("issue_type", format!("Issue type must be one of {}", ISSUE_TYPES.join(", "))))
}

/// The Mongo filter for a parsed search, without the state part.
async fn query_filter(state: &AppState, access: &RepoAccess, q: &IssueQuery) -> Result<Document, ApiError> {
    let repo = &access.repo;
    let nothing = || doc! {"_id": {"$in": Vec::<ObjectId>::new()}};
    let mut and: Vec<Document> = vec![doc! {"repo_id": repo.id}];

    if !q.text.is_empty() {
        let re = doc! {"$regex": escape_regex(&q.text), "$options": "i"};
        let mut or = Vec::new();
        if q.in_title {
            or.push(doc! {"title": re.clone()});
        }
        if q.in_body {
            or.push(doc! {"body": re});
        }
        and.push(doc! {"$or": or});
    }

    let label_id = |name: &str| {
        let filter = doc! {"repo_id": repo.id, "name_lower": name.to_lowercase()};
        async move { state.db.labels.find_one(filter).await.map(|l| l.map(|l| l.id)) }
    };
    let mut wanted = Vec::new();
    for name in &q.labels {
        match label_id(name).await? {
            Some(id) => wanted.push(id),
            None => and.push(nothing()),
        }
    }
    if !wanted.is_empty() {
        and.push(doc! {"label_ids": {"$all": wanted}});
    }
    let mut unwanted = Vec::new();
    for name in &q.not_labels {
        if let Some(id) = label_id(name).await? {
            unwanted.push(id);
        }
    }
    if !unwanted.is_empty() {
        and.push(doc! {"label_ids": {"$nin": unwanted}});
    }

    for missing in &q.missing {
        and.push(match missing {
            Missing::Label => doc! {"label_ids.0": {"$exists": false}},
            Missing::Assignee => doc! {"assignee_ids.0": {"$exists": false}},
            Missing::Milestone => doc! {"milestone_id": Bson::Null},
            Missing::Type => doc! {"issue_type": Bson::Null},
        });
    }

    // `@me` is the caller.
    let user_id = |login: &str| {
        let me = access.viewer.as_ref().map(|u| u.id);
        let filter = doc! {"login_lower": login.to_lowercase()};
        let is_me = login.eq_ignore_ascii_case("@me");
        async move {
            if is_me {
                return Ok::<_, ApiError>(me);
            }
            Ok(state.db.users.find_one(filter).await?.map(|u| u.id))
        }
    };
    if let Some(login) = &q.author {
        and.push(match user_id(login).await? {
            Some(id) => doc! {"author_id": id},
            None => nothing(),
        });
    }
    if let Some(login) = &q.assignee {
        and.push(match user_id(login).await? {
            Some(id) => doc! {"assignee_ids": id},
            None => nothing(),
        });
    }
    if let Some(login) = &q.mentions {
        and.push(match user_id(login).await? {
            Some(id) => doc! {"mentioned_ids": id},
            None => nothing(),
        });
    }
    if let Some(login) = &q.commenter {
        and.push(match user_id(login).await? {
            Some(id) => doc! {"commenter_ids": id},
            None => nothing(),
        });
    }
    if let Some(login) = &q.involves {
        and.push(match user_id(login).await? {
            Some(id) => doc! {"$or": [{"author_id": id}, {"assignee_ids": id}, {"mentioned_ids": id}, {"commenter_ids": id}]},
            None => nothing(),
        });
    }
    if q.blocked {
        and.push(doc! {"blocked_by_ids.0": {"$exists": true}});
    }
    if q.blocking {
        let ids = state.db.issues.distinct("blocked_by_ids", doc! {"repo_id": repo.id}).await?;
        and.push(doc! {"_id": {"$in": ids}});
    }
    if let Some(title) = &q.milestone {
        let found = state.db.milestones.find_one(doc! {"repo_id": repo.id, "title_lower": title.to_lowercase()}).await?;
        and.push(match found {
            Some(m) => doc! {"milestone_id": m.id},
            None => nothing(),
        });
    }
    if let Some(name) = &q.issue_type {
        and.push(match resolve_type(name) {
            Ok(t) => doc! {"issue_type": t},
            Err(_) => nothing(),
        });
    }
    Ok(doc! {"$and": and})
}

#[derive(Deserialize)]
pub struct ListQuery {
    /// Search syntax, see `shared::query`. Its `state:` and `sort:` are used
    /// unless the explicit parameters below are given.
    q: Option<String>,
    /// `open`, `closed` or `all`. Without `q` the default is `open`.
    state: Option<String>,
    /// `created`, `updated` or `comments`.
    sort: Option<String>,
    /// `desc` or `asc`.
    direction: Option<String>,
    page: Option<u64>,
    per_page: Option<u64>,
}

fn and_state(filter: &Document, state: &str) -> Document {
    doc! {"$and": [filter.clone(), {"state": state}]}
}

pub async fn list(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: RepoPath,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let issues = &state.db.issues;
    let parsed = query::parse(query.q.as_deref().unwrap_or(""));
    let base = query_filter(&state, &access, &parsed).await?;

    let wanted = match query.state.as_deref() {
        Some("open") => Some(IssueState::Open),
        Some("closed") => Some(IssueState::Closed),
        Some("all") => None,
        Some(other) => return Err(ApiError::BadRequest(format!("state must be open, closed or all, got {other}"))),
        None if query.q.is_some() => parsed.state,
        None => Some(IssueState::Open),
    };
    let open_count = issues.count_documents(and_state(&base, "open")).await?;
    let closed_count = issues.count_documents(and_state(&base, "closed")).await?;
    let (filter, total_count) = match wanted {
        Some(IssueState::Open) => (and_state(&base, "open"), open_count),
        Some(IssueState::Closed) => (and_state(&base, "closed"), closed_count),
        None => (base, open_count + closed_count),
    };

    let field = match query.sort.as_deref() {
        Some("created") => SortField::Created,
        Some("updated") => SortField::Updated,
        Some("comments") => SortField::Comments,
        Some(other) => return Err(ApiError::BadRequest(format!("sort must be created, updated or comments, got {other}"))),
        None => parsed.sort,
    };
    let field = match field {
        SortField::Created => "created_at",
        SortField::Updated => "updated_at",
        SortField::Comments => "comments",
    };
    let ascending = match query.direction.as_deref() {
        Some(d) => d == "asc",
        None => parsed.ascending,
    };
    let dir = if ascending { 1 } else { -1 };
    let per_page = query.per_page.unwrap_or(25).clamp(1, 100);
    let page = query.page.unwrap_or(1).max(1);
    let docs: Vec<IssueDoc> = issues
        .find(filter)
        .sort(doc! {field: dir, "number": dir})
        .skip((page - 1) * per_page)
        .limit(per_page as i64)
        .await?
        .try_collect()
        .await?;
    let ctx = IssueCtx::load(&state, &access, &docs, false).await?;
    let items = docs.into_iter().map(|d| issue_dto(d, &access, &ctx)).collect();
    Ok(HttpResponse::Ok().json(IssueList { total_count, open_count, closed_count, page, per_page, items }))
}

/// Hands out the next issue number of a repository.
pub async fn next_number(state: &AppState, repo_id: ObjectId) -> Result<i64, ApiError> {
    // The repository hands out numbers, so two issues created together never share one.
    let repo = state
        .db
        .repos
        .find_one_and_update(doc! {"_id": repo_id}, doc! {"$inc": {"issue_seq": 1}})
        .return_document(ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(repo.issue_seq)
}

pub async fn create(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: RepoPath,
    body: web::Json<CreateIssue>,
) -> Result<HttpResponse, ApiError> {
    let access = load_repo(&state, &path.0, &path.1, viewer.0).await?;
    let user = access.require(Role::Read)?;
    let body = body.into_inner();
    let title = check_title(&body.title)?;
    check_body(&body.body)?;

    // Organising fields need triage; without it they are dropped, as on GitHub.
    let triage = access.has(Role::Triage);
    let labels = if triage { resolve_labels(&state, &access.repo, &body.labels).await? } else { Vec::new() };
    let assignees = if triage { resolve_assignees(&state, &access.repo, &body.assignees).await? } else { Vec::new() };
    let milestone = match body.milestone.filter(|_| triage) {
        Some(n) => Some(resolve_milestone(&state, &access.repo, n).await?),
        None => None,
    };
    let issue_type = match body.issue_type.as_deref().filter(|_| triage) {
        Some(t) => Some(resolve_type(t)?),
        None => None,
    };

    let rendered = markdown::render(&state, &body.body, Some(&access.repo)).await?;
    let now = DateTime::now();
    let issue = IssueDoc {
        id: ObjectId::new(),
        repo_id: access.repo.id,
        number: next_number(&state, access.repo.id).await?,
        title,
        body: body.body.clone(),
        body_html: rendered.html,
        edits: Vec::new(),
        state: IssueState::Open,
        state_reason: None,
        author_id: user.id,
        author_login: user.login.clone(),
        comments: 0,
        created_at: now,
        updated_at: now,
        closed_at: None,
        label_ids: labels.iter().map(|l| l.id).collect(),
        assignee_ids: assignees.iter().map(|u| u.id).collect(),
        milestone_id: milestone.map(|m| m.id),
        issue_type,
        locked: false,
        lock_reason: None,
        pinned: false,
        parent_id: None,
        blocked_by_ids: Vec::new(),
        related_ids: Vec::new(),
        mentioned_ids: rendered.mentions.clone(),
        commenter_ids: Vec::new(),
    };
    state.db.issues.insert_one(&issue).await?;
    record_references(&state, &issue, user, &rendered.issue_refs).await?;
    crate::notify::issue_opened(&state, &access.repo, &issue, user, &rendered.mentions, &assignees).await;
    Ok(HttpResponse::Created().json(one(&state, &access, issue).await?))
}

pub async fn find_issue(state: &AppState, access: &RepoAccess, number: i64) -> Result<IssueDoc, ApiError> {
    state.db.issues.find_one(doc! {"repo_id": access.repo.id, "number": number}).await?.ok_or(ApiError::NotFound)
}

pub async fn get(state: web::Data<AppState>, viewer: Viewer, path: IssuePath) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let issue = find_issue(&state, &access, number).await?;
    // Opening an issue reads its notification.
    if let Some(viewer) = &access.viewer {
        let read = state
            .db
            .notifications
            .update_one(doc! {"user_id": viewer.id, "issue_id": issue.id, "unread": true}, doc! {"$set": {"unread": false}})
            .await?;
        if read.modified_count > 0 {
            crate::notify::signal_user(&state, viewer.id);
        }
    }
    Ok(HttpResponse::Ok().json(one(&state, &access, issue).await?))
}

/// Applies one change set to one issue: permissions, the update itself and the
/// timeline entries. Shared by the single and the bulk endpoint.
pub async fn apply_update(state: &AppState, access: &RepoAccess, issue: &IssueDoc, body: UpdateIssue) -> Result<(), ApiError> {
    let user = access.user()?;
    let is_author = user.id == issue.author_id;
    let denied = || ApiError::forbidden("You do not have permission to change this issue");
    let mut set = doc! {};
    let mut push = doc! {};
    let mut events: Vec<EventDoc> = Vec::new();
    let mut references: Vec<i64> = Vec::new();
    let mut mentions: Vec<ObjectId> = Vec::new();
    let mut newly_assigned: Vec<UserDoc> = Vec::new();
    let mut closed = false;
    let mut reopened = false;

    if body.title.is_some() || body.body.is_some() {
        if !(is_author || access.has(Role::Write)) {
            return Err(denied());
        }
        if let Some(t) = &body.title {
            let title = check_title(t)?;
            if title != issue.title {
                let mut e = EventDoc::new(issue, user, EventKind::Renamed);
                e.rename = Some(Rename { from: issue.title.clone(), to: title.clone() });
                events.push(e);
                set.insert("title", title);
            }
        }
        if let Some(b) = body.body.as_ref().filter(|b| **b != issue.body) {
            check_body(b)?;
            let rendered = markdown::render(state, b, Some(&access.repo)).await?;
            set.insert("body", b);
            set.insert("body_html", rendered.html);
            let old = EditDoc { editor_login: user.login.clone(), edited_at: DateTime::now(), body: issue.body.clone() };
            push.insert("edits", bson(&old)?);
            references = rendered.issue_refs;
            mentions = rendered.mentions;
            if !mentions.is_empty() {
                state.db.issues.update_one(doc! {"_id": issue.id}, doc! {"$addToSet": {"mentioned_ids": {"$each": mentions.clone()}}}).await?;
            }
        }
    }

    if let Some(new_state) = body.state {
        if !(is_author || access.has(Role::Triage)) {
            return Err(denied());
        }
        match new_state {
            IssueState::Closed => {
                let reason = match body.state_reason {
                    Some(StateReason::Reopened) | None => StateReason::Completed,
                    Some(r) => r,
                };
                if issue.state == IssueState::Open || issue.state_reason != Some(reason) {
                    set.insert("state", bson(&new_state)?);
                    set.insert("state_reason", bson(&reason)?);
                    if issue.state == IssueState::Open {
                        set.insert("closed_at", DateTime::now());
                        closed = true;
                    }
                    let mut e = EventDoc::new(issue, user, EventKind::Closed);
                    e.state_reason = Some(reason);
                    events.push(e);
                }
            }
            IssueState::Open => {
                if issue.state == IssueState::Closed {
                    set.insert("state", bson(&new_state)?);
                    set.insert("state_reason", bson(&StateReason::Reopened)?);
                    set.insert("closed_at", Bson::Null);
                    events.push(EventDoc::new(issue, user, EventKind::Reopened));
                    reopened = true;
                }
            }
        }
    }

    let organising = body.labels.is_some() || body.assignees.is_some() || body.milestone.is_some() || body.issue_type.is_some();
    if organising && !access.has(Role::Triage) {
        return Err(denied());
    }
    if let Some(names) = &body.labels {
        let labels = resolve_labels(state, &access.repo, names).await?;
        let new_ids: Vec<ObjectId> = labels.iter().map(|l| l.id).collect();
        for l in labels.iter().filter(|l| !issue.label_ids.contains(&l.id)) {
            let mut e = EventDoc::new(issue, user, EventKind::Labeled);
            e.label = Some(LabelRef { name: l.name.clone(), color: l.color.clone() });
            events.push(e);
        }
        let removed: Vec<ObjectId> = issue.label_ids.iter().filter(|id| !new_ids.contains(id)).copied().collect();
        if !removed.is_empty() {
            let docs: Vec<LabelDoc> = state.db.labels.find(doc! {"_id": {"$in": removed}}).await?.try_collect().await?;
            for l in docs {
                let mut e = EventDoc::new(issue, user, EventKind::Unlabeled);
                e.label = Some(LabelRef { name: l.name, color: l.color });
                events.push(e);
            }
        }
        if new_ids != issue.label_ids {
            set.insert("label_ids", new_ids);
        }
    }
    if let Some(logins) = &body.assignees {
        let users = resolve_assignees(state, &access.repo, logins).await?;
        let new_ids: Vec<ObjectId> = users.iter().map(|u| u.id).collect();
        for u in users.iter().filter(|u| !issue.assignee_ids.contains(&u.id)) {
            let mut e = EventDoc::new(issue, user, EventKind::Assigned);
            e.assignee_login = Some(u.login.clone());
            events.push(e);
            newly_assigned.push(u.clone());
        }
        let removed: Vec<ObjectId> = issue.assignee_ids.iter().filter(|id| !new_ids.contains(id)).copied().collect();
        if !removed.is_empty() {
            let docs: Vec<UserDoc> = state.db.users.find(doc! {"_id": {"$in": removed}}).await?.try_collect().await?;
            for u in docs {
                let mut e = EventDoc::new(issue, user, EventKind::Unassigned);
                e.assignee_login = Some(u.login);
                events.push(e);
            }
        }
        if new_ids != issue.assignee_ids {
            set.insert("assignee_ids", new_ids);
        }
    }
    if let Some(choice) = body.milestone {
        let new = match choice {
            Some(n) => Some(resolve_milestone(state, &access.repo, n).await?),
            None => None,
        };
        if new.as_ref().map(|m| m.id) != issue.milestone_id {
            if let Some(old_id) = issue.milestone_id {
                if let Some(old) = state.db.milestones.find_one(doc! {"_id": old_id}).await? {
                    let mut e = EventDoc::new(issue, user, EventKind::Demilestoned);
                    e.milestone = Some(old.title);
                    events.push(e);
                }
            }
            match &new {
                Some(m) => {
                    let mut e = EventDoc::new(issue, user, EventKind::Milestoned);
                    e.milestone = Some(m.title.clone());
                    events.push(e);
                    set.insert("milestone_id", m.id);
                }
                None => {
                    set.insert("milestone_id", Bson::Null);
                }
            }
        }
    }
    if let Some(choice) = &body.issue_type {
        let new = match choice {
            Some(t) => Some(resolve_type(t)?),
            None => None,
        };
        if new != issue.issue_type {
            let mut e = EventDoc::new(issue, user, EventKind::TypeChanged);
            e.detail = new.clone();
            events.push(e);
            set.insert("issue_type", new.map_or(Bson::Null, Bson::String));
        }
    }

    if set.is_empty() {
        return Ok(());
    }
    set.insert("updated_at", DateTime::now());
    let mut update = doc! {"$set": set};
    if !push.is_empty() {
        update.insert("$push", push);
    }
    state.db.issues.update_one(doc! {"_id": issue.id}, update).await?;
    if !events.is_empty() {
        state.db.events.insert_many(events).await?;
    }
    record_references(state, issue, user, &references).await?;
    crate::notify::issue_changed(state, &access.repo, issue, user, closed, reopened, &mentions, &newly_assigned).await;
    Ok(())
}

pub async fn update(
    state: web::Data<AppState>,
    viewer: Viewer,
    path: IssuePath,
    body: web::Json<UpdateIssue>,
) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let issue = find_issue(&state, &access, number).await?;
    apply_update(&state, &access, &issue, body.into_inner()).await?;
    let issue = find_issue(&state, &access, number).await?;
    Ok(HttpResponse::Ok().json(one(&state, &access, issue).await?))
}

/// Removes an issue and everything hanging off it.
pub async fn purge(state: &AppState, issue: &IssueDoc) -> Result<(), ApiError> {
    let comments: Vec<crate::models::CommentDoc> = state.db.comments.find(doc! {"issue_id": issue.id}).await?.try_collect().await?;
    let mut subjects: Vec<ObjectId> = comments.iter().map(|c| c.id).collect();
    subjects.push(issue.id);
    state.db.reactions.delete_many(doc! {"subject_id": {"$in": subjects}}).await?;
    state.db.comments.delete_many(doc! {"issue_id": issue.id}).await?;
    state.db.project_items.delete_many(doc! {"issue_id": issue.id}).await?;
    state.db.events.delete_many(doc! {"$or": [{"issue_id": issue.id}, {"source_issue_id": issue.id}]}).await?;
    state.db.issues.update_many(doc! {"parent_id": issue.id}, doc! {"$set": {"parent_id": Bson::Null}}).await?;
    state.db.issues.update_many(doc! {"blocked_by_ids": issue.id}, doc! {"$pull": {"blocked_by_ids": issue.id}}).await?;
    state.db.issues.update_many(doc! {"related_ids": issue.id}, doc! {"$pull": {"related_ids": issue.id}}).await?;
    crate::notify::issue_deleted(state, issue).await;
    state.db.issues.delete_one(doc! {"_id": issue.id}).await?;
    Ok(())
}

pub async fn delete(state: web::Data<AppState>, viewer: Viewer, path: IssuePath) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    access.require(Role::Admin)?;
    let issue = find_issue(&state, &access, number).await?;
    purge(&state, &issue).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// `GET /issues/{number}/edits`: earlier versions of the body, newest first.
pub async fn edits(state: web::Data<AppState>, viewer: Viewer, path: IssuePath) -> Result<HttpResponse, ApiError> {
    let (owner, name, number) = path.into_inner();
    let access = load_repo(&state, &owner, &name, viewer.0).await?;
    let issue = find_issue(&state, &access, number).await?;
    Ok(HttpResponse::Ok().json(edits_dto(issue.edits)))
}
