//! Types shared by the server and the CLI: API request and response bodies.

pub mod query;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Default port of the server, also the CLI's default host port.
pub const DEFAULT_PORT: u16 = 23500;

/// `GET /api/health`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub status: String,
    pub version: String,
}

/// Body of every non-2xx answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<FieldError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

/// Repository role, lowest to highest. The derive order is the permission order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Read,
    Triage,
    Write,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateReason {
    Completed,
    NotPlanned,
    Duplicate,
    Reopened,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub login: String,
    pub name: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

/// `GET /api/user`: the signed-in account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUser {
    pub login: String,
    pub name: Option<String>,
    pub email: String,
    pub is_admin: bool,
    pub email_notifications: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateUser {
    pub name: Option<String>,
    pub email_notifications: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignupRequest {
    pub login: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    /// Username or email.
    pub login: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub owner: String,
    pub name: String,
    pub description: String,
    pub private: bool,
    pub created_at: DateTime<Utc>,
    pub open_issues_count: u64,
    /// The caller's role; `None` when not signed in.
    pub viewer_role: Option<Role>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRepo {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub private: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateRepo {
    pub name: Option<String>,
    pub description: Option<String>,
    pub private: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collaborator {
    pub login: String,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCollaborator {
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRef {
    pub login: String,
}

/// How a comment's author relates to the repository; shown as a badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Association {
    Owner,
    Collaborator,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub body: String,
    /// Rendered, sanitized markdown.
    pub body_html: String,
    pub state: IssueState,
    pub state_reason: Option<StateReason>,
    pub author: UserRef,
    pub author_association: Association,
    /// How many times the body was edited.
    pub edit_count: usize,
    pub comments: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub labels: Vec<Label>,
    pub assignees: Vec<UserRef>,
    pub milestone: Option<MilestoneRef>,
    /// One of [`ISSUE_TYPES`].
    pub issue_type: Option<String>,
    pub locked: bool,
    pub lock_reason: Option<String>,
    pub pinned: bool,
    pub parent: Option<IssueRef>,
    pub sub_issues: Vec<IssueRef>,
    /// Issues that must be finished before this one can be.
    #[serde(default)]
    pub blocked_by: Vec<IssueRef>,
    /// Issues waiting on this one.
    #[serde(default)]
    pub blocking: Vec<IssueRef>,
    /// Issues linked as related; the link shows on both.
    #[serde(default)]
    pub related: Vec<IssueRef>,
    pub reactions: Vec<ReactionGroup>,
    pub viewer_can_edit: bool,
    pub viewer_can_close: bool,
    /// Triage and up: labels, assignees, milestone, type, lock.
    pub viewer_can_triage: bool,
    /// False when the conversation is locked and the viewer is not a collaborator.
    pub viewer_can_comment: bool,
    /// Whether the viewer gets notifications for this issue. Only set on single-issue reads.
    pub viewer_subscribed: bool,
}

/// `GET /api/repos/{owner}/{repo}/issues`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueList {
    /// Issues matching the query in the requested state.
    pub total_count: u64,
    pub open_count: u64,
    pub closed_count: u64,
    pub page: u64,
    pub per_page: u64,
    pub items: Vec<Issue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIssue {
    pub title: String,
    #[serde(default)]
    pub body: String,
    /// The fields below need triage access; they are ignored without it, as on GitHub.
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub assignees: Vec<String>,
    #[serde(default)]
    pub milestone: Option<i64>,
    #[serde(default)]
    pub issue_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateIssue {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<IssueState>,
    pub state_reason: Option<StateReason>,
    /// Replaces the whole set.
    pub labels: Option<Vec<String>>,
    /// Replaces the whole set.
    pub assignees: Option<Vec<String>>,
    /// Milestone number; `null` clears it. Absent leaves it alone.
    #[serde(default, deserialize_with = "double_option", skip_serializing_if = "Option::is_none")]
    pub milestone: Option<Option<i64>>,
    /// `null` clears it. Absent leaves it alone.
    #[serde(default, deserialize_with = "double_option", skip_serializing_if = "Option::is_none")]
    pub issue_type: Option<Option<String>>,
}

/// Tells "field absent" (`None`) from "field is null" (`Some(None)`).
pub fn double_option<'de, T, D>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Option::<T>::deserialize(de).map(Some)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author: UserRef,
    pub author_association: Association,
    pub body: String,
    pub body_html: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub edit_count: usize,
    pub reactions: Vec<ReactionGroup>,
    pub viewer_can_edit: bool,
    pub viewer_can_delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentBody {
    pub body: String,
}

/// An earlier version of an issue body or comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edit {
    pub editor: UserRef,
    /// When this version was replaced.
    pub edited_at: DateTime<Utc>,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Closed,
    Reopened,
    Renamed,
    CrossReferenced,
    Labeled,
    Unlabeled,
    Assigned,
    Unassigned,
    Milestoned,
    Demilestoned,
    Locked,
    Unlocked,
    Pinned,
    Unpinned,
    Transferred,
    SubIssueAdded,
    SubIssueRemoved,
    ParentAdded,
    ParentRemoved,
    BlockedByAdded,
    BlockedByRemoved,
    BlockingAdded,
    BlockingRemoved,
    RelatedAdded,
    RelatedRemoved,
    TypeChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rename {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelRef {
    pub name: String,
    /// Six hex digits, no `#`.
    pub color: String,
}

/// Another issue, as shown inside a timeline entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueRef {
    pub number: i64,
    pub title: String,
    pub state: IssueState,
    pub state_reason: Option<StateReason>,
}

/// Something that happened to an issue other than a comment. Which optional
/// fields are set depends on `event`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event: EventKind,
    pub actor: UserRef,
    pub created_at: DateTime<Utc>,
    pub state_reason: Option<StateReason>,
    pub rename: Option<Rename>,
    /// The issue that mentioned this one, or the sub-issue / parent involved.
    pub source: Option<IssueRef>,
    pub label: Option<LabelRef>,
    pub assignee: Option<UserRef>,
    pub milestone: Option<String>,
    /// Free text: new issue type, lock reason, transfer origin.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimelineItem {
    Comment(Comment),
    Event(Event),
}

/// `POST /api/markdown`: preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownRequest {
    pub text: String,
    /// `owner/name`, so `#12` and `@user` resolve.
    pub repo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownResponse {
    pub html: String,
}

/// The issue types every repository has.
pub const ISSUE_TYPES: &[&str] = &["Bug", "Feature", "Task"];

/// Emoji reactions, by their GitHub API names.
pub const REACTIONS: &[&str] = &["+1", "-1", "laugh", "hooray", "confused", "heart", "rocket", "eyes"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionGroup {
    pub content: String,
    pub count: usize,
    pub viewer_reacted: bool,
    /// Who reacted, for the tooltip (first few).
    pub users: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    /// Six hex digits, no `#`.
    pub color: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelWithCount {
    #[serde(flatten)]
    pub label: Label,
    pub open_issues: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLabel {
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateLabel {
    pub new_name: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneRef {
    pub number: i64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub number: i64,
    pub title: String,
    pub description: String,
    pub state: IssueState,
    pub due_on: Option<DateTime<Utc>>,
    pub open_issues: u64,
    pub closed_issues: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMilestone {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub due_on: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateMilestone {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "double_option", skip_serializing_if = "Option::is_none")]
    pub due_on: Option<Option<DateTime<Utc>>>,
    pub state: Option<IssueState>,
}

/// `POST /issues/bulk`: the same change on many issues.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BulkUpdate {
    pub numbers: Vec<i64>,
    pub state: Option<IssueState>,
    pub state_reason: Option<StateReason>,
    #[serde(default)]
    pub add_labels: Vec<String>,
    #[serde(default)]
    pub remove_labels: Vec<String>,
    #[serde(default)]
    pub add_assignees: Vec<String>,
    #[serde(default)]
    pub remove_assignees: Vec<String>,
    #[serde(default, deserialize_with = "double_option", skip_serializing_if = "Option::is_none")]
    pub milestone: Option<Option<i64>>,
}

pub const LOCK_REASONS: &[&str] = &["off-topic", "too heated", "resolved", "spam"];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockRequest {
    pub lock_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubIssueRequest {
    pub number: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    /// `owner/name` of the destination.
    pub repo: String,
}

/// Where an issue lives after a transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueLocation {
    pub owner: String,
    pub repo: String,
    pub number: i64,
}

/// A starting point offered on the new issue page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub issue_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub url: String,
    pub name: String,
    pub content_type: String,
    pub size: u64,
    /// Ready to paste into a comment.
    pub markdown: String,
}

/// One row of the notification inbox: the latest activity on one issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    /// `owner/name`
    pub repo: String,
    pub issue: IssueRef,
    /// Why the viewer is told: `mention`, `assign`, `author`, `comment`, `subscribed`, `state_change`.
    pub reason: String,
    pub unread: bool,
    pub done: bool,
    pub last_actor: UserRef,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationList {
    pub unread_count: u64,
    pub total_count: u64,
    pub items: Vec<Notification>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateNotification {
    pub unread: Option<bool>,
    pub done: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub subscribed: bool,
}

// ---- Projects ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    SingleSelect,
    Text,
    Number,
    Date,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldOption {
    pub id: String,
    pub name: String,
    /// One of [`OPTION_COLORS`].
    pub color: String,
}

pub const OPTION_COLORS: &[&str] = &["gray", "blue", "green", "yellow", "orange", "red", "pink", "purple"];

/// The id of the built-in Status field every project has; the board groups by it.
pub const STATUS_FIELD: &str = "status";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectField {
    pub id: String,
    pub name: String,
    pub kind: FieldKind,
    /// Only for `single_select`.
    #[serde(default)]
    pub options: Vec<FieldOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub number: i64,
    pub title: String,
    pub description: String,
    pub closed: bool,
    pub item_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectItem {
    pub id: String,
    pub issue: Issue,
    /// Field id to value: an option id, text, a number, or a `YYYY-MM-DD` date.
    pub values: serde_json::Map<String, serde_json::Value>,
    /// Order inside its status column.
    pub position: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(flatten)]
    pub summary: ProjectSummary,
    pub fields: Vec<ProjectField>,
    pub items: Vec<ProjectItem>,
    /// Write access: rename, fields, delete.
    pub viewer_can_admin: bool,
    /// Triage access: add, move and edit items.
    pub viewer_can_edit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProject {
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateProject {
    pub title: Option<String>,
    pub description: Option<String>,
    pub closed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateField {
    pub name: String,
    pub kind: FieldKind,
    /// Option names, for `single_select`.
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionInput {
    /// Present to keep (and maybe rename) an existing option, absent to add one.
    pub id: Option<String>,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateField {
    pub name: Option<String>,
    /// The complete new option list, in order. Options left out are removed.
    pub options: Option<Vec<OptionInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddItem {
    pub issue_number: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateItem {
    /// Field id to new value; `null` clears a field.
    pub values: Option<serde_json::Map<String, serde_json::Value>>,
    /// Move within the item's status column: the id of the item to sit before,
    /// or `null` for the end. Absent leaves the order alone.
    #[serde(default, deserialize_with = "double_option", skip_serializing_if = "Option::is_none")]
    pub before: Option<Option<String>>,
}

/// A project an issue belongs to, for the issue sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueProject {
    pub number: i64,
    pub title: String,
    pub item_id: String,
    pub status: Option<FieldOption>,
}

// ---- Personal access tokens ----

/// A token as listed in settings; the secret itself is only shown when created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    pub id: String,
    pub name: String,
    /// The first characters, enough to recognise it.
    pub prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateToken {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewToken {
    #[serde(flatten)]
    pub info: AccessToken,
    /// The secret. It cannot be read again.
    pub token: String,
}

/// `POST /api/auth/token`: trades a password for a new token, for `it auth login`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLogin {
    pub login: String,
    pub password: String,
    /// What the token is called in settings, e.g. `it CLI on laptop`.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLoginResult {
    pub login: String,
    pub token: String,
}
